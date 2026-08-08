//! 空闲秒:`GetLastInputInfo` + `GetTickCount64` 差值(对应 Electron
//! `powerMonitor.getSystemIdleTime`,计划能力映射表第 36 行)。
//!
//! `GetLastInputInfo` 的 `dwTime` 是 32 位 tick,约 49.7 天回绕;对 `GetTickCount64`
//! 的低 32 位做模 2^32 差值即得到正确的毫秒空闲时长(空闲 < 49.7 天前提)。

#![allow(dead_code)]

use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// 毫秒差值 → 秒(截断,下限 0)。纯函数,可单测。
pub fn tick_delta_to_sec(delta_ms: u64) -> u64 {
    delta_ms / 1000
}

/// 自上次键盘/鼠标输入以来的秒数。失败(理论上不发生)返 0。
pub fn get_idle_sec() -> u64 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    let ok = unsafe { GetLastInputInfo(&mut info) };
    if !ok.as_bool() {
        return 0;
    }
    let tick = unsafe { GetTickCount64() };
    let delta = tick.wrapping_sub(info.dwTime as u64) & 0xFFFF_FFFF;
    tick_delta_to_sec(delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_delta_to_sec_truncates() {
        assert_eq!(tick_delta_to_sec(0), 0);
        assert_eq!(tick_delta_to_sec(999), 0);
        assert_eq!(tick_delta_to_sec(1000), 1);
        assert_eq!(tick_delta_to_sec(60_999), 60);
        assert_eq!(tick_delta_to_sec(3_600_000), 3600);
    }
}
