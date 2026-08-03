<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { JournalEntry } from '@shared/types'
import { formatLocalShort, localDateString } from '@shared/time'
import { parseLabels } from '@shared/labels'

const entries = ref<JournalEntry[]>([])
const day = ref(todayLocal())
const exporting = ref(false)
const lastExport = ref<string | null>(null)

interface EditorState {
  id: string | null
  title: string
  body: string
  date: string
}
const editor = ref<EditorState | null>(null)
const saveError = ref<string | null>(null)

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
  editor.value = { id: null, title: '', body: '', date: day.value }
  saveError.value = null
}

function openEdit(e: JournalEntry): void {
  editor.value = { id: e.id, title: e.title ?? '', body: e.body, date: e.date }
  saveError.value = null
}

function closeEditor(): void {
  editor.value = null
  saveError.value = null
}

async function save(): Promise<void> {
  if (!editor.value) return
  const { id, title, body, date } = editor.value
  if (body.trim().length === 0) {
    saveError.value = '正文不能为空'
    return
  }
  try {
    if (id) {
      await window.api.journal.update(id, { title: title || null, body, date })
    } else {
      await window.api.journal.create({ title: title || null, body, date })
    }
    closeEditor()
    await refresh()
  } catch (e) {
    saveError.value = e instanceof Error ? e.message : String(e)
  }
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

function editorLabelPreview(): string[] {
  return editor.value ? parseLabels(editor.value.body) : []
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
          <span class="entry-title">{{ entryHead(e) }}</span>
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

    <div v-if="editor" class="overlay" @click.self="closeEditor">
      <div class="modal">
        <h3>{{ editor.id ? '编辑日志' : '新建日志' }}</h3>
        <label class="field">
          <span>标题（可选）</span>
          <input v-model="editor.title" placeholder="无标题则以正文首行展示" />
        </label>
        <label class="field">
          <span>正文（必填）</span>
          <textarea
            v-model="editor.body"
            rows="8"
            placeholder="记录思考、会议纪要…#标签 自动识别"
          ></textarea>
          <span v-if="editorLabelPreview().length" class="hint">
            将解析标签：{{ editorLabelPreview().join(', ') }}
          </span>
        </label>
        <label class="field">
          <span>归属日</span>
          <input type="date" v-model="editor.date" />
        </label>
        <p v-if="saveError" class="error">{{ saveError }}</p>
        <div class="modal-actions">
          <button class="ghost" @click="closeEditor">取消</button>
          <button class="primary" @click="save">保存</button>
        </div>
      </div>
    </div>
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
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}
.modal {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 16px;
  width: 520px;
  max-width: 92vw;
  max-height: 88vh;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.modal h3 {
  margin: 0;
  font-size: 15px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field > span {
  font-size: 12px;
  color: var(--muted);
}
.hint {
  font-size: 11px;
  color: var(--muted);
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>