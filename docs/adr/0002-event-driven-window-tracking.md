# 事件驱动采集前台窗口，自写 N-API 原生模块

里程碑二的时间记录采用**纯事件驱动**采集：自写 node-addon-api 原生模块，在独立线程跑 Win32 消息循环并注册 `SetWinEventHook`（监听 `EVENT_SYSTEM_FOREGROUND` 前台切换，以及过滤到当前前台窗口的 `EVENT_OBJECT_NAMECHANGE` 标题变化），事件经 ThreadSafeFunction 回调到 JS，前台进程或标题变化时关闭当前活动片段并开启新片段。

选择事件驱动而非轮询采样，是因为轮询（每秒一行或按间隔合并）要么数据量大、要么在边界处损失精度，事件驱动天然产生精确的活动片段。选择自写 N-API 而非 koffi/ffi-napi 或独立辅助进程，是因为 `SetWinEventHook` 的回调必须运行在打带消息循环的线程上，FFI 方案在该场景限制多、稳定性存疑；辅助进程则多一个构件，打包分发与生命周期管理变复杂。项目已有 electron-rebuild 编译链路（better-sqlite3），新增一个原生模块的边际成本低，代价是需维护约百行 C++。

空闲判定不违背事件驱动方向：窗口变化走钩子，"人离开了"没有 Win32 事件，由主进程低频轮询 `powerMonitor.getSystemIdleTime`（`GetLastInputInfo` 的封装）补足。
