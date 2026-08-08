// renderer 薄封装:导出 `createTauriApi(): Api`,形状与 `src/shared/types.ts` 的 `Api` 完全等价,
// 内部走 Tauri `invoke`(请求类命令)+ `listen`(事件类),让 Vue 组件零改动即可在 Tauri 运行。
//
// 命令名/事件名严格遵循跨切片命名契约(见 `.kilo/plans/...md`):
//   todos_list / todos_create / todos_update / todos_toggle / todos_soft_delete / todos_labels
//   settings_get / settings_set / settings_get_all(切片1 已注册)
//   segments_list / segments_update / journal_* / tracker_get_off_work / tracker_set_off_work
//   export_default_dir / export_pull_day / export_write_file(切片5 已注册)
//   shell_open_external / clipboard_read_text / dialog_pick_directory / window_quick_add_hide
//   事件:shortcut://error  quickadd://show | hide | blur
//
// 切片5:export/time/journal 三个导出方法改为全 TS 实现 —— 拉数据(invoke)→ 调 shared
// markdown 纯函数生成 → 写文件(invoke)→ 返回 ExportResult。Rust 不再需要 export_export_markdown
// / time_export / journal_export 占位命令。
// settings_get_all 后端返回 `Vec<(String,String)>`(数组对),此处适配为 `Partial<Settings>` 对象,
// 与 Electron preload 的 `Api.settings.getAll` 形状一致。

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { exportMarkdown as renderTodosMarkdown } from '@shared/markdown'
import { exportTimeMarkdown } from '@shared/time-markdown'
import { exportJournalMarkdown } from '@shared/journal-markdown'
import type {
  Api,
  ExportResult,
  JournalEntry,
  JournalExportResult,
  LabelCount,
  OffWorkState,
  Segment,
  Settings,
  TimeExportResult,
  Todo
} from '@shared/types'

// 与 Rust `export_pull_day` 的 `ExportDayData` 对齐:segments/journal 当日数据 +
// 当日片段关联 todo 的标题映射(免整表拉取)。
interface ExportDayData {
  segments: Segment[]
  journal: JournalEntry[]
  todoTitles: [string, string][]
}

// 请求类命令封装:返回 detach 函数 (() => void);Tauri 的 listen 是异步的,此处把 unlisten 异步解析后惰性调用,
// 形状与 Electron preload 的同步 detach 保持一致(onMounted/onUnmounted 直接成对使用)。
function attach(event: string, payload: (p: unknown) => void): () => void {
  const p = listen<unknown>(event, (e) => payload(e.payload))
  return () => {
    void p.then((u: UnlistenFn) => u())
  }
}

// 对齐 TS `resolveExportDir`:settings 里的 exportDir 优先,未设置回退到默认目录
// (%APPDATA%/herbie,Rust export_default_dir)。
async function resolveExportDir(): Promise<string> {
  return (
    (await invoke<string | null>('settings_get', { key: 'exportDir' })) ||
    (await invoke<string>('export_default_dir'))
  )
}

function toError(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

export function createTauriApi(): Api {
  return {
    todos: {
      list: (filter) => invoke<Todo[]>('todos_list', { filter }),
      create: (input) => invoke<Todo>('todos_create', { input }),
      update: (id, patch) => invoke<Todo>('todos_update', { id, patch }),
      toggle: (id, done) => invoke<Todo>('todos_toggle', { id, done }),
      softDelete: (id) => invoke<void>('todos_soft_delete', { id }),
      labels: () => invoke<LabelCount[]>('todos_labels')
    },
    settings: {
      get: (key) => invoke<string | null>('settings_get', { key }),
      set: (key, value) => invoke<void>('settings_set', { key, value }),
      getAll: async () => {
        const pairs = await invoke<[string, string][]>('settings_get_all')
        const out: Record<string, string> = {}
        for (const [k, v] of pairs) out[k] = v
        return out as Partial<Settings>
      }
    },
    export: {
      exportMarkdown: async (): Promise<ExportResult> => {
        try {
          const dir = await resolveExportDir()
          const todos = await invoke<Todo[]>('todos_list')
          const content = renderTodosMarkdown(todos)
          const path = await invoke<string>('export_write_file', {
            dir,
            filename: 'todos.md',
            content
          })
          return { ok: true, path }
        } catch (e) {
          return { ok: false, error: toError(e) }
        }
      }
    },
    segments: {
      list: (day) => invoke<Segment[]>('segments_list', { day }),
      update: (id, patch) => invoke<Segment | null>('segments_update', { id, patch })
    },
    time: {
      export: async (day: string): Promise<TimeExportResult> => {
        try {
          const dir = await resolveExportDir()
          const data = await invoke<ExportDayData>('export_pull_day', { day })
          const titles: Record<string, string> = {}
          for (const [id, title] of data.todoTitles) titles[id] = title
          const content = exportTimeMarkdown(day, data.segments, titles)
          const path = await invoke<string>('export_write_file', {
            dir,
            filename: `time/${day}.md`,
            content
          })
          return { ok: true, path, day }
        } catch (e) {
          return { ok: false, error: toError(e), day }
        }
      }
    },
    journal: {
      list: (day) => invoke<JournalEntry[]>('journal_list', { day }),
      create: (input) => invoke<JournalEntry>('journal_create', { input }),
      update: (id, patch) => invoke<JournalEntry>('journal_update', { id, patch }),
      softDelete: (id) => invoke<void>('journal_soft_delete', { id }),
      export: async (day: string): Promise<JournalExportResult> => {
        try {
          const dir = await resolveExportDir()
          const data = await invoke<ExportDayData>('export_pull_day', { day })
          const content = exportJournalMarkdown(day, data.journal)
          const path = await invoke<string>('export_write_file', {
            dir,
            filename: `journal/${day}.md`,
            content
          })
          return { ok: true, path, day }
        } catch (e) {
          return { ok: false, error: toError(e), day }
        }
      }
    },
    tracker: {
      getOffWork: () => invoke<OffWorkState>('tracker_get_off_work'),
      setOffWork: (on) => invoke<OffWorkState>('tracker_set_off_work', { on })
    },
    shell: {
      openExternal: (url) => invoke<void>('shell_open_external', { url })
    },
    clipboard: {
      readText: () => invoke<string>('clipboard_read_text')
    },
    dialog: {
      pickDirectory: () => invoke<string | null>('dialog_pick_directory')
    },
    window: {
      quickAddHide: () => invoke<void>('window_quick_add_hide')
    },
    onShortcutError: (cb) => attach('shortcut://error', (p) => cb(String(p))),
    quickadd: {
      onShow: (cb) => attach('quickadd://show', () => cb()),
      onHide: (cb) => attach('quickadd://hide', () => cb()),
      onBlur: (cb) => attach('quickadd://blur', () => cb())
    }
  }
}