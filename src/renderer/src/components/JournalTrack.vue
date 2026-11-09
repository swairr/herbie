<script setup lang="ts">
import { computed } from 'vue'
import type { JournalEntry } from '@shared/types'
import { layoutRanges, minutesToTime, type MinuteRange } from '@shared/schedule'
import type { TimedJournal } from '@shared/timeline'
import { topFor } from '../timeline-geometry'

// 日志窄轨:只有当天带时刻的 Journal Entry 出现;无时刻日志留在「全天」区。
// 窄屏时折叠为列表(见 `narrow`),桌面宽度下按时刻定位。
const props = defineProps<{
  items: TimedJournal[]
  range: MinuteRange
  narrow: boolean
}>()

const emit = defineEmits<{
  (e: 'open', entry: JournalEntry): void
}>()

const positioned = computed(() =>
  layoutRanges(
    props.items.map((item) => ({
      item,
      startMin: item.startMin,
      endMin: item.startMin + 20
    }))
  )
)

function head(e: JournalEntry): string {
  if (e.title && e.title.trim()) return e.title.trim()
  const idx = e.body.indexOf('\n')
  return ((idx === -1 ? e.body : e.body.slice(0, idx)).trim() || '(无内容)')
}
</script>

<template>
  <div v-if="narrow" class="jlist">
    <button
      v-for="item in items"
      :key="item.entry.id"
      class="jrow"
      @click="emit('open', item.entry)"
    >
      <span class="jtime">{{ minutesToTime(item.startMin) }}</span>
      <span class="jhead">{{ head(item.entry) }}</span>
    </button>
    <p v-if="items.length === 0" class="empty-hint">无时刻日志在下方「全天」区</p>
  </div>

  <template v-else>
    <article
      v-for="p in positioned"
      :key="p.item.entry.id"
      class="jcard"
      :style="{
        top: `${topFor(p.startMin, range)}px`,
        left: `calc(${(100 / p.lanes) * p.lane}% + 2px)`,
        width: `calc(${100 / p.lanes}% - 4px)`
      }"
      @click="emit('open', p.item.entry)"
    >
      <span class="jtime">{{ minutesToTime(p.startMin) }}</span>
      <span class="jhead">{{ head(p.item.entry) }}</span>
    </article>
    <p v-if="items.length === 0" class="empty-hint">当天没有带时刻的日志</p>
  </template>
</template>

<style scoped>
.jcard {
  position: absolute;
  z-index: 2;
  min-height: 40px;
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-left: 3px solid #b58cff;
  border-radius: 6px;
  padding: 3px 5px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 1px;
  overflow: hidden;
}
.jcard:hover {
  border-color: #b58cff;
}
.jtime {
  font-size: 10px;
  color: var(--muted);
  white-space: nowrap;
}
.jhead {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.jlist {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 4px 2px;
}
.jrow {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 8px;
  align-items: center;
  text-align: left;
  background: var(--panel);
  border: 1px solid var(--border);
  border-left: 3px solid #b58cff;
  border-radius: 6px;
  padding: 5px 8px;
}
.jrow:hover {
  border-color: #b58cff;
}
.empty-hint {
  position: absolute;
  top: 10px;
  left: 8px;
  right: 8px;
  margin: 0;
  color: var(--muted);
  font-size: 12px;
  pointer-events: none;
}
.jlist .empty-hint {
  position: static;
  padding: 6px 2px;
}
</style>
