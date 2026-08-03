import { describe, it, expect, afterEach } from 'vitest'
import { mkdtemp, rm, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { makeDb, resetDb } from './helpers/db'
import { openSegment, closeOpen } from '../src/main/segments'
import { buildTimeContent, writeTimeFile } from '../src/main/export-time'

afterEach(resetDb)

async function tempDir(): Promise<string> {
  return await mkdtemp(join(tmpdir(), 'herbie-'))
}

describe('export-time', () => {
  it('buildTimeContent renders segments stored for the day', () => {
    makeDb()
    openSegment({ processName: 'app.exe', title: 'Main' })
    // close it immediately with a fixed timestamp so the day query stays deterministic
    closeOpen('2026-08-03T10:30:00')
    const content = buildTimeContent('2026-08-03')
    expect(content).toContain('# 时间记录 2026-08-03')
    expect(content).toContain('app.exe')
    expect(content).toContain('Main')
  })

  it('buildTimeContent returns empty tables for a day with no segments', () => {
    makeDb()
    const content = buildTimeContent('2026-08-03')
    expect(content).toContain('(无)')
    expect(content).toContain('无片段')
  })

  it('writeTimeFile writes to <dir>/time/<day>.md and returns the path', async () => {
    const dir = await tempDir()
    try {
      const path = await writeTimeFile(dir, '2026-08-03', '# hello\n')
      expect(path).toBe(join(dir, 'time', '2026-08-03.md'))
      const data = await readFile(path, 'utf8')
      expect(data).toBe('# hello\n')
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('writeTimeFile creates nested time dir if missing', async () => {
    const dir = await tempDir()
    try {
      const path = await writeTimeFile(dir, '2026-08-03', 'x')
      expect(path).toContain(join('time', '2026-08-03.md'))
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('buildTimeContent rejects a path-traversal day', () => {
    makeDb()
    expect(() => buildTimeContent('../evil')).toThrow()
    expect(() => buildTimeContent('2026-08-03/../..')).toThrow()
  })

  it('writeTimeFile rejects a path-traversal day without writing', async () => {
    const dir = await tempDir()
    try {
      await expect(writeTimeFile(dir, '../../escape', 'x')).rejects.toThrow()
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
})