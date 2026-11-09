// 时间线渲染几何:三轨共用同一像素刻度,块位置/尺寸只由分钟范围推导,
// 保证计划、实际、日志三轨对齐同一小时刻度与 now 线。

import type { MinuteRange } from '@shared/schedule'
import { formatDuration } from '@shared/time'

/** 时间轴像素刻度(与既有日程时间轴一致)。 */
export const PX_PER_MIN = 0.8

/** 块内最小高度,避免短块文字被裁掉。 */
export const MIN_BLOCK_PX = 24

export function gridHeight(range: MinuteRange, pxPerMin = PX_PER_MIN): number {
  return (range.endMin - range.startMin) * pxPerMin
}

export function topFor(startMin: number, range: MinuteRange, pxPerMin = PX_PER_MIN): number {
  return (startMin - range.startMin) * pxPerMin
}

export function heightFor(
  startMin: number,
  endMin: number,
  range: MinuteRange,
  pxPerMin = PX_PER_MIN
): number {
  return Math.max(MIN_BLOCK_PX, (endMin - startMin) * pxPerMin)
}

/** 重叠簇内按 lane/lanes 分栏的定位样式。 */
export function laneStyle(lane: number, lanes: number): Record<string, string> {
  const width = 100 / Math.max(1, lanes)
  return {
    left: `calc(${lane * width}% + 2px)`,
    width: `calc(${width}% - 4px)`
  }
}

/** 由 Todo 派生的稳定色相,让同一待办的「计划」与「实际」块同色。 */
export function hueVar(hue: number | null): Record<string, string> {
  return { '--hue': String(hue ?? 210) }
}

/** 块内时长文案 `计划 60m · 实际 45m`;无实际时不显示实际部分。 */
export function planActualLabel(plannedMs: number, actualMs: number): string {
  const planned = `计划 ${formatDuration(plannedMs)}`
  return actualMs > 0 ? `${planned} · 实际 ${formatDuration(actualMs)}` : planned
}
