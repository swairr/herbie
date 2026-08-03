# 里程碑二实施计划 — 活动片段时间记录

目标见 `docs/milestone-2-requirements.md`。本计划基于现有里程碑一代码库扩展，遵循 ADR 0001/0002。所有新代码遵循仓库现有约定：shared 纯函数 + 单测；main 持有 DB 单例；renderer 不触 Node。

## 关键决策（已与用户确认）

1. **原生模块**: 自写 in-tree N-API 原生模块 `native/herbie-winhook/`（binding.gyp + node-addon-api C++），随 `electron-rebuild` 在 `postinstall` 编译。仅暴露 `start(cb)` / `stop()`，事件经 ThreadSafeFunction 回调 JS（`{ type: 'foreground'|'namechange', hwnd, processName, title }`）。从不承载业务逻辑。
2. **开段的存储表示**: 切换前台/标题时立即 INSERT 新片段（`endAt = NULL`）并 UPDATE 上一行的 `endAt`。"当前片段"即 `endAt IS NULL` 的行。崩溃后保留至最后一次事件的数据。
3. **空闲检测**: Electron `powerMonitor.getSystemIdleTime` 低频 `setInterval` 轮询（默认 20s）。阈值默认 300s，存于 settings(`idleThresholdSec`)。空闲时在 `now - idleTime`（最后一次输入时刻）关闭当前段，并写入一条 `kind='idle'` 的空闲片段。
4. **下班恢复**: 复用空闲轮询 + 前台钩子。下班期间不写入任何片段；当收到前台切换事件 *或* `getSystemIdleTime` 由 >0 变为 0 时，结束下班恢复记录。锁屏/睡眠（powerMonitor `lock`/`suspend`）立即关闭当前段且不产生空闲片段。
5. **跨午夜切分**: 不改写存储；切片在 `src/shared/time.ts` 增加的纯函数 `splitAtMidnight(seg, day)` 中完成，日视图查询与导出共用。
6. **托盘**: 新增 `src/main/tray.ts`，菜单项「下班 / 恢复记录」(toggle) + 「退出」。

## 任务清单

### T1 — 数据库迁移 0002
新增 `migrations/0002.sql`（在 `src/main/migrations.ts` 注册）：
```sql
CREATE TABLE segments (
  id          TEXT PRIMARY KEY,
  startAt     TEXT NOT NULL,
  endAt       TEXT,                    -- NULL = 当前开段
  processName TEXT NOT NULL DEFAULT '',
  title       TEXT NOT NULL DEFAULT '', -- 不可变快照
  note        TEXT NOT NULL DEFAULT '', -- 可编辑
  todoId      TEXT REFERENCES todos(id) ON DELETE SET NULL,
  kind        TEXT NOT NULL DEFAULT 'activity' -- 'activity' | 'idle'
);
CREATE INDEX idx_segments_start ON segments(startAt);
CREATE INDEX idx_segments_todo  ON segments(todoId);
CREATE INDEX idx_segments_end   ON segments(endAt);
```
`endAt` 为开段标记；`todoId` 软引用（删除 Todo 时 SET NULL 保留片段历史）。`kind` 区分空闲片段（聚合摘要中可独立或并入；按进程聚合时空闲不计入进程排行——见 T6）。在 `tests/migrations.test.ts` 补 idempotent + 表存在性。

### T2 — 原生模块 herbie-winhook
新增 `native/herbie-winhook/`：
- `binding.gyp`（target_name='herbie_winhook'，deps `node-addon-api`，`node-addon-api` 与 `nothing`）。在根 `package.json` 的 `onlyBuiltDependencies` 追加 `"herbie-winhook"`，`postinstall`/`rebuild` 覆盖它。
- `index.cpp`（~100 行）：独立 std::thread 跑 `SetWinEventHook`（`EVENT_SYSTEM_FOREGROUND` + `EVENT_OBJECT_NAMECHANGE` 过滤到当前前台 hwnd），`UnhookWinEvent` + `PostQuitMessage` 退出。用 `Napi::ThreadSafeFunction` 把事件 push 回 JS。前台进程名经 `GetWindowThreadProcessId` + `QueryFullProcessImageName` 取 exe 基名。
- `index.js`：`module.exports = { start(cb) / stop() }`，`start` 返回的 `cb` 携带 `{ type, hwnd, processName, title }`。
- `index.d.ts`：导出 `WinHookEvent` 类型与 `start/stop` 签名，供 `src/main` 强类型导入。
- `.gitignore` 追加 `native/herbie-winhook/build/`.
- 在 README/AGENTS.md 补一句 Windows-only 原生模块的 rebuild 提示。

不写原生模块的单元测试；事件→片段的业务逻辑全部放在可测的纯 TS 层（T3）。原生模块仅做一次手测：`pnpm rebuild && pnpm dev`，alt-tab 时控制台应见事件。

### T3 — 片段业务层 `src/main/segments.ts`
事件驱动状态机封装，**不直接 import** 原生模块（由 `index.ts` 注入回调以保可测）：
- `startTracking(notifier)`：调用 `notifier.start(cb)`，cb 内处理事件：
  - 任何 foreground / namechange：`closeOpen(now)` + `openSegment({processName,title})`.
  - 注意 namechange 仅当 hwnd == 当前前台时处理（与原生侧过滤一致）。
- `openSegment(input)`：`INSERT segments(id, startAt=now, endAt=NULL, kind='activity', ...)`.
- `closeOpen(at)`：`UPDATE segments SET endAt=? WHERE endAt IS NULL`。
- `stopTracking()`：`closeOpen(now)` + `notifier.stop()`。
- 幂等：`closeOpen` 若无开段则空操作。
导出供 T4（idle/off-work）调用的 `closeOpen`。新文件 `tests/segments.test.ts`：mock notifier，断言事件序列产生正确的开/关段、空闲片段、kind 字段。

### T4 — 空闲与下班 `src/main/tracker.ts`
聚合空闲轮询、powerMonitor 事件、下班状态：
- `startTracker()`: 注册 `setInterval(20s)` 轮询 `powerMonitor.getSystemIdleTime`；订阅 `powerMonitor` 的 `suspend`/`lock`→`closeOpen(now)`；`resume`/`unlock` 不主动开段（等下一次前台事件自然开）。
- 空闲超阈值：`closeOpen(now - idleSec*1000)`，然后 `INSERT ... kind='idle'`（processName='[idle]'，title=''），endAt 仍 NULL（开段）。下次有输入（前台事件或 idle 归零）时关闭该空闲段。
- **下班状态机** `setOffWork(on)`：on → `closeOpen(now)` 并设标志；off-work 期间 T3 的 cb 收到事件也忽略、轮询也跳过写入。前台事件 / idleTime→0 触发 `setOffWork(false)` 恢复。
- 暴露 `getOffWork()` 给托盘/IPC。
- 测试：mock powerMonitor（在测试中由 tracker 接口注入 `getIdle`/事件触发函数），断言开段/关段/idle 段/下班切换。**不能**实际依赖 Electron `powerMonitor`——用依赖注入让纯逻辑可测。

### T5 — IPC + 类型和 preload 扩展
- `src/shared/ipc.ts` 增：`segments`(`list` by day `YYYY-MM-DD`、`update` patch note/todoId)、`export`(`exportTime`)、`tracker`(`getOffWork` / `setOffWork`)、`settings` 已有。
- `src/shared/types.ts` 增 `Segment`/`SegmentPatch`/`OffWorkState`/day 查询入参/导出结果；扩展 `Api` 接口。
- `src/preload/index.ts` 增对应 `ipcRenderer.invoke` 包装。
- `src/main/ipc.ts` 注册新 handlers。

### T6 — segments 仓库查询 `src/main/segments-query.ts`
- `listSegmentsByDay(localDate: 'YYYY-MM-DD')`：取 `startAt` 落在该本地自然日的片段；用 `splitAtMidnight`（T7）把跨午夜段切成两半（只返回属于该日的部分），按 `startAt` 升序。
- `aggregateByProcess(segments)`：按 processName 求时长（毫秒）；**空闲段(kind='idle')不计入进程排行**。
- `aggregateByTodo(segments)`：按 todoId 求时长，无关联的归入「未关联」。
- 三者均为纯函数（接受 segments 数组），放 `src/shared/segments-agg.ts`（dependency-free、单测）以便 main/renderer 复用聚合逻辑；`segments-query.ts` 只负责 DB IO + split 装配。
- `tests/segments-agg.test.ts`：聚合与空闲排除、跨午夜切片归属、未关联桶。

### T7 — 纯时区工具 `src/shared/time.ts`
- `localDateString(iso)` → `'YYYY-MM-DD'`（本地）。
- `splitAtMidnight(seg, localDate)` → `{ start, end }`：若 seg 跨过该日 00:00 则 S 截到 00:00、E 从 00:00 起；返回属于该日的子片段（可能为 null）。本地时区靠 `Date` 的本地方法。补单测覆盖「整段在日内」「跨午夜跨两日」「两段拼接」。
- `formatDuration(ms)` → `'Hh Mm'` 文案（聚合/导出复用）。

### T8 — markdown 时间导出 `src/shared/time-markdown.ts` + `src/main/export-time.ts`
- 纯函数 `exportTimeMarkdown(day, segments)`：标题 `# 时间记录 YYYY-MM-DD`，两段时长表（进程排行、Todo 排行），随后片段清单（时间段、进程、标题、note 优先于 processName/title、关联 Todo title）。跨午夜段先过 split。
- `src/main/export-time.ts`：mirror `export.ts` 结构：`buildTimeContent(day)` / `resolveExportDir()`（复用 settings） / `writeTimeFile(dir, day, content)`（`time/YYYY-MM-DD.md`，mkdir recursive）/ `exportTime(day)`。
- 单测 `tests/export-time.test.ts`：纯函数渲染 + 跨午夜切片；`tests/export-time.test.ts` 中 DB 集成用 `makeDb` 注入若干段断言文件路径/内容。

### T9 — UI：时间页签与片段编辑 `src/renderer/src/views/TimeView.vue` + App 路由
- `App.vue` 增顶层 tab：「待办」(`#/main`) / 「时间」(`#/time`)，hash 路由。
- `TimeView.vue`：日期选择（默认今天，`<` `>` 切换日，加一个日历 input type=date 回退）；顶部两块聚合表卡（进程时长排行、Todo 时长排行）；下方片段列表（时间段 + processName/title 或 note + 关联 Todo title）。
- 点击片段弹编辑层：note 输入 + Todo 搜索框（`window.api.todos.list({ })` 后按 title 客户端过滤，或新增 `todos.search(q)` —— 复用现有 `list` 取全量前端 filter 最简，v1 不加新 IPC）。保存 → `segments.update`。
- 「导出时间记录」按钮 → `export.exportTime(currentDay)`，复用 ListView 的 toast 模式。
- 主界面顶栏加「下班」按钮（与设置 ⚙ 同行）：显示当前下班态，点击 toggle（`tracker.setOffWork`），onMounted 拉取 `tracker.getOffWork`。
- 复用 `style.css` 变量与组件风格；新增 `components/SimpleTable.vue`（小排行表，可选）。

### T10 — 托盘 `src/main/tray.ts`
- 在 `index.ts` `app.whenReady` 内 `createTray()`。`Tray` + 生成的占位图标（用 `nativeImage.createFromBuffer` 配一段 1x1 PNG 常量，避免新增资源文件依赖；或把 16x16 PNG 放 `src/main/assets/`——视简洁度二选一）。
- 菜单：「下班 / 恢复记录」（点击调 `setOffWork(!getOffWork())`，菜单项 label 实时反映状态）、「退出」。下班态变化通过 `tray.setContextMenu` 重渲染或 IPC 推送给主窗口按钮同步。
- `app.on('window-all-closed'` 不退出（改为隐藏到托盘，Windows）：里程碑二要求托盘常驻——这里需谨慎：现有逻辑 `window-all-closed` 在非 darwin 下 `app.quit()`。改为：主窗口关 → `mainWindow.hide()`；仅托盘「退出」真正 quit。此改动需用户确认对里程碑一行为的影响（见风险）。

### T11 — settings 扩展
- `Settings` 增 `idleThresholdSec: string`（默认 '300'）。`SettingsView.vue` 增一项数字输入「空闲阈值（秒）」。运行时 `tracker` 通过 `getSettingWithDefault('idleThresholdSec')` 读取；需要 `settings.set` 后通知 tracker 重读——复用现有 `ipcMain.on(IPC.settings.set,...)` 模式扩展到 `idleThresholdSec` 触发 tracker 重配轮询间隔/阈值（实现里拿出一个 `reregisterTracker` 钩子）。

## 接线点（`src/main/index.ts`）
`app.whenReady` 内在 `createMainWindow` 之后：`createTray()`、`startTracking()`（注入原生模块的 `start/stop`）、`startTracker()`（注入 `getIdle` 与 `onPowerEvent` 适配，便于测试）。`before-quit`/`window-all-closed-tray-quit`：`stopTracking()` / `stopTracker()` / `closeDb`。**不**在启动时调用 `exportTime`（与里程碑一对齐：手动导出）。

## 验证计划
- `pnpm typecheck`、`pnpm lint`、`pnpm test`（Electron-as-Node vitest）全绿。
- 新增单测：migrations v2、segments 状态机、tracker 空闲/下班、time splitAtMidnight、segments-agg、time-markdown、export-time。
- 手测脚本：`pnpm rebuild && pnpm dev`，确认：(1) alt-tab 产生片段；(2) 5 分钟无操作产生 idle 段；(3) 时间页签显示当日聚合 + 列表；(4) 编辑 note/todoId 持久化；(5) 下班按钮/托盘切下班→无新段→动鼠标恢复；(6) 「导出时间记录」生成 `time/YYYY-MM-DD.md` 与本地日切分一致；(7) 锁屏/睡眠即时关闭当前段。

## 风险
- **托盘改变 `window-all-closed` 语义**：里程碑一主窗口关闭即退出。改为常驻托盘需用户确认是否接受行为变更（不影响数据）。若不接受，托盘菜单仍可保留，但窗口关闭继续 `app.quit()`——下班/恢复将仅由主窗口按钮控制，托盘「退出」冗余但仍可提供。**待用户拍板，默认按"常驻托盘"实现并在 PR 描述里标注行为变更。**
- 原生模块 CI 编译：仓库 CI 未见（无 .github）；首次本地编译依赖 VS Build Tools + Python。已在 AGENTS.md 记录依赖提示。
- `QueryFullProcessImageName` 需进程对当前用户可读；通常前台进程均满足，但提权进程会取不到 exe 名——回退为空字符串，不影响事件流。
- `powerMonitor` 在 `app.whenReady` 之前不可用——确保 `startTracker` 在 `whenReady` 内调用。

## 不做（对齐 requirements §里程碑二明确不做）
隐私过滤/打码、片段编辑器（拆分合并边界）、甘特条、短片段折叠、非 Windows、相似 Todo/依赖（里程碑三）、计划/日历/番茄/报表/日历同步（里程碑四）。