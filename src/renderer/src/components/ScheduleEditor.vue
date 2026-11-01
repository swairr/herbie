<script setup lang="ts">
import { ref, watch } from 'vue'
import type { Todo, TodoSchedule } from '@shared/types'

const props = defineProps<{
  todo: Todo
  defaultDate: string
}>()

const emit = defineEmits<{
  (e: 'updated', todo: Todo): void
  (e: 'close'): void
}>()

const date = ref(props.todo.plannedDate ?? props.defaultDate)
const useTime = ref(Boolean(props.todo.plannedStart && props.todo.plannedEnd))
const start = ref(props.todo.plannedStart ?? '09:00')
const end = ref(props.todo.plannedEnd ?? '10:00')
const error = ref<string | null>(null)
const busy = ref(false)

watch(useTime, (on) => {
  if (!on) return
  if (!start.value) start.value = '09:00'
  if (!end.value) end.value = '10:00'
})

function currentSlot(): TodoSchedule {
  if (!date.value) {
    return { plannedDate: null, plannedStart: null, plannedEnd: null }
  }
  return {
    plannedDate: date.value,
    plannedStart: useTime.value ? start.value || null : null,
    plannedEnd: useTime.value ? end.value || null : null
  }
}

async function save(): Promise<void> {
  error.value = null
  if (!date.value) {
    error.value = '请先选择日期'
    return
  }
  if (useTime.value) {
    if (!start.value || !end.value) {
      error.value = '请填写开始和结束时间'
      return
    }
    if (start.value >= end.value) {
      error.value = '结束时间必须晚于开始时间'
      return
    }
  }
  busy.value = true
  try {
    const updated = await window.api.todos.schedule(props.todo.id, currentSlot())
    emit('updated', updated)
    emit('close')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}

async function clear(): Promise<void> {
  busy.value = true
  error.value = null
  try {
    const updated = await window.api.todos.schedule(props.todo.id, {
      plannedDate: null,
      plannedStart: null,
      plannedEnd: null
    })
    emit('updated', updated)
    emit('close')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}

async function toggleDone(): Promise<void> {
  busy.value = true
  error.value = null
  try {
    const updated = await window.api.todos.toggle(props.todo.id, !props.todo.completedAt)
    emit('updated', updated)
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
      <h3 class="todo-title">{{ todo.title }}</h3>

      <label class="field">
        <span>计划日期</span>
        <input v-model="date" type="date" />
      </label>

      <label class="check-row">
        <input v-model="useTime" type="checkbox" />
        <span>安排具体时间段</span>
      </label>

      <div v-if="useTime" class="time-row">
        <label class="field">
          <span>开始</span>
          <input v-model="start" type="time" step="900" />
        </label>
        <label class="field">
          <span>结束</span>
          <input v-model="end" type="time" step="900" />
        </label>
      </div>

      <p v-if="error" class="error">{{ error }}</p>

      <div class="modal-actions">
        <button v-if="todo.plannedDate" class="danger" :disabled="busy" @click="clear">清除安排</button>
        <button class="ghost" :disabled="busy" @click="toggleDone">
          {{ todo.completedAt ? '取消完成' : '完成' }}
        </button>
        <span class="spacer"></span>
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
  width: 400px;
  max-width: 92vw;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.todo-title {
  margin: 0;
  font-size: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--muted);
}
.check-row input {
  width: auto;
}
.time-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
}
.modal-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.spacer {
  flex: 1;
}
</style>
