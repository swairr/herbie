import { join } from 'node:path'
import { writeFile, mkdir } from 'node:fs/promises'
import type { TimeExportResult } from '@shared/types'
import { exportTimeMarkdown } from '@shared/time-markdown'
import { listSegmentsByDay, fetchTodoTitles } from './segments-query'
import { resolveExportDir } from './export'

// Strict local "YYYY-MM-DD" — the only day strings the renderer may send. Also guards the
// filesystem path against traversal via renderer-supplied `day`.
const DAY_RE = /^\d{4}-\d{2}-\d{2}$/

function assertDay(day: string): void {
  if (typeof day !== 'string' || !DAY_RE.test(day)) {
    throw new Error(`invalid day: ${day}`)
  }
}

// Pure: build the markdown content for a local natural day from the DB.
export function buildTimeContent(day: string): string {
  assertDay(day)
  const segments = listSegmentsByDay(day)
  const ids = segments.map((s) => s.todoId).filter((x): x is string => !!x)
  const titles = fetchTodoTitles(ids)
  return exportTimeMarkdown(day, segments, titles)
}

// Side-effectful: write the content to <dir>/time/<day>.md (recursive mkdir). Returns path.
export async function writeTimeFile(dir: string, day: string, content: string): Promise<string> {
  assertDay(day)
  const sub = join(dir, 'time')
  await mkdir(sub, { recursive: true })
  const file = join(sub, `${day}.md`)
  await writeFile(file, content, 'utf8')
  return file
}

export async function exportTime(day: string): Promise<TimeExportResult> {
  try {
    const content = buildTimeContent(day)
    const dir = resolveExportDir()
    const file = await writeTimeFile(dir, day, content)
    return { ok: true, path: file, day }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e), day }
  }
}