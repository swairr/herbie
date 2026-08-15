//! segments 仓储,逐行翻译 `src/main/segments.ts` + `segments-row.ts` + `segments-query.ts`。
//!
//! 写/读路径:**`open_segment` / `close_open` / `list_all` / `list_segments_by_day` /
//! `update_segment` / `fetch_todo_titles`**。本片(3c-A)补 **`start_tracking` /
//! `stop_tracking`**(前台事件 → 段开关,翻译 segments.test.ts 全部 7 条);
//! 真实原生 SetWinEventHook 接入留 3c-B。
//!
//! 仓储函数取 `&Connection`(不自行锁全局、不需 `&mut`——segments 无事务);命令层在
//! `lib.rs` 锁 `db::get()` 后借 `&Connection` 传入,与 settings/todos 模式一致。
//! `start_tracking` 的回调持 `Arc<C: ConnAccess>`(prod 全局 / test 局部连接均可用)。
//!
//! `Segment`/`SegmentKind`/`SegmentPatch` serde 对齐 TS `src/shared/types.ts`:
//! camelCase 字段名;`kind` ∈ "activity"/"idle";`SegmentPatch.todo_id` 三态见字段注释。

use std::sync::Arc;

use chrono::Utc;
use rusqlite::{params, params_from_iter, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::hook::{HookNotifier, WinHookEvent};
use crate::settings;
use crate::time::{day_bounds, parse_iso_to_ms, split_at_midnight};
use crate::todos::now_iso;
use crate::tracker::ConnAccess;

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

/// 读当前开放段(`endAt IS NULL`)。模型保证至多一条;防御性取 `startAt` 最早的一条。
pub fn get_open_segment(conn: &Connection) -> rusqlite::Result<Option<Segment>> {
    let mut stmt =
        conn.prepare("SELECT * FROM segments WHERE endAt IS NULL ORDER BY startAt ASC LIMIT 1")?;
    let mut rows = stmt.query_map([], row_to_segment)?;
    match rows.next() {
        Some(r) => r.map(Some),
        None => Ok(None),
    }
}

/// 把当前开放段的 `processName`/`title` 改写为新窗口,保持 `startAt`/`kind`/`note`/
/// `todoId` 不变。用于「最小片段时长合并」:短于防抖时间的片段不单独记录,并入下一个窗口。
pub fn replace_open_process_title(
    conn: &Connection,
    process_name: &str,
    title: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE segments SET processName = ?1, title = ?2 WHERE endAt IS NULL",
        params![process_name, title],
    )?;
    Ok(())
}

/// 判断当前开放 activity 段在 `now` 时刻是否短于 `threshold_sec` 而应被合并。
/// - `threshold_sec == 0`(未配置/无效)→ 不合并;
/// - 开放段非 activity(如 idle)或 startAt/now 无法解析 → 不合并(保守,正常关旧开新)。
pub fn should_merge_open(conn: &Connection, now: &str, threshold_sec: u64) -> bool {
    if threshold_sec == 0 {
        return false;
    }
    let seg = match get_open_segment(conn) {
        Ok(Some(s)) => s,
        _ => return false,
    };
    if seg.kind != SegmentKind::Activity {
        return false;
    }
    match (parse_iso_to_ms(&seg.start_at), parse_iso_to_ms(now)) {
        (Some(start_ms), Some(now_ms)) => {
            let elapsed_ms = now_ms.saturating_sub(start_ms).max(0) as u64;
            elapsed_ms < threshold_sec.saturating_mul(1000)
        }
        _ => false,
    }
}

/// `listAllSegments()`:`SELECT * ORDER BY startAt ASC`。TS 镜像读 API,由测试覆盖;
/// 生产 renderer 走按日查询(`segments_list`),故仅测试使用。
#[allow(dead_code)]
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

/// `startTracking(notifier)`:注册回调把前台/改题事件翻译为段开关。
///
/// dedup 状态(`current_hwnd` / `open_process` / `open_title`)寄生于闭包捕获,跨事件保持;
/// DB 经 `ConnAccess` 锁取(测试传 `Arc<LocalConn>`)。整块错误吞掉仅 `eprintln!`(原生回调
/// 边界不得让业务错误穿透),且仅当两条写库全部成功才推进 dedup 状态(等价 TS 的 try/catch)。
pub fn start_tracking<N: HookNotifier, C: ConnAccess + 'static>(notifier: N, conn: Arc<C>) -> N {
    let mut notifier = notifier;
    let mut current_hwnd: i64 = -1;
    let mut open_process = String::new();
    let mut open_title = String::new();
    notifier.start(Box::new(move |e: WinHookEvent| {
        // NAMECHANGE 仅对当前前台窗口有意义;迟到的旧窗口事件忽略。
        if e.kind == "namechange" && current_hwnd != e.hwnd {
            return;
        }
        // 去重:重复的 (processName, title) 不再 close+reopen,避免写放大。
        if e.kind == "namechange"
            && e.process_name == open_process
            && (e.title == open_title || e.title.is_empty())
        {
            return;
        }
        let written: Result<(), String> = conn.lock_conn(|c| {
            let now = now_iso();
            let threshold = settings::get_with_default(c, settings::KEY_MIN_SEGMENT_SEC)
                .parse::<u64>()
                .ok()
                .filter(|&n| n > 0)
                .unwrap_or(0);
            if should_merge_open(c, &now, threshold) {
                replace_open_process_title(c, &e.process_name, &e.title)
                    .map_err(|err| err.to_string())?;
            } else {
                let _ = close_open(c, &now);
                open_segment(
                    c,
                    &OpenSegmentInput {
                        process_name: e.process_name.clone(),
                        title: e.title.clone(),
                        kind: None,
                        todo_id: None,
                        start_at: None,
                    },
                )
                .map_err(|err| err.to_string())?;
            }
            Ok(())
        });
        match written {
            Ok(()) => {
                current_hwnd = e.hwnd;
                open_process = e.process_name.clone();
                open_title = e.title.clone();
            }
            Err(err) => eprintln!("[segments] failed to process native window event: {err}"),
        }
    }));
    notifier
}

/// `stopTracking(notifier, at)`:先关当前开放段(幂等),再停掉通知器。
pub fn stop_tracking<N: HookNotifier, C: ConnAccess>(mut notifier: N, conn: &C, at: &str) {
    conn.lock_conn(|c| {
        let _ = close_open(c, at);
    });
    notifier.stop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;
    use crate::tracker::LocalConn;
    use chrono::TimeZone;
    use std::sync::{Arc, Mutex};

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
    // stopTracking/错误隔离)在下方随 HookNotifier 抽象补齐。

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

    fn open_at(p: &str, t: &str, start: &str) -> OpenSegmentInput {
        OpenSegmentInput {
            process_name: p.into(),
            title: t.into(),
            kind: None,
            todo_id: None,
            start_at: Some(start.into()),
        }
    }

    #[test]
    fn get_open_segment_returns_none_then_some() {
        let conn = make();
        assert!(get_open_segment(&conn).unwrap().is_none());
        open_segment(&conn, &open_at("a", "t", "2026-08-03T10:00:00Z")).unwrap();
        let open = get_open_segment(&conn).unwrap().unwrap();
        assert_eq!(open.process_name, "a");
        assert!(open.end_at.is_none());
    }

    #[test]
    fn replace_open_process_title_keeps_start_at_and_kind() {
        let conn = make();
        let s = open_segment(&conn, &open_at("a", "A", "2026-08-03T10:00:00Z")).unwrap();
        replace_open_process_title(&conn, "b.exe", "B").unwrap();
        let open = get_open_segment(&conn).unwrap().unwrap();
        assert_eq!(open.id, s.id);
        assert_eq!(open.start_at, "2026-08-03T10:00:00Z");
        assert_eq!(open.process_name, "b.exe");
        assert_eq!(open.title, "B");
        assert_eq!(open.kind, SegmentKind::Activity);
        assert!(open.end_at.is_none());
    }

    #[test]
    fn should_merge_open_merges_short_activity_open() {
        let conn = make();
        open_segment(&conn, &open_at("a", "A", "2026-08-03T10:00:00Z")).unwrap();
        assert!(should_merge_open(&conn, "2026-08-03T10:00:30Z", 60));
    }

    #[test]
    fn should_merge_open_does_not_merge_at_or_beyond_threshold() {
        let conn = make();
        open_segment(&conn, &open_at("a", "A", "2026-08-03T10:00:00Z")).unwrap();
        assert!(!should_merge_open(&conn, "2026-08-03T10:01:00Z", 60));
        assert!(!should_merge_open(&conn, "2026-08-03T10:02:00Z", 60));
    }

    #[test]
    fn should_merge_open_does_not_merge_idle_open() {
        let conn = make();
        open_segment(
            &conn,
            &OpenSegmentInput {
                process_name: "[idle]".into(),
                title: String::new(),
                kind: Some(SegmentKind::Idle),
                todo_id: None,
                start_at: Some("2026-08-03T10:00:00Z".into()),
            },
        )
        .unwrap();
        assert!(!should_merge_open(&conn, "2026-08-03T10:00:30Z", 60));
    }

    #[test]
    fn should_merge_open_false_when_no_open_or_zero_threshold() {
        let conn = make();
        assert!(!should_merge_open(&conn, "2026-08-03T10:00:30Z", 60));
        open_segment(&conn, &open_at("a", "A", "2026-08-03T10:00:00Z")).unwrap();
        assert!(!should_merge_open(&conn, "2026-08-03T10:00:30Z", 0));
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

    // 等价 TS `fakeNotifier`:计数 starts/stops + 存 cb 供外部 emit。cb 存
    // `Arc<Mutex<Option<...>>>`,clone 出的 emit 句柄共享同一存储(对应 JS 对象引用语义)。
    #[derive(Clone)]
    struct FakeHook {
        cb: Arc<Mutex<Option<Box<dyn FnMut(WinHookEvent) + Send + 'static>>>>,
        starts: Arc<Mutex<i64>>,
        stops: Arc<Mutex<i64>>,
    }
    impl FakeHook {
        fn new() -> Self {
            Self {
                cb: Arc::new(Mutex::new(None)),
                starts: Arc::new(Mutex::new(0)),
                stops: Arc::new(Mutex::new(0)),
            }
        }
        fn starts(&self) -> i64 {
            *self.starts.lock().unwrap()
        }
        fn stops(&self) -> i64 {
            *self.stops.lock().unwrap()
        }
        fn emit(&mut self, e: WinHookEvent) {
            let mut g = self.cb.lock().unwrap();
            if let Some(cb) = g.as_mut() {
                cb(e);
            }
        }
    }
    impl HookNotifier for FakeHook {
        fn start(&mut self, cb: Box<dyn FnMut(WinHookEvent) + Send + 'static>) {
            *self.starts.lock().unwrap() += 1;
            *self.cb.lock().unwrap() = Some(cb);
        }
        fn stop(&mut self) {
            *self.stops.lock().unwrap() += 1;
            *self.cb.lock().unwrap() = None;
        }
    }

    fn foreground(hwnd: i64, process: &str, title: &str) -> WinHookEvent {
        WinHookEvent {
            kind: "foreground",
            hwnd,
            process_name: process.into(),
            title: title.into(),
        }
    }

    fn name_change(hwnd: i64, process: &str, title: &str) -> WinHookEvent {
        WinHookEvent {
            kind: "namechange",
            hwnd,
            process_name: process.into(),
            title: title.into(),
        }
    }

    // 与 tracker 测试同款局部连接:(Arc<Mutex<Connection>>, Arc<LocalConn>),fk + 迁移,
    // 各测试独立持有避免并行竞态。
    fn make_local() -> (Arc<Mutex<Connection>>, Arc<LocalConn>) {
        let mut conn = Connection::open_in_memory().unwrap();
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        run_migrations(&mut conn).unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let ca = Arc::new(LocalConn::new(conn.clone()));
        (conn, ca)
    }

    fn with_conn<F, R>(conn: &Arc<Mutex<Connection>>, f: F) -> R
    where
        F: FnOnce(&Connection) -> R,
    {
        f(&*conn.lock().unwrap())
    }

    // 翻译 segments.test.ts 的 5 条依赖 notifier 的用例。

    #[test]
    fn start_tracking_opens_new_and_closes_previous_on_each_foreground() {
        let (conn, ca) = make_local();
        // 关闭合并(阈值 0),验证原始「逐事件关旧开新」语义。
        with_conn(&conn, |c| settings::set(c, settings::KEY_MIN_SEGMENT_SEC, "0").unwrap());
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let hook = start_tracking(hook, ca.clone());
        assert_eq!(hook.starts(), 1);

        emit.emit(foreground(1, "a.exe", "A"));
        emit.emit(foreground(2, "b.exe", "B"));

        let rows = with_conn(&conn, |c| list_all(c).unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].process_name, "a.exe");
        assert!(rows[0].end_at.is_some());
        assert_eq!(rows[1].process_name, "b.exe");
        assert!(rows[1].end_at.is_none());
    }

    #[test]
    fn namechange_for_current_hwnd_updates_title_via_close_open() {
        let (conn, ca) = make_local();
        with_conn(&conn, |c| settings::set(c, settings::KEY_MIN_SEGMENT_SEC, "0").unwrap());
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let _hook = start_tracking(hook, ca.clone());

        emit.emit(foreground(5, "c.exe", "Doc1"));
        emit.emit(name_change(5, "c.exe", "Doc2"));

        let rows = with_conn(&conn, |c| list_all(c).unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "Doc1");
        assert_eq!(rows[1].title, "Doc2");
        assert!(rows[0].end_at.is_some());
        assert!(rows[1].end_at.is_none());
    }

    #[test]
    fn namechange_for_non_foreground_hwnd_is_ignored() {
        let (conn, ca) = make_local();
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let _hook = start_tracking(hook, ca.clone());

        emit.emit(foreground(7, "c.exe", "X"));
        emit.emit(name_change(99, "other.exe", "Y"));

        let rows = with_conn(&conn, |c| list_all(c).unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "X");
        assert!(rows[0].end_at.is_none());
    }

    #[test]
    fn foreground_events_within_debounce_merge_into_one_segment() {
        let (conn, ca) = make_local();
        // 阈值设大,使紧邻的两次前台切换必然短于阈值 → 合并为一段。
        with_conn(&conn, |c| settings::set(c, settings::KEY_MIN_SEGMENT_SEC, "3600").unwrap());
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let _hook = start_tracking(hook, ca.clone());

        emit.emit(foreground(1, "a.exe", "A"));
        emit.emit(foreground(2, "b.exe", "B"));

        let rows = with_conn(&conn, |c| list_all(c).unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].process_name, "b.exe");
        assert_eq!(rows[0].title, "B");
        assert!(rows[0].end_at.is_none());
    }

    #[test]
    fn foreground_does_not_merge_an_open_idle_segment() {
        let (conn, ca) = make_local();
        with_conn(&conn, |c| {
            settings::set(c, settings::KEY_MIN_SEGMENT_SEC, "3600").unwrap();
            // 预开一个 idle 段(模拟 tracker 进入空闲),idle 段绝不应被吞并。
            open_segment(
                c,
                &OpenSegmentInput {
                    process_name: "[idle]".into(),
                    title: String::new(),
                    kind: Some(SegmentKind::Idle),
                    todo_id: None,
                    start_at: Some("2026-08-03T10:00:00Z".into()),
                },
            )
            .unwrap();
        });
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let _hook = start_tracking(hook, ca.clone());

        emit.emit(foreground(2, "b.exe", "B"));

        let rows = with_conn(&conn, |c| list_all(c).unwrap());
        assert_eq!(rows.len(), 2);
        let idle = rows.iter().find(|r| r.kind == SegmentKind::Idle).unwrap();
        assert!(idle.end_at.is_some());
        let act = rows.iter().find(|r| r.kind == SegmentKind::Activity).unwrap();
        assert_eq!(act.process_name, "b.exe");
        assert!(act.end_at.is_none());
    }

    #[test]
    fn stop_tracking_closes_open_and_stops_the_notifier() {
        let (conn, ca) = make_local();
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let hook = start_tracking(hook, ca.clone());
        emit.emit(foreground(1, "a.exe", "A"));

        let check = hook.clone();
        stop_tracking(hook, &*ca, "2026-08-03T12:00:00Z");
        assert_eq!(check.stops(), 1);

        let rows = with_conn(&conn, |c| list_all(c).unwrap());
        assert_eq!(rows[0].end_at.as_deref(), Some("2026-08-03T12:00:00Z"));
    }

    #[test]
    fn isolates_event_handler_errors_from_the_native_callback_boundary() {
        // 未迁移的局部连接:无 segments 表 → open_segment INSERT 报错,cb 吞错不 panic。
        let raw = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(raw));
        let ca = Arc::new(LocalConn::new(conn));
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let _hook = start_tracking(hook, ca.clone());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            emit.emit(foreground(2, "b.exe", "B"));
        }));
        assert!(result.is_ok());
    }
}