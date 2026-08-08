//! settings 仓库,逐条翻译 `src/main/settings.ts`。
//!
//! 仓储层取 `&Connection` 参数(不自行锁全局);命令层在 `lib.rs` 锁全局后传入,
//! 以避免 Tauri 命令层与仓储层重入同一 `Mutex`(rusqlite `Mutex` 不可重入)。
//!
//! 本切片未引入 `serde`/`SettingsKey` 枚举 —— 当前命令入参用 `String` 键即可,
//! 避免引入非必要依赖;待切片2 与前端契约确定后再补 `serde` enum。

use rusqlite::{params, Connection};

/// 键名常量(对齐 TS `SettingsKey = keyof Settings`;renderer 侧 exportDir/draft 以
/// 字符串字面量读写,此处只保留 Rust 侧用到的键)。
pub const KEY_SHORTCUT: &str = "shortcut";
pub const KEY_IDLE_THRESHOLD_SEC: &str = "idleThresholdSec";
pub const KEY_SHORTCUT_ERROR: &str = "shortcutError";

/// 默认值表(对齐 TS `DEFAULTS`。TS 中 `shortcutError: null`,`getDefault` 把 null/undefined
/// 归一为空串;此处直接给空串)。
fn default_for(key: &str) -> String {
    match key {
        "shortcut" => "Ctrl+Shift+Space".to_string(),
        "exportDir" => String::new(),
        "draft" => String::new(),
        "idleThresholdSec" => "300".to_string(),
        "shortcutError" => String::new(),
        _ => String::new(),
    }
}

/// `getSetting(key) -> string | null`:查无返 None。
pub fn get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

/// `setSetting(key, value)`:upsert。
pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// `getSettingWithDefault(key)`:get ?? default。
pub fn get_with_default(conn: &Connection, key: &str) -> String {
    get(conn, key).unwrap_or_else(|| default_for(key))
}

/// `getAllSettings()`:取全表 key/value,以 `Vec<(String,String)>` 返回。
pub fn get_all(conn: &Connection) -> Vec<(String, String)> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings").unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;
    use rusqlite::Connection;

    // 等价 better-sqlite3 默认 `foreign_keys = ON`;用例各自持有**局部** `Connection`,
    // 不经全局 `db::DB`,避免 cargo test 多线程并行下共享单例的竞态数据污染。
    fn make() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        run_migrations(&mut conn).unwrap();
        conn
    }

    #[test]
    fn returns_none_for_unset_key() {
        let conn = make();
        assert_eq!(get(&conn, "shortcut"), None);
    }

    #[test]
    fn writes_and_reads_back_a_value() {
        let conn = make();
        set(&conn, "shortcut", "Alt+Space").unwrap();
        assert_eq!(get(&conn, "shortcut"), Some("Alt+Space".to_string()));
    }

    #[test]
    fn upserts_on_conflict() {
        let conn = make();
        set(&conn, "exportDir", "/a").unwrap();
        set(&conn, "exportDir", "/b").unwrap();
        assert_eq!(get(&conn, "exportDir"), Some("/b".to_string()));
    }

    #[test]
    fn returns_default_when_unset_via_getting_with_default() {
        let conn = make();
        assert_eq!(get_with_default(&conn, "shortcut"), "Ctrl+Shift+Space");
    }

    #[test]
    fn overrides_default_once_set() {
        let conn = make();
        set(&conn, "shortcut", "Ctrl+K").unwrap();
        assert_eq!(get_with_default(&conn, "shortcut"), "Ctrl+K");
    }

    #[test]
    fn get_all_returns_empty_when_nothing_set() {
        let conn = make();
        assert!(get_all(&conn).is_empty());
    }

    #[test]
    fn get_all_returns_all_set_keys() {
        let conn = make();
        set(&conn, "shortcut", "Ctrl+K").unwrap();
        set(&conn, "exportDir", "/x").unwrap();
        let all = get_all(&conn);
        assert_eq!(all.len(), 2);
        let map: std::collections::HashMap<String, String> = all.into_iter().collect();
        assert_eq!(map.get("shortcut"), Some(&"Ctrl+K".to_string()));
        assert_eq!(map.get("exportDir"), Some(&"/x".to_string()));
    }
}