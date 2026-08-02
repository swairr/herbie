<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { LabelCount, Todo, TodoFilter } from '@shared/types'
import { formatLocalShort } from '@shared/time'
import { parseLabels } from '@shared/labels'
import { groupItems } from '../utils'
import DetailText from '../components/DetailText.vue'

const all = ref<Todo[]>([])
const labels = ref<LabelCount[]>([])
const selected = ref<Set<string>>(new Set())
const expanded = ref<Set<string>>(new Set())
const doneOpen = ref(false)
const editing = ref<Record<string, { title: string; detail: string }>>({})
const exporting = ref(false)
const lastExport = ref<string | null>(null)

const filter = computed<TodoFilter>(() => ({
  labels: selected.value.size ? Array.from(selected.value) : undefined
}))

async function refresh(): Promise<void> {
  all.value = await window.api.todos.list(filter.value)
  labels.value = await window.api.todos.labels()
}

async function toggleLabel(label: string): Promise<void> {
  if (selected.value.has(label)) selected.value.delete(label)
  else selected.value.add(label)
  selected.value = new Set(selected.value)
  await refresh()
}

const grouped = computed(() => groupItems(all.value))

function expand(t: Todo): void {
  const s = new Set(expanded.value)
  if (s.has(t.id)) s.delete(t.id)
  else {
    s.add(t.id)
    editing.value[t.id] = { title: t.title, detail: t.detail }
  }
  expanded.value = s
}

async function save(t: Todo): Promise<void> {
  const ed = editing.value[t.id]
  if (!ed) return
  const updated = await window.api.todos.update(t.id, {
    title: ed.title,
    detail: ed.detail
  })
  const idx = all.value.findIndex((x) => x.id === t.id)
  if (idx >= 0) all.value[idx] = updated
  delete editing.value[t.id]
  expanded.value = new Set([...expanded.value].filter((id) => id !== t.id))
  await refresh()
}

async function toggleDone(t: Todo, done: boolean): Promise<void> {
  const updated = await window.api.todos.toggle(t.id, done)
  const idx = all.value.findIndex((x) => x.id === t.id)
  if (idx >= 0) all.value[idx] = updated
}

async function remove(t: Todo): Promise<void> {
  await window.api.todos.softDelete(t.id)
  await refresh()
}

function labelList(t: Todo): string[] {
  return parseLabels(t.detail)
}

async function doExport(): Promise<void> {
  exporting.value = true
  const res = await window.api.export.exportMarkdown()
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
      <h1>Herbie</h1>
      <div class="actions">
        <button :disabled="exporting" @click="doExport">
          {{ exporting ? '导出中…' : '导出 Markdown' }}
        </button>
        <button class="ghost" title="设置" @click="goSettings">⚙</button>
      </div>
    </header>

    <div v-if="lastExport" class="toast">{{ lastExport }}</div>

    <section v-if="labels.length" class="labelbar">
      <button
        v-for="l in labels"
        :key="l.label"
        class="chip-btn"
        :class="{ active: selected.has(l.label) }"
        @click="toggleLabel(l.label)"
      >
        #{{ l.label }} <span class="cnt">{{ l.count }}</span>
      </button>
    </section>

    <section class="list">
      <article
        v-for="t in grouped.pending"
        :key="t.id"
        class="item"
        :class="{ expanded: expanded.has(t.id) }"
      >
        <div class="row" @click="expand(t)">
          <input class="check" type="checkbox" :checked="false" @click.stop="toggleDone(t, true)" />
          <div class="title-area">
            <div class="title">{{ t.title }}</div>
          </div>
          <span class="meta">{{ formatLocalShort(t.createdAt) }}</span>
        </div>

        <div v-if="expanded.has(t.id)" class="edit" @click.stop>
          <input v-model="editing[t.id]!.title" placeholder="标题" />
          <textarea v-model="editing[t.id]!.detail" rows="4" placeholder="详情，#标签 自动识别"></textarea>
          <div class="edit-actions">
            <span v-if="labelList(t).length" class="hint">将解析标签：{{ labelList(t).join(', ') }}</span>
            <div>
              <button class="ghost" @click="expand(t)">取消</button>
              <button class="primary" @click="save(t)">保存</button>
            </div>
          </div>
        </div>
        <div v-else class="preview">
          <DetailText :text="t.detail" />
        </div>
      </article>

      <section class="done-group">
        <button class="done-head" @click="doneOpen = !doneOpen">
          已完成 ({{ grouped.done.length }}) {{ doneOpen ? '▾' : '▸' }}
        </button>
        <div v-show="doneOpen">
          <article v-for="t in grouped.done" :key="t.id" class="item done">
            <div class="row">
              <input
                class="check"
                type="checkbox"
                :checked="true"
                @click="toggleDone(t, false)"
              />
              <div class="title-area">
                <div class="title strokethrough">{{ t.title }}</div>
              </div>
              <span class="meta">{{ formatLocalShort(t.completedAt) }}</span>
              <button class="danger" @click="remove(t)">删除</button>
            </div>
            <div class="preview">
              <DetailText :text="t.detail" />
            </div>
          </article>
        </div>
      </section>
    </section>
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
.labelbar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 12px 0;
}
.chip-btn {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 2px 10px;
  font-size: 12px;
  color: var(--muted);
}
.chip-btn.active {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}
.cnt {
  opacity: 0.7;
  margin-left: 2px;
}
.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.item {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 10px;
}
.item.expanded {
  border-color: var(--accent);
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}
.title-area {
  flex: 1;
  min-width: 0;
}
.title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.strokethrough {
  text-decoration: line-through;
  color: var(--muted);
}
.meta {
  color: var(--muted);
  font-size: 11px;
  flex-shrink: 0;
}
.check {
  width: auto;
  cursor: pointer;
}
.preview {
  padding-left: 28px;
}
.edit {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
}
.edit-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}
.hint {
  font-size: 11px;
  color: var(--muted);
}
.done-group {
  margin-top: 16px;
}
.done-head {
  width: 100%;
  text-align: left;
  background: transparent;
  border-color: transparent;
  color: var(--muted);
}
.done .row {
  cursor: default;
}
</style>