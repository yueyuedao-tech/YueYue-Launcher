use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 实例 id 即目录名：拒绝空、.、.. 及路径分隔符，防止越出 instances 根
pub fn validate_id(id: &str) -> Result<String, String> {
    if id.is_empty() {
        return Err("实例 id 不能为空".into());
    }
    if id == "." || id == ".." {
        return Err("非法实例 id".into());
    }
    if id.contains('/') || id.contains('\\') {
        return Err("实例 id 不能包含路径分隔符".into());
    }
    if id.chars().any(|c| matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err("实例 id 包含非法字符".into());
    }
    Ok(id.to_string())
}

/// java 可执行校验：带分隔符按文件存在判断，否则尝试 -version 探测
fn validate_java(java: &str) -> Result<(), String> {
    if java.contains('\\') || java.contains('/') {
        if !Path::new(java).is_file() {
            return Err(format!("java 不存在: {java}"));
        }
    } else {
        let probe = Command::new(java).arg("-version").output();
        if probe.is_err() {
            return Err(format!("java 不可用: {java}"));
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub jar_path: String,
    pub java_path: String,
    pub data_dir: String,
    // launch.config.json
    pub isolate: bool,
    pub memory_mb: u32,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    #[serde(default)]
    pub running: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: String,
    name: String,
    created_at: String,
    jar_path: String,
    java_path: String,
    data_dir: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct LaunchConfig {
    isolate: bool,
    memory_mb: u32,
    jvm_args: Vec<String>,
    game_args: Vec<String>,
}

fn default_config() -> LaunchConfig {
    LaunchConfig {
        isolate: true,
        memory_mb: 4096,
        jvm_args: vec![],
        game_args: vec![],
    }
}

pub fn instances_root() -> PathBuf {
    if cfg!(windows) {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(appdata).join("StarlightLauncher").join("instances")
    } else {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".local").join("share")
            });
        base.join("starlight-launcher").join("instances")
    }
}

fn sanitize_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("实例名不能为空".into());
    }
    if name.len() > 40 {
        return Err("实例名过长（≤40 字符）".into());
    }
    if name.chars().any(|c| matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err("实例名包含非法字符 \\ / : * ? \" < > |".into());
    }
    Ok(name.to_string())
}

fn load_instance_dir(dir: &Path) -> Option<InstanceInfo> {
    let manifest_raw = fs::read_to_string(dir.join("instance.json")).ok()?;
    let manifest: Manifest = serde_json::from_str(&manifest_raw).ok()?;
    let config_raw = fs::read_to_string(dir.join("launch.config.json")).ok()
        .unwrap_or_else(|| "{}".to_string());
    let config: LaunchConfig = serde_json::from_str(&config_raw).unwrap_or_else(|_| default_config());
    Some(InstanceInfo {
        id: manifest.id,
        name: manifest.name,
        created_at: manifest.created_at,
        jar_path: manifest.jar_path,
        java_path: manifest.java_path,
        data_dir: manifest.data_dir,
        isolate: config.isolate,
        memory_mb: config.memory_mb,
        jvm_args: config.jvm_args,
        game_args: config.game_args,
        running: false,
    })
}

pub fn read_instance(id: &str) -> Result<InstanceInfo, String> {
    let id = validate_id(id)?;
    let dir = instances_root().join(&id);
    load_instance_dir(&dir).ok_or_else(|| format!("实例不存在: {id}"))
}

/// Windows canonicalize 会带 \\?\ 前缀，Java 与展示都不需要，统一剥掉
fn abs_jar_path(p: &Path) -> String {
    let s = p
        .canonicalize()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned());
    s.strip_prefix(r"\\?\").map(|x| x.to_string()).unwrap_or(s)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceList {
    pub items: Vec<InstanceInfo>,
    pub skipped: usize,
}

#[tauri::command]
pub async fn list_instances() -> InstanceList {
    let root = instances_root();
    let mut out = Vec::new();
    let mut skipped = 0usize;
    if let Ok(rd) = fs::read_dir(&root) {
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let has_manifest = path.join("instance.json").exists();
            let has_config = path.join("launch.config.json").exists();
            if !has_manifest && !has_config {
                continue; // 无关目录（如临时目录），不计数
            }
            match load_instance_dir(&path) {
                Some(info) => out.push(info),
                None => skipped += 1, // 损坏/缺失的实例条目
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let running = crate::launcher::running_ids();
    for i in &mut out {
        i.running = running.contains(&i.id);
    }
    InstanceList { items: out, skipped }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_instance(
    name: String,
    jar_path: String,
    java_path: String,
    jvm_args: Vec<String>,
    memory_mb: u32,
    isolate: bool,
) -> Result<InstanceInfo, String> {
    let name = sanitize_name(&name)?;
    let jar = PathBuf::from(&jar_path);
    if !jar.is_file() {
        return Err(format!("jar 不存在: {jar_path}"));
    }
    validate_java(&java_path)?;
    if !(512..=65536).contains(&memory_mb) {
        return Err("内存需在 512-65536 MB 之间".into());
    }
    let root = instances_root();
    let dir = root.join(&name);
    if dir.exists() {
        return Err(format!("实例已存在: {name}"));
    }
    fs::create_dir_all(dir.join("data")).map_err(|e| e.to_string())?;

    // 任一步失败则回滚整个目录，避免留下半成品条目
    let created = chrono_lite_now();
    let id = name.clone();
    let write_result: Result<(), String> = (|| {
        let manifest = Manifest {
            id: id.clone(),
            name: name.clone(),
            created_at: created,
            jar_path: abs_jar_path(&jar),
            java_path: java_path.clone(),
            data_dir: "data".into(),
        };
        let config = LaunchConfig {
            isolate,
            memory_mb,
            jvm_args,
            game_args: vec![],
        };
        fs::write(
            dir.join("instance.json"),
            serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            dir.join("launch.config.json"),
            serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }

    read_instance(&id)
}

/// 全字段更新（前端编辑表单始终提交完整对象，避免 Option 缺键歧义）
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_instance(
    id: String,
    jar_path: String,
    java_path: String,
    jvm_args: Vec<String>,
    game_args: Vec<String>,
    isolate: bool,
    memory_mb: u32,
) -> Result<InstanceInfo, String> {
    let id = validate_id(&id)?;
    let dir = instances_root().join(&id);
    if !dir.is_dir() {
        return Err(format!("实例不存在: {id}"));
    }
    let mut info = load_instance_dir(&dir).ok_or("实例文件损坏")?;

    let p = PathBuf::from(&jar_path);
    if !p.is_file() {
        return Err(format!("jar 不存在: {jar_path}"));
    }
    info.jar_path = abs_jar_path(&p);

    validate_java(&java_path)?;
    info.java_path = java_path;
    if memory_mb < 512 || memory_mb > 65536 {
        return Err("内存需在 512-65536 MB 之间".into());
    }
    info.memory_mb = memory_mb;
    info.jvm_args = jvm_args;
    info.game_args = game_args;
    info.isolate = isolate;

    let manifest = Manifest {
        id: info.id.clone(),
        name: info.name.clone(),
        created_at: info.created_at.clone(),
        jar_path: info.jar_path.clone(),
        java_path: info.java_path.clone(),
        data_dir: info.data_dir.clone(),
    };
    let config = LaunchConfig {
        isolate: info.isolate,
        memory_mb: info.memory_mb,
        jvm_args: info.jvm_args.clone(),
        game_args: info.game_args.clone(),
    };
    fs::write(dir.join("instance.json"), serde_json::to_string_pretty(&manifest).unwrap())
        .map_err(|e| e.to_string())?;
    fs::write(dir.join("launch.config.json"), serde_json::to_string_pretty(&config).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(info)
}

#[tauri::command]
pub async fn delete_instance(id: String) -> Result<(), String> {
    let id = validate_id(&id)?;
    let dir = instances_root().join(&id);
    if !dir.is_dir() {
        return Err(format!("实例不存在: {id}"));
    }
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

pub fn data_dir_abs(info: &InstanceInfo) -> PathBuf {
    instances_root().join(&info.id).join(&info.data_dir)
}

/// 自动扫描本机 Mindustry jar：各盘 steam 目录 + Desktop 4 层内 Mindustry.jar
#[tauri::command]
pub async fn scan_jars() -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut push = |p: PathBuf| {
        if p.is_file() {
            let s = p.to_string_lossy().into_owned();
            if !found.contains(&s) {
                found.push(s);
            }
        }
    };

    for drive in ['C', 'D', 'E', 'F', 'G'] {
        for rel in [
            format!("{drive}:\\steamapps\\common\\Mindustry\\Mindustry.jar"),
            format!("{drive}:\\SteamLibrary\\steamapps\\common\\Mindustry\\Mindustry.jar"),
            format!("{drive}:\\Steam\\steamapps\\common\\Mindustry\\Mindustry.jar"),
        ] {
            push(PathBuf::from(rel));
        }
    }

    if let Some(desktop) = dirs_desktop() {
        scan_dir_depth(&desktop, 0, 4, &mut push);
    }
    found
}

fn dirs_desktop() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(PathBuf::from(home).join("Desktop"))
}

fn scan_dir_depth(dir: &Path, depth: usize, max: usize, out: &mut impl FnMut(PathBuf)) {
    if depth > max {
        return;
    }
    // 跳过重量级目录，避免大型工程树把扫描拖到秒级
    const SKIP: [&str; 6] = ["node_modules", ".git", ".gradle", "target", "dist", ".cache"];
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        // file_type 来自 readdir 结果，不额外 stat，大幅降低 IO
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_file() {
            if entry.file_name() == "Mindustry.jar" {
                out(entry.path());
            }
        } else if ft.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP.contains(&name.as_ref()) {
                continue;
            }
            scan_dir_depth(&entry.path(), depth + 1, max, out);
        }
    }
}

/// 扫描本机可用 Java：PATH 中的 java + Program Files\Java 下各 JDK/JRE
#[tauri::command]
pub async fn scan_javas() -> Vec<String> {
    let mut out: Vec<String> = vec!["java".into()];
    if let Ok(jh) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(&jh).join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        if p.is_file() {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    #[cfg(windows)]
    if let Ok(rd) = fs::read_dir(r"C:\Program Files\Java") {
        for e in rd.flatten() {
            let p = e.path().join("bin").join("java.exe");
            if p.is_file() {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
    out.dedup();
    out
}

fn chrono_lite_now() -> String {
    // 本地时间（修复 UTC 显示偏差）
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_illegal_and_empty() {
        assert!(sanitize_name("").is_err());
        assert!(sanitize_name("a/b").is_err());
        assert!(sanitize_name("ok-name_01").is_ok());
        assert_eq!(sanitize_name("  trim  ").unwrap(), "trim");
    }

    #[test]
    fn manifest_json_roundtrip_camel_case() {
        let m = Manifest {
            id: "inst-1".into(),
            name: "inst-1".into(),
            created_at: "2026-10-04 12:00:00".into(),
            jar_path: "C:/games/Mindustry.jar".into(),
            java_path: "java".into(),
            data_dir: "data".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains("jarPath") && s.contains("createdAt") && s.contains("javaPath"));
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.jar_path, m.jar_path);
        assert_eq!(back.data_dir, "data");
    }

    #[test]
    fn launch_config_defaults_isolate_on() {
        let c = default_config();
        assert!(c.isolate);
        assert_eq!(c.memory_mb, 4096);
    }
}
