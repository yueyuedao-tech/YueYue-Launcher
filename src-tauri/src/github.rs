use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GithubVersion {
    pub tag: String,
    pub title: String,
    pub updated: String,
    pub page_url: String,
    pub jar_url: String,
}

fn tag_of(entry: &str) -> Option<String> {
    // 从 <link ... href=".../releases/tag/<tag>"...> 提取 tag
    let marker = "/releases/tag/";
    let idx = entry.find(marker)?;
    let rest = &entry[idx + marker.len()..];
    let end = rest
        .find(|c: char| c == '"' || c == '\'' || c == '<' || c == ' ' || c == '&')
        .unwrap_or(rest.len());
    let tag = &rest[..end];
    (!tag.is_empty()).then(|| tag.to_string())
}

fn field(entry: &str, open: &str, close: &str) -> String {
    let Some(i) = entry.find(open) else { return String::new() };
    let s = i + open.len();
    match entry[s..].find(close) {
        Some(j) => entry[s..s + j].trim().to_string(),
        None => String::new(),
    }
}

pub(crate) fn repo_ok(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let seg_ok = |p: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    seg_ok(owner) && seg_ok(name)
}

fn asset_ok(asset: &str) -> bool {
    !asset.is_empty()
        && !asset.contains('/')
        && !asset.contains('\\')
        && !asset.contains(':')
        && asset.len() < 100
}

/// 拉取并解析 GitHub releases.atom（免认证限额）。
/// prefix=镜像前缀（拼在 github.com 前），proxy=HTTP 代理，asset=下载资产文件名。
#[tauri::command]
pub async fn fetch_repo_versions(
    repo: String,
    asset: String,
    proxy: String,
    prefix: String,
) -> Result<Vec<GithubVersion>, String> {
    let repo = repo.trim().to_string();
    if !repo_ok(&repo) {
        return Err(format!("repo 需为 owner/name 形态: {repo}"));
    }
    let asset = if asset.trim().is_empty() {
        "Mindustry.jar".to_string()
    } else {
        asset.trim().to_string()
    };
    if !asset_ok(&asset) {
        return Err(format!("非法资产文件名: {asset}"));
    }

    let url = format!("{prefix}https://github.com/{repo}/releases.atom");
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-sSL", "--max-time", "30"]);
    if !proxy.trim().is_empty() {
        cmd.args(["--proxy", proxy.trim()]);
    }
    cmd.arg(&url);
    let out = cmd.output().map_err(|e| format!("无法执行 curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "拉取 releases.atom 失败（code {}）。可检查网络/代理/加速前缀",
            out.status.code().unwrap_or(-1)
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if !text.contains("<feed") {
        return Err("返回内容不是 Atom feed（可能被镜像/网页拦截）".into());
    }

    let jar_base = format!("https://github.com/{repo}/releases/download/");
    let mut result: Vec<GithubVersion> = Vec::new();
    for entry in text.split("<entry>").skip(1) {
        if result.len() >= 12 {
            break;
        }
        let Some(tag) = tag_of(entry) else { continue };
        let title = field(entry, "<title>", "</title>").replace('\n', " ");
        let updated = field(entry, "<updated>", "</updated>");
        result.push(GithubVersion {
            page_url: format!("https://github.com/{repo}/releases/tag/{tag}"),
            jar_url: format!("{jar_base}{tag}/{asset}"),
            tag,
            title,
            updated,
        });
    }
    if result.is_empty() {
        return Err("Atom 中没有解析到任何 release".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Release notes from Anuken/Mindustry</title>
  <entry>
    <title>v146</title>
    <updated>2024-07-01T00:00:00Z</updated>
    <link rel="alternate" type="text/html" href="https://github.com/Anuken/Mindustry/releases/tag/v146"/>
    <content type="html"></content>
  </entry>
  <entry>
    <title>v145</title>
    <updated>2024-01-15T00:00:00Z</updated>
    <link rel="alternate" href="https://github.com/Anuken/Mindustry/releases/tag/v145"/>
  </entry>
</feed>"#;

    #[test]
    fn parses_entries_in_order() {
        let jar_base = "https://github.com/Anuken/Mindustry/releases/download/";
        let mut out = Vec::new();
        for entry in SAMPLE.split("<entry>").skip(1) {
            let Some(tag) = tag_of(entry) else { continue };
            out.push((
                tag.clone(),
                field(entry, "<title>", "</title>"),
                format!("{jar_base}{tag}/Mindustry.jar"),
            ));
        }
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, "v146");
        assert_eq!(out[0].1, "v146");
        assert!(out[0].2.ends_with("/v146/Mindustry.jar"));
        assert_eq!(out[1].0, "v145");
    }

    #[test]
    fn repo_and_asset_validation() {
        assert!(repo_ok("Anuken/Mindustry"));
        assert!(!repo_ok("no-slash"));
        assert!(!repo_ok("a/b/c"));
        assert!(!repo_ok("bad repo/x"));
        assert!(asset_ok("Mindustry.jar"));
        assert!(!asset_ok("../evil"));
        assert!(!asset_ok(""));
    }
}
