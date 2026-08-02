-- migration 0001: initial schema

CREATE TABLE todos (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  detail      TEXT NOT NULL DEFAULT '',
  createdAt   TEXT NOT NULL,
  updatedAt   TEXT NOT NULL,
  completedAt TEXT,
  deletedAt   TEXT
);

CREATE INDEX idx_todos_createdAt ON todos(createdAt);
CREATE INDEX idx_todos_completedAt ON todos(completedAt);

CREATE TABLE todo_labels (
  todoId  TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
  label   TEXT NOT NULL,
  PRIMARY KEY (todoId, label)
);
CREATE INDEX idx_todo_labels_label ON todo_labels(label);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS migrations (
  version   INTEGER PRIMARY KEY,
  appliedAt TEXT NOT NULL
);