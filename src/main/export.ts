import { app } from 'electron'
import { join } from 'node:path'
import { writeFile, mkdir } from 'node:fs/promises'
import type { Todo, ExportResult } from '@shared/types'
import { exportMarkdown } from '@shared/markdown'
import { listTodos } from './todos'
import { getSetting } from './settings'

const EXPORT_FILENAME = 'todos.md'

// Pure: derive the markdown content from the current todos. Testable without disk.
export function buildExportContent(): string {
  const todos: Todo[] = listTodos()
  return exportMarkdown(todos)
}

// Pure: resolve the target directory, falling back to userData when unset.
export function resolveExportDir(): string {
  const configured = getSetting('exportDir')
  if (configured) return configured
  return app.getPath('userData')
}

// Side-effectful: write content to <dir>/todos.md. Returns the absolute path.
export async function writeExportFile(dir: string, content: string): Promise<string> {
  await mkdir(dir, { recursive: true })
  const file = join(dir, EXPORT_FILENAME)
  await writeFile(file, content, 'utf8')
  return file
}

export async function exportTodos(): Promise<ExportResult> {
  try {
    const content = buildExportContent()
    const dir = resolveExportDir()
    const file = await writeExportFile(dir, content)
    return { ok: true, path: file }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

export function exportDirPreview(): string {
  return resolveExportDir()
}