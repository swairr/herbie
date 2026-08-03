import type { Segment } from './types'
import { aggregateByProcess, aggregateByTodo, aggregateIdle } from './segments-agg'
import { formatLocalShort, formatDuration } from './time'

function esc(s: string): string {
  return s.replace(/\|/g, '\\|').replace(/\n/g, ' ')
}

// Pure renderer for a single day's time log. Both aggregate tables and the per-segment
// listing are derived purely from the input array + a {todoId -> title} map, so this is
// testable without fs or electron.
export function exportTimeMarkdown(
  day: string,
  segments: Segment[],
  todoTitles: Record<string, string> = {}
): string {
  const procRows = aggregateByProcess(segments)
  const todoRows = aggregateByTodo(segments, todoTitles)
  const idleMs = aggregateIdle(segments)

  const lines: string[] = [`# 时间记录 ${day}`, '']

  lines.push('## 进程时长排行', '')
  lines.push('| 进程 | 时长 |', '| --- | --- |')
  if (procRows.length === 0) lines.push('| (无) | 0m |')
  for (const r of procRows) lines.push(`| ${esc(r.label)} | ${formatDuration(r.ms)} |`)
  lines.push('')

  lines.push('## Todo 时长排行', '')
  lines.push('| Todo | 时长 |', '| --- | --- |')
  if (todoRows.length === 0) lines.push('| (无) | 0m |')
  for (const r of todoRows) lines.push(`| ${esc(r.label)} | ${formatDuration(r.ms)} |`)
  lines.push('')

  if (idleMs > 0) {
    lines.push(`> 空闲合计：${formatDuration(idleMs)}`, '')
  }

  lines.push('## 片段清单', '')
  lines.push('| 时间段 | 进程 | 标题 / note | 关联 Todo |', '| --- | --- | --- | --- |')
  if (segments.length === 0) lines.push('| (无片段) | | | |')
  for (const s of segments) {
    const range = `${formatLocalShort(s.startAt)} - ${formatLocalShort(s.endAt)}`
    const display = s.note.trim().length > 0 ? s.note : s.title || s.processName
    const todo = s.todoId ? todoTitles[s.todoId] ?? '(已删除)' : ''
    lines.push(`| ${range} | ${esc(s.processName) || '-'} | ${esc(display)} | ${esc(todo)} |`)
  }
  lines.push('')

  return lines.join('\n').trimEnd() + '\n'
}