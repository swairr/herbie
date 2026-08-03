import type { JournalEntry } from './types'

// Pure markdown renderer for a single day's journal. §4 milestone-3: entries ordered by
// createdAt ascending, soft-deleted excluded; each entry shows title (or body first line
// when no title), the body, and a stable id in a trailing HTML comment mirroring todos.md
// for a future bidirectional sync upgrade path (ADR 0001).

function firstLine(body: string): string {
  const idx = body.indexOf('\n')
  return (idx === -1 ? body : body.slice(0, idx)).trim()
}

function restBody(body: string): string {
  const idx = body.indexOf('\n')
  return idx === -1 ? '' : body.slice(idx + 1)
}

function renderEntry(e: JournalEntry): string[] {
  const title = e.title && e.title.trim().length > 0 ? e.title.trim() : null
  const head = (title ?? firstLine(e.body)) || '(无内容)'
  const block = title ? e.body : restBody(e.body)
  const out: string[] = [`## ${head} <!-- id:${e.id} -->`]
  const trimmed = block.trim()
  if (trimmed.length > 0) {
    out.push('', trimmed)
  }
  return out
}

export function exportJournalMarkdown(day: string, entries: JournalEntry[]): string {
  const sorted = entries
    .filter((e) => !e.deletedAt)
    .sort((a, b) => a.createdAt.localeCompare(b.createdAt))

  const lines: string[] = [`# 日志 ${day}`, '']
  for (const e of sorted) {
    lines.push(...renderEntry(e))
    lines.push('')
  }
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n'
}