<script setup lang="ts">
import { computed } from 'vue'
import { minutesToTime, type MinuteRange } from '@shared/schedule'
import { PX_PER_MIN, gridHeight, topFor } from '../timeline-geometry'

// 三轨共享的时间轴:统一小时刻度、now 线、计划 / 实际 / 日志的并排或窄屏单轨布局。
// 各轨内容由具名插槽提供,块定位仍用相同的分钟刻度(见 timeline-geometry)。
const props = defineProps<{
  range: MinuteRange
  nowLineTop: number | null
  showPlan: boolean
  showActual: boolean
  showJournal: boolean
}>()

const emit = defineEmits<{
  (e: 'drop-minutes', minutes: number): void
}>()

const height = computed(() => gridHeight(props.range))
const hours = computed(() => {
  const out: number[] = []
  for (let m = props.range.startMin; m <= props.range.endMin; m += 60) out.push(m)
  return out
})

const gridClass = computed(() => {
  const count = [props.showPlan, props.showActual, props.showJournal].filter(Boolean).length
  return `cols-${count}`
})

const dropEnabled = computed(() => props.showPlan)

function onDrop(e: DragEvent): void {
  e.preventDefault()
  if (!dropEnabled.value) return
  const root = e.currentTarget as HTMLElement
  const area = root.querySelector('.area') as HTMLElement | null
  if (!area) return
  const rect = area.getBoundingClientRect()
  emit('drop-minutes', props.range.startMin + (e.clientY - rect.top) / PX_PER_MIN)
}
</script>

<template>
  <div class="day-timeline" @dragover.prevent @drop="onDrop">
    <div class="heads" :class="gridClass">
      <div class="gutter"></div>
      <div v-if="showPlan" class="head">计划</div>
      <div v-if="showActual" class="head">实际</div>
      <div v-if="showJournal" class="head">日志</div>
    </div>

    <div class="body">
      <div class="gutter hours" :style="{ height: height + 'px' }">
        <span
          v-for="m in hours"
          :key="m"
          class="hour-label"
          :style="{ top: topFor(m, range) + 'px' }"
        >
          {{ minutesToTime(m) }}
        </span>
      </div>

      <div class="area" :style="{ height: height + 'px' }">
        <div
          v-for="m in hours"
          :key="m"
          class="hour-line"
          :style="{ top: topFor(m, range) + 'px' }"
        ></div>
        <div
          v-if="nowLineTop !== null"
          class="now-line"
          :style="{ top: nowLineTop + 'px' }"
        ></div>

        <div class="tracks" :class="gridClass">
          <div v-if="showPlan" class="track track-plan">
            <slot name="plan"></slot>
          </div>
          <div v-if="showActual" class="track track-actual">
            <slot name="actual"></slot>
          </div>
          <div v-if="showJournal" class="track track-journal">
            <slot name="journal"></slot>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.day-timeline {
  min-width: 0;
}
.heads {
  position: sticky;
  top: 0;
  z-index: 6;
  display: grid;
  align-items: end;
  margin-bottom: 4px;
  background: var(--bg);
}
.heads.cols-1 {
  grid-template-columns: 42px minmax(0, 1fr);
}
.heads.cols-2 {
  grid-template-columns: 42px minmax(0, 1fr) minmax(0, 1fr);
}
.heads.cols-3 {
  grid-template-columns: 42px minmax(0, 1fr) minmax(0, 1fr) 120px;
}
.head {
  font-size: 11px;
  color: var(--muted);
  padding-bottom: 2px;
  border-bottom: 1px solid var(--border);
  text-align: center;
}
.body {
  display: flex;
  gap: 0;
}
.gutter {
  width: 42px;
  flex-shrink: 0;
}
.hours {
  position: relative;
}
.hour-label {
  position: absolute;
  right: 0;
  transform: translateY(-50%);
  font-size: 10px;
  color: var(--muted);
  white-space: nowrap;
}
.area {
  position: relative;
  flex: 1;
  min-width: 0;
  border-left: 1px solid var(--border);
}
.hour-line {
  position: absolute;
  left: 0;
  right: 0;
  border-top: 1px solid var(--border);
  opacity: 0.5;
  pointer-events: none;
}
.now-line {
  position: absolute;
  left: 0;
  right: 0;
  border-top: 2px solid var(--danger);
  z-index: 3;
  pointer-events: none;
}
.tracks {
  position: absolute;
  inset: 0;
  display: grid;
}
.tracks.cols-1 {
  grid-template-columns: minmax(0, 1fr);
}
.tracks.cols-2 {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
}
.tracks.cols-3 {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 120px;
}
.track {
  position: relative;
  min-width: 0;
}
.track-plan,
.track-actual {
  border-right: 1px dashed var(--border);
}
.track-journal {
  border-right: none;
}
</style>
