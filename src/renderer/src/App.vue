<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import ListView from './views/ListView.vue'
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

const view = computed(() => {
  if (route.value === 'quickadd') return QuickAddView
  if (route.value === 'settings') return SettingsView
  return ListView
})
</script>

<template>
  <component :is="view" />
</template>