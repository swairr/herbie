#![allow(linker_messages)]
// 过渡期(切片1)保留若干公共仓库/持有者 API(open_file、close、default_db_path、
// settings 键常量与 default*/get_with_default 等)供后续切片(todos/labels/export)接线,
// 当前尚未被调用 —— 与其逐个 #[allow(dead_code)],在 crate 级统一豁免。
#![allow(dead_code)]

mod db;
mod labels;
mod labels_store;
mod migrations;
mod settings;
mod spike_power;
mod todos;

use spike_power::start_power_watcher;
use todos::{LabelCount, Todo, TodoFilter, TodoInput, TodoPatch};

#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

#[tauri::command]
fn power_subscribe(app: tauri::AppHandle) -> Result<(), String> {
    start_power_watcher(app).map_err(|e| e.to_string())
}

#[tauri::command]
fn settings_get(key: String) -> Result<Option<String>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    Ok(settings::get(conn, &key))
}

#[tauri::command]
fn settings_set(key: String, value: String) -> Result<(), String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    settings::set(conn, &key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
fn settings_get_all() -> Result<Vec<(String, String)>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    Ok(settings::get_all(conn))
}

#[tauri::command]
fn todos_list(filter: Option<TodoFilter>) -> Result<Vec<Todo>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    todos::list_todos(conn, filter.as_ref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn todos_create(input: TodoInput) -> Result<Todo, String> {
    let mut g = db::get();
    let conn = g.as_mut().ok_or("DB not initialized")?;
    todos::create_todo(conn, &input).map_err(|e| e.to_string())
}

#[tauri::command]
fn todos_update(id: String, patch: TodoPatch) -> Result<Todo, String> {
    let mut g = db::get();
    let conn = g.as_mut().ok_or("DB not initialized")?;
    todos::update_todo(conn, &id, &patch)
}

#[tauri::command]
fn todos_toggle(id: String, done: bool) -> Result<Todo, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    todos::toggle_todo(conn, &id, done).map_err(|e| e.to_string())
}

#[tauri::command]
fn todos_soft_delete(id: String) -> Result<(), String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    todos::soft_delete_todo(conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn todos_labels() -> Result<Vec<LabelCount>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    todos::list_todo_labels(conn).map_err(|e| e.to_string())
}

pub fn run() {
    // 启动占位:先开内存库,使 `pnpm tauri dev` 在数据路径仍未接入时也不崩。
    // 切片6/7 应改为 `db::open_file(db::default_db_path()).expect(...)`(沿用旧数据)。
    db::open_in_memory();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ping,
            power_subscribe,
            settings_get,
            settings_set,
            settings_get_all,
            todos_list,
            todos_create,
            todos_update,
            todos_toggle,
            todos_soft_delete,
            todos_labels
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}