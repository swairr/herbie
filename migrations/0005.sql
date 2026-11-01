-- migration 0005: planned slot on todos (milestone 4 scheduling)
--
-- 计划时段使用本地墙钟:日期 YYYY-MM-DD + 可选 HH:MM 起止时刻(ADR 0005)。
-- 三列为 NULL 表示未安排;有日期无时刻表示当天「全天 / 未定时间」。
-- 一个 Todo 最多一个计划时段,故直接挂在 todos 上,不建独立表。

ALTER TABLE todos ADD COLUMN plannedDate TEXT;
ALTER TABLE todos ADD COLUMN plannedStart TEXT;
ALTER TABLE todos ADD COLUMN plannedEnd TEXT;

CREATE INDEX IF NOT EXISTS idx_todos_plannedDate ON todos(plannedDate);
