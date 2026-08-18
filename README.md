# H.E.R.B.I.E

H.E.R.B.I.E 是用于待办跟踪、任务安排、时间记录、日志手账的个人助手。

## 框架与基础架构

- 基于 Tauri v2 + Rust + Vue 3 开发，Windows 平台
- 数据存于本地 SQLite（`%APPDATA%/herbie/herbie.db`），WAL + 外键，SQL 迁移体系管理 schema
- 驻留托盘：关闭窗口后停留在托盘，仅通过托盘菜单退出
- 全局快捷键唤醒 Quick Add 小窗口（默认 `Ctrl+Shift+Space`，可配置）
- 设置页：导出目录、快捷键、空闲阈值、片段防抖时长等可配置
- CI：Rust 单测 + 跨语言夹具对拍 + 绿色版 exe 发布流程

使用命令：

- `pnpm dev` — 开发模式（Tauri + Vite renderer）
- `pnpm build` — 构建并打包安装器
- `pnpm typecheck` / `pnpm lint` / `pnpm test` — 类型检查 / 代码检查 / 单测
- `cargo test --manifest-path src-tauri/Cargo.toml` — Rust 侧单测

## 待办 / 任务

- [x] 待办列表：未完成按创建时间倒序，完成项折叠分组
- [x] 快速添加：`Ctrl+Shift+Space` 唤醒小窗口，剪贴板内容作为 placeholder，`Tab` 填充/切焦点，`Enter` 提交，`Esc` 取消
- [x] 标题/详情行内编辑，记录创建/修改/完成时间
- [x] 软删除（数据保留，UI 不展示）
- [x] 链接识别：title / detail 中的 URL 可点击，用默认浏览器打开
- [x] Label 标签：详情中 `#tag` 解析，顶部 chips 多选并集过滤
- [x] 待办拖动排序（拖动手柄，仅未完成项）
- [x] 手动导出 markdown（`todos.md`，含稳定 id 注释）
- [ ] 输入时查找相似待办（尤其链接），右侧展开预览
- [ ] 任务依赖：设置前置/后置任务，树状依赖视图（科技树）
- [ ] 子任务拆解：主任务进度百分比
- [ ] 定期自动备份数据库（周期可配）

## 时间记录

- [x] 自动采集前台程序/窗口（`SetWinEventHook`），记录活动片段
- [x] 空闲检测（`GetLastInputInfo`）与睡眠/唤醒/锁屏事件，空闲记为 Idle 片段
- [x] 下班模式：暂停记录，下一次键鼠输入自动恢复（托盘 + 主界面入口）
- [x] 片段可编辑备注、搜索关联 Todo 归因
- [x] 日视图：聚合摘要（按进程 / 按关联 Todo 时长排行）+ 按时间顺序片段列表
- [x] 片段防抖（minSegmentSec 可配置），过滤 IME / 工具窗口
- [x] 手动导出 markdown（`time/YYYY-MM-DD.md`，按自然日切分）

## 日程安排

- [ ] 计划完成时间 / 计划时间段，超期标红，按周/日过滤
- [ ] 日历视图，拖拽修改开始/完成时间
- [ ] 番茄钟
- [ ] 报表：按时完成率、延迟次数统计等
- [ ] 外部日历同步
- [ ] 任务栏图标显示剩余待办数

## 日志 / 手账

- [x] 统一「日志条目」：归属自然日（天粒度），标题可选 + 正文必填，支持多行
- [x] 可补记过去日期、可改期；创建后随时编辑；软删除
- [x] 正文 `#tag` 与待办共享同一标签宇宙
- [x] 「日志」页签：按日导航查看，无标题条目以正文首行摘要展示
- [x] 手动导出 markdown（`journal/YYYY-MM-DD.md`），归纳总结由外部 AI 完成
