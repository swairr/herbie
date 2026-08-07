//! 前台窗口钩子事件 + 通知器抽象,对应 TS `src/main/segments.ts` 的
//! `WinHookEvent` 与 `HookNotifier`。本切片(3b)仅定义类型与 trait;真实原生
//! SetWinEventHook 接入留 3c(连同 segments.test.ts 剩余 5 条用例)。

/// 前台窗口事件。`kind` 对齐 TS `"foreground" | "namechange"`;`hwnd` 为窗口句柄;
/// `process_name` 为可执行文件名;`title` 为窗口标题。
pub struct WinHookEvent {
    pub kind: &'static str,
    pub hwnd: i64,
    pub process_name: String,
    pub title: String,
}

/// 通知器抽象:外部把"当捕获到一个 WinHookEvent 时如何处理"以 `FnMut` 回调注入,
/// `start` 启动原生事件监听,`stop` 停止。3c 由 windows-rs 的 SetWinEventHook 实现;
/// 3b 仅有测试用 `FakeHook` 实现。
pub trait HookNotifier {
    fn start(&mut self, cb: Box<dyn FnMut(WinHookEvent) + Send + 'static>);
    fn stop(&mut self);
}