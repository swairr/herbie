#![allow(linker_messages)]

mod spike_power;

use spike_power::start_power_watcher;

#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

#[tauri::command]
fn power_subscribe(app: tauri::AppHandle) -> Result<(), String> {
    start_power_watcher(app).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping, power_subscribe])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}