use serde::Serialize;
use std::fs;
use std::path::PathBuf;

const APPID: &str = "1127400";

pub fn mods_dir_for(instance_id: &str) -> Result<PathBuf, String> {
    let id = crate::instances::validate_id(instance_id)?;
    let dir = crate::instances::instances_root()
        .join(&id)
        .join("data")
        .join("Mindustry")
        .join("mods");
    Ok(dir)
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
