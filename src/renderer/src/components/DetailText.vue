<script setup lang="ts">
import { computed } from 'vue'
import { parseLabels } from '@shared/labels'
import { splitLinks } from '../utils'

const props = defineProps<{ text: string }>()

const segments = computed(() => splitLinks(props.text))
const labels = computed(() => parseLabels(props.text))

async function openLink(url: string): Promise<void> {
  await window.api.shell.openExternal(url)
}
</script>

<template>
  <div class="detail">
    <p class="detail-text">
      <template v-for="(seg, i) in segments" :key="i">
        <a v-if="seg.type === 'url'" href="#" @click.prevent="openLink(seg.value)">{{ seg.value }}</a>
        <span v-else>{{ seg.value }}</span>
      </template>
    </p>
    <div v-if="labels.length" class="chips">
      <span v-for="l in labels" :key="l" class="chip">#{{ l }}</span>
    </div>
  </div>
</template>

<style scoped>
.detail-text {
  white-space: pre-wrap;
  margin: 4px 0;
  line-height: 1.5;
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 4px;
}
.chip {
  font-size: 12px;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 1px 6px;
  border-radius: 8px;
}
</style>