-- migration 0002: activity/idle segment tracking (milestone 2)

CREATE TABLE IF NOT EXISTS segments (
  id          TEXT PRIMARY KEY,
  startAt     TEXT NOT NULL,
  endAt       TEXT,                    -- NULL = currently open segment
  processName TEXT NOT NULL DEFAULT '',
  title       TEXT NOT NULL DEFAULT '', -- immutable snapshot at open time
  note        TEXT NOT NULL DEFAULT '', -- user-editable
  todoId      TEXT REFERENCES todos(id) ON DELETE SET NULL,
  kind        TEXT NOT NULL DEFAULT 'activity' -- 'activity' | 'idle'
);

CREATE INDEX IF NOT EXISTS idx_segments_start ON segments(startAt);
CREATE INDEX IF NOT EXISTS idx_segments_todo  ON segments(todoId);
CREATE INDEX IF NOT EXISTS idx_segments_end   ON segments(endAt);