//! Linux 平台集成:追踪三件套(前台窗口 / 空闲 / 电源)尚未实现 —— 见
//! `docs/adr/0004-linux-tracking-unavailable.md`。
//!
//! 本模块只声明能力缺失(`tracking_unavailable_reason` 返回 `Some(_)`),下面的空实现
//! 仅为满足编译:`lib.rs::start_tracking_system` 由门面守卫不会接线,运行路径不可达。
//! UI 以 Tracking Unavailable 显式呈现(横幅 / 托盘 / 设置页)。

use crate::hook::{HookNotifier, WinHookEvent};
use crate::tracker::PowerEvent;

/// 追踪不可用原因(UI 展示)。`None` 表示平台具备完整追踪能力。
pub fn tracking_unavailable_reason() -> Option<&'static str> {
    Some("Linux 平台尚未实现活动追踪（前台窗口 / 空闲 / 电源事件）")
}

/// 空闲秒空实现:恒 0(无空闲),仅 off-work 命令路径可能触达,无记录效果。
pub fn get_idle_sec() -> u64 {
    0
}

/// 前台钩子空实现:不可达(由 `tracking_unavailable_reason` 守卫)。
pub struct ForegroundHook;

impl ForegroundHook {
    pub fn new() -> ForegroundHook {
        ForegroundHook
    }
}

impl Default for ForegroundHook {
    fn default() -> Self {
        Self::new()
    }
}

impl HookNotifier for ForegroundHook {
    fn start(&mut self, _cb: Box<dyn FnMut(WinHookEvent) + Send + 'static>) {}
    fn stop(&mut self) {}
}

/// 电源 watcher 空实现:不可达(同 `ForegroundHook`)。
pub fn start_power_watcher(_cb: Box<dyn Fn(PowerEvent) + Send + 'static>) {}
