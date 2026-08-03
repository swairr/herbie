import { describe, it, expect, afterEach } from 'vitest'
import { mkdtemp, rm, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { makeDb, resetDb } from './helpers/db'
import { createJournal, softDeleteJournal } from '../src/main/journals'
import { buildJournalContent, writeJournalFile } from '../src/main/export-journal'

afterEach(resetDb)

async function tempDir(): Promise<string> {
  return await mkdtemp(join(tmpdir(), 'herbie-'))
}

describe('export-journal', () => {
  it('buildJournalContent renders entries written for the day', () => {
    makeDb()
    const e = createJournal({ title: '会议', body: '讨论了 #工作\n细节' })
    const content = buildJournalContent(e.date)
    expect(content).toContain(`# 日志 ${e.date}`)
    expect(content).toContain('## 会议')
    expect(content).toContain('讨论了')
    expect(content).toContain(`<!-- id:${e.id} -->`)
  })

  it('buildJournalContent renders an empty header for a day with no entries', () => {
    makeDb()
    const content = buildJournalContent('2026-08-04')
    expect(content.trim()).toBe('# 日志 2026-08-04')
  })

  it('buildJournalContent excludes soft-deleted entries', () => {
    makeDb()
    const e = createJournal({ body: 'still here' })
    const gone = createJournal({ body: 'will be deleted' })
    softDeleteJournal(gone.id)
    const content = buildJournalContent(e.date)
    expect(content).toContain('still here')
    expect(content).not.toContain('will be deleted')
  })

  it('writeJournalFile writes to <dir>/journal/<day>.md and returns the path', async () => {
    const dir = await tempDir()
    try {
      const path = await writeJournalFile(dir, '2026-08-04', '# hello\n')
      expect(path).toBe(join(dir, 'journal', '2026-08-04.md'))
      const data = await readFile(path, 'utf8')
      expect(data).toBe('# hello\n')
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('writeJournalFile creates nested journal dir if missing', async () => {
    const dir = await tempDir()
    try {
      const path = await writeJournalFile(dir, '2026-08-04', 'x')
      expect(path).toContain(join('journal', '2026-08-04.md'))
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('writeJournalFile overwrites an existing file (idempotent re-export)', async () => {
    const dir = await tempDir()
    try {
      await writeJournalFile(dir, '2026-08-04', '# first\n')
      const path = await writeJournalFile(dir, '2026-08-04', '# second\n')
      const data = await readFile(path, 'utf8')
      expect(data).toBe('# second\n')
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('buildJournalContent rejects a path-traversal day', () => {
    makeDb()
    expect(() => buildJournalContent('../evil')).toThrow()
    expect(() => buildJournalContent('2026-08-04/../..')).toThrow()
  })

  it('writeJournalFile rejects a path-traversal day without writing', async () => {
    const dir = await tempDir()
    try {
      await expect(writeJournalFile(dir, '../../escape', 'x')).rejects.toThrow()
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
})