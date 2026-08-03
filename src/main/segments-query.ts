import { getDb } from './db-access'
import type { Segment, SegmentPatch } from '@shared/types'
import { splitAtMidnight, dayBounds } from '@shared/time'
import { rowToSegment } from './segments-row'

// List all segments whose interval overlaps the given local natural day, clipped to that
// day via splitAtMidnight (so cross-midnight segments appear once per day, only with the
// portion that belongs to that day). Open segments (endAt NULL) are clamped to "now".
export function listSegmentsByDay(localDate: string, now: Date = new Date()): Segment[] {
  const bounds = dayBounds(localDate)
  if (!bounds) return []
  const db = getDb()
  // Rows are returned ORDER BY startAt ASC (idx_segments_start); splitAtMidnight preserves
  // that ordering (a slice's start is clamped to [seg.start, dayStart] with a stable key,),
  // so no additional in-memory sort is needed.
  const rows = db
    .prepare(
      `SELECT * FROM segments
       WHERE startAt < ? AND (endAt IS NULL OR endAt > ?)
       ORDER BY startAt ASC`
    )
    .all(bounds.endIso, bounds.startIso) as any[]
  const out: Segment[] = []
  for (const r of rows) {
    const seg = rowToSegment(r)
    const slice = splitAtMidnight(seg, localDate, now)
    if (slice) out.push(slice)
  }
  return out
}

export function updateSegment(id: string, patch: SegmentPatch): Segment | null {
  const db = getDb()
  const existing = db.prepare('SELECT * FROM segments WHERE id = ?').get(id) as any
  if (!existing) return null
  const note = patch.note != null ? patch.note : existing.note
  const todoId =
    patch.todoId !== undefined ? (patch.todoId === '' ? null : patch.todoId) : existing.todoId
  db.prepare('UPDATE segments SET note = ?, todoId = ? WHERE id = ?').run(note, todoId, id)
  return rowToSegment(db.prepare('SELECT * FROM segments WHERE id = ?').get(id) as any)
}

// Resolve { todoId -> todo title } for a set of segment rows. Missing todos are omitted
// (they are shown as "(已删除)" by the aggregation layer).
export function fetchTodoTitles(todoIds: string[]): Record<string, string> {
  const ids = Array.from(new Set(todoIds.filter(Boolean)))
  if (ids.length === 0) return {}
  const placeholders = ids.map(() => '?').join(',')
  const rows = getDb()
    .prepare(`SELECT id, title FROM todos WHERE id IN (${placeholders})`)
    .all(...ids) as { id: string; title: string }[]
  const out: Record<string, string> = {}
  for (const r of rows) out[r.id] = r.title
  return out
}