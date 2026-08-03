import { contextBridge, ipcRenderer, clipboard } from 'electron'
import { IPC } from '@shared/ipc'
import type { Api } from '@shared/types'

const api: Api = {
  todos: {
    list: (filter) => ipcRenderer.invoke(IPC.todos.list, filter),
    create: (input) => ipcRenderer.invoke(IPC.todos.create, input),
    update: (id, patch) => ipcRenderer.invoke(IPC.todos.update, id, patch),
    toggle: (id, done) => ipcRenderer.invoke(IPC.todos.toggle, id, done),
    softDelete: (id) => ipcRenderer.invoke(IPC.todos.softDelete, id),
    labels: () => ipcRenderer.invoke(IPC.todos.labels)
  },
  settings: {
    get: (key) => ipcRenderer.invoke(IPC.settings.get, key),
    set: (key, value) => ipcRenderer.invoke(IPC.settings.set, key, value),
    getAll: () => ipcRenderer.invoke(IPC.settings.getAll)
  },
  export: {
    exportMarkdown: () => ipcRenderer.invoke(IPC.export.exportMarkdown)
  },
  segments: {
    list: (day) => ipcRenderer.invoke(IPC.segments.list, day),
    update: (id, patch) => ipcRenderer.invoke(IPC.segments.update, id, patch)
  },
  time: {
    export: (day) => ipcRenderer.invoke(IPC.time.export, day)
  },
  tracker: {
    getOffWork: () => ipcRenderer.invoke(IPC.tracker.getOffWork),
    setOffWork: (on) => ipcRenderer.invoke(IPC.tracker.setOffWork, on)
  },
  shell: {
    openExternal: (url) => ipcRenderer.invoke(IPC.shell.openExternal, url)
  },
  clipboard: {
    readText: () => Promise.resolve(clipboard.readText())
  },
  dialog: {
    pickDirectory: () => ipcRenderer.invoke(IPC.dialog.pickDirectory)
  },
  window: {
    quickAddHide: () => ipcRenderer.invoke(IPC.window.quickAddHide)
  },
  onShortcutError: (cb) => {
    const handler = (_e: unknown, msg: string) => cb(msg)
    ipcRenderer.on(IPC.shortcut.error, handler as any)
    return () => ipcRenderer.removeListener(IPC.shortcut.error, handler as any)
  },
  quickadd: {
    onShow: (cb) => {
      const h = () => cb()
      ipcRenderer.on(IPC.quickadd.showEvent, h as any)
      return () => ipcRenderer.removeListener(IPC.quickadd.showEvent, h as any)
    },
    onHide: (cb) => {
      const h = () => cb()
      ipcRenderer.on(IPC.quickadd.hideEvent, h as any)
      return () => ipcRenderer.removeListener(IPC.quickadd.hideEvent, h as any)
    },
    onBlur: (cb) => {
      const h = () => cb()
      ipcRenderer.on(IPC.quickadd.blurEvent, h as any)
      return () => ipcRenderer.removeListener(IPC.quickadd.blurEvent, h as any)
    }
  }
}

if (process.contextIsolated) {
  try {
    contextBridge.exposeInMainWorld('api', api)
  } catch (error) {
    console.error(error)
  }
} else {
  ;(globalThis as unknown as { api: typeof api }).api = api
}