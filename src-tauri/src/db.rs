//! tauri-free 持有者:全局 rusqlite 单例(对应 TS `src/main/db-access.ts`)。
//!
//! 并发模型说明:
//! - 全局 `static DB: Mutex<Option<Connection>>`。`rusqlite::Connection` 仅实现 `Send`
//!   (内部非线程安全)。包一层 `Mutex` 使其成为 `Sync`,因此可作为 `static`。
//! - 命令线程取连接:`db::get()` 返回 `MutexGuard<'static, Option<Connection>>`。
//!   Tauri 命令体持锁期间借用 `&Connection` 使用。互斥保证同一时刻仅一个命令访问
//!   DB —— 对齐 better-sqlite3 同步单连接语义。仓储层函数取 `&Connection`/`&mut Connection`
//!   参数(不自行锁全局),从而避免命令层锁后调用仓储层再次锁全局导致的重入死锁。
//!
//! 启动时机由 `lib.rs` 的 setup 调 `open_file` 注入(见 `default_db_path`)。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::migrations::run_migrations;

static DB: std::sync::Mutex<Option<Connection>> = std::sync::Mutex::new(None);

/// 取全局连接的互斥守卫。仓储层不自行调用本函数(避免重入);命令层先调用本函数,
/// 再将守卫内的 `&Connection` 传入仓储函数。
pub fn get() -> std::sync::MutexGuard<'static, Option<Connection>> {
    DB.lock().expect("DB mutex poisoned")
}

/// 打开文件库:启用 foreign_keys + WAL + busy_timeout、应用迁移、装入全局。
/// 父目录不存在则创建。失败返回 `Err`(调用方负责用户可见的错误处理)。
pub fn open_file<P: AsRef<Path>>(path: P) -> Result<(), String> {
    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut conn = Connection::open(p).map_err(|e| e.to_string())?;
    setup_and_migrate(&mut conn).map_err(|e| e.to_string())?;
    replace(conn);
    Ok(())
}

fn setup_and_migrate(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    let _ = conn.pragma_update(None, "foreign_keys", "ON");
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    // 对齐 better-sqlite3 默认 5000ms 超时:托盘驻留下二次启动并发写同一 WAL 库时,
    // 短暂锁争用重试而非立刻 SQLITE_BUSY 丢写。
    let _ = conn.pragma_update(None, "busy_timeout", "5000");
    run_migrations(conn).map(|_| ())
}

fn replace(conn: Connection) {
    let mut g = DB.lock().expect("DB mutex poisoned");
    if let Some(old) = g.take() {
        let _ = old.close();
    }
    *g = Some(conn);
}

/// 沿用旧 Electron 数据路径 `%APPDATA%/herbie/herbie.db`(计划第25行、第57行);
/// 不使用 Tauri 默认 `app_data_dir`(它会落到 `%APPDATA%/com.herbie.app` 而与旧数据错位)。
/// 切片6/7 在 `run()` 启动处正式调用 `open_file(default_db_path())`。
pub fn default_db_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").expect("APPDATA not set");
    PathBuf::from(appdata).join("herbie").join("herbie.db")
}