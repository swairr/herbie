# Electron → Tauri 重写计划

## 目标与成功标准

- **动机(已锁定):体积与常驻内存。** 量化验收:安装包从 electron-builder NSIS(~80MB 级)降至 <15MB 级;托盘常驻内存显著下降(前后 Task Manager 对比)。
- **功能等价:** 里程碑 1-3(待办、时间追踪、日志)行为不变,逐条对照 `docs/milestone-*-requirements.md` 走查。
- **数据零迁移:** 沿用现有 sqlite 文件与 schema。

## 已锁定决策(与用户逐条确认)

| # | 决策点 | 结论 |
|---|--------|------|
| 1 | 动机 | 体积与常驻内存 |
| 2 | DB 层 | 全量 Rust 化(rusqlite),无 Node sidecar |
| 3 | 纯逻辑归属 | 薄 Rust:Rust = SQL CRUD + 文件写入 + 原生集成;聚合/markdown 格式化留 TS(renderer 调用);labels 解析与 splitAtMidnight 为写/读双实现 |
| 4 | 推进策略 | 大爆炸 + 垂直切片验收;迁移期间冻结新功能 |
| 5 | 测试策略 | 现有 vitest 用例逐条翻译为 cargo test(测试名一一对应);双实现点共享 JSON 夹具跨语言对拍 |
| 6 | WebView2 | `downloadBootstrapper`(在线引导,安装包最小) |

计划直接定的小决策(无异议项):renderer 侧保留薄封装模块(形状等价现有 preload API),内部走 `invoke`,Vue 组件零改动;Tauri v2 稳定版;维持无单实例锁现状;数据路径显式沿用旧位置。

## 现状关键事实(已核实)

- main 进程 22 个 TS 模块;IPC 约 30 通道(`src/shared/ipc.ts`);renderer 经 preload 访问,不直触 Electron。
- DB:`app.getPath('userData')/herbie.db` = `%APPDATA%/herbie/herbie.db`,WAL + foreign_keys;3 个迁移文件(`migrations/*.sql`,`?raw` 内嵌 → Rust 用 `include_str!`)。
- 原生:`native/herbie-winhook/index.cpp` 仅 ~161 行(C++ N-API),SetWinEventHook 抓前台窗口;电源侧仅 4 事件(suspend/resume/lock/unlock)+ `getSystemIdleTime()`。
- Quick Add 窗口:`frame:false` + `alwaysOnTop` + `skipTaskbar`,blur 隐藏,全局快捷键唤起。
- 测试:20 个测试文件;仓库类约 12 个(内存库 + `runMigrations` 模式)需翻译为 Rust;shared 纯函数测试保留。
- 导出:markdown 字符串生成在 TS(shared/*-markdown.ts),Rust 只负责写文件;目录选择走 dialog。

## 能力映射表(Electron → Tauri/Rust)

| 现状 | 目标 |
|------|------|
| better-sqlite3(同步) | rusqlite(同步,语义最接近) |
| powerMonitor.getSystemIdleTime | Win32 `GetLastInputInfo` + tick 差值 |
| suspend/resume | 隐藏窗口 `WM_POWERBROADCAST`(PBT_APMSUSPEND / PBT_APMRESUMEAUTOMATIC) |
| lock/unlock | `WTSRegisterSessionNotification` + `WM_WTSSESSION_CHANGE` |
| herbie-winhook(N-API) | `windows-rs`:SetWinEventHook(EVENT_SYSTEM_FOREGROUND)+ GetWindowThreadProcessId + QueryFullProcessImageNameW |
| Tray / 菜单 | Tauri 内置 TrayIconBuilder + menu |
| 全局快捷键 | tauri-plugin-global-shortcut |
| dialog.pickDirectory | tauri-plugin-dialog |
| shell.openExternal / clipboard | tauri-plugin-shell / tauri-plugin-clipboard-manager |
| Quick Add(无边框/置顶/skipTaskbar/blur 隐藏) | Tauri window 配置 + focus 事件 |
| 事件类通道(quickadd:show/hide/blur、shortcut:error) | Tauri event emit/listen |
| 请求类通道(~26 个) | `#[tauri::command]` + invoke |

## 切片任务(有序,每片验收后再进下一片)

0. **脚手架 + 技术 spike**:Tauri v2 工程(`src-tauri/`)、renderer 切到纯 vite 构建、一条 ping command 打通;**spike 验证电源/锁屏事件方案**(message-only window 或子类化 tao 窗口句柄,与 Tauri 事件循环共存)——这是全计划最高技术风险点,先排雷。
1. **DB 骨架 + settings**:rusqlite 单例、迁移器(`include_str!`)、settings 仓库;翻译 settings/migrations 测试为 cargo test。
2. **todos + labels**:todos 仓库 + labels-store;#tag 解析 Rust 实现;共享 JSON 夹具与 TS `labels.ts` 对拍。
3. **segments + tracker + win 原生**:前台窗口钩子移植、GetLastInputInfo 空闲、电源 4 事件、tracker 状态机 Rust 重写(DI 测试同步翻译)、splitAtMidnight Rust 实现 + 夹具对拍。
4. **journal**:journals 仓库 + 测试翻译。
5. **export**:Rust 提供「拉当日数据」与「写文件」命令;markdown 生成留 TS;翻译 export 测试(TS 侧格式化测试保留)。
6. **外壳**:主窗口 + Quick Add(无边框/置顶/隐藏任务栏/blur 隐藏/快捷键唤起)、托盘菜单、全局快捷键注册与重注册、shortcut:error 事件。
7. **打包**:tauri NSIS、`downloadBootstrapper` WebView2、显式 DB 路径 `%APPDATA%/herbie/herbie.db`(沿用旧数据)、升级/卸载行为验证。
8. **清理与文档**:删除 electron/electron-builder/electron-vite/@electron-toolkit/*/better-sqlite3/native-(N-API);`pnpm test` 回归纯 Node vitest(去掉 ELECTRON_RUN_AS_NODE);更新 `AGENTS.md`(命令、架构、原生模块章节)与 `CONTEXT.md`。

## 验收(每切片)

- `cargo test` + `pnpm test`(纯 Node)+ `pnpm typecheck` + `pnpm lint` 全绿。
- 最终:新旧安装包体积、托盘常驻内存对比(记录数值)。

## 风险与对策

- **电源/锁屏事件与 Tauri 事件循环共存**(最高风险)→ 切片 0 spike 先行,不过不进入后续切片。
- **labels / splitAtMidnight 双实现漂移** → 共享 JSON 夹具对拍,任一侧改动必须同步夹具。
- **SQL 方言/类型差异** → 低(两侧同为 sqlite 同步 API);翻译测试即防护网。
- **Quick Add blur/快捷键时序** → 切片 6 对照现行 `windows.ts`/`shortcut.ts` 行为走查。
- **WAL 文件一致性** → 直接沿用旧路径,不做文件复制。

## Out of scope

- auto-updater、跨平台(维持 Windows-only)、安装器代码签名、单实例锁、任何新功能。
