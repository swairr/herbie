# H.E.R.B.I.E — Agent 说明

## 语言

所有回复、解释、计划清单必须使用中文（代码、专业术语和文件名除外）。

## 命令

- `pnpm dev` — 以开发模式运行应用。
- `pnpm build` — 类型检查 + 将 main/preload/renderer 构建到 `out/`。
- `pnpm typecheck` — 对 main/preload (`tsc`) 与 renderer (`vue-tsc`) 做类型检查;不产出文件。
- `pnpm lint` — 对 `src/` 与 `tests/` 跑 ESLint（flat config,`eslint.config.js`）。
- `pnpm test` — 跑一次 Vitest 单元 + 集成测试。测试在 **Electron 作为 Node 运行**
  （`ELECTRON_RUN_AS_NODE=1 electron ...vitest`）,这样为 Electron 的 Node 编译的
  `better-sqlite3` 原生模块（ABI 匹配 Electron 的 Node）才能加载。在系统 Node 下直接跑
  `vitest` 会因原生模块 ABI 不匹配而失败 —— 请用 `pnpm test`。
- `pnpm test:watch` — Vitest watch 模式（同样在 Electron 下运行）。
- `pnpm rebuild` — 针对已安装的 Electron 重新构建 `better-sqlite3` 原生模块。
- `pnpm build:installer` — 构建并通过 electron-builder 打包安装器。

## 架构

- 行为的事实来源:`docs/milestone-1-requirements.md` 与实现计划。`CONTEXT.md` 定义领域
  术语表（Todo / Label / Quick Add）。
- Electron 主进程持有 `better-sqlite3` 单例及所有持久化逻辑。Renderer 不接触 Node。
  `contextIsolation: true`、`sandbox: true`、`nodeIntegration: false`。
- `src/main/db-access.ts` 是一个 **不依赖 Electron** 的 DB 单例持有者,使得各仓库
  （`todos`、`settings`、`labels-store`、`export`）能在纯 Node 环境下通过内存数据库
  `new Database(':memory:')` + `runMigrations` 做单元测试。`src/main/db.ts` 是唯一绑定
  Electron 的模块（用到 `app.getPath`),在 ready 时通过 `setDb` 注入单例。
- 共享纯代码（`src/shared/`）被 main 与 renderer 共同引用（labels、time、markdown）。
  保持它们无依赖且有单元测试。
- 迁移以有序 SQL 文件形式位于 `migrations/`（通过 `?raw` 内嵌）;在启动时由
  `src/main/migrations.ts` 中的迁移运行器执行,后者接受一个 `Database` 参数,因此测试
  可直接调用。

## 与需求的偏差

`milestone-1-requirements.md` §5 要求启动时自动导出。用户已改写为 **仅手动导出**。不要在
`app.whenReady` 中加入导出调用。重新启用自动导出的唯一接入点在 `src/main/index.ts`
（见注释）。

## 里程碑 2 — 时间追踪

- **原生模块 `herbie-winhook`** 是位于 `native/herbie-winhook/` 的树内 N-API 模块
  （仅 Windows,C++ + `node-addon-api`）。它作为本地文件依赖接入 `package.json`,并列入
  `pnpm.onlyBuiltDependencies`。构建它需要 **Visual Studio Build Tools (C++) + Python**,
  并在 `postinstall` / `pnpm rebuild` 时由 `electron-rebuild` 编译。所需原生库:
  `user32`、`psapi`、`kernel32`（在 `binding.gyp` 中声明）。
- 若原生模块缺失或构建失败,`src/main/index.ts` 中的 `loadWinHook()` 会降级为空操作通知器,
  应用继续运行 **但不记录 segment** —— 这是为了跨平台开发而有意为之。
- **驻留托盘的行为变更**:里程碑 2 让应用在最后一个窗口被关闭时停留在托盘中
  （`window-all-closed` 现在在 Windows 上是隐藏主窗口而非退出）。退出只通过托盘的 退出
  项发生（`before-quit` 会拆除追踪 + 关闭 DB）。这是相对里程碑 1 的行为变更。
- Segment 追踪位于 `src/main/segments.ts`（状态机）+ `src/main/tracker.ts`
  （空闲轮询 + 下班,**依赖注入** 以保持可单元测试）。聚合在 `src/shared/segments-agg.ts`
  中为纯函数;跨天切分在 `src/shared/time.ts` 中为纯函数（`splitAtMidnight`）。
  `powerMonitor` 只能 `app.whenReady` 内部触碰。
