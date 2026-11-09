import type { JournalEntry } from './types'
import { localTimeOfDay } from './time'

// Pure markdown renderer for a single day's journal. §4 milestone-3 + ADR 0007: entries
// ordered by local time-of-day (explicit `time` when present, otherwise the local clock
// time of `createdAt`) ascending, soft-deleted excluded; each entry shows title (or body
// first line when no title), an optional `时间：HH:MM` line, the body, and a stable id in a
// trailing HTML comment mirroring todos.md for a future bidirectional sync upgrade path
// (ADR 0001).

function firstLine(body: string): string {
  const idx = body.indexOf('\n')
  return (idx === -1 ? body : body.slice(0, idx)).trim()
}

function restBody(body: string): string {
  const idx = body.indexOf('\n')
  return idx === -1 ? '' : body.slice(idx + 1)
}

/** Timeline position key: explicit wall-clock time, else the local time of `createdAt`. */
export function journalSortTime(e: Pick<JournalEntry, 'time' | 'createdAt'>): string {
  const explicit = e.time && e.time.trim().length > 0 ? e.time.trim() : null
  return explicit ?? localTimeOfDay(e.createdAt) ?? ''
}

/** Ascending order used by both `list_journals` and the exported markdown. */
export function compareJournals(a: JournalEntry, b: JournalEntry): number {
  const at = journalSortTime(a)
  const bt = journalSortTime(b)
  if (at < bt) return -1
  if (at > bt) return 1
  return a.createdAt < b.createdAt ? -1 : a.createdAt > b.createdAt ? 1 : 0
}

function renderEntry(e: JournalEntry): string[] {
  const title = e.title && e.title.trim().length > 0 ? e.title.trim() : null
  const head = (title ?? firstLine(e.body)) || '(无内容)'
  const block = title ? e.body : restBody(e.body)
  const out: string[] = [`## ${head} <!-- id:${e.id} -->`]
  const explicit = e.time && e.time.trim().length > 0 ? e.time.trim() : null
  if (explicit) out.push('', `时间：${explicit}`)
  const trimmed = block.trim()
  if (trimmed.length > 0) {
    out.push('', trimmed)
  }
  return out
}

export function exportJournalMarkdown(day: string, entries: JournalEntry[]): string {
  const sorted = entries.filter((e) => !e.deletedAt).sort(compareJournals)

  const lines: string[] = [`# 日志 ${day}`, '']
  for (const e of sorted) {
    lines.push(...renderEntry(e))
    lines.push('')
  }
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n'
}
