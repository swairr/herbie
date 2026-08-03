import { describe, it, expect, afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import { updateTodoLabels, labelsForTodo, updateJournalLabels, labelsForJournal } from '../src/main/labels-store'
import { getDb } from '../src/main/db-access'

afterEach(resetDb)

function insertTodo(id: string): void {
  getDb()
    .prepare(
      `INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    )
    .run(id, 't', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', null, null)
}

function insertJournal(id: string): void {
  getDb()
    .prepare(
      `INSERT INTO journal_entries (id, title, body, date, createdAt, updatedAt, deletedAt)
       VALUES (?,?,?,?,?,?,?)`
    )
    .run(id, null, 'body', '2026-08-04', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', null)
}

describe('labels-store', () => {
  it('inserts labels for a todo', () => {
    makeDb()
    insertTodo('t1')
    updateTodoLabels('t1', ['work', 'home'])
    expect(labelsForTodo('t1')).toEqual(['home', 'work'])
  })

  it('replaces labels on re-parse (delete then insert)', () => {
    makeDb()
    insertTodo('t1')
    updateTodoLabels('t1', ['work', 'home'])
    updateTodoLabels('t1', ['work', 'meta'])
    expect(labelsForTodo('t1')).toEqual(['meta', 'work'])
  })

  it('clears all labels when given empty set', () => {
    makeDb()
    insertTodo('t1')
    updateTodoLabels('t1', ['work'])
    updateTodoLabels('t1', [])
    expect(labelsForTodo('t1')).toEqual([])
  })

  it('ignores duplicate labels within one parse', () => {
    makeDb()
    insertTodo('t1')
    updateTodoLabels('t1', ['work', 'work', 'home'])
    expect(labelsForTodo('t1')).toEqual(['home', 'work'])
  })
})

describe('journal labels-store', () => {
  it('inserts labels for a journal entry', () => {
    makeDb()
    insertJournal('j1')
    updateJournalLabels('j1', ['work', 'meeting'])
    expect(labelsForJournal('j1')).toEqual(['meeting', 'work'])
  })

  it('replaces labels on re-parse (delete then insert)', () => {
    makeDb()
    insertJournal('j1')
    updateJournalLabels('j1', ['work', 'home'])
    updateJournalLabels('j1', ['work', 'meta'])
    expect(labelsForJournal('j1')).toEqual(['meta', 'work'])
  })

  it('shares the label namespace with todos (same label string)', () => {
    makeDb()
    insertTodo('t1')
    insertJournal('j1')
    updateTodoLabels('t1', ['work'])
    updateJournalLabels('j1', ['work'])
    // same literal "work" label is stored in each table — unify in one universe
    expect(labelsForTodo('t1')).toEqual(['work'])
    expect(labelsForJournal('j1')).toEqual(['work'])
  })
})