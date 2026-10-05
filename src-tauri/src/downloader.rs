use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// 正在下载的文件名 -> curl 进程
fn active() -> &'static Mutex<HashMap<String, Arc<Mutex<Child>>>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Arc<Mutex<Child>>>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 文件名 -> 实际写入路径（供 stop 清理）
fn dests() -> &'static Mutex<HashMap<String, std::path::PathBuf>> {
    static TABLE: OnceLock<Mutex<HashMap<String, std::path::PathBuf>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub file_name: String,
    pub received: u64,
    pub total: u64,
    pub percent: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DonePayload {
    pub file_name: String,
    pub path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub file_name: String,
    pub code: i32,
}

/// download_dir 非空时覆盖默认下载目录
fn resolve_dir(download_dir: &str) -> PathBuf {
    if !download_dir.trim().is_empty() {
        return PathBuf::from(download_dir.trim());
    }
    if cfg!(windows) {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(appdata).join("StarlightLauncher").join("downloads")
    } else {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".local").join("share")
            });
        base.join("starlight-launcher").join("downloads")
    }
}

/// HEAD 请求取最终 Content-Length（跟随重定向），失败返回 0
fn head_content_length(url: &str, proxy: &str) -> u64 {
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-sIL", "--max-time", "20"]);
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd.arg(url);
    let Ok(out) = cmd.output() else { return 0 };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut total = 0u64;
    for line in text.lines() {
        if let Some(v) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            if let Ok(n) = v.trim().parse::<u64>() {
                total = n;
            }
        }
    }
    total
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    url: String,
    file_name: String,
    proxy: String,
    download_dir: String,
) -> Result<(), String> {
    let id = crate::instances::validate_id(&file_name)?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("URL 必须以 http(s):// 开头: {url}"));
    }
    if active().lock().unwrap().contains_key(&id) {
        return Err("该文件已在下载中".into());
    }

    let dir = resolve_dir(&download_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&id);
    if dest.exists() {
        std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
    }
    // 不做阻塞 HEAD：先 spawn 登记（否则 HEAD 最长20s 的空窗期无法取消），总大小由后台线程异步探测
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-L", "--fail", "--retry", "2", "-sS", "-o"]);
    cmd.arg(&dest);
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd.arg(&url);
    cmd.stdin(Stdio::null());
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::null());

    // 检查-启动-登记为一个临界区，消除同名并发窗口
    let mut table = active().lock().unwrap();
    if table.contains_key(&id) {
        return Err("该文件已在下载中".into());
    }
    let child = cmd
        .spawn()
        .map_err(|e| format!("无法执行 curl（系统需自带 curl）: {e}"))?;

    let handle = Arc::new(Mutex::new(child));
    table.insert(id.clone(), handle.clone());
    drop(table);
    dests().lock().unwrap().insert(id.clone(), dest.clone());

    // 总大小异步探测（完成前任何时刻读到 0 → 进度按已收字节显示）
    let total = Arc::new(std::sync::atomic::AtomicU64::new(0));
    {
        let total = total.clone();
        let url2 = url.clone();
        let proxy2 = proxy.clone();
        std::thread::spawn(move || {
            let n = head_content_length(&url2, &proxy2);
            total.store(n, std::sync::atomic::Ordering::Relaxed);
        });
    }

    let app_p = app.clone();
    let id_p = id.clone();
    let dest_p = dest.clone();
    let app_w = app;
    let id_w = id;
    let handle_w = handle;

    // 进度轮询 + 等待退出
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(250));
        // 已被 stop 移除 → 静默退出（不发 done/error）
        let still_ours = {
            let table = active().lock().unwrap();
            table.get(&id_w).map(|a| Arc::ptr_eq(a, &handle_w)).unwrap_or(false)
        };
        if !still_ours {
            break;
        }

        let received = std::fs::metadata(&dest_p).map(|m| m.len()).unwrap_or(0);
        let total_now = total.load(std::sync::atomic::Ordering::Relaxed);
        let percent = if total_now > 0 {
            (received as f64 / total_now as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let _ = app_p.emit(
            "download-progress",
            ProgressPayload { file_name: id_p.clone(), received, total: total_now, percent },
        );

        enum Wait {
            Running,
            Exited(std::process::ExitStatus),
            Failed,
        }
        let outcome = {
            let mut g = handle_w.lock().unwrap();
            match g.try_wait() {
                Ok(None) => Wait::Running,
                Ok(Some(st)) => Wait::Exited(st),
                Err(_) => Wait::Failed,
            }
        };

        if let Wait::Running = outcome {
            continue;
        }

        let mut table = active().lock().unwrap();
        let ours = table
            .get(&id_w)
            .map(|a| Arc::ptr_eq(a, &handle_w))
            .unwrap_or(false);
        if ours {
            table.remove(&id_w);
            drop(table);
            dests().lock().unwrap().remove(&id_w);
            match outcome {
                Wait::Exited(st) if st.success() && dest_p.exists() => {
                    let _ = app_w.emit(
                        "download-done",
                        DonePayload {
                            file_name: id_w.clone(),
                            path: dest_p.to_string_lossy().into_owned(),
                        },
                    );
                }
                other => {
                    let _ = std::fs::remove_file(&dest_p);
                    // 本机 curl 在 --retry 耗尽后可能以 0 退出却无产物，
                    // 此时用 -2 标记「成功退出但无文件」的幽灵失败
                    let code = match other {
                        Wait::Exited(st) => {
                            if st.success() {
                                -2
                            } else {
                                st.code().unwrap_or(-1)
                            }
                        }
                        _ => -1,
                    };
                    let _ = app_w.emit(
                        "download-error",
                        ErrorPayload { file_name: id_w.clone(), code },
                    );
                }
            }
        }
        break;
    });
    Ok(())
}

#[tauri::command]
pub async fn stop_download(file_name: String) -> Result<(), String> {
    let id = crate::instances::validate_id(&file_name)?;
    let Some(handle) = active().lock().unwrap().remove(&id) else {
        return Err("该文件未在下载".into());
    };
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
    // 删除半截文件
    if let Some(p) = dests().lock().unwrap().remove(&id) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}
