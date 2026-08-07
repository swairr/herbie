#![allow(linker_messages)]
// 过渡期(切片1)保留若干公共仓库/持有者 API(open_file、close、default_db_path、
// settings 键常量与 default*/get_with_default 等)供后续切片(todos/labels/export)接线,
// 当前尚未被调用 —— 与其逐个 #[allow(dead_code)],在 crate 级统一豁免。
#![allow(dead_code)]

mod db;
mod hook;
mod labels;
mod labels_store;
mod migrations;
mod segment;
mod settings;
mod spike_power;
mod time;
mod todos;
mod tracker;

use std::sync::{Arc, OnceLock};

use spike_power::start_power_watcher;

use segment::{Segment, SegmentPatch};
use todos::{LabelCount, Todo, TodoFilter, TodoInput, TodoPatch};
use tracker::{GlobalConn, OffWorkState, ProdDeps, Tracker};

// 进程级共享 tracker(对齐 TS `setTrackerInstance`/`getTracker` 的持有人)。命令经此取实例,
// 不必在注册处逐个透传。3c 接真实 power 路由后,Tracker 仍由同一 Arc 持有。
static TRACKER: OnceLock<Arc<Tracker>> = OnceLock::new();

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

#[tauri::command]
fn segments_list(day: String) -> Result<Vec<Segment>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    let now = chrono::Utc::now();
    Ok(segment::list_segments_by_day(conn, &day, now))
}

#[tauri::command]
fn segments_update(id: String, patch: SegmentPatch) -> Result<Option<Segment>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    Ok(segment::update_segment(conn, &id, &patch))
}

#[tauri::command]
fn tracker_get_off_work() -> Result<OffWorkState, String> {
    Ok(OffWorkState {
        off_work: TRACKER.get().ok_or("tracker not ready")?.get_off_work(),
    })
}

#[tauri::command]
fn tracker_set_off_work(on: bool) -> Result<OffWorkState, String> {
    let t = TRACKER.get().ok_or("tracker not ready")?;
    let conn = GlobalConn;
    let mut deps = ProdDeps;
    t.set_off_work(&conn, &mut deps, on);
    Ok(OffWorkState {
        off_work: t.get_off_work(),
    })
}

pub fn run() {
    // 启动占位:先开内存库,使 `pnpm tauri dev` 在数据路径仍未接入时也不崩。
    // 切片6/7 应改为 `db::open_file(db::default_db_path()).expect(...)`(沿用旧数据)。
    db::open_in_memory();

    // 创建进程级 tracker(3b 仅 off-work 命令路径在用;3c 接 power/idle 路由与 start()).
    TRACKER.set(Arc::new(Tracker::new())).ok();

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
            todos_labels,
            segments_list,
            segments_update,
            tracker_get_off_work,
            tracker_set_off_work
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}