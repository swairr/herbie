// 日程时间线的展示层纯函数:把当天的 Activity Segment 聚合成「实际轨」块、
// 按 todoId 汇总实际时长与计划配对,并把可选时刻的 Journal Entry 放到时间轴上。
//
// 这些是展示规则,不写回数据库:原始片段仍是唯一事实源,聚合块可展开回原始片段。
// 时间语义:片段是 UTC 瞬时,块坐标用运行时本地墙钟分钟(跨午夜由调用方先用
// `splitAtMidnight` 切到当天);日志 `time` 本身已是本地墙钟(ADR 0007)。

import type { JournalEntry, Segment } from './types'
import { durationMs, localDateString } from './time'
import { layoutRanges, timeToMinutes, type MinuteRange } from './schedule'

/** 相邻同组片段允许合并的最大间隙(毫秒);超过即视为两次不同活动。 */
export const MERGE_GAP_MS = 2 * 60 * 1000

export interface ActualBlock extends MinuteRange {
  /** 聚合块的唯一 id(同一组可因间隙拆成多块)。 */
  id: string
  /** 归组键(待办 / 进程 / 空闲),用于配对与着色。 */
  key: string
  label: string
  /** 无关联待办时为 null。 */
  todoId: string | null
  /** 块内全部片段都是空闲。 */
  idle: boolean
  /** 最近片段的 note/title,作为进程块的副标题。 */
  detail: string
  startAt: string
  endAt: string
  /** 片段时长之和,不含合并间隙。 */
  durationMs: number
  /** 展开后的原始片段(按 startAt 升序)。 */
  segments: Segment[]
  lane: number
  lanes: number
}

interface DraftBlock {
  key: string
  label: string
  todoId: string | null
  detail: string
  startAt: string
  endAt: string
  startMs: number
  endMs: number
  durationMs: number
  segments: Segment[]
}

function groupOf(
  s: Segment,
  todoTitles: Record<string, string>
): { key: string; label: string; todoId: string | null } {
  if (s.todoId) {
    return {
      key: `todo:${s.todoId}`,
      label: todoTitles[s.todoId] ?? '(已删除)',
      todoId: s.todoId
    }
  }
  if (s.kind === 'idle') return { key: 'idle', label: '空闲', todoId: null }
  const proc = s.processName || '(未知进程)'
  return { key: `process:${proc}`, label: proc, todoId: null }
}

/** 本地墙钟分钟;跨入前一天/后一天时夹到 0 / 1440,保证午夜切片画满全天。 */
function minuteInDay(iso: string, day: string): number {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return 0
  const isoDay = localDateString(iso)
  if (isoDay === day) return d.getHours() * 60 + d.getMinutes()
  return isoDay < day ? 0 : 24 * 60
}

/**
 * 聚合当天片段:`todoId` 优先、否则 `processName` 归组;只合并相邻同组且间隙
 * ≤ `MERGE_GAP_MS` 的片段。空闲片段无 todoId 时归为「空闲」块。
 * 返回按开始时间排序、已分配重叠列(`lane/lanes`)的块。
 */
export function aggregateDaySegments(
  segments: Segment[],
  todoTitles: Record<string, string>,
  day: string,
  now: Date = new Date()
): ActualBlock[] {
  const sorted = [...segments].sort((a, b) => {
    const as = new Date(a.startAt).getTime()
    const bs = new Date(b.startAt).getTime()
    return as - bs || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0)
  })

  const drafts: DraftBlock[] = []
  for (const s of sorted) {
    const startMs = new Date(s.startAt).getTime()
    if (Number.isNaN(startMs)) continue
    const rawEnd = s.endAt ?? now.toISOString()
    const endMs = new Date(rawEnd).getTime()
    if (Number.isNaN(endMs) || endMs < startMs) continue
    const group = groupOf(s, todoTitles)
    const duration = durationMs(s.startAt, rawEnd, now)
    const detail = (s.note.trim() || s.title).trim()

    const last = drafts[drafts.length - 1]
    const merges =
      last !== undefined &&
      last.key === group.key &&
      startMs - last.endMs <= MERGE_GAP_MS &&
      startMs >= last.startMs
    if (merges) {
      last.endMs = Math.max(last.endMs, endMs)
      last.endAt = new Date(last.endMs).toISOString()
      last.durationMs += duration
      last.label = group.label
      if (detail) last.detail = detail
      last.segments.push(s)
      continue
    }
    drafts.push({
      key: group.key,
      label: group.label,
      todoId: group.todoId,
      detail,
      startAt: s.startAt,
      endAt: new Date(endMs).toISOString(),
      startMs,
      endMs,
      durationMs: duration,
      segments: [s]
    })
  }

  const ranges = drafts.map((d) => ({
    startMin: minuteInDay(d.startAt, day),
    endMin: minuteInDay(d.endAt, day)
  }))
  const laidOut = layoutRanges(
    drafts.map((d, i) => ({ draft: d, ...ranges[i] }))
  )
  return laidOut.map(({ draft, startMin, endMin, lane, lanes }, index) => ({
    id: `${draft.key}#${index}`,
    key: draft.key,
    label: draft.label,
    todoId: draft.todoId,
    idle: draft.segments.every((s) => s.kind === 'idle'),
    detail: draft.detail,
    startAt: draft.startAt,
    endAt: draft.endAt,
    durationMs: draft.durationMs,
    startMin,
    endMin,
    segments: draft.segments,
    lane,
    lanes
  }))
}

/** 每个待办当天的实际时长(含关联到该待办的空闲片段)。 */
export function actualMsByTodo(segments: Segment[], now: Date = new Date()): Record<string, number> {
  const out: Record<string, number> = {}
  for (const s of segments) {
    if (!s.todoId) continue
    out[s.todoId] = (out[s.todoId] ?? 0) + durationMs(s.startAt, s.endAt, now)
  }
  return out
}

export interface TimedJournal {
  entry: JournalEntry
  startMin: number
}

/**
 * 拆分当天日志:有 `time` 的按墙钟排序进时间线窄轨,其余归入「全天」区。
 * 无法解析的 `time` 视作无时刻(与列表展示的容忍度一致)。
 */
export function placeJournals(entries: JournalEntry[]): {
  timed: TimedJournal[]
  untimed: JournalEntry[]
} {
  const timed: TimedJournal[] = []
  const untimed: JournalEntry[] = []
  for (const entry of entries) {
    const startMin = timeToMinutes(entry.time)
    if (startMin === null) {
      untimed.push(entry)
    } else {
      timed.push({ entry, startMin })
    }
  }
  timed.sort(
    (a, b) =>
      a.startMin - b.startMin ||
      (a.entry.createdAt < b.entry.createdAt ? -1 : a.entry.createdAt > b.entry.createdAt ? 1 : 0)
  )
  return { timed, untimed }
}

/** 由任意稳定键派生色相(0–359),让同一 Todo 的计划块与实际块同色。 */
export function stableHue(key: string): number {
  // FNV-1a:相同键稳定;不同键(即使只差一个字符)也会散开到不同色相。
  let hash = 2166136261
  for (let i = 0; i < key.length; i++) {
    hash = Math.imul(hash ^ key.charCodeAt(i), 16777619) >>> 0
  }
  return hash % 360
}
