//! 生产版电源/锁屏事件投递:把切片0 `spike_power.rs`(AppHandle emit demo)改造为
//! **回调版** —— 独立线程建不可见顶层 tool 窗口(父=桌面,WS_POPUP +
//! WS_EX_TOOLWINDOW + WS_EX_NOACTIVATE,不 ShowWindow),接收 `WM_POWERBROADCAST`
//! 与 `WM_WTSSESSION_CHANGE`,映射为 `tracker::PowerEvent` 调 `cb`。
//!
//! 与 spike 的隔离(有意各自独立):spike 保留给 demo(独立线程/窗口 + `power://event`
//! 前端 emit);本模块是 tracker 生产接线用,也独立开线程/窗口。窗口类名不同、消息泵
//! 互不干扰,两个 watcher 共存无害。
//!
//! 为何不用 message-only window:系统不会把 broadcast 消息(含 WM_POWERBROADCAST,
//! 即挂起/唤醒)投递给 HWND_MESSAGE 窗口,只有注册性质的 WM_WTSSESSION_CHANGE
//! 能到达 —— 与切片0 spike 同一结论,故照抄「不可见顶层 tool 窗口」方案。

#![allow(dead_code)]

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
    TranslateMessage, MSG, WNDCLASSEXW, WS_POPUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WM_POWERBROADCAST, WM_WTSSESSION_CHANGE,
};

use crate::tracker::PowerEvent;

const PBT_APMSUSPEND: u32 = 4;
const PBT_APMRESUMEAUTOMATIC: u32 = 18;
const WTS_SESSION_LOCK: u32 = 7;
const WTS_SESSION_UNLOCK: u32 = 8;

static STARTED: AtomicBool = AtomicBool::new(false);

// 回调存线程局部:窗口过程只被本线程的消息泵调用,单线程访问安全。
thread_local! {
    static CALLBACK: RefCell<Option<Box<dyn Fn(PowerEvent) + Send + 'static>>> = RefCell::new(None);
}

/// 消息 → PowerEvent 映射(纯函数):仅认 WM_POWERBROADCAST 与 WM_WTSSESSION_CHANGE
/// 的 4 个 wParam,其余返回 None。
pub fn map_event(msg: u32, wparam: u32) -> Option<PowerEvent> {
    if msg == WM_POWERBROADCAST {
        match wparam {
            PBT_APMSUSPEND => Some(PowerEvent::Suspend),
            PBT_APMRESUMEAUTOMATIC => Some(PowerEvent::Resume),
            _ => None,
        }
    } else if msg == WM_WTSSESSION_CHANGE {
        match wparam {
            WTS_SESSION_LOCK => Some(PowerEvent::SessionLock),
            WTS_SESSION_UNLOCK => Some(PowerEvent::SessionUnlock),
            _ => None,
        }
    } else {
        None
    }
}

/// 窗口过程:回调来自 native 栈(USER32 派发),业务 panic 不得跨 FFI 外泄,故 catch_unwind。
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(evt) = map_event(msg, wparam.0 as u32) {
        eprintln!("[win:power] event = {evt:?}");
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            CALLBACK.with(|c| {
                if let Some(cb) = c.borrow().as_ref() {
                    cb(evt);
                }
            });
        }));
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 启动电源/锁屏事件投递(单次,幂等 guard)。线程常驻 detach;若切片6/7 需优雅停止,
/// 再补停句柄/消息,当前以进程退出回收(注释锚定 slice6/7)。
pub fn start_power_watcher(cb: Box<dyn Fn(PowerEvent) + Send + 'static>) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::Builder::new()
        .name("herbie-power-watcher".to_string())
        .spawn(move || {
            CALLBACK.with(|c| *c.borrow_mut() = Some(cb));

            let class_name = w!("HerbiePowerTracker");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            };
            let atom = unsafe { RegisterClassExW(&wc) };
            if atom == 0 {
                eprintln!("[win:power] RegisterClassExW failed");
                return;
            }
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class_name,
                    windows::core::PCWSTR::null(),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    None,
                    None,
                )
            };
            let hwnd = match hwnd {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("[win:power] CreateWindowExW failed: {e}");
                    return;
                }
            };
            if unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }.is_err() {
                eprintln!("[win:power] WTSRegisterSessionNotification failed");
            }
            let mut msg = MSG::default();
            loop {
                let r = unsafe { GetMessageW(&mut msg, Some(hwnd), 0, 0) };
                if !r.as_bool() {
                    break;
                }
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            eprintln!("[win:power] message loop exited");
        })
        .expect("failed to spawn power watcher thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_power_and_session_messages() {
        assert_eq!(
            map_event(WM_POWERBROADCAST, PBT_APMSUSPEND),
            Some(PowerEvent::Suspend)
        );
        assert_eq!(
            map_event(WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC),
            Some(PowerEvent::Resume)
        );
        assert_eq!(
            map_event(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK),
            Some(PowerEvent::SessionLock)
        );
        assert_eq!(
            map_event(WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK),
            Some(PowerEvent::SessionUnlock)
        );
    }

    #[test]
    fn ignores_unrelated_messages_and_params() {
        assert_eq!(map_event(WM_POWERBROADCAST, 1234), None);
        assert_eq!(map_event(WM_WTSSESSION_CHANGE, 999), None);
        assert_eq!(map_event(0x0111, PBT_APMSUSPEND), None);
        assert_eq!(map_event(0x0111, 0), None);
    }

    #[test]
    fn message_constants_nonzero_and_distinct() {
        assert_ne!(WM_POWERBROADCAST, 0);
        assert_ne!(WM_WTSSESSION_CHANGE, 0);
        assert_ne!(WM_POWERBROADCAST, WM_WTSSESSION_CHANGE);
    }
}
