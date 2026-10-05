use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CentralItem {
    pub id: String,
    pub name: String,
    /// "file-list" | "github-repo" | "direct-url"
    pub kind: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default = "d_asset")]
    pub asset: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "d_group")]
    pub group: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub size: u64,
    /// 游戏/源 logo，由中心列表服务器下发；缺失时前端用首字母兜底
    #[serde(default)]
    pub logo: String,
    /// "client" | "server"：当前只接客户端，服务端稍后接入
    #[serde(default = "d_scope")]
    pub scope: String,
}

fn d_asset() -> String {
    "Mindustry.jar".into()
}
fn d_group() -> String {
    "默认".into()
}
fn d_scope() -> String {
    "client".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CentralAsset {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CentralVersion {
    pub tag: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub page_url: String,
    /// 已滤除服务端产物，只留客户端下载
    #[serde(default)]
    pub assets: Vec<CentralAsset>,
    /// 被滤掉的服务端资源数（界面据此显示「已滤除 N 个服务端」）
    #[serde(default)]
    pub dropped: usize,
    /// 文件站条目是目录，只能打开不能直接下
    #[serde(default)]
    pub folder: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionIndex {
    /// 上次成功同步的毫秒时间戳；0 = 从未同步
    pub synced_at: u64,
    pub ok: usize,
    pub total: usize,
    pub sync_ms: u64,
    /// 源 id → 版本列表
    #[serde(default)]
    pub sources: BTreeMap<String, Vec<CentralVersion>>,
    /// 源 id → 内容 hash；一致则不覆盖本地
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CentralIndex {
    /// "remote" = 来自中心化服务器；"builtin" = 内置兜底
    pub source: String,
    /// 失败原因（成功为空）
    pub note: String,
    pub items: Vec<CentralItem>,
}

#[derive(Deserialize)]
struct IndexFile {
    #[serde(default)]
    items: Vec<CentralItem>,
}

/// 内置中心索引：服务器不可用时下载中心仍可用。
/// 当前只收客户端源，服务端源待后续接入。
fn builtin_items() -> Vec<CentralItem> {
    vec![
        CentralItem {
            id: "src-mdtbbs-v8".into(),
            name: "Mindustry v8".into(),
            kind: "file-list".into(),
            url: "https://file.mdtbbs.cn/category/Mindustry/v8".into(),
            repo: String::new(),
            asset: d_asset(),
            note: "mdtbbs 文件站分类目录".into(),
            group: "中心".into(),
            tags: vec!["客户端".into(), "文件站".into(), "v8".into()],
            size: 0,
            logo: String::new(),
            scope: d_scope(),
        },
        CentralItem {
            id: "src-minedx".into(),
            name: "MindustryX".into(),
            kind: "github-repo".into(),
            url: "https://github.com/TinyLake/MindustryX/releases".into(),
            repo: "TinyLake/MindustryX".into(),
            asset: d_asset(),
            note: "TinyLake/MindustryX releases".into(),
            group: "中心".into(),
            tags: vec!["客户端".into(), "GitHub".into()],
            size: 0,
            logo: String::new(),
            scope: d_scope(),
        },
    ]
}

/// 逐项裁剪：非法条目直接丢弃，超长字段截断（控制索引体积与注入风险）
fn sanitize(items: Vec<CentralItem>) -> Vec<CentralItem> {
    let mut out: Vec<CentralItem> = Vec::new();
    for mut it in items.into_iter().take(200) {
        it.name = it.name.trim().chars().take(60).collect();
        if it.name.is_empty() {
            continue;
        }
        it.group = it.group.trim().chars().take(24).collect();
        if it.group.is_empty() {
            it.group = "中心".into();
        }
        it.id = it.id.trim().chars().take(48).collect();
        if it.id.is_empty() {
            continue;
        }
        let url_ok = |u: &str| u.starts_with("https://") || u.starts_with("http://");
        match it.kind.as_str() {
            "direct-url" | "file-list" => {
                if !url_ok(&it.url) {
                    continue;
                }
            }
            "github-repo" => {
                if !crate::github::repo_ok(&it.repo) {
                    continue;
                }
            }
            _ => continue,
        }
        if !url_ok(&it.logo) && !it.logo.is_empty() {
            it.logo = String::new();
        }
        if it.scope != "server" {
            it.scope = d_scope();
        }
        it.tags = it
            .tags
            .into_iter()
            .filter(|t| !t.trim().is_empty())
            .map(|t| t.trim().chars().take(16).collect())
            .take(8)
            .collect();
        it.tags.dedup();
        it.note = it.note.trim().chars().take(80).collect();
        out.push(it);
    }
    out
}

fn http_get(url: &str, proxy: &str, headers: &[&str], timeout: u32) -> Result<String, String> {
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-sSL", "--max-time", &timeout.to_string()]);
    for h in headers {
        cmd.args(["-H", h]);
    }
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd.arg(url);
    let out = cmd.output().map_err(|e| format!("无法执行 curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "拉取失败（code {}）",
            out.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn fetch_sync(base: &str, proxy: &str) -> Result<CentralIndex, String> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Ok(CentralIndex {
            source: "builtin".into(),
            note: String::new(),
            items: sanitize(builtin_items()),
        });
    }
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err("中心化服务器地址需以 http(s):// 开头".into());
    }

    let url = format!("{base}/index.json");
    match http_get(&url, proxy, &[], 15)
        .and_then(|text| {
            serde_json::from_str::<IndexFile>(&text).map_err(|e| format!("索引 JSON 解析失败: {e}"))
        })
        .map(|f| sanitize(f.items))
    {
        Ok(items) if !items.is_empty() => Ok(CentralIndex {
            source: "remote".into(),
            note: String::new(),
            items,
        }),
        Ok(_) => Ok(CentralIndex {
            source: "builtin".into(),
            note: "中心化服务器返回空索引，已用内置索引".into(),
            items: sanitize(builtin_items()),
        }),
        Err(e) => Ok(CentralIndex {
            source: "builtin".into(),
            note: format!("{e}；已用内置索引"),
            items: sanitize(builtin_items()),
        }),
    }
}

/// 查询中心化服务器的索引清单（`{base}/index.json`）。
/// base 为空或拉取失败时退回内置索引，保证下载中心始终可用。
///
/// 必须是 async + spawn_blocking：同步命令会在主线程执行，curl 一堵窗口就「未响应」。
#[tauri::command]
pub async fn fetch_central_index(base: String, proxy: String) -> Result<CentralIndex, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_sync(&base, &proxy))
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
}

/* ================= 版本索引：开机预取 → 本地缓存 → 仅在变化时覆盖 ================= */

fn app_dir() -> PathBuf {
    if cfg!(windows) {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(appdata).join("StarlightLauncher")
    } else {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".local").join("share")
            });
        base.join("starlight-launcher")
    }
}

fn index_path() -> PathBuf {
    app_dir().join("central_versions.json")
}

fn read_index() -> VersionIndex {
    fs::read_to_string(index_path())
        .ok()
        .and_then(|s| serde_json::from_str::<VersionIndex>(&s).ok())
        .unwrap_or_default()
}

/// 只读本地缓存：界面立刻有内容，不等网络
#[tauri::command]
pub async fn list_central_versions() -> VersionIndex {
    tauri::async_runtime::spawn_blocking(read_index)
        .await
        .unwrap_or_default()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_of(v: &Vec<CentralVersion>) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    // FNV-1a：够用即可，只用于判断「服务器列表/版本是否更新」
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// 内容没变就不覆盖本地，避免无谓刷新
fn merge_index(idx: &mut VersionIndex, id: &str, list: Vec<CentralVersion>) -> bool {
    let h = hash_of(&list);
    if idx.hashes.get(id).map(|old| *old == h).unwrap_or(false) {
        return false;
    }
    idx.hashes.insert(id.to_string(), h);
    idx.sources.insert(id.to_string(), list);
    true
}

fn save_index(idx: &VersionIndex) -> Result<(), String> {
    let path = index_path();
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::write(&path, serde_json::to_string(idx).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn is_server_asset(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("server")
        || n.starts_with("server-")
        || n.contains("-server")
        || n.starts_with("dedicated")
}

/* ---------- GitHub releases（分页拉全，仅客户端资源） ---------- */

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct GhRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

fn gh_versions(repo: &str, proxy: &str) -> Result<Vec<CentralVersion>, String> {
    let headers = [
        "User-Agent: YueYue-Launcher",
        "Accept: application/vnd.github+json",
    ];
    let mut out: Vec<CentralVersion> = Vec::new();
    for page in 1..=5u32 {
        let url = format!(
            "https://api.github.com/repos/{repo}/releases?per_page=100&page={page}"
        );
        let text = http_get(&url, proxy, &headers, 20)?;
        let rels: Vec<GhRelease> =
            serde_json::from_str(&text).map_err(|e| format!("GitHub JSON 解析失败: {e}"))?;
        if rels.is_empty() {
            break;
        }
        let n = rels.len();
        for r in rels {
            let mut assets = Vec::new();
            let mut dropped = 0usize;
            for a in r.assets {
                if is_server_asset(&a.name) {
                    dropped += 1;
                    continue;
                }
                assets.push(CentralAsset {
                    name: a.name,
                    url: a.browser_download_url,
                    size: a.size,
                });
            }
            out.push(CentralVersion {
                tag: r.tag_name.clone(),
                title: if r.name.is_empty() {
                    r.tag_name.clone()
                } else {
                    r.name.clone()
                },
                date: r.published_at.chars().take(10).collect(),
                page_url: r.html_url,
                assets,
                dropped,
                folder: false,
            });
        }
        if n < 100 {
            break;
        }
    }
    Ok(out)
}

/* ---------- 文件站目录页 ---------- */

fn attr_of(block: &str, key: &str) -> String {
    let pat = format!("{key}=\"");
    if let Some(i) = block.find(&pat) {
        let s = i + pat.len();
        if let Some(e) = block[s..].find('"') {
            return block[s..s + e].to_string();
        }
    }
    String::new()
}

/// 相对链接按源地址补全（页面是 https://host/a/b，链接形如 /a/b/c）
fn abs_url(href: &str, base: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let origin = base
        .split_once("://")
        .and_then(|(s, rest)| rest.split('/').next().map(|h| format!("{s}://{h}")))
        .unwrap_or_default();
    if href.starts_with('/') {
        format!("{origin}{href}")
    } else {
        format!("{origin}/{href}")
    }
}

/// 解析文件站目录：条目是 <a class="term-file ...">，名称取最后一个 <span>
fn parse_file_list(html: &str, base: &str) -> Vec<CentralVersion> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut idx = 0usize;
    while out.len() < 200 {
        let Some(a) = html[idx..].find("<a ") else {
            break;
        };
        let s = idx + a;
        let Some(e) = html[s..].find("</a>") else {
            break;
        };
        let block = &html[s..s + e];
        idx = s + e + 4;
        if !block.contains("term-file") {
            continue;
        }
        let href = attr_of(block, "href");
        if href.is_empty() {
            continue;
        }
        let mut name = String::new();
        if let Some(p) = block.rfind("<span>") {
            if let Some(q) = block[p..].find("</span>") {
                name = block[p + 6..p + q].trim().to_string();
            }
        }
        if name.is_empty() {
            name = block
                .replace(['<', '>'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            name = name.replace("term-file term-folder-link", "").trim().to_string();
        }
        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        out.push(CentralVersion {
            tag: name.clone(),
            title: name.clone(),
            date: String::new(),
            page_url: abs_url(&href, base),
            assets: Vec::new(),
            dropped: 0,
            folder: true,
        });
    }
    out
}

fn direct_versions(url: &str) -> Vec<CentralVersion> {
    vec![CentralVersion {
        tag: url.rsplit('/').next().unwrap_or(url).to_string(),
        title: String::new(),
        date: String::new(),
        page_url: url.to_string(),
        assets: vec![CentralAsset {
            name: url.rsplit('/').next().unwrap_or(url).to_string(),
            url: url.to_string(),
            size: 0,
        }],
        dropped: 0,
        folder: false,
    }]
}

fn versions_of(it: &CentralItem, proxy: &str) -> Result<Vec<CentralVersion>, String> {
    match it.kind.as_str() {
        "github-repo" => gh_versions(&it.repo, proxy),
        "file-list" => {
            let html = http_get(&it.url, proxy, &[], 20)?;
            let list = parse_file_list(&html, &it.url);
            if list.is_empty() {
                return Err("目录页没解析到任何条目".into());
            }
            Ok(list)
        }
        "direct-url" => Ok(direct_versions(&it.url)),
        _ => Err(format!("未知源类型: {}", it.kind)),
    }
}

/// 开机预取所有源的版本到本地；并行拉取。
/// 拉失败的源沿用本地缓存，全部失败则缓存原样保留。
///
/// 实现放在阻塞线程池：curl 单次要几秒到二十秒，占主线程会让窗口「未响应」。
fn sync_blocking(base: &str, proxy: &str) -> Result<VersionIndex, String> {
    let t0 = std::time::Instant::now();
    let index = fetch_sync(base, proxy)?;
    let items = index.items;
    let total = items.len();

    // 代理串先降为 &str（Copy），才能被多个线程闭包同时捕获
    let proxy_ref: &str = proxy;
    let results: Vec<Result<Vec<CentralVersion>, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = items
            .iter()
            .map(|it| s.spawn(move || versions_of(it, proxy_ref)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("线程崩溃".into())))
            .collect()
    });

    let mut idx = read_index();
    let mut ok = 0usize;
    for (it, res) in items.iter().zip(results.into_iter()) {
        match res {
            Ok(list) if !list.is_empty() => {
                merge_index(&mut idx, &it.id, list);
                ok += 1;
            }
            Ok(_) | Err(_) => { /* 沿用本地缓存，界面照常显示 */ }
        }
    }
    if ok > 0 {
        idx.synced_at = now_ms();
    }
    idx.ok = ok;
    idx.total = total;
    idx.sync_ms = t0.elapsed().as_millis() as u64;
    let _ = save_index(&idx);
    Ok(idx)
}

#[tauri::command]
pub async fn sync_central_versions(base: String, proxy: String) -> Result<VersionIndex, String> {
    tauri::async_runtime::spawn_blocking(move || sync_blocking(&base, &proxy))
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_index_is_all_client_sources() {
        let items = sanitize(builtin_items());
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| !i.name.is_empty() && !i.tags.is_empty()));
        assert!(items.iter().all(|i| i.scope == "client"));
        assert!(items.iter().any(|i| i.kind == "file-list"));
        assert!(items.iter().any(|i| i.kind == "github-repo"));
    }

    #[test]
    fn sanitize_drops_invalid_and_caps_tags() {
        let bad = CentralItem {
            id: "a".into(),
            name: "n".into(),
            kind: "direct-url".into(),
            url: "ftp://x".into(),
            repo: String::new(),
            asset: d_asset(),
            note: String::new(),
            group: String::new(),
            tags: vec![],
            size: 0,
            logo: String::new(),
            scope: d_scope(),
        };
        assert!(sanitize(vec![bad]).is_empty());

        let mut ok = builtin_items().pop().unwrap();
        ok.tags = (0..40).map(|i| format!("t{i}")).collect();
        ok.name = "x".repeat(500);
        ok.logo = "javascript:alert(1)".into();
        let out = sanitize(vec![ok]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tags.len(), 8);
        assert!(out[0].name.chars().count() <= 60);
        assert!(out[0].logo.is_empty());
    }

    #[test]
    fn parse_file_list_extracts_entries_and_absolutizes() {
        let html = r#"
          <a href="/category/Mindustry" class="term-nav-item">Mindustry</a>
          <a href="/category/Mindustry/v8/build-157.1-stable" class="term-file term-folder-link">
            <div class="term-file-name"><span class="term-folder-icon"></span><span>build-157.1-stable</span></div>
          </a>
          <a href="/category/Mindustry/v8/build-160-stable" class="term-file term-folder-link">
            <div class="term-file-name"><span class="term-folder-icon"></span><span>build-160-stable</span></div>
          </a>"#;
        let list = parse_file_list(html, "https://file.mdtbbs.cn/category/Mindustry/v8");
        assert_eq!(list.len(), 2, "面包屑链接不应计入");
        assert_eq!(list[0].tag, "build-157.1-stable");
        assert_eq!(
            list[0].page_url,
            "https://file.mdtbbs.cn/category/Mindustry/v8/build-157.1-stable"
        );
        assert!(list[0].folder);
    }

    #[test]
    fn server_assets_are_filtered_out() {
        assert!(is_server_asset("server-2026.10.02.B502.jar"));
        assert!(is_server_asset("MindustryX-server.jar"));
        assert!(!is_server_asset("MindustryX-2026.10.02-Desktop.jar"));
        assert!(!is_server_asset("dexed-MindustryX.loader.jar"));
        assert!(!is_server_asset("MindustryX-Android.apk"));
    }

    #[test]
    fn merge_keeps_local_when_hash_unchanged() {
        let mut idx = VersionIndex::default();
        let list = parse_file_list(
            r#"<a href="/a/x" class="term-file"><span>x</span></a>"#,
            "https://h/p",
        );
        assert!(merge_index(&mut idx, "s1", list.clone()));
        // 同样内容再合并 → 不覆盖（返回 false）
        assert!(!merge_index(&mut idx, "s1", list.clone()));
        assert_eq!(idx.sources["s1"].len(), 1);
    }

    fn curl_available() -> bool {
        std::process::Command::new("curl")
            .arg("--version")
            .output()
            .is_ok()
    }

    /// 真跑一遍「客户端 → curl → HTTP → 解析 → 清洗」全链路
    #[test]
    fn fetches_remote_index_over_http() {
        if !curl_available() {
            eprintln!("skip: curl 不在 PATH");
            return;
        }
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = r#"{"schema":1,"items":[
            {"id":"x","name":"测试条目","kind":"direct-url","url":"https://a/b.jar","group":"分组","tags":["甲","乙"],"size":123},
            {"id":"bad","name":"坏条目","kind":"direct-url","url":"ftp://nope","group":"G","tags":[]}
        ]}"#;
        let responder = std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes());
            }
        });

        let idx = fetch_sync(&format!("http://{addr}"), "").unwrap();
        responder.join().unwrap();

        assert_eq!(idx.source, "remote");
        assert_eq!(idx.note, "");
        assert_eq!(idx.items.len(), 1);
        assert_eq!(idx.items[0].name, "测试条目");
        assert_eq!(idx.items[0].tags, vec!["甲".to_string(), "乙".to_string()]);
        assert_eq!(idx.items[0].size, 123);
    }

    /// 服务器不可达 → 降级为内置索引而不是报错
    #[test]
    fn unreachable_server_falls_back_to_builtin() {
        if !curl_available() {
            eprintln!("skip: curl 不在 PATH");
            return;
        }
        let idx = fetch_sync("http://127.0.0.1:1", "").unwrap();
        assert_eq!(idx.source, "builtin");
        assert!(!idx.items.is_empty());
        assert!(!idx.note.is_empty());
    }

    #[test]
    fn empty_base_uses_builtin_and_bad_scheme_is_rejected() {
        let idx = fetch_sync("", "").unwrap();
        assert_eq!(idx.source, "builtin");
        assert!(idx.note.is_empty());
        assert!(fetch_sync("ftp://x", "").is_err());
    }
}
