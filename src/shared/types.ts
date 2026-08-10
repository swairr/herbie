export interface Todo {
  id: string
  title: string
  detail: string
  createdAt: string
  updatedAt: string
  completedAt: string | null
  deletedAt: string | null
  sortOrder: number
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
  idleThresholdSec: string
  shortcutError?: string | null
}

export type SettingsKey = keyof Settings

export interface ExportResult {
  ok: boolean
  path?: string
  error?: string
}

export type SegmentKind = 'activity' | 'idle'

export interface Segment {
  id: string
  startAt: string
  endAt: string | null
  processName: string
  title: string
  note: string
  todoId: string | null
  kind: SegmentKind
}

export interface SegmentPatch {
  note?: string
  todoId?: string | null
}

export interface OffWorkState {
  offWork: boolean
}

export interface TimeExportResult extends ExportResult {
  day?: string
}

export interface JournalEntry {
  id: string
  title: string | null
  body: string
  date: string
  createdAt: string
  updatedAt: string
  deletedAt: string | null
}

export interface JournalInput {
  title?: string | null
  body: string
  date?: string
}

export interface JournalPatch {
  title?: string | null
  body?: string
  date?: string
}

export interface JournalExportResult extends ExportResult {
  day?: string
}

export interface Api {
  todos: {
    list: (filter?: TodoFilter) => Promise<Todo[]>
    create: (input: TodoInput) => Promise<Todo>
    update: (id: string, patch: TodoPatch) => Promise<Todo>
    toggle: (id: string, done: boolean) => Promise<Todo>
    softDelete: (id: string) => Promise<void>
    labels: () => Promise<LabelCount[]>
    move: (id: string, beforeId: string | null) => Promise<Todo>
  }
  settings: {
    get: (key: SettingsKey) => Promise<string | null>
    set: (key: SettingsKey, value: string) => Promise<void>
    getAll: () => Promise<Partial<Settings>>
  }
  export: {
    exportMarkdown: () => Promise<ExportResult>
  }
  segments: {
    list: (day: string) => Promise<Segment[]>
    update: (id: string, patch: SegmentPatch) => Promise<Segment | null>
  }
  time: {
    export: (day: string) => Promise<TimeExportResult>
  }
  journal: {
    list: (day: string) => Promise<JournalEntry[]>
    create: (input: JournalInput) => Promise<JournalEntry>
    update: (id: string, patch: JournalPatch) => Promise<JournalEntry>
    softDelete: (id: string) => Promise<void>
    export: (day: string) => Promise<JournalExportResult>
  }
  tracker: {
    getOffWork: () => Promise<OffWorkState>
    setOffWork: (on: boolean) => Promise<OffWorkState>
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