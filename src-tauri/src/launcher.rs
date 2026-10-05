use crate::instances;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

/// 进程级运行表（单 app 生命周期，全局即可，避开 Tauri State 跨线程限制）
fn running() -> &'static Mutex<HashMap<String, Arc<Mutex<Child>>>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Arc<Mutex<Child>>>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogPayload {
    pub id: String,
    pub line: String,
    pub stream: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExitPayload {
    pub id: String,
    pub code: i32,
}

pub fn running_ids() -> Vec<String> {
    running().lock().unwrap().keys().cloned().collect()
}

#[tauri::command]
pub async fn launch_instance(app: AppHandle, id: String) -> Result<(), String> {
    let info = instances::read_instance(&id)?; // 内含 validate_id

    let jar = std::path::PathBuf::from(&info.jar_path);
    if !jar.is_file() {
        return Err(format!("jar 不存在: {}", info.jar_path));
    }
    let cwd = jar.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| ".".into());

    let data_abs = instances::data_dir_abs(&info);
    std::fs::create_dir_all(&data_abs).map_err(|e| e.to_string())?;

    let mut cmd = crate::cmdutil::no_console(&info.java_path);
    cmd.arg(format!("-Xmx{}M", info.memory_mb));
    for a in &info.jvm_args {
        if !a.trim().is_empty() {
            cmd.arg(a);
        }
    }
    cmd.arg("-jar").arg(&jar);
    for a in &info.game_args {
        if !a.trim().is_empty() {
            cmd.arg(a);
        }
    }
    cmd.current_dir(&cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // 新建进程组：stop 时可整组终止（游戏可能再派生子进程）
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    // 数据隔离：Arc/游戏经 AppData(Windows) 或 XDG_DATA_HOME(Linux) 决定数据目录
    if info.isolate {
        if cfg!(windows) {
            cmd.env("AppData", &data_abs);
        } else {
            cmd.env("XDG_DATA_HOME", &data_abs);
        }
    }

    // 检查-启动-登记为一个临界区，防止双击重复拉起
    let mut table = running().lock().unwrap();
    if table.contains_key(&id) {
        return Err("该实例已在运行".into());
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动失败（javaPath={}）: {e}", info.java_path))?;

    let stdout = child.stdout.take().ok_or("无法读取 stdout")?;
    let stderr = child.stderr.take().ok_or("无法读取 stderr")?;

    let app_out = app.clone();
    let id_out = id.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = app_out.emit(
                "launch-log",
                LogPayload { id: id_out.clone(), line, stream: "out".into() },
            );
        }
    });
    let app_err = app.clone();
    let id_err = id.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = app_err.emit(
                "launch-log",
                LogPayload { id: id_err.clone(), line, stream: "err".into() },
            );
        }
    });

    let handle = Arc::new(Mutex::new(child));
    table.insert(id.clone(), handle.clone());
    drop(table);

    // waiter：自然退出 → 表中仍是同一 Arc 则移除并发 launch-exit；已被 stop 移除则静默退出
    let app_w = app.clone();
    let id_w = id.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let status = {
            let mut guard = handle.lock().unwrap();
            match guard.try_wait() {
                Ok(Some(st)) => Some(st),
                Ok(None) => None,
                Err(_) => None,
            }
        };
        if let Some(st) = status {
            let mut table = running().lock().unwrap();
            let still_ours = table
                .get(&id_w)
                .map(|a| Arc::ptr_eq(a, &handle))
                .unwrap_or(false);
            if still_ours {
                table.remove(&id_w);
                drop(table);
                let _ = app_w.emit(
                    "launch-exit",
                    ExitPayload { id: id_w.clone(), code: st.code().unwrap_or(-1) },
                );
            }
            break;
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn stop_instance(id: String) -> Result<(), String> {
    let id = instances::validate_id(&id)?;
    let Some(handle) = running().lock().unwrap().remove(&id) else {
        return Err("该实例未在运行".into());
    };
    let pid = {
        let guard = handle.lock().unwrap();
        guard.id()
    };
    #[cfg(windows)]
    {
        let _ = crate::cmdutil::no_console("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        // 进程组终止（spawn 时 process_group(0)，pgid == 子进程 pid）
        let _ = crate::cmdutil::no_console("kill")
            .args(["-KILL", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let mut guard = handle.lock().unwrap();
        let _ = guard.kill();
        let _ = guard.wait();
    }
    #[cfg(windows)]
    let _ = pid;
    Ok(())
}
