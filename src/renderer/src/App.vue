<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import ListView from './views/ListView.vue'
import TimeView from './views/TimeView.vue'
import JournalView from './views/JournalView.vue'
import SettingsView from './views/SettingsView.vue'
import QuickAddView from './views/QuickAddView.vue'

const route = ref(getRoute())

function getRoute(): string {
  const h = location.hash.replace(/^#\/?/, '')
  return h || 'main'
}

onMounted(() => {
  window.addEventListener('hashchange', () => {
    route.value = getRoute()
  })
})

const showTabs = computed(() => route.value === 'main' || route.value === 'time' || route.value === 'journal')

const view = computed(() => {
  if (route.value === 'quickadd') return QuickAddView
  if (route.value === 'settings') return SettingsView
  if (route.value === 'time') return TimeView
  if (route.value === 'journal') return JournalView
  return ListView
})

function goTo(r: string): void {
  location.hash = `#/${r}`
}
</script>

<template>
  <component v-if="route === 'quickadd'" :is="view" />
  <template v-else>
    <nav v-if="showTabs" class="tabs">
      <button :class="{ active: route === 'main' }" @click="goTo('main')">待办</button>
      <button :class="{ active: route === 'time' }" @click="goTo('time')">时间</button>
      <button :class="{ active: route === 'journal' }" @click="goTo('journal')">日志</button>
    </nav>
    <component :is="view" />
  </template>
</template>

<style scoped>
.tabs {
  display: flex;
  gap: 4px;
  padding: 6px 12px 0;
  max-width: 760px;
  margin: 0 auto;
}
.tabs button {
  border-bottom: 2px solid transparent;
  border-radius: 6px 6px 0 0;
  padding: 6px 14px;
  color: var(--muted);
  background: transparent;
  border-color: transparent;
}
.tabs button.active {
  color: var(--text);
  border-bottom-color: var(--accent);
}
.tabs button:hover {
  color: var(--text);
}
</style>