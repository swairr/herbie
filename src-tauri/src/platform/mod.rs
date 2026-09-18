//! 平台集成门面:追踪系统所需原生能力的唯一出口,平台差异收敛于此。
//!
//! - Windows(`crate::win/`):前台钩子 / 空闲秒 / 电源与锁屏事件,追踪完整可用。
//! - Linux(`crate::linux/`):追踪整体未实现(ADR 0004),仅声明不可用原因与空实现。
//!
//! `lib.rs` 只依赖本模块(不直接引用 `win/`);未来实现 Linux 追踪时填充 `linux/`,
//! 核心状态机与接线不动。

#[cfg(windows)]
mod imp {
    pub use crate::win::foreground::ForegroundHook;
    pub use crate::win::idle::get_idle_sec;
    pub use crate::win::power::start_power_watcher;
    pub use crate::win::tracking_unavailable_reason;
}

#[cfg(target_os = "linux")]
mod imp {
    pub use crate::linux::{
        get_idle_sec, start_power_watcher, tracking_unavailable_reason, ForegroundHook,
    };
}

pub use imp::*;

/// 自动更新自替换能力:Windows 绿色版 exe 支持;Linux 降级为手动下载(ADR 0004)。
/// `update_apply` 据此早退;检查链路(`update_check`)不受影响、平台无关。
#[cfg(windows)]
pub fn auto_update_supported() -> bool {
    true
}

#[cfg(target_os = "linux")]
pub fn auto_update_supported() -> bool {
    false
}

/// 追踪能力状态,序列化 camelCase(`{ available, reason }`),对齐 renderer 的
/// `TrackingStatus`(见 `src/shared/types.ts`)。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingStatus {
    pub available: bool,
    pub reason: Option<&'static str>,
}

/// 组合门面状态:available ⇔ 平台未声明不可用原因。
pub fn tracking_status() -> TrackingStatus {
    match tracking_unavailable_reason() {
        Some(reason) => TrackingStatus {
            available: false,
            reason: Some(reason),
        },
        None => TrackingStatus {
            available: true,
            reason: None,
        },
    }
}
