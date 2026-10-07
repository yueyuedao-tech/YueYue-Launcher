#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod central;
mod cmdutil;
mod downloader;
mod github;
mod instances;
mod launcher;
mod maps;
mod mods;
mod sources;

use std::sync::{Mutex, OnceLock};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// "exit" | "minimize"（最小化到托盘）
struct CloseMode(Mutex<String>);

impl Default for CloseMode {
    fn default() -> Self {
        CloseMode(Mutex::new("exit".into()))
    }
}

#[tauri::command]
fn set_close_behavior(app: AppHandle, mode: String) -> Result<(), String> {
    match mode.as_str() {
        "exit" | "minimize" => {}
        _ => return Err(format!("非法关闭行为: {mode}")),
    }
    *app.state::<CloseMode>().0.lock().unwrap() = mode.clone();
    if mode == "minimize" {
        ensure_tray(&app)?;
    }
    Ok(())
}

/// 懒创建托盘（仅当用户选择最小化行为）
fn ensure_tray(app: &AppHandle) -> Result<(), String> {
    static TRAY: OnceLock<()> = OnceLock::new();
    if TRAY.get().is_some() {
        return Ok(());
    }
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>).map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&show, &quit]).map_err(|e| e.to_string())?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("YYL · YueYue Launcher")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app).map_err(|e| e.to_string())?;
    TRAY.set(()).ok();
    Ok(())
}

fn main() {
    tauri::Builder::default()
        // 单实例：重复启动改为聚焦已有窗口，否则第二个实例会抢同一 WebView2
        // 用户数据目录、在 setup 阶段 panic（panic=abort + windows 子系统 = 静默闪退）
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(CloseMode::default())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            instances::list_instances,
            instances::create_instance,
            instances::update_instance,
            instances::delete_instance,
            instances::list_hidden_instances,
            instances::restore_instance,
            instances::sharing_jar,
            instances::list_saves,
            instances::scan_jars,
            instances::scan_javas,
            launcher::launch_instance,
            launcher::stop_instance,
            sources::list_sources,
            sources::save_sources,
            central::fetch_central_index,
            central::fetch_text,
            central::list_central_versions,
            central::sync_central_versions,
            central::list_folder_files,
            github::fetch_repo_versions,
            mods::search_workshop,
            mods::list_mods,
            mods::mods_dir,
            mods::delete_mod,
            maps::search_maps,
            maps::install_map,
            downloader::start_download,
            downloader::stop_download,
            downloader::list_downloads,
            downloader::clear_download,
            downloader::clear_finished_downloads,
            downloader::get_system_proxy,
            downloader::downloads_dir,
            downloader::existing_download,
            downloader::register_reused_download,
            set_close_behavior,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            if let Some(win) = app.get_webview_window("main") {
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let mode = handle.state::<CloseMode>().0.lock().unwrap().clone();
                        if mode == "minimize" {
                            api.prevent_close();
                            if let Some(w) = handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                            if let Err(e) = ensure_tray(&handle) {
                                eprintln!("tray error: {e}");
                            }
                        }
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
