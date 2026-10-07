use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const API: &str = "https://api.mindustry.top/maps";
const BATCH: usize = 15;
const PAGE: usize = 20;
const MAX_MAP_BYTES: u64 = 20 * 1024 * 1024;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPage {
    items: Vec<MapItem>,
    has_more: bool,
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
        let first = page as usize * PAGE;
        let end = first + PAGE + 1;
        let mut items = Vec::new();
        let mut offset = first / BATCH * BATCH;
        while offset < end {
            let url = format!("{API}/list?begin={offset}&search={}", encode_query(search.trim()));
            let data = get(&url, &proxy, None)?;
            let batch: Vec<MapItem> = serde_json::from_slice(&data).map_err(|e| format!("地图列表格式错误: {e}"))?;
            let count = batch.len();
            items.extend(batch);
            if count < BATCH { break; }
            offset += BATCH;
        }
        let start = first - first / BATCH * BATCH;
        let selected: Vec<MapItem> = items.into_iter().skip(start).take(PAGE + 1).collect();
        Ok(MapPage { has_more: selected.len() > PAGE, items: selected.into_iter().take(PAGE).collect() })
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
