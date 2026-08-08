//! 有序迁移运行器,逐字翻译自 `src/main/migrations.ts` 的语义。
//! SQL 文件用 `include_str!` 内嵌(等价 TS 的 `?raw`)。

use std::collections::HashSet;

use chrono::Utc;
use rusqlite::Connection;

const SQL_0001: &str = include_str!("../../migrations/0001.sql");
const SQL_0002: &str = include_str!("../../migrations/0002.sql");
const SQL_0003: &str = include_str!("../../migrations/0003.sql");

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: [Migration; 3] = [
    Migration { version: 1, sql: SQL_0001 },
    Migration { version: 2, sql: SQL_0002 },
    Migration { version: 3, sql: SQL_0003 },
];

/// 在给定连接上执行所有未应用的迁移,返回最后应用的版本号。
/// 语义对齐 TS:`CREATE TABLE IF NOT EXISTS migrations` → 读已应用版本入 Set →
/// 对每个未应用迁移在单个 transaction 内 `execute_batch(sql)` + 插入 `(version, appliedAt)`。
pub fn run_migrations(conn: &mut Connection) -> Result<i64, rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS migrations (\n\
         \x20\x20version   INTEGER PRIMARY KEY,\n\
         \x20\x20appliedAt TEXT NOT NULL\n\
         );",
    )?;

    let mut applied: HashSet<i64> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT version FROM migrations")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        for r in rows {
            applied.insert(r?);
        }
    }

    let mut last: i64 = 0;
    for m in &MIGRATIONS {
        if applied.contains(&m.version) {
            if m.version > last {
                last = m.version;
            }
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.execute(
            "INSERT INTO migrations (version, appliedAt) VALUES (?1, ?2)",
            rusqlite::params![m.version, crate::time::iso_utc_z_millis(Utc::now())],
        )?;
        tx.commit()?;
        last = m.version;
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    // 等价 better-sqlite3 默认 `foreign_keys = ON`(TS 迁移测试 beforeEach 仅 new Database,
    // 而 better-sqlite3 构造时默认开启 fk)。rusqlite 默认 fk=OFF,故在此显式开启以对齐。
    fn make() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        conn
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    #[test]
    fn creates_all_v1_tables() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        let names = table_names(&conn);
        for expected in ["todos", "todo_labels", "settings", "migrations"] {
            assert!(names.contains(&expected.to_string()), "missing table {expected}: {names:?}");
        }
    }

    #[test]
    fn records_version_1_as_applied() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        let v: i64 = conn
            .query_row("SELECT version FROM migrations WHERE version = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn is_idempotent_running_twice_does_not_error() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        run_migrations(&mut conn).expect("rerun should not error");
        let mut stmt = conn.prepare("SELECT version FROM migrations ORDER BY version").unwrap();
        let rows: Vec<i64> = stmt.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], 1);
        assert_eq!(rows[1], 2);
        assert_eq!(rows[2], 3);
    }

    #[test]
    fn creates_the_segments_table_v2_and_records_version_2() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        let names = table_names(&conn);
        assert!(names.contains(&"segments".to_string()));
        let v: i64 = conn
            .query_row("SELECT version FROM migrations WHERE version = 2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
    }

    #[test]
    fn v2_migration_is_idempotent_on_rerun() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        run_migrations(&mut conn).expect("rerun should not error");
    }

    #[test]
    fn segments_table_accepts_a_row_with_open_endat_and_defaults() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO segments (id, startAt, endAt, processName, title, note, todoId, kind)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, NULL, ?6)",
            params!["seg1", "2026-08-03T10:00:00Z", "app.exe", "Title", "", "activity"],
        )
        .unwrap();
        let (end_at, kind, process_name): (Option<String>, String, String) = conn
            .query_row("SELECT endAt, kind, processName FROM segments WHERE id = ?1", params!["seg1"], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert!(end_at.is_none());
        assert_eq!(kind, "activity");
        assert_eq!(process_name, "app.exe");
    }

    #[test]
    fn segments_todo_id_is_set_null_when_its_todo_is_hard_deleted() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
             VALUES (?1, ?2, '', ?3, ?4, NULL, NULL)",
            params!["t1", "t", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO segments (id, startAt, endAt, processName, title, note, todoId, kind)
             VALUES (?1, ?2, NULL, ?3, '', '', ?4, 'activity')",
            params!["seg1", "2026-08-03T10:00:00Z", "app.exe", "t1"],
        )
        .unwrap();
        conn.execute("DELETE FROM todos WHERE id = ?1", params!["t1"]).unwrap();
        let todo_id: Option<String> = conn
            .query_row("SELECT todoId FROM segments WHERE id = ?1", params!["seg1"], |r| r.get(0))
            .unwrap();
        assert!(todo_id.is_none());
    }

    #[test]
    fn enables_inserting_a_todo_row_with_all_required_fields() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
             VALUES (?1, ?2, '', ?3, ?4, NULL, NULL)",
            params!["id1", "t", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
        )
        .unwrap();
        let title: String = conn
            .query_row("SELECT title FROM todos WHERE id = ?1", params!["id1"], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "t");
    }

    #[test]
    fn cascades_todo_delete_to_todo_labels() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
             VALUES (?1, ?2, '', ?3, ?4, NULL, NULL)",
            params!["id1", "t", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO todo_labels (todoId, label) VALUES (?1, ?2)",
            params!["id1", "work"],
        )
        .unwrap();
        conn.execute("DELETE FROM todos WHERE id = ?1", params!["id1"]).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn creates_the_journal_entries_table_v3_and_records_version_3() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        let names = table_names(&conn);
        assert!(names.contains(&"journal_entries".to_string()));
        assert!(names.contains(&"journal_labels".to_string()));
        let v: i64 = conn
            .query_row("SELECT version FROM migrations WHERE version = 3", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 3);
    }

    #[test]
    fn journal_entries_accepts_a_row_with_optional_null_title() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
             VALUES (?1, NULL, ?2, ?3, ?4, ?5, NULL)",
            params![
                "j1",
                "日记正文",
                "2026-08-04",
                "2026-08-04T10:00:00Z",
                "2026-08-04T10:00:00Z"
            ],
        )
        .unwrap();
        let (title, body, date): (Option<String>, String, String) = conn
            .query_row(
                "SELECT title, body, date FROM journal_entries WHERE id = ?1",
                params!["j1"],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(title.is_none());
        assert_eq!(body, "日记正文");
        assert_eq!(date, "2026-08-04");
    }

    #[test]
    fn cascades_journal_delete_to_journal_labels() {
        let mut conn = make();
        run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
             VALUES (?1, NULL, ?2, ?3, ?4, ?5, NULL)",
            params!["j1", "b", "2026-08-04", "2026-08-04T10:00:00Z", "2026-08-04T10:00:00Z"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO journal_labels (journalId, label) VALUES (?1, ?2)",
            params!["j1", "work"],
        )
        .unwrap();
        conn.execute("DELETE FROM journal_entries WHERE id = ?1", params!["j1"]).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM journal_labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}