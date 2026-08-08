//! 外壳(切片6):Quick Add 窗口、主窗口托盘驻留、托盘菜单、全局快捷键。
//!
//! 行为对照 Electron `src/main/windows.ts` / `tray.ts` / `shortcut.ts` / `index.ts`:
//! - Quick Add:运行时创建的 `quickadd` 子窗口 —— 无边框/置顶/跳过任务栏/不可缩放,
//!   默认不可见;`show()` + `set_focus()` 后 emit `quickadd://show`;`hide()` 时
//!   emit `quickadd://hide`;失焦(`Focused(false)`)emit `quickadd://blur`
//!   (renderer 冲草稿)。URL 用默认 App URL(不加 hash),由 renderer `entry.ts`
//!   按窗口 label 重定向到 `#/quickadd`。
//! - 主窗口:关窗不退出(托盘驻留)。`CloseRequested` 在非退出期间
//!   `api.prevent_close()` + `hide()`;退出由托盘「退出」置 `QUITTING` 后
//!   `app.exit(0)`,`lib.rs` 的 `RunEvent::ExitRequested` 拆除追踪/快捷键。
//! - 全局快捷键:tauri-plugin-global-shortcut。注册失败写 `shortcutError` 设置并
//!   emit `shortcut://error`;成功清空错误设置。`settings_set` 改快捷键后重注册。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::settings;
use crate::tracker::{GlobalConn, ProdDeps};
use crate::TRACKER;

/// 全局退出标志:置位后主窗口 `CloseRequested` 不再拦截(放行销毁)。
/// 托盘「退出」先置位再 `app.exit(0)`;`lib.rs` 的 `RunEvent::ExitRequested` 也置位。
pub static QUITTING: AtomicBool = AtomicBool::new(false);

/// Quick Add 窗口 label(与 capabilities `windows` 白名单一致)。
pub const QUICK_ADD_LABEL: &str = "quickadd";

/// 当前已注册的快捷键字符串(None = 未注册)。`unregister` 用它反注册。
static CURRENT_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);

/// 托盘图标持有(进程生命周期内创建一次;下班 label 用 `set_text` 原地更新,
/// 避免在菜单事件回调内重建菜单 —— Tauri 已知 panic 模式)。
static TRAY: Mutex<Option<TrayIcon>> = Mutex::new(None);

/// 托盘「下班 / 恢复记录」菜单项引用:`refresh_tray_menu` 只 `set_text` 换 label。
static TRAY_TOGGLE_ITEM: Mutex<Option<MenuItem<tauri::Wry>>> = Mutex::new(None);

const TRAY_TOGGLE_ID: &str = "tray-toggle";
const TRAY_SHOW_ID: &str = "tray-show";
const TRAY_QUIT_ID: &str = "tray-quit";

/// Quick Add 是否已完成页面加载(对齐 Electron `quickAddReady`)。
static QUICK_ADD_READY: AtomicBool = AtomicBool::new(false);
/// 页面未就绪期间的 show 请求挂起(对齐 Electron `quickAddShowPending`)。
static QUICK_ADD_SHOW_PENDING: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// 纯函数(供托盘 label 与快捷键错误消息复用,可单测)
// ---------------------------------------------------------------------------

/// 托盘「下班 / 恢复记录」菜单 label,依 off-work 状态切换。
pub fn off_work_label(off_work: bool) -> &'static str {
    if off_work {
        "恢复记录"
    } else {
        "下班 / 停止记录"
    }
}

/// 快捷键注册失败的提示文案(对齐 Electron `shortcut.ts` 的消息)。
pub fn shortcut_error_message(accel: &str) -> String {
    format!("快捷键 \"{accel}\" 注册失败,请在设置中更换")
}

/// 判断外部 URL 是否允许由 `shell_open_external` 打开:仅 http/https/mailto,
/// 对齐 tauri-plugin-opener 的默认 scope(CVE-2025-20605 修复点)。手动提取 scheme,
/// 避免引入新依赖;含 scheme 语法校验(首字母 + [A-Za-z0-9+.-])。
pub fn is_external_url_allowed(url: &str) -> bool {
    let colon = match url.find(':') {
        Some(i) if i > 0 => i,
        _ => return false,
    };
    let scheme = &url[..colon];
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')) {
        return false;
    }
    matches!(scheme, "http" | "https" | "mailto")
}

// ---------------------------------------------------------------------------
// 主窗口:托盘驻留
// ---------------------------------------------------------------------------

/// 主窗口(label `main`,来自 tauri.conf.json)关窗拦截:
/// 非退出期间 `prevent_close` + `hide` —— 对齐 Electron `window-all-closed → hide`,
/// 仅托盘「退出」为真正退出路径。
pub fn setup_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let window_for_close = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window_for_close.hide();
                }
            }
        });
    }
}

/// 显示并聚焦主窗口(托盘「显示主窗口」/ 双击托盘)。
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

// ---------------------------------------------------------------------------
// Quick Add 窗口
// ---------------------------------------------------------------------------

/// 运行时创建 `quickadd` 子窗口(不入 tauri.conf.json,标签与 capabilities 白名单一致):
/// 无边框、置顶、跳过任务栏、不可缩放、默认不可见、460x360。
/// 事件接线:`Focused(false)` → emit `quickadd://blur`(renderer 冲草稿);
/// 页面加载完成(PageLoadEvent::Finished)→ 置 READY 并消费挂起的 show 请求。
pub fn create_quick_add_window(app: &AppHandle) {
    if app.get_webview_window(QUICK_ADD_LABEL).is_some() {
        return;
    }
    let window = WebviewWindowBuilder::new(app, QUICK_ADD_LABEL, WebviewUrl::default())
        .title("Quick Add")
        .inner_size(460.0, 360.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .on_page_load(move |win, payload| {
            use tauri::webview::PageLoadEvent;
            if payload.event() == PageLoadEvent::Finished {
                QUICK_ADD_READY.store(true, Ordering::SeqCst);
                if QUICK_ADD_SHOW_PENDING.swap(false, Ordering::SeqCst) {
                    show_quick_add_inner(&win);
                }
            }
        })
        .build()
        .expect("failed to create quick add window");

    let window_for_blur = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            let _ = window_for_blur.emit("quickadd://blur", ());
        }
    });
}

/// 唤起 Quick Add(全局快捷键入口):页面就绪则直接 show,否则挂起待加载完成后 show。
pub fn show_quick_add(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_ADD_LABEL) {
        if QUICK_ADD_READY.load(Ordering::SeqCst) {
            show_quick_add_inner(&window);
        } else {
            QUICK_ADD_SHOW_PENDING.store(true, Ordering::SeqCst);
        }
    }
}

fn show_quick_add_inner(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("quickadd://show", ());
}

/// 收起 Quick Add:窗口可见才 hide 并 emit `quickadd://hide`(不可见时幂等返回,
/// 避免 renderer `onHide → quickAddHide()` 的回环造成事件风暴)。
pub fn hide_quick_add(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_ADD_LABEL) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            let _ = window.emit("quickadd://hide", ());
        }
    }
}

// ---------------------------------------------------------------------------
// 托盘
// ---------------------------------------------------------------------------

/// 创建托盘图标 + 菜单事件分派(事件处理用固定 id 匹配,与菜单重建解耦)。
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    if TRAY.lock().expect("tray mutex poisoned").is_some() {
        return Ok(());
    }
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundle icon not embedded");
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Herbie")
        // 左键显示主窗口(对齐 Electron `tray.on('click')`),右键弹菜单。
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            if event.id() == TRAY_TOGGLE_ID {
                // 下班 ↔ 恢复:切换后原地更新菜单 label。
                if let Some(t) = TRACKER.get() {
                    let on = !t.get_off_work();
                    t.set_off_work(&GlobalConn, &mut ProdDeps, on);
                }
                let _ = refresh_tray_menu(app);
            } else if event.id() == TRAY_SHOW_ID {
                show_main_window(app);
            } else if event.id() == TRAY_QUIT_ID {
                QUITTING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
        })
        .build(app)?;
    *TRAY.lock().expect("tray mutex poisoned") = Some(tray);
    refresh_tray_menu(app)
}

/// 刷新托盘「下班 / 恢复记录」label:已建菜单则 `set_text` 原地更新(避免菜单事件
/// 回调内重建菜单的已知 panic 模式);尚未建菜单时兜底重建。
pub fn refresh_tray_menu(app: &AppHandle) -> tauri::Result<()> {
    let off_work = TRACKER.get().map(|t| t.get_off_work()).unwrap_or(false);
    if let Some(item) = TRAY_TOGGLE_ITEM.lock().expect("tray toggle mutex poisoned").as_ref() {
        return item.set_text(off_work_label(off_work));
    }
    let toggle = MenuItem::with_id(app, TRAY_TOGGLE_ID, off_work_label(off_work), true, None::<&str>)?;
    let show = MenuItem::with_id(app, TRAY_SHOW_ID, "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &separator, &show, &quit])?;
    *TRAY_TOGGLE_ITEM.lock().expect("tray toggle mutex poisoned") = Some(toggle);
    if let Some(tray) = TRAY.lock().expect("tray mutex poisoned").as_ref() {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 全局快捷键(tauri-plugin-global-shortcut 2.3.x)
// ---------------------------------------------------------------------------

/// 注册快捷键:读 `settings.shortcut`(默认 `Ctrl+Shift+Space`),空串则卸载。
/// 成功清空 `shortcutError`;失败写错误消息 + emit `shortcut://error`。
pub fn register_shortcut(app: &AppHandle) {
    unregister_shortcut(app);

    let accel = {
        let g = crate::db::get();
        let conn = match g.as_ref() {
            Some(c) => c,
            None => return,
        };
        settings::get_with_default(conn, settings::KEY_SHORTCUT)
    };
    if accel.trim().is_empty() {
        return;
    }

    let accel_clone = accel.clone();
    let result = app.global_shortcut().on_shortcut(
        accel.as_str(),
        move |app, _shortcut, event| {
            // 仅按下触发(避免释放事件重复唤起)。
            if event.state == ShortcutState::Pressed {
                show_quick_add(app);
            }
        },
    );

    match result {
        Ok(()) => {
            *CURRENT_SHORTCUT.lock().expect("shortcut mutex poisoned") = Some(accel_clone);
            let g = crate::db::get();
            if let Some(conn) = g.as_ref() {
                let _ = settings::set(conn, settings::KEY_SHORTCUT_ERROR, "");
            }
        }
        Err(_e) => {
            *CURRENT_SHORTCUT.lock().expect("shortcut mutex poisoned") = None;
            let msg = shortcut_error_message(&accel);
            let g = crate::db::get();
            if let Some(conn) = g.as_ref() {
                let _ = settings::set(conn, settings::KEY_SHORTCUT_ERROR, &msg);
            }
            let _ = app.emit("shortcut://error", msg);
        }
    }
}

/// 反注册当前快捷键(幂等)。
pub fn unregister_shortcut(app: &AppHandle) {
    if let Some(accel) = CURRENT_SHORTCUT.lock().expect("shortcut mutex poisoned").take() {
        let _ = app.global_shortcut().unregister(accel.as_str());
    }
}

/// 快捷键变更(settings_set)后重注册。
pub fn reregister_shortcut(app: &AppHandle) {
    register_shortcut(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_work_label_toggles_with_state() {
        assert_eq!(off_work_label(false), "下班 / 停止记录");
        assert_eq!(off_work_label(true), "恢复记录");
    }

    #[test]
    fn shortcut_error_message_embeds_accelerator() {
        let msg = shortcut_error_message("Ctrl+Shift+Space");
        assert!(msg.contains("Ctrl+Shift+Space"));
        assert!(msg.contains("注册失败"));
    }

    #[test]
    fn external_url_scheme_whitelist() {
        assert!(is_external_url_allowed("https://example.com/a#b"));
        assert!(is_external_url_allowed("http://example.com"));
        assert!(is_external_url_allowed("mailto:a@b.com"));
        assert!(!is_external_url_allowed("file:///C:/evil.exe"));
        assert!(!is_external_url_allowed("javascript:alert(1)"));
        assert!(!is_external_url_allowed("C:\\evil.exe"));
        assert!(!is_external_url_allowed("//relative"));
        assert!(!is_external_url_allowed(""));
        assert!(!is_external_url_allowed("1http://x"));
    }
}
