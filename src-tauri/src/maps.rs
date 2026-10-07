use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const API: &str = "https://api.mindustry.top/maps";
const BATCH: usize = 15;
const PAGE: usize = 20;
const MAX_MAP_BYTES: u64 = 20 * 1024 * 1024;
const MAP_CACHE_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const MAP_CACHE_LIMIT: usize = 60;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MapItem {
    id: u64,
    name: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    preview: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    mode: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MapPage {
    items: Vec<MapItem>,
    has_more: bool,
}

#[derive(Deserialize, Serialize)]
struct MapCacheEntry {
    key: String,
    fetched_at: u64,
    page: MapPage,
}

#[derive(Deserialize, Serialize, Default)]
struct MapCache {
    entries: Vec<MapCacheEntry>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn cache_path() -> PathBuf {
    crate::cmdutil::app_root().join("map-metadata-cache.json")
}

fn cache_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn read_cached(key: &str) -> Option<MapPage> {
    let _guard = cache_lock().lock().ok()?;
    let raw = fs::read_to_string(cache_path()).ok()?;
    let cache: MapCache = serde_json::from_str(&raw).ok()?;
    let entry = cache.entries.into_iter().find(|e| e.key == key)?;
    if now_ms().saturating_sub(entry.fetched_at) > MAP_CACHE_TTL_MS {
        return None;
    }
    Some(entry.page)
}

fn write_cached(key: String, page: MapPage) {
    let Ok(_guard) = cache_lock().lock() else { return };
    let path = cache_path();
    let mut cache = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<MapCache>(&raw).ok())
        .unwrap_or_default();
    cache.entries.retain(|e| e.key != key);
    cache.entries.push(MapCacheEntry { key, fetched_at: now_ms(), page });
    cache.entries.sort_by_key(|e| std::cmp::Reverse(e.fetched_at));
    cache.entries.truncate(MAP_CACHE_LIMIT);
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let temp = path.with_extension("json.tmp");
    if let Ok(data) = serde_json::to_vec_pretty(&cache) {
        if fs::write(&temp, data).is_ok() { let _ = fs::rename(temp, path); }
    }
}

fn get(url: &str, proxy: &str, output: Option<&std::path::Path>) -> Result<Vec<u8>, String> {
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-fLsS", "--max-time", "25", "--max-filesize", "20971520"]);
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    if let Some(path) = output {
        cmd.arg("-o").arg(path);
    }
    let result = cmd.arg(url).output().map_err(|e| format!("无法运行 curl: {e}"))?;
    if !result.status.success() {
        return Err(format!("地图站点请求失败: {}", String::from_utf8_lossy(&result.stderr).trim()));
    }
    Ok(result.stdout)
}

fn encode_query(query: &str) -> String {
    crate::mods::urlencode(query)
}

#[tauri::command]
pub async fn search_maps(page: u32, query: String, version: String, proxy: String) -> Result<MapPage, String> {
    if page > 1000 {
        return Err("页码超出范围".into());
    }
    if query.chars().count() > 100 {
        return Err("搜索词过长".into());
    }
    if !version.is_empty() && !["3", "4", "5", "7", "8", "9", "10", "11"].contains(&version.as_str()) {
        return Err("未知地图版本".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let search = format!("{}{}", query.trim(), if version.is_empty() { String::new() } else { format!(" @version:{version}") });
        let cache_key = format!("{}\0{}\0{}", search.trim(), version, page);
        if let Some(cached) = read_cached(&cache_key) {
            return Ok(cached);
        }
        let first = page as usize * PAGE;
        let end = first + PAGE + 1;
        let first_batch = first / BATCH;
        let last_batch = (end.saturating_sub(1) / BATCH) + 1;
        let urls: Vec<String> = (first_batch..last_batch)
            .map(|batch| format!("{API}/list?begin={}&search={}", batch * BATCH, encode_query(search.trim())))
            .collect();
        let batches = std::thread::scope(|scope| {
            let handles = urls.iter().map(|url| scope.spawn(|| get(url, &proxy, None))).collect::<Vec<_>>();
            handles.into_iter().map(|h| h.join().map_err(|_| "地图请求线程异常".to_string())?).collect::<Result<Vec<_>, String>>()
        })?;
        let mut items = Vec::new();
        for data in batches {
            let batch: Vec<MapItem> = serde_json::from_slice(&data).map_err(|e| format!("地图列表格式错误: {e}"))?;
            items.extend(batch);
        }
        let start = first - first / BATCH * BATCH;
        let selected: Vec<MapItem> = items.into_iter().skip(start).take(PAGE + 1).collect();
        let result = MapPage { has_more: selected.len() > PAGE, items: selected.into_iter().take(PAGE).collect() };
        write_cached(cache_key, result.clone());
        Ok(result)
    }).await.map_err(|e| e.to_string())?
}

fn maps_dir(instance_id: &str) -> Result<PathBuf, String> {
    let info = crate::instances::read_instance(instance_id)?;
    let root = if info.isolate {
        crate::instances::data_dir_abs(&info)
    } else {
        crate::instances::shared_data()
    };
    Ok(root.join("Mindustry").join("maps"))
}

#[tauri::command]
pub async fn install_map(instance_id: String, map_id: u64, proxy: String) -> Result<String, String> {
    if map_id == 0 { return Err("地图 ID 无效".into()); }
    let dir = maps_dir(&instance_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dest = dir.join(format!("mindustry-top-{map_id}.msav"));
        if dest.exists() { return Err("该地图已安装到这个游戏".into()); }
        let temp = dir.join(format!(".mindustry-top-{map_id}-{}.tmp", std::process::id()));
        let result = (|| {
            get(&format!("{API}/{map_id}.msav"), &proxy, Some(&temp))?;
            let meta = fs::metadata(&temp).map_err(|e| e.to_string())?;
            if meta.len() < 8 || meta.len() > MAX_MAP_BYTES {
                return Err("地图文件大小异常".into());
            }
            let bytes = fs::read(&temp).map_err(|e| e.to_string())?;
            if bytes[0] & 0x0f != 8 || ((bytes[0] as u16 * 256 + bytes[1] as u16) % 31 != 0) {
                return Err("下载内容不是 Mindustry 地图文件".into());
            }
            if dest.exists() { return Err("该地图已安装到这个游戏".into()); }
            fs::rename(&temp, &dest).map_err(|e| e.to_string())?;
            Ok(dest.to_string_lossy().into_owned())
        })();
        if result.is_err() { let _ = fs::remove_file(&temp); }
        result
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_encoding_and_page_boundaries() {
        assert_eq!(encode_query("中文 @version:11"), "%E4%B8%AD%E6%96%87%20%40version%3A11");
        assert_eq!(20 / BATCH * BATCH, 15);
        assert_eq!(40 / BATCH * BATCH, 30);
    }
}
