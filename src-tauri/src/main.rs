#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod instances;
mod launcher;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            instances::list_instances,
            instances::create_instance,
            instances::update_instance,
            instances::delete_instance,
            instances::scan_jars,
            launcher::launch_instance,
            launcher::stop_instance,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
