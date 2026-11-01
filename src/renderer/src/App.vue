<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { TrackingStatus } from '@shared/types'
import ListView from './views/ListView.vue'
import ScheduleView from './views/ScheduleView.vue'
import TimeView from './views/TimeView.vue'
import JournalView from './views/JournalView.vue'
import SettingsView from './views/SettingsView.vue'
import QuickAddView from './views/QuickAddView.vue'

const route = ref(getRoute())
// Tracking Unavailable 状态(ADR 0004):不可用期间顶部横幅常驻不可关(Q17 决策)。
const tracking = ref<TrackingStatus | null>(null)

function getRoute(): string {
  const h = location.hash.replace(/^#\/?/, '')
  return h || 'main'
}

onMounted(() => {
  window.addEventListener('hashchange', () => {
    route.value = getRoute()
  })
  // 不可用与否在进程生命周期内是常驻事实,启动拉一次即可(动态探测留待未来实现追踪时)。
  window.api.tracker.getStatus().then((s) => (tracking.value = s))
})

const showTabs = computed(
  () =>
    route.value === 'main' ||
    route.value === 'schedule' ||
    route.value === 'time' ||
    route.value === 'journal'
)

const view = computed(() => {
  if (route.value === 'quickadd') return QuickAddView
  if (route.value === 'settings') return SettingsView
  if (route.value === 'schedule') return ScheduleView
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
    <div v-if="tracking && !tracking.available" class="tracking-banner">
      活动记录不可用：{{ tracking.reason }}
    </div>
    <nav v-if="showTabs" class="tabs">
      <button :class="{ active: route === 'main' }" @click="goTo('main')">待办</button>
      <button :class="{ active: route === 'schedule' }" @click="goTo('schedule')">日程</button>
      <button :class="{ active: route === 'time' }" @click="goTo('time')">时间</button>
      <button :class="{ active: route === 'journal' }" @click="goTo('journal')">日志</button>
    </nav>
    <component :is="view" />
  </template>
</template>

<style scoped>
.tracking-banner {
  background: var(--panel-2);
  border-bottom: 1px solid var(--border);
  color: var(--muted);
  font-size: 12px;
  padding: 6px 12px;
  text-align: center;
}
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