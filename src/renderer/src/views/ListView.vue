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
const offWork = ref(false)

const filter = computed<TodoFilter>(() => ({
  labels: selected.value.size ? Array.from(selected.value) : undefined
}))

const dragId = ref<string | null>(null)
const dropHint = ref<{ id: string; pos: 'before' | 'after' } | null>(null)

// 标签过滤时展示的是全局列表的子集,局部拖拽会破坏不可见项间的相对顺序,故禁用
const canDrag = computed(() => selected.value.size === 0)

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

function isHint(id: string, pos: 'before' | 'after'): boolean {
  return dropHint.value?.id === id && dropHint.value.pos === pos
}

function onDragStart(t: Todo, e: DragEvent): void {
  dragId.value = t.id
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', t.id)
  }
}

function onDragEnd(): void {
  dragId.value = null
  dropHint.value = null
}

function onDragOverItem(t: Todo, e: DragEvent): void {
  if (!dragId.value || dragId.value === t.id) return
  const el = e.currentTarget as HTMLElement | null
  if (!el) return
  const rect = el.getBoundingClientRect()
  const pos = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
  if (dropHint.value?.id !== t.id || dropHint.value.pos !== pos) {
    dropHint.value = { id: t.id, pos }
  }
}

async function moveTodo(beforeId: string | null): Promise<void> {
  const id = dragId.value
  dragId.value = null
  dropHint.value = null
  if (!id) return
  try {
    await window.api.todos.move(id, beforeId)
  } finally {
    await refresh()
  }
}

function onDrop(t: Todo, e: DragEvent): void {
  e.preventDefault()
  const id = dragId.value
  const hint = dropHint.value
  if (!id || !hint || id === t.id) {
    onDragEnd()
    return
  }
  const list = grouped.value.pending
  const targetIdx = list.findIndex((x) => x.id === t.id)
  const draggedIdx = list.findIndex((x) => x.id === id)
  if (targetIdx < 0 || draggedIdx < 0) {
    onDragEnd()
    return
  }
  if (hint.pos === 'before' && draggedIdx === targetIdx - 1) {
    onDragEnd()
    return
  }
  if (hint.pos === 'after' && draggedIdx === targetIdx + 1) {
    onDragEnd()
    return
  }
  let beforeId: string | null
  if (hint.pos === 'before') {
    beforeId = t.id
  } else {
    const next = list.slice(targetIdx + 1).find((x) => x.id !== id)
    beforeId = next ? next.id : null
  }
  void moveTodo(beforeId)
}

function onDropListEnd(e: DragEvent): void {
  e.preventDefault()
  const id = dragId.value
  if (!id) return
  // 按光标位置在 pending 项中解析插入点:第一个"光标在上半部"的项即插入点之前;
  // 遍历完仍无命中(光标在最后一项下半部之后,含 done 区)则移到末尾。
  const scope = e.currentTarget as HTMLElement | null
  const els = Array.from((scope ?? document).querySelectorAll<HTMLElement>('[data-todo-id]'))
  let beforeId: string | null = null
  for (const el of els) {
    const rect = el.getBoundingClientRect()
    if (e.clientY <= rect.top + rect.height / 2) {
      beforeId = el.dataset.todoId ?? null
      break
    }
  }
  const list = grouped.value.pending
  if (beforeId === id) {
    onDragEnd()
    return
  }
  if (beforeId === null && list.length && list[list.length - 1].id === id) {
    onDragEnd()
    return
  }
  void moveTodo(beforeId)
}

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

async function loadOffWork(): Promise<void> {
  const state = await window.api.tracker.getOffWork()
  offWork.value = state.offWork
}

async function toggleOffWork(): Promise<void> {
  const state = await window.api.tracker.setOffWork(!offWork.value)
  offWork.value = state.offWork
}

function goSettings(): void {
  location.hash = '#/settings'
}

onMounted(async () => {
  await refresh()
  await loadOffWork()
})
</script>

<template>
  <div class="page">
    <header class="header">
      <h1>Herbie</h1>
      <div class="actions">
        <button
          class="offwork"
          :class="{ active: offWork }"
          :title="offWork ? '点击恢复记录' : '点击进入下班（停止记录）'"
          @click="toggleOffWork"
        >
          {{ offWork ? '下班中' : '上班中' }}
        </button>
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

    <section class="list" @dragover.prevent @drop="onDropListEnd">
      <article
        v-for="t in grouped.pending"
        :key="t.id"
        class="item"
        :class="{
          expanded: expanded.has(t.id),
          dragging: dragId === t.id,
          'drop-before': isHint(t.id, 'before'),
          'drop-after': isHint(t.id, 'after')
        }"
        :data-todo-id="t.id"
        @dragover.prevent="onDragOverItem(t, $event)"
        @drop.stop="onDrop(t, $event)"
      >
        <div class="row" @click="expand(t)">
          <span
            v-if="canDrag && !expanded.has(t.id)"
            class="drag-handle"
            :draggable="true"
            title="拖动排序"
            @dragstart="onDragStart(t, $event)"
            @dragend="onDragEnd"
          >⠿</span>
          <input class="check" type="checkbox" :checked="false" @click.stop="toggleDone(t, true)" />
          <div class="title-area">
            <div class="title">{{ t.title }}</div>
          </div>
          <span class="meta">{{ formatLocalShort(t.createdAt) }}</span>
        </div>

        <div v-if="expanded.has(t.id)" class="edit" @click.stop>
          <input v-model="editing[t.id]!.title" spellcheck="false" placeholder="标题" />
          <textarea v-model="editing[t.id]!.detail" spellcheck="false" rows="4" placeholder="详情，#标签 自动识别"></textarea>
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
  margin-bottom: 12px;
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
.item.dragging {
  opacity: 0.4;
}
.drag-handle {
  cursor: grab;
  color: var(--muted);
  font-size: 13px;
  line-height: 1;
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.12s ease;
  user-select: none;
  -webkit-user-select: none;
}
.item:hover .drag-handle {
  opacity: 0.6;
}
.drag-handle:hover {
  opacity: 1;
}
.drag-handle:active {
  cursor: grabbing;
}
.item.drop-before {
  box-shadow: 0 -2px 0 0 var(--accent);
}
.item.drop-after {
  box-shadow: 0 2px 0 0 var(--accent);
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
.offwork {
  border-radius: 12px;
  padding: 2px 10px;
  font-size: 12px;
  color: var(--muted);
}
.offwork.active {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}
</style>
