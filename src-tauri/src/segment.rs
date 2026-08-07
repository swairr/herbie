//! segments 仓储,逐行翻译 `src/main/segments.ts` + `segments-row.ts` + `segments-query.ts`。
//!
//! 本切片(3a)实现了写/读路径:**`open_segment` / `close_open` / `list_all` /
//! `list_segments_by_day` / `update_segment` / `fetch_todo_titles`**。涉及 `HookNotifier`
//! 抽象的 `start_tracking`/`stop_tracking` 留 3c(连同 segments.test.ts 其余 5 条用例)。
//!
//! 仓储函数取 `&Connection`(不自行锁全局、不需 `&mut`——segments 无事务);命令层在
//! `lib.rs` 锁 `db::get()` 后借 `&Connection` 传入,与 settings/todos 模式一致。
//!
//! `Segment`/`SegmentKind`/`SegmentPatch` serde 对齐 TS `src/shared/types.ts`:
//! camelCase 字段名;`kind` ∈ "activity"/"idle";`SegmentPatch.todo_id` 三态见字段注释。

use chrono::Utc;
use rusqlite::{params, params_from_iter, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::{day_bounds, split_at_midnight};
use crate::todos::now_iso;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SegmentKind {
    Activity,
    Idle,
}

impl SegmentKind {
    fn as_db_str(self) -> &'static str {
        match self {
            SegmentKind::Activity => "activity",
            SegmentKind::Idle => "idle",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub process_name: String,
    pub title: String,
    pub note: String,
    pub todo_id: Option<String>,
    pub kind: SegmentKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSegmentInput {
    pub process_name: String,
    pub title: String,
    pub kind: Option<SegmentKind>,
    pub todo_id: Option<String>,
    pub start_at: Option<String>,
}

/// `SegmentPatch` 对齐 TS `{ note?: string; todoId?: string | null }`。
///
/// `todo_id` 用 **`Option<Option<String>>`** 表示三种语义:
/// - 外层 `None`:patch 未带 `todoId` 字段 → **保持原值**;
/// - `Some(None)`:前端显式传 `todoId: null`(或空语义)→ **置空**(todoId = NULL);
/// - `Some(Some(x))`:传 `todoId: "x"` → **设值**。
///
/// serde 不区分 JSON `null` 与缺省字段的标准 `Option<Option<T>>` 会把两者都映射为外层 `None`,
/// 故此处用 `#[serde(default)]`(缺省 → `None` 不调 deserializer)+ 自定义 deserializer
/// (到场值 `null` → `Some(None)`,`"x"` → `Some(Some(x))`)补齐三态。
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SegmentPatch {
    pub note: Option<String>,
    #[serde(default, deserialize_with = "deserialize_todo_id")]
    pub todo_id: Option<Option<String>>,
}

fn deserialize_todo_id<'de, D>(d: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // 仅在字段到场时被调用;缺省由 serde default 处理为外层 None。
    let v = Option::<String>::deserialize(d)?;
    // 对齐 TS `updateSegment`:`todoId === '' ? null` —— 空串也归为置空(置 NULL)。
    Ok(match v {
        None => Some(None),
        Some(s) if s.is_empty() => Some(None),
        Some(s) => Some(Some(s)),
    })
}

fn row_to_segment(r: &Row) -> rusqlite::Result<Segment> {
    let kind_str: String = r.get("kind")?;
    let kind = if kind_str == "idle" {
        SegmentKind::Idle
    } else {
        SegmentKind::Activity
    };
    Ok(Segment {
        id: r.get("id")?,
        start_at: r.get("startAt")?,
        end_at: r.get("endAt")?,
        process_name: r.get("processName")?,
        title: r.get("title")?,
        note: r.get("note")?,
        todo_id: r.get("todoId")?,
        kind,
    })
}

/// `openSegment(input)`:id=uuid;startAt=input.startAt ?? now_iso();kind ?? Activity;
/// todoId ?? null;插入(endAt NULL);返回组装 Segment(省一次回读)。
pub fn open_segment(conn: &Connection, input: &OpenSegmentInput) -> rusqlite::Result<Segment> {
    let id = Uuid::new_v4().to_string();
    let start_at = input.start_at.clone().unwrap_or_else(now_iso);
    let kind = input.kind.unwrap_or(SegmentKind::Activity);
    let todo_id = input.todo_id.clone();
    conn.execute(
        "INSERT INTO segments (id, startAt, endAt, processName, title, note, todoId, kind)
         VALUES (?1, ?2, NULL, ?3, ?4, '', ?5, ?6)",
        params![id, start_at, input.process_name, input.title, todo_id, kind.as_db_str()],
    )?;
    Ok(Segment {
        id,
        start_at,
        end_at: None,
        process_name: input.process_name.clone(),
        title: input.title.clone(),
        note: String::new(),
        todo_id,
        kind,
    })
}

/// `closeOpen(at)`:把当前开放段(endAt NULL)的 endAt 置为 `at`,幂等。返回受影响行数(0 或 1)。
// 注:此处吞 DB 错误返 0(与 TS 抛错语义不同);当前调用方(测试/segments 命令)无需区分
// "无开放段"与"DB 错",3c tracker 接入时若需该区分,改为返回 `rusqlite::Result<i64>`。
pub fn close_open(conn: &Connection, at: &str) -> i64 {
    conn.execute(
        "UPDATE segments SET endAt = ?1 WHERE endAt IS NULL",
        params![at],
    )
    .unwrap_or(0) as i64
}

/// `listAllSegments()`:`SELECT * ORDER BY startAt ASC`。
pub fn list_all(conn: &Connection) -> rusqlite::Result<Vec<Segment>> {
    let mut stmt = conn.prepare("SELECT * FROM segments ORDER BY startAt ASC")?;
    let rows = stmt.query_map([], row_to_segment)?;
    rows.collect()
}

/// `listSegmentsByDay(localDate, now)`:先用 `day_bounds` 的 ISO 边界做 SQL 预筛
///(`startAt < endIso AND (endAt IS NULL OR endAt > startIso)`),再逐行 `split_at_midnight`
/// 切到当日。开放段按 `now` 截断。非法 `localDate` 返空。
pub fn list_segments_by_day(
    conn: &Connection,
    local_date: &str,
    now: chrono::DateTime<Utc>,
) -> Vec<Segment> {
    let bounds = match day_bounds(local_date) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let mut stmt = match conn.prepare(
        "SELECT * FROM segments
         WHERE startAt < ?1 AND (endAt IS NULL OR endAt > ?2)
         ORDER BY startAt ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map(params![bounds.end_iso, bounds.start_iso], row_to_segment) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<Segment> = Vec::new();
    for r in rows {
        if let Ok(seg) = r {
            if let Some(slice) = split_at_midnight(&seg, local_date, now) {
                out.push(slice);
            }
        }
    }
    out
}

/// `updateSegment(id, patch)`:无则 `None`;note=patch.note ?? existing.note;
/// todoId 三态;UPDATE;回读。
pub fn update_segment(conn: &Connection, id: &str, patch: &SegmentPatch) -> Option<Segment> {
    let existing = match conn.query_row(
        "SELECT * FROM segments WHERE id = ?1",
        params![id],
        row_to_segment,
    ) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let note = patch.note.clone().unwrap_or(existing.note.clone());
    let todo_id = match &patch.todo_id {
        None => existing.todo_id.clone(),
        Some(None) => None,
        Some(Some(s)) => Some(s.clone()),
    };
    let _ = conn.execute(
        "UPDATE segments SET note = ?1, todoId = ?2 WHERE id = ?3",
        params![note, todo_id, id],
    );
    conn.query_row(
        "SELECT * FROM segments WHERE id = ?1",
        params![id],
        row_to_segment,
    )
    .ok()
}

/// `fetchTodoTitles(todoIds)`:去重非空后 `SELECT id, title FROM todos WHERE id IN (...)`,
/// 返回 `(id, title)` 列表(供 3c/切片5 的聚合/导出层构造映射)。
pub fn fetch_todo_titles(conn: &Connection, todo_ids: &[String]) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let ids: Vec<&String> = todo_ids
        .iter()
        .filter(|i| !i.is_empty() && seen.insert(*i))
        .collect();
    if ids.is_empty() {
        return Vec::new();
    }
    let placeholders: Vec<&str> = (0..ids.len()).map(|_| "?").collect();
    let sql = format!(
        "SELECT id, title FROM todos WHERE id IN ({})",
        placeholders.join(",")
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map(params_from_iter(ids.iter().copied()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|r| r.ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;
    use chrono::TimeZone;

    fn make() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        run_migrations(&mut conn).unwrap();
        conn
    }

    fn open_input(p: &str, t: &str) -> OpenSegmentInput {
        OpenSegmentInput {
            process_name: p.into(),
            title: t.into(),
            kind: None,
            todo_id: None,
            start_at: None,
        }
    }

    // 翻译自 segments.test.ts 的 2 条不依赖 notifier 的用例;其余 5 条(startTracking/
    // stopTracking/错误隔离)在 3c 连同 HookNotifier 抽象补齐。

    #[test]
    fn open_segment_inserts_an_open_activity_row() {
        let conn = make();
        let s = open_segment(&conn, &open_input("app.exe", "Main")).unwrap();
        assert!(s.end_at.is_none());
        assert_eq!(s.process_name, "app.exe");
        assert_eq!(s.title, "Main");
        assert_eq!(s.kind, SegmentKind::Activity);
        assert!(Uuid::parse_str(&s.id).is_ok());
    }

    #[test]
    fn close_open_sets_endat_and_is_idempotent() {
        let conn = make();
        open_segment(&conn, &open_input("a", "t")).unwrap();
        let at = "2026-08-03T10:30:00Z";
        assert_eq!(close_open(&conn, at), 1);
        assert_eq!(close_open(&conn, at), 0);
    }

    #[test]
    fn list_segments_by_day_clips_and_orders() {
        let conn = make();
        // 全在内的当日段。
        open_segment(
            &conn,
            &OpenSegmentInput {
                process_name: "a".into(),
                title: "in".into(),
                kind: None,
                todo_id: None,
                start_at: Some("2026-08-03T10:00:00".into()),
            },
        )
        .unwrap();
        close_open(&conn, "2026-08-03T11:00:00");
        // 不同日。
        open_segment(
            &conn,
            &OpenSegmentInput {
                process_name: "b".into(),
                title: "out".into(),
                kind: None,
                todo_id: None,
                start_at: Some("2026-08-05T10:00:00".into()),
            },
        )
        .unwrap();
        close_open(&conn, "2026-08-05T11:00:00");
        let now = chrono::Local
            .with_ymd_and_hms(2026, 8, 3, 23, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let day = list_segments_by_day(&conn, "2026-08-03", now);
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].process_name, "a");
        assert!(day[0].end_at.is_some());
    }

    #[test]
    fn update_segment_note_and_todo_id_three_states() {
        let conn = make();
        // 先插入被引用的 todo,满足 FK(todoId REFERENCES todos)(foreign_keys ON)。
        conn.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
             VALUES (?1, 'todo1', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', NULL, NULL)",
            params!["todo1"],
        )
        .unwrap();
        let s = open_segment(&conn, &open_input("a", "t")).unwrap();
        // 设值。
        let updated = update_segment(
            &conn,
            &s.id,
            &SegmentPatch {
                note: Some("hi".into()),
                todo_id: Some(Some("todo1".into())),
            },
        );
        let updated = updated.expect("seg exists after update");
        assert_eq!(updated.note, "hi");
        assert_eq!(updated.todo_id, Some("todo1".to_string()));
        // 不变:patch 全 None -> 保留 note/todoId。
        let same = update_segment(
            &conn,
            &s.id,
            &SegmentPatch {
                note: None,
                todo_id: None,
            },
        )
        .unwrap();
        assert_eq!(same.note, "hi");
        assert_eq!(same.todo_id, Some("todo1".to_string()));
        // 置空:Some(None)。
        let cleared = update_segment(
            &conn,
            &s.id,
            &SegmentPatch {
                note: None,
                todo_id: Some(None),
            },
        )
        .unwrap();
        assert!(cleared.todo_id.is_none());
        assert_eq!(cleared.note, "hi");
    }

    #[test]
    fn update_segment_unknown_id_returns_none() {
        let conn = make();
        let patch = SegmentPatch {
            note: None,
            todo_id: None,
        };
        assert!(update_segment(&conn, "no-such-id", &patch).is_none());
    }

    #[test]
    fn segment_kind_serde_is_lowercase() {
        let s = Segment {
            id: "x".into(),
            start_at: "2026-08-03T10:00:00.000Z".into(),
            end_at: None,
            process_name: "p".into(),
            title: String::new(),
            note: String::new(),
            todo_id: None,
            kind: SegmentKind::Idle,
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("idle"));
        assert_eq!(json.get("startAt").and_then(|v| v.as_str()), Some("2026-08-03T10:00:00.000Z"));
        assert!(json.get("todoId").map(|v| v.is_null()).unwrap_or(true));
    }

    #[test]
    fn segment_patch_deserializes_three_states() {
        let only_note: SegmentPatch = serde_json::from_str(r#"{"note":"x"}"#).unwrap();
        assert!(only_note.todo_id.is_none());
        let note_and_null: SegmentPatch = serde_json::from_str(r#"{"note":"x","todoId":null}"#).unwrap();
        assert_eq!(note_and_null.todo_id, Some(None));
        let note_and_val: SegmentPatch = serde_json::from_str(r#"{"note":"x","todoId":"t"}"#).unwrap();
        assert_eq!(note_and_val.todo_id, Some(Some("t".into())));
        // 空串按 TS 语义归为置空(置 NULL)。
        let empty_str: SegmentPatch = serde_json::from_str(r#"{"todoId":""}"#).unwrap();
        assert_eq!(empty_str.todo_id, Some(None));
    }
}