import { getDb } from './db-access'
import { parseLabels } from '@shared/labels'
import { nowIso, localDateString, dayBounds } from '@shared/time'
import { updateJournalLabels } from './labels-store'
import type { JournalEntry, JournalInput, JournalPatch } from '@shared/types'
import { randomUUID } from 'node:crypto'

export function rowToJournal(r: {
  id: string
  title: string | null
  body: string
  date: string
  createdAt: string
  updatedAt: string
  deletedAt: string | null
}): JournalEntry {
  return {
    id: r.id,
    title: r.title,
    body: r.body,
    date: r.date,
    createdAt: r.createdAt,
    updatedAt: r.updatedAt,
    deletedAt: r.deletedAt
  }
}

export function listJournals(day: string): JournalEntry[] {
  const db = getDb()
  const rows = db
    .prepare(
      `SELECT * FROM journal_entries
       WHERE date = ? AND deletedAt IS NULL
       ORDER BY createdAt ASC`
    )
    .all(day) as any[]
  return rows.map(rowToJournal)
}

export function createJournal(input: JournalInput): JournalEntry {
  const db = getDb()
  const body = input.body == null ? '' : input.body
  if (body.trim().length === 0) {
    throw new Error('journal body must not be empty')
  }
  const date =
    input.date && input.date.trim().length > 0
      ? input.date.trim()
      : localDateString(nowIso())
  if (!dayBounds(date)) throw new Error(`invalid date: ${date}`)

  const titleRaw = input.title == null ? '' : input.title
  const title = titleRaw.trim().length > 0 ? titleRaw.trim() : null

  const id = randomUUID()
  const now = nowIso()
  const stmt = db.prepare(
    `INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
     VALUES (?, ?, ?, ?, ?, ?, NULL)`
  )
  const tx = db.transaction(() => {
    stmt.run(id, title, body, date, now, now)
    updateJournalLabels(id, parseLabels(body))
  })
  tx()
  const row = db.prepare('SELECT * FROM journal_entries WHERE id = ?').get(id) as any
  return rowToJournal(row)
}

export function updateJournal(id: string, patch: JournalPatch): JournalEntry {
  const db = getDb()
  const existing = db.prepare('SELECT * FROM journal_entries WHERE id = ?').get(id) as any
  if (!existing) throw new Error('journal not found: ' + id)

  const title =
    patch.title !== undefined
      ? patch.title == null || patch.title.trim().length === 0
        ? null
        : patch.title.trim()
      : existing.title

  let body = existing.body
  if (patch.body !== undefined) {
    if (patch.body.trim().length === 0) {
      throw new Error('journal body must not be empty')
    }
    body = patch.body
  }

  const date = patch.date !== undefined ? patch.date : existing.date
  if (!dayBounds(date)) throw new Error(`invalid date: ${date}`)

  const now = nowIso()
  const tx = db.transaction(() => {
    db.prepare(
      'UPDATE journal_entries SET title = ?, body = ?, date = ?, updatedAt = ? WHERE id = ?'
    ).run(title, body, date, now, id)
    updateJournalLabels(id, parseLabels(body))
  })
  tx()
  const row = db.prepare('SELECT * FROM journal_entries WHERE id = ?').get(id) as any
  return rowToJournal(row)
}

export function softDeleteJournal(id: string): void {
  const db = getDb()
  db.prepare('UPDATE journal_entries SET deletedAt = ?, updatedAt = ? WHERE id = ?').run(
    nowIso(),
    nowIso(),
    id
  )
}