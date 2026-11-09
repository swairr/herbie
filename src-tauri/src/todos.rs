//! todos 仓储,逐行翻译自 `src/main/todos.ts`。
//!
//! 模型与命令入参用 `serde`;返回 `Todo` 序列化为 **camelCase**(`createdAt`/`updatedAt`/...)以与
//! TS `Api.Todo` 形状一致;SQL 列名为 camelCase(见 `migrations/0001.sql`),`row_to_todo` 按
//! 列名读出后映射到结构体 snake_case 字段。
//!
//! 仓储层取 `&Connection`/`&mut Connection` 参数,不自行锁全局(对齐切片1 Settings 模式)。
//! `create_todo`/`update_todo` 在单个事务内写入 todos + todo_labels,保证与标签重解析原子。

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::normalize_time_of_day;

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
    pub sort_order: f64,
    pub planned_date: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
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

/// 计划时段入参(里程碑四)。三个字段都使用「null / 空串 = 清除」语义:
/// 日期为空时起止时刻必须也为空;有日期时可只给日期(全天/未定时间)。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoSchedule {
    pub planned_date: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

/// 等价 JS `new Date().toISOString()`:毫秒精度、`Z` 后缀(非 `+00:00`),与既有数据格式一致。
pub fn now_iso() -> String {
    crate::time::iso_utc_z_millis(Utc::now())
}

const TODO_COLUMNS: &str =
    "id, title, detail, createdAt, updatedAt, completedAt, deletedAt, sortOrder, \
     plannedDate, plannedStart, plannedEnd";

fn row_to_todo(r: &rusqlite::Row) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: r.get("id")?,
        title: r.get("title")?,
        detail: r.get("detail")?,
        created_at: r.get("createdAt")?,
        updated_at: r.get("updatedAt")?,
        completed_at: r.get("completedAt")?,
        deleted_at: r.get("deletedAt")?,
        sort_order: r.get("sortOrder")?,
        planned_date: r.get("plannedDate")?,
        planned_start: r.get("plannedStart")?,
        planned_end: r.get("plannedEnd")?,
    })
}

fn fetch_todo(conn: &Connection, id: &str) -> rusqlite::Result<Todo> {
    conn.query_row(
        &format!("SELECT {} FROM todos WHERE id = ?1", TODO_COLUMNS),
        params![id],
        row_to_todo,
    )
}

/// `listTodos(filter?)`:`SELECT * FROM todos WHERE deletedAt IS NULL`
/// [+ `AND id IN (SELECT todoId FROM todo_labels WHERE label IN (?,...))]`
/// + `ORDER BY (completedAt IS NULL) DESC,
///             CASE WHEN completedAt IS NULL THEN sortOrder END ASC,
///             completedAt DESC, createdAt DESC`。
/// pending 按手动排序 `sortOrder` 升序(拖拽排序,迁移 0004);done 仍按 completedAt 降序。
pub fn list_todos(
    conn: &Connection,
    filter: Option<&TodoFilter>,
) -> rusqlite::Result<Vec<Todo>> {
    let mut sql = format!("SELECT {} FROM todos WHERE deletedAt IS NULL", TODO_COLUMNS);
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
    sql.push_str(
        " ORDER BY (completedAt IS NULL) DESC, \
         CASE WHEN completedAt IS NULL THEN sortOrder END ASC, \
         completedAt DESC, createdAt DESC",
    );

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
/// `sortOrder` = 当前 pending 最小值 − 1(新待办置顶,与迁移前 createdAt DESC 头插一致);空列表取 1。
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
        let next_order: f64 = tx
            .query_row(
                "SELECT MIN(sortOrder) FROM todos WHERE deletedAt IS NULL AND completedAt IS NULL",
                [],
                |r| r.get::<_, Option<f64>>(0),
            )?
            .map(|min| min - 1.0)
            .unwrap_or(1.0);
        tx.execute(
            "INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt, sortOrder)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6)",
            params![id, title, detail, &now, &now, next_order],
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

/// 相邻 sortOrder 间隙低于该阈值时整体重排为 1..n(防反复取中点耗尽浮点精度)。
const MIN_GAP: f64 = 1e-6;

/// pending 的当前展示顺序(id + sortOrder)。直接复用 `list_todos` 的结果作为唯一事实来源,
/// 避免与展示排序出现两份实现漂移(拖拽落点依赖此顺序)。
fn pending_order(conn: &Connection) -> rusqlite::Result<Vec<(String, f64)>> {
    Ok(list_todos(conn, None)?
        .into_iter()
        .filter(|t| t.completed_at.is_none())
        .map(|t| (t.id, t.sort_order))
        .collect())
}

/// `moveTodo(id, beforeId)`:把 pending 待办 `id` 移到 `before` 之前(`before`=None 移到末尾),
/// 新 sortOrder 取相邻中点、首前/末后取 ±1;相邻间隙低于阈值时同事务内先整体重排 1..n。
/// `id`/`before` 不存在或非 pending 报 `todo not found: <id>`;`before == id` 为无操作。
pub fn move_todo(conn: &mut Connection, id: &str, before: Option<&str>) -> Result<Todo, String> {
    if let Some(b) = before {
        if b == id {
            return fetch_todo(conn, id).map_err(|e| e.to_string());
        }
    }
    let mut order = pending_order(conn).map_err(|e| e.to_string())?;
    if !order.iter().any(|(rid, _)| rid == id) {
        return Err(format!("todo not found: {}", id));
    }
    if let Some(b) = before {
        if !order.iter().any(|(rid, _)| rid == b) {
            return Err(format!("todo not found: {}", b));
        }
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if order.windows(2).any(|w| w[1].1 - w[0].1 < MIN_GAP) {
        for (i, (rid, _)) in order.iter().enumerate() {
            tx.execute(
                "UPDATE todos SET sortOrder = ?1 WHERE id = ?2",
                params![(i + 1) as f64, rid],
            )
            .map_err(|e| e.to_string())?;
        }
        for (i, item) in order.iter_mut().enumerate() {
            item.1 = (i + 1) as f64;
        }
    }
    let others: Vec<&(String, f64)> = order.iter().filter(|(rid, _)| rid != id).collect();
    let new_order = match before {
        None => others.last().map(|(_, o)| o + 1.0).unwrap_or(1.0),
        Some(b) => {
            let i = others
                .iter()
                .position(|(rid, _)| rid == b)
                .expect("presence checked above");
            let hi = others[i].1;
            if i == 0 {
                hi - 1.0
            } else {
                (others[i - 1].1 + hi) / 2.0
            }
        }
    };
    tx.execute(
        "UPDATE todos SET sortOrder = ?1 WHERE id = ?2",
        params![new_order, id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    fetch_todo(conn, id).map_err(|e| e.to_string())
}

fn minutes_of_day(raw: &str) -> u32 {
    let (h, m) = raw.split_once(':').unwrap_or(("0", "0"));
    h.parse::<u32>().unwrap_or(0) * 60 + m.parse::<u32>().unwrap_or(0)
}

/// 空串与 None 等价(清除)。
fn clean_optional(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// 校验并规范化为零填充的本地自然日 `YYYY-MM-DD`。
/// 复用 `day_bounds` 的溢出规范化(如 `2026-02-30` → `2026-03-02`),保证
/// `plannedDate` 的字典序比较与范围查询可靠。
fn normalize_planned_day(raw: &str) -> Option<String> {
    let bounds = crate::time::day_bounds(raw)?;
    let dt = chrono::DateTime::<Utc>::from_timestamp_millis(bounds.start_ms)?;
    Some(crate::time::local_date_string(dt))
}

/// `scheduleTodo(id, slot)`:校验并整体替换计划时段;传空即清除。
///
/// 规则(里程碑四):
/// - `plannedDate` 为空 → `plannedStart` / `plannedEnd` 必须也为空;
/// - 有日期、无开始 → 结束必须也为空(全天/未定时间);
/// - 有开始 → 结束必须存在且 `start < end`,不跨天;
/// - 日期用 `day_bounds` 校验,时刻规范化为 `HH:MM`;
/// - 成功后更新 `updatedAt` 并回读。
pub fn schedule_todo(
    conn: &Connection,
    id: &str,
    slot: &TodoSchedule,
) -> Result<Todo, String> {
    fetch_todo(conn, id).map_err(|_| format!("todo not found: {}", id))?;

    let date = match clean_optional(&slot.planned_date) {
        Some(d) => Some(
            normalize_planned_day(&d).ok_or_else(|| format!("invalid plannedDate: {}", d))?,
        ),
        None => None,
    };

    let start = match clean_optional(&slot.planned_start) {
        Some(s) => Some(
            normalize_time_of_day(&s).ok_or_else(|| format!("invalid plannedStart: {}", s))?,
        ),
        None => None,
    };
    let end = match clean_optional(&slot.planned_end) {
        Some(s) => Some(
            normalize_time_of_day(&s).ok_or_else(|| format!("invalid plannedEnd: {}", s))?,
        ),
        None => None,
    };

    if date.is_none() && (start.is_some() || end.is_some()) {
        return Err("plannedDate is required when a time range is set".into());
    }
    match (&start, &end) {
        (None, Some(_)) => return Err("plannedStart is required when plannedEnd is set".into()),
        (Some(_), None) => return Err("plannedEnd is required when plannedStart is set".into()),
        (Some(s), Some(e)) if minutes_of_day(s) >= minutes_of_day(e) => {
            return Err("plannedStart must be before plannedEnd".into())
        }
        _ => {}
    }

    let now = now_iso();
    conn.execute(
        "UPDATE todos SET plannedDate = ?1, plannedStart = ?2, plannedEnd = ?3, updatedAt = ?4
         WHERE id = ?5",
        params![date, start, end, now, id],
    )
    .map_err(|e| e.to_string())?;
    fetch_todo(conn, id).map_err(|e| e.to_string())
}

/// 取本地日期范围 `[from, to]`(含端点)内已安排的待办,含已完成;按日期 →
/// 「无具体时间在前 → 开始时刻 → 手动 sortOrder」排序。
pub fn list_scheduled_todos(
    conn: &Connection,
    from: &str,
    to: &str,
) -> Result<Vec<Todo>, String> {
    if crate::time::day_bounds(from).is_none() {
        return Err(format!("invalid day: {}", from));
    }
    if crate::time::day_bounds(to).is_none() {
        return Err(format!("invalid day: {}", to));
    }
    let from_day = normalize_planned_day(from).expect("validated above");
    let to_day = normalize_planned_day(to).expect("validated above");
    let sql = format!(
        "SELECT {} FROM todos
         WHERE deletedAt IS NULL AND plannedDate IS NOT NULL AND plannedDate >= ?1 AND plannedDate <= ?2
         ORDER BY plannedDate ASC,
                  CASE WHEN plannedStart IS NULL THEN 0 ELSE 1 END ASC,
                  plannedStart ASC,
                  sortOrder ASC",
        TODO_COLUMNS
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![from_day, to_day], row_to_todo)
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

/// 取尚未安排计划的 pending 待办(未完成、未删除、`plannedDate IS NULL`),
/// 沿用待办列表的手动 `sortOrder` 升序,供日程页「待安排」使用。
pub fn list_unscheduled_todos(conn: &Connection) -> rusqlite::Result<Vec<Todo>> {
    let sql = format!(
        "SELECT {} FROM todos
         WHERE deletedAt IS NULL AND completedAt IS NULL AND plannedDate IS NULL
         ORDER BY sortOrder ASC",
        TODO_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_todo)?;
    rows.collect()
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

    fn pending_ids(conn: &Connection) -> Vec<String> {
        list_todos(conn, None)
            .unwrap()
            .into_iter()
            .filter(|t| t.completed_at.is_none())
            .map(|t| t.id)
            .collect()
    }

    fn make_abc(conn: &mut Connection) -> (Todo, Todo, Todo) {
        let a = create_todo(conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        wait();
        let b = create_todo(conn, &TodoInput { title: "b".into(), detail: String::new() }).unwrap();
        wait();
        let c = create_todo(conn, &TodoInput { title: "c".into(), detail: String::new() }).unwrap();
        (a, b, c)
    }

    #[test]
    fn new_todo_goes_to_top_and_order_is_stable() {
        let mut conn = make();
        let (a, b, c) = make_abc(&mut conn);
        assert_eq!(pending_ids(&conn), vec![c.id.clone(), b.id.clone(), a.id.clone()]);
    }

    #[test]
    fn move_todo_before_middle_item() {
        let mut conn = make();
        let (a, b, c) = make_abc(&mut conn);
        // [c, b, a] → 把 c 移到 a 之前 → [b, c, a]
        move_todo(&mut conn, &c.id, Some(&a.id)).unwrap();
        assert_eq!(pending_ids(&conn), vec![b.id.clone(), c.id.clone(), a.id.clone()]);
    }

    #[test]
    fn move_todo_to_top_and_to_end() {
        let mut conn = make();
        let (a, b, c) = make_abc(&mut conn);
        // [c, b, a] → a 置顶 → [a, c, b]
        move_todo(&mut conn, &a.id, Some(&c.id)).unwrap();
        assert_eq!(pending_ids(&conn), vec![a.id.clone(), c.id.clone(), b.id.clone()]);
        // a 移到末尾 → [c, b, a]
        move_todo(&mut conn, &a.id, None).unwrap();
        assert_eq!(pending_ids(&conn), vec![c.id.clone(), b.id.clone(), a.id.clone()]);
    }

    #[test]
    fn move_todo_noop_when_before_is_self() {
        let mut conn = make();
        let (a, b, c) = make_abc(&mut conn);
        move_todo(&mut conn, &b.id, Some(&b.id)).unwrap();
        assert_eq!(pending_ids(&conn), vec![c.id, b.id, a.id]);
    }

    #[test]
    fn move_todo_errors_on_unknown_or_done_todos() {
        let mut conn = make();
        let (a, _b, c) = make_abc(&mut conn);
        assert!(move_todo(&mut conn, "nope", Some(&a.id)).is_err());
        assert!(move_todo(&mut conn, &a.id, Some("nope")).is_err());
        toggle_todo(&conn, &c.id, true).unwrap();
        assert!(move_todo(&mut conn, &c.id, Some(&a.id)).is_err());
        assert!(move_todo(&mut conn, &a.id, Some(&c.id)).is_err());
    }

    #[test]
    fn move_todo_renormalizes_when_gaps_are_exhausted() {
        let mut conn = make();
        let (a, b, c) = make_abc(&mut conn);
        // 人为制造零间隙:[c=1.0, b=1.0, a=2.0]
        conn.execute("UPDATE todos SET sortOrder = 1.0 WHERE id = ?1", params![c.id]).unwrap();
        conn.execute("UPDATE todos SET sortOrder = 1.0 WHERE id = ?1", params![b.id]).unwrap();
        conn.execute("UPDATE todos SET sortOrder = 2.0 WHERE id = ?1", params![a.id]).unwrap();
        // 触发重排后仍按展示序(createdAt DESC: [c, b, a])归一为 1..3,移动 a 到 c 前生效
        move_todo(&mut conn, &a.id, Some(&c.id)).unwrap();
        assert_eq!(pending_ids(&conn), vec![a.id.clone(), c.id.clone(), b.id.clone()]);
        let orders: Vec<f64> = list_todos(&conn, None)
            .unwrap()
            .into_iter()
            .filter(|t| t.completed_at.is_none())
            .map(|t| t.sort_order)
            .collect();
        assert!(orders.windows(2).all(|w| w[1] - w[0] >= 1e-6), "gaps restored: {:?}", orders);
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

    fn slot(
        date: Option<&str>,
        start: Option<&str>,
        end: Option<&str>,
    ) -> TodoSchedule {
        TodoSchedule {
            planned_date: date.map(str::to_string),
            planned_start: start.map(str::to_string),
            planned_end: end.map(str::to_string),
        }
    }

    #[test]
    fn schedules_a_todo_with_a_time_range_and_returns_it_from_list_todos() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        let scheduled = schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), Some("9:00"), Some("10:30"))).unwrap();
        assert_eq!(scheduled.planned_date.as_deref(), Some("2026-11-02"));
        assert_eq!(scheduled.planned_start.as_deref(), Some("09:00"));
        assert_eq!(scheduled.planned_end.as_deref(), Some("10:30"));

        let from_list = list_todos(&conn, None).unwrap();
        let stored = from_list.iter().find(|x| x.id == t.id).unwrap();
        assert_eq!(stored.planned_start.as_deref(), Some("09:00"));
    }

    #[test]
    fn schedules_a_todo_with_date_only_and_clears_it() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        let scheduled = schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), None, None)).unwrap();
        assert_eq!(scheduled.planned_date.as_deref(), Some("2026-11-02"));
        assert!(scheduled.planned_start.is_none());
        assert!(scheduled.planned_end.is_none());

        let cleared = schedule_todo(&conn, &t.id, &slot(None, None, None)).unwrap();
        assert!(cleared.planned_date.is_none());
        assert!(cleared.planned_start.is_none());
        assert!(cleared.planned_end.is_none());
    }

    #[test]
    fn normalizes_unpadded_planned_dates() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        let scheduled = schedule_todo(&conn, &t.id, &slot(Some("2026-2-3"), None, None)).unwrap();
        assert_eq!(scheduled.planned_date.as_deref(), Some("2026-02-03"));
        let got = list_scheduled_todos(&conn, "2026-2-3", "2026-2-3").unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn rejects_invalid_schedule_combinations() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();

        assert!(schedule_todo(&conn, "nope", &slot(Some("2026-11-02"), None, None)).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(None, Some("09:00"), Some("10:00"))).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), None, Some("10:00"))).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), Some("09:00"), None)).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), Some("10:00"), Some("10:00"))).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), Some("11:00"), Some("10:00"))).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(Some("not-a-day"), None, None)).is_err());
        assert!(schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), Some("24:00"), Some("25:00"))).is_err());
    }

    #[test]
    fn empty_strings_clear_the_slot_like_null() {
        let mut conn = make();
        let t = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        schedule_todo(&conn, &t.id, &slot(Some("2026-11-02"), Some("09:00"), Some("10:00"))).unwrap();
        let cleared = schedule_todo(&conn, &t.id, &slot(Some(""), Some(""), Some(""))).unwrap();
        assert!(cleared.planned_date.is_none());
    }

    #[test]
    fn list_scheduled_orders_by_day_then_untimed_then_start_and_includes_done() {
        let mut conn = make();
        let a = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        wait();
        let b = create_todo(&mut conn, &TodoInput { title: "b".into(), detail: String::new() }).unwrap();
        wait();
        let c = create_todo(&mut conn, &TodoInput { title: "c".into(), detail: String::new() }).unwrap();

        schedule_todo(&conn, &a.id, &slot(Some("2026-11-03"), Some("10:00"), Some("11:00"))).unwrap();
        schedule_todo(&conn, &b.id, &slot(Some("2026-11-02"), None, None)).unwrap();
        schedule_todo(&conn, &c.id, &slot(Some("2026-11-02"), Some("09:00"), Some("10:00"))).unwrap();
        toggle_todo(&conn, &a.id, true).unwrap();

        let got = list_scheduled_todos(&conn, "2026-11-02", "2026-11-03").unwrap();
        assert_eq!(
            got.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            vec![b.id.clone(), c.id.clone(), a.id.clone()]
        );
        assert!(got[2].completed_at.is_some(), "done scheduled todos are included");
    }

    #[test]
    fn list_scheduled_excludes_deleted_and_out_of_range() {
        let mut conn = make();
        let a = create_todo(&mut conn, &TodoInput { title: "a".into(), detail: String::new() }).unwrap();
        let b = create_todo(&mut conn, &TodoInput { title: "b".into(), detail: String::new() }).unwrap();
        schedule_todo(&conn, &a.id, &slot(Some("2026-11-02"), None, None)).unwrap();
        schedule_todo(&conn, &b.id, &slot(Some("2026-11-09"), None, None)).unwrap();
        soft_delete_todo(&conn, &a.id).unwrap();

        assert!(list_scheduled_todos(&conn, "2026-11-02", "2026-11-08").unwrap().is_empty());
        assert!(list_scheduled_todos(&conn, "bad", "2026-11-08").is_err());
    }

    #[test]
    fn list_unscheduled_returns_only_pending_untimed_todos_in_sort_order() {
        let mut conn = make();
        let (a, b, c) = make_abc(&mut conn);
        // 展示序 [c, b, a] → 待安排顺序应保持一致
        schedule_todo(&conn, &b.id, &slot(Some("2026-11-02"), None, None)).unwrap();
        toggle_todo(&conn, &a.id, true).unwrap();
        soft_delete_todo(&conn, &c.id).unwrap();

        let got = list_unscheduled_todos(&conn).unwrap();
        // c 被软删、a 已完成、b 已安排 → 空
        assert!(got.is_empty());

        let d = create_todo(&mut conn, &TodoInput { title: "d".into(), detail: String::new() }).unwrap();
        let got = list_unscheduled_todos(&conn).unwrap();
        assert_eq!(got.iter().map(|t| t.id.clone()).collect::<Vec<_>>(), vec![d.id]);
    }
}