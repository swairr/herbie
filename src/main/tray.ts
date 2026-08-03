import { app, Tray, Menu, nativeImage } from 'electron'
import { getMainWindow } from './windows'
import { getTracker } from './tracker'

let tray: Tray | null = null

const TRAY_ICON_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAOklEQVR42mNk+M9QDwACRgHFkQFkCRgHWw4A' +
  'YmBgYGBgYmDyAWQZGAcYgICAgICAgABk/BJkZAD2AQwA2NoB/QMG5QAAAABJRU5ErkJggg=='

function icon() {
  return nativeImage.createFromBuffer(Buffer.from(TRAY_ICON_BASE64, 'base64'))
}

function offWorkLabel(): string {
  return getTracker()?.getOffWork() ? '恢复记录' : '下班 / 停止记录'
}

function refreshMenu(): void {
  if (!tray) return
  tray.setContextMenu(
    Menu.buildFromTemplate([
      {
        label: offWorkLabel(),
        click: () => {
          const t = getTracker()
          if (!t) return
          t.setOffWork(!t.getOffWork())
          refreshMenu()
        }
      },
      { type: 'separator' },
      {
        label: '显示主窗口',
        click: () => {
          const w = getMainWindow()
          if (w) {
            w.show()
            w.focus()
          }
        }
      },
      {
        label: '退出',
        click: () => {
          app.quit()
        }
      }
    ])
  )
}

export function createTray(): Tray {
  if (tray && !tray.isDestroyed()) return tray
  tray = new Tray(icon())
  tray.setToolTip('Herbie')
  refreshMenu()
  tray.on('click', () => {
    const w = getMainWindow()
    if (w) {
      w.show()
      w.focus()
    }
    refreshMenu()
  })
  return tray
}

export function refreshTrayMenu(): void {
  refreshMenu()
}