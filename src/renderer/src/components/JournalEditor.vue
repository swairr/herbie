<script setup lang="ts">
import { computed, ref } from 'vue'
import type { JournalEntry } from '@shared/types'
import { parseLabels } from '@shared/labels'

// 抽出「日志编辑器」:旧「日志」页与日程时间线的日志轨 / 全天区共用。
// `time` 可选:勾选后记录本地墙钟时刻,进入当天的日志窄轨(ADR 0007)。
const props = defineProps<{
  entry: JournalEntry | null
  defaultDate: string
}>()

const emit = defineEmits<{
  (e: 'saved'): void
  (e: 'close'): void
}>()

const title = ref(props.entry?.title ?? '')
const body = ref(props.entry?.body ?? '')
const date = ref(props.entry?.date ?? props.defaultDate)
const useTime = ref(Boolean(props.entry?.time))
const time = ref(props.entry?.time ?? '')
const saveError = ref<string | null>(null)
const busy = ref(false)

const labels = computed(() => parseLabels(body.value))

async function save(): Promise<void> {
  saveError.value = null
  if (body.value.trim().length === 0) {
    saveError.value = '正文不能为空'
    return
  }
  if (useTime.value && !time.value) {
    saveError.value = '请填写时刻，或取消「记录具体时刻」'
    return
  }
  busy.value = true
  try {
    const payload = {
      title: title.value || null,
      body: body.value,
      date: date.value,
      time: useTime.value ? time.value : null
    }
    if (props.entry) {
      await window.api.journal.update(props.entry.id, payload)
    } else {
      await window.api.journal.create(payload)
    }
    emit('saved')
    emit('close')
  } catch (e) {
    saveError.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="modal">
      <h3>{{ entry ? '编辑日志' : '新建日志' }}</h3>
      <label class="field">
        <span>标题（可选）</span>
        <input v-model="title" spellcheck="false" placeholder="无标题则以正文首行展示" />
      </label>
      <label class="field">
        <span>正文（必填）</span>
        <textarea
          v-model="body"
          spellcheck="false"
          rows="8"
          placeholder="记录思考、会议纪要…#标签 自动识别"
        ></textarea>
        <span v-if="labels.length" class="hint">将解析标签：{{ labels.join(', ') }}</span>
      </label>
      <label class="field">
        <span>归属日</span>
        <input v-model="date" type="date" />
      </label>
      <label class="check-row">
        <input v-model="useTime" type="checkbox" />
        <span>记录具体时刻（进入时间线日志轨）</span>
      </label>
      <label v-if="useTime" class="field">
        <span>时刻</span>
        <input v-model="time" type="time" step="60" />
      </label>
      <p v-if="saveError" class="error">{{ saveError }}</p>
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
