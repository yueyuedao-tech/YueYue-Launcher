use std::collections::HashMap;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct RoomProcesses(pub Mutex<HashMap<String, Child>>);

impl Default for RoomProcesses {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

#[tauri::command]
pub fn connect_room(
    app: AppHandle,
    room_id: String,
    network_name: String,
    network_secret: String,
    upid: String,
) -> Result<(), String> {
    if !room_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || room_id.is_empty()
        || !network_name.starts_with("yyl-")
        || network_name.len() > 64
        || network_secret.len() < 24
        || network_secret.len() > 128
        || upid.len() < 248
        || upid.len() > 256
        || !upid.bytes().all(|b| b == b'0' || b == b'1')
    {
        return Err("Invalid room connection parameters".into());
    }

    let exe = if cfg!(windows) { "easytier-core.exe" } else { "easytier-core" };
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?;
    let candidates = [
        resource_dir.join("resources").join("easytier").join(exe),
        resource_dir.join("easytier").join(exe),
    ];
    let path = candidates.into_iter().find(|candidate| candidate.is_file()).ok_or_else(|| "EasyTier core is missing from this installation".to_string())?;
    let config_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("rooms").join(&room_id);
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let rpc_portal = listener.local_addr().map_err(|e| e.to_string())?.to_string();
    drop(listener);
    let state = app.state::<RoomProcesses>();
    let mut processes = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut old) = processes.remove(&room_id) {
        let _ = old.kill();
        let _ = old.wait();
    }
    let child = Command::new(path)
        .env("ET_NETWORK_NAME", network_name)
        .env("ET_NETWORK_SECRET", network_secret)
        .env("ET_HOSTNAME", format!("YYL-{}", &upid[..16]))
        .arg("--no-listener")
        .arg("--dhcp")
        .args(["--rpc-portal", &rpc_portal])
        .arg("--config-dir")
        .arg(config_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("EasyTier failed to start: {e}"))?;
    processes.insert(room_id, child);
    Ok(())
}

#[tauri::command]
pub fn disconnect_room(app: AppHandle, room_id: String) -> Result<(), String> {
    let state = app.state::<RoomProcesses>();
    let mut processes = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = processes.remove(&room_id) {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

pub fn disconnect_all(app: &AppHandle) {
    let state = app.state::<RoomProcesses>();
    if let Ok(mut processes) = state.0.lock() {
        for (_, mut child) in processes.drain() {
            let _ = child.kill();
            let _ = child.wait();
        }
    };
}

#[tauri::command]
pub fn disconnect_all_rooms(app: AppHandle) {
    disconnect_all(&app);
}
