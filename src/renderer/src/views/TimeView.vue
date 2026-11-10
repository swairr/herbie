<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { Segment, Todo } from '@shared/types'
import { formatLocalShort, localDateString, formatDuration, durationMs } from '@shared/time'
import { aggregateByProcess, aggregateByTodo, aggregateIdle } from '@shared/segments-agg'
import SegmentEditor from '../components/SegmentEditor.vue'

const segments = ref<Segment[]>([])
const todos = ref<Todo[]>([])
const day = ref(todayLocal())
const exporting = ref(false)
const lastExport = ref<string | null>(null)
const editing = ref<Segment | null>(null)

function todayLocal(): string {
  return localDateString(new Date().toISOString())
}

const todoTitles = computed<Record<string, string>>(() => {
  const map: Record<string, string> = {}
  for (const t of todos.value) map[t.id] = t.title
  return map
})

const procRows = computed(() => aggregateByProcess(segments.value))
const todoRows = computed(() => aggregateByTodo(segments.value, todoTitles.value))
const idleMs = computed(() => aggregateIdle(segments.value))

function shiftDay(delta: number): void {
  const [y, m, d] = day.value.split('-').map(Number)
  const next = new Date(y, m - 1, d + delta, 12, 0, 0, 0)
  day.value = localDateString(next.toISOString())
  void refresh()
}

function onDayInput(e: Event): void {
  const v = (e.target as HTMLInputElement).value
  if (v) {
    day.value = v
    void refresh()
  }
}

async function refresh(): Promise<void> {
  segments.value = await window.api.segments.list(day.value)
  if (todos.value.length === 0) {
    todos.value = await window.api.todos.list()
  }
}

function segDuration(s: Segment): string {
  return formatDuration(durationMs(s.startAt, s.endAt))
}

function segLabel(s: Segment): string {
  if (s.note.trim().length > 0) return s.note
  if (s.title) return s.title
  if (s.kind === 'idle') return '空闲'
  return s.processName
}

function todoTitleOf(s: Segment): string {
  if (!s.todoId) return ''
  return todoTitles.value[s.todoId] ?? '(已删除)'
}

async function doExport(): Promise<void> {
  exporting.value = true
  const res = await window.api.time.export(day.value)
  exporting.value = false
  lastExport.value = res.ok ? res.path ?? '' : `失败：${res.error ?? ''}`
  setTimeout(() => (lastExport.value = null), 3000)
}


function onSaved(): void {
  void refresh()
}

onMounted(refresh)
</script>

<template>
  <div class="page">
    <header class="header">
      <h1>时间</h1>
      <div class="actions">
        <button :disabled="exporting" @click="doExport">
          {{ exporting ? '导出中…' : '导出时间记录' }}
        </button>
      </div>
    </header>

    <div v-if="lastExport" class="toast">{{ lastExport }}</div>

    <section class="daybar">
      <button @click="shiftDay(-1)">‹</button>
      <input type="date" :value="day" @change="onDayInput" />
      <button @click="shiftDay(1)">›</button>
      <button class="ghost" @click="(() => { day = todayLocal(); void refresh() })">今天</button>
    </section>

    <section class="cards">
      <div class="card">
        <h2>进程时长</h2>
        <table>
          <thead><tr><th>进程</th><th class="num">时长</th></tr></thead>
          <tbody>
            <tr v-for="r in procRows" :key="r.key">
              <td class="proc">{{ r.label }}</td>
              <td class="num">{{ formatDuration(r.ms) }}</td>
            </tr>
            <tr v-if="procRows.length === 0"><td class="muted">(无)</td><td></td></tr>
          </tbody>
        </table>
      </div>
      <div class="card">
        <h2>Todo 时长</h2>
        <table>
          <thead><tr><th>Todo</th><th class="num">时长</th></tr></thead>
          <tbody>
            <tr v-for="r in todoRows" :key="r.key">
              <td>{{ r.label }}</td>
              <td class="num">{{ formatDuration(r.ms) }}</td>
            </tr>
            <tr v-if="todoRows.length === 0"><td class="muted">(无)</td><td></td></tr>
          </tbody>
        </table>
      </div>
    </section>

    <p v-if="idleMs > 0" class="idle-line">空闲合计：{{ formatDuration(idleMs) }}</p>

    <section class="segs">
      <article
        v-for="s in segments"
        :key="s.id"
        class="seg"
        :class="{ idle: s.kind === 'idle' }"
        @click="editing = s"
      >
        <div class="seg-time">
          {{ formatLocalShort(s.startAt) }} – {{ s.endAt ? formatLocalShort(s.endAt) : '进行中' }}
        </div>
        <div class="seg-main">
          <span class="seg-proc">{{ s.processName || (s.kind === 'idle' ? '空闲' : '-') }}</span>
          <span class="seg-label">{{ segLabel(s) }}</span>
        </div>
        <div class="seg-meta">
          <span class="seg-dur">{{ segDuration(s) }}</span>
          <span v-if="todoTitleOf(s)" class="seg-todo">{{ todoTitleOf(s) }}</span>
        </div>
      </article>
      <p v-if="segments.length === 0" class="muted empty">当日暂无片段</p>
    </section>

    <SegmentEditor
      v-if="editing"
      :segment="editing"
      :todos="todos"
      @saved="onSaved"
      @close="editing = null"
    />
  </div>
</template>

<style scoped>
.page {
  max-width: 760px;
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
.actions {
  display: flex;
  gap: 8px;
}
.toast {
  margin: 8px 0;
  padding: 8px 10px;
  background: var(--accent-soft);
  border-radius: 6px;
  font-size: 12px;
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
.cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
@media (max-width: 620px) {
  .cards {
    grid-template-columns: 1fr;
  }
}
.card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px;
}
.card h2 {
  font-size: 13px;
  margin: 0 0 6px;
  color: var(--muted);
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
td,
th {
  text-align: left;
  padding: 2px 4px;
}
.num {
  text-align: right;
  white-space: nowrap;
}
.proc {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.idle-line {
  margin: 10px 0;
  color: var(--muted);
  font-size: 12px;
}
.segs {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 14px;
}
.seg {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: 10px;
  align-items: center;
}
.seg:hover {
  border-color: var(--accent);
}
.seg.idle {
  opacity: 0.7;
}
.seg-time {
  font-size: 11px;
  color: var(--muted);
  white-space: nowrap;
}
.seg-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.seg-proc {
  font-size: 11px;
  color: var(--muted);
}
.seg-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.seg-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  font-size: 11px;
  color: var(--muted);
}
.seg-dur {
  font-weight: 600;
  color: var(--text);
}
.seg-todo {
  font-size: 10px;
}
.muted {
  color: var(--muted);
  font-size: 12px;
}
.empty {
  text-align: center;
  padding: 20px 0;
}
</style>
