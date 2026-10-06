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
    /// 标语：一句话说明这个默认游戏端是什么；服务端优先，缺失时客户端补（见 fallback_slogan）
    #[serde(default)]
    pub slogan: String,
    /// "client" | "server"：当前只接客户端，服务端稍后接入
    #[serde(default = "d_scope")]
    pub scope: String,
    /// 服务端标注好的版本快照。非空 → 客户端直接用，不再直连 api.github.com（403 由此消失）
    #[serde(default)]
    pub versions: Vec<CentralVersion>,
    /// 服务端标注版本失败的原因。有值 → 客户端只报错、保留本地缓存，不自己再打一遍网络
    #[serde(default)]
    pub versions_error: String,
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
    /// 客户端过滤/排序规则的版本号；与 RULES_VERSION 不一致的旧缓存整份作废，
    /// 否则用户会在 15 分钟 TTL 内继续看到老规则的结果（例如还没滤掉的服务端包）
    #[serde(default)]
    pub rules: u32,
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
    /// 源 id → 本次拉取失败原因（便于排查，成功时清除）
    #[serde(default)]
    pub errors: BTreeMap<String, String>,
    /// 中心下发的镜像清单：跟着缓存一起留着，中心服务器临时不可达时设置页也不会空
    #[serde(default)]
    pub mirrors: Mirrors,
    /// 每日信息 Markdown 原文：跟着缓存留一份，中心临时不可达时公告不会突然消失
    #[serde(default)]
    pub info_bar: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CentralIndex {
    /// "remote" = 来自中心化服务器；"builtin" = 内置兜底
    pub source: String,
    /// 失败原因（成功为空）
    pub note: String,
    pub items: Vec<CentralItem>,
    /// 中心下发的镜像清单（工坊镜像等），客户端不再手填地址
    #[serde(default)]
    pub mirrors: Mirrors,
    /// 每日信息：Markdown 原文（首页底部公告）
    #[serde(default)]
    pub info_bar: String,
}

/// 工坊镜像条目：名称 + 前缀地址
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MirrorItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
}

/// 中心下发的镜像清单
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Mirrors {
    #[serde(default)]
    pub workshop: Vec<MirrorItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    #[serde(default)]
    items: Vec<CentralItem>,
    #[serde(default)]
    mirrors: Mirrors,
    /// 每日信息 Markdown 原文
    #[serde(default)]
    info_bar: String,
}

/// 镜像清单裁剪：只留 http(s)、名称与地址都非空，条数与长度封顶
fn sanitize_mirrors(m: Mirrors) -> Mirrors {
    let workshop = m
        .workshop
        .into_iter()
        .filter(|x| x.url.starts_with("https://") || x.url.starts_with("http://"))
        .map(|x| MirrorItem {
            name: {
                let n: String = x.name.trim().chars().take(40).collect();
                if n.is_empty() {
                    x.url.chars().take(40).collect()
                } else {
                    n
                }
            },
            url: x.url.trim().chars().take(300).collect(),
        })
        .take(20)
        .collect();
    Mirrors { workshop }
}

/// 内置中心索引：服务器不可用时下载中心仍可用。
/// 字段与 `server/central.mjs` 的 SOURCES 一一对齐（含 slogan），
/// 保证「服务器在线 / 离线」看到的是同一套名称、logo、标语、标签。
/// 当前只收客户端源，服务端源待后续接入。
fn builtin_items() -> Vec<CentralItem> {
    vec![
        CentralItem {
            id: "src-mindustry-official".into(),
            name: "Mindustry 官方".into(),
            kind: "github-repo".into(),
            url: "https://github.com/Anuken/Mindustry/releases".into(),
            repo: "Anuken/Mindustry".into(),
            asset: d_asset(),
            note: "Anuken/Mindustry 官方 releases".into(),
            group: "中心".into(),
            tags: vec!["官方".into()],
            size: 0,
            logo: "https://github.com/Anuken.png?size=96".into(),
            slogan: "Anuken 官方原版 · 紧跟上游 stable".into(),
            scope: d_scope(),
            versions: Vec::new(),
            versions_error: String::new(),
        },
        CentralItem {
            id: "src-mdtbbs-v8".into(),
            name: "Mindustry v8".into(),
            kind: "file-list".into(),
            url: "https://file.mdtbbs.cn/category/Mindustry/v8".into(),
            repo: String::new(),
            asset: d_asset(),
            note: "mdtbbs 文件站分类目录".into(),
            group: "中心".into(),
            tags: vec!["v8".into()],
            size: 0,
            logo: "https://file.mdtbbs.cn/favicon.svg".into(),
            slogan: "mdtbbs 文件站 · 国内直连目录".into(),
            scope: d_scope(),
            versions: Vec::new(),
            versions_error: String::new(),
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
            tags: vec!["GitHub".into()],
            size: 0,
            logo: "https://github.com/TinyLake.png?size=96".into(),
            slogan: "TinyLake 分支 · 桌面端增强整合".into(),
            scope: d_scope(),
            versions: Vec::new(),
            versions_error: String::new(),
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
        it.note = it.note.trim().chars().take(80).collect();
        // 客户端打标：服务器给过就用服务器的，没给就用同一套规则本地补
        it.tags = derive_tags(&it);
        it.slogan = it.slogan.trim().chars().take(60).collect();
        if it.slogan.is_empty() {
            it.slogan = fallback_slogan(&it);
        }
        out.push(it);
    }
    out
}

fn kind_tag(kind: &str) -> &'static str {
    match kind {
        "direct-url" => "直链",
        "github-repo" => "仓库",
        "file-list" => "目录",
        _ => "",
    }
}

/// 客户端打标：与 `server/central.mjs` 的 `tagsOf` 同一套规则
/// （类型标签 + 分组 + 自带标签 → 去空 → 截 16 字 → 去重 → 封顶 8）。
/// 两边规则必须一致，否则同一个源在线/离线会看到两套标签。
fn derive_tags(it: &CentralItem) -> Vec<String> {
    fn add(out: &mut Vec<String>, raw: &str) {
        let t: String = raw.trim().chars().take(16).collect();
        if !t.is_empty() && out.len() < 8 && !out.iter().any(|x| x == &t) {
            out.push(t);
        }
    }
    let mut out: Vec<String> = Vec::new();
    add(&mut out, kind_tag(&it.kind));
    add(&mut out, &it.group);
    for t in &it.tags {
        add(&mut out, t);
    }
    out
}

/// 标语兜底表：与 `server/central.mjs` 的 SOURCES.slogan 对齐
fn builtin_slogan(id: &str) -> &'static str {
    match id {
        "src-mindustry-official" => "Anuken 官方原版 · 紧跟上游 stable",
        "src-mdtbbs-v8" => "mdtbbs 文件站 · 国内直连目录",
        "src-minedx" => "TinyLake 分支 · 桌面端增强整合",
        _ => "",
    }
}

/// 服务器没下发标语时的本地兜底：先查内置表，再按类型给一句通用的
fn fallback_slogan(it: &CentralItem) -> String {
    let known = builtin_slogan(&it.id);
    if !known.is_empty() {
        return known.to_string();
    }
    let s = match it.kind.as_str() {
        "github-repo" if !it.repo.is_empty() => format!("GitHub 仓库 · {}", it.repo),
        "github-repo" => "GitHub 仓库源".to_string(),
        "file-list" => "文件站目录 · 客户端解析版本".to_string(),
        "direct-url" => "直链下载源".to_string(),
        _ => String::new(),
    };
    s.chars().take(60).collect()
}

/// 从错误体里抠出 GitHub 的 message 字段，取不到就截断正文
fn message_of(body: &str) -> String {
    if let Some(i) = body.find("\"message\"") {
        let rest = &body[i + "\"message\"".len()..];
        if let Some(j) = rest.find('"') {
            let val = &rest[j + 1..];
            if let Some(k) = val.find('"') {
                return val[..k].chars().take(140).collect();
            }
        }
    }
    body.trim().chars().take(80).collect()
}

/// curl 默认对 4xx/5xx 也返回 0（不加 -f），所以必须把状态码单独取回来判断，
/// 否则 rate limit 的错误 JSON 会被当成正常数据丢给解析器。
/// 按链接拉一段文本（每日信息用）：走 curl，支持代理，只允许 http(s)。
/// 放在 Rust 侧而不是前端 fetch：WebView 里直接请求会撞 CORS，也不走代理设置。
#[tauri::command]
pub async fn fetch_text(url: String, proxy: String) -> Result<String, String> {
    let url = url.trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("链接需以 http(s):// 开头".into());
    }
    tauri::async_runtime::spawn_blocking(move || http_get(&url, &proxy, &[], 15))
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
        // 把 curl 的退出码翻译成人话，设置页直接显示这条
        .map_err(|e| {
            if e.contains("退出码 28") {
                "链接读取超时（服务不可达或响应过慢）".to_string()
            } else if e.contains("退出码 6") || e.contains("Could not resolve") {
                "域名解析失败".to_string()
            } else if e.contains("退出码 7") {
                "连接被拒绝".to_string()
            } else {
                e
            }
        })
}

fn http_get(url: &str, proxy: &str, headers: &[&str], timeout: u32) -> Result<String, String> {
    let mut cmd = crate::cmdutil::no_console("curl");
    cmd.args(["-sSL", "--compressed", "-w", "\n%{http_code}", "--max-time", &timeout.to_string()]);
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
            "curl 退出码 {}",
            out.status.code().unwrap_or(-1)
        ));
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let (body, code) = match raw.rsplit_once('\n') {
        Some((b, c)) if c.len() == 3 && c.chars().all(|d| d.is_ascii_digit()) => {
            (b.to_string(), c.to_string())
        }
        _ => (raw.trim_end().to_string(), String::new()),
    };
    if !code.is_empty() && !code.starts_with('2') {
        return Err(format!("HTTP {code}: {}", message_of(&body)));
    }
    Ok(body)
}

fn fetch_sync(base: &str, proxy: &str) -> Result<CentralIndex, String> {
    fetch_sync_with(base, proxy, 15)
}

/// timeout 可调：新鲜缓存只需要「短探一下有没有新源」，不必按 15s 等
fn fetch_sync_with(base: &str, proxy: &str, timeout: u32) -> Result<CentralIndex, String> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Ok(CentralIndex {
            source: "builtin".into(),
            note: String::new(),
            items: sanitize(builtin_items()),
            mirrors: Mirrors::default(),
            info_bar: String::new(),
        });
    }
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err("中心化服务器地址需以 http(s):// 开头".into());
    }

    let url = format!("{base}/index.json");
    match http_get(&url, proxy, &[], timeout)
        .and_then(|text| {
            serde_json::from_str::<IndexFile>(&text).map_err(|e| format!("索引 JSON 解析失败: {e}"))
        })
        .map(|f| {
            (
                sanitize(f.items),
                sanitize_mirrors(f.mirrors),
                f.info_bar.chars().take(8000).collect::<String>(),
            )
        })
    {
        Ok((items, mirrors, info_bar)) if !items.is_empty() => Ok(CentralIndex {
            source: "remote".into(),
            note: String::new(),
            items,
            mirrors,
            info_bar,
        }),
        Ok(_) => Ok(CentralIndex {
            source: "builtin".into(),
            note: "中心化服务器返回空索引，已用内置索引".into(),
            items: sanitize(builtin_items()),
            mirrors: Mirrors::default(),
            info_bar: String::new(),
        }),
        Err(e) => Ok(CentralIndex {
            source: "builtin".into(),
            note: format!("中心化服务器不可达：{e}；已用内置索引"),
            items: sanitize(builtin_items()),
            // 内置索引没有镜像清单与每日信息；但都不能因此消失，保留上次下发的
            mirrors: Mirrors::default(),
            info_bar: String::new(),
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
    crate::cmdutil::app_root()
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

/// 客户端可下载资产：名称**严格以「Mindustry」开头**、排除 apk、排除服务端产物。
/// 与 `server/central.mjs` 的 `isClientAsset` 同一套规则（两边都打标，规则必须一致）。
/// 这样 `dependencies.jar` / `assets.jar` / `desktop-release.jar` / `dexed-*.loader.jar`
/// 这些非客户端本体的包不会出现在下载按钮里。
fn is_client_asset(name: &str) -> bool {
    if !name.starts_with("Mindustry") {
        return false;
    }
    !name.to_ascii_lowercase().ends_with(".apk") && !is_server_asset(name)
}

/// 只保留「能下载到客户端」的版本：
/// - github 源：资产按 `is_client_asset` 过滤，过滤后一条不剩的版本整体隐藏
///   （纯服务端/测试版本连标签都不该出现）
/// - file-list 源：条目是目录、本来就没有资产，原样保留
/// - direct-url 源：资产名由用户自定，不做前缀假设
fn client_only(it: &CentralItem, list: Vec<CentralVersion>) -> Vec<CentralVersion> {
    if it.kind != "github-repo" {
        return list;
    }
    list.into_iter()
        .filter_map(|mut v| {
            v.assets.retain(|a| is_client_asset(&a.name));
            if v.assets.is_empty() {
                None
            } else {
                Some(v)
            }
        })
        .collect()
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
        // 单页可达 659KB（官方仓库 100 条），慢链路下 20s 不够用
        let text = http_get(&url, proxy, &headers, 45)?;
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
                if !is_client_asset(&a.name) {
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

/// 取名称里第一段版本号（可带一个小数点）当排序键：`build-160.5-stable` → (160, 5)。
/// 目录页给的是「旧 → 新」的顺序，必须自己按版本号排，否则第一条会被当成最新版。
fn ver_key(s: &str) -> (u64, u64) {
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            break;
        }
        chars.next();
    }
    let mut major: u64 = 0;
    while let Some(&c) = chars.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        chars.next();
        major = major.saturating_mul(10).saturating_add(c as u64 - 48);
    }
    let mut minor: u64 = 0;
    // 小数点必须在这里才消费：上一轮循环是靠 peek 退出的，'.' 还在流里
    if chars.peek() == Some(&'.') {
        chars.next();
        while let Some(&c) = chars.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            chars.next();
            minor = minor.saturating_mul(10).saturating_add(c as u64 - 48);
        }
    }
    (major, minor)
}

/// 解析文件站目录：条目是 <a class="term-file ...">，名称取最后一个 <span>。
/// 返回前按版本号降序排（新 → 旧），保证首条就是最新版。
fn parse_file_list(html: &str, base: &str) -> Vec<CentralVersion> {
    let entries = parse_term_entries(html, base);
    let mut out: Vec<CentralVersion> = entries
        .iter()
        .filter(|e| e.folder)
        .map(|e| CentralVersion {
            tag: e.name.clone(),
            title: e.name.clone(),
            date: String::new(),
            page_url: e.url.clone(),
            assets: Vec::new(),
            dropped: 0,
            folder: true,
        })
        .collect();

    // 有些目录直接就是一堆文件（没有子目录）：那就把文件本身当成可下载条目，
    // 否则用户点进去只能看到「打开目录」，永远下不了东西。
    if out.is_empty() {
        out = entries
            .iter()
            .filter(|e| !e.folder && !is_server_asset(&e.name))
            .map(|e| CentralVersion {
                tag: e.name.clone(),
                title: e.name.clone(),
                date: String::new(),
                page_url: e.url.clone(),
                assets: vec![CentralAsset {
                    name: e.name.clone(),
                    url: download_url(&e.url),
                    size: e.size,
                }],
                dropped: 0,
                folder: false,
            })
            .collect();
    }

    // 目录页是「旧 → 新」，这里反过来；同版本号时按名称降序兜底，保证顺序稳定
    out.sort_by(|a, b| {
        ver_key(&b.tag)
            .cmp(&ver_key(&a.tag))
            .then_with(|| b.tag.cmp(&a.tag))
    });
    out
}

/// 文件站条目（目录页里既可能是文件夹也可能是文件）
struct TermEntry {
    name: String,
    /// 详情页地址（`/Mindustry/v8/xxx/Mindustry.jar` 这种）
    url: String,
    folder: bool,
    size: u64,
}

/// 去掉标签只留文字（`term-file-name` 里可能夹一个空图标 span）
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 取块内某个 class 的 <div> 纯文本
fn block_text(block: &str, cls: &str) -> Option<String> {
    let pat = format!("class=\"{cls}\"");
    let i = block.find(&pat)?;
    let after = &block[i + pat.len()..];
    let gt = after.find('>')?;
    let body = &after[gt + 1..];
    let end = body.find("</div>")?;
    let text = strip_tags(&body[..end]);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// `75.0 MB` / `1.2 GB` / `900 KB` → 字节
fn parse_size(s: &str) -> u64 {
    let s = s.trim();
    let cut = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    let num: f64 = s[..cut].trim().parse().unwrap_or(0.0);
    let unit = s[cut..].trim().to_ascii_uppercase();
    let mul = match unit.as_str() {
        "GB" | "G" => 1024.0 * 1024.0 * 1024.0,
        "MB" | "M" => 1024.0 * 1024.0,
        "KB" | "K" => 1024.0,
        _ => 1.0,
    };
    (num * mul) as u64
}

fn block_size(block: &str) -> u64 {
    let Some(i) = block.find("class=\"size\"") else {
        return 0;
    };
    let after = &block[i..];
    let Some(gt) = after.find('>') else { return 0 };
    let body = &after[gt + 1..];
    let Some(end) = body.find('<') else { return 0 };
    parse_size(&body[..end])
}

/// 文件站的实际下载直链是 `/d/<原路径>`：
/// 目录页里的 href 是**详情页**（返回 text/html），真正的 jar 在详情页的下载按钮上，
/// 指向 `/d/` + 同一路径。直接由详情页地址推出，省掉「每个文件再拉一次详情页」。
fn download_url(detail: &str) -> String {
    let Some((scheme, rest)) = detail.split_once("://") else {
        return detail.to_string();
    };
    match rest.split_once('/') {
        Some((host, path)) => format!("{scheme}://{host}/d/{path}"),
        None => detail.to_string(),
    }
}

/// 解析目录页里的所有 term-file 条目（文件夹与文件都算）
fn parse_term_entries(html: &str, base: &str) -> Vec<TermEntry> {
    let mut out: Vec<TermEntry> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut idx = 0usize;
    while out.len() < 500 {
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
        let folder = block.contains("term-folder-link");
        // 名称优先取 term-file-name 里的纯文本：目录页与文件页的结构不同，
        // 老代码取「最后一个 <span>」在文件页会取到「最近: 2026-10-06 07:32」。
        // 没有该结构时退回老写法（最后一个 <span>），兼容结构更简单的镜像站。
        let name = block_text(block, "term-file-name")
            .or_else(|| {
                let p = block.rfind("<span>")?;
                let q = block[p..].find("</span>")?;
                let t = block[p + 6..p + q].trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            })
            .unwrap_or_default();
        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        out.push(TermEntry {
            name,
            url: abs_url(&href, base),
            folder,
            size: block_size(block),
        });
    }
    out
}

/// 展开文件站里的某个目录，列出可直接下载的文件。
/// 目录页给的是**详情页**地址，这里换算成 `/d/…` 直链。
fn list_folder_files_sync(url: &str, proxy: &str) -> Result<Vec<CentralAsset>, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("目录地址需以 http(s):// 开头".into());
    }
    let html = http_get(url, proxy, &[], 25)?;
    let files: Vec<CentralAsset> = parse_term_entries(&html, url)
        .into_iter()
        .filter(|e| !e.folder && !is_server_asset(&e.name))
        .map(|e| CentralAsset {
            name: e.name,
            url: download_url(&e.url),
            size: e.size,
        })
        .collect();
    if files.is_empty() {
        return Err("这个目录里没解析到可下载文件".into());
    }
    Ok(files)
}

#[tauri::command]
pub async fn list_folder_files(url: String, proxy: String) -> Result<Vec<CentralAsset>, String> {
    tauri::async_runtime::spawn_blocking(move || list_folder_files_sync(&url, &proxy))
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
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

/// 客户端「只留客户端资产 + 目录按版本降序」这套规则的版本号。规则一变就 +1。
/// 老缓存（含比这个数小的、或旧版本写出的没有该字段的缓存）会被整份丢弃后重扫。
const RULES_VERSION: u32 = 2;

/// 规则版本不一致的缓存不能复用：整份作废后重扫。
/// （旧版本写出的缓存没有 rules 字段，serde 默认成 0，同样会走到这里。）
fn usable_cache(idx: VersionIndex) -> VersionIndex {
    if idx.rules == RULES_VERSION {
        idx
    } else {
        VersionIndex {
            rules: RULES_VERSION,
            ..Default::default()
        }
    }
}

/// 服务端已经标注过版本的源：直接用，一次网络都不打。
/// 这就是「403 消掉」的落点——客户端不再自己访问 api.github.com。
/// 返回 None 表示服务端没管这个源（如 file-list 目录页），由客户端自己解析。
fn preloaded(it: &CentralItem) -> Option<Result<Vec<CentralVersion>, String>> {
    if !it.versions.is_empty() {
        return Some(Ok(it.versions.clone()));
    }
    let e = it.versions_error.trim();
    if !e.is_empty() {
        return Some(Err(e.chars().take(160).collect()));
    }
    None
}

/// 开机预取所有源的版本到本地；并行拉取。
/// 拉失败的源沿用本地缓存，全部失败则缓存原样保留。
///
/// 实现放在阻塞线程池：curl 单次要几秒到二十秒，占主线程会让窗口「未响应」。
fn sync_blocking(base: &str, proxy: &str, force: bool) -> Result<VersionIndex, String> {
    let t0 = std::time::Instant::now();

    // 新鲜缓存直接用：GitHub 未认证只有 60 次/小时，反复开应用不该每次都打请求
    const SYNC_TTL_MS: u64 = 15 * 60 * 1000;
    // 规则版本不一致 → 老规则产出的数据整份作废，不与新结果混用
    let base_idx = usable_cache(read_index());
    let cache_fresh = !force
        && base_idx.synced_at > 0
        && !base_idx.sources.is_empty()
        && now_ms().saturating_sub(base_idx.synced_at) < SYNC_TTL_MS;

    let index = if cache_fresh {
        // 缓存新鲜时仍短探一次索引：服务端**新增的源**必须立刻出现版本，
        // 否则它的 id 不在缓存里，用户要等满 15 分钟 TTL 才看得到（实测踩到）。
        // 探不通就用缓存，不为一次探测把启动拖到 15s。
        match fetch_sync_with(base, proxy, 5) {
            // 源、镜像清单、每日信息都没变，才继续用缓存
            Ok(idx)
                if idx.items.iter().all(|it| base_idx.sources.contains_key(&it.id))
                    && idx.mirrors.workshop == base_idx.mirrors.workshop
                    && idx.info_bar == base_idx.info_bar =>
            {
                return Ok(base_idx);
            }
            Ok(idx) => idx,
            Err(_) => return Ok(base_idx),
        }
    } else {
        fetch_sync(base, proxy)?
    };
    let items = index.items;
    let mirrors = index.mirrors;
    let info_bar = index.info_bar;
    let total = items.len();

    // 代理串先降为 &str（Copy），才能被多个线程闭包同时捕获
    let proxy_ref: &str = proxy;
    let results: Vec<Result<Vec<CentralVersion>, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = items
            .iter()
            .map(|it| {
                s.spawn(move || preloaded(it).unwrap_or_else(|| versions_of(it, proxy_ref)))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("线程崩溃".into())))
            .collect()
    });

    let mut idx = base_idx;
    let mut ok = 0usize;
    for (it, res) in items.iter().zip(results.into_iter()) {
        // 服务端快照和客户端自解析都要过一遍「只留客户端」的过滤
        match res.map(|list| client_only(it, list)) {
            Ok(list) if !list.is_empty() => {
                merge_index(&mut idx, &it.id, list);
                idx.errors.remove(&it.id);
                ok += 1;
            }
            Ok(_) => {
                idx.errors
                    .insert(it.id.clone(), "没有客户端版本（只剩服务端/测试包）".into());
            }
            Err(e) => {
                idx.errors.insert(it.id.clone(), e.chars().take(160).collect());
            }
        }
    }
    if ok > 0 {
        idx.synced_at = now_ms();
    }
    idx.rules = RULES_VERSION;
    idx.mirrors = mirrors;
    idx.info_bar = info_bar;
    idx.ok = ok;
    idx.total = total;
    idx.sync_ms = t0.elapsed().as_millis() as u64;
    let _ = save_index(&idx);
    Ok(idx)
}

#[tauri::command]
pub async fn sync_central_versions(
    base: String,
    proxy: String,
    force: Option<bool>,
) -> Result<VersionIndex, String> {
    let force = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || sync_blocking(&base, &proxy, force))
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_index_is_all_client_sources() {
        let items = sanitize(builtin_items());
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| !i.name.is_empty() && !i.tags.is_empty()));
        assert!(items.iter().all(|i| i.scope == "client"));
        assert!(items.iter().any(|i| i.kind == "file-list"));
        assert!(items.iter().any(|i| i.repo == "Anuken/Mindustry"));
        // 官方源排在最前
        assert_eq!(items[0].repo, "Anuken/Mindustry");
        // 每个默认游戏端都要有 logo + 标语（用户要求）
        assert!(items.iter().all(|i| !i.logo.is_empty()));
        assert!(items.iter().all(|i| !i.slogan.is_empty()));
        assert!(items.iter().all(|i| i.slogan.chars().count() <= 60));
    }

    #[test]
    fn client_tagging_matches_server_rules() {
        // 与 server/central.mjs 的 tagsOf 同序：类型 → 分组 → 自带标签
        let items = sanitize(builtin_items());
        assert_eq!(items[0].tags, vec!["仓库", "中心", "官方"]);
        assert_eq!(items[1].tags, vec!["目录", "中心", "v8"]);
        assert_eq!(items[2].tags, vec!["仓库", "中心", "GitHub"]);
    }

    #[test]
    fn server_slogan_wins_and_client_fills_the_gap() {
        // 服务器给了就用服务器的
        let mut with_slogan = builtin_items().remove(0);
        with_slogan.slogan = "服务器下发的标语".into();
        assert_eq!(sanitize(vec![with_slogan])[0].slogan, "服务器下发的标语");

        // 服务器没给：内置表兜底
        let mut no_slogan = builtin_items().remove(0);
        no_slogan.slogan = String::new();
        assert_eq!(sanitize(vec![no_slogan])[0].slogan, builtin_slogan("src-mindustry-official"));

        // 陌生源：按类型给通用兜底，不能是空串
        let mut unknown = builtin_items().remove(0);
        unknown.id = "someone-else".into();
        unknown.slogan = String::new();
        let out = sanitize(vec![unknown]);
        assert!(out[0].slogan.contains("GitHub 仓库"));
    }

    #[test]
    fn server_preloaded_versions_skip_the_network() {
        // 服务端标注过 → 直接用，不打网络（403 消失的落点）
        let mut it = builtin_items().remove(0);
        it.versions = vec![CentralVersion {
            tag: "v999".into(),
            title: "服务端快照".into(),
            ..Default::default()
        }];
        let got = preloaded(&it).expect("应命中服务端快照").unwrap();
        assert_eq!(got[0].tag, "v999");

        // 服务端明确失败 → 只报错，不重试网络
        let mut failed = builtin_items().remove(0);
        failed.versions_error = "GitHub HTTP 403".into();
        assert!(preloaded(&failed).unwrap().is_err());

        // 服务端没管（file-list）→ 交给客户端解析
        let file_list = builtin_items().remove(1);
        assert!(preloaded(&file_list).is_none());
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
            slogan: String::new(),
            scope: d_scope(),
            versions: Vec::new(),
            versions_error: String::new(),
        };
        assert!(sanitize(vec![bad]).is_empty());

        let mut ok = builtin_items().pop().unwrap();
        ok.tags = (0..40).map(|i| format!("t{i}")).collect();
        ok.name = "x".repeat(500);
        ok.logo = "javascript:alert(1)".into();
        ok.slogan = "s".repeat(500);
        let out = sanitize(vec![ok]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tags.len(), 8);
        assert!(out[0].name.chars().count() <= 60);
        assert!(out[0].slogan.chars().count() <= 60);
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
        // 目录页是旧→新，解析后必须反过来，首条才是最新版
        assert_eq!(list[0].tag, "build-160-stable");
        assert_eq!(list[1].tag, "build-157.1-stable");
        assert_eq!(
            list[0].page_url,
            "https://file.mdtbbs.cn/category/Mindustry/v8/build-160-stable"
        );
        assert!(list[0].folder);
    }

    /// 真实现场：mdtbbs v8 目录页第 1 条是 build-147-prerelease、最后一条才是 build-160.5-stable，
    /// 直接取第一条会把 147 当成「最新」（用户报的 bug）。
    #[test]
    fn file_list_is_sorted_newest_first() {
        let html = r#"
          <a href="/a/build-147-prerelease" class="term-file"><span>build-147-prerelease</span></a>
          <a href="/a/build-150.1-prerelease" class="term-file"><span>build-150.1-prerelease</span></a>
          <a href="/a/build-159.7-stable" class="term-file"><span>build-159.7-stable</span></a>
          <a href="/a/build-160-stable" class="term-file"><span>build-160-stable</span></a>
          <a href="/a/build-160.5-stable" class="term-file"><span>build-160.5-stable</span></a>"#;
        let list = parse_file_list(html, "https://h/p");
        assert_eq!(list[0].tag, "build-160.5-stable", "160.5 必须排在 160 前面");
        assert_eq!(list[1].tag, "build-160-stable");
        assert_eq!(list.last().unwrap().tag, "build-147-prerelease");
    }

    /// 规则升级后老缓存必须整份作废，否则用户会在 TTL 内继续看到旧过滤结果
    #[test]
    fn cache_from_older_rules_is_discarded() {
        let mut old = VersionIndex::default();
        old.rules = RULES_VERSION - 1;
        old.synced_at = now_ms();
        old.sources.insert("s1".into(), Vec::new());
        let out = usable_cache(old);
        assert_eq!(out.rules, RULES_VERSION);
        assert_eq!(out.synced_at, 0, "老规则缓存不能被视为新鲜");
        assert!(out.sources.is_empty());

        // 没写 rules 字段的老缓存（serde 默认 0）同样作废
        let legacy: VersionIndex = serde_json::from_str(r#"{"syncedAt":123,"ok":1,"total":1,"syncMs":1}"#)
            .expect("老缓存 JSON 必须能解析");
        assert_eq!(legacy.rules, 0);
        assert_eq!(usable_cache(legacy).synced_at, 0);

        // 规则一致的原样保留
        let mut fresh = VersionIndex::default();
        fresh.rules = RULES_VERSION;
        fresh.synced_at = 12345;
        assert_eq!(usable_cache(fresh).synced_at, 12345);
    }

    #[test]
    fn version_key_reads_major_and_minor() {        assert_eq!(ver_key("build-160.5-stable"), (160, 5));
        assert_eq!(ver_key("build-160-stable"), (160, 0));
        assert_eq!(ver_key("build-147-prerelease"), (147, 0));
        assert_eq!(ver_key("v2.0.1-beta"), (2, 0));
        assert_eq!(ver_key("no-digits"), (0, 0));
    }

    /// 只留客户端：严格「Mindustry」开头 + 非 apk + 非服务端产物
    #[test]
    fn only_mindustry_prefixed_non_apk_jars_are_client_assets() {
        assert!(is_client_asset("Mindustry.jar"));
        assert!(is_client_asset("MindustryX-2026.10.02.B502-Desktop.jar"));
        assert!(is_client_asset("MindustryX-2026.10.02.B502-Desktop-SDL3.jar"));
        // 严格开头：依赖 / 资源 / loader 包装都不是客户端本体
        assert!(!is_client_asset("dexed-MindustryX-2026.10.02.B502.loader.jar"));
        assert!(!is_client_asset("dependencies.jar"));
        assert!(!is_client_asset("assets.jar"));
        assert!(!is_client_asset("desktop-release.jar"));
        // apk 与服务端产物
        assert!(!is_client_asset("MindustryX-2026.10.02.B502-Android.apk"));
        assert!(!is_client_asset("MindustryX-server.jar"));
        assert!(!is_client_asset("mindustry.jar"));
    }

    /// 过滤后没有客户端资产的版本整条隐藏；file-list 目录条目不受影响
    #[test]
    fn versions_without_client_assets_are_hidden() {
        let src = builtin_items().remove(0); // github-repo
        let mk = |tag: &str, names: &[&str]| CentralVersion {
            tag: tag.into(),
            assets: names
                .iter()
                .map(|n| CentralAsset {
                    name: (*n).into(),
                    url: "https://x".into(),
                    size: 0,
                })
                .collect(),
            ..Default::default()
        };
        let out = client_only(
            &src,
            vec![
                mk("only-server", &["server-2026.10.02.B502.jar", "MindustryX-server.jar"]),
                mk("mixed", &["Mindustry.jar", "assets.jar"]),
                mk("empty", &[]),
            ],
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tag, "mixed");
        assert_eq!(out[0].assets.len(), 1);
        assert_eq!(out[0].assets[0].name, "Mindustry.jar");

        // file-list 条目是目录，没有资产也要留着
        let dir = builtin_items().remove(1);
        let kept = client_only(&dir, vec![mk("build-160.5-stable", &[])]);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn file_station_splits_folders_and_files() {
        // 目录页：文件夹
        let dir_page = r#"
          <a href="/category/Mindustry/v8/build-160.5-stable" class="term-file term-folder-link">
            <div class="term-file-name"><span class="term-folder-icon"></span><span>build-160.5-stable</span></div>
          </a>
          <a href="/category/Mindustry/v8/build-159-stable" class="term-file term-folder-link">
            <div class="term-file-name"><span class="term-folder-icon"></span><span>build-159-stable</span></div>
          </a>"#;
        let folders = parse_file_list(dir_page, "https://file.mdtbbs.cn/category/Mindustry/v8");
        assert_eq!(folders.len(), 2);
        assert!(folders.iter().all(|v| v.folder));
        assert_eq!(folders[0].tag, "build-160.5-stable");
        assert_eq!(
            folders[0].page_url,
            "https://file.mdtbbs.cn/category/Mindustry/v8/build-160.5-stable"
        );

        // 文件页：文件（名称在 div 里，日期在最后的 span —— 老代码会把日期当文件名）
        let file_page = r#"
          <a href="/Mindustry/v8/build-160.5-stable/Mindustry.jar" class="term-file">
            <div class="term-file-name">Mindustry.jar</div>
            <div class="term-file-meta">
              <span class="size">84.8 MB</span>
              <span class="term-sep">|</span>
              <span class="count">24 次下载</span>
              <span class="term-sep">|</span>
              <span>最近: 2026-10-06 06:11</span>
            </div>
          </a>
          <a href="/Mindustry/v8/build-160.5-stable/server-release.jar" class="term-file">
            <div class="term-file-name">server-release.jar</div>
            <div class="term-file-meta"><span class="size">80.0 MB</span></div>
          </a>"#;
        let entries = parse_term_entries(file_page, "https://file.mdtbbs.cn/Mindustry/v8/build-160.5-stable");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Mindustry.jar", "不能把「最近: …」当文件名");
        assert!(!entries[0].folder);
        assert_eq!(entries[0].size, (84.8 * 1024.0 * 1024.0) as u64);
        assert_eq!(
            entries[0].url,
            "https://file.mdtbbs.cn/Mindustry/v8/build-160.5-stable/Mindustry.jar"
        );
    }

    #[test]
    fn file_station_download_url_is_d_prefixed() {
        // 目录页给的是详情页（text/html），真直链是 /d/ + 同一路径
        assert_eq!(
            download_url("https://file.mdtbbs.cn/Mindustry/v8/build-160.5-stable/Mindustry.jar"),
            "https://file.mdtbbs.cn/d/Mindustry/v8/build-160.5-stable/Mindustry.jar"
        );
        assert_eq!(download_url("not a url"), "not a url");
    }

    #[test]
    fn file_station_parses_sizes() {
        assert_eq!(parse_size("84.8 MB"), (84.8 * 1024.0 * 1024.0) as u64);
        assert_eq!(parse_size("1.5 GB"), (1.5 * 1024.0 * 1024.0 * 1024.0) as u64);
        assert_eq!(parse_size("900 KB"), 900 * 1024);
        assert_eq!(parse_size("1234"), 1234);
        assert_eq!(parse_size(""), 0);
        assert_eq!(parse_size("怪东西"), 0);
    }

    #[test]
    fn strip_tags_keeps_text_and_collapses_space() {
        assert_eq!(strip_tags("<span class=\"x\"></span><span>abc</span>"), "abc");
        assert_eq!(strip_tags("  a   b  "), "a b");
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
        let body = r#"{"schema":2,"items":[
            {"id":"x","name":"测试条目","kind":"direct-url","url":"https://a/b.jar","group":"分组","tags":["甲","乙"],"size":123,
             "logo":"https://a/l.png","slogan":"服务器下发的标语",
             "versions":[{"tag":"v1","title":"快照","date":"2026-01-01","pageUrl":"https://a/p","dropped":2,
                          "assets":[{"name":"client.jar","url":"https://a/c.jar","size":9}]}]},
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
        // 客户端打标与服务器同规则：类型 → 分组 → 自带
        assert_eq!(
            idx.items[0].tags,
            vec![
                "直链".to_string(),
                "分组".to_string(),
                "甲".to_string(),
                "乙".to_string()
            ]
        );
        assert_eq!(idx.items[0].size, 123);
        assert_eq!(idx.items[0].logo, "https://a/l.png");
        assert_eq!(idx.items[0].slogan, "服务器下发的标语");
        // 服务器下发的版本快照要原样到达客户端（客户端据此不再直连 GitHub）
        assert_eq!(idx.items[0].versions.len(), 1);
        assert_eq!(idx.items[0].versions[0].tag, "v1");
        assert_eq!(idx.items[0].versions[0].dropped, 2);
        assert_eq!(idx.items[0].versions[0].assets[0].name, "client.jar");
        assert!(preloaded(&idx.items[0]).is_some());
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

    #[test]
    fn extracts_github_error_message() {
        // GitHub 限流时返回的是对象而不是数组，必须把 message 抠出来给人看
        let body = r#"{"message":"API rate limit exceeded for 1.2.3.4.","documentation_url":"https://x"}"#;
        assert_eq!(message_of(body), "API rate limit exceeded for 1.2.3.4.");
        assert_eq!(message_of("  not json at all  "), "not json at all");
        assert_eq!(message_of(""), "");
    }
}
