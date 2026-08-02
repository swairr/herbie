# H.E.R.B.I.E 领域术语表

个人待办助手：待办跟踪、任务安排、时间记录。

## Todo（待办）

一条待办事项。核心字段：

- `title` — 标题，必填，快速输入时的第一行
- `detail` — 详细内容，可选，多行文本
- `createdAt` — 创建时间
- `updatedAt` — 最后修改时间
- `completedAt` — 完成时间，为空表示未完成
- `deletedAt` — 删除时间（软删除），为空表示未删除

### 状态

Todo 只有两个生命周期状态，由 `completedAt` 派生，不单独存状态字段：

- **pending（进行中）** — `completedAt` 为空
- **done（已完成）** — `completedAt` 非空；可一键改回 pending（清空 `completedAt`）

删除为**软删除**：设置 `deletedAt`，UI 默认不展示，数据保留可恢复。

## Label（标签）

用于 Todo 分类的标记。**纯派生数据**：唯一事实源是 `detail` 文本中以 `#` 前缀书写的标记（如 `#工作`），title 不参与解析。

- 数据库可为查询过滤冗余存储解析结果，但事实源永远是 detail 文本；修改 detail 即修改标签。
- 一个 Todo 可有 0..n 个 Label；不同 Todo 的同名 `#tag` 视为同一 Label。
- 没有独立的"重命名标签"操作 —— 改名即修改所有含该标记的 detail 文本。

## Quick Add（快速添加）

通过全局快捷键唤醒的小窗口，用于以最少按键创建 Todo。利用剪贴板内容加速输入；`title` 必填，空标题不允许提交。
