import { getDb } from './db-access'

// Re-parse and store labels for a todo: delete existing rows then insert the new set.
export function updateTodoLabels(todoId: string, labels: string[]): void {
  const db = getDb()
  const del = db.prepare('DELETE FROM todo_labels WHERE todoId = ?')
  const ins = db.prepare('INSERT OR IGNORE INTO todo_labels (todoId, label) VALUES (?, ?)')
  const tx = db.transaction(() => {
    del.run(todoId)
    for (const label of labels) ins.run(todoId, label)
  })
  tx()
}

export function labelsForTodo(todoId: string): string[] {
  const rows = getDb()
    .prepare('SELECT label FROM todo_labels WHERE todoId = ? ORDER BY label')
    .all(todoId) as { label: string }[]
  return rows.map((r) => r.label)
}

// §3 milestone-3: Journal Entry body tags live in the same Label universe as Todo detail
// tags. Re-parse and store labels for a journal entry: delete existing rows then insert.
export function updateJournalLabels(journalId: string, labels: string[]): void {
  const db = getDb()
  const del = db.prepare('DELETE FROM journal_labels WHERE journalId = ?')
  const ins = db.prepare('INSERT OR IGNORE INTO journal_labels (journalId, label) VALUES (?, ?)')
  const tx = db.transaction(() => {
    del.run(journalId)
    for (const label of labels) ins.run(journalId, label)
  })
  tx()
}

export function labelsForJournal(journalId: string): string[] {
  const rows = getDb()
    .prepare('SELECT label FROM journal_labels WHERE journalId = ? ORDER BY label')
    .all(journalId) as { label: string }[]
  return rows.map((r) => r.label)
}