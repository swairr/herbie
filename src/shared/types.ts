export interface Todo {
  id: string
  title: string
  detail: string
  createdAt: string
  updatedAt: string
  completedAt: string | null
  deletedAt: string | null
  sortOrder: number
  plannedDate: string | null
  plannedStart: string | null
  plannedEnd: string | null
}

/** 计划时段(里程碑四):日期 + 可选本地墙钟时间段,三字段均 null 表示未安排。 */
export interface TodoSchedule {
  plannedDate: string | null
  plannedStart: string | null
  plannedEnd: string | null
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
  mainShortcut: string
  exportDir: string
  draft: string
  idleThresholdSec: string
  minSegmentSec: string
  shortcutError?: string | null
  mainShortcutError?: string | null
  autostart: string
  autoUpdateCheck: string
  // 更新时绕过证书校验("true" 绕过,默认校验);与 Rust `settings::KEY_UPDATE_SKIP_CERT_CHECK` 对齐。
  updateSkipCertCheck: string
  lastUpdateError?: string | null
}

export type SettingsKey = keyof Settings

// 更新子系统 settings 键名(与 Rust `settings::KEY_*` 保持一致;对拍锚点:
// `tests/fixtures/update-state.json` 的 `settingsKeys` + cargo `serialized_ipc_shapes_match_shared_fixture`)。
export const UPDATE_SETTINGS_KEYS = [
  'autoUpdateCheck',
  'updateSkipCertCheck',
  'updateNotifiedSha',
  'lastUpdateCheck',
  'lastUpdateError',
] as const

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

// 追踪能力状态(与 Rust `platform::TrackingStatus` 对齐):Linux 下 available=false +
// 原因(ADR 0004),UI 以 Tracking Unavailable 显式呈现(横幅 / 托盘 / 设置页)。
export interface TrackingStatus {
  available: boolean
  reason: string | null
}

// 与 Rust `update::UpdateState` 对齐(tagged enum,serde tag="kind" + camelCase)。
export type UpdateState =
  | { kind: 'idle' }
  | { kind: 'devBuild' }
  | { kind: 'upToDate' }
  | { kind: 'available'; sha: string; builtAt: string }
  | { kind: 'downloading'; progress: number }
  | { kind: 'failed'; reason: string }

// 与 Rust `update::UpdateVersion` 对齐(camelCase)。
export interface UpdateVersion {
  sha: string | null
  devBuild: boolean
  lastCheck: string | null
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
    schedule: (id: string, slot: TodoSchedule) => Promise<Todo>
    scheduled: (from: string, to: string) => Promise<Todo[]>
    unscheduled: () => Promise<Todo[]>
    onChanged: (cb: (todo: Todo) => void) => () => void
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
    getStatus: () => Promise<TrackingStatus>
  }
  shell: {
    openExternal: (url: string) => Promise<void>
  }
  update: {
    check: () => Promise<void>
    getState: () => Promise<UpdateState>
    getVersion: () => Promise<UpdateVersion>
    apply: () => Promise<void>
    onStatus: (cb: (s: UpdateState) => void) => () => void
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
  onMainShortcutError: (cb: (msg: string) => void) => () => void
  quickadd: {
    onShow: (cb: () => void) => () => void
    onHide: (cb: () => void) => () => void
    onBlur: (cb: () => void) => () => void
  }
}