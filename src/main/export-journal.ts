import { join } from 'node:path'
import { writeFile, mkdir } from 'node:fs/promises'
import type { JournalExportResult } from '@shared/types'
import { exportJournalMarkdown } from '@shared/journal-markdown'
import { listJournals } from './journals'
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
export function buildJournalContent(day: string): string {
  assertDay(day)
  const entries = listJournals(day)
  return exportJournalMarkdown(day, entries)
}

// Side-effectful: write the content to <dir>/journal/<day>.md (recursive mkdir). Returns path.
export async function writeJournalFile(
  dir: string,
  day: string,
  content: string
): Promise<string> {
  assertDay(day)
  const sub = join(dir, 'journal')
  await mkdir(sub, { recursive: true })
  const file = join(sub, `${day}.md`)
  await writeFile(file, content, 'utf8')
  return file
}

export async function exportJournal(day: string): Promise<JournalExportResult> {
  try {
    const content = buildJournalContent(day)
    const dir = resolveExportDir()
    const file = await writeJournalFile(dir, day, content)
    return { ok: true, path: file, day }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e), day }
  }
}