import type { Todo } from './types'
import { formatLocalShort } from './time'

// Pure markdown exporter. Single source ordering: pending (createdAt desc) then
// done (completedAt desc). Soft-deleted todos are excluded. Stable id is preserved
// in a trailing HTML comment for a future bidirectional sync upgrade path (ADR 0001).

function indent(line: string): string {
  return '    ' + line
}

function renderItem(t: Todo): string[] {
  const box = t.completedAt ? '[x]' : '[ ]'
  const head = `- ${box} ${t.title} <!-- id:${t.id} -->`
  const out = [head]
  if (t.detail.trim().length > 0) {
    for (const ln of t.detail.split('\n')) out.push(indent(ln))
  }
  out.push(indent(`Created: ${formatLocalShort(t.createdAt)}`))
  if (t.completedAt) {
    out.push(indent(`Completed: ${formatLocalShort(t.completedAt)}`))
  }
  return out
}

export function exportMarkdown(todos: Todo[]): string {
  const pending = todos
    .filter((t) => !t.deletedAt && !t.completedAt)
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
  const done = todos
    .filter((t) => !t.deletedAt && t.completedAt)
    .sort((a, b) => (b.completedAt || '').localeCompare(a.completedAt || ''))

  const lines: string[] = ['# Todos', '']
  for (const t of pending) {
    lines.push(...renderItem(t))
    lines.push('')
  }
  if (pending.length && done.length) lines.push('')
  for (const t of done) {
    lines.push(...renderItem(t))
    lines.push('')
  }
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n'
}