# 领域文档

探索代码库时，工程类 skill 应如何消费本仓库的领域文档。

## 探索之前先读这些

- 根目录的 **`GLOSSARY.md`**（本仓库为单上下文，没有 `GLOSSARY-MAP.md`）。
- **`docs/adr/`**：阅读与即将改动的区域相关的 ADR。

若这些文件不存在，**静默跳过**。不要指出其缺失，也不要主动建议创建——`/domain-modeling` skill（经由 `/grill-with-docs` 与 `/improve-codebase-architecture` 触达）会在术语或决策真正定下来时按需创建。

## 文件结构

单上下文仓库：

```
/
├── GLOSSARY.md
├── docs/adr/
│   ├── 0001-one-way-markdown-export.md
│   └── ...
└── src/
```

## 使用术语表的词汇

当你的产出提到某个领域概念时（issue 标题、重构提案、假设、测试名），使用 `GLOSSARY.md` 中定义的术语。不要漂移到术语表明确回避的同义词。

若你需要的概念还不在术语表中，这是一个信号：要么你在发明项目不使用的语言（重新考虑），要么确有缺口（记为 `/domain-modeling` 的输入）。

## 标记 ADR 冲突

若你的产出与既有 ADR 矛盾，明确点出来，而不是悄悄覆盖：

> _与 ADR-0007（event-sourced orders）矛盾，但值得重新讨论，因为……_
