# 里程碑一 需求方案

目标：搭建应用框架，跑通"快速添加 → 列表管理 → 本地存储 → markdown 导出"的最小闭环。

技术栈：Electron + TypeScript + better-sqlite3。

领域术语见根目录 `CONTEXT.md`；架构决策见 `docs/adr/`。

## 1. 主界面：待办列表

- pending（未完成）按**创建时间倒序**排列在上方。
- done（已完成）折叠为底部一个分组，默认收起，可展开；done 可一键改回 pending。
- 顶部一行 label chips：点选过滤列表，多选为**并集（OR）**，再点取消。
- 删除为软删除，UI 不展示已删除项。
- 非编辑态下，title 和 detail 中的 `http(s)://` URL 渲染为可点击链接，点击用系统默认浏览器打开（`shell.openExternal`）。

### 编辑

- 点击列表项展开，title / detail 可直接编辑保存。
- 保存时重新解析 label、更新 `updatedAt`。
- 展开态同时承载完成 / 取消完成 / 删除操作入口。

## 2. Quick Add 小窗口

- 全局快捷键唤醒，默认 `Ctrl+Shift+Space`，可在设置中改键；注册失败时启动提示。
- 窗口置顶；失焦自动收起，**草稿保留**，下次唤醒恢复未提交内容。
- 唤醒时读取剪贴板：内容显示为 title 输入框的 placeholder。
- 第一次 `Tab`：剪贴板内容填入 title，光标留在 title（可继续修改）。
- 第二次 `Tab`：焦点切换到 detail 输入框。
- `Enter` 提交：`title` 必填，空 title 时抖动提示、不提交；提交成功显示简短确认后收起窗口。
- `Esc` 取消并关闭窗口。

## 3. Label（标签）

- 唯一事实源是 `detail` 文本中的 `#tag` 标记（如 `#工作`）；title 不参与解析。
- 数据库冗余存储解析结果用于查询过滤；修改 detail 即修改标签，保存时重新解析。
- 一个 Todo 可有 0..n 个标签；同名 `#tag` 视为同一 Label。

## 4. 本地存储

- sqlite（better-sqlite3），存于用户数据目录。
- Todo 字段：`id`、`title`、`detail`、`createdAt`、`updatedAt`、`completedAt`、`deletedAt`。
- 状态由 `completedAt` 派生，不单独存状态字段。

## 5. Markdown 导出（单向，见 ADR 0001）

- sqlite 是唯一事实源，markdown 为导出产物，外部修改不回流。
- 触发方式：手动触发 + 每次应用启动时自动导出一次。
- 导出目录用户可配置（指向一个 git 仓库目录）。
- 格式：单个 `todos.md`，每条 Todo 一个任务列表项：
  - `- [ ] 标题 <!-- id:xxx -->`（pending）/ `- [x] 标题 <!-- id:xxx -->`（done）
  - detail 作为缩进内容跟在该项下方，含创建/完成时间。
- 稳定 id 保留在行尾 HTML 注释中，为未来升级双向同步留路径。

## 里程碑一明确不做

- 双向 markdown 同步（只留升级路径）
- 相似待办查找、任务依赖、子任务拆解（后续里程碑）
- 计划时间、日历视图、番茄钟、报表、外部日历同步（后续里程碑）
- 已删除 Todo 的恢复界面（数据保留即可）
