import type { Segment } from './types'
import { durationMs } from './time'

export interface AggregateRow {
  key: string
  label: string
  ms: number
}

// Group segments by processName and sum elapsed milliseconds. Idle segments
// (kind === 'idle') are excluded from the process ranking — they are not work.
export function aggregateByProcess(segments: Segment[], now: Date = new Date()): AggregateRow[] {
  const map = new Map<string, number>()
  for (const s of segments) {
    if (s.kind === 'idle') continue
    const key = s.processName || '(unknown)'
    map.set(key, (map.get(key) ?? 0) + durationMs(s.startAt, s.endAt, now))
  }
  return toSortedRows(map)
}

// Group segments by their associated todo. Segments without a todoId are bucketed
// under "(未关联)". Both activity and idle segments contribute time here so the user
// sees total wall-clock spent per todo.
export function aggregateByTodo(
  segments: Segment[],
  todoTitles: Record<string, string>,
  now: Date = new Date()
): AggregateRow[] {
  const map = new Map<string, { label: string; ms: number }>()
  const UNASSIGNED = '(未关联)'
  for (const s of segments) {
    const id = s.todoId ?? ''
    const label = id ? todoTitles[id] ?? '(已删除)' : UNASSIGNED
    const key = id || UNASSIGNED
    const acc = map.get(key) ?? { label, ms: 0 }
    acc.label = label
    acc.ms += durationMs(s.startAt, s.endAt, now)
    map.set(key, acc)
  }
  return Array.from(map.entries())
    .map(([key, v]) => ({ key, label: v.label, ms: v.ms }))
    .sort((a, b) => b.ms - a.ms || a.label.localeCompare(b.label))
}

// Group segments by processName for the idle bucket. Returns a single aggregate for
// all idle segments so the day view can show "下线/空闲" totals separately.
export function aggregateIdle(segments: Segment[], now: Date = new Date()): number {
  let total = 0
  for (const s of segments) {
    if (s.kind !== 'idle') continue
    total += durationMs(s.startAt, s.endAt, now)
  }
  return total
}

function toSortedRows(map: Map<string, number>): AggregateRow[] {
  return Array.from(map.entries())
    .map(([key, ms]) => ({ key, label: key, ms }))
    .sort((a, b) => b.ms - a.ms || a.label.localeCompare(b.label))
}