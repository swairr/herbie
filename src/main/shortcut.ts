import { globalShortcut } from 'electron'
import { getSettingWithDefault, setSetting } from './settings'
import { showQuickAdd } from './windows'
import { sendShortcutError } from './notify'

let currentAccelerator: string | null = null

export function registerShortcut(): void {
  unregisterShortcut()
  const accel = getSettingWithDefault('shortcut')
  if (!accel) return
  const ok = globalShortcut.register(accel, () => {
    showQuickAdd()
  })
  if (ok) {
    currentAccelerator = accel
    setSetting('shortcutError', '')
  } else {
    currentAccelerator = null
    const msg = `快捷键 "${accel}" 注册失败，请在设置中更换`
    setSetting('shortcutError', msg)
    sendShortcutError(msg)
  }
}

export function unregisterShortcut(): void {
  if (currentAccelerator) {
    globalShortcut.unregister(currentAccelerator)
    currentAccelerator = null
  }
}

export function reregisterShortcut(): void {
  registerShortcut()
}