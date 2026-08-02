import { ipcMain, shell, clipboard, BrowserWindow, dialog } from 'electron'
import { IPC } from '@shared/ipc'
import {
  listTodos,
  createTodo,
  updateTodo,
  toggleTodo,
  softDeleteTodo,
  listTodoLabels
} from './todos'
import { getSetting, setSetting, getAllSettings } from './settings'
import { exportTodos } from './export'
import type { SettingsKey, TodoFilter, TodoInput, TodoPatch } from '@shared/types'

function isHttpUrl(url: string): boolean {
  return /^https?:\/\//i.test(url)
}

export function registerIpcHandlers(): void {
  ipcMain.handle(IPC.todos.list, (_e, filter?: TodoFilter) => listTodos(filter))
  ipcMain.handle(IPC.todos.create, (_e, input: TodoInput) => createTodo(input))
  ipcMain.handle(IPC.todos.update, (_e, id: string, patch: TodoPatch) => updateTodo(id, patch))
  ipcMain.handle(IPC.todos.toggle, (_e, id: string, done: boolean) => toggleTodo(id, done))
  ipcMain.handle(IPC.todos.softDelete, (_e, id: string) => {
    softDeleteTodo(id)
    return undefined
  })
  ipcMain.handle(IPC.todos.labels, () => listTodoLabels())

  ipcMain.handle(IPC.settings.get, (_e, key: SettingsKey) => getSetting(key))
  ipcMain.handle(IPC.settings.set, (_e, key: SettingsKey, value: string) => {
    setSetting(key, value)
    return undefined
  })
  ipcMain.handle(IPC.settings.getAll, () => getAllSettings())

  ipcMain.handle(IPC.export.exportMarkdown, () => exportTodos())

  ipcMain.handle(IPC.shell.openExternal, async (_e, url: string) => {
    if (isHttpUrl(url)) await shell.openExternal(url)
  })

  ipcMain.handle(IPC.clipboard.readText, () => clipboard.readText())

  ipcMain.handle(IPC.window.quickAddHide, (e) => {
    const win = BrowserWindow.fromId(e.sender.id)
    win?.hide()
    return undefined
  })

  ipcMain.handle(IPC.dialog.pickDirectory, async (e) => {
    const win = BrowserWindow.fromId(e.sender.id) ?? undefined
    const opts = { properties: ['openDirectory' as const] }
    const res = win
      ? await dialog.showOpenDialog(win, opts)
      : await dialog.showOpenDialog(opts)
    if (res.canceled || res.filePaths.length === 0) return null
    return res.filePaths[0]
  })
}