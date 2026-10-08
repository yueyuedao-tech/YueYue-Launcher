use serde::{Deserialize, Serialize};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const APPID: &str = "1127400";
const MODS_INDEX: &str = "https://raw.githubusercontent.com/Anuken/MindustryMods/master/mods.json";
const MOD_CACHE_MS: u64 = 60 * 60 * 1000;

fn cache_dir() -> PathBuf {
    crate::cmdutil::app_root().join("cache").join("mods")
}

fn index_cache_path(url: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    cache_dir().join("indexes").join(format!("{:016x}.json", hasher.finish()))
}

fn icon_cache_path(url: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    cache_dir().join("icons").join(format!("{:016x}.img", hasher.finish()))
}

fn fresh_cache(path: &std::path::Path) -> bool {
    fs::metadata(path).ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| SystemTime::now().duration_since(time).ok())
        .map(|age| age.as_millis() < MOD_CACHE_MS as u128)
        .unwrap_or(false)
}

fn read_index(url: &str, proxy: &str, remote_proxy: &str) -> Result<Vec<RepoMod>, String> {
    let path = index_cache_path(url);
    if fresh_cache(&path) {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(items) = serde_json::from_str(&raw) { return Ok(items); }
        }
    }
    let routed_proxy = if url == MODS_INDEX { remote_proxy } else { "" };
    let raw = http_text_remote(url, proxy, routed_proxy)?;
    let items: Vec<RepoMod> = serde_json::from_str(&raw).map_err(|e| format!("Mod 清单格式错误: {e}"))?;
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_ok() {
            if fs::write(&path, raw).is_ok() {
                if let Ok(entries) = fs::read_dir(parent) {
                    let mut files: Vec<_> = entries.flatten().collect();
                    files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
                    while files.len() > 10 { let old = files.remove(0); let _ = fs::remove_file(old.path()); }
                }
            }
        }
    }
    Ok(items)
}

#[tauri::command]
pub async fn cache_mod_icon(url: String, proxy: String, remote_proxy: String) -> Result<String, String> {
    if !url.starts_with("https://raw.githubusercontent.com/Anuken/MindustryMods/master/icons/") || url.len() > 500 {
        return Err("Mod 图标地址无效".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let path = icon_cache_path(&url);
        if fresh_cache(&path) { return Ok(path.to_string_lossy().into_owned()); }
        if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let temp = path.with_extension(format!("{nonce}.tmp"));
        let base = remote_proxy.trim().trim_end_matches('/');
        let request_url = if base.is_empty() { url.clone() } else { format!("{base}/proxy?url={}", urlencode(&url)) };
        let mut cmd = crate::cmdutil::no_console("curl");
        cmd.args(["-fLsS", "--max-time", "30", "--max-filesize", "1048576"]);
        if !proxy.trim().is_empty() { cmd.args(["--proxy", proxy.trim()]); }
        let result = cmd.arg("-o").arg(&temp).arg(request_url).output().map_err(|e| e.to_string())?;
        if !result.status.success() { let _ = fs::remove_file(&temp); return Err("Mod 图标下载失败".into()); }
        if path.exists() { fs::remove_file(&path).map_err(|e| e.to_string())?; }
        fs::rename(&temp, &path).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            if let Ok(entries) = fs::read_dir(parent) {
                let mut files: Vec<_> = entries.flatten().filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("img")).collect();
                files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
                while files.len() > 60 { let old = files.remove(0); let _ = fs::remove_file(old.path()); }
            }
        }
        Ok(path.to_string_lossy().into_owned())
    }).await.map_err(|e| e.to_string())?
}

pub fn mods_dir_for(instance_id: &str) -> Result<PathBuf, String> {
    let info = crate::instances::read_instance(instance_id)?;
    let root = if info.isolate {
        crate::instances::data_dir_abs(&info)
    } else {
        crate::instances::shared_data()
    };
    Ok(root.join("Mindustry").join("mods"))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopItem {
    pub id: String,
    pub title: String,
    pub url: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub name: String,
    pub size: u64,
    pub mtime: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RepoMod {
    repo: String,
    #[serde(default)]
    internal_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    last_updated: String,
    #[serde(default)]
    stars: u64,
    #[serde(default)]
    version: String,
    #[serde(default)]
    min_game_version: String,
    #[serde(default)]
    has_java: bool,
    #[serde(default)]
    has_icon: bool,
    #[serde(default)]
    description: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModCatalogItem {
    pub repo: String,
    pub internal_name: String,
    pub name: String,
    pub author: String,
    pub last_updated: String,
    pub stars: u64,
    pub version: String,
    pub min_game_version: String,
    pub has_java: bool,
    pub icon_url: String,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModCatalogPage {
    pub items: Vec<ModCatalogItem>,
    pub total: usize,
}

#[derive(Deserialize)]
struct GithubRelease {
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDownloadInfo {
    pub file_name: String,
    pub url: String,
}

fn http_text_remote(url: &str, proxy: &str, remote_proxy: &str) -> Result<String, String> {
    let base = remote_proxy.trim().trim_end_matches('/');
    let request_url = if base.is_empty() { url.to_string() } else { format!("{base}/proxy?url={}", urlencode(url)) };
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-fLsS", "--max-time", "30", "-H", "User-Agent: YueYue-Launcher"]);
    if !proxy.trim().is_empty() { cmd.args(["--proxy", proxy.trim()]); }
    let out = cmd.arg(request_url).output().map_err(|e| format!("无法运行 curl: {e}"))?;
    if !out.status.success() { return Err(format!("GitHub 请求失败: {}", String::from_utf8_lossy(&out.stderr).trim())); }
    String::from_utf8(out.stdout).map_err(|e| format!("GitHub 返回编码错误: {e}"))
}

fn catalog_item(m: RepoMod) -> ModCatalogItem {
    let icon_url = if m.has_icon {
        // MindustryMods stores icons in one shared directory, keyed by owner_repo.
        let icon_key = m.repo.replace('/', "_");
        format!("https://raw.githubusercontent.com/Anuken/MindustryMods/master/icons/{icon_key}")
    } else {
        String::new()
    };
    ModCatalogItem { repo: m.repo, internal_name: m.internal_name, name: m.name, author: m.author, last_updated: m.last_updated, stars: m.stars, version: m.version, min_game_version: m.min_game_version, has_java: m.has_java, icon_url, description: m.description }
}

#[tauri::command]
pub async fn list_github_mods(query: String, page: u32, proxy: String, remote_proxy: String, index_url: String) -> Result<ModCatalogPage, String> {
    if page > 1000 { return Err("页码超出范围".into()); }
    let query = query.trim().to_lowercase();
    if query.chars().count() > 100 { return Err("搜索词过长".into()); }
    let index_url = if index_url.trim().is_empty() { MODS_INDEX.to_string() } else { index_url.trim().to_string() };
    if index_url.len() > 2000 || !(index_url.starts_with("https://") || index_url.starts_with("http://")) {
        return Err("Mod 索引地址必须是 HTTP 或 HTTPS 链接".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let all = read_index(&index_url, &proxy, &remote_proxy)?;
        let filtered: Vec<ModCatalogItem> = all.into_iter()
            .filter(|m| query.is_empty() || [m.name.as_str(), m.internal_name.as_str(), m.repo.as_str(), m.author.as_str(), m.description.as_str()].iter().any(|s| s.to_lowercase().contains(&query)))
            .map(catalog_item)
            .collect();
        let total = filtered.len();
        let start = page as usize * 20;
        Ok(ModCatalogPage { items: filtered.into_iter().skip(start).take(20).collect(), total })
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn resolve_github_mod(repo: String, proxy: String, remote_proxy: String) -> Result<ModDownloadInfo, String> {
    if !repo.contains('/') || repo.len() > 120 { return Err("仓库地址无效".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("https://api.github.com/repos/{repo}/releases?per_page=5");
        let raw = http_text_remote(&url, &proxy, &remote_proxy)?;
        let releases: Vec<GithubRelease> = serde_json::from_str(&raw).map_err(|e| format!("Release 列表格式错误: {e}"))?;
        let mut fallback: Option<ModDownloadInfo> = None;
        for release in releases {
            for asset in release.assets {
                let lower = asset.name.to_lowercase();
                if !(lower.ends_with(".jar") || lower.ends_with(".zip")) || lower.contains("source") { continue; }
                let info = ModDownloadInfo { file_name: asset.name, url: asset.browser_download_url };
                if lower.ends_with(".jar") { return Ok(info); }
                fallback = Some(info);
            }
        }
        fallback.ok_or_else(|| "这个 Mod 没有可下载的 Release 文件".into())
    }).await.map_err(|e| e.to_string())?
}

/// 百分号编码（零依赖）：保留 RFC3986 unreserved，其余按 UTF-8 字节编码
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// 解析 workshop 列表页。
/// 新版 Steam 无 workshopItemTitle 类（已实测 0 次），标题链接形态为
/// `<a href="…/sharedfiles/filedetails/?id=NNN">标题文本</a>`（无其他属性）；
/// 缩略图链接同 id 但带 class 等属性且内容是 `<img>`，天然被排除。
pub fn parse_workshop_items(html: &str, limit: usize) -> Vec<WorkshopItem> {
    const MARK_ABS: &str = "<a href=\"https://steamcommunity.com/sharedfiles/filedetails/?id=";
    const MARK_REL: &str = "<a href=\"/sharedfiles/filedetails/?id=";
    let mut out: Vec<WorkshopItem> = Vec::new();
    let mut rest = html;
    while out.len() < limit {
        let mark = if rest.contains(MARK_ABS) {
            MARK_ABS
        } else if rest.contains(MARK_REL) {
            MARK_REL
        } else {
            break;
        };
        let start = match rest.find(mark) {
            Some(i) => i + mark.len(),
            None => break,
        };
        // 数字 id
        let id_bytes = rest.as_bytes();
        let mut j = start;
        while j < id_bytes.len() && id_bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == start {
            rest = &rest[start..];
            continue;
        }
        let id = rest[start..j].to_string();
        // 必须紧跟 `">`（标题链接无其他属性）
        if !rest[j..].starts_with("\">") {
            rest = &rest[j..];
            continue;
        }
        let title_start = j + 2;
        let Some(close) = rest[title_start..].find("</a>") else {
            break;
        };
        let raw = &rest[title_start..title_start + close];
        if !raw.contains('<') {
            let title = html_unescape(raw).trim().to_string();
            if !title.is_empty() {
                out.push(WorkshopItem {
                    id: id.clone(),
                    title,
                    url: format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}"),
                });
            }
        }
        rest = &rest[title_start + close..];
    }
    out
}

#[tauri::command]
pub async fn search_workshop(
    query: String,
    proxy: String,
    mirror: String,
) -> Result<Vec<WorkshopItem>, String> {
    if query.trim().is_empty() {
        return Err("搜索词不能为空".into());
    }
    let path = format!(
        "https://steamcommunity.com/workshop/browse/?appid={APPID}&searchtext={}&browsesort=trend&days=90",
        urlencode(query.trim())
    );
    let url = if mirror.trim().is_empty() {
        path
    } else {
        format!("{}{}", mirror.trim(), path)
    };
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-sSL", "--max-time", "25"]);
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd.arg(&url);
    let out = cmd.output().map_err(|e| format!("curl 不可用: {e}"))?;
    let html = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || html.len() < 500 {
        return Err("拉取工坊页面失败——请检查 工坊镜像/代理 设置".into());
    }
    let items = parse_workshop_items(&html, 20);
    if items.is_empty() {
        return Err("页面解析到 0 条（可能被重定向到登录页/验证页）——尝试镜像或代理".into());
    }
    Ok(items)
}

#[tauri::command]
pub async fn list_mods(instance_id: String) -> Result<Vec<ModFile>, String> {
    let dir = mods_dir_for(&instance_id)?;
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
            .map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        out.push(ModFile {
            name: entry.file_name().to_string_lossy().into_owned(),
            size: meta.len(),
            mtime,
        });
    }
    out.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    Ok(out)
}

#[tauri::command]
pub async fn mods_dir(instance_id: String) -> Result<String, String> {
    Ok(mods_dir_for(&instance_id)?.to_string_lossy().into_owned())
}

fn valid_mod_name(name: &str) -> bool {
    !name.is_empty()
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

#[tauri::command]
pub async fn delete_mod(instance_id: String, name: String) -> Result<(), String> {
    if !valid_mod_name(&name) {
        return Err("非法 Mod 文件名".into());
    }
    let dir = mods_dir_for(&instance_id)?;
    let target = dir.join(&name);
    if !target.is_file() {
        return Err(format!("Mod 不存在: {name}"));
    }
    fs::remove_file(&target).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_keeps_unreserved_and_encodes_rest() {
        assert_eq!(urlencode("abc-123_~."), "abc-123_~.");
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("中文"), "%E4%B8%AD%E6%96%87");
        assert_eq!(urlencode("c++"), "c%2B%2B");
    }

    const SAMPLE: &str = r#"
    <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=3776239024" class="tK5agp"><img src="thumb.jpg"></a>
    <div class="Sw3NX"><a href="https://steamcommunity.com/sharedfiles/filedetails/?id=3776239024">subdustry (subnautica)</a></div>
    <div class="Sw3NX"><a href="https://steamcommunity.com/sharedfiles/filedetails/?id=123456">第二 &lt;测试&gt; &amp; More</a></div>
    <div><a href="/sharedfiles/filedetails/?id=999">相对链接标题</a></div>"#;

    #[test]
    fn parses_workshop_titles_and_ids() {
        let items = parse_workshop_items(SAMPLE, 20);
        assert_eq!(items.len(), 3, "缩略图链接应被排除: {:?}", items);
        assert_eq!(items[0].id, "3776239024");
        assert_eq!(items[0].title, "subdustry (subnautica)");
        assert_eq!(items[1].title, "第二 <测试> & More");
        assert_eq!(items[2].id, "999");
        assert_eq!(items[2].title, "相对链接标题");
    }

    #[test]
    fn mod_name_validation() {
        assert!(valid_mod_name("abc.jar"));
        assert!(!valid_mod_name(""));
        assert!(!valid_mod_name(".."));
        assert!(!valid_mod_name("a/b"));
        assert!(!valid_mod_name("a\\b"));
    }
}
