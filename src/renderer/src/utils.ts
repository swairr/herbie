import type { Todo } from '@shared/types'

export const DEFAULT_SHORTCUT = 'Ctrl+Shift+Space'

export function groupItems(todos: Todo[]): { pending: Todo[]; done: Todo[] } {
  const pending: Todo[] = []
  const done: Todo[] = []
  for (const t of todos) {
    if (t.deletedAt) continue
    if (t.completedAt) done.push(t)
    else pending.push(t)
  }
  return { pending, done }
}

export type Segment = { type: 'text'; value: string } | { type: 'url'; value: string }

export function splitLinks(text: string): Segment[] {
  const urlRe = /\bhttps?:\/\/[^\s]+/g
  const out: Segment[] = []
  let last = 0
  let m: RegExpExecArray | null
  while ((m = urlRe.exec(text)) !== null) {
    if (m.index > last) out.push({ type: 'text', value: text.slice(last, m.index) })
    out.push({ type: 'url', value: m[0] })
    last = m.index + m[0].length
  }
  if (last < text.length) out.push({ type: 'text', value: text.slice(last) })
  return out
}