<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Segment, Todo } from '@shared/types'

// 抽出「编辑片段」弹层:旧「时间」页与日程时间线的实际轨共用。
// 只编辑 note + todoId(片段边界编辑不在本轮范围)。
const props = defineProps<{
  segment: Segment
  todos: Todo[]
}>()

const emit = defineEmits<{
  (e: 'saved'): void
  (e: 'close'): void
}>()

const note = ref(props.segment.note)
const todoId = ref<string | null>(props.segment.todoId)
const query = ref(props.segment.todoId ? titleOf(props.segment.todoId) : '')
const menuOpen = ref(false)
const busy = ref(false)
const error = ref<string | null>(null)

const options = computed(() => {
  const q = query.value.trim().toLowerCase()
  const list = q ? props.todos.filter((t) => t.title.toLowerCase().includes(q)) : props.todos
  return list.slice(0, 20)
})

function titleOf(id: string): string {
  return props.todos.find((t) => t.id === id)?.title ?? ''
}

function pick(t: Todo): void {
  todoId.value = t.id
  query.value = t.title
  menuOpen.value = false
}

function clear(): void {
  todoId.value = null
  query.value = ''
  menuOpen.value = false
}

async function save(): Promise<void> {
  busy.value = true
  error.value = null
  try {
    await window.api.segments.update(props.segment.id, {
      note: note.value,
      todoId: todoId.value
    })
    emit('saved')
    emit('close')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="modal">
      <h3>编辑片段</h3>
      <label class="field">
        <span>备注</span>
        <textarea v-model="note" spellcheck="false" rows="3" placeholder="为这段时间加一句说明"></textarea>
      </label>
      <label class="field">
        <span>关联 Todo</span>
        <input v-model="query" spellcheck="false" placeholder="搜索 Todo…" @focus="menuOpen = true" />
        <div v-if="menuOpen" class="todo-menu">
          <button v-for="t in options" :key="t.id" @click="pick(t)">{{ t.title }}</button>
          <button class="ghost clear" @click="clear">取消关联</button>
        </div>
      </label>
      <p v-if="error" class="error">{{ error }}</p>
      <div class="modal-actions">
        <button class="ghost" :disabled="busy" @click="emit('close')">取消</button>
        <button class="primary" :disabled="busy" @click="save">保存</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
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
  width: 420px;
  max-width: 92vw;
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
  position: relative;
}
.field > span {
  font-size: 12px;
  color: var(--muted);
}
.todo-menu {
  position: absolute;
  bottom: 36px;
  left: 0;
  right: 0;
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: 6px;
  max-height: 200px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  z-index: 60;
}
.todo-menu button {
  border: none;
  border-radius: 0;
  text-align: left;
  background: transparent;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.todo-menu button:hover {
  background: var(--accent-soft);
}
.todo-menu .clear {
  color: var(--danger);
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
