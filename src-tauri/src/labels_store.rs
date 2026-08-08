//! labels 仓储,逐字翻译 `src/main/labels-store.ts`。仓储层取 `&Connection` 参数(不自行锁全局,
//! 不自行包事务),由命令层(或上层 todos 仓储)在事务内调用以保证与关联写入的原子性。
//! 对齐 better-sqlite3 `INSERT OR IGNORE` 的去重语义(依赖 PK 约束)。

use rusqlite::{params, Connection};

/// 重新解析并存储某 todo 的标签:删除该 todo 既有行后批量插入新集合。
/// 不自行包事务;调用方按需在外层事务内调用。重复标签由 PK + `INSERT OR IGNORE` 去重。
pub fn update_todo_labels(
    conn: &Connection,
    todo_id: &str,
    labels: &[String],
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM todo_labels WHERE todoId = ?1",
        params![todo_id],
    )?;
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO todo_labels (todoId, label) VALUES (?1, ?2)",
    )?;
    for label in labels {
        stmt.execute(params![todo_id, label])?;
    }
    Ok(())
}

/// 取某 todo 的所有标签,按 label 升序。TS 镜像仓储读 API,由本模块单测覆盖;
/// 生产 renderer 走聚合路径(`todos_labels`),故仅测试使用。
#[allow(dead_code)]
pub fn labels_for_todo(conn: &Connection, todo_id: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT label FROM todo_labels WHERE todoId = ?1 ORDER BY label")
        .unwrap();
    stmt.query_map(params![todo_id], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

/// 重新解析并存储某 journal entry 的标签(表 `journal_labels`,列 `journalId`)。
pub fn update_journal_labels(
    conn: &Connection,
    journal_id: &str,
    labels: &[String],
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM journal_labels WHERE journalId = ?1",
        params![journal_id],
    )?;
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO journal_labels (journalId, label) VALUES (?1, ?2)",
    )?;
    for label in labels {
        stmt.execute(params![journal_id, label])?;
    }
    Ok(())
}

/// 取某 journal entry 的标签,按 label 升序。同 `labels_for_todo`:仅测试使用。
#[allow(dead_code)]
pub fn labels_for_journal(conn: &Connection, journal_id: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT label FROM journal_labels WHERE journalId = ?1 ORDER BY label")
        .unwrap();
    stmt.query_map(params![journal_id], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;
    use rusqlite::params;

    fn make() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        run_migrations(&mut conn).unwrap();
        conn
    }

    fn insert_todo(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL)",
            params![id, "t", "", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
        )
        .unwrap();
    }

    fn insert_journal(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
             VALUES (?1, NULL, ?2, ?3, ?4, ?5, NULL)",
            params![id, "body", "2026-08-04", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
        )
        .unwrap();
    }

    #[test]
    fn inserts_labels_for_a_todo() {
        let conn = make();
        insert_todo(&conn, "t1");
        update_todo_labels(&conn, "t1", &["work".to_string(), "home".to_string()]).unwrap();
        assert_eq!(labels_for_todo(&conn, "t1"), vec!["home".to_string(), "work".to_string()]);
    }

    #[test]
    fn replaces_labels_on_reparse_delete_then_insert() {
        let conn = make();
        insert_todo(&conn, "t1");
        update_todo_labels(&conn, "t1", &["work".to_string(), "home".to_string()]).unwrap();
        update_todo_labels(&conn, "t1", &["work".to_string(), "meta".to_string()]).unwrap();
        assert_eq!(
            labels_for_todo(&conn, "t1"),
            vec!["meta".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn clears_all_labels_when_given_empty_set() {
        let conn = make();
        insert_todo(&conn, "t1");
        update_todo_labels(&conn, "t1", &["work".to_string()]).unwrap();
        update_todo_labels(&conn, "t1", &[]).unwrap();
        assert!(labels_for_todo(&conn, "t1").is_empty());
    }

    #[test]
    fn ignores_duplicate_labels_within_one_parse() {
        let conn = make();
        insert_todo(&conn, "t1");
        update_todo_labels(
            &conn,
            "t1",
            &["work".to_string(), "work".to_string(), "home".to_string()],
        )
        .unwrap();
        assert_eq!(
            labels_for_todo(&conn, "t1"),
            vec!["home".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn inserts_labels_for_a_journal_entry() {
        let conn = make();
        insert_journal(&conn, "j1");
        update_journal_labels(&conn, "j1", &["work".to_string(), "meeting".to_string()]).unwrap();
        assert_eq!(
            labels_for_journal(&conn, "j1"),
            vec!["meeting".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn replaces_journal_labels_on_reparse_delete_then_insert() {
        let conn = make();
        insert_journal(&conn, "j1");
        update_journal_labels(&conn, "j1", &["work".to_string(), "home".to_string()]).unwrap();
        update_journal_labels(&conn, "j1", &["work".to_string(), "meta".to_string()]).unwrap();
        assert_eq!(
            labels_for_journal(&conn, "j1"),
            vec!["meta".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn shares_the_label_namespace_with_todos_same_label_string() {
        let conn = make();
        insert_todo(&conn, "t1");
        insert_journal(&conn, "j1");
        update_todo_labels(&conn, "t1", &["work".to_string()]).unwrap();
        update_journal_labels(&conn, "j1", &["work".to_string()]).unwrap();
        assert_eq!(labels_for_todo(&conn, "t1"), vec!["work".to_string()]);
        assert_eq!(labels_for_journal(&conn, "j1"), vec!["work".to_string()]);
    }
}