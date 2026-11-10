<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { JournalEntry, Segment, Todo, TrackingStatus } from '@shared/types'
import { localDateString } from '@shared/time'
import {
  addLocalDays,
  dayOfMonth,
  expandGridRange,
  formatPlanLabel,
  isSlotOverdue,
  layoutDay,
  minutesToTime,
  snapMinutes,
  timeToMinutes,
  weekDates,
  weekdayLabel,
  SNAP_MINUTES
} from '@shared/schedule'
import { actualMsByTodo, aggregateDaySegments, placeJournals } from '@shared/timeline'
import { groupItems } from '../utils'
import { useAddTodo } from '../composables/useAddTodo'
import { topFor } from '../timeline-geometry'
import ScheduleEditor from '../components/ScheduleEditor.vue'
import SegmentEditor from '../components/SegmentEditor.vue'
import JournalEditor from '../components/JournalEditor.vue'
import DayTimeline from '../components/DayTimeline.vue'
import PlanTrack from '../components/PlanTrack.vue'
import ActualTrack from '../components/ActualTrack.vue'
import JournalTrack from '../components/JournalTrack.vue'

// 日程页只做编排:状态、取数、拖拽/改期、弹层开关;三轨渲染在各自组件里,
// 聚合与布局规则在 `shared/timeline.ts` / `shared/schedule.ts`(可纯函数测试)。
const SNAP = SNAP_MINUTES
type NarrowTrack = 'plan' | 'actual' | 'journal'

const day = ref(todayLocal())
const weekTodos = ref<Todo[]>([])
const allTodos = ref<Todo[]>([])
const segments = ref<Segment[]>([])
const journals = ref<JournalEntry[]>([])
const tracking = ref<TrackingStatus | null>(null)

const editingTodo = ref<Todo | null>(null)
const editingSegment = ref<Segment | null>(null)
const editingJournal = ref<JournalEntry | null>(null)

const now = ref(new Date())
const narrow = ref(false)
const narrowTrack = ref<NarrowTrack>('plan')
const hideIdle = ref(false)
const errorMsg = ref<string | null>(null)
const journalDraftOpen = ref(false)

const newTodoTitle = ref('')
const newTodoDetail = ref('')
const addTitleInput = ref<HTMLInputElement | null>(null)
const {
  adding: addAdding,
  shaking: addShaking,
  submit: submitTodo
} = useAddTodo(newTodoTitle, newTodoDetail)

const dragId = ref<string | null>(null)
const dragDuration = ref(60)
const timelineScrollEl = ref<HTMLElement | null>(null)

let tickTimer: ReturnType<typeof setInterval> | undefined
let errorTimer: ReturnType<typeof setTimeout> | undefined
let mediaQuery: MediaQueryList | null = null
let offTodos: (() => void) | null = null
let refreshSeq = 0

function todayLocal(): string {
  return localDateString(new Date().toISOString())
}

const today = computed(() => localDateString(now.value.toISOString()))
const week = computed(() => weekDates(day.value))
const weekStart = computed(() => week.value[0])
const weekEnd = computed(() => week.value[6])

const dayTodos = computed(() => weekTodos.value.filter((t) => t.plannedDate === day.value))
const untimedTodos = computed(() => dayTodos.value.filter((t) => !isTimed(t)))
const todoTitles = computed<Record<string, string>>(() => {
  const map: Record<string, string> = {}
  for (const t of allTodos.value) map[t.id] = t.title
  return map
})

const planBlocks = computed(() => layoutDay(dayTodos.value))
const actualMs = computed(() => actualMsByTodo(segments.value, now.value))
const actualBlocks = computed(() =>
  aggregateDaySegments(segments.value, todoTitles.value, day.value, now.value)
)
const visibleActualBlocks = computed(() =>
  hideIdle.value ? actualBlocks.value.filter((b) => !b.idle) : actualBlocks.value
)
const journalPlacement = computed(() => placeJournals(journals.value))
const timedJournals = computed(() => journalPlacement.value.timed)
const untimedJournals = computed(() => journalPlacement.value.untimed)

const range = computed(() =>
  expandGridRange([
    ...planBlocks.value.map((b) => ({ startMin: b.startMin, endMin: b.endMin })),
    ...actualBlocks.value.map((b) => ({ startMin: b.startMin, endMin: b.endMin })),
    ...timedJournals.value.map((t) => ({ startMin: t.startMin, endMin: t.startMin }))
  ])
)
const nowLineTop = computed(() => {
  if (day.value !== today.value) return null
  const minutes = now.value.getHours() * 60 + now.value.getMinutes()
  if (minutes < range.value.startMin || minutes > range.value.endMin) return null
  return topFor(minutes, range.value)
})

const showPlan = computed(() => !narrow.value || narrowTrack.value === 'plan')
const showActual = computed(() => !narrow.value || narrowTrack.value === 'actual')
const showJournal = computed(() =>
  narrow.value ? narrowTrack.value === 'journal' : timedJournals.value.length > 0
)

const actualEmptyHint = computed(() => {
  if (actualBlocks.value.length > 0 && visibleActualBlocks.value.length === 0) {
    return '已隐藏空闲片段'
  }
  if (tracking.value && !tracking.value.available) return '活动记录不可用（当前平台不支持）'
  return '当日暂无活动片段'
})

const dayCounts = computed<Record<string, number>>(() => {
  const out: Record<string, number> = {}
  for (const t of weekTodos.value) {
    if (t.completedAt || !t.plannedDate) continue
    out[t.plannedDate] = (out[t.plannedDate] ?? 0) + 1
  }
  return out
})

// 右侧展示完整待办列表(未完成在前、已完成在后),拖拽任意条目即可安排/改期。
const groupedTodos = computed(() => groupItems(allTodos.value))
const visibleTodos = computed(() => [...groupedTodos.value.pending, ...groupedTodos.value.done])

function isTimed(t: Todo): boolean {
  const start = timeToMinutes(t.plannedStart)
  const end = timeToMinutes(t.plannedEnd)
  return start !== null && end !== null && end > start
}

function isOverdue(t: Todo): boolean {
  return isSlotOverdue(t, now.value)
}

function planLabel(t: Todo): string {
  return formatPlanLabel(t, today.value)
}

function journalHead(e: JournalEntry): string {
  if (e.title && e.title.trim()) return e.title.trim()
  const idx = e.body.indexOf('\n')
  return (idx === -1 ? e.body : e.body.slice(0, idx)).trim() || '(无内容)'
}

function flashError(e: unknown): void {
  errorMsg.value = e instanceof Error ? e.message : String(e)
  if (errorTimer) clearTimeout(errorTimer)
  errorTimer = setTimeout(() => (errorMsg.value = null), 4000)
}

async function refresh(): Promise<void> {
  const seq = ++refreshSeq
  try {
    const [scheduled, all, daySegments, dayJournals] = await Promise.all([
      window.api.todos.scheduled(weekStart.value, weekEnd.value),
      window.api.todos.list(),
      window.api.segments.list(day.value),
      window.api.journal.list(day.value)
    ])
    if (seq !== refreshSeq) return
    weekTodos.value = scheduled
    allTodos.value = all
    segments.value = daySegments
    journals.value = dayJournals
  } catch (e) {
    if (seq === refreshSeq) flashError(e)
  }
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


function onDayInput(e: Event): void {
  const v = (e.target as HTMLInputElement).value
  if (v) day.value = v
}

function openTodo(t: Todo): void {
  editingTodo.value = t
}

function openSegment(s: Segment): void {
  editingSegment.value = s
}

function openJournal(e: JournalEntry): void {
  editingJournal.value = e
}

function openJournalCreate(): void {
  journalDraftOpen.value = true
}

async function focusAddTodo(): Promise<void> {
  await nextTick()
  addTitleInput.value?.focus()
}

async function submitAddTodo(): Promise<void> {
  const created = await submitTodo()
  if (!created) return
  await refresh()
  await focusAddTodo()
}

function onAddTodoKeydown(e: KeyboardEvent): void {
  if (e.key === 'Enter') {
    e.preventDefault()
    void submitAddTodo()
  }
}

function findTodo(id: string): Todo | undefined {
  return weekTodos.value.find((t) => t.id === id) ?? allTodos.value.find((t) => t.id === id)
}

async function assign(
  id: string,
  slot: { plannedDate: string | null; plannedStart: string | null; plannedEnd: string | null }
): Promise<void> {
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

async function onTimelineDrop(minutes: number): Promise<void> {
  const id = dragId.value
  dragId.value = null
  if (!id) return
  const start = Math.min(snapMinutes(minutes, SNAP), 1440 - SNAP)
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

async function onReschedule(todo: Todo, startMin: number, endMin: number): Promise<void> {
  await assign(todo.id, {
    plannedDate: day.value,
    plannedStart: minutesToTime(startMin),
    plannedEnd: minutesToTime(endMin)
  })
}

function onEditorUpdated(): void {
  void refresh()
}

function onChildSaved(): void {
  void refresh()
}

function onMediaChange(): void {
  narrow.value = mediaQuery?.matches ?? false
}

function onVisibility(): void {
  if (document.hidden) return
  now.value = new Date()
  void refresh()
}

/** 打开/切日时把当前时刻放进时间线可视范围(约上 1/3 处);查看其他日期则回到顶部。 */
function scrollToCurrentTime(): void {
  const el = timelineScrollEl.value
  if (!el) return
  if (day.value !== today.value) {
    el.scrollTop = 0
    return
  }
  const minutes = now.value.getHours() * 60 + now.value.getMinutes()
  const target = topFor(minutes, range.value) - el.clientHeight * 0.35
  const max = Math.max(0, el.scrollHeight - el.clientHeight)
  el.scrollTop = Math.min(Math.max(0, target), max)
}

watch(day, async () => {
  await refresh()
  await nextTick()
  scrollToCurrentTime()
})

onMounted(async () => {
  mediaQuery = window.matchMedia('(max-width: 899px)')
  narrow.value = mediaQuery.matches
  mediaQuery.addEventListener('change', onMediaChange)
  document.addEventListener('visibilitychange', onVisibility)
  offTodos = window.api.todos.onChanged(() => {
    void refresh()
  })
  void window.api.tracker
    .getStatus()
    .then((s) => (tracking.value = s))
    .catch(() => {})
  await refresh()
  await nextTick()
  scrollToCurrentTime()
  tickTimer = setInterval(() => {
    if (document.hidden) return
    now.value = new Date()
    void refresh()
  }, 60000)
})

onBeforeUnmount(() => {
  if (tickTimer) clearInterval(tickTimer)
  if (errorTimer) clearTimeout(errorTimer)
  mediaQuery?.removeEventListener('change', onMediaChange)
  document.removeEventListener('visibilitychange', onVisibility)
  if (offTodos) offTodos()
})
</script>

<template>
  <div class="page page--fixed">
    <header class="page-header">
      <h1>日程</h1>
      <div class="page-actions">
        <button :disabled="addAdding" @click="focusAddTodo">+ 待办</button>
        <button class="primary" @click="openJournalCreate">+ 日志</button>
      </div>
    </header>

    <div v-if="errorMsg" class="toast error-toast">{{ errorMsg }}</div>

    <section class="dayrow">
      <div class="date-controls">
        <button class="nav-btn" title="前一天" @click="shiftDay(-1)">‹</button>
        <input class="date-input" type="date" :value="day" @change="onDayInput" />
        <button class="nav-btn" title="后一天" @click="shiftDay(1)">›</button>
        <button class="ghost" @click="goToday">今天</button>
      </div>

      <div class="weekstrip">
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
      </div>

      <label class="idle-toggle">
        <input v-model="hideIdle" type="checkbox" />
        <span>隐藏空闲</span>
      </label>
    </section>

    <div class="board">
      <section class="day-panel">
        <div v-if="narrow" class="segmented">
          <button :class="{ active: narrowTrack === 'plan' }" @click="narrowTrack = 'plan'">计划</button>
          <button :class="{ active: narrowTrack === 'actual' }" @click="narrowTrack = 'actual'">实际</button>
          <button :class="{ active: narrowTrack === 'journal' }" @click="narrowTrack = 'journal'">日志</button>
        </div>

        <div ref="timelineScrollEl" class="timeline-scroll">
          <DayTimeline
            :range="range"
            :now-line-top="nowLineTop"
            :show-plan="showPlan"
            :show-actual="showActual"
            :show-journal="showJournal"
            @drop-minutes="onTimelineDrop"
          >
            <template #plan>
              <PlanTrack
                :blocks="planBlocks"
                :actual-ms="actualMs"
                :range="range"
                :now="now"
                @open="openTodo"
                @drag-start="onDragStart"
                @drag-end="onDragEnd"
                @reschedule="onReschedule"
              />
            </template>
            <template #actual>
              <ActualTrack
                :blocks="visibleActualBlocks"
                :range="range"
                :empty-hint="actualEmptyHint"
                @open-segment="openSegment"
              />
            </template>
            <template #journal>
              <JournalTrack
                :items="timedJournals"
                :range="range"
                :narrow="narrow"
                @open="openJournal"
              />
            </template>
          </DayTimeline>
        </div>
      </section>

      <aside class="side-panel">
        <section class="untimed-zone panel" @dragover.prevent @drop="onUntimedDrop">
          <h2>全天 / 未定时间</h2>
          <div class="untimed-list">
            <article
              v-for="t in untimedTodos"
              :key="t.id"
              class="untimed-item"
              :class="{ done: !!t.completedAt, overdue: isOverdue(t) }"
              draggable="true"
              @dragstart="onDragStart(t, $event)"
              @dragend="onDragEnd"
              @click="openTodo(t)"
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
            <article
              v-for="e in untimedJournals"
              :key="e.id"
              class="untimed-item journal"
              @click="openJournal(e)"
            >
              <span class="kind">日志</span>
              <span class="title">{{ journalHead(e) }}</span>
            </article>
            <p v-if="untimedTodos.length === 0 && untimedJournals.length === 0" class="muted hint">
              把待办拖到这里，安排到当天
            </p>
          </div>
        </section>

        <section class="todos-panel panel">
          <h2>待办 <span class="total">{{ allTodos.length }}</span></h2>
          <section class="add-todo" :class="{ shake: addShaking }">
            <input
              ref="addTitleInput"
              v-model="newTodoTitle"
              spellcheck="false"
              placeholder="添加待办，回车提交"
              @keydown="onAddTodoKeydown"
            />
          </section>
          <div class="todos-scroll">
            <article
              v-for="t in visibleTodos"
              :key="t.id"
              class="todo-item"
              :class="{ done: !!t.completedAt, overdue: !t.completedAt && isOverdue(t) }"
              draggable="true"
              @dragstart="onDragStart(t, $event)"
              @dragend="onDragEnd"
              @click="openTodo(t)"
            >
              <input
                class="check"
                type="checkbox"
                :checked="!!t.completedAt"
                @click.stop="toggleDone(t)"
              />
              <div class="todo-main">
                <div class="todo-title" :class="{ strikethrough: !!t.completedAt }">{{ t.title }}</div>
                <div class="todo-meta">
                  <span
                    v-if="t.plannedDate"
                    class="plan-chip"
                    :class="{ overdue: !t.completedAt && isOverdue(t) }"
                  >
                    {{ planLabel(t) }}
                  </span>
                  <span v-else class="muted">未安排</span>
                </div>
              </div>
              <span class="drag-dots">⠿</span>
            </article>
            <p v-if="allTodos.length === 0" class="muted empty">还没有待办</p>
          </div>
        </section>
      </aside>
    </div>

    <ScheduleEditor
      v-if="editingTodo"
      :todo="editingTodo"
      :default-date="day"
      @updated="onEditorUpdated"
      @close="editingTodo = null"
    />

    <SegmentEditor
      v-if="editingSegment"
      :segment="editingSegment"
      :todos="allTodos"
      @saved="onChildSaved"
      @close="editingSegment = null"
    />

    <JournalEditor
      v-if="editingJournal"
      :entry="editingJournal"
      :default-date="day"
      @saved="onChildSaved"
      @close="editingJournal = null"
    />

    <JournalEditor
      v-if="journalDraftOpen"
      :entry="null"
      :default-date="day"
      @saved="onChildSaved"
      @close="journalDraftOpen = false"
    />
  </div>
</template>


<style scoped>
.error-toast {
  color: var(--danger);
}

/* 日期导航与周条合并为一行，给时间轴让出垂直空间；窄屏自动换行。 */
.dayrow {
  display: flex;
  flex-shrink: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  margin: 0 0 12px;
}
.date-controls {
  display: flex;
  align-items: center;
  gap: 4px;
}
.nav-btn {
  padding: 4px 9px;
}
.date-input {
  width: 138px;
  padding: 4px 8px;
}
.weekstrip {
  display: grid;
  flex: 1 1 340px;
  order: 2;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 4px;
}
.weekday {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
  padding: 3px 2px 4px;
  border-radius: 7px;
  background: var(--panel);
  color: var(--muted);
  line-height: 1.2;
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
  font-size: 14px;
  font-weight: 600;
}
.weekday .count {
  position: absolute;
  top: 2px;
  right: 4px;
  font-size: 9px;
  color: var(--accent);
}
.idle-toggle {
  display: flex;
  order: 3;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  font-size: 12px;
  color: var(--muted);
  cursor: pointer;
  white-space: nowrap;
}
.idle-toggle input {
  width: auto;
}

/* 左侧只保留时间轴，右侧待办栏承载「全天」与全部待办，互不挤压。 */
.board {
  display: grid;
  flex: 1;
  min-height: 0;
  grid-template-columns: minmax(0, 1fr) 252px;
  gap: 12px;
}
.day-panel {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.segmented {
  display: flex;
  flex-shrink: 0;
  align-self: flex-start;
  gap: 2px;
  margin-bottom: 8px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 7px;
  padding: 2px;
}
.segmented button {
  border: none;
  background: transparent;
  color: var(--muted);
  padding: 3px 12px;
  border-radius: 5px;
}
.segmented button.active {
  background: var(--accent-soft);
  color: var(--text);
}
.timeline-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}

.side-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
}
.untimed-zone {
  display: flex;
  flex-shrink: 0;
  flex-direction: column;
  max-height: 150px;
  padding: 8px 10px;
  border-style: dashed;
}
.untimed-list {
  min-height: 32px;
  overflow-y: auto;
}
.untimed-zone h2,
.todos-panel h2 {
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
.untimed-item.journal .kind {
  font-size: 10px;
  color: #b58cff;
  border: 1px solid #b58cff;
  border-radius: 8px;
  padding: 0 5px;
  flex-shrink: 0;
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
.hint {
  margin: 4px 0 0;
}

.todos-panel {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  padding: 10px;
}
.todos-panel h2 .total {
  margin-left: 2px;
  color: var(--text);
}
.add-todo {
  flex-shrink: 0;
  margin-bottom: 6px;
}
.add-todo input {
  width: 100%;
  font-size: 12px;
  padding: 4px 6px;
}
.add-todo.shake {
  animation: shake 0.4s;
}
@keyframes shake {
  0%,
  100% {
    transform: translateX(0);
  }
  25% {
    transform: translateX(-6px);
  }
  75% {
    transform: translateX(6px);
  }
}
.todos-scroll {
  flex: 1;
  min-height: 0;
  margin: 0 -4px;
  padding: 0 4px;
  overflow-y: auto;
}
.todo-item {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 5px 4px;
  border-radius: 6px;
  cursor: pointer;
}
.todo-item:hover {
  background: var(--panel-2);
}
.todo-item.done {
  opacity: 0.6;
}
.todo-item.overdue .todo-title {
  color: var(--danger);
}
.todo-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.todo-title {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.todo-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
  color: var(--muted);
}
.plan-chip {
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 0 7px;
  font-size: 10px;
  white-space: nowrap;
}
.plan-chip.overdue {
  color: var(--danger);
  border-color: var(--danger);
}
.drag-dots {
  color: var(--muted);
  font-size: 11px;
  cursor: grab;
  flex-shrink: 0;
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

@media (max-width: 899px) {
  .weekstrip {
    order: 3;
    flex-basis: 100%;
  }
  .idle-toggle {
    order: 2;
  }
  /* 窄屏不再把时间轴和待办挤在同一屏：整页可滚，时间轴保持可用高度，
     待办栏顺次排在下方。 */
  .page--fixed {
    height: auto;
    overflow: visible;
  }
  .board {
    display: flex;
    flex-direction: column;
    flex: none;
    gap: 10px;
  }
  .day-panel {
    flex: none;
    height: max(320px, 55vh);
  }
  .side-panel {
    flex: none;
  }
  .untimed-zone {
    max-height: 120px;
  }
  .todos-panel {
    flex: none;
    max-height: 320px;
  }
}
</style>
