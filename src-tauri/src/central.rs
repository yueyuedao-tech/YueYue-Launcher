use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CentralItem {
    pub id: String,
    pub name: String,
    /// "direct-url" | "github-repo"
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
}

fn d_asset() -> String {
    "Mindustry.jar".into()
}
fn d_group() -> String {
    "默认".into()
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

/// 内置兜底索引：直接由预置源表生成，服务器不可用时下载中心仍可用
fn builtin_items() -> Vec<CentralItem> {
    crate::sources::preset_sources()
        .into_iter()
        .map(|s| {
            let (kind, tags): (&str, Vec<&str>) = match s.kind {
                crate::sources::SourceKind::GithubRepo => ("github-repo", vec!["官方", "版本索引"]),
                crate::sources::SourceKind::DirectUrl => ("direct-url", vec!["官方", "稳定", "直链"]),
            };
            let mut all: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
            all.push(s.group.clone());
            CentralItem {
                id: s.id,
                name: s.name,
                kind: kind.into(),
                url: s.url,
                repo: s.repo,
                asset: s.asset,
                note: s.note,
                group: s.group,
                tags: all,
                size: 0,
            }
        })
        .collect()
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
            it.group = "默认".into();
        }
        it.id = it.id.trim().chars().take(48).collect();
        if it.id.is_empty() {
            continue;
        }
        match it.kind.as_str() {
            "direct-url" => {
                if !(it.url.starts_with("https://") || it.url.starts_with("http://")) {
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

fn http_get(url: &str, proxy: &str) -> Result<String, String> {
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-sSL", "--max-time", "15"]);
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd.arg(url);
    let out = cmd.output().map_err(|e| format!("无法执行 curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "拉取索引失败（code {}）",
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
    match http_get(&url, proxy)
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
#[tauri::command]
pub async fn fetch_central_index(base: String, proxy: String) -> Result<CentralIndex, String> {
    fetch_sync(&base, &proxy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_index_is_usable() {
        let items = sanitize(builtin_items());
        assert!(!items.is_empty());
        assert!(items.iter().all(|i| !i.name.is_empty() && !i.tags.is_empty()));
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
        };
        assert!(sanitize(vec![bad]).is_empty());

        let mut ok = builtin_items().pop().unwrap();
        ok.tags = (0..40).map(|i| format!("t{i}")).collect();
        ok.name = "x".repeat(500);
        let out = sanitize(vec![ok]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tags.len(), 8);
        assert!(out[0].name.chars().count() <= 60);
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
        // 非法 URL 的条目被清洗掉
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
