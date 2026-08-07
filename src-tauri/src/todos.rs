//! todos 仓储,逐行翻译自 `src/main/todos.ts`。
//!
//! 模型与命令入参用 `serde`;返回 `Todo` 序列化为 **camelCase**(`createdAt`/`updatedAt`/...)以与
//! TS `Api.Todo` 形状一致;SQL 列名为 camelCase(见 `migrations/0001.sql`),`row_to_todo` 按
//! 列名读出后映射到结构体 snake_case 字段。
//!
//! 仓储层取 `&Connection`/`&mut Connection` 参数,不自行锁全局(对齐切片1 Settings 模式)。
//! `create_todo`/`update_todo` 在单个事务内写入 todos + todo_labels,保证与标签重解析原子。

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoFilter {
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoInput {
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoPatch {
    pub title: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

/// 等价 JS `new Date().toISOString()`:毫秒精度、`Z` 后缀(非 `+00:00`),与既有数据格式一致。
pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn row_to_todo(r: &rusqlite::Row) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: r.get("id")?,
        title: r.get("title")?,
        detail: r.get("detail")?,
        created_at: r.get("createdAt")?,
        updated_at: r.get("updatedAt")?,
        completed_at: r.get("completedAt")?,
        deleted_at: r.get("deletedAt")?,
    })
}

fn fetch_todo(conn: &Connection, id: &str) -> rusqlite::Result<Todo> {
    conn.query_row(
        "SELECT id, title, detail, createdAt, updatedAt, completedAt, deletedAt FROM todos WHERE id = ?1",
        params![id],
        row_to_todo,
    )
}

/// `listTodos(filter?)`:`SELECT * FROM todos WHERE deletedAt IS NULL`
/// [+ `AND id IN (SELECT todoId FROM todo_labels WHERE label IN (?,...))]`
/// + `ORDER BY (completedAt IS NULL) DESC, completedAt DESC, createdAt DESC`。
pub fn list_todos(
    conn: &Connection,
    filter: Option<&TodoFilter>,
) -> rusqlite::Result<Vec<Todo>> {
    let mut sql = String::from("SELECT id, title, detail, createdAt, updatedAt, completedAt, deletedAt FROM todos WHERE deletedAt IS NULL");
    let mut binds: Vec<String> = Vec::new();
    if let Some(f) = filter {
        if let Some(labels) = &f.labels {
            if !labels.is_empty() {
                let placeholders: Vec<&str> = (0..labels.len()).map(|_| "?").collect();
                sql.push_str(&format!(
                    " AND id IN (SELECT todoId FROM todo_labels WHERE label IN ({}))",
                    placeholders.join(",")
                ));
                binds.extend(labels.iter().cloned());
            }
        }
    }
    sql.push_str(" ORDER BY (completedAt IS NULL) DESC, completedAt DESC, createdAt DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(binds.iter().map(|s| s.as_str())),
        row_to_todo,
    )?;
    rows.collect()
}

/// `listTodoLabels()`:`SELECT label, COUNT(DISTINCT tl.todoId) AS count ...`,`ORDER BY count DESC, label ASC`。
pub fn list_todo_labels(conn: &Connection) -> rusqlite::Result<Vec<LabelCount>> {
    let mut stmt = conn.prepare(
        "SELECT label, COUNT(DISTINCT tl.todoId) AS count
         FROM todo_labels tl
         JOIN todos t ON t.id = tl.todoId
         WHERE t.deletedAt IS NULL
         GROUP BY label
         ORDER BY count DESC, label ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(LabelCount {
            label: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    rows.collect()
}

/// `createTodo(input)`:id=randomUUID;now;title=trim;detail 不 trim;事务内 INSERT + updateTodoLabels(parseLabels(detail));回读。
pub fn create_todo(
    conn: &mut Connection,
    input: &TodoInput,
) -> rusqlite::Result<Todo> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let title = input.title.trim();
    let detail = &input.detail;
    let labels = crate::labels::parse_labels(detail);
    {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL)",
            params![id, title, detail, &now, &now],
        )?;
        crate::labels_store::update_todo_labels(&tx, &id, &labels)?;
        tx.commit()?;
    }
    fetch_todo(conn, &id)
}

/// `updateTodo(id,patch)`:无则 `todo not found: <id>`;title=Some?trim:existing;detail=Some?:existing(不 trim);事务内 UPDATE + updateTodoLabels;回读。
pub fn update_todo(
    conn: &mut Connection,
    id: &str,
    patch: &TodoPatch,
) -> Result<Todo, String> {
    let existing = fetch_todo(conn, id).map_err(|_| format!("todo not found: {}", id))?;
    let title = match &patch.title {
        Some(t) => t.trim(),
        None => &existing.title[..],
    };
    let detail = match &patch.detail {
        Some(d) => d.clone(),
        None => existing.detail.clone(),
    };
    let now = now_iso();
    let labels = crate::labels::parse_labels(&detail);
    {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE todos SET title = ?1, detail = ?2, updatedAt = ?3 WHERE id = ?4",
            params![title, detail, now, id],
        )
        .map_err(|e| e.to_string())?;
        crate::labels_store::update_todo_labels(&tx, id, &labels).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    fetch_todo(conn, id).map_err(|e| e.to_string())
}

/// `toggleTodo(id,done)`:completedAt=done?now:null;UPDATE;回读。
pub fn toggle_todo(conn: &Connection, id: &str, done: bool) -> rusqlite::Result<Todo> {
    let now = now_iso();
    let completed_at: Option<String> = if done {
        Some(now.clone())
    } else {
        None
    };
    conn.execute(
        "UPDATE todos SET completedAt = ?1, updatedAt = ?2 WHERE id = ?3",
        params![completed_at, now, id],
    )?;
    fetch_todo(conn, id)
}

/// `softDeleteTodo(id)`:`UPDATE todos SET deletedAt=?, updatedAt=? WHERE id=?`。
pub fn soft_delete_todo(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    let now = now_iso();
    conn.execute(
        "UPDATE todos SET deletedAt = ?1, updatedAt = ?2 WHERE id = ?3",
        params![now, now, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;
    use std::thread::sleep;
    use std::time::Duration;

    fn make() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        run_migrations(&mut conn).unwrap();
        conn
    }

    fn wait() {
        sleep(Duration::from_millis(5));
    }

    fn label_counts(conn: &Connection) -> Vec<(String, i64)> {
        list_todo_labels(conn)
            .unwrap()
            .into_iter()
            .map(|c| (c.label, c.count))
            .collect()
    }

    #[test]
    fn creates_a_todo_with_trimmed_title() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput {
            title: "  Buy milk  ".to_string(),
            detail: String::new(),
        })
        .unwrap();
        assert_eq!(t.title, "Buy milk");
        assert!(t.completed_at.is_none());
        assert!(t.deleted_at.is_none());
        assert!(Uuid::parse_str(&t.id).is_ok(), "id is a uuid v4 string: {}", t.id);
    }

    #[test]
    fn lists_todos_by_created_at_descending_among_pending() {
        let mut conn = make();
        let a = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        wait();
        let b = create_todo(&mut conn, &TodoInput { title: "b".into(), detail: String::new() }).unwrap();
        let list = list_todos(&conn, None).unwrap();
        assert_eq!(list.iter().map(|t| t.id.clone()).collect::<Vec<_>>(), vec![b.id, a.id]);
    }

    #[test]
    fn places_done_items_after_pending() {
        let mut conn = make();
        let a = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        wait();
        let b = create_todo(&mut conn, &TodoInput { title: "b".into(), detail: String::new() }).unwrap();
        toggle_todo(&conn, &b.id, true).unwrap();
        let list = list_todos(&conn, None).unwrap();
        assert_eq!(list[0].id, a.id);
        assert_eq!(list[1].id, b.id);
    }

    #[test]
    fn parses_labels_on_create_and_exposes_counts_via_list_todo_labels() {
        let mut conn = make();
        create_todo(&mut conn, &TodoInput { title: "t1".into(), detail: "do #work and #meta".into() }).unwrap();
        create_todo(&mut conn, &TodoInput { title: "t2".into(), detail: "also #work".into() }).unwrap();
        let counts = label_counts(&conn);
        let work = counts.iter().find(|(l, _)| l == "work").map(|(_, c)| *c);
        let meta = counts.iter().find(|(l, _)| l == "meta").map(|(_, c)| *c);
        assert_eq!(work, Some(2));
        assert_eq!(meta, Some(1));
    }

    #[test]
    fn re_parses_labels_on_update() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "t".into(), detail: "#work".into() }).unwrap();
        update_todo(&mut conn, &t.id, &TodoPatch {
            title: None,
            detail: Some("#home #work".into()),
        })
        .unwrap();
        let mut labels: Vec<String> = label_counts(&conn).into_iter().map(|(l, _)| l).collect();
        labels.sort();
        assert_eq!(labels, vec!["home".to_string(), "work".to_string()]);
    }

    #[test]
    fn removes_a_label_when_detail_no_longer_has_it() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "t".into(), detail: "#work #home".into() }).unwrap();
        update_todo(&mut conn, &t.id, &TodoPatch {
            title: None,
            detail: Some("no tags here".into()),
        })
        .unwrap();
        assert!(list_todo_labels(&conn).unwrap().is_empty());
    }

    #[test]
    fn skips_hash_inside_urls_when_parsing_labels() {
        let mut conn = make();
        create_todo(&mut conn, &TodoInput { title: "t".into(), detail: "see https://x.io/p#sec and #real".into() }).unwrap();
        let labels: Vec<String> = list_todo_labels(&conn).unwrap().into_iter().map(|c| c.label).collect();
        assert_eq!(labels, vec!["real".to_string()]);
    }

    #[test]
    fn toggle_true_sets_completed_at_false_clears_it() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "t".into(), detail: String::new() }).unwrap();
        let done = toggle_todo(&conn, &t.id, true).unwrap();
        assert!(done.completed_at.is_some());
        let undone = toggle_todo(&conn, &t.id, false).unwrap();
        assert!(undone.completed_at.is_none());
    }

    #[test]
    fn soft_delete_removes_from_list_and_excludes_from_label_counts() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "t".into(), detail: "#work".into() }).unwrap();
        soft_delete_todo(&conn, &t.id).unwrap();
        assert!(list_todos(&conn, None).unwrap().is_empty());
        assert!(list_todo_labels(&conn).unwrap().is_empty());
    }

    #[test]
    fn filters_by_labels_using_or_union() {
        let mut conn = make();
        let t1 = create_todo(&mut conn, &TodoInput { title: "t1".into(), detail: "#work".into() }).unwrap();
        create_todo(&mut conn, &TodoInput { title: "t2".into(), detail: "#home".into() }).unwrap();
        let t3 = create_todo(&mut conn, &TodoInput { title: "t3".into(), detail: "#work #home".into() }).unwrap();
        let filtered = list_todos(&conn, Some(&TodoFilter { labels: Some(vec!["work".into()]) })).unwrap();
        let mut got: Vec<String> = filtered.iter().map(|t| t.id.clone()).collect();
        got.sort();
        let mut want = vec![t1.id, t3.id];
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn filter_with_multiple_labels_is_union_not_intersection() {
        let mut conn = make();
        let t1 = create_todo(&mut conn, &TodoInput { title: "t1".into(), detail: "#work".into() }).unwrap();
        let t2 = create_todo(&mut conn, &TodoInput { title: "t2".into(), detail: "#home".into() }).unwrap();
        create_todo(&mut conn, &TodoInput { title: "t3".into(), detail: "#meta".into() }).unwrap();
        let filtered = list_todos(&conn, Some(&TodoFilter { labels: Some(vec!["work".into(), "home".into()]) })).unwrap();
        let mut got: Vec<String> = filtered.iter().map(|t| t.id.clone()).collect();
        got.sort();
        let mut want = vec![t1.id, t2.id];
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn update_throws_on_unknown_id() {
        let mut conn = make();
        let res = update_todo(&mut conn, "does-not-exist", &TodoPatch {
            title: None,
            detail: Some("x".into()),
        });
        assert!(res.is_err());
    }

    #[test]
    fn omits_completed_at_column_ordering_done_sorted_by_completed_at_desc() {
        let mut conn = make();
        let a = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        wait();
        let b = create_todo(&mut conn, &TodoInput { title: "b".into(), detail: String::new() }).unwrap();
        toggle_todo(&conn, &a.id, true).unwrap();
        wait();
        toggle_todo(&conn, &b.id, true).unwrap();
        let done: Vec<String> = list_todos(&conn, None)
            .unwrap()
            .into_iter()
            .filter(|t| t.completed_at.is_some())
            .map(|t| t.id)
            .collect();
        assert_eq!(done, vec![b.id, a.id]);
    }
}