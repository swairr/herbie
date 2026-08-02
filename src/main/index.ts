import { app, BrowserWindow, ipcMain } from 'electron'
import { electronApp, optimizer } from '@electron-toolkit/utils'
import { initDb, closeDb } from './db'
import { createMainWindow, getOrCreateQuickAddWindow } from './windows'
import { registerIpcHandlers } from './ipc'
import { registerShortcut, unregisterShortcut, reregisterShortcut } from './shortcut'
import { IPC } from '@shared/ipc'

// Single attach point to re-enable auto-export-on-startup (deliberately disabled —
// see AGENTS.md "Deviation from requirements"). Do NOT add a call to exportTodos()
// here unless the user re-enables it.

app.whenReady().then(() => {
  electronApp.setAppUserModelId('com.herbie.app')

  app.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window)
  })

  // Initialize DB + migrations early (also seeds nothing; defaults handled lazily).
  initDb()

  registerIpcHandlers()

  // React to shortcut change requests from the settings UI.
  ipcMain.on(IPC.settings.set, (_e, key: string, _value: string) => {
    if (key === 'shortcut') reregisterShortcut()
  })

  createMainWindow()
  getOrCreateQuickAddWindow()

  registerShortcut()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow()
  })
})

app.on('window-all-closed', () => {
  unregisterShortcut()
  closeDb()
  if (process.platform !== 'darwin') app.quit()
})

app.on('before-quit', () => {
  unregisterShortcut()
  closeDb()
})