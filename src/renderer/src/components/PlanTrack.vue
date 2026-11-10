<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import type { Todo } from '@shared/types'
import type { MinuteRange, PositionedBlock } from '@shared/schedule'
import { isSlotOverdue, snapMinutes, SNAP_MINUTES } from '@shared/schedule'
import { stableHue } from '@shared/timeline'
import { PX_PER_MIN, heightFor, hueVar, laneStyle, planActualLabel, topFor } from '../timeline-geometry'

// 计划轨:待办的时间块,保留原有的拖拽 / 缩放 / 完成 / 超期样式。
const props = defineProps<{
  blocks: PositionedBlock[]
  actualMs: Record<string, number>
  range: MinuteRange
  now: Date
}>()

const emit = defineEmits<{
  (e: 'open', todo: Todo): void
  (e: 'drag-start', todo: Todo, ev: DragEvent): void
  (e: 'drag-end'): void
  (e: 'reschedule', todo: Todo, startMin: number, endMin: number): void
}>()

interface ResizeState {
  id: string
  startMin: number
  endMin: number
  startY: number
  preview: number
}

const resizing = ref<ResizeState | null>(null)

function isOverdue(t: Todo): boolean {
  return isSlotOverdue(t, props.now)
}

function actualOf(id: string): number {
  return props.actualMs[id] ?? 0
}

function styleFor(b: PositionedBlock): Record<string, string> {
  const active = resizing.value?.id === b.todo.id ? resizing.value : null
  const endMin = active ? active.preview : b.endMin
  return {
    top: `${topFor(b.startMin, props.range)}px`,
    height: `${heightFor(b.startMin, endMin, props.range)}px`,
    ...laneStyle(b.lane, b.lanes),
    ...hueVar(stableHue(b.todo.id))
  }
}

function startResize(b: PositionedBlock, e: PointerEvent): void {
  e.preventDefault()
  e.stopPropagation()
  resizing.value = {
    id: b.todo.id,
    startMin: b.startMin,
    endMin: b.endMin,
    startY: e.clientY,
    preview: b.endMin
  }
  window.addEventListener('pointermove', onResizeMove)
  window.addEventListener('pointerup', stopResize, { once: true })
  window.addEventListener('pointercancel', stopResize, { once: true })
}

function onResizeMove(e: PointerEvent): void {
  const r = resizing.value
  if (!r) return
  const delta = (e.clientY - r.startY) / PX_PER_MIN
  r.preview = Math.max(r.startMin + SNAP_MINUTES, Math.min(1439, snapMinutes(r.endMin + delta)))
}

function stopResize(): void {
  window.removeEventListener('pointermove', onResizeMove)
  const r = resizing.value
  resizing.value = null
  if (!r || r.preview === r.endMin) return
  const todo = props.blocks.find((b) => b.todo.id === r.id)?.todo
  if (todo) emit('reschedule', todo, r.startMin, r.preview)
}

const hasBlocks = computed(() => props.blocks.length > 0)

onBeforeUnmount(() => {
  window.removeEventListener('pointermove', onResizeMove)
  window.removeEventListener('pointerup', stopResize)
  window.removeEventListener('pointercancel', stopResize)
})
</script>

<template>
  <article
    v-for="b in blocks"
    :key="b.todo.id"
    class="block"
    :class="{
      done: !!b.todo.completedAt,
      overdue: isOverdue(b.todo),
      resizing: resizing?.id === b.todo.id
    }"
    :style="styleFor(b)"
    draggable="true"
    @dragstart="emit('drag-start', b.todo, $event)"
    @dragend="emit('drag-end')"
    @click="emit('open', b.todo)"
  >
    <div class="block-time">
      <span>{{ b.todo.plannedStart }}–{{ b.todo.plannedEnd }}</span>
      <span v-if="isOverdue(b.todo)" class="block-overdue">超期</span>
    </div>
    <div class="block-title" :class="{ strikethrough: !!b.todo.completedAt }">
      {{ b.todo.title }}
    </div>
    <div class="block-meta">
      {{ planActualLabel((b.endMin - b.startMin) * 60000, actualOf(b.todo.id)) }}
    </div>
    <div
      class="resize-handle"
      draggable="false"
      title="拖动调整结束时间"
      @pointerdown="startResize(b, $event)"
      @click.stop
    ></div>
  </article>

  <p v-if="!hasBlocks" class="empty-hint">
    把「待安排」拖到时间轴，或拖到上方全天区
  </p>
</template>

<style scoped>
.block {
  position: absolute;
  z-index: 2;
  background: hsl(var(--hue) 60% 55% / 0.16);
  border: 1px solid hsl(var(--hue) 60% 60% / 0.85);
  border-left-width: 3px;
  border-radius: 6px;
  padding: 2px 6px;
  overflow: hidden;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.block:hover {
  background: hsl(var(--hue) 60% 55% / 0.26);
}
.block.resizing {
  opacity: 0.75;
}
.block.done {
  opacity: 0.55;
  border-color: var(--done);
  background: rgba(107, 158, 107, 0.12);
}
.block.overdue {
  border-color: var(--danger);
  background: rgba(255, 107, 107, 0.12);
}
.block-time {
  line-height: 1.2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  font-size: 10px;
  color: var(--muted);
  flex-shrink: 0;
  white-space: nowrap;
}
.block-title {
  font-size: 12px;
  line-height: 1.2;
  flex-shrink: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.block-meta {
  font-size: 10px;
  line-height: 1.2;
  color: var(--muted);
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.block-overdue {
  font-size: 9px;
  color: var(--danger);
  flex-shrink: 0;
}
.resize-handle {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 7px;
  cursor: ns-resize;
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
.strikethrough {
  text-decoration: line-through;
  color: var(--muted);
}
</style>
