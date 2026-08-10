-- migration 0004: manual sort order for pending todos (drag to reorder)
--
-- sortOrder 越小越靠前(升序展示)。仅 pending(completedAt IS NULL 且未软删)
-- 参与手动排序;新建待办取 MIN(sortOrder) - 1 置顶。
-- 回填:按既有展示顺序(createdAt DESC)赋 1..n 的连续序号,行为与迁移前一致。

ALTER TABLE todos ADD COLUMN sortOrder REAL NOT NULL DEFAULT 0;

UPDATE todos
SET sortOrder = (
  SELECT COUNT(*)
  FROM todos AS t2
  WHERE t2.deletedAt IS NULL
    AND t2.completedAt IS NULL
    AND (t2.createdAt > todos.createdAt
         OR (t2.createdAt = todos.createdAt AND t2.rowid <= todos.rowid))
)
WHERE deletedAt IS NULL AND completedAt IS NULL;
