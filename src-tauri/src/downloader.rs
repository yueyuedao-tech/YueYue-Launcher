use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 小于这个体积不值得分段：切段/合并的固定开销盖过并发收益
const MIN_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_THREADS: usize = 8;
const DEFAULT_THREADS: usize = 4;
/// 进度轮询间隔：250ms 够顺滑，也不至于把 CPU 打满
const POLL_MS: u64 = 250;

/// 一个进行中的下载任务。
/// 分段的每个 curl 都是独立子进程，各自写自己的 `.partN`，全部成功后再按序拼成目标文件。
struct Job {
    /// 探测进程（`Range: 0-0`）；单独放一个字段，因为它可能以 63 退出（服务器不支持分段），
    /// 不能混进「分段全部完成」的判定里
    probe: Mutex<Option<Arc<Mutex<Child>>>>,
    children: Mutex<Vec<Arc<Mutex<Child>>>>,
    /// 临时分段文件；单连接时就是目标文件本身
    parts: Mutex<Vec<PathBuf>>,
    dest: PathBuf,
    total: AtomicU64,
    threads: AtomicUsize,
    cancelled: AtomicBool,
    /// 发起方给的唯一标识，随 download-done 原样回传
    token: Option<String>,
}

/// 面板要展示的任务快照
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub file_name: String,
    pub name: String,
    pub url: String,
    /// downloading | done | error
    pub status: String,
    pub received: u64,
    pub total: u64,
    pub percent: f64,
    /// 字节/秒（指数平滑）
    pub speed: f64,
    pub threads: usize,
    pub path: String,
    pub code: i32,
    pub started_at: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub file_name: String,
    pub received: u64,
    pub total: u64,
    pub percent: f64,
    pub speed: f64,
    pub threads: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DonePayload {
    pub file_name: String,
    pub path: String,
    /// 前端发起下载时带的唯一标识，原样回传。
    /// 同一个版本可能被下载两次（换个游戏名再建一个实例），光靠文件名分不清是哪一次，
    /// 于是新建实例的「意图」会被后一次覆盖 —— 这个 token 就是为此加的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub file_name: String,
    pub code: i32,
}

fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tasks() -> &'static Mutex<HashMap<String, TaskInfo>> {
    static TABLE: OnceLock<Mutex<HashMap<String, TaskInfo>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn task_update(id: &str, f: impl FnOnce(&mut TaskInfo)) {
    if let Some(t) = tasks().lock().unwrap().get_mut(id) {
        f(t);
    }
}

fn registered(id: &str, job: &Arc<Job>) -> bool {
    jobs()
        .lock()
        .unwrap()
        .get(id)
        .map(|j| Arc::ptr_eq(j, job))
        .unwrap_or(false)
}

/// 任务结束（成功/失败/取消）必须摘掉登记，否则同一个文件名会永远被判定成
/// 「已在下载中」，之后再点下载全部失败。只在仍是我们自己时才摘，避免踩到新任务。
fn unregister(id: &str, job: &Arc<Job>) {
    let mut table = jobs().lock().unwrap();
    if table.get(id).map(|j| Arc::ptr_eq(j, job)).unwrap_or(false) {
        table.remove(id);
    }
}

/* ================= 探测与切段 ================= */

/// 从 `curl -D -` 的响应头里解析（文件总长, 是否支持分段）。
/// 抽成纯函数便于单测：真实响应里 Content-Length / Content-Range / Accept-Ranges 混在一起。
fn parse_probe(head: &str) -> (u64, bool) {
    let mut content_length = 0u64;
    let mut content_range: Option<u64> = None;
    let mut last_status = String::new();
    for line in head.lines() {
        let l = line.trim();
        if l.starts_with("HTTP/") {
            last_status = l.to_string();
            continue;
        }
        let lower = l.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-range:") {
            // 形如 `bytes 0-0/85632441`；`*` 表示总长未知
            if let Some((_, t)) = v.trim().split_once('/') {
                if let Ok(n) = t.trim().parse::<u64>() {
                    content_range = Some(n);
                }
            }
        } else if let Some(v) = lower.strip_prefix("content-length:") {
            if let Ok(n) = v.trim().parse::<u64>() {
                content_length = n;
            }
        }
    }
    // 只有真的回了 206 才认为服务器支持 Range：光有 Accept-Ranges 也可能被忽略
    let partial = last_status.contains(" 206");
    let total = content_range.unwrap_or(content_length);
    (total, partial && content_range.is_some())
}

/// 把 `[0, total)` 切成 n 段闭区间；调用方保证 total > 0 且 n > 1。
/// 除不尽时余数全部给最后一段，保证每段非空且首尾相接、无重叠。
fn split_ranges(total: u64, n: usize) -> Vec<(u64, u64)> {
    if total == 0 {
        return Vec::new();
    }
    if n <= 1 {
        return vec![(0, total - 1)];
    }
    let n = n.min(total as usize).max(1);
    let chunk = total / n as u64;
    (0..n)
        .map(|i| {
            let start = i as u64 * chunk;
            let end = if i == n - 1 {
                total - 1
            } else {
                (i as u64 + 1) * chunk - 1
            };
            (start, end)
        })
        .collect()
}

fn part_path(dest: &Path, i: usize) -> PathBuf {
    PathBuf::from(format!("{}.part{i}", dest.display()))
}

fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn base_curl(proxy: &str) -> std::process::Command {
    let mut cmd = crate::cmdutil::no_console("curl");
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd
}

/// 用一个 `Range: bytes=0-0` 的请求同时问出「总长」和「支不支持分段」。
/// `--max-filesize 1` 是安全阀：服务器若忽略 Range 回 200，curl 会在读正文前就中止，
/// 不会为了探测把整个 85MB 拖下来。
fn probe(url: &str, proxy: &str, job: &Job) -> (u64, bool) {
    let mut cmd = base_curl(proxy);
    cmd.args([
        "-sL",
        "-r",
        "0-0",
        "--max-filesize",
        "1",
        "-D",
        "-",
        "-o",
        null_device(),
        "--max-time",
        "25",
    ]);
    cmd.arg(url);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let Ok(child) = cmd.spawn() else {
        return (0, false);
    };

    // 登记探测进程，让 stop 能立刻杀掉它（否则取消会卡在这 25s 的网络窗口里）
    let handle = Arc::new(Mutex::new(child));
    *job.probe.lock().unwrap() = Some(handle.clone());

    // 先把 stdout 取出来再读：持锁读会让 stop_download 干等
    let stdout = handle.lock().unwrap().stdout.take();
    let mut head = String::new();
    if let Some(mut so) = stdout {
        let _ = so.read_to_string(&mut head);
    }
    let _ = handle.lock().unwrap().wait();
    parse_probe(&head)
}

/* ================= 进程与文件 ================= */

fn kill_child(handle: &Arc<Mutex<Child>>) {
    let pid = {
        let g = handle.lock().unwrap();
        g.id()
    };
    #[cfg(windows)]
    {
        let _ = crate::cmdutil::no_console("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let mut g = handle.lock().unwrap();
        let _ = g.kill();
        let _ = g.wait();
    }
    #[cfg(windows)]
    let _ = pid;
}

fn kill_all(job: &Job) {
    let mut kids: Vec<Arc<Mutex<Child>>> = job.children.lock().unwrap().clone();
    if let Some(p) = job.probe.lock().unwrap().clone() {
        kids.push(p);
    }
    for k in kids {
        kill_child(&k);
    }
}

/// Windows 上刚被 taskkill 掉的进程不一定已经释放文件句柄，紧接着 remove_file 会失败。
/// 不重试就会留下半截的分段文件（实测：取消后 4 个 .partN 全部残留）。
fn remove_with_retry(path: &Path) {
    for i in 0..12 {
        if std::fs::remove_file(path).is_ok() || !path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(if i < 4 { 60 } else { 150 }));
    }
}

/// 删掉所有临时产物（分段 + 暂存文件）。
/// 注意：**绝不动最终目标文件**——重下同一个版本时，已有实例的 jar 指向的就是它，
/// 下载失败或取消都不该把老文件删掉。
fn cleanup_files(job: &Job) {
    let parts = std::mem::take(&mut *job.parts.lock().unwrap());
    for p in parts {
        remove_with_retry(&p);
    }
    remove_with_retry(&staging_path(&job.dest));
}

/// 下载暂存文件：先写它，全部成功再改名成目标文件（同名重下不会先删老文件）
fn staging_path(dest: &Path) -> PathBuf {
    PathBuf::from(format!("{}.downloading", dest.display()))
}

/// 按顺序把分段拼成目标文件
fn merge_parts(parts: &[PathBuf], dest: &Path) -> std::io::Result<()> {
    let mut out = std::fs::File::create(dest)?;
    for p in parts {
        let mut f = std::fs::File::open(p)?;
        std::io::copy(&mut f, &mut out)?;
    }
    out.flush()?;
    let _ = out.sync_all();
    Ok(())
}

/// 暂存文件改名成目标文件；Windows 上目标已存在时 rename 会失败，先删再改
fn promote(staging: &Path, dest: &Path) -> std::io::Result<()> {
    if dest.exists() {
        let _ = remove_with_retry(dest);
    }
    std::fs::rename(staging, dest)
}

fn resolve_dir(download_dir: &str) -> PathBuf {
    if !download_dir.trim().is_empty() {
        return PathBuf::from(download_dir.trim());
    }
    crate::cmdutil::app_root().join("downloads")
}

/* ================= 任务主体 ================= */

fn finish_error(app: &AppHandle, id: &str, job: &Arc<Job>, code: i32) {
    kill_all(job);
    cleanup_files(job);
    unregister(id, job);
    task_update(id, |t| {
        t.status = "error".into();
        t.code = code;
        t.speed = 0.0;
    });
    let _ = app.emit(
        "download-error",
        ErrorPayload {
            file_name: id.to_string(),
            code,
        },
    );
}

fn run_job(app: AppHandle, id: String, url: String, proxy: String, job: Arc<Job>) {
    let (total, ranges) = probe(&url, &proxy, &job);
    if job.cancelled.load(Ordering::Relaxed) || !registered(&id, &job) {
        cleanup_files(&job);
        unregister(&id, &job);
        return;
    }
    job.total.store(total, Ordering::Relaxed);
    task_update(&id, |t| t.total = total);

    let want = job.threads.load(Ordering::Relaxed);
    let seg = ranges && total >= MIN_SEGMENT_BYTES && want > 1;
    let planned: Vec<(u64, u64)> = if seg {
        split_ranges(total, want)
    } else {
        Vec::new()
    };
    let use_parts = planned.len() > 1;

    // 分段 → 各自的 .partN；单连接 → 直接写暂存文件（成功后再改名成目标）
    let staging = staging_path(&job.dest);
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut spec: Vec<Option<(u64, u64)>> = Vec::new();
    if use_parts {
        for (i, r) in planned.iter().enumerate() {
            paths.push(part_path(&job.dest, i));
            spec.push(Some(*r));
        }
    } else {
        paths.push(staging.clone());
        spec.push(None);
    }
    *job.parts.lock().unwrap() = paths.clone();
    job.threads
        .store(if use_parts { paths.len() } else { 1 }, Ordering::Relaxed);
    task_update(&id, |t| {
        t.threads = job.threads.load(Ordering::Relaxed);
        t.received = 0;
        t.percent = 0.0;
    });

    // 起进程
    let mut spawned: Vec<Arc<Mutex<Child>>> = Vec::new();
    for (p, r) in paths.iter().zip(spec.iter()) {
        let mut cmd = base_curl(&proxy);
        cmd.args(["-L", "--fail", "--retry", "2", "-sS", "-o"]);
        cmd.arg(p);
        if let Some((s, e)) = r {
            cmd.args(["-r", &format!("{s}-{e}")]);
        }
        cmd.arg(&url);
        cmd.stdin(Stdio::null());
        // 多段并发的 stderr 交错没有可读性，错误一律靠退出码判断
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
        match cmd.spawn() {
            Ok(c) => spawned.push(Arc::new(Mutex::new(c))),
            Err(_) => {
                *job.children.lock().unwrap() = spawned;
                finish_error(&app, &id, &job, -1);
                return;
            }
        }
    }
    *job.children.lock().unwrap() = spawned;

    // 轮询：累计各段已写字节 → 进度 + 速度
    let mut last_bytes = 0u64;
    let mut last_t = Instant::now();
    let mut speed = 0f64;
    loop {
        std::thread::sleep(Duration::from_millis(POLL_MS));
        if job.cancelled.load(Ordering::Relaxed) || !registered(&id, &job) {
            cleanup_files(&job);
            unregister(&id, &job);
            return;
        }

        let received: u64 = paths
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        let dt = last_t.elapsed().as_secs_f64();
        if dt > 0.05 {
            let inst = received.saturating_sub(last_bytes) as f64 / dt;
            speed = if speed <= 0.0 {
                inst
            } else {
                speed * 0.6 + inst * 0.4
            };
            last_bytes = received;
            last_t = Instant::now();
        }
        let total_now = job.total.load(Ordering::Relaxed);
        let percent = if total_now > 0 {
            (received as f64 / total_now as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let threads = job.threads.load(Ordering::Relaxed);
        task_update(&id, |t| {
            t.received = received;
            t.total = total_now;
            t.percent = percent;
            t.speed = speed;
        });
        let _ = app.emit(
            "download-progress",
            ProgressPayload {
                file_name: id.clone(),
                received,
                total: total_now,
                percent,
                speed,
                threads,
            },
        );

        // 任一段失败 → 立刻收摊，不必等其余段跑完
        let mut running = 0usize;
        let mut failed: Option<i32> = None;
        {
            let kids = job.children.lock().unwrap();
            for c in kids.iter() {
                match c.lock().unwrap().try_wait() {
                    Ok(Some(st)) => {
                        if !st.success() && failed.is_none() {
                            failed = Some(st.code().unwrap_or(-1));
                        }
                    }
                    Ok(None) => running += 1,
                    Err(_) => {
                        if failed.is_none() {
                            failed = Some(-1);
                        }
                    }
                }
            }
        }
        if let Some(code) = failed {
            finish_error(&app, &id, &job, code);
            return;
        }
        if running > 0 {
            continue;
        }

        // 全部退出且都成功：校验字节数 → 合并
        let final_bytes: u64 = paths
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        if total_now > 0 && final_bytes != total_now {
            // curl 可能以 0 退出却没写全（幽灵失败），这类必须报错而不是当成功
            finish_error(&app, &id, &job, -2);
            return;
        }
        if use_parts {
            // 分段先拼到暂存文件，成功后再整体改名过去
            if let Err(_e) = merge_parts(&paths, &staging) {
                finish_error(&app, &id, &job, -3);
                return;
            }
            for p in &paths {
                remove_with_retry(p);
            }
        }
        if !staging.exists() {
            finish_error(&app, &id, &job, -4);
            return;
        }
        if let Err(_e) = promote(&staging, &job.dest) {
            finish_error(&app, &id, &job, -5);
            return;
        }
        job.parts.lock().unwrap().clear();
        let path = job.dest.to_string_lossy().into_owned();
        unregister(&id, &job);
        task_update(&id, |t| {
            t.status = "done".into();
            t.percent = 100.0;
            t.speed = 0.0;
            t.received = t.total.max(final_bytes);
            t.path = path.clone();
        });
        let _ = app.emit(
            "download-done",
            DonePayload {
                file_name: id.clone(),
                path,
                token: job.token.clone(),
            },
        );
        return;
    }
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    url: String,
    file_name: String,
    proxy: String,
    download_dir: String,
    threads: Option<u32>,
    name: Option<String>,
    token: Option<String>,
) -> Result<(), String> {
    let id = crate::instances::validate_id(&file_name)?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("URL 必须以 http(s):// 开头: {url}"));
    }
    let n = threads
        .unwrap_or(DEFAULT_THREADS as u32)
        .clamp(1, MAX_THREADS as u32) as usize;

    let dir = resolve_dir(&download_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&id);
    // 只清上一次留下的暂存文件；**不动** dest —— 同名重下不能先把老 jar 删了，
    // 否则下载中途失败会让指向它的现有实例全都起不来。
    remove_with_retry(&staging_path(&dest));

    let job = Arc::new(Job {
        probe: Mutex::new(None),
        children: Mutex::new(Vec::new()),
        parts: Mutex::new(Vec::new()),
        dest: dest.clone(),
        total: AtomicU64::new(0),
        threads: AtomicUsize::new(n),
        cancelled: AtomicBool::new(false),
        token: token.clone(),
    });

    // 检查-登记为一个临界区，消除同名并发窗口
    {
        let mut table = jobs().lock().unwrap();
        if table.contains_key(&id) {
            return Err("该文件已在下载中".into());
        }
        table.insert(id.clone(), job.clone());
    }

    // 登记后立刻建任务记录：面板马上就能看到这条，不用等探测
    tasks().lock().unwrap().insert(
        id.clone(),
        TaskInfo {
            file_name: id.clone(),
            name: name.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| id.clone()),
            url: url.clone(),
            status: "downloading".into(),
            threads: n,
            path: dest.to_string_lossy().into_owned(),
            started_at: now_ms(),
            ..Default::default()
        },
    );

    // 探测与分段都在后台线程：主线程不阻塞，取消也不用等网络窗口
    let app_w = app.clone();
    let id_w = id.clone();
    std::thread::spawn(move || run_job(app_w, id_w, url, proxy, job));
    Ok(())
}

#[tauri::command]
pub async fn stop_download(file_name: String) -> Result<(), String> {
    let id = crate::instances::validate_id(&file_name)?;
    let Some(job) = jobs().lock().unwrap().remove(&id) else {
        return Err("该文件未在下载".into());
    };
    job.cancelled.store(true, Ordering::Relaxed);
    kill_all(&job);
    cleanup_files(&job);
    // 取消 = 这条任务不存在了（与前端「取消即复位」的语义一致）
    tasks().lock().unwrap().remove(&id);
    Ok(())
}

/// 面板打开时拉一次，避免只依赖事件而漏掉状态
#[tauri::command]
pub async fn list_downloads() -> Vec<TaskInfo> {
    let mut v: Vec<TaskInfo> = tasks().lock().unwrap().values().cloned().collect();
    v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    v
}

#[tauri::command]
pub async fn clear_download(file_name: String) -> Result<(), String> {
    let id = crate::instances::validate_id(&file_name)?;
    if jobs().lock().unwrap().contains_key(&id) {
        return Err("该任务仍在下载中，请先取消".into());
    }
    tasks().lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub async fn clear_finished_downloads() -> usize {
    let mut t = tasks().lock().unwrap();
    let before = t.len();
    t.retain(|_, v| v.status == "downloading");
    before - t.len()
}

/// 设置里「打开下载目录」用：返回当前真正生效的下载目录（与落盘同一口径）
#[tauri::command]
pub async fn downloads_dir(download_dir: String) -> String {
    resolve_dir(&download_dir).to_string_lossy().into_owned()
}

/// 本地是否已有同一个客户端文件。
///
/// 用途：同一个版本再下一次（换个游戏名建第二个实例）时**直接引用现有文件**建实例，
/// 省掉一次几十 MB 的下载。存在的文件只可能是「完整下载过」的——下载过程写的是
/// `.downloading` 暂存文件，成功后才改名——所以大小对得上即可放心复用。
///
/// - `expected_size` 给了就要求大小完全一致，避免拿到同名但内容不符的残留
/// - 返回绝对路径；没有可复用的返回 None
#[tauri::command]
pub async fn existing_download(
    file_name: String,
    download_dir: String,
    expected_size: Option<u64>,
) -> Option<String> {
    let id = crate::instances::validate_id(&file_name).ok()?;
    let path = resolve_dir(&download_dir).join(&id);
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() == 0 {
        return None;
    }
    if let Some(want) = expected_size {
        if want > 0 && want != meta.len() {
            return None;
        }
    }
    Some(path.to_string_lossy().into_owned())
}

/// 设置里「下载代理 = 系统代理」时，前端用这个值去填下载与索引请求
#[tauri::command]
pub async fn get_system_proxy() -> String {
    crate::cmdutil::system_proxy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_covers_whole_file_without_gap_or_overlap() {
        let total = 85_632_441u64;
        let segs = split_ranges(total, 4);
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0].0, 0, "必须从 0 开始");
        assert_eq!(segs[3].1, total - 1, "必须覆盖到最后一个字节");
        let mut sum = 0u64;
        for w in segs.windows(2) {
            assert_eq!(w[0].1 + 1, w[1].0, "相邻段必须首尾相接");
        }
        for (s, e) in &segs {
            assert!(e >= s, "段不能是负长度");
            sum += e - s + 1;
        }
        assert_eq!(sum, total, "各段长度之和必须等于文件大小");
    }

    #[test]
    fn split_handles_indivisible_and_tiny_inputs() {
        // 10 字节切 4 段：不能出现空段，也不能漏字节
        let segs = split_ranges(10, 4);
        let sum: u64 = segs.iter().map(|(s, e)| e - s + 1).sum();
        assert_eq!(sum, 10);
        assert!(segs.iter().all(|(s, e)| e >= s));
        assert_eq!(segs[0].0, 0);
        assert_eq!(segs.last().unwrap().1, 9);

        // 段数超过字节数 → 每段至少 1 字节
        let segs = split_ranges(3, 8);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs, vec![(0, 0), (1, 1), (2, 2)]);

        // 单线程与 0 长度
        assert_eq!(split_ranges(100, 1), vec![(0, 99)]);
        assert!(split_ranges(0, 4).is_empty());
    }

    #[test]
    fn probe_parses_206_with_content_range() {
        let head = "HTTP/1.1 302 Found\r\nlocation: https://objects.example/x\r\n\r\n\
                    HTTP/1.1 206 Partial Content\r\nContent-Length: 1\r\n\
                    Content-Range: bytes 0-0/85632441\r\nAccept-Ranges: bytes\r\n";
        assert_eq!(parse_probe(head), (85_632_441, true));
    }

    #[test]
    fn probe_falls_back_when_range_is_ignored() {
        // 服务器忽略 Range 回 200：总长还能从 Content-Length 拿到，但不能分段
        let head = "HTTP/1.1 200 OK\r\nContent-Length: 85632441\r\nAccept-Ranges: bytes\r\n";
        assert_eq!(parse_probe(head), (85_632_441, false));

        // 只有 Accept-Ranges 却回 200（谎报）→ 仍按不分段处理
        let head = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nAccept-Ranges: bytes\r\n";
        assert_eq!(parse_probe(head).1, false);

        // 拿不到总长时也不能崩
        assert_eq!(parse_probe(""), (0, false));
        assert_eq!(parse_probe("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/*\r\n"), (0, false));
    }

    #[test]
    fn finished_jobs_must_be_unregistered() {
        // 这条一旦漏掉，同一个文件名会永远被判成「已在下载中」，再也下不动
        let id = "unit-test-unregister.jar";
        let mk = || {
            Arc::new(Job {
                probe: Mutex::new(None),
                children: Mutex::new(Vec::new()),
                parts: Mutex::new(Vec::new()),
                dest: PathBuf::from("x"),
                total: AtomicU64::new(0),
                threads: AtomicUsize::new(1),
                cancelled: AtomicBool::new(false),
                token: None,
            })
        };
        let job = mk();
        jobs().lock().unwrap().insert(id.to_string(), job.clone());
        assert!(registered(id, &job));

        // 另一个同名任务不能把当前这条摘掉
        unregister(id, &mk());
        assert!(registered(id, &job), "不是自己的任务不应被摘除");

        unregister(id, &job);
        assert!(!registered(id, &job), "结束的任务必须摘掉登记");
        assert!(!jobs().lock().unwrap().contains_key(id));
    }

    #[test]
    fn part_paths_are_derived_from_dest() {
        let d = PathBuf::from(r"C:\dl\Mindustry.jar");
        assert!(part_path(&d, 0).to_string_lossy().ends_with("Mindustry.jar.part0"));
        assert!(part_path(&d, 3).to_string_lossy().ends_with("Mindustry.jar.part3"));
    }
}
