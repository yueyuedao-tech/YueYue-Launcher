use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    GithubRepo,
    DirectUrl,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceItem {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub repo: String,
    /// github-repo 源的资产文件名
    #[serde(default = "default_asset")]
    pub asset: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default = "default_true")]
    pub latest_enabled: bool,
    #[serde(default)]
    pub open_in_new_page: bool,
}

fn default_asset() -> String {
    "Mindustry.jar".into()
}
fn default_group() -> String {
    "默认".into()
}
fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct SourcesFile {
    sources: Vec<SourceItem>,
}

/// 预置：只保留官方仓库索引。直链条目已随中心索引一起下线
/// （下载中心改为只展示「中心 · 官方源」）
pub fn preset_sources() -> Vec<SourceItem> {
    vec![SourceItem {
        id: "src-github-official".into(),
        name: "GitHub 官方仓库".into(),
        kind: SourceKind::GithubRepo,
        url: String::new(),
        repo: "Anuken/Mindustry".into(),
        asset: default_asset(),
        note: "官方 releases 索引（Atom）".into(),
        group: "官方源".into(),
        collapsed: false,
        latest_enabled: true,
        open_in_new_page: true,
    }]
}

/// 已下线的预置源 id：老安装里存下来的这两条同样要清掉
const RETIRED_PRESET_IDS: [&str; 2] = ["src-github-official", "src-v146-direct"];

/// 过滤已下线的预置源，并报告是否发生了变化（用于回写）
fn drop_retired(items: Vec<SourceItem>) -> (Vec<SourceItem>, bool) {
    let before = items.len();
    let kept: Vec<SourceItem> = items
        .into_iter()
        .filter(|s| !RETIRED_PRESET_IDS.contains(&s.id.as_str()))
        .collect();
    let changed = kept.len() != before;
    (kept, changed)
}

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

fn sources_path() -> PathBuf {
    app_dir().join("sources.json")
}

pub fn validate_source(s: &SourceItem) -> Result<(), String> {
    crate::instances::validate_id(&s.id)?;
    if s.name.trim().is_empty() || s.name.chars().count() > 60 {
        return Err("源名称需为 1-60 字符".into());
    }
    match s.kind {
        SourceKind::DirectUrl => {
            if !(s.url.starts_with("https://") || s.url.starts_with("http://")) {
                return Err(format!("URL 必须以 http(s):// 开头: {}", s.url));
            }
        }
        SourceKind::GithubRepo => {
            let repo = s.repo.trim();
            let ok = repo.split('/').count() == 2
                && repo.split('/').all(|p| {
                    !p.is_empty()
                        && p.chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                });
            if !ok {
                return Err(format!("repo 需为 owner/name 形态: {}", s.repo));
            }
        }
    }
    Ok(())
}

/// 旧平铺订阅 → direct-url 源（只读迁移，不回写旧文件）
fn migrate_legacy() -> Option<Vec<SourceItem>> {
    let legacy = app_dir().join("subscriptions.json");
    let text = fs::read_to_string(legacy).ok()?;
    #[derive(Deserialize)]
    struct OldFile {
        items: Vec<OldItem>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OldItem {
        name: String,
        url: String,
        #[serde(default)]
        note: String,
    }
    let old: OldFile = serde_json::from_str(&text).ok()?;
    if old.items.is_empty() {
        return None;
    }
    Some(
        old.items
            .into_iter()
            .enumerate()
            .map(|(i, o)| SourceItem {
                id: format!("migrated-{}", i),
                name: o.name,
                kind: SourceKind::DirectUrl,
                url: o.url,
                repo: String::new(),
                asset: default_asset(),
                note: o.note,
                group: "直链".into(),
                collapsed: false,
                latest_enabled: false,
                open_in_new_page: false,
            })
            // 迁移产物复验：非法 URL 直接丢弃，避免页面按钮打开非 http(s)
            .filter(|s| validate_source(s).is_ok())
            .collect(),
    )
}

fn write_sources(items: &[SourceItem]) -> Result<(), String> {
    write_sources_to(&sources_path(), items)
}

fn write_sources_to(path: &std::path::Path, items: &[SourceItem]) -> Result<(), String> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let file = SourcesFile { sources: items.to_vec() };
    fs::write(path, serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// 读取已有源表并清掉已下线的预置源，发生变化则回写一次
fn load_existing(path: &std::path::Path) -> Vec<SourceItem> {
    let loaded = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<SourcesFile>(&s).ok())
        .map(|f| f.sources)
        .unwrap_or_default();
    let (items, changed) = drop_retired(loaded);
    if changed {
        let _ = write_sources_to(path, &items);
    }
    items
}

#[tauri::command]
pub async fn list_sources() -> Vec<SourceItem> {
    let path = sources_path();
    if !path.exists() {
        let items = migrate_legacy().unwrap_or_else(preset_sources);
        let _ = write_sources(&items);
        return items;
    }
    load_existing(&path)
}

#[tauri::command]
pub async fn save_sources(items: Vec<SourceItem>) -> Result<Vec<SourceItem>, String> {
    for s in &items {
        validate_source(s)?;
    }
    write_sources(&items)?;
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct() -> SourceItem {
        SourceItem {
            id: "s1".into(),
            name: "n".into(),
            kind: SourceKind::DirectUrl,
            url: "https://x/y.jar".into(),
            repo: String::new(),
            asset: default_asset(),
            note: String::new(),
            group: "g".into(),
            collapsed: false,
            latest_enabled: true,
            open_in_new_page: false,
        }
    }

    #[test]
    fn validation_rules() {
        assert!(validate_source(&direct()).is_ok());
        let mut bad = direct();
        bad.url = "ftp://x".into();
        assert!(validate_source(&bad).is_err());
        let mut g = direct();
        g.kind = SourceKind::GithubRepo;
        // kind 改为 github-repo 但 repo 为空 → 校验失败
        assert!(validate_source(&g).is_err());
    }

    #[test]
    fn github_repo形态校验() {
        let mut g = direct();
        g.kind = SourceKind::GithubRepo;
        g.url = String::new();
        g.repo = "Anuken/Mindustry".into();
        assert!(validate_source(&g).is_ok());
        g.repo = "bad repo".into();
        assert!(validate_source(&g).is_err());
        g.repo = "a/b/c".into();
        assert!(validate_source(&g).is_err());
    }

    #[test]
    fn preset_and_migrate_json_roundtrip() {
        let s = serde_json::to_string(&SourcesFile { sources: preset_sources() }).unwrap();
        assert!(s.contains("github-repo") && s.contains("latestEnabled"));
        let back: SourcesFile = serde_json::from_str(&s).unwrap();
        // 预置只留官方仓库索引，直链已随中心索引下线
        assert_eq!(back.sources.len(), 1);
        assert_eq!(back.sources[0].kind, SourceKind::GithubRepo);
        assert!(back.sources[0].group == "官方源");
    }

    #[test]
    fn retired_presets_are_dropped() {
        let mut kept = direct();
        kept.id = "user-added".into();
        let mut a = direct();
        a.id = "src-github-official".into();
        let mut b = direct();
        b.id = "src-v146-direct".into();
        let (out, changed) = drop_retired(vec![a, kept, b]);
        assert!(changed);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "user-added");
        // 没有预置源时不应误触发回写
        let (same, changed) = drop_retired(out);
        assert!(!changed);
        assert_eq!(same.len(), 1);
    }

    #[test]
    fn legacy_flat_item_parses() {
        let legacy = r#"{"items":[{"name":"a","url":"https://x/y.jar","note":"n"}]}"#;
        #[derive(Deserialize)]
        struct OldFile {
            items: Vec<serde_json::Value>,
        }
        let v: OldFile = serde_json::from_str(legacy).unwrap();
        assert_eq!(v.items.len(), 1);
    }

    /// 真实迁移：老 sources.json 里的两条预置源被清掉并回写，用户自添的源保留
    #[test]
    fn loading_existing_file_clears_retired_presets() {
        let dir = std::env::temp_dir().join(format!("yyl-src-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("sources.json");

        let mut user = direct();
        user.id = "user-added".into();
        user.name = "我自己加的".into();
        let mut a = direct();
        a.id = "src-github-official".into();
        let mut b = direct();
        b.id = "src-v146-direct".into();
        write_sources_to(&path, &[a, b, user]).unwrap();

        let got = load_existing(&path);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "user-added");

        // 回写已生效：再读一次仍然只剩一条，且不会反复改写
        let again = load_existing(&path);
        assert_eq!(again.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
