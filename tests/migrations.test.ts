import { describe, it, expect, beforeEach } from 'vitest'
import Database from 'better-sqlite3'
import { runMigrations } from '../src/main/migrations'

let db: Database.Database

beforeEach(() => {
  db = new Database(':memory:')
})

describe('runMigrations', () => {
  it('creates all v1 tables', () => {
    runMigrations(db)
    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
      .all() as { name: string }[]
    const names = tables.map((t) => t.name)
    expect(names).toEqual(expect.arrayContaining(['todos', 'todo_labels', 'settings', 'migrations']))
  })

  it('records version 1 as applied', () => {
    runMigrations(db)
    const row = db.prepare('SELECT version FROM migrations WHERE version = 1').get() as
      | { version: number }
      | undefined
    expect(row?.version).toBe(1)
  })

  it('is idempotent — running twice does not duplicate or error', () => {
    runMigrations(db)
    expect(() => runMigrations(db)).not.toThrow()
    const rows = db.prepare('SELECT version FROM migrations ORDER BY version').all() as {
      version: number
    }[]
    expect(rows).toHaveLength(3)
    expect(rows[0].version).toBe(1)
    expect(rows[1].version).toBe(2)
    expect(rows[2].version).toBe(3)
  })

  it('creates the segments table (v2) and records version 2', () => {
    runMigrations(db)
    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
      .all() as { name: string }[]
    expect(tables.map((t) => t.name)).toContain('segments')
    const v2 = db.prepare('SELECT version FROM migrations WHERE version = 2').get() as
      | { version: number }
      | undefined
    expect(v2?.version).toBe(2)
  })

  it('v2 migration is idempotent on rerun', () => {
    runMigrations(db)
    expect(() => runMigrations(db)).not.toThrow()
  })

  it('segments table accepts a row with open endAt and defaults', () => {
    runMigrations(db)
    db.prepare(
      `INSERT INTO segments (id, startAt, endAt, processName, title, note, todoId, kind)
       VALUES (?,?,?,?,?,?,?,?)`
    ).run('seg1', '2026-08-03T10:00:00Z', null, 'app.exe', 'Title', '', null, 'activity')
    const row = db.prepare('SELECT * FROM segments WHERE id = ?').get('seg1') as {
      endAt: string | null
      kind: string
      processName: string
    }
    expect(row.endAt).toBeNull()
    expect(row.kind).toBe('activity')
    expect(row.processName).toBe('app.exe')
  })

  it('segments.todoId is SET NULL when its todo is hard-deleted', () => {
    runMigrations(db)
    db.prepare(
      `INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    ).run('t1', 't', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', null, null)
    db.prepare(
      `INSERT INTO segments (id, startAt, endAt, processName, title, note, todoId, kind)
       VALUES (?,?,?,?,?,?,?,?)`
    ).run('seg1', '2026-08-03T10:00:00Z', null, 'app.exe', '', '', 't1', 'activity')
    db.prepare('DELETE FROM todos WHERE id = ?').run('t1')
    const row = db.prepare('SELECT todoId FROM segments WHERE id = ?').get('seg1') as {
      todoId: string | null
    }
    expect(row.todoId).toBeNull()
  })

  it('enables inserting a todo row with all required fields', () => {
    runMigrations(db)
    db.prepare(
      `INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    ).run('id1', 't', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', null, null)
    const row = db.prepare('SELECT * FROM todos WHERE id = ?').get('id1') as { title: string }
    expect(row.title).toBe('t')
  })

  it('cascades todo delete to todo_labels', () => {
    runMigrations(db)
    db.prepare(
      `INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    ).run('id1', 't', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', null, null)
    db.prepare('INSERT INTO todo_labels (todoId, label) VALUES (?, ?)').run('id1', 'work')
    db.prepare('DELETE FROM todos WHERE id = ?').run('id1')
    const remain = db.prepare('SELECT COUNT(*) AS n FROM todo_labels').get() as { n: number }
    expect(remain.n).toBe(0)
  })

  it('creates the journal_entries table (v3) and records version 3', () => {
    runMigrations(db)
    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
      .all() as { name: string }[]
    expect(tables.map((t) => t.name)).toContain('journal_entries')
    expect(tables.map((t) => t.name)).toContain('journal_labels')
    const v3 = db.prepare('SELECT version FROM migrations WHERE version = 3').get() as
      | { version: number }
      | undefined
    expect(v3?.version).toBe(3)
  })

  it('journal_entries accepts a row with optional null title', () => {
    runMigrations(db)
    db.prepare(
      `INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    ).run('j1', null, '日记正文', '2026-08-04', '2026-08-04T10:00:00Z', '2026-08-04T10:00:00Z', null)
    const row = db.prepare('SELECT * FROM journal_entries WHERE id = ?').get('j1') as {
      title: string | null
      body: string
      date: string
    }
    expect(row.title).toBeNull()
    expect(row.body).toBe('日记正文')
    expect(row.date).toBe('2026-08-04')
  })

  it('cascades journal delete to journal_labels', () => {
    runMigrations(db)
    db.prepare(
      `INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    ).run('j1', null, 'b', '2026-08-04', '2026-08-04T10:00:00Z', '2026-08-04T10:00:00Z', null)
    db.prepare('INSERT INTO journal_labels (journalId, label) VALUES (?, ?)').run('j1', 'work')
    db.prepare('DELETE FROM journal_entries WHERE id = ?').run('j1')
    const remain = db.prepare('SELECT COUNT(*) AS n FROM journal_labels').get() as { n: number }
    expect(remain.n).toBe(0)
  })
})