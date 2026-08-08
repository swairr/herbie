//! tracker 状态机 + 电源/idle 分支,逐字翻译 `src/main/tracker.ts` 的 `createTracker`。
//!
//! 与 TS 的关键差异(为可测 + 可并发的设计,见 `.kilo/plans/...md` 3b 契约):
//! - 状态寄生于 `Arc<Mutex<TrackerState>>`,所有公开方法取 `&self`,从而:
//!   (1) 命令层可持有 `Arc<Tracker>` 与 `OnceLock` 共享同一实例;
//!   (2) `gate_notifier` 返回的 `GatedNotifier` 持同一 `Arc`,其回调跨闭包 mutate 状态。
//! - DB 连接经 `ConnAccess` 抽象:`GlobalConn`(prod 锁全局 `db::get()`)/
//!   `LocalConn`(test 锁局部 `Arc<Mutex<Connection>>`),仓储函数仍取 `&Connection`。
//! - DI:`TrackerDeps` 仅暴露"当前空闲秒"与"本机时刻"两个纯取值,3c 才接真实
//!   `GetLastInputInfo` 与 power 路由(本片 `start()` 仅置 `started=true`,power 由测试
//!   直接调 `on_power` 驱动 —— 行为等价于 TS 的 `deps.fire(...)`).
//!
//! 锁顺序:始终「先 state 后 db」——方法先锁 `self.state`,再在需要写库时经
//! `conn.lock_conn` 锁连接。3b 无并发场景,该顺序留作 3c 注意锚点。

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Local, SecondsFormat, Utc};
use serde::Serialize;

use crate::hook::{HookNotifier, WinHookEvent};
use crate::segment::{self, OpenSegmentInput, SegmentKind};
use crate::settings;
use crate::todos::now_iso;

const IDLE_PROCESS: &str = "[idle]";
const DEFAULT_THRESHOLD_SEC: u64 = 300;

/// tracker 的 off-work 状态,序列化 camelCase 为 `{ offWork: bool }`,对齐 renderer
/// 薄封装 `invoke<OffWorkState>`(见 `src/shared/types.ts` 的 `OffWorkState`)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OffWorkState {
    pub off_work: bool,
}

/// 统一的连接访问:`lock_conn` 在持连接锁期间执行闭包。仓储函数仍取 `&Connection`:这里
/// 只是把"连接怎么锁"这件事抽出去,使 prod(全局单例)与 test(局部内存库)共用同一状态机。
///
/// 方法带泛型,故本 trait **非 object-safe**(刻意如此);调用处用 `<C: ConnAccess>` 泛型
/// 而非 `&dyn`,既允许 list 在闭包内按值返回任意类型,也保证 `Connection` 在闭包内零拷贝借用。
pub trait ConnAccess: Send + Sync {
    fn lock_conn<R>(&self, f: impl FnOnce(&rusqlite::Connection) -> R) -> R;
}

/// prod 连接器:锁全局 `db::get()` 后借 `&Connection`。
pub struct GlobalConn;
impl ConnAccess for GlobalConn {
    fn lock_conn<R>(&self, f: impl FnOnce(&rusqlite::Connection) -> R) -> R {
        let g = crate::db::get();
        match g.as_ref() {
            Some(c) => f(c),
            None => panic!("DB not initialized"),
        }
    }
}

/// test 连接器:锁局部 `Arc<Mutex<Connection>>` 后借 `&Connection`。各测试独立持有的局部
/// 连接避免 cargo test 多线程并行下共享全局单例的竞态(切片1 教训)。
pub struct LocalConn(Arc<Mutex<rusqlite::Connection>>);
impl LocalConn {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self(conn)
    }
}
impl ConnAccess for LocalConn {
    fn lock_conn<R>(&self, f: impl FnOnce(&rusqlite::Connection) -> R) -> R {
        let g = self.0.lock().expect("conn poisoned");
        f(&*g)
    }
}

/// 对齐 spike 的 4 个电源/会话事件;`start()` 的真实 power 路由留 3c,测试直接构造此枚举
/// 调 `on_power` 驱动,等价于 TS `deps.fire('lock' | 'suspend')`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    Suspend,
    Resume,
    SessionLock,
    SessionUnlock,
}

/// DI 表面:仅暴露 tracker 状态机所需的两个取值。`get_idle_sec` 在 prod 由 3c 接
/// `GetLastInputInfo`(本片 `ProdDeps` 返 0 占位);`now_local` 即本机墙钟。
pub trait TrackerDeps {
    fn get_idle_sec(&mut self) -> u64;
    fn now_local(&mut self) -> DateTime<Local>;
}

/// prod deps:`now_local` 即 `Local::now()`;`get_idle_sec` 本片返 0(锚定 3c 接真实空闲),
/// 故 3b prod poll 永不进 idle 分支 —— 仅 off-work 命令路径被 UI 触发使用。
pub struct ProdDeps;
impl TrackerDeps for ProdDeps {
    fn get_idle_sec(&mut self) -> u64 {
        // 3c: Win32 GetLastInputInfo + tick 差值。
        0
    }
    fn now_local(&mut self) -> DateTime<Local> {
        Local::now()
    }
}

/// 可变状态(被 `Arc<Mutex<...>>` 包裹)。三字段对齐 TS 的闭包变量:
/// `offWork` / `wasIdle` / `started`。
struct TrackerState {
    off_work: bool,
    was_idle: bool,
    started: bool,
}

pub struct Tracker {
    state: Arc<Mutex<TrackerState>>,
}

impl Tracker {
    pub fn new() -> Tracker {
        Tracker {
            state: Arc::new(Mutex::new(TrackerState {
                off_work: false,
                was_idle: false,
                started: false,
            })),
        }
    }

    /// 读 `idleThresholdSec`,有限且 >0 用之,否则默认 300。逐字对齐 TS `threshold()`。
    pub fn threshold<C: ConnAccess>(&self, conn: &C) -> u64 {
        let raw = conn.lock_conn(|c| settings::get_with_default(c, settings::KEY_IDLE_THRESHOLD_SEC));
        raw.parse::<u64>()
            .ok()
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_THRESHOLD_SEC)
    }

    /// 翻译 TS `poll()`。先锁 state,再取 idle/now;off-work 与 idle 跨阈/open-idle/close-idle
    /// 分支逐字对照。写库经 `conn.lock_conn`。
    pub fn poll<C: ConnAccess>(&self, conn: &C, deps: &mut impl TrackerDeps) {
        let mut st = self.state.lock().expect("state poisoned");
        let idle_sec = deps.get_idle_sec();
        let now = deps.now_local();
        if st.off_work {
            // 下班期间不写段;仅靠 idle 归 0 判定返岗。
            if idle_sec == 0 && st.was_idle {
                st.off_work = false;
                st.was_idle = false;
            } else if idle_sec > 0 {
                st.was_idle = true;
            }
            return;
        }
        let thr = self.threshold(conn);
        if idle_sec >= thr && !st.was_idle {
            // 刚进 idle:在"最后一次输入"瞬时关掉当前活动段,并从同一瞬时开 idle 段。
            let last_input_iso = (now - Duration::seconds(idle_sec as i64))
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true);
            conn.lock_conn(|c| {
                let _ = segment::close_open(c, &last_input_iso);
                let _ = segment::open_segment(
                    c,
                    &OpenSegmentInput {
                        process_name: IDLE_PROCESS.to_string(),
                        title: String::new(),
                        kind: Some(SegmentKind::Idle),
                        start_at: Some(last_input_iso.clone()),
                        todo_id: None,
                    },
                );
            });
            st.was_idle = true;
        } else if idle_sec == 0 && st.was_idle {
            // 从 idle 返岗:用"当前时刻"关闭开放 idle 段。
            let iso = now
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true);
            conn.lock_conn(|c| {
                let _ = segment::close_open(c, &iso);
            });
            st.was_idle = false;
        }
    }

    /// 翻译 TS `onPower(name)`。suspend/lock 关当前开放段并清 wasIdle;
    /// resume/unlock 仅清 wasIdle(不开新段,段由后续前台事件自然开)。
    /// 用 `todos::now_iso()` 关段(等价 TS `deps.now().toISOString()`;时间戳在测试中无断言)。
    pub fn on_power<C: ConnAccess>(
        &self,
        conn: &C,
        _deps: &mut impl TrackerDeps,
        name: PowerEvent,
    ) {
        let mut st = self.state.lock().expect("state poisoned");
        match name {
            PowerEvent::Suspend | PowerEvent::SessionLock => {
                let iso = now_iso();
                conn.lock_conn(|c| {
                    let _ = segment::close_open(c, &iso);
                });
                st.was_idle = false;
            }
            PowerEvent::Resume | PowerEvent::SessionUnlock => {
                st.was_idle = false;
            }
        }
    }

    /// 翻译 TS `setOffWork(on)`。on && !offWork → 关开放段 + 置 offWork + 清 wasIdle;
    /// !on && offWork → 仅清 offWork。
    pub fn set_off_work<C: ConnAccess>(
        &self,
        conn: &C,
        _deps: &mut impl TrackerDeps,
        on: bool,
    ) {
        let mut st = self.state.lock().expect("state poisoned");
        if on && !st.off_work {
            let iso = now_iso();
            conn.lock_conn(|c| {
                let _ = segment::close_open(c, &iso);
            });
            st.off_work = true;
            st.was_idle = false;
        } else if !on && st.off_work {
            st.off_work = false;
        }
    }

    pub fn get_off_work(&self) -> bool {
        self.state.lock().expect("state poisoned").off_work
    }

    /// 启动:本片仅置 `started=true`。3c 在此注册 power 路由(suspend/resume/lock/unlock →
    /// `on_power`)与 idle 轮询(`setInterval(poll, 20s)` 对应项)。`deps` 参数留作 3c 路由
    /// 依赖锚点(本片未用,前缀 `_` 抑制告警)。
    pub fn start(&self, _deps: &mut impl TrackerDeps) {
        self.state.lock().expect("state poisoned").started = true;
    }

    /// 翻译 TS `gateNotifier(real)`:把任意 `HookNotifier` 包一层,使其回调在每条前台
    /// 事件到来时先"返岗"(offWork=false + wasIdle=false)再转发原始事件 —— off-work
    /// 期间被搁置的前台事件因此被提升为正常开工信号。
    pub fn gate_notifier<R: HookNotifier>(&self, real: R) -> GatedNotifier<R> {
        GatedNotifier {
            state: self.state.clone(),
            real,
        }
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

/// `gateNotifier` 的产物:持同一 `state`(与 `Tracker` 共享可变状态)与被包的真实通知器。
pub struct GatedNotifier<R: HookNotifier> {
    state: Arc<Mutex<TrackerState>>,
    real: R,
}

impl<R: HookNotifier> HookNotifier for GatedNotifier<R> {
    fn start(&mut self, mut cb: Box<dyn FnMut(WinHookEvent) + Send + 'static>) {
        let state = self.state.clone();
        self.real.start(Box::new(move |e: WinHookEvent| {
            {
                let mut st = state.lock().expect("state poisoned");
                st.off_work = false;
                st.was_idle = false;
            }
            cb(e);
        }));
    }
    fn stop(&mut self) {
        self.real.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;
    use chrono::TimeZone;
    use rusqlite::Connection;

    // 等价 TS `fakeDeps`:可变 idle/now_ms,`setIdle`/`advance`/`nowMs`。
    struct FakeDeps {
        idle: u64,
        now_ms: i64,
    }
    impl FakeDeps {
        fn new() -> Self {
            // 等价 TS `Date.UTC(2026,7,3,10,0,0)`(月 0-indexed → 8 月)。
            Self {
                idle: 0,
                now_ms: Utc
                    .with_ymd_and_hms(2026, 8, 3, 10, 0, 0)
                    .unwrap()
                    .timestamp_millis(),
            }
        }
        fn set_idle(&mut self, s: u64) {
            self.idle = s;
        }
        fn advance(&mut self, ms: i64) {
            self.now_ms += ms;
        }
    }
    impl TrackerDeps for FakeDeps {
        fn get_idle_sec(&mut self) -> u64 {
            self.idle
        }
        fn now_local(&mut self) -> DateTime<Local> {
            Local.timestamp_millis_opt(self.now_ms).unwrap()
        }
    }

    // 等价 TS `fakeNotifier`:存一条 cb,`emit` 命中。cb 存 `Arc<Mutex<Option<cb>>>`,
    // 以便克隆出句柄在外部 emit(对应 JS 对象引用语义)—— gateNotifier 按值取走后,
    // 该句柄仍与被包的 FakeHook 共享同一 cb 存储。
    #[derive(Clone)]
    struct FakeHook {
        cb: Arc<Mutex<Option<Box<dyn FnMut(WinHookEvent) + Send + 'static>>>>,
    }
    impl FakeHook {
        fn new() -> Self {
            Self {
                cb: Arc::new(Mutex::new(None)),
            }
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
            *self.cb.lock().unwrap() = Some(cb);
        }
        fn stop(&mut self) {
            *self.cb.lock().unwrap() = None;
        }
    }

    // `make()`:( Arc<Mutex<Connection>>, Arc<LocalConn> )。局部内存库 + fk + 迁移。
    fn make() -> (Arc<Mutex<Connection>>, Arc<LocalConn>) {
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

    // ①跨阈开 idle 段。
    #[test]
    fn opens_an_idle_segment_when_idle_crosses_threshold() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        with_conn(&conn, |c| settings::set(c, settings::KEY_IDLE_THRESHOLD_SEC, "10").unwrap());
        let t = Tracker::new();
        t.start(&mut deps);
        deps.set_idle(12);
        deps.advance(12_000);
        t.poll(&*ca, &mut deps);
        let rows = with_conn(&conn, |c| segment::list_all(c).unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, SegmentKind::Idle);
        assert_eq!(rows[0].process_name, "[idle]");
        assert!(rows[0].end_at.is_none());
    }

    // ②idle 归 0 关闭。
    #[test]
    fn closes_the_idle_segment_when_idle_returns_to_0() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        with_conn(&conn, |c| settings::set(c, settings::KEY_IDLE_THRESHOLD_SEC, "10").unwrap());
        let t = Tracker::new();
        t.start(&mut deps);
        deps.set_idle(12);
        deps.advance(12_000);
        t.poll(&*ca, &mut deps);
        assert_eq!(
            with_conn(&conn, |c| segment::list_all(c).unwrap()).len(),
            1
        );
        deps.set_idle(0);
        deps.advance(5_000);
        t.poll(&*ca, &mut deps);
        let rows = with_conn(&conn, |c| segment::list_all(c).unwrap());
        assert_eq!(rows.len(), 1);
        assert!(rows[0].end_at.is_some());
    }

    // ③suspend/lock closeOpen 不开 idle。
    #[test]
    fn suspend_and_lock_close_open_segment_without_creating_idle() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        let t = Tracker::new();
        t.start(&mut deps);
        t.on_power(&*ca, &mut deps, PowerEvent::SessionLock);
        t.on_power(&*ca, &mut deps, PowerEvent::Suspend);
        assert_eq!(
            with_conn(&conn, |c| segment::list_all(c).unwrap()).len(),
            0
        );
    }

    // ④setOffWork(true) 关开放段 + gated foreground 清 offWork。
    #[test]
    fn set_off_work_true_closes_open_and_gated_foreground_clears_off_work() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        let t = Tracker::new();
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let mut gated = t.gate_notifier(hook);
        let received = Arc::new(Mutex::new(None));
        let received_cb = received.clone();
        gated.start(Box::new(move |e: WinHookEvent| {
            *received_cb.lock().unwrap() = Some(e);
        }));
        t.start(&mut deps);
        t.set_off_work(&*ca, &mut deps, true);
        assert!(t.get_off_work());
        emit.emit(WinHookEvent {
            kind: "foreground",
            hwnd: 1,
            process_name: "a.exe".into(),
            title: "A".into(),
        });
        assert!(!t.get_off_work());
        assert!(received.lock().unwrap().is_some());
        let _ = &ca;
        let _ = &conn;
    }

    // ⑤off-work 期间 idle poll 写 0 段;idle 归 0 清 offWork。
    #[test]
    fn idle_poll_during_off_work_writes_nothing() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        with_conn(&conn, |c| settings::set(c, settings::KEY_IDLE_THRESHOLD_SEC, "10").unwrap());
        let t = Tracker::new();
        t.start(&mut deps);
        t.set_off_work(&*ca, &mut deps, true);
        deps.set_idle(60);
        deps.advance(60_000);
        t.poll(&*ca, &mut deps);
        assert_eq!(
            with_conn(&conn, |c| segment::list_all(c).unwrap()).len(),
            0
        );
        deps.set_idle(0);
        t.poll(&*ca, &mut deps);
        assert!(!t.get_off_work());
    }

    // ⑥gated foreground 在结束 off-work 后开新段。
    #[test]
    fn gated_foreground_opens_new_segment_after_ending_off_work() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        let t = Tracker::new();
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let mut current_hwnd: i64 = -1;
        let ca_for_cb = ca.clone();
        let mut gated = t.gate_notifier(hook);
        gated.start(Box::new(move |e: WinHookEvent| {
            if e.kind == "namechange" && current_hwnd != e.hwnd {
                return;
            }
            current_hwnd = e.hwnd;
            ca_for_cb.lock_conn(|c| {
                let _ = segment::close_open(c, &now_iso());
                let _ = segment::open_segment(
                    c,
                    &OpenSegmentInput {
                        process_name: e.process_name.clone(),
                        title: e.title.clone(),
                        kind: None,
                        todo_id: None,
                        start_at: None,
                    },
                );
            });
        }));
        t.start(&mut deps);
        t.set_off_work(&*ca, &mut deps, true);
        emit.emit(WinHookEvent {
            kind: "foreground",
            hwnd: 1,
            process_name: "a.exe".into(),
            title: "A".into(),
        });
        let rows = with_conn(&conn, |c| segment::list_all(c).unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].process_name, "a.exe");
    }

    // ⑦从 idle 经 foreground 返回后下一 poll 不关新 activity 段。
    #[test]
    fn returning_from_idle_via_foreground_keeps_next_activity_open() {
        let (conn, ca) = make();
        let mut deps = FakeDeps::new();
        with_conn(&conn, |c| settings::set(c, settings::KEY_IDLE_THRESHOLD_SEC, "10").unwrap());
        let t = Tracker::new();
        let hook = FakeHook::new();
        let mut emit = hook.clone();
        let mut current_hwnd: i64 = -1;
        let ca_for_cb = ca.clone();
        let mut gated = t.gate_notifier(hook);
        gated.start(Box::new(move |e: WinHookEvent| {
            if e.kind == "namechange" && current_hwnd != e.hwnd {
                return;
            }
            current_hwnd = e.hwnd;
            ca_for_cb.lock_conn(|c| {
                let _ = segment::close_open(c, &now_iso());
                let _ = segment::open_segment(
                    c,
                    &OpenSegmentInput {
                        process_name: e.process_name.clone(),
                        title: e.title.clone(),
                        kind: None,
                        todo_id: None,
                        start_at: None,
                    },
                );
            });
        }));
        t.start(&mut deps);
        // 1) 进 idle:poll 开 idle 段。
        deps.set_idle(15);
        deps.advance(15_000);
        t.poll(&*ca, &mut deps);
        let rows = with_conn(&conn, |c| segment::list_all(c).unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, SegmentKind::Idle);
        assert!(rows[0].end_at.is_none());
        // 2) foreground 返岗:idle 关 + activity 开。
        emit.emit(WinHookEvent {
            kind: "foreground",
            hwnd: 2,
            process_name: "a.exe".into(),
            title: "A".into(),
        });
        let rows = with_conn(&conn, |c| segment::list_all(c).unwrap());
        assert_eq!(rows.len(), 2);
        let idle_seg = rows.iter().find(|r| r.kind == SegmentKind::Idle).unwrap();
        let act_seg = rows.iter().find(|r| r.kind == SegmentKind::Activity).unwrap();
        assert!(idle_seg.end_at.is_some());
        assert!(act_seg.end_at.is_none());
        // 3) 下一 poll,active(idle 0):新 activity 段保持开放。
        deps.set_idle(0);
        deps.advance(5_000);
        t.poll(&*ca, &mut deps);
        let rows = with_conn(&conn, |c| segment::list_all(c).unwrap());
        assert_eq!(rows.len(), 2);
        let act_seg_after = rows
            .iter()
            .find(|r| r.kind == SegmentKind::Activity)
            .unwrap();
        assert!(act_seg_after.end_at.is_none());
    }
}