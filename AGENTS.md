# H.E.R.B.I.E — Agent 说明

## 语言

所有回复、解释、计划清单必须使用中文（代码、专业术语和文件名除外）。

## 命令

- `pnpm dev` — 以开发模式运行应用（`tauri dev`,Rust + WebView2 + vite renderer）。
- `pnpm build` — 构建并打包 Tauri 安装器（`tauri build`,Windows NSIS）。
- `pnpm typecheck` — 对 `src/shared` / `tests`（`tsc`）与 renderer（`vue-tsc`）做类型检查;不产出文件。
- `pnpm lint` — 对 `src/` 与 `tests/` 跑 ESLint（flat config,`eslint.config.mjs`）。
- `pnpm test` — 跑一次 Vitest,纯 Node 环境（保留的 shared 纯函数测试 + 双实现点夹具对拍）。
- `pnpm test:watch` — Vitest watch 模式。
- `pnpm build:renderer` — 单独构建 renderer 到 `dist/renderer`（vite,`vite.renderer.config.ts`）。
- `cargo test --manifest-path src-tauri/Cargo.toml` — 跑 Rust 侧全部单测（当前 143 条;Windows 与 Linux 均可跑）。
- Linux 绿色版打包：`pnpm tauri build --no-bundle` + CI `build-linux` action（tar.gz:二进制 + `.desktop` + 图标）。
- `pnpm tauri ...` — 透传 Tauri CLI（如 `pnpm tauri dev`、`pnpm tauri build`、`pnpm tauri icon`）。

## 架构

- 行为的事实来源:`docs/milestone-*-requirements.md` 与实现计划。`GLOSSARY.md` 定义领域
  术语表（Todo / Label / Quick Add / Activity Segment / Journal Entry 等）。
- **Rust 主进程**持有 `rusqlite` 单例（`src-tauri/src/db.rs`）。数据根目录平台化：Windows
  沿用旧路径 `%APPDATA%/herbie/herbie.db`（刻意不用 Tauri 默认 `app_data_dir`,避免与旧数据错位）;
  Linux `$XDG_DATA_HOME/herbie/herbie.db`（默认 `~/.local/share/herbie/`）。打开时启用
  WAL + `foreign_keys`,并跑迁移。导出默认目录与 DB 同根（`db::herbie_dir()`）。
- 迁移以有序 SQL 文件位于仓库根 `migrations/`,由 `src-tauri/src/migrations.rs`
  的迁移运行器执行（`include_str!` 内嵌）,接受 `&mut Connection` 参数,测试可直接调用。
- **锁模型**:全局 `Mutex<Option<Connection>>`;命令层取 `db::get()` 的守卫并把
  `&Connection` 传给仓储函数,仓储层**不自锁**（避免重入死锁）,互斥保证同一时刻仅一个
  命令访问 DB（对齐 better-sqlite3 同步单连接语义）。
- 共享纯逻辑（`src/shared/`）:标签解析 / 时间 / 聚合 / markdown,被 Rust 与 renderer
  共同引用或双实现。**双实现点**（`labels` 解析、`splitAtMidnight`）用
  `tests/fixtures/*.json` 在 cargo test 与 vitest 之间跨语言对拍;任一侧改动必须同步夹具。
  其余 TS 纯函数（markdown 生成、聚合）仅 renderer 侧保留,行为由 vitest 覆盖。
- renderer（`src/renderer`,vite）经 `window.api` 薄封装
  （`src/renderer/src/api/tauri.ts`）走 `invoke`（请求类命令）/ `listen`（事件类）,
  形状与旧 preload API 等价,**Vue 组件零改动**。

## 原生模块与平台层

- **平台门面** `src-tauri/src/platform/`：追踪系统所需原生能力的唯一出口
  （`get_idle_sec` / 前台钩子 / 电源监听 / 追踪可用性 / 自更新支持）;`lib.rs` 只依赖
  门面,不直接引用具体平台模块。未来实现 Linux 追踪 = 填充 `linux/`,核心状态机不动。
- **Windows**（`src-tauri/src/win/`,`windows-rs`,依赖在 `[target.'cfg(windows)'.dependencies]`）:
  前台窗口钩子 `foreground.rs`（`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` + 进程名）;
  空闲检测 `idle.rs`（`GetLastInputInfo`）;电源事件 `power.rs`（隐藏窗口 `WM_POWERBROADCAST`
  睡眠/唤醒 + `WTSRegisterSessionNotification` 锁屏/解锁）。这是电源事件的唯一实现。
- **Linux**（`src-tauri/src/linux/`）:追踪三件套整体未实现,`tracking_unavailable_reason()`
  返回原因,`start_tracking_system` 由守卫直接 no-op;UI 显式呈现 Tracking Unavailable
  （见 `docs/adr/0004-linux-tracking-unavailable.md`）。自动更新仅检查、不支持自替换。
- Tracker 状态机在 `src-tauri/src/tracker.rs`（空闲轮询 + 下班,依赖注入以保持可单元
  测试）;前台事件与电源事件经通知器喂给 Tracker。聚合在 `src/shared/segments-agg.ts`
  为纯函数;跨天切分在 `src/shared/time.ts` 为纯函数（`splitAtMidnight`,与 Rust 对拍）。

## 与需求的偏差

`milestone-1-requirements.md` §5 要求启动时自动导出。用户已改写为 **仅手动导出**。不要
在启动流程中加入导出调用。

## 里程碑 2 行为约定

- **驻留托盘**:应用在最后一个窗口被关闭时停留在托盘中（隐藏主窗口而非退出）。退出只
  通过托盘的 退出 项发生（`before-quit` 等价流程:拆除追踪 + 关闭 DB）。
- **仅手动导出**:导出手动触发;不要在启动时自动导出。
- **Linux 追踪不可用**:Linux 上不接线任何追踪原生能力（无活动片段产生）,以主窗口横幅 /
  托盘 tooltip / 设置页显式提示（ADR 0004）;「下班」入口与托盘菜单项禁用置灰。不要在
  Linux 上“顺便”实现部分追踪（前台 / 空闲 / 电源任一）——整体推迟是有意决策。

## Agent skills

### Issue tracker

议题与规格以 GitHub Issues 形式存放在 `swairr/herbie`，统一用 `gh` CLI 操作。见 `docs/agents/issue-tracker.md`。

### Triage labels

五个 triage 角色沿用默认标签名（`needs-triage` / `needs-info` / `ready-for-agent` / `ready-for-human` / `wontfix`）。见 `docs/agents/triage-labels.md`。

### Domain docs

单上下文（single-context）：根目录 `GLOSSARY.md` + `docs/adr/`。见 `docs/agents/domain.md`。
