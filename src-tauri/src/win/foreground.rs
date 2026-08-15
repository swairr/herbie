//! 前台窗口钩子:`ForegroundHook` 实现 `HookNotifier`,把 `native/herbie-winhook`
//! (C++ N-API)移植到 windows-rs。
//!
//! 线程模型:回调 `cb` 被 **move 进一个专用线程**,该线程 SetWinEventHook 后跑
//! GetMessage 消息泵。`WINEVENT_OUTOFCONTEXT` 保证 WinEventProc 在**安装钩子的线程**
//! (即该专用线程)被派发,而非钩子源进程 —— 因此回调、去重状态、pid 缓存全部单线程访问,
//! 闭包 `&mut` 捕获安全(无需额外互斥;评审锚点)。stop 经 `PostThreadMessage(WM_QUIT)`
//! 唤醒消息泵后 join,线程退出前显式 UnhookWinEvent。
//!
//! IME/工具窗过滤:`is_ime_tool_window` 丢弃两类噪声窗口 —— (1) TSF 输入法 UI 窗口
//! (类名 `CiceroUIWndFrame`,现代输入法候选词/语言条统一类名,零维护);(2) 不激活且
//! 不进任务栏/ALT+TAB 且无标题的工具窗(IME 悬浮窗等)。过滤在去重**之前**进行,
//! 避免 IME 事件污染去重状态。

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{
    SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_NAMECHANGE, EVENT_SYSTEM_FOREGROUND, GetClassNameW,
    GetForegroundWindow, GetMessageW, GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId,
    GWL_EXSTYLE, PostThreadMessageW, TranslateMessage, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_QUIT, MSG,
};

use crate::hook::{HookNotifier, WinHookEvent};

/// 事件常量 → `WinHookEvent.kind`(纯函数):对齐 C++ `WinEventProc` 的 type 判定。
pub fn event_kind(event: u32) -> &'static str {
    if event == EVENT_SYSTEM_FOREGROUND {
        "foreground"
    } else {
        "namechange"
    }
}

/// 全路径 → 可执行文件 basename(纯函数):`C:\a\b.exe` → `b.exe`;无分隔符原样返回。
pub fn process_basename_from_path(full: &str) -> &str {
    full.rsplit(['\\', '/']).next().unwrap_or(full)
}

/// TSF 输入法 UI 窗口类名:现代输入法(搜狗/微软拼音等)的候选词窗、语言条统一走
/// Text Services Framework,UI 线程窗口固定用此类名 —— 命中即视为 IME 窗口,零维护。
pub const TSF_IME_CLASS: &str = "CiceroUIWndFrame";

// GWL_EXSTYLE 相关位(手工常量,避免依赖 WINDOW_EX_STYLE 的类型包装):
// - WS_EX_NOACTIVATE 0x08000000:窗口不抢键盘焦点(IME 悬浮窗特征);
// - WS_EX_TOOLWINDOW 0x00000080:工具窗,不占任务栏条目;
// - WS_EX_APPWINDOW 0x00040000:强制出现在任务栏/ALT+TAB(真实应用窗口通常带)。
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;

/// 判定窗口是否应被忽略(IME 候选词窗 / 无任务栏的悬浮工具窗,纯函数)。
///
/// 两条规则(见模块注释):
/// 1. 类名 == `CiceroUIWndFrame` → TSF 输入法 UI 窗口,一律丢弃(标题/样式不管);
/// 2. `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` 且**不带** `WS_EX_APPWINDOW` 且标题为空
///    → 不激活、不进任务栏/切换器、无标题的工具窗,IME 悬浮窗符合;真实应用极少同时满足。
pub fn is_ime_tool_window(class_name: &str, ex_style: u32, title: &str) -> bool {
    if class_name == TSF_IME_CLASS {
        return true;
    }
    ex_style & WS_EX_NOACTIVATE != 0
        && ex_style & WS_EX_TOOLWINDOW != 0
        && ex_style & WS_EX_APPWINDOW == 0
        && title.is_empty()
}

/// 去重状态机(纯函数):照 C++ `EmitEvent` 的 lastType/lastProc/lastTitle。同一
/// (kind, process, title) 连续重复时 `should_emit` 返 false,抑制高频事件风暴。
#[derive(Default)]
pub struct Dedup {
    last_type: &'static str,
    last_proc: String,
    last_title: String,
}

impl Dedup {
    /// 空状态(const 构造,供钩子线程 thread_local 初始化)。
    pub const fn new() -> Dedup {
        Dedup {
            last_type: "",
            last_proc: String::new(),
            last_title: String::new(),
        }
    }

    pub fn should_emit(&mut self, kind: &'static str, process: &str, title: &str) -> bool {
        if self.last_type == kind && self.last_proc == process && self.last_title == title {
            return false;
        }
        self.last_type = kind;
        self.last_proc = process.to_string();
        self.last_title = title.to_string();
        true
    }
}

// 钩子线程的本地状态:回调 / 去重 / pid 缓存均只在钩子线程被访问(见模块注释),
// thread_local 恰好表达"单线程专有"。
thread_local! {
    static CALLBACK: RefCell<Option<Box<dyn FnMut(WinHookEvent) + Send + 'static>>> = RefCell::new(None);
    static DEDUP: RefCell<Dedup> = RefCell::new(Dedup::new());
    static PID_CACHE: RefCell<HashMap<u32, String>> = RefCell::new(HashMap::new());
}

struct Running {
    thread_id: u32,
    join: thread::JoinHandle<()>,
}

/// 真实 HookNotifier:SetWinEventHook(EVENT_SYSTEM_FOREGROUND + EVENT_OBJECT_NAMECHANGE)。
pub struct ForegroundHook {
    state: Mutex<Option<Running>>,
}

impl ForegroundHook {
    pub fn new() -> ForegroundHook {
        ForegroundHook {
            state: Mutex::new(None),
        }
    }
}

impl Default for ForegroundHook {
    fn default() -> Self {
        Self::new()
    }
}

impl HookNotifier for ForegroundHook {
    fn start(&mut self, cb: Box<dyn FnMut(WinHookEvent) + Send + 'static>) {
        let mut st = self.state.lock().expect("foreground hook state poisoned");
        if st.is_some() {
            eprintln!("[win:foreground] 已在运行,忽略重复 start");
            return;
        }
        // 线程把本线程 OS id 经 channel 送回:保证 start 返回时 thread_id 已就绪,
        // stop 的 PostThreadMessage(WM_QUIT) 不会因竞态而打空。
        let (tx, rx) = mpsc::channel::<u32>();
        let join = thread::Builder::new()
            .name("herbie-foreground-hook".to_string())
            .spawn(move || pump_thread(cb, tx))
            .expect("failed to spawn foreground hook thread");
        let thread_id = rx
            .recv()
            .expect("foreground hook thread exited before reporting id");
        *st = Some(Running { thread_id, join });
    }

    fn stop(&mut self) {
        let mut st = self.state.lock().expect("foreground hook state poisoned");
        if let Some(running) = st.take() {
            // 唤醒消息泵;GetMessageW 收到 WM_QUIT 返 0,循环退出,线程可 join。
            let _ = unsafe { PostThreadMessageW(running.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
            let _ = running.join.join();
        }
    }
}

/// 消息泵线程入口:装 cb → 设两个钩子 → GetMessage 循环 → 退出前解钩。
fn pump_thread(cb: Box<dyn FnMut(WinHookEvent) + Send + 'static>, tx: mpsc::Sender<u32>) {
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = tx.send(thread_id);
    CALLBACK.with(|c| *c.borrow_mut() = Some(cb));

    // WINEVENT_OUTOFCONTEXT:系统把事件投递给**本线程**(SetWinEventHook 所在线程),
    // 本线程的 GetMessage/Dispatch 栈内调用 win_event_proc —— 回调单线程派发的依据。
    let fg_hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    let name_hook = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_NAMECHANGE,
            EVENT_OBJECT_NAMECHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if fg_hook.is_invalid() && name_hook.is_invalid() {
        eprintln!("[win:foreground] SetWinEventHook 全部失败");
    }

    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if !r.as_bool() {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // 消息循环退出(收到 WM_QUIT)后显式解钩;线程退出时系统也会自动清钩。
    if !fg_hook.is_invalid() {
        let _ = unsafe { UnhookWinEvent(fg_hook) };
    }
    if !name_hook.is_invalid() {
        let _ = unsafe { UnhookWinEvent(name_hook) };
    }
}

/// SetWinEventHook 回调(仅被钩子线程派发)。照 C++ `WinEventProc` + `EmitEvent`:
/// - NAMECHANGE 仅保留当前前台窗口;
/// - hwnd → pid(GetWindowThreadProcessId);
/// - pid → 镜像 basename(带 pid 缓存,避免高频 namechange 反复 OpenProcess);
/// - GetWindowTextW 取标题;三要素与上次相同则去重跳过;
/// - 组 `WinHookEvent` 调 `cb`。回调来自 native 栈,业务 panic 不得跨 FFI 外泄,
///   故 catch_unwind 包裹。
unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if hwnd.is_invalid() {
        return;
    }
    // 原生侧过滤:NAMECHANGE 只对当前前台窗口有意义,迟到的旧窗口事件忽略(照 C++)。
    if event == EVENT_OBJECT_NAMECHANGE {
        let fg = unsafe { GetForegroundWindow() };
        if hwnd != fg {
            return;
        }
    }
    let kind = event_kind(event);
    let process_name = pid_to_basename(window_pid(hwnd));
    let title = window_title(hwnd);

    // IME/工具窗过滤(见 `is_ime_tool_window`):在去重之前丢弃,不污染去重状态。
    if is_ime_tool_window(&window_class_name(hwnd), window_ex_style(hwnd), &title) {
        return;
    }

    let emit = DEDUP.with(|d| d.borrow_mut().should_emit(kind, &process_name, &title));
    if !emit {
        return;
    }
    let evt = WinHookEvent {
        kind,
        hwnd: hwnd.0 as i64,
        process_name,
        title,
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CALLBACK.with(|c| {
            if let Some(cb) = c.borrow_mut().as_mut() {
                cb(evt);
            }
        });
    }));
}

fn window_pid(hwnd: HWND) -> Option<u32> {
    let mut pid: u32 = 0;
    let tid = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if tid == 0 || pid == 0 {
        return None;
    }
    Some(pid)
}

/// pid → 镜像 basename,带缓存(同一 pid 的镜像名在其生命周期内不变)。仅钩子线程调用。
fn pid_to_basename(pid: Option<u32>) -> String {
    let pid = match pid {
        Some(p) => p,
        None => return String::new(),
    };
    PID_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some(name) = cache.get(&pid) {
            return name.clone();
        }
        let name = query_pid_basename(pid);
        cache.insert(pid, name.clone());
        name
    })
}

/// OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) + QueryFullProcessImageNameW → basename。
/// 提升权限的进程打开失败 → 空串(照 C++ 语义,不报错)。
fn query_pid_basename(pid: u32) -> String {
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return String::new(),
    };
    let mut buf = [0u16; 260];
    let mut size = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size)
    };
    let _ = unsafe { CloseHandle(handle) };
    if ok.is_err() || size == 0 {
        return String::new();
    }
    process_basename_from_path(&String::from_utf16_lossy(&buf[..size as usize])).to_string()
}

/// GetWindowTextW 标题(照 C++:失败/空 → 空串)。
fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

/// GetClassNameW 类名:供 `is_ime_tool_window` 识别 TSF 输入法 UI 窗口。失败/空 → 空串。
fn window_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

/// GetWindowLongPtrW(GWL_EXSTYLE) → u32(扩展样式位,供 `is_ime_tool_window` 用)。
/// 失败返回 0(照标题的"失败 → 空"语义,不报错)。
fn window_ex_style(hwnd: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_basename_from_path_extracts_base() {
        assert_eq!(process_basename_from_path(r"C:\a\b.exe"), "b.exe");
        assert_eq!(process_basename_from_path("C:/a/b.exe"), "b.exe");
        assert_eq!(process_basename_from_path("b.exe"), "b.exe");
        assert_eq!(process_basename_from_path(r"C:\a\"), "");
        assert_eq!(process_basename_from_path(r"C:\"), "");
    }

    #[test]
    fn event_kind_maps_foreground_and_namechange() {
        assert_eq!(event_kind(EVENT_SYSTEM_FOREGROUND), "foreground");
        assert_eq!(event_kind(EVENT_OBJECT_NAMECHANGE), "namechange");
        assert_eq!(event_kind(99999), "namechange");
    }

    #[test]
    fn tsf_ime_class_is_always_filtered() {
        // CiceroUIWndFrame 命中即过滤,与标题/样式无关。
        assert!(is_ime_tool_window(TSF_IME_CLASS, 0, ""));
        assert!(is_ime_tool_window(TSF_IME_CLASS, 0, "候选词"));
        assert!(is_ime_tool_window(TSF_IME_CLASS, WS_EX_APPWINDOW, ""));
    }

    #[test]
    fn noactivate_toolwindow_without_title_is_filtered() {
        let tool = WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
        assert!(is_ime_tool_window("Other", tool, ""));
    }

    #[test]
    fn real_app_windows_are_not_filtered() {
        let tool = WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
        // 带任务栏/切换器入口 → 不过滤。
        assert!(!is_ime_tool_window("Other", tool | WS_EX_APPWINDOW, ""));
        // 无 NOACTIVATE → 不过滤。
        assert!(!is_ime_tool_window("Other", WS_EX_TOOLWINDOW, ""));
        // 有可见标题 → 不过滤。
        assert!(!is_ime_tool_window("Other", tool, "文档 - Notepad"));
        // 完全无工具窗样式 → 不过滤。
        assert!(!is_ime_tool_window("Other", 0, ""));
    }

    #[test]
    fn dedup_skips_repeated_triples_and_keeps_changes() {
        let mut d = Dedup::new();
        assert!(d.should_emit("foreground", "a.exe", "A"));
        assert!(!d.should_emit("foreground", "a.exe", "A"));
        assert!(d.should_emit("foreground", "a.exe", "B"));
        assert!(d.should_emit("namechange", "a.exe", "B"));
        assert!(!d.should_emit("namechange", "a.exe", "B"));
        assert!(d.should_emit("namechange", "b.exe", "B"));
        assert!(d.should_emit("namechange", "b.exe", ""));
    }
}
