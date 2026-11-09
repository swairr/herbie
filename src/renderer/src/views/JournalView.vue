<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { JournalEntry } from '@shared/types'
import { formatLocalShort, localDateString } from '@shared/time'
import { parseLabels } from '@shared/labels'
import JournalEditor from '../components/JournalEditor.vue'

const entries = ref<JournalEntry[]>([])
const day = ref(todayLocal())
const exporting = ref(false)
const lastExport = ref<string | null>(null)
const editorOpen = ref(false)
const editingEntry = ref<JournalEntry | null>(null)

function todayLocal(): string {
  return localDateString(new Date().toISOString())
}

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
  entries.value = await window.api.journal.list(day.value)
}

function openCreate(): void {
  editingEntry.value = null
  editorOpen.value = true
}

function openEdit(e: JournalEntry): void {
  editingEntry.value = e
  editorOpen.value = true
}

async function remove(e: JournalEntry): Promise<void> {
  await window.api.journal.softDelete(e.id)
  await refresh()
}

function entryHead(e: JournalEntry): string {
  if (e.title && e.title.trim()) return e.title
  return firstLine(e.body)
}

function firstLine(body: string): string {
  const idx = body.indexOf('\n')
  return (idx === -1 ? body : body.slice(0, idx)).trim() || '(无内容)'
}

function labelList(e: JournalEntry): string[] {
  return parseLabels(e.body)
}

async function doExport(): Promise<void> {
  exporting.value = true
  const res = await window.api.journal.export(day.value)
  exporting.value = false
  lastExport.value = res.ok ? res.path ?? '' : `失败：${res.error ?? ''}`
  setTimeout(() => (lastExport.value = null), 3000)
}

function goSettings(): void {
  location.hash = '#/settings'
}

function onSaved(): void {
  void refresh()
}

onMounted(refresh)
</script>

<template>
  <div class="page">
    <header class="header">
      <h1>日志</h1>
      <div class="actions">
        <button class="primary" @click="openCreate">新建条目</button>
        <button :disabled="exporting" @click="doExport">
          {{ exporting ? '导出中…' : '导出当日' }}
        </button>
        <button class="ghost" title="设置" @click="goSettings">⚙</button>
      </div>
    </header>

    <div v-if="lastExport" class="toast">{{ lastExport }}</div>

    <section class="daybar">
      <button @click="shiftDay(-1)">‹</button>
      <input type="date" :value="day" @change="onDayInput" />
      <button @click="shiftDay(1)">›</button>
      <button class="ghost" @click="(() => { day = todayLocal(); void refresh() })">今天</button>
    </section>

    <section class="entries">
      <article
        v-for="e in entries"
        :key="e.id"
        class="entry"
        @click="openEdit(e)"
      >
        <div class="entry-head">
          <span class="entry-title">
            <span v-if="e.time" class="entry-time">{{ e.time }}</span>
            {{ entryHead(e) }}
          </span>
          <button class="danger sm" @click.stop="remove(e)">删除</button>
        </div>
        <pre v-if="e.title" class="entry-body">{{ e.body }}</pre>
        <div v-if="labelList(e).length" class="entry-labels">
          <span v-for="l in labelList(e)" :key="l" class="tag">#{{ l }}</span>
        </div>
        <div class="entry-meta">{{ formatLocalShort(e.createdAt) }}</div>
      </article>
      <p v-if="entries.length === 0" class="muted empty">当日暂无日志</p>
    </section>

    <JournalEditor
      v-if="editorOpen"
      :entry="editingEntry"
      :default-date="day"
      @saved="onSaved"
      @close="editorOpen = false"
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
  word-break: break-all;
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
.entries {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.entry {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 12px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.entry:hover {
  border-color: var(--accent);
}
.entry-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.entry-title {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.entry-time {
  font-family: ui-monospace, monospace;
  font-size: 11px;
  color: #b58cff;
  margin-right: 4px;
}
.entry-body {
  margin: 0;
  font-family: inherit;
  font-size: 13px;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text);
  max-height: 320px;
  overflow: auto;
}
.entry-labels {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.tag {
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 1px 8px;
  font-size: 11px;
  color: var(--muted);
}
.entry-meta {
  font-size: 11px;
  color: var(--muted);
}
.sm {
  padding: 2px 8px;
  font-size: 12px;
  flex-shrink: 0;
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
