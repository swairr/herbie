#![allow(linker_messages)]
// 过渡期(切片1)保留若干公共仓库/持有者 API(open_file、close、default_db_path、
// settings 键常量与 default*/get_with_default 等)供后续切片(todos/labels/export)接线,
// 当前尚未被调用 —— 与其逐个 #[allow(dead_code)],在 crate 级统一豁免。
#![allow(dead_code)]

mod db;
mod export;
mod hook;
mod journal;
mod labels;
mod labels_store;
mod migrations;
mod segment;
mod settings;
mod spike_power;
mod time;
mod todos;
mod tracker;
#[cfg(windows)]
mod win;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use spike_power::start_power_watcher;

use segment::{Segment, SegmentPatch};
use journal::{JournalEntry, JournalInput, JournalPatch};
use export::ExportDayData;
use todos::{LabelCount, Todo, TodoFilter, TodoInput, TodoPatch};
use tracker::{GatedNotifier, GlobalConn, OffWorkState, ProdDeps, Tracker};
#[cfg(windows)]
use win::foreground::ForegroundHook;

// 进程级共享 tracker(对齐 TS `setTrackerInstance`/`getTracker` 的持有人)。命令经此取实例,
// 不必在注册处逐个透传。3c-B 接真实前台钩子/idle/电源后,Tracker 仍由同一 Arc 持有。
static TRACKER: OnceLock<Arc<Tracker>> = OnceLock::new();

// 3c-B 生产接线的生命周期 guard:`start_tracking_system` 幂等;`stop_tracking_system`
// 供切片6 外壳退出时调用,当前先 join 轮询线程 + 拆前台钩子,电源线程为守护常驻
// (优雅停完善留切片7,见 `stop_tracking_system` 注释)。
#[cfg(windows)]
static TRACKING_STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static POLL_STOP: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static POLL_THREAD: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);
#[cfg(windows)]
static FOREGROUND_HOOK: Mutex<Option<GatedNotifier<ForegroundHook>>> = Mutex::new(None);

/// 生产接线(3c-B,仅 Windows,本机运行即真实前台钩子/空闲/电源):
/// 1) 前台钩子:`gate_notifier` 把 off-work 期间的每条事件先提升为返岗信号,
///    `start_tracking` 再写段开关;handle 存起来供 stop 拆除。
/// 2) idle 轮询线程:对应 TS `setInterval(poll, 20s)`(先睡再 poll,首次在启动 20s 后)。
/// 3) 电源/锁屏:`start_power_watcher` 映射为 `PowerEvent` 驱动 `on_power`。
/// 锁顺序全程「先 tracker 后 db」:钩子/轮询回调内先锁 tracker state,再经 ConnAccess
/// 锁 DB;此处不引入与 tracker 反向的锁序。
#[cfg(windows)]
pub fn start_tracking_system(_app: &tauri::AppHandle) {
    if TRACKING_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let t = TRACKER.get().expect("tracker");
    // 对齐 TS `start(deps)`:置 started 标志(当前仅供状态确认,轮询/电源不依赖)。
    t.start(&mut ProdDeps);

    let hook = ForegroundHook::new();
    let gated = t.gate_notifier(hook);
    let hook_owned = segment::start_tracking(gated, Arc::new(GlobalConn));
    *FOREGROUND_HOOK.lock().expect("hook mutex poisoned") = Some(hook_owned);

    let tracker_for_poll = t.clone();
    let poll_thread = thread::Builder::new()
        .name("herbie-idle-poll".to_string())
        .spawn(move || {
            let mut deps = ProdDeps;
            loop {
                thread::sleep(Duration::from_secs(20));
                if POLL_STOP.load(Ordering::SeqCst) {
                    break;
                }
                tracker_for_poll.poll(&GlobalConn, &mut deps);
            }
        })
        .expect("failed to spawn idle poll thread");
    *POLL_THREAD.lock().expect("poll mutex poisoned") = Some(poll_thread);

    let tracker_for_power = t.clone();
    win::power::start_power_watcher(Box::new(move |e| {
        tracker_for_power.on_power(&GlobalConn, &mut ProdDeps, e);
    }));
}

/// 停止追踪系统:先停轮询(poll 可能持有 tracker state / DB 写路径),再关开放段并停
/// 前台钩子。电源线程为守护常驻,进程退出即回收;若需优雅停止,切片7 再补停句柄。
/// 切片6 外壳在退出前调用。
#[cfg(windows)]
pub fn stop_tracking_system() {
    if !TRACKING_STARTED.load(Ordering::SeqCst) {
        return;
    }
    POLL_STOP.store(true, Ordering::SeqCst);
    if let Some(join) = POLL_THREAD.lock().expect("poll mutex poisoned").take() {
        let _ = join.join();
    }
    if let Some(hook) = FOREGROUND_HOOK.lock().expect("hook mutex poisoned").take() {
        segment::stop_tracking(hook, &GlobalConn, &todos::now_iso());
    }
}

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
fn journal_list(day: String) -> Result<Vec<JournalEntry>, String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    journal::list_journals(conn, &day).map_err(|e| e.to_string())
}

#[tauri::command]
fn journal_create(input: JournalInput) -> Result<JournalEntry, String> {
    let mut g = db::get();
    let conn = g.as_mut().ok_or("DB not initialized")?;
    journal::create_journal(conn, &input)
}

#[tauri::command]
fn journal_update(id: String, patch: JournalPatch) -> Result<JournalEntry, String> {
    let mut g = db::get();
    let conn = g.as_mut().ok_or("DB not initialized")?;
    journal::update_journal(conn, &id, &patch)
}

#[tauri::command]
fn journal_soft_delete(id: String) -> Result<(), String> {
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    journal::soft_delete_journal(conn, &id).map_err(|e| e.to_string())
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

// 切片5 export:Rust 只做「拉当日数据」「写文件」「默认目录」;markdown 生成留在
// renderer(TS shared)。`export_write_file` 的 filename 由 renderer 以 `todos.md` /
// `time/<day>.md` / `journal/<day>.md` 传入(day 已先经 export_pull_day 的 assert_day)。
#[tauri::command]
fn export_write_file(dir: String, filename: String, content: String) -> Result<String, String> {
    export::write_file(&dir, &filename, &content)
}

#[tauri::command]
fn export_default_dir() -> Result<String, String> {
    Ok(export::default_export_dir())
}

#[tauri::command]
fn export_pull_day(day: String) -> Result<ExportDayData, String> {
    export::assert_day(&day)?;
    let g = db::get();
    let conn = g.as_ref().ok_or("DB not initialized")?;
    Ok(ExportDayData {
        segments: segment::list_segments_by_day(conn, &day, chrono::Utc::now()),
        journal: journal::list_journals(conn, &day).map_err(|e| e.to_string())?,
    })
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

    // 创建进程级 tracker(3b 仅 off-work 命令路径在用;3c-B 由 start_tracking_system 接线).
    TRACKER.set(Arc::new(Tracker::new())).ok();

    tauri::Builder::default()
        .setup(|app| {
            // 3c-B 生产接线:TRACKER 已在上面提前 set,setup 内即可启动前台钩子/
            // idle 轮询/电源事件(本机 Windows 运行即真实生效)。
            #[cfg(windows)]
            start_tracking_system(&app.handle());
            Ok(())
        })
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
            journal_list,
            journal_create,
            journal_update,
            journal_soft_delete,
            segments_list,
            segments_update,
            export_write_file,
            export_default_dir,
            export_pull_day,
            tracker_get_off_work,
            tracker_set_off_work
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}