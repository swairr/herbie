-- migration 0003: journal entries (milestone 3)

CREATE TABLE IF NOT EXISTS journal_entries (
  id          TEXT PRIMARY KEY,
  title       TEXT,                            -- optional; NULL when absent (CONTEXT.md)
  body        TEXT NOT NULL,                    -- required, multiline
  date        TEXT NOT NULL,                    -- local natural day "YYYY-MM-DD" (ADR 0003)
  createdAt   TEXT NOT NULL,
  updatedAt   TEXT NOT NULL,
  deletedAt   TEXT
);

CREATE INDEX IF NOT EXISTS idx_journal_entries_date ON journal_entries(date);
CREATE INDEX IF NOT EXISTS idx_journal_entries_createdAt ON journal_entries(createdAt);

CREATE TABLE IF NOT EXISTS journal_labels (
  journalId TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
  label     TEXT NOT NULL,
  PRIMARY KEY (journalId, label)
);
CREATE INDEX IF NOT EXISTS idx_journal_labels_label ON journal_labels(label);