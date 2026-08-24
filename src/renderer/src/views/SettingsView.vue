<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { captureAccelerator } from '../utils'

const shortcut = ref('')
const mainShortcut = ref('')
const exportDir = ref('')
const idleThresholdSec = ref('')
const minSegmentSec = ref('')
const shortcutError = ref<string | null>(null)
const mainShortcutError = ref<string | null>(null)
const autostart = ref(false)
const autostartError = ref<string | null>(null)
const saved = ref(false)
const capturing = ref<'shortcut' | 'mainShortcut' | null>(null)

async function load(): Promise<void> {
  const all = await window.api.settings.getAll()
  shortcut.value = (all.shortcut as string) || 'Ctrl+Shift+Space'
  mainShortcut.value = (all.mainShortcut as string) || 'Ctrl+Shift+M'
  exportDir.value = (all.exportDir as string) || ''
  idleThresholdSec.value = (all.idleThresholdSec as string) || '300'
  minSegmentSec.value = (all.minSegmentSec as string) || '60'
  shortcutError.value = (all.shortcutError as string) || null
  mainShortcutError.value = (all.mainShortcutError as string) || null
  autostart.value = all.autostart === 'true'
}

async function onToggleAutostart(): Promise<void> {
  try {
    await window.api.settings.set('autostart', autostart.value ? 'true' : 'false')
    autostartError.value = null
  } catch (e) {
    autostart.value = !autostart.value
    autostartError.value = String(e)
  }
}

async function saveAll(): Promise<void> {
  await window.api.settings.set('shortcut', shortcut.value.trim())
  await window.api.settings.set('mainShortcut', mainShortcut.value.trim())
  await window.api.settings.set('exportDir', exportDir.value.trim())
  await window.api.settings.set('idleThresholdSec', idleThresholdSec.value.trim())
  await window.api.settings.set('minSegmentSec', minSegmentSec.value.trim())
  saved.value = true
  setTimeout(() => (saved.value = false), 1500)
  await load()
}

async function pickDir(): Promise<void> {
  const p = await window.api.dialog?.pickDirectory?.()
  if (p) exportDir.value = p
}

function back(): void {
  location.hash = '#/main'
}

function startCapture(target: 'shortcut' | 'mainShortcut'): void {
  capturing.value = target
}

function endCapture(): void {
  capturing.value = null
}

function onCaptureKeydown(e: KeyboardEvent): void {
  if (!capturing.value) return
  e.preventDefault()
  e.stopPropagation()
  const res = captureAccelerator(e)
  if (res.status === 'cancel') {
    endCapture()
    return
  }
  if (res.status === 'captured') {
    if (capturing.value === 'shortcut') shortcut.value = res.accelerator
    else mainShortcut.value = res.accelerator
    endCapture()
  }
}

onMounted(() => {
  load()
  window.addEventListener('keydown', onCaptureKeydown, true)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onCaptureKeydown, true)
})
</script>

<template>
  <div class="page">
    <header class="header">
      <button class="ghost" @click="back">← 返回</button>
      <h1>设置</h1>
    </header>

    <section class="form">
      <label class="field">
        <span>快捷键</span>
        <button
          type="button"
          class="capture"
          :class="{ capturing: capturing === 'shortcut' }"
          :disabled="capturing !== null && capturing !== 'shortcut'"
          @click="startCapture('shortcut')"
        >
          {{ capturing === 'shortcut' ? '请按下组合键…（Esc 取消）' : shortcut || '未设置' }}
        </button>
        <em class="hint">点击后按下组合键自动录入，唤起快速添加小窗口</em>
      </label>
      <p v-if="shortcutError" class="error">{{ shortcutError }}</p>

      <label class="field">
        <span>唤醒主窗口快捷键</span>
        <button
          type="button"
          class="capture"
          :class="{ capturing: capturing === 'mainShortcut' }"
          :disabled="capturing !== null && capturing !== 'mainShortcut'"
          @click="startCapture('mainShortcut')"
        >
          {{ capturing === 'mainShortcut' ? '请按下组合键…（Esc 取消）' : mainShortcut || '未设置' }}
        </button>
        <em class="hint">主窗口隐藏到托盘后，用该快捷键重新唤出</em>
      </label>
      <p v-if="mainShortcutError" class="error">{{ mainShortcutError }}</p>

      <label class="field">
        <span>导出目录</span>
        <div class="dir-row">
          <input v-model="exportDir" spellcheck="false" placeholder="留空则在应用数据目录导出" />
          <button @click="pickDir">选择…</button>
        </div>
      </label>

      <label class="field">
        <span>空闲阈值（秒）</span>
        <input v-model="idleThresholdSec" type="number" min="1" placeholder="300" />
      </label>

      <label class="field">
        <span>防抖时间（秒）</span>
        <input v-model="minSegmentSec" type="number" min="1" placeholder="60" />
        <em class="hint">短于该时长的片段会并入下一个窗口，避免时间段过于零散</em>
      </label>

      <label class="field">
        <div class="check-row">
          <input type="checkbox" v-model="autostart" @change="onToggleAutostart" />
          <span>开机自动启动</span>
        </div>
        <em class="hint">开启后开机时自动启动并最小化到托盘</em>
      </label>
      <p v-if="autostartError" class="error">{{ autostartError }}</p>

      <div class="actions">
        <button class="primary" @click="saveAll">保存</button>
        <span v-if="saved" class="saved">已保存</span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.page {
  max-width: 600px;
  margin: 0 auto;
  padding: 16px;
}
.header {
  display: flex;
  align-items: center;
  gap: 12px;
}
.header h1 {
  font-size: 18px;
  margin: 0;
}
.form {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-top: 16px;
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
.field .hint {
  font-size: 11px;
  font-style: normal;
  color: var(--muted);
}
.capture {
  font: inherit;
  text-align: left;
  color: var(--text);
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px 8px;
  width: 100%;
  cursor: pointer;
}
.capture:focus-visible {
  outline: none;
  border-color: var(--accent);
}
.capture.capturing {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--accent);
}
.capture:disabled {
  opacity: 0.5;
  cursor: default;
}
.capture:disabled:hover {
  border-color: var(--border);
}
.dir-row {
  display: flex;
  gap: 8px;
}
.dir-row input {
  flex: 1;
}
.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.check-row input {
  width: auto;
}
.check-row span {
  font-size: 12px;
  color: var(--muted);
}
.actions {
  display: flex;
  align-items: center;
  gap: 10px;
}
.saved {
  color: var(--done);
  font-size: 12px;
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: -6px 0 0;
}
</style>
