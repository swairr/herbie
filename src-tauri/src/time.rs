//! 纯时间函数,逐行翻译 `src/shared/time.ts` 的 `dayBounds` 与 `splitAtMidnight`。
//!
//! `formatLocalShort/nowIso/formatDuration/durationMs` 等其余 time 函数**不**翻译
//! (留 TS renderer 用唯一来源)。此处仅实现写/读路径(切片3 segments 查询)所需的两条。
//!
//! 时区语义:`dayBounds` 的本地午夜依赖运行时**本地时区**(等价 JS
//! `new Date(y,m-1,d,0,0,0,0)` 的本地构造),用 `chrono::Local`。返回的
//! `start_ms/end_ms` 为 UTC 毫秒整型;`start_iso/end_iso` 为该瞬时绝对 UTC 形式
//! (`...Z`,毫秒精度),与存储侧 `now_iso()` 同形,从而与 `startAt` 列做字典序预筛。
//!
//! ISO 解析兼容两种存储形态:**带 offset/`Z`** 的绝对时间,以及 **naive**
//! (`2026-08-03T10:00:00`,JS `new Date` 解释为 LOCAL)。先按 rfc3339 解析,失败则按
//! naive 解释为本地墙钟,再转 UTC 毫秒。

use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde::Serialize;

use crate::segment::Segment;

/// `DayBounds`:本地自然日 `[start, end)` 的 UTC 毫秒边界与对应 ISO 串。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayBounds {
    pub start_ms: i64,
    pub end_ms: i64,
    pub start_iso: String,
    pub end_iso: String,
}

/// 本地日历日 -> 提供毫秒和 ISO 边界。无效输入（不是三个用连字符分隔的数字，
/// 或不存在的日期）返回 `None`。
pub fn day_bounds(local_date: &str) -> Option<DayBounds> {
    let parts: Vec<&str> = local_date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let d: u32 = parts[2].parse().ok()?;
    // `with_ymd_and_hms` 会考虑夏令时歧义（none/ambiguous）；`.single()` 仅接受单一明确日期。
    let start = Local.with_ymd_and_hms(y, m, d, 0, 0, 0).single().or_else(|| {
        // 对齐 JS `new Date(y, m-1, d)` 的溢出规范化:`2026-02-30` → 3 月 2 日。
        // 仅当月不存在该日(如 2/30、4/31)时回退到「当月 1 日 + (d-1) 天」滚动;
        // 年/月本身非法(m 不在 1..=12)仍返 None。
        let first = Local.with_ymd_and_hms(y, m, 1, 0, 0, 0).single()?;
        Some(first + chrono::Duration::days((d as i64) - 1))
    })?;
    let end = start + chrono::Duration::days(1);
    let start_ms = start.timestamp_millis();
    let end_ms = end.timestamp_millis();
    Some(DayBounds {
        start_ms,
        end_ms,
        start_iso: ms_to_iso_utc(start_ms),
        end_iso: ms_to_iso_utc(end_ms),
    })
}

/// 本地时区 `YYYY-MM-DD`,等价 TS `localDateString(nowIso())`:
/// 把 UTC 绝对时刻换算到运行时本地时区的墙钟日期。入参恒为有效时刻,故无 TS 对
/// 无效 ISO 返回空串的分支。
pub fn local_date_string(now_utc: DateTime<Utc>) -> String {
    now_utc.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

/// 解析 ISO 字符串为 UTC 毫秒。匹配 JS `new Date(iso).getTime()`：
/// rfc3339（带 offset/`Z`）成功则用其 offset；失败则按 naive 解释为**本地**墙钟时间
///（等价 JS 对无 offset 串的 local 解释）。无法解析返回 `None`。
pub fn parse_iso_to_ms(s: &str) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    // naive：匹配无 offset 的存储形式；可选毫秒位。本地时间歧义折叠与 JS 一致取 single。
    let ndt = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .ok()?;
    let local = Local.from_local_datetime(&ndt).single()?;
    Some(local.timestamp_millis())
}

/// 将 segment 切片到本地自然日 `local_date`。返回该日重叠部分 `[lo, hi)`，无重叠返回 `None`。
/// 开放的 segment（endAt None）按 `now` 截断，使聚合仅计入已流逝时间。绝不因非法输入 panic。
pub fn split_at_midnight(seg: &Segment, local_date: &str, now: DateTime<Utc>) -> Option<Segment> {
    let bounds = day_bounds(local_date)?;
    let start_ms = parse_iso_to_ms(&seg.start_at)?;
    let end_ms = match &seg.end_at {
        Some(e) => parse_iso_to_ms(e)?,
        None => now.timestamp_millis(),
    };
    if end_ms < start_ms {
        return None;
    }
    let lo = start_ms.max(bounds.start_ms);
    let hi = end_ms.min(bounds.end_ms);
    if hi <= lo {
        return None;
    }
    Some(Segment {
        id: seg.id.clone(),
        start_at: ms_to_iso_utc(lo),
        end_at: Some(ms_to_iso_utc(hi)),
        process_name: seg.process_name.clone(),
        title: seg.title.clone(),
        note: seg.note.clone(),
        todo_id: seg.todo_id.clone(),
        kind: seg.kind,
    })
}

fn ms_to_iso_utc(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .unwrap()
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// 统一「当前时刻/任意瞬时」的存储格式:UTC、毫秒、`Z` 后缀(等价 JS `toISOString`,
/// 亦即 `todos::now_iso`)。全库唯一格式化入口,避免多处内联漂移破坏按字典序的时间预筛。
pub fn iso_utc_z_millis(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    // 测试断言用本地 wall 时分比对（等价 TS `new Date(out.startAt).getHours()`），
    // 避开具体运行机器时区耦合。

    fn local_at(ms: i64) -> DateTime<Local> {
        Local.timestamp_millis_opt(ms).unwrap()
    }

    fn seg(start: &str, end: Option<&str>) -> Segment {
        Segment {
            id: "s".into(),
            start_at: start.into(),
            end_at: end.map(str::to_string),
            process_name: "app.exe".into(),
            title: String::new(),
            note: String::new(),
            todo_id: None,
            kind: crate::segment::SegmentKind::Activity,
        }
    }

    fn now_at(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn day_bounds_returns_invalid_for_malformed() {
        assert!(day_bounds("bad").is_none());
        assert!(day_bounds("2026-08").is_none());
        assert!(day_bounds("abc-def-ghi").is_none());
        // 年/月非法(m > 12)仍拒绝,与 JS 不同但 renderer 不会产这类串。
        assert!(day_bounds("2026-13-01").is_none());
    }

    #[test]
    fn day_bounds_normalizes_overflow_day_like_js() {
        // JS `new Date(2026, 1, 30)` → 3 月 2 日:溢出日应滚动而非拒绝,
        // 避免遗留库中此类日期的 journal 变不可编辑。
        let overflow = day_bounds("2026-02-30").unwrap();
        let normalized = day_bounds("2026-03-02").unwrap();
        assert_eq!(overflow.start_ms, normalized.start_ms);
        assert_eq!(overflow.end_ms, normalized.end_ms);
    }

    #[test]
    fn day_bounds_marks_local_midnight_to_next_midnight() {
        let b = day_bounds("2026-08-03").unwrap();
        let s = local_at(b.start_ms);
        let e = local_at(b.end_ms);
        assert_eq!(s.day(), 3);
        assert_eq!(s.hour(), 0);
        assert_eq!(s.minute(), 0);
        assert_eq!(e.day(), 4);
        assert_eq!(e.hour(), 0);
    }

    #[test]
    fn local_date_string_formats_local_calendar_date() {
        // 以本地墙钟构造 2026-08-03 12:00,再转 UTC,断言回读仍是本地同日 —— 与时区解耦。
        let noon = Local
            .with_ymd_and_hms(2026, 8, 3, 12, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(local_date_string(noon), "2026-08-03");
        // 午夜前 1 分钟仍属当日,深夜 23:59 也属当日;跨日由 day_bounds 覆盖。
        let late = Local
            .with_ymd_and_hms(2026, 8, 3, 23, 59, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(local_date_string(late), "2026-08-03");
    }

    #[test]
    fn split_unchanged_when_fully_within_day() {
        let s = seg("2026-08-03T10:00:00", Some("2026-08-03T11:30:00"));
        let out = split_at_midnight(&s, "2026-08-03", Utc::now()).unwrap();
        assert_eq!(local_at(parse_iso_to_ms(&out.start_at).unwrap()).hour(), 10);
        let e = local_at(parse_iso_to_ms(out.end_at.as_ref().unwrap()).unwrap());
        assert_eq!(e.hour(), 11);
        assert_eq!(e.minute(), 30);
        assert_eq!(out.id, "s");
    }

    #[test]
    fn split_into_day_segment_starts() {
        let s = seg("2026-08-03T23:30:00", Some("2026-08-04T00:30:00"));
        let out = split_at_midnight(&s, "2026-08-03", Utc::now()).unwrap();
        assert_eq!(local_at(parse_iso_to_ms(&out.start_at).unwrap()).hour(), 23);
        let e = local_at(parse_iso_to_ms(out.end_at.as_ref().unwrap()).unwrap());
        assert_eq!(e.day(), 4);
        assert_eq!(e.hour(), 0);
    }

    #[test]
    fn split_into_day_segment_ends() {
        let s = seg("2026-08-03T23:30:00", Some("2026-08-04T00:30:00"));
        let out = split_at_midnight(&s, "2026-08-04", Utc::now()).unwrap();
        let st = local_at(parse_iso_to_ms(&out.start_at).unwrap());
        assert_eq!(st.hour(), 0);
        let e = local_at(parse_iso_to_ms(out.end_at.as_ref().unwrap()).unwrap());
        assert_eq!(e.hour(), 0);
        assert_eq!(e.minute(), 30);
    }

    #[test]
    fn split_null_when_on_a_different_day() {
        let s = seg("2026-08-03T10:00:00", Some("2026-08-03T11:00:00"));
        assert!(split_at_midnight(&s, "2026-08-05", Utc::now()).is_none());
    }

    #[test]
    fn split_clamps_open_segment_to_now() {
        let now = now_at(2026, 8, 3, 15, 0);
        let s = seg("2026-08-03T10:00:00", None);
        let out = split_at_midnight(&s, "2026-08-03", now).unwrap();
        assert_eq!(out.end_at.unwrap(), ms_to_iso_utc(now.timestamp_millis()));
    }

    #[test]
    fn split_open_yesterday_appears_today_as_midnight_to_now() {
        let now = now_at(2026, 8, 4, 9, 0);
        let s = seg("2026-08-03T22:00:00", None);
        let out = split_at_midnight(&s, "2026-08-04", now).unwrap();
        assert_eq!(local_at(parse_iso_to_ms(&out.start_at).unwrap()).hour(), 0);
        assert_eq!(out.end_at.unwrap(), ms_to_iso_utc(now.timestamp_millis()));
    }

    #[test]
    fn split_handles_malformed_start_as_null() {
        let s = seg("not-a-date", Some("2026-08-03T11:00:00"));
        assert!(split_at_midnight(&s, "2026-08-03", Utc::now()).is_none());
    }

    #[test]
    fn split_boundary_exactly_at_midnight_keeps_start() {
        let s = seg("2026-08-03T00:00:00", Some("2026-08-04T00:00:00"));
        let out = split_at_midnight(&s, "2026-08-03", Utc::now()).unwrap();
        assert_eq!(
            parse_iso_to_ms(&out.start_at).unwrap(),
            parse_iso_to_ms("2026-08-03T00:00:00").unwrap()
        );
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SplitFixtureCase {
        seg: SplitFixtureSeg,
        local_date: String,
        now: Option<String>,
        expected: Option<SplitFixtureExpected>,
    }

    #[derive(serde::Deserialize)]
    struct SplitFixtureSeg {
        start: String,
        end: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct SplitFixtureExpected {
        start: String,
        end: String,
    }

    /// absolute ISO → 本机本地墙钟 `YYYY-MM-DDTHH:MM`(与 TS 夹具测试的 `toWall` 同语义)。
    fn wall_minutes(iso: &str) -> String {
        let ms = parse_iso_to_ms(iso).expect("output iso must parse");
        local_at(ms).format("%Y-%m-%dT%H:%M").to_string()
    }

    /// 共享 JSON 夹具对拍:与 TS `tests/segments-split-fixture.test.ts` 读同一份
    /// `tests/fixtures/segments-split.json`。夹具时间均为 naive 墙钟,两侧把 impl 的
    /// absolute 输出转回本机本地墙钟分钟再比 → 任意时区一致命中。
    #[test]
    fn split_at_midnight_fixture_parity() {
        let raw = include_str!("../../tests/fixtures/segments-split.json");
        let cases: Vec<SplitFixtureCase> =
            serde_json::from_str(raw).expect("segments-split.json must parse as array of cases");
        for c in &cases {
            let seg = Segment {
                id: "s".into(),
                start_at: c.seg.start.clone(),
                end_at: c.seg.end.clone(),
                process_name: String::new(),
                title: String::new(),
                note: String::new(),
                todo_id: None,
                kind: crate::segment::SegmentKind::Activity,
            };
            // naive → 按本地解释转 UTC 毫秒(parse_iso_to_ms),再构造 DateTime<Utc> 作 now。
            let now = match &c.now {
                Some(n) => Utc
                    .timestamp_millis_opt(parse_iso_to_ms(n).expect("now must parse"))
                    .unwrap(),
                None => Utc::now(),
            };
            let out = split_at_midnight(&seg, &c.local_date, now);
            match &c.expected {
                None => assert!(out.is_none(), "expected null slice for {}", c.local_date),
                Some(exp) => {
                    let out = out.expect("expected a slice");
                    assert_eq!(
                        wall_minutes(&out.start_at),
                        exp.start,
                        "start wall for {}",
                        c.local_date
                    );
                    let end_iso = out.end_at.as_ref().expect("slices always have end_at");
                    assert_eq!(
                        wall_minutes(end_iso),
                        exp.end,
                        "end wall for {}",
                        c.local_date
                    );
                }
            }
        }
    }
}