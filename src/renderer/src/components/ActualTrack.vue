<script setup lang="ts">
import { ref } from 'vue'
import type { Segment } from '@shared/types'
import { minutesToTime, type MinuteRange } from '@shared/schedule'
import { stableHue, type ActualBlock } from '@shared/timeline'
import { durationMs, formatDuration, localTimeOfDay } from '@shared/time'
import { heightFor, laneStyle, topFor } from '../timeline-geometry'

// 实际轨:当天 Activity Segment 的展示层聚合。点击聚合块就地展开原始片段,
// 点原始片段打开 SegmentEditor(不做片段边界编辑)。
const props = defineProps<{
  blocks: ActualBlock[]
  range: MinuteRange
  emptyHint: string
}>()

const emit = defineEmits<{
  (e: 'open-segment', segment: Segment): void
}>()

const expanded = ref<string | null>(null)

function toggle(key: string): void {
  expanded.value = expanded.value === key ? null : key
}

function styleFor(b: ActualBlock): Record<string, string> {
  const open = expanded.value === b.id
  return {
    top: `${topFor(b.startMin, props.range)}px`,
    ...(open
      ? { 'min-height': `${actualHeight(b)}px` }
      : { height: `${actualHeight(b)}px` }),
    ...laneStyle(b.lane, b.lanes)
  }
}

// 实际块至少容纳「时间 + 标签」两行,短片段(30–40 分钟)也能看清是什么。
const DETAIL_MIN_PX = 52

function actualHeight(b: ActualBlock): number {
  return Math.max(40, heightFor(b.startMin, b.endMin, props.range))
}

/** 块太矮时不挤第三行详情,优先保证时间和标签可见。 */
function showDetail(b: ActualBlock): boolean {
  return actualHeight(b) >= DETAIL_MIN_PX
}

function blockHue(b: ActualBlock): Record<string, string> {
  return b.todoId ? { '--hue': String(stableHue(b.todoId)) } : {}
}

function segLabel(s: Segment): string {
  if (s.note.trim().length > 0) return s.note
  if (s.title) return s.title
  if (s.kind === 'idle') return '空闲'
  return s.processName
}

function segRange(s: Segment): string {
  const start = localTimeOfDay(s.startAt) ?? ''
  const end = s.endAt ? localTimeOfDay(s.endAt) ?? '' : '进行中'
  return `${start}–${end}`
}
</script>

<template>
  <article
    v-for="b in blocks"
    :key="b.id"
    class="block"
    :class="{ idle: b.idle, expanded: expanded === b.id }"
    :style="{ ...styleFor(b), ...blockHue(b) }"
    @click="toggle(b.id)"
  >
    <div class="block-time">
      {{ minutesToTime(b.startMin) }}–{{ minutesToTime(b.endMin) }}
      <span class="dur">{{ formatDuration(b.durationMs) }}</span>
    </div>
    <div class="block-label">{{ b.label }}</div>
    <div v-if="b.detail && b.detail !== b.label && showDetail(b)" class="block-detail">
      {{ b.detail }}
    </div>

    <div v-if="expanded === b.id" class="seg-list" @click.stop>
      <button
        v-for="s in b.segments"
        :key="s.id"
        class="seg-row"
        @click.stop="emit('open-segment', s)"
      >
        <span class="seg-time">{{ segRange(s) }}</span>
        <span class="seg-title">{{ segLabel(s) }}</span>
        <span class="seg-dur">{{ formatDuration(durationMs(s.startAt, s.endAt)) }}</span>
      </button>
    </div>
  </article>

  <p v-if="blocks.length === 0" class="empty-hint">{{ emptyHint }}</p>
</template>

<style scoped>
.block {
  position: absolute;
  z-index: 2;
  background: hsl(var(--hue, 210) 55% 55% / 0.14);
  border: 1px solid hsl(var(--hue, 210) 55% 60% / 0.8);
  border-left-width: 3px;
  border-radius: 6px;
  padding: 3px 6px;
  overflow: hidden;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.block:hover {
  background: hsl(var(--hue, 210) 55% 55% / 0.24);
}
.block.idle {
  border-style: dashed;
  opacity: 0.65;
}
.block.expanded {
  max-height: 320px;
  overflow: auto;
  z-index: 20;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  background: var(--panel);
}
.block-time {
  font-size: 10px;
  line-height: 1.2;
  color: var(--muted);
  flex-shrink: 0;
  display: flex;
  justify-content: space-between;
  gap: 6px;
}
.dur {
  color: var(--text);
}
.block-label {
  font-size: 12px;
  line-height: 1.2;
  flex-shrink: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.block-detail {
  font-size: 10px;
  line-height: 1.2;
  color: var(--muted);
  flex-shrink: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.seg-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 4px;
}
.seg-row {
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: 6px;
  align-items: center;
  padding: 2px 4px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: var(--panel-2);
  font-size: 11px;
  text-align: left;
}
.seg-row:hover {
  border-color: var(--accent);
}
.seg-time {
  color: var(--muted);
  white-space: nowrap;
  font-size: 10px;
}
.seg-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.seg-dur {
  color: var(--muted);
  font-size: 10px;
  white-space: nowrap;
}
.empty-hint {
  position: absolute;
  top: 10px;
  left: 8px;
  right: 8px;
  margin: 0;
  color: var(--muted);
  font-size: 12px;
  pointer-events: none;
}
</style>
