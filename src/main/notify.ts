import { Notification } from 'electron'
import { IPC } from '@shared/ipc'
import { getMainWindow, getQuickAddWindow } from './windows'

export function sendShortcutError(msg: string): void {
  try {
    new Notification({ title: 'Herbie', body: msg }).show()
  } catch {
    // notification may be unavailable; ignore
  }
  for (const w of [getMainWindow(), getQuickAddWindow()]) {
    if (w && !w.isDestroyed()) {
      w.webContents.send(IPC.shortcut.error, msg)
    }
  }
}