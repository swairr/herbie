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
    expect(rows).toHaveLength(1)
    expect(rows[0].version).toBe(1)
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
})