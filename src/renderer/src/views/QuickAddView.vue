<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue'
import { parseLabels } from '@shared/labels'

const title = ref('')
const titleInput = ref<HTMLInputElement | null>(null)
const detail = ref('')
const placeholder = ref('')
const clipboardFilled = ref(false)
const shaking = ref(false)
const confirmMsg = ref('')
const dirty = ref(false)

function markDirty(): void {
  dirty.value = true
}

async function flushDraft(hideAfter = false): Promise<void> {
  if (dirty.value) {
    const draft = JSON.stringify({ t: title.value, d: detail.value })
    await window.api.settings.set('draft', draft)
  }
  dirty.value = false
  if (hideAfter) await window.api.window.quickAddHide()
}

async function wake(): Promise<void> {
  const clip = await window.api.clipboard.readText()
  placeholder.value = clip ? clip.slice(0, 200) : ''
  const raw = await window.api.settings.get('draft')
  if (raw) {
    try {
      const obj = JSON.parse(raw)
      title.value = obj.t ?? ''
      detail.value = obj.d ?? ''
    } catch {
      title.value = ''
      detail.value = ''
    }
  } else {
    title.value = ''
    detail.value = ''
  }
  dirty.value = false
  clipboardFilled.value = false
  focusTitle()
}

function focusTitle(): void {
  const attempt = (remaining = 10): void => {
    const el = titleInput.value
    if (!el) return

    const active = document.activeElement
    if (active && active !== document.body && active !== document.documentElement && active !== el) {
      return
    }

    window.focus()
    el.focus({ preventScroll: true })
    if (document.activeElement !== el && remaining > 0) {
      setTimeout(() => attempt(remaining - 1), 16)
    }
  }

  setTimeout(() => attempt(), 0)
}

function onTab(e: KeyboardEvent): void {
  if (e.key !== 'Tab') return
  const target = e.target as HTMLElement
  if (target?.id !== 'qa-title') return
  if (!clipboardFilled.value && title.value.trim() === '' && placeholder.value) {
    title.value = placeholder.value
    clipboardFilled.value = true
    dirty.value = true
    e.preventDefault()
    const el = titleInput.value
    el?.focus()
    const len = title.value.length
    el?.setSelectionRange(len, len)
  }
}

async function onSubmit(): Promise<void> {
  const t = title.value.trim()
  if (!t) {
    shake()
    return
  }
  await window.api.todos.create({ title: t, detail: detail.value })
  title.value = ''
  detail.value = ''
  dirty.value = false
  await window.api.settings.set('draft', '')
  confirmMsg.value = '已添加'
  setTimeout(() => {
    confirmMsg.value = ''
    void window.api.window.quickAddHide()
  }, 600)
}

function shake(): void {
  shaking.value = true
  setTimeout(() => (shaking.value = false), 400)
}

async function onEsc(): Promise<void> {
  await flushDraft(true)
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault()
    void onEsc()
  } else if (e.key === 'Enter' && (e.target as HTMLElement)?.id === 'qa-title') {
    e.preventDefault()
    void onSubmit()
  } else {
    onTab(e)
  }
}

function parsedLabels(): string[] {
  return parseLabels(detail.value)
}

function onShow(): void {
  focusTitle()
  void wake()
}
function onHide(): void {
  void flushDraft(false)
  void window.api.window.quickAddHide()
}
function onBlur(): void {
  void flushDraft(false)
}

let detachShow: () => void = () => {}
let detachHide: () => void = () => {}
let detachBlur: () => void = () => {}

onMounted(() => {
  void wake()
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('blur', onBlur)
  detachShow = window.api.quickadd.onShow(onShow)
  detachHide = window.api.quickadd.onHide(onHide)
  detachBlur = window.api.quickadd.onBlur(onBlur)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('blur', onBlur)
  detachShow()
  detachHide()
  detachBlur()
})
</script>

<template>
  <div class="qa" :class="{ shake: shaking }">
    <input
      ref="titleInput"
      id="qa-title"
      autofocus
      v-model="title"
      spellcheck="false"
      :placeholder="placeholder ? '剪贴板：' + placeholder : '输入标题，Enter 提交'"
      @input="markDirty"
    />
    <textarea
      id="qa-detail"
      v-model="detail"
      spellcheck="false"
      rows="3"
      placeholder="详情（#标签 自动识别）"
      @input="markDirty"
    ></textarea>
    <div class="footer">
      <span v-if="parsedLabels().length" class="labels">#{{ parsedLabels().join(' #') }}</span>
      <span v-if="confirmMsg" class="ok">{{ confirmMsg }}</span>
      <span v-else class="hint">Tab 填剪贴板 · Enter 提交 · Esc 收起 · 失焦自收起</span>
    </div>
  </div>
</template>

<style scoped>
.qa {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  height: 100vh;
  background: var(--bg);
  -webkit-app-region: drag;
}
.qa input,
.qa textarea {
  -webkit-app-region: no-drag;
}
.footer {
  font-size: 11px;
  color: var(--muted);
  display: flex;
  justify-content: space-between;
}
.labels {
  color: var(--accent);
}
.ok {
  color: var(--done);
}
.qa.shake {
  animation: shake 0.4s;
}
@keyframes shake {
  0%,
  100% {
    transform: translateX(0);
  }
  25% {
    transform: translateX(-6px);
  }
  75% {
    transform: translateX(6px);
  }
}
</style>
