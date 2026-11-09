// 日程计划的纯函数:本地墙钟解析、超期判定、周范围与时间轴布局。
// 计划日期/时刻均为本地语义(ADR 0005),不经过 UTC 瞬时换算。

import type { Todo } from './types'
import { localDateString } from './time'

/** 时间轴拖动吸附粒度(分钟)。 */
export const SNAP_MINUTES = 15
/** 时间轴默认展示范围:06:00 – 24:00,有更早/更晚安排时由 `gridRange` 扩展。 */
export const DEFAULT_GRID_START_MIN = 6 * 60
export const DEFAULT_GRID_END_MIN = 24 * 60

const WEEKDAY_LABELS = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']

function pad2(n: number): string {
  return n < 10 ? '0' + n : String(n)
}

function dateParts(day: string): [number, number, number] | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day)
  if (!m) return null
  const y = Number(m[1])
  const mo = Number(m[2])
  const d = Number(m[3])
  if (mo < 1 || mo > 12 || d < 1 || d > 31) return null
  const dt = new Date(y, mo - 1, d, 12, 0, 0, 0)
  if (dt.getFullYear() !== y || dt.getMonth() !== mo - 1 || dt.getDate() !== d) return null
  return [y, mo, d]
}

function localDateTime(day: string, minutes: number): Date | null {
  const parts = dateParts(day)
  if (!parts) return null
  const [y, mo, d] = parts
  return new Date(y, mo - 1, d, Math.floor(minutes / 60), minutes % 60, 0, 0)
}

/** `HH:MM` / `H:MM` → 当日分钟数;非法返回 null。 */
export function timeToMinutes(value: string | null | undefined): number | null {
  if (!value) return null
  const m = /^(\d{1,2}):(\d{2})$/.exec(value.trim())
  if (!m) return null
  const h = Number(m[1])
  const mi = Number(m[2])
  if (h > 23 || mi > 59) return null
  return h * 60 + mi
}

/** 当日分钟数 → `HH:MM`;24:00 仅用于时间轴末端展示。 */
export function minutesToTime(min: number): string {
  const clamped = Math.max(0, Math.min(24 * 60, Math.round(min)))
  if (clamped === 24 * 60) return '24:00'
  return `${pad2(Math.floor(clamped / 60))}:${pad2(clamped % 60)}`
}

/** 吸附到最近的 `step` 分钟,并限制在 [0, 24:00]。 */
export function snapMinutes(min: number, step = SNAP_MINUTES): number {
  if (!Number.isFinite(min) || step <= 0) return 0
  const snapped = Math.round(min / step) * step
  return Math.max(0, Math.min(24 * 60, snapped))
}

/** 计划时段是否已超期:未完成 + 有时间段看结束时刻,仅日期看自然日是否已过。 */
export function isSlotOverdue(
  todo: Pick<Todo, 'completedAt' | 'deletedAt' | 'plannedDate' | 'plannedStart' | 'plannedEnd'>,
  now: Date = new Date()
): boolean {
  if (todo.completedAt || todo.deletedAt || !todo.plannedDate) return false
  const start = timeToMinutes(todo.plannedStart)
  const end = timeToMinutes(todo.plannedEnd)
  if (start !== null && end !== null && end > start) {
    const endAt = localDateTime(todo.plannedDate, end)
    return endAt !== null && endAt.getTime() < now.getTime()
  }
  return todo.plannedDate < localDateString(now.toISOString())
}

/** 本地自然日加减天数;非法输入原样返回。 */
export function addLocalDays(day: string, delta: number): string {
  const parts = dateParts(day)
  if (!parts) return day
  const [y, mo, d] = parts
  const dt = new Date(y, mo - 1, d + delta, 12, 0, 0, 0)
  return localDateString(dt.toISOString())
}

/** `to - from` 的自然日差;非法输入返回 null。 */
export function dayDiff(from: string, to: string): number | null {
  const a = dateParts(from)
  const b = dateParts(to)
  if (!a || !b) return null
  const am = Date.UTC(a[0], a[1] - 1, a[2])
  const bm = Date.UTC(b[0], b[1] - 1, b[2])
  return Math.round((bm - am) / 86400000)
}

/** 所在周的周一(中文习惯周一开头);非法输入原样返回。 */
export function startOfWeek(day: string): string {
  const parts = dateParts(day)
  if (!parts) return day
  const [y, mo, d] = parts
  const dow = new Date(y, mo - 1, d, 12, 0, 0, 0).getDay()
  return addLocalDays(day, -((dow + 6) % 7))
}

/** 所在周的 7 天(周一 → 周日)。 */
export function weekDates(day: string): string[] {
  const start = startOfWeek(day)
  return Array.from({ length: 7 }, (_, i) => addLocalDays(start, i))
}

/** 相对今天展示日期:今天 / 明天 / 昨天 / MM-DD。 */
export function formatPlanDate(day: string, today: string): string {
  const diff = dayDiff(today, day)
  if (diff === 0) return '今天'
  if (diff === 1) return '明天'
  if (diff === -1) return '昨天'
  const parts = dateParts(day)
  if (!parts) return day
  return `${pad2(parts[1])}-${pad2(parts[2])}`
}

/** 待办列表的计划 chip 文案;未安排返回空串。 */
export function formatPlanLabel(todo: Todo, today: string): string {
  if (!todo.plannedDate) return ''
  const date = formatPlanDate(todo.plannedDate, today)
  const start = timeToMinutes(todo.plannedStart)
  const end = timeToMinutes(todo.plannedEnd)
  if (start !== null && end !== null) {
    return `${date} ${minutesToTime(start)}–${minutesToTime(end)}`
  }
  return date
}

/** 周条的星期文案(周一…周日)。 */
export function weekdayLabel(day: string): string {
  const parts = dateParts(day)
  if (!parts) return ''
  return WEEKDAY_LABELS[new Date(parts[0], parts[1] - 1, parts[2], 12, 0, 0, 0).getDay()]
}

/** 周条的日号。 */
export function dayOfMonth(day: string): number {
  return dateParts(day)?.[2] ?? 0
}

export interface PositionedBlock {
  todo: Todo
  startMin: number
  endMin: number
  /** 重叠簇内的列索引(0 起)。 */
  lane: number
  /** 重叠簇的总列数;块宽度 = 1 / lanes。 */
  lanes: number
}

/** 时间轴上的分钟区间;无时长条目(如日志)可令 `endMin === startMin`。 */
export interface MinuteRange {
  startMin: number
  endMin: number
}

/**
 * 通用重叠布局:按开始时间排序,重叠簇内贪心分配列,同一簇内所有块共享列数,
 * 保证左右分栏互不遮挡。计划块与活动聚合块共用这一规则。
 */
export function layoutRanges<T extends MinuteRange>(items: T[]): (T & { lane: number; lanes: number })[] {
  const sorted = [...items].sort((a, b) => a.startMin - b.startMin || a.endMin - b.endMin)
  const out: (T & { lane: number; lanes: number })[] = []
  let cluster: T[] = []
  let clusterEnd = -1

  function flush(): void {
    if (cluster.length === 0) return
    const laneEnds: number[] = []
    const assigned = cluster.map((item) => {
      let lane = laneEnds.findIndex((end) => end <= item.startMin)
      if (lane === -1) {
        lane = laneEnds.length
        laneEnds.push(item.endMin)
      } else {
        laneEnds[lane] = item.endMin
      }
      return { item, lane }
    })
    const lanes = laneEnds.length
    for (const { item, lane } of assigned) {
      out.push({ ...item, lane, lanes })
    }
    cluster = []
    clusterEnd = -1
  }

  for (const item of sorted) {
    if (cluster.length > 0 && item.startMin >= clusterEnd) flush()
    cluster.push(item)
    clusterEnd = Math.max(clusterEnd, item.endMin)
  }
  flush()
  return out
}

/**
 * 计算当天有时间段待办的时间轴布局。
 */
export function layoutDay(todos: Todo[]): PositionedBlock[] {
  const items: { todo: Todo; startMin: number; endMin: number }[] = []
  for (const todo of todos) {
    const startMin = timeToMinutes(todo.plannedStart)
    const endMin = timeToMinutes(todo.plannedEnd)
    if (startMin === null || endMin === null || endMin <= startMin) continue
    items.push({ todo, startMin, endMin })
  }
  return layoutRanges(items)
}

/**
 * 时间轴展示范围:默认 06:00–24:00,按传入区间的最早开始/最晚结束向外扩展,
 * 并取整到小时。日志等无时长条目可传 `endMin = startMin`。
 */
export function expandGridRange(intervals: MinuteRange[]): { startMin: number; endMin: number } {
  let start = DEFAULT_GRID_START_MIN
  let end = DEFAULT_GRID_END_MIN
  for (const item of intervals) {
    if (!Number.isFinite(item.startMin)) continue
    start = Math.min(start, Math.floor(item.startMin / 60) * 60)
    const itemEnd = Number.isFinite(item.endMin) ? item.endMin : item.startMin
    end = Math.max(end, Math.ceil(itemEnd / 60) * 60)
  }
  start = Math.max(0, start)
  end = Math.min(24 * 60, Math.max(end, start + 60))
  return { startMin: start, endMin: end }
}

/** 当天有时间段待办的展示范围(保留旧签名,内部走 `expandGridRange`)。 */
export function gridRange(todos: Todo[]): { startMin: number; endMin: number } {
  const intervals: MinuteRange[] = []
  for (const todo of todos) {
    const s = timeToMinutes(todo.plannedStart)
    const e = timeToMinutes(todo.plannedEnd)
    if (s === null) continue
    intervals.push({ startMin: s, endMin: e ?? s })
  }
  return expandGridRange(intervals)
}
