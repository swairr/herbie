import { app, ipcMain, powerMonitor } from 'electron'
import { electronApp, optimizer } from '@electron-toolkit/utils'
import { initDb, closeDb } from './db'
import { createMainWindow, getOrCreateQuickAddWindow, getMainWindow } from './windows'
import { registerIpcHandlers } from './ipc'
import { registerShortcut, unregisterShortcut, reregisterShortcut } from './shortcut'
import { createTray, refreshTrayMenu } from './tray'
import { startTracking, stopTracking, type HookNotifier, type WinHookEvent } from './segments'
import { createTracker, setTrackerInstance, type TrackerDeps } from './tracker'
import { IPC } from '@shared/ipc'

// Single attach point to re-enable auto-export-on-startup (deliberately disabled —
// see AGENTS.md "Deviation from requirements"). Do NOT add a call to exportTodos()
// here unless the user re-enables it.

function loadWinHook(): HookNotifier {
  try {
    // The native package is externalized (onlyBuiltDependencies + electron.vite external).
    // When not built / non-Windows, require throws and we degrade to a no-op notifier so
    // the rest of the app keeps working without segment recording.
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('herbie-winhook') as {
      start: (cb: (e: WinHookEvent) => void) => void
      stop: () => void
    }
    return { start: (cb) => mod.start(cb), stop: () => mod.stop() }
  } catch {
    return { start() {}, stop() {} }
  }
}

function makeTrackerDeps(): TrackerDeps {
  return {
    getIdle: () => powerMonitor.getSystemIdleTime(),
    onPowerEvent: (name, cb) => powerMonitor.on(name as any, cb as any),
    setInterval: (cb, ms) => setInterval(cb, ms),
    clearInterval: (handle) => clearInterval(handle as any),
    now: () => new Date()
  }
}

let liveTracker: ReturnType<typeof createTracker> | null = null
let liveHook: HookNotifier | null = null

function teardownTracking(): void {
  if (liveTracker) liveTracker.stop()
  if (liveHook) stopTracking(liveHook)
  liveHook = null
  liveTracker = null
  setTrackerInstance(null)
}

app.whenReady().then(() => {
  electronApp.setAppUserModelId('com.herbie.app')

  app.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window)
  })

  initDb()
  registerIpcHandlers()

  const tracker = createTracker(makeTrackerDeps())
  setTrackerInstance(tracker)
  liveTracker = tracker

  const rawHook = loadWinHook()
  liveHook = tracker.gateNotifier(rawHook)

  startTracking(liveHook)
  tracker.start()
  createTray()

  // React to shortcut change requests from the settings UI.
  ipcMain.on(IPC.settings.set, (_e, key: string, _value: string) => {
    if (key === 'shortcut') reregisterShortcut()
    if (key === 'idleThresholdSec') tracker.reconfigure()
    refreshTrayMenu()
  })

  createMainWindow()
  getOrCreateQuickAddWindow()

  registerShortcut()

  app.on('activate', () => {
    if (!getMainWindow()) createMainWindow()
  })
})

// Tray residency (milestone 2): closing the last window hides to the tray instead of
// quitting. Behavior change from milestone 1 — see AGENTS.md. The tray's 退出 is the only
// true exit path.
app.on('window-all-closed', () => {
  const win = getMainWindow()
  if (win && !win.isDestroyed()) win.hide()
})

app.on('before-quit', () => {
  unregisterShortcut()
  teardownTracking()
  closeDb()
})
