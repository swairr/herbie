//! Windows 原生集成(仅 Windows):前台钩子 / 空闲秒 / 电源与锁屏事件。
//!
//! 3c-B 把切片0 spike 与 `native/herbie-winhook`(C++ N-API)升级为生产回调版:
//! - `foreground`:`ForegroundHook`(SetWinEventHook + 消息泵),对应 C++ 模块;
//! - `idle`:`get_idle_sec()`(GetLastInputInfo + GetTickCount64),对应
//!   Electron `powerMonitor.getSystemIdleTime`(能力映射表第 36 行);
//! - `power`:`start_power_watcher(cb)`,把 spike 的 AppHandle emit 改为 tracker 回调。

#[cfg(windows)]
pub mod foreground;
#[cfg(windows)]
pub mod idle;
#[cfg(windows)]
pub mod power;

/// 追踪能力状态:Windows 下原生能力齐备(前台钩子 / 空闲 / 电源),恒可用。
/// 与 `crate::linux::tracking_unavailable_reason` 对称,由 `platform/` 门面转发。
pub fn tracking_unavailable_reason() -> Option<&'static str> {
    None
}
