export interface Todo {
  id: string
  title: string
  detail: string
  createdAt: string
  updatedAt: string
  completedAt: string | null
  deletedAt: string | null
}

export interface LabelCount {
  label: string
  count: number
}

export type TodoFilter = {
  labels?: string[]
}

export interface TodoInput {
  title: string
  detail: string
}

export interface TodoPatch {
  title?: string
  detail?: string
}

export interface Settings {
  shortcut: string
  exportDir: string
  draft: string
  shortcutError?: string | null
}

export type SettingsKey = keyof Settings

export interface ExportResult {
  ok: boolean
  path?: string
  error?: string
}

export interface Api {
  todos: {
    list: (filter?: TodoFilter) => Promise<Todo[]>
    create: (input: TodoInput) => Promise<Todo>
    update: (id: string, patch: TodoPatch) => Promise<Todo>
    toggle: (id: string, done: boolean) => Promise<Todo>
    softDelete: (id: string) => Promise<void>
    labels: () => Promise<LabelCount[]>
  }
  settings: {
    get: (key: SettingsKey) => Promise<string | null>
    set: (key: SettingsKey, value: string) => Promise<void>
    getAll: () => Promise<Partial<Settings>>
  }
  export: {
    exportMarkdown: () => Promise<ExportResult>
  }
  shell: {
    openExternal: (url: string) => Promise<void>
  }
  clipboard: {
    readText: () => Promise<string>
  }
  dialog: {
    pickDirectory: () => Promise<string | null>
  }
  window: {
    quickAddHide: () => Promise<void>
  }
  onShortcutError: (cb: (msg: string) => void) => () => void
  quickadd: {
    onShow: (cb: () => void) => () => void
    onHide: (cb: () => void) => () => void
    onBlur: (cb: () => void) => () => void
  }
}