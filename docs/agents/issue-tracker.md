# 议题追踪器：GitHub

本仓库的议题与规格以 GitHub Issues 形式存在。所有操作使用 `gh` CLI。

## 约定

- **创建议题**：`gh issue create --title "..." --body "..."`。多行正文用 heredoc。
- **读取议题**：`gh issue view <number> --json number,title,body,labels,comments`。
- **列出议题**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，配以合适的 `--label` / `--state` 过滤。
- **设为子议题**：`gh issue create --parent <parent> ...`，或事后 `gh issue edit <parent> --add-sub-issue <child>`（`gh` 2.94+）。旧版 `gh`：`gh api --method POST repos/<owner>/<repo>/issues/<parent>/sub_issues -F sub_issue_id=<child-db-id>`（数据库 id）。不支持子议题时，在子议题正文顶部写 `Part of #<parent>`。
- **阻塞**：使用 GitHub 原生议题依赖（UI 可见的规范表示）。用 `gh issue create --blocked-by <n>,<n> ...` 或 `gh issue edit <child> --add-blocked-by <blocker>`（`gh` 2.94+）添加。旧版：`gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`，其中 `<blocker-db-id>` 是阻塞者的数字**数据库 id**（`gh api repos/<owner>/<repo>/issues/<n> --jq .id`，不是 `#number` 或 `node_id`）。依赖不可用时，退化为正文顶部的 `Blocked by: #<n>, #<n>`。所有阻塞者关闭后即解除阻塞。
- **统计未关闭的阻塞者**：`gh api repos/<owner>/<repo>/issues/<n> --jq .issue_dependencies_summary.blocked_by`（仅未关闭；0 表示可开始）。`gh issue view <n> --json blockedBy` 也会列出已关闭者，需过滤：`--jq '[.blockedBy.nodes[] | select(.state == "OPEN")] | length'`。
- **评论**：`gh issue comment <number> --body "..."`。
- **加 / 移除标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`。
- **关闭**：先 `gh issue comment <number> --body "..."` 说明原因，再 `gh issue close <number>`。对已关闭的议题，`gh issue close --comment` 会丢弃评论但仍返回 0。

从 `git remote -v` 推断仓库；在克隆目录内运行时 `gh` 会自动推断。

只向用户自己的仓库提议题。绝不要向 `mattpocock/skills`（这些 skill 的来源仓库）提，除非用户拥有它。在 fork 中 `gh` 可能默认指向上游父仓库，所以每次会话首次 `gh issue create` 前，用 `gh repo view --json nameWithOwner` 检查目标，若不是用户仓库就先确认。

## PR 作为 triage 表面

**PR 作为请求表面：否。**（若本仓库把外部 PR 当作功能请求，改成 `yes`；`/triage` 会读取此标记。）

设为 `yes` 时，PR 走与议题相同的标签和状态，使用 `gh pr` 等价命令：

- **读取 PR**：`gh pr view <number>`，再 `gh pr view <number> --comments`（非 TTY 下 `--comments` 只打印评论，不含正文），diff 用 `gh pr diff <number>`。
- **列出待 triage 的外部 PR**：`gh api --paginate 'repos/{owner}/{repo}/pulls?state=open' --jq '.[] | select(.author_association | IN("OWNER","MEMBER","COLLABORATOR") | not) | {number, title, author: .user.login, author_association, labels: [.labels[].name]}'`。
- **评论 / 加标签 / 关闭**：`gh pr comment`、`gh pr edit --add-label`/`--remove-label`、`gh pr close`。

GitHub 的议题与 PR 共用编号空间，裸 `#42` 可能是任一种：先用 `gh pr view 42` 解析，失败再回退 `gh issue view 42`。

## 当 skill 说「发布到议题追踪器」

创建一个 GitHub issue。

## 当 skill 说「取出相关工单」

按上文 **读取议题** 读取。

## Wayfinding 操作

供 `/wayfinder` 使用。**地图**是单个 issue，**子 issue** 作为工单。

- **地图**：带 `wayfinder:map` 标签的单个 issue，正文承载 Notes / Decisions-so-far / Fog。`gh issue create --label wayfinder:map`。
- **子工单**：作为 GitHub 子议题链接到地图（见 **设为子议题**）。不支持子议题时，把子议题加入地图正文的任务列表，并在子议题正文顶部写 `Part of #<map>`。标签：`wayfinder:<type>`（`research`/`prototype`/`grilling`/`task`）。认领后指派给驱动开发者。
- **阻塞**：见 **约定** 下的 **阻塞**。
- **前沿查询**：列出地图的未关闭子议题（`gh issue list --state open`，限定为地图的子议题 / 任务列表），剔除有未关闭阻塞者（见 **统计未关闭的阻塞者**，或 `Blocked by` 行中有未关闭 issue）或有指派者的；按地图顺序取第一个。
- **认领**：`gh issue edit <n> --add-assignee @me`，这是会话的第一次写入。
- **解决**：`gh issue comment <n> --body "<answer>"`，再 `gh issue close <n>`，然后把上下文指针（要点 + 链接）追加到地图的 Decisions-so-far。
