<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const result = ref('')
const error = ref('')
const events = ref<string[]>([])
let unlisten: (() => void) | null = null

async function ping(): Promise<void> {
  try {
    result.value = await invoke<string>('ping')
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}

onMounted(async () => {
  try {
    unlisten = await listen<string>('power://event', (e) => {
      events.value.unshift(e.payload)
      if (events.value.length > 8) events.value.pop()
    })
  } catch (e) {
    error.value = `listen 失败: ${String(e)}`
  }
})

onUnmounted(() => {
  unlisten?.()
})
</script>

<template>
  <div class="tauri-ping-demo">
    <button @click="ping">Tauri Ping</button>
    <span v-if="result" class="ok">→ {{ result }}</span>
    <span v-if="error" class="err">err: {{ error }}</span>
    <ul v-if="events.length" class="ev">
      <li v-for="(e, i) in events" :key="i">{{ e }}</li>
    </ul>
  </div>
</template>

<style scoped>
.tauri-ping-demo {
  position: fixed;
  top: 6px;
  right: 12px;
  z-index: 9999;
  font-size: 12px;
  background: var(--bg, #fff);
  border: 1px solid var(--accent, #36c);
  border-radius: 6px;
  padding: 4px 8px;
  max-width: 220px;
}
.tauri-ping-demo button {
  cursor: pointer;
  border: 1px solid var(--accent, #36c);
  background: transparent;
  color: var(--accent, #36c);
  border-radius: 6px;
  padding: 2px 8px;
}
.ok {
  margin-left: 6px;
  color: var(--accent, #36c);
}
.err {
  margin-left: 6px;
  color: #c33;
}
.ev {
  margin: 4px 0 0;
  padding-left: 16px;
  color: var(--muted, #666);
}
</style>