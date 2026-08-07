// 技术 spike：电源/锁屏事件与 Tauri 事件循环共存。
// 实现方案：在独立线程上创建一个「不可见的顶层 tool 窗口」
// （父窗口 = 桌面 HWND NULL；WS_POPUP + WS_EX_TOOLWINDOW + WS_EX_NOACTIVATE，
//  不 ShowWindow，因而不出现在任务栏/Alt-Tab），该线程跑自己的 GetMessage
// 循环接收 WM_POWERBROADCAST / WM_WTSSESSION_CHANGE，捕获事件后通过
// tauri::AppHandle 的 emit 推给前端（事件名 "power://event"），
// Tauri/tao 的主事件循环线程不参与 Win32 消息处理。
//
// 为何不用 message-only window（HWND_MESSAGE）：
//   系统不会把 broadcast 消息（含 WM_POWERBROADCAST，即挂起/唤醒）投递给
//   message-only 窗口，只有注册性质的 WM_WTSSESSION_CHANGE 能到达它。
//   因此用「不可见顶层 tool 窗口」才能同时收到 suspend/resume 与 lock/unlock。
//
// 如何手动验证（本环境无 UI，无法在真实 WebView 跑通）：
//   1) pnpm tauri dev 起应用；
//   2) 在渲染端调用 invoke('power_subscribe') 一次（demo 按钮或控制台）；
//   3) 让电脑挂起/唤醒：Win+L 锁屏再解锁、合盖/开盖、或开始菜单「睡眠」；
//   4) 观察 stderr / 前端 power://event 监听回调，应分别出现
//      "suspend" / "resume-automatic" / "session-lock" / "session-unlock"。

#![allow(dead_code)]

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use tauri::{AppHandle, Emitter};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
    TranslateMessage, MSG, WNDCLASSEXW, WS_POPUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WM_POWERBROADCAST, WM_WTSSESSION_CHANGE,
};

const PBT_APMSUSPEND: u32 = 4;
const PBT_APMRESUMEAUTOMATIC: u32 = 18;
const WTS_SESSION_LOCK: u32 = 7;
const WTS_SESSION_UNLOCK: u32 = 8;

thread_local! {
    static APP_HANDLE: RefCell<Option<AppHandle>> = RefCell::new(None);
}

static STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerEvent {
    Suspend,
    ResumeAutomatic,
    SessionLock,
    SessionUnlock,
}

impl PowerEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            PowerEvent::Suspend => "suspend",
            PowerEvent::ResumeAutomatic => "resume-automatic",
            PowerEvent::SessionLock => "session-lock",
            PowerEvent::SessionUnlock => "session-unlock",
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let event = if msg == WM_POWERBROADCAST {
        match wparam.0 as u32 {
            PBT_APMSUSPEND => Some(PowerEvent::Suspend),
            PBT_APMRESUMEAUTOMATIC => Some(PowerEvent::ResumeAutomatic),
            _ => None,
        }
    } else if msg == WM_WTSSESSION_CHANGE {
        match wparam.0 as u32 {
            WTS_SESSION_LOCK => Some(PowerEvent::SessionLock),
            WTS_SESSION_UNLOCK => Some(PowerEvent::SessionUnlock),
            _ => None,
        }
    } else {
        None
    };

    if let Some(evt) = event {
        eprintln!("[spike_power] event = {}", evt.as_str());
        APP_HANDLE.with(|cell| {
            if let Some(app) = cell.borrow().as_ref() {
                let _ = app.emit("power://event", evt.as_str());
            }
        });
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub fn start_power_watcher(app: AppHandle) -> Result<(), String> {
    if STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    thread::spawn(move || {
        APP_HANDLE.with(|cell| *cell.borrow_mut() = Some(app.clone()));

        let class_name = w!("HerbiePowerWatcher");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            eprintln!("[spike_power] RegisterClassExW failed");
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
                eprintln!("[spike_power] CreateWindowExW failed: {e}");
                return;
            }
        };

        let notified = unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) };
        if notified.is_err() {
            eprintln!("[spike_power] WTSRegisterSessionNotification failed");
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

        eprintln!("[spike_power] message loop exited");
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names() {
        assert_eq!(PowerEvent::Suspend.as_str(), "suspend");
        assert_eq!(PowerEvent::ResumeAutomatic.as_str(), "resume-automatic");
        assert_eq!(PowerEvent::SessionLock.as_str(), "session-lock");
        assert_eq!(PowerEvent::SessionUnlock.as_str(), "session-unlock");
    }

    #[test]
    fn power_event_roundtrip() {
        let cases = [
            PowerEvent::Suspend,
            PowerEvent::ResumeAutomatic,
            PowerEvent::SessionLock,
            PowerEvent::SessionUnlock,
        ];
        for ev in cases {
            for other in cases {
                assert_eq!(ev == other, ev.as_str() == other.as_str());
            }
        }
    }

    #[test]
    fn message_constants_nonzero() {
        assert!(WM_POWERBROADCAST != 0, "WM_POWERBROADCAST must be non-zero");
        assert!(
            WM_WTSSESSION_CHANGE != 0,
            "WM_WTSSESSION_CHANGE must be non-zero"
        );
    }

    #[test]
    fn wts_change_is_power_distinct() {
        assert_ne!(WM_POWERBROADCAST, WM_WTSSESSION_CHANGE);
    }
}