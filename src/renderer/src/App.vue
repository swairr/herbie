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
const viewSlot = ref<HTMLElement | null>(null)
// Tracking Unavailable 状态(ADR 0004):不可用期间顶部横幅常驻不可关(Q17 决策)。
const tracking = ref<TrackingStatus | null>(null)

function getRoute(): string {
  const h = location.hash.replace(/^#\/?/, '')
  // 日程是默认首页(日程以本地自然日为锚点,ADR 0006)。
  return h || 'schedule'
}

function onHashChange(): void {
  route.value = getRoute()
  // 切换页签回到页首,避免从长页面(如设置)带着滚动位置进入下一页。
  window.scrollTo({ top: 0 })
  viewSlot.value?.scrollTo({ top: 0 })
}

onMounted(() => {
  window.addEventListener('hashchange', onHashChange)
  // 不可用与否在进程生命周期内是常驻事实,启动拉一次即可(动态探测留待未来实现追踪时)。
  window.api.tracker.getStatus().then((s) => (tracking.value = s))
})

const showTabs = computed(
  () =>
    route.value === 'main' ||
    route.value === 'schedule' ||
    route.value === 'time' ||
    route.value === 'journal' ||
    route.value === 'settings'
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
    <div class="app-shell">
      <div v-if="tracking && !tracking.available" class="tracking-banner">
        活动记录不可用：{{ tracking.reason }}
      </div>
      <div v-if="showTabs" class="tabs-bar">
        <nav class="tabs">
          <div class="tab-list">
            <button :class="{ active: route === 'schedule' }" @click="goTo('schedule')">日程</button>
            <button :class="{ active: route === 'main' }" @click="goTo('main')">待办</button>
            <button :class="{ active: route === 'time' }" @click="goTo('time')">时间</button>
            <button :class="{ active: route === 'journal' }" @click="goTo('journal')">日志</button>
            <button :class="{ active: route === 'settings' }" @click="goTo('settings')">设置</button>
          </div>
        </nav>
      </div>
      <div ref="viewSlot" class="view-slot">
        <component :is="view" />
      </div>
    </div>
  </template>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}
.view-slot {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.tracking-banner {
  background: var(--panel-2);
  border-bottom: 1px solid var(--border);
  color: var(--muted);
  font-size: 12px;
  padding: 6px 12px;
  text-align: center;
}
.tabs-bar {
  position: sticky;
  top: 0;
  z-index: 20;
  background: var(--bg);
  border-bottom: 1px solid var(--border);
}
.tabs {
  display: flex;
  align-items: center;
  padding: 6px var(--page-x);
  max-width: var(--page-max);
  margin: 0 auto;
}
.tab-list {
  display: flex;
  gap: 4px;
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