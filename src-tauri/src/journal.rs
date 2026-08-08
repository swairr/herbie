//! journal 仓储,逐行翻译自 `src/main/journals.ts`。
//!
//! serde 对齐 TS `src/shared/types.ts`:`JournalEntry` 返回 **camelCase** 字段(createdAt/
//! updatedAt/deletedAt),`JournalInput`/`JournalPatch` 接收 camelCase;SQL 列名同为 camelCase
//! (见 `migrations/0003.sql`),`row_to_journal` 按列名读出后映射到 snake_case 字段。
//!
//! `JournalPatch.title` 用 **`Option<Option<String>>`** 表示三态(null/空串/串),处理方式
//! 照 `segment.rs` 的 `deserialize_todo_id` 模式(见字段注释);`body`/`date` 用 `Option<String>`
//! (缺省 None=保持原值)。
//!
//! 仓储层取 `&Connection`/`&mut Connection` 参数,不自行锁全局;`create_journal`/`update_journal`
//! 在单个事务内写 journal_entries + journal_labels,保证与标签重解析原子。

use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::{day_bounds, local_date_string};
use crate::todos::now_iso;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: String,
    pub title: Option<String>,
    pub body: String,
    pub date: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalInput {
    // body 在 TS 标注必填,但运行时 `input.body == null ? ''` 兜底,故用 Option 对齐。
    pub title: Option<String>,
    pub body: Option<String>,
    pub date: Option<String>,
}

/// `JournalPatch` 对齐 TS `{ title?: string | null; body?: string; date?: string }`。
///
/// `title` 用 **`Option<Option<String>>`** 表示三种语义:
/// - 外层 `None`:patch 未带 `title` 字段 → **保持原值**;
/// - `Some(None)`:前端显式传 `title: null`(或空串)→ **置空**(title = NULL);
/// - `Some(Some(x))`:传 `title: "x"` → **设值**(trim 后)。
///
/// serde 对标准 `Option<Option<T>>` 不区分 JSON `null` 与缺省字段(两者都映射为外层 None),
/// 故此处用 `#[serde(default)]`(缺省 → None 不调 deserializer)+ 自定义 deserializer
/// (到场值 `null`/空串 → `Some(None)`,非空串 trim 后 → `Some(Some(x))`)补齐三态。
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JournalPatch {
    #[serde(default, deserialize_with = "deserialize_title")]
    pub title: Option<Option<String>>,
    pub body: Option<String>,
    pub date: Option<String>,
}

fn deserialize_title<'de, D>(d: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // 仅在字段到场时被调用;缺省由 serde default 处理为外层 None。
    let v = Option::<String>::deserialize(d)?;
    // 对齐 TS `updateJournal` 的 `patch.title == null || patch.title.trim().length === 0
    // ? null : patch.title.trim()` —— null 与空串都归为置空(置 NULL),非空串 trim 后设值。
    Ok(match v {
        None => Some(None),
        Some(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(None)
            } else {
                Some(Some(t.to_string()))
            }
        }
    })
}

fn row_to_journal(r: &Row) -> rusqlite::Result<JournalEntry> {
    Ok(JournalEntry {
        id: r.get("id")?,
        title: r.get("title")?,
        body: r.get("body")?,
        date: r.get("date")?,
        created_at: r.get("createdAt")?,
        updated_at: r.get("updatedAt")?,
        deleted_at: r.get("deletedAt")?,
    })
}

fn fetch_journal(conn: &Connection, id: &str) -> rusqlite::Result<JournalEntry> {
    conn.query_row(
        "SELECT id, title, body, date, createdAt, updatedAt, deletedAt FROM journal_entries WHERE id = ?1",
        params![id],
        row_to_journal,
    )
}

/// `listJournals(day)`:`SELECT * WHERE date = ? AND deletedAt IS NULL ORDER BY createdAt ASC`。
pub fn list_journals(conn: &Connection, day: &str) -> rusqlite::Result<Vec<JournalEntry>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM journal_entries
         WHERE date = ?1 AND deletedAt IS NULL
         ORDER BY createdAt ASC",
    )?;
    let rows = stmt.query_map(params![day], row_to_journal)?;
    rows.collect()
}

/// `createJournal(input)`:body 空则 Err;date 缺省/空串 → 本地今天;`day_bounds` 校验;
/// title 空 → None 否则 trim;id=uuid;now=now_iso;事务 INSERT + updateJournalLabels(parseLabels(body));回读。
pub fn create_journal(
    conn: &mut Connection,
    input: &JournalInput,
) -> Result<JournalEntry, String> {
    let body = input.body.as_deref().unwrap_or("");
    if body.trim().is_empty() {
        return Err("journal body must not be empty".to_string());
    }
    let date = match &input.date {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => local_date_string(Utc::now()),
    };
    if day_bounds(&date).is_none() {
        return Err(format!("invalid date: {date}"));
    }
    let title_raw = input.title.as_deref().unwrap_or("");
    let title: Option<String> = {
        let t = title_raw.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    };
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let labels = crate::labels::parse_labels(body);
    {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![id, title, body, date, &now, &now],
        )
        .map_err(|e| e.to_string())?;
        crate::labels_store::update_journal_labels(&tx, &id, &labels).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    fetch_journal(conn, &id).map_err(|e| e.to_string())
}

/// `updateJournal(id, patch)`:无则 Err("journal not found: {id}");title 三态;body 有则
/// 空则 Err 否则用;date 用 patch ?? existing(day_bounds 校验);now;事务 UPDATE +
/// updateJournalLabels(parseLabels(body));回读。
pub fn update_journal(
    conn: &mut Connection,
    id: &str,
    patch: &JournalPatch,
) -> Result<JournalEntry, String> {
    let existing = fetch_journal(conn, id).map_err(|_| format!("journal not found: {}", id))?;
    let title = match &patch.title {
        None => existing.title.clone(),
        Some(t) => t.clone(),
    };
    let body = match &patch.body {
        Some(b) => {
            if b.trim().is_empty() {
                return Err("journal body must not be empty".to_string());
            }
            b.clone()
        }
        None => existing.body.clone(),
    };
    let date = match &patch.date {
        Some(d) => d.clone(),
        None => existing.date.clone(),
    };
    if day_bounds(&date).is_none() {
        return Err(format!("invalid date: {date}"));
    }
    let now = now_iso();
    let labels = crate::labels::parse_labels(&body);
    {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE journal_entries SET title = ?1, body = ?2, date = ?3, updatedAt = ?4 WHERE id = ?5",
            params![title, body, date, &now, id],
        )
        .map_err(|e| e.to_string())?;
        crate::labels_store::update_journal_labels(&tx, id, &labels).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    fetch_journal(conn, id).map_err(|e| e.to_string())
}

/// `softDeleteJournal(id)`:`UPDATE journal_entries SET deletedAt=?, updatedAt=? WHERE id=?`(now, now)。
pub fn soft_delete_journal(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    let now = now_iso();
    conn.execute(
        "UPDATE journal_entries SET deletedAt = ?1, updatedAt = ?2 WHERE id = ?3",
        params![now, now, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels_store::labels_for_journal;
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

    fn input(body: &str) -> JournalInput {
        JournalInput {
            title: None,
            body: Some(body.to_string()),
            date: None,
        }
    }

    fn ids(list: Vec<JournalEntry>) -> Vec<String> {
        list.into_iter().map(|e| e.id).collect()
    }

    #[test]
    fn creates_a_journal_entry_with_required_body_and_defaults_date_to_today() {
        let mut conn = make();
        let e = create_journal(&mut conn, &input("First note #work")).unwrap();
        assert!(Uuid::parse_str(&e.id).is_ok(), "id is a uuid v4 string: {}", e.id);
        assert!(e.title.is_none());
        assert_eq!(e.body, "First note #work");
        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        assert!(re.is_match(&e.date), "date should be YYYY-MM-DD: {}", e.date);
        assert!(e.deleted_at.is_none());
    }

    #[test]
    fn stores_a_trimmed_title_when_provided_null_when_blank() {
        let mut conn = make();
        let a = create_journal(
            &mut conn,
            &JournalInput {
                title: Some("  Meeting  ".into()),
                body: Some("body".into()),
                date: None,
            },
        )
        .unwrap();
        assert_eq!(a.title, Some("Meeting".to_string()));
        let b = create_journal(
            &mut conn,
            &JournalInput {
                title: Some("   ".into()),
                body: Some("body".into()),
                date: None,
            },
        )
        .unwrap();
        assert!(b.title.is_none());
    }

    #[test]
    fn rejects_an_empty_body_on_create() {
        let mut conn = make();
        let res = create_journal(
            &mut conn,
            &JournalInput {
                title: None,
                body: Some("   ".into()),
                date: None,
            },
        );
        assert!(res.is_err());
        let res2 = create_journal(
            &mut conn,
            &JournalInput {
                title: None,
                body: Some(String::new()),
                date: None,
            },
        );
        assert!(res2.is_err());
    }

    #[test]
    fn rejects_an_invalid_date_on_create() {
        let mut conn = make();
        let res = create_journal(
            &mut conn,
            &JournalInput {
                title: None,
                body: Some("x".into()),
                date: Some("not-a-date".into()),
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn lists_entries_for_a_day_ordered_by_created_at_ascending() {
        let mut conn = make();
        let a = create_journal(&mut conn, &input("a")).unwrap();
        wait();
        let b = create_journal(&mut conn, &input("b")).unwrap();
        let list = list_journals(&conn, &a.date).unwrap();
        assert_eq!(ids(list), vec![a.id, b.id]);
    }

    #[test]
    fn only_lists_entries_of_the_requested_day() {
        let mut conn = make();
        let today = create_journal(&mut conn, &input("today")).unwrap();
        let other = create_journal(
            &mut conn,
            &JournalInput {
                title: None,
                body: Some("past".into()),
                date: Some("2020-01-01".into()),
            },
        )
        .unwrap();
        assert_eq!(
            ids(list_journals(&conn, &today.date).unwrap()),
            vec![today.id]
        );
        assert_eq!(
            ids(list_journals(&conn, "2020-01-01").unwrap()),
            vec![other.id]
        );
    }

    #[test]
    fn re_parses_labels_from_body_on_create_and_update() {
        let mut conn = make();
        let e = create_journal(&mut conn, &input("do #work and #meeting")).unwrap();
        assert_eq!(
            labels_for_journal(&conn, &e.id),
            vec!["meeting".to_string(), "work".to_string()]
        );
        update_journal(
            &mut conn,
            &e.id,
            &JournalPatch {
                title: None,
                body: Some("now only #work".into()),
                date: None,
            },
        )
        .unwrap();
        assert_eq!(labels_for_journal(&conn, &e.id), vec!["work".to_string()]);
    }

    #[test]
    fn update_edits_title_body_date_and_bumps_updated_at() {
        let mut conn = make();
        let e = create_journal(&mut conn, &input("orig")).unwrap();
        wait();
        let updated = update_journal(
            &mut conn,
            &e.id,
            &JournalPatch {
                title: Some(Some("T".into())),
                body: Some("new #tag".into()),
                date: Some("2020-06-06".into()),
            },
        )
        .unwrap();
        assert_eq!(updated.title, Some("T".to_string()));
        assert_eq!(updated.body, "new #tag");
        assert_eq!(updated.date, "2020-06-06");
        assert_ne!(updated.updated_at, e.updated_at);
        assert_eq!(labels_for_journal(&conn, &e.id), vec!["tag".to_string()]);
    }

    #[test]
    fn update_rejects_empty_body_when_body_is_provided() {
        let mut conn = make();
        let e = create_journal(&mut conn, &input("orig")).unwrap();
        let res = update_journal(
            &mut conn,
            &e.id,
            &JournalPatch {
                title: None,
                body: Some("   ".into()),
                date: None,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn update_clears_title_when_set_to_null() {
        let mut conn = make();
        let e = create_journal(
            &mut conn,
            &JournalInput {
                title: Some("T".into()),
                body: Some("orig".into()),
                date: None,
            },
        )
        .unwrap();
        let updated = update_journal(
            &mut conn,
            &e.id,
            &JournalPatch {
                title: Some(None),
                body: None,
                date: None,
            },
        )
        .unwrap();
        assert!(updated.title.is_none());
    }

    #[test]
    fn update_throws_on_unknown_id() {
        let mut conn = make();
        let res = update_journal(
            &mut conn,
            "nope",
            &JournalPatch {
                title: None,
                body: Some("x".into()),
                date: None,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn soft_delete_hides_entry_from_list() {
        let mut conn = make();
        let e = create_journal(&mut conn, &input("gone")).unwrap();
        soft_delete_journal(&conn, &e.id).unwrap();
        assert!(list_journals(&conn, &e.date).unwrap().is_empty());
    }

    #[test]
    fn reschedules_entry_via_date_patch_to_a_different_day() {
        let mut conn = make();
        let e = create_journal(
            &mut conn,
            &JournalInput {
                title: None,
                body: Some("x".into()),
                date: Some("2020-01-01".into()),
            },
        )
        .unwrap();
        update_journal(
            &mut conn,
            &e.id,
            &JournalPatch {
                title: None,
                body: None,
                date: Some("2020-02-02".into()),
            },
        )
        .unwrap();
        assert!(list_journals(&conn, "2020-01-01").unwrap().is_empty());
        assert_eq!(
            ids(list_journals(&conn, "2020-02-02").unwrap()),
            vec![e.id]
        );
    }

    // 计划要求补的 serde 三态单测:缺省(title 不带)→ 保持;null/空串 → 置空;非空串 trim 后设值。
    #[test]
    fn journal_patch_title_deserializes_three_states() {
        let absent: JournalPatch = serde_json::from_str(r#"{"body":"x"}"#).unwrap();
        assert!(absent.title.is_none());
        let null: JournalPatch = serde_json::from_str(r#"{"title":null,"body":"x"}"#).unwrap();
        assert_eq!(null.title, Some(None));
        let empty: JournalPatch = serde_json::from_str(r#"{"title":"   "}"#).unwrap();
        assert_eq!(empty.title, Some(None));
        let value: JournalPatch = serde_json::from_str(r#"{"title":"  T  "}"#).unwrap();
        assert_eq!(value.title, Some(Some("T".to_string())));
    }
}
