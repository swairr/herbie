<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { TrackingStatus, UpdateState, UpdateVersion } from '@shared/types'
import { UPDATE_SETTINGS_KEYS } from '@shared/types'
import { captureAccelerator } from '../utils'

const shortcut = ref('')
const mainShortcut = ref('')
const exportDir = ref('')
const idleThresholdSec = ref('')
const minSegmentSec = ref('')
const shortcutError = ref<string | null>(null)
const mainShortcutError = ref<string | null>(null)
const autostart = ref(false)
const autostartError = ref<string | null>(null)
const saved = ref(false)
const capturing = ref<'shortcut' | 'mainShortcut' | null>(null)

const autoUpdateCheck = ref(true)
const updateSkipCertCheck = ref(false)
const autoUpdateError = ref<string | null>(null)
const updateSkipCertError = ref<string | null>(null)
const updateState = ref<UpdateState>({ kind: 'idle' })
const updateVersion = ref<UpdateVersion>({ sha: null, devBuild: true, lastCheck: null })
// 追踪能力状态(Tracking Unavailable 的设置页状态行,ADR 0004)。
const tracking = ref<TrackingStatus | null>(null)
// 检查/更新触发层的同步错误(如「正在下载更新」守卫)与上一次失败的持久化记录。
const applyError = ref<string | null>(null)
const lastUpdateError = ref<string | null>(null)

let detachStatus: (() => void) | undefined

async function load(): Promise<void> {
  const all = await window.api.settings.getAll()
  shortcut.value = (all.shortcut as string) || 'Ctrl+Shift+Space'
  mainShortcut.value = (all.mainShortcut as string) || 'Ctrl+Shift+M'
  exportDir.value = (all.exportDir as string) || ''
  idleThresholdSec.value = (all.idleThresholdSec as string) || '300'
  minSegmentSec.value = (all.minSegmentSec as string) || '60'
  shortcutError.value = (all.shortcutError as string) || null
  mainShortcutError.value = (all.mainShortcutError as string) || null
  autostart.value = all.autostart === 'true'
  autoUpdateCheck.value = (all.autoUpdateCheck as string) !== 'false'
  updateSkipCertCheck.value = (all.updateSkipCertCheck as string) === 'true'
  lastUpdateError.value = (all.lastUpdateError as string) || null
}

async function onToggleAutostart(): Promise<void> {
  try {
    await window.api.settings.set('autostart', autostart.value ? 'true' : 'false')
    autostartError.value = null
  } catch (e) {
    autostart.value = !autostart.value
    autostartError.value = String(e)
  }
}

async function onToggleAutoUpdate(): Promise<void> {
  try {
    await window.api.settings.set(UPDATE_SETTINGS_KEYS[0], autoUpdateCheck.value ? 'true' : 'false')
    autoUpdateError.value = null
  } catch (e) {
    autoUpdateCheck.value = !autoUpdateCheck.value
    autoUpdateError.value = String(e)
  }
}

async function onToggleUpdateSkipCert(): Promise<void> {
  try {
    await window.api.settings.set('updateSkipCertCheck', updateSkipCertCheck.value ? 'true' : 'false')
    updateSkipCertError.value = null
  } catch (e) {
    updateSkipCertCheck.value = !updateSkipCertCheck.value
    updateSkipCertError.value = String(e)
  }
}

async function refreshUpdate(): Promise<void> {
  updateVersion.value = await window.api.update.getVersion()
  updateState.value = await window.api.update.getState()
}

async function onCheckUpdate(): Promise<void> {
  try {
    await window.api.update.check()
    applyError.value = null
    await refreshUpdate()
  } catch (e) {
    applyError.value = String(e)
  }
}

function onApplyUpdate(): void {
  const ok = window.confirm('将下载并替换为最新 nightly 版本,随后自动重启应用。当前活动记录会与托盘退出一致地正常收尾。')
  if (!ok) return
  // 同步守卫类错误(如重复点击「正在下载更新,请稍候」)必须可见,不能被 void 吞掉。
  window.api.update
    .apply()
    .then(() => (applyError.value = null))
    .catch((e: unknown) => (applyError.value = String(e)))
}

async function saveAll(): Promise<void> {
  await window.api.settings.set('shortcut', shortcut.value.trim())
  await window.api.settings.set('mainShortcut', mainShortcut.value.trim())
  await window.api.settings.set('exportDir', exportDir.value.trim())
  await window.api.settings.set('idleThresholdSec', idleThresholdSec.value.trim())
  await window.api.settings.set('minSegmentSec', minSegmentSec.value.trim())
  saved.value = true
  setTimeout(() => (saved.value = false), 1500)
  await load()
}

async function pickDir(): Promise<void> {
  const p = await window.api.dialog?.pickDirectory?.()
  if (p) exportDir.value = p
}

function startCapture(target: 'shortcut' | 'mainShortcut'): void {
  capturing.value = target
}

function endCapture(): void {
  capturing.value = null
}

function onCaptureKeydown(e: KeyboardEvent): void {
  if (!capturing.value) return
  e.preventDefault()
  e.stopPropagation()
  const res = captureAccelerator(e)
  if (res.status === 'cancel') {
    endCapture()
    return
  }
  if (res.status === 'captured') {
    if (capturing.value === 'shortcut') shortcut.value = res.accelerator
    else mainShortcut.value = res.accelerator
    endCapture()
  }
}

onMounted(() => {
  load()
  void window.api.tracker.getStatus().then((s) => (tracking.value = s))
  detachStatus = window.api.update.onStatus((s) => {
    updateState.value = s
    if (s.kind === 'failed') {
      // 失败原因带时间戳持久化在 settings,事件到达时重读一次拿到最新记录。
      void window.api.settings
        .getAll()
        .then((all) => (lastUpdateError.value = (all.lastUpdateError as string) || null))
    } else {
      applyError.value = null
      if (s.kind !== 'idle') lastUpdateError.value = null
    }
  })
  void refreshUpdate()
  window.addEventListener('keydown', onCaptureKeydown, true)
})

onBeforeUnmount(() => {
  detachStatus?.()
  window.removeEventListener('keydown', onCaptureKeydown, true)
})
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h1>设置</h1>
    </header>

    <section class="form">
      <h2 class="section-title">通用</h2>
      <label class="field">
        <span>快捷键</span>
        <button
          type="button"
          class="capture"
          :class="{ capturing: capturing === 'shortcut' }"
          :disabled="capturing !== null && capturing !== 'shortcut'"
          @click="startCapture('shortcut')"
        >
          {{ capturing === 'shortcut' ? '请按下组合键…（Esc 取消）' : shortcut || '未设置' }}
        </button>
        <em class="hint">点击后按下组合键自动录入，唤起快速添加小窗口</em>
      </label>
      <p v-if="shortcutError" class="error">{{ shortcutError }}</p>

      <label class="field">
        <span>唤醒主窗口快捷键</span>
        <button
          type="button"
          class="capture"
          :class="{ capturing: capturing === 'mainShortcut' }"
          :disabled="capturing !== null && capturing !== 'mainShortcut'"
          @click="startCapture('mainShortcut')"
        >
          {{ capturing === 'mainShortcut' ? '请按下组合键…（Esc 取消）' : mainShortcut || '未设置' }}
        </button>
        <em class="hint">主窗口隐藏到托盘后，用该快捷键重新唤出</em>
      </label>
      <p v-if="mainShortcutError" class="error">{{ mainShortcutError }}</p>

      <label class="field wide">
        <span>导出目录</span>
        <div class="dir-row">
          <input v-model="exportDir" spellcheck="false" placeholder="留空则在应用数据目录导出" />
          <button @click="pickDir">选择…</button>
        </div>
      </label>

      <label class="field">
        <span>空闲阈值（秒）</span>
        <input v-model="idleThresholdSec" type="number" min="1" placeholder="300" />
      </label>

      <label class="field">
        <span>防抖时间（秒）</span>
        <input v-model="minSegmentSec" type="number" min="1" placeholder="60" />
        <em class="hint">短于该时长的片段会并入下一个窗口，避免时间段过于零散</em>
      </label>

      <label class="field">
        <div class="check-row">
          <input type="checkbox" v-model="autostart" @change="onToggleAutostart" />
          <span>开机自动启动</span>
        </div>
        <em class="hint">开启后开机时自动启动并最小化到托盘</em>
      </label>
      <p v-if="autostartError" class="error">{{ autostartError }}</p>

      <h2 class="section-title">活动记录</h2>
      <div class="field wide">
        <span>状态</span>
        <span>{{ tracking?.available ? '正常记录' : '不可用' }}</span>
        <em v-if="tracking && !tracking.available" class="hint">{{ tracking.reason }}</em>
      </div>

      <h2 class="section-title">更新</h2>
      <label class="field">
        <div class="check-row">
          <input type="checkbox" v-model="autoUpdateCheck" @change="onToggleAutoUpdate" />
          <span>自动检查更新</span>
        </div>
        <em class="hint">启动后每 24 小时自动检查 nightly 更新，新版本仅经托盘提示</em>
      </label>
      <p v-if="autoUpdateError" class="error">{{ autoUpdateError }}</p>

      <label class="field">
        <div class="check-row">
          <input type="checkbox" v-model="updateSkipCertCheck" @change="onToggleUpdateSkipCert" />
          <span>绕过证书校验</span>
        </div>
        <em class="hint">默认校验更新服务器证书；仅在自签名证书 / 内网代理场景下开启，存在安全风险</em>
      </label>
      <p v-if="updateSkipCertError" class="error">{{ updateSkipCertError }}</p>

      <div class="field wide">
        <span>当前版本</span>
        <div class="update-line">
          <span>{{ updateVersion.sha ? `nightly-${updateVersion.sha.slice(0, 7)}` : '开发模式' }}</span>
          <button class="ghost" :disabled="updateVersion.devBuild" @click="onCheckUpdate">检查更新</button>
        </div>
        <em class="hint">开发模式（本地构建）不参与更新</em>
      </div>
      <p v-if="updateState.kind === 'failed'" class="error">{{ updateState.reason }}</p>
      <em v-if="updateState.kind === 'failed'" class="hint">
        详细排查日志：应用数据目录下的 herbie/update.log
      </em>
      <p v-if="applyError" class="error">{{ applyError }}</p>
      <p v-else-if="lastUpdateError && updateState.kind !== 'failed'" class="error">
        上次更新问题：{{ lastUpdateError }}
      </p>
      <div v-else-if="updateState.kind === 'available'" class="field wide">
        <div class="update-line">
          <span>新版本构建于 {{ updateState.builtAt }}</span>
          <button class="primary" @click="onApplyUpdate">立即更新</button>
        </div>
      </div>
      <div v-else-if="updateState.kind === 'downloading'" class="field wide">
        <div class="progress">
          <div class="bar" :style="{ width: updateState.progress + '%' }"></div>
        </div>
        <em class="hint">正在下载 {{ updateState.progress }}%…</em>
      </div>
      <div v-else-if="updateState.kind === 'upToDate'" class="ok">已是最新版本</div>
      <em v-if="updateVersion.lastCheck" class="hint last-check">
        上次检查：{{ new Date(updateVersion.lastCheck).toLocaleString() }}
      </em>

      <div class="actions">
        <button class="primary" @click="saveAll">保存</button>
        <span v-if="saved" class="saved">已保存</span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.form {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px 28px;
  margin-top: 4px;
}
.form > .section-title,
.form > .error,
.form > .actions,
.form > .ok,
.form > .progress {
  grid-column: 1 / -1;
}
.form > .field.wide {
  grid-column: 1 / -1;
}
@media (max-width: 900px) {
  .form {
    grid-template-columns: minmax(0, 1fr);
  }
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.field > span {
  font-size: 12px;
  color: var(--muted);
}
.field .hint {
  font-size: 11px;
  font-style: normal;
  color: var(--muted);
}
.capture {
  font: inherit;
  text-align: left;
  color: var(--text);
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px 8px;
  width: 100%;
  cursor: pointer;
}
.capture:focus-visible {
  outline: none;
  border-color: var(--accent);
}
.capture.capturing {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--accent);
}
.capture:disabled {
  opacity: 0.5;
  cursor: default;
}
.capture:disabled:hover {
  border-color: var(--border);
}
.dir-row {
  display: flex;
  gap: 8px;
}
.dir-row input {
  flex: 1;
}
.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.check-row input {
  width: auto;
}
.check-row span {
  font-size: 12px;
  color: var(--muted);
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
.section-title {
  font-size: 13px;
  font-weight: 600;
  margin: 10px 0 -2px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border);
  color: var(--text);
}
.update-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.update-line button:disabled {
  opacity: 0.5;
  cursor: default;
}
.update-line button:disabled:hover {
  border-color: transparent;
}
.progress {
  height: 8px;
  border-radius: 4px;
  background: var(--panel-2);
  border: 1px solid var(--border);
  overflow: hidden;
}
.bar {
  height: 100%;
  background: var(--accent);
  transition: width 0.3s ease;
}
.ok {
  color: var(--done);
  font-size: 12px;
}
.form > .last-check {
  grid-column: 1 / -1;
}
</style>
