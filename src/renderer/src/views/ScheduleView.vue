<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Todo } from '@shared/types'
import { localDateString } from '@shared/time'
import {
  addLocalDays,
  dayOfMonth,
  gridRange,
  isSlotOverdue,
  layoutDay,
  minutesToTime,
  snapMinutes,
  timeToMinutes,
  weekDates,
  weekdayLabel,
  SNAP_MINUTES,
  type PositionedBlock
} from '@shared/schedule'
import ScheduleEditor from '../components/ScheduleEditor.vue'

const PX_PER_MIN = 0.8
const SNAP = SNAP_MINUTES

const day = ref(todayLocal())
const weekTodos = ref<Todo[]>([])
const backlog = ref<Todo[]>([])
const editing = ref<Todo | null>(null)
const now = ref(new Date())
const errorMsg = ref<string | null>(null)
const backlogQuery = ref('')

const dragId = ref<string | null>(null)
const dragDuration = ref(60)
const timelineEl = ref<HTMLElement | null>(null)

const resizing = ref<{ id: string; startMin: number; endMin: number; startY: number } | null>(null)
const resizePreview = ref<{ id: string; endMin: number } | null>(null)

let tickTimer: ReturnType<typeof setInterval> | undefined
let errorTimer: ReturnType<typeof setTimeout> | undefined

function todayLocal(): string {
  return localDateString(new Date().toISOString())
}

const today = computed(() => localDateString(now.value.toISOString()))
const week = computed(() => weekDates(day.value))
const weekStart = computed(() => week.value[0])
const weekEnd = computed(() => week.value[6])

const dayTodos = computed(() => weekTodos.value.filter((t) => t.plannedDate === day.value))
const untimedTodos = computed(() => dayTodos.value.filter((t) => !isTimed(t)))
const blocks = computed(() => layoutDay(dayTodos.value))
const range = computed(() => gridRange(dayTodos.value))
const gridHeight = computed(() => (range.value.endMin - range.value.startMin) * PX_PER_MIN)
const hourMarks = computed(() => {
  const out: number[] = []
  for (let m = range.value.startMin; m <= range.value.endMin; m += 60) out.push(m)
  return out
})
const nowLineTop = computed(() => {
  if (day.value !== today.value) return null
  const minutes = now.value.getHours() * 60 + now.value.getMinutes()
  if (minutes < range.value.startMin || minutes > range.value.endMin) return null
  return (minutes - range.value.startMin) * PX_PER_MIN
})

const dayCounts = computed<Record<string, number>>(() => {
  const out: Record<string, number> = {}
  for (const t of weekTodos.value) {
    if (t.completedAt || !t.plannedDate) continue
    out[t.plannedDate] = (out[t.plannedDate] ?? 0) + 1
  }
  return out
})

const filteredBacklog = computed(() => {
  const q = backlogQuery.value.trim().toLowerCase()
  if (!q) return backlog.value
  return backlog.value.filter((t) => t.title.toLowerCase().includes(q))
})

function isTimed(t: Todo): boolean {
  const start = timeToMinutes(t.plannedStart)
  const end = timeToMinutes(t.plannedEnd)
  return start !== null && end !== null && end > start
}

function isOverdue(t: Todo): boolean {
  return isSlotOverdue(t, now.value)
}

function flashError(e: unknown): void {
  errorMsg.value = e instanceof Error ? e.message : String(e)
  if (errorTimer) clearTimeout(errorTimer)
  errorTimer = setTimeout(() => (errorMsg.value = null), 4000)
}

async function refresh(): Promise<void> {
  const [scheduled, unscheduled] = await Promise.all([
    window.api.todos.scheduled(weekStart.value, weekEnd.value),
    window.api.todos.unscheduled()
  ])
  weekTodos.value = scheduled
  backlog.value = unscheduled
}

function shiftDay(delta: number): void {
  day.value = addLocalDays(day.value, delta)
}

function goToday(): void {
  day.value = today.value
}

function selectDay(d: string): void {
  day.value = d
}

function goSettings(): void {
  location.hash = '#/settings'
}

function onDayInput(e: Event): void {
  const v = (e.target as HTMLInputElement).value
  if (v) day.value = v
}

function openEditor(t: Todo): void {
  editing.value = t
}

function findTodo(id: string): Todo | undefined {
  return weekTodos.value.find((t) => t.id === id) ?? backlog.value.find((t) => t.id === id)
}

async function assign(id: string, slot: { plannedDate: string | null; plannedStart: string | null; plannedEnd: string | null }): Promise<void> {
  try {
    await window.api.todos.schedule(id, slot)
    await refresh()
  } catch (e) {
    flashError(e)
  }
}

async function toggleDone(t: Todo): Promise<void> {
  try {
    await window.api.todos.toggle(t.id, !t.completedAt)
    await refresh()
  } catch (e) {
    flashError(e)
  }
}

function onDragStart(t: Todo, e: DragEvent): void {
  dragId.value = t.id
  const start = timeToMinutes(t.plannedStart)
  const end = timeToMinutes(t.plannedEnd)
  dragDuration.value = start !== null && end !== null && end > start ? end - start : 60
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', t.id)
  }
}

function onDragEnd(): void {
  dragId.value = null
}

async function onTimelineDrop(e: DragEvent): Promise<void> {
  e.preventDefault()
  const id = dragId.value
  dragId.value = null
  if (!id) return
  const el = timelineEl.value
  if (!el) return
  const rect = el.getBoundingClientRect()
  const raw = range.value.startMin + (e.clientY - rect.top) / PX_PER_MIN
  const start = Math.min(snapMinutes(raw, SNAP), 1440 - SNAP)
  const end = Math.min(start + Math.max(SNAP, dragDuration.value), 1439)
  await assign(id, {
    plannedDate: day.value,
    plannedStart: minutesToTime(start),
    plannedEnd: minutesToTime(end)
  })
}

async function onUntimedDrop(e: DragEvent): Promise<void> {
  e.preventDefault()
  const id = dragId.value
  dragId.value = null
  if (!id) return
  await assign(id, { plannedDate: day.value, plannedStart: null, plannedEnd: null })
}

async function onWeekDrop(target: string, e: DragEvent): Promise<void> {
  e.preventDefault()
  const id = dragId.value
  dragId.value = null
  if (!id) return
  const t = findTodo(id)
  const keepTime = t ? isTimed(t) : false
  await assign(id, {
    plannedDate: target,
    plannedStart: keepTime ? t!.plannedStart : null,
    plannedEnd: keepTime ? t!.plannedEnd : null
  })
  day.value = target
}

function blockStyle(b: PositionedBlock): Record<string, string> {
  const preview = resizePreview.value?.id === b.todo.id ? resizePreview.value : null
  const endMin = preview ? preview.endMin : b.endMin
  const top = (b.startMin - range.value.startMin) * PX_PER_MIN
  const height = Math.max(24, (endMin - b.startMin) * PX_PER_MIN)
  const width = 100 / b.lanes
  return {
    top: `${top}px`,
    height: `${height}px`,
    left: `calc(${b.lane * width}% + 2px)`,
    width: `calc(${width}% - 4px)`
  }
}

function startResize(t: Todo, e: PointerEvent): void {
  e.preventDefault()
  e.stopPropagation()
  const b = blocks.value.find((x) => x.todo.id === t.id)
  if (!b) return
  resizing.value = { id: t.id, startMin: b.startMin, endMin: b.endMin, startY: e.clientY }
  resizePreview.value = { id: t.id, endMin: b.endMin }
  window.addEventListener('pointermove', onResizeMove)
  window.addEventListener('pointerup', stopResize, { once: true })
  window.addEventListener('pointercancel', stopResize, { once: true })
}

function onResizeMove(e: PointerEvent): void {
  const r = resizing.value
  if (!r) return
  const delta = (e.clientY - r.startY) / PX_PER_MIN
  const end = Math.max(r.startMin + SNAP, Math.min(1439, snapMinutes(r.endMin + delta, SNAP)))
  resizePreview.value = { id: r.id, endMin: end }
}

async function stopResize(): Promise<void> {
  window.removeEventListener('pointermove', onResizeMove)
  const r = resizing.value
  const preview = resizePreview.value
  resizing.value = null
  resizePreview.value = null
  if (!r || !preview) return
  await assign(r.id, {
    plannedDate: day.value,
    plannedStart: minutesToTime(r.startMin),
    plannedEnd: minutesToTime(preview.endMin)
  })
}

function onEditorUpdated(): void {
  void refresh()
}

watch(day, () => void refresh())

onMounted(async () => {
  await refresh()
  tickTimer = setInterval(() => {
    now.value = new Date()
  }, 60000)
})

onBeforeUnmount(() => {
  if (tickTimer) clearInterval(tickTimer)
  if (errorTimer) clearTimeout(errorTimer)
  window.removeEventListener('pointermove', onResizeMove)
  window.removeEventListener('pointerup', stopResize)
  window.removeEventListener('pointercancel', stopResize)
})
</script>

<template>
  <div class="page">
    <header class="header">
      <h1>日程</h1>
      <div class="actions">
        <button class="ghost" title="设置" @click="goSettings">⚙</button>
      </div>
    </header>

    <div v-if="errorMsg" class="toast error-toast">{{ errorMsg }}</div>

    <section class="daybar">
      <button @click="shiftDay(-1)">‹</button>
      <input type="date" :value="day" @change="onDayInput" />
      <button @click="shiftDay(1)">›</button>
      <button class="ghost" @click="goToday">今天</button>
    </section>

    <section class="weekstrip">
      <button
        v-for="d in week"
        :key="d"
        class="weekday"
        :class="{ active: d === day, today: d === today }"
        @click="selectDay(d)"
        @dragover.prevent
        @drop="onWeekDrop(d, $event)"
      >
        <span class="wd">{{ weekdayLabel(d) }}</span>
        <span class="dn">{{ dayOfMonth(d) }}</span>
        <span v-if="dayCounts[d]" class="count">{{ dayCounts[d] }}</span>
      </button>
    </section>

    <div class="board">
      <section class="day-panel">
        <div class="untimed-zone" @dragover.prevent @drop="onUntimedDrop">
          <h2>全天 / 未定时间</h2>
          <article
            v-for="t in untimedTodos"
            :key="t.id"
            class="untimed-item"
            :class="{ done: !!t.completedAt, overdue: isOverdue(t) }"
            draggable="true"
            @dragstart="onDragStart(t, $event)"
            @dragend="onDragEnd"
            @click="openEditor(t)"
          >
            <input
              class="check"
              type="checkbox"
              :checked="!!t.completedAt"
              @click.stop="toggleDone(t)"
            />
            <span class="title" :class="{ strikethrough: !!t.completedAt }">{{ t.title }}</span>
            <span v-if="isOverdue(t)" class="overdue-tag">超期</span>
          </article>
          <p v-if="untimedTodos.length === 0" class="muted hint">把待办拖到这里，安排到当天</p>
        </div>

        <div class="timeline-wrap">
          <div class="hours" :style="{ height: gridHeight + 'px' }">
            <span
              v-for="m in hourMarks"
              :key="m"
              class="hour-label"
              :style="{ top: (m - range.startMin) * PX_PER_MIN + 'px' }"
            >
              {{ minutesToTime(m) }}
            </span>
          </div>
          <div
            ref="timelineEl"
            class="timeline"
            :style="{ height: gridHeight + 'px' }"
            @dragover.prevent
            @drop="onTimelineDrop"
          >
            <div
              v-for="m in hourMarks"
              :key="m"
              class="hour-line"
              :style="{ top: (m - range.startMin) * PX_PER_MIN + 'px' }"
            ></div>
            <div v-if="nowLineTop !== null" class="now-line" :style="{ top: (nowLineTop ?? 0) + 'px' }"></div>

            <article
              v-for="b in blocks"
              :key="b.todo.id"
              class="block"
              :class="{
                done: !!b.todo.completedAt,
                overdue: isOverdue(b.todo),
                dragging: dragId === b.todo.id
              }"
              :style="blockStyle(b)"
              draggable="true"
              @dragstart="onDragStart(b.todo, $event)"
              @dragend="onDragEnd"
              @click="openEditor(b.todo)"
            >
              <div class="block-time">{{ b.todo.plannedStart }}–{{ b.todo.plannedEnd }}</div>
              <div class="block-title" :class="{ strikethrough: !!b.todo.completedAt }">
                {{ b.todo.title }}
              </div>
              <div v-if="isOverdue(b.todo)" class="block-overdue">超期</div>
              <div
                class="resize-handle"
                draggable="false"
                title="拖动调整结束时间"
                @pointerdown="startResize(b.todo, $event)"
                @click.stop
              ></div>
            </article>

            <p v-if="blocks.length === 0" class="timeline-empty muted">
              把「待安排」拖到时间轴，或拖到上方全天区
            </p>
          </div>
        </div>
      </section>

      <aside class="backlog">
        <h2>待安排 <span class="count">{{ backlog.length }}</span></h2>
        <input
          v-model="backlogQuery"
          class="backlog-search"
          spellcheck="false"
          placeholder="搜索待安排…"
        />
        <article
          v-for="t in filteredBacklog"
          :key="t.id"
          class="backlog-item"
          draggable="true"
          @dragstart="onDragStart(t, $event)"
          @dragend="onDragEnd"
          @click="openEditor(t)"
        >
          <span class="drag-dots">⠿</span>
          <span class="title">{{ t.title }}</span>
          <button class="ghost sm" @click.stop="openEditor(t)">安排</button>
        </article>
        <p v-if="backlog.length === 0" class="muted empty">没有未安排的待办</p>
        <p v-else-if="filteredBacklog.length === 0" class="muted empty">没有匹配的待办</p>
      </aside>
    </div>

    <ScheduleEditor
      v-if="editing"
      :todo="editing"
      :default-date="day"
      @updated="onEditorUpdated"
      @close="editing = null"
    />
  </div>
</template>

<style scoped>
.page {
  max-width: 1000px;
  margin: 0 auto;
  padding: 16px;
}
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.header h1 {
  font-size: 18px;
  margin: 0;
}
.toast {
  margin: 8px 0;
  padding: 8px 10px;
  background: var(--accent-soft);
  border-radius: 6px;
  font-size: 12px;
  word-break: break-all;
}
.error-toast {
  color: var(--danger);
}
.daybar {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 12px 0;
}
.daybar input {
  width: auto;
}
.weekstrip {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
  margin-bottom: 12px;
}
.weekday {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 6px 2px;
  border-radius: 8px;
  background: var(--panel);
  color: var(--muted);
}
.weekday.active {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--text);
}
.weekday.today .dn {
  color: var(--accent);
  font-weight: 700;
}
.wd {
  font-size: 10px;
}
.dn {
  font-size: 15px;
  font-weight: 600;
}
.count {
  position: absolute;
  top: 2px;
  right: 4px;
  font-size: 9px;
  color: var(--accent);
}
.board {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  gap: 14px;
  align-items: start;
}
@media (max-width: 760px) {
  .board {
    grid-template-columns: 1fr;
  }
}
.day-panel {
  min-width: 0;
}
.untimed-zone {
  background: var(--panel);
  border: 1px dashed var(--border);
  border-radius: 8px;
  padding: 8px 10px;
  margin-bottom: 12px;
  min-height: 54px;
}
.untimed-zone h2,
.backlog h2 {
  font-size: 12px;
  color: var(--muted);
  margin: 0 0 6px;
}
.untimed-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 6px;
  cursor: pointer;
}
.untimed-item:hover {
  background: var(--panel-2);
}
.untimed-item .title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.untimed-item.done {
  opacity: 0.6;
}
.overdue {
  color: var(--danger);
}
.overdue-tag {
  font-size: 10px;
  color: var(--danger);
  border: 1px solid var(--danger);
  border-radius: 8px;
  padding: 0 5px;
  flex-shrink: 0;
}
.timeline-wrap {
  display: flex;
  gap: 6px;
}
.hours {
  position: relative;
  width: 42px;
  flex-shrink: 0;
}
.hour-label {
  position: absolute;
  right: 0;
  transform: translateY(-50%);
  font-size: 10px;
  color: var(--muted);
  white-space: nowrap;
}
.timeline {
  position: relative;
  flex: 1;
  min-width: 0;
  border-left: 1px solid var(--border);
}
.hour-line {
  position: absolute;
  left: 0;
  right: 0;
  border-top: 1px solid var(--border);
  opacity: 0.5;
  pointer-events: none;
}
.now-line {
  position: absolute;
  left: 0;
  right: 0;
  border-top: 2px solid var(--danger);
  z-index: 3;
  pointer-events: none;
}
.block {
  position: absolute;
  z-index: 2;
  background: var(--accent-soft);
  border: 1px solid var(--accent);
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
  background: rgba(74, 158, 255, 0.25);
}
.block.dragging {
  opacity: 0.4;
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
  font-size: 10px;
  color: var(--muted);
  flex-shrink: 0;
}
.block-title {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.block-overdue {
  font-size: 10px;
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
.timeline-empty {
  position: absolute;
  top: 10px;
  left: 8px;
  right: 8px;
  pointer-events: none;
}
.backlog {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px;
  position: sticky;
  top: 12px;
  max-height: calc(100vh - 220px);
  overflow: auto;
}
.backlog-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 4px;
  border-radius: 6px;
  cursor: pointer;
}
.backlog-search {
  margin-bottom: 6px;
  font-size: 12px;
  padding: 4px 6px;
}
.backlog-item:hover {
  background: var(--panel-2);
}
.backlog-item .title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.drag-dots {
  color: var(--muted);
  font-size: 11px;
  cursor: grab;
}
.sm {
  padding: 1px 7px;
  font-size: 11px;
  flex-shrink: 0;
}
.muted {
  color: var(--muted);
  font-size: 12px;
}
.hint {
  margin: 4px 0 0;
}
.empty {
  text-align: center;
  padding: 12px 0;
}
.strikethrough {
  text-decoration: line-through;
  color: var(--muted);
}
.check {
  width: auto;
  flex-shrink: 0;
  cursor: pointer;
}
</style>
