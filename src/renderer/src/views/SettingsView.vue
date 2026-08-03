<script setup lang="ts">
import { onMounted, ref } from 'vue'

const shortcut = ref('')
const exportDir = ref('')
const idleThresholdSec = ref('')
const shortcutError = ref<string | null>(null)
const saved = ref(false)

async function load(): Promise<void> {
  const all = await window.api.settings.getAll()
  shortcut.value = (all.shortcut as string) || 'Ctrl+Shift+Space'
  exportDir.value = (all.exportDir as string) || ''
  idleThresholdSec.value = (all.idleThresholdSec as string) || '300'
  shortcutError.value = (all.shortcutError as string) || null
}

async function saveAll(): Promise<void> {
  await window.api.settings.set('shortcut', shortcut.value.trim())
  await window.api.settings.set('exportDir', exportDir.value.trim())
  await window.api.settings.set('idleThresholdSec', idleThresholdSec.value.trim())
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

onMounted(load)
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
        <input v-model="shortcut" placeholder="Ctrl+Shift+Space" />
      </label>
      <p v-if="shortcutError" class="error">{{ shortcutError }}</p>

      <label class="field">
        <span>导出目录</span>
        <div class="dir-row">
          <input v-model="exportDir" placeholder="留空则在应用数据目录导出" />
          <button @click="pickDir">选择…</button>
        </div>
      </label>

      <label class="field">
        <span>空闲阈值（秒）</span>
        <input v-model="idleThresholdSec" type="number" min="1" placeholder="300" />
      </label>

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
.dir-row {
  display: flex;
  gap: 8px;
}
.dir-row input {
  flex: 1;
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