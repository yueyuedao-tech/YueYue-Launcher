use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const APP_DIR_NAME: &str = "YueYue Launcher";
#[cfg(windows)]
const APP_DIR_LEGACY: &str = "StarlightLauncher";
#[cfg(not(windows))]
const APP_DIR_NAME: &str = "yueyue-launcher";
#[cfg(not(windows))]
const APP_DIR_LEGACY: &str = "starlight-launcher";

fn raw_root(name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(appdata).join(name)
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".local").join("share")
            });
        base.join(name)
    }
}

/// 启动器数据根目录（实例、下载、缓存、配置都在它下面）。
/// Windows 是 `%APPDATA%\YueYue Launcher`，Linux 是 `$XDG_DATA_HOME/yueyue-launcher`。
///
/// 老版本用的是 StarlightLauncher / starlight-launcher：第一次访问时整目录搬过来，
/// 这样改名之后用户的实例与下载不会丢。
pub fn app_root() -> PathBuf {
    let new = raw_root(APP_DIR_NAME);
    migrate_legacy(&raw_root(APP_DIR_LEGACY), &new);
    new
}

/// 老目录在、新目录不在 → 搬过去。同卷直接 rename，失败（跨卷/占用）退化为逐项复制。
fn migrate_legacy(old: &Path, new: &Path) {
    if new.exists() || !old.exists() {
        return;
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::rename(old, new).is_ok() {
        return;
    }
    if copy_tree(old, new).is_ok() {
        let _ = std::fs::remove_dir_all(old);
    }
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// 共享数据目录：实例关掉「数据隔离」时，游戏共用这一份数据（而不是系统 AppData），
/// 这样多开之间共享存档，又不会污染系统里的 Mindustry。
pub fn shared_data_dir() -> PathBuf {
    app_root().join("shared").join("data")
}

/// 构造子进程命令：Windows 下加 CREATE_NO_WINDOW，避免每次调用闪出 CMD 黑窗
/// （curl / java / taskkill / tar 等全部走这里）
pub fn no_console(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// 读系统代理（设置里的「下载代理 = 系统代理」用）。
/// 顺序：环境变量 → Windows 注册表 WinINET 设置。
/// 用 `reg query` 而不是引入 winreg crate：本工程体积优先，且只要读两个值。
pub fn system_proxy() -> String {
    for key in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    #[cfg(windows)]
    {
        const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let query = |name: &str| -> String {
            let out = no_console("reg")
                .args(["query", KEY, "/v", name])
                .output();
            let Ok(out) = out else { return String::new() };
            parse_reg_value(&String::from_utf8_lossy(&out.stdout), name)
        };
        // ProxyEnable 是 REG_DWORD（0x1），ProxyServer 是 REG_SZ —— 两种类型都要认
        if query("ProxyEnable").eq_ignore_ascii_case("0x1") {
            let server = query("ProxyServer");
            // 形如 `http=1.2.3.4:8080;https=...` 时取 https/http 那段
            if server.contains('=') {
                for part in server.split(';') {
                    let part = part.trim();
                    for key in ["https=", "http="] {
                        if let Some(v) = part.strip_prefix(key) {
                            if !v.is_empty() {
                                return v.to_string();
                            }
                        }
                    }
                }
            }
            if !server.is_empty() {
                return server;
            }
        }
    }
    String::new()
}

/// 解析 `reg query` 的输出里某个值。行形如：
/// `    ProxyServer    REG_SZ    127.0.0.1:7897`
/// `    ProxyEnable    REG_DWORD    0x1`
/// 注意 ProxyEnable 是 REG_DWORD：只认 REG_SZ 的话永远读不到「系统代理已启用」。
fn parse_reg_value(text: &str, name: &str) -> String {
    for line in text.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix(name) else {
            continue;
        };
        // 名字后面必须是空白，避免 ProxyEnableFoo 被当成 ProxyEnable
        if !rest.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        for ty in ["REG_EXPAND_SZ", "REG_SZ", "REG_DWORD"] {
            if let Some(idx) = rest.find(ty) {
                return rest[idx + ty.len()..].trim().to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reg_sz_and_dword_values() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\r\n    ProxyEnable    REG_DWORD    0x1\r\n    ProxyServer    REG_SZ    127.0.0.1:7897\r\n\r\n";
        assert_eq!(parse_reg_value(out, "ProxyEnable"), "0x1");
        assert_eq!(parse_reg_value(out, "ProxyServer"), "127.0.0.1:7897");
        // 名字前缀相近的不能误命中
        assert_eq!(parse_reg_value(out, "Proxy"), "");
        assert_eq!(parse_reg_value(out, "ProxyEnableX"), "");
        assert_eq!(parse_reg_value("", "ProxyServer"), "");
    }

    #[test]
    fn parses_expand_sz_and_spaced_values() {
        let out = "    AutoConfigURL    REG_SZ    http://x/proxy.pac\r\n    ProxyServer    REG_SZ    http=1.2.3.4:80;https=5.6.7.8:443\r\n    Foo    REG_EXPAND_SZ    %USERPROFILE%\\a\r\n";
        assert_eq!(parse_reg_value(out, "AutoConfigURL"), "http://x/proxy.pac");
        assert_eq!(
            parse_reg_value(out, "ProxyServer"),
            "http=1.2.3.4:80;https=5.6.7.8:443"
        );
        assert_eq!(parse_reg_value(out, "Foo"), "%USERPROFILE%\\a");
    }
}
