-- migration 0006: optional local wall-clock time on journal entries
--
-- 日志条目可选记录本地墙钟时刻 HH:MM(ADR 0007)。有 time 的条目落在当天
-- 时间线的日志窄轨;NULL 表示保持天粒度,归入「全天」区。
-- 只加可空列,无回填、无索引;既有日志行为不变。

ALTER TABLE journal_entries ADD COLUMN time TEXT;
