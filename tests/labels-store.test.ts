import { describe, it, expect, afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import { updateTodoLabels, labelsForTodo } from '../src/main/labels-store'
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