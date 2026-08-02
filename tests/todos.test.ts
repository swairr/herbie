import { describe, it, expect, afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import {
  createTodo,
  listTodos,
  updateTodo,
  toggleTodo,
  softDeleteTodo,
  listTodoLabels
} from '../src/main/todos'

afterEach(resetDb)

function wait(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms))
}

describe('todos repo', () => {
  it('creates a todo with trimmed title', () => {
    makeDb()
    const t = createTodo({ title: '  Buy milk  ', detail: '' })
    expect(t.title).toBe('Buy milk')
    expect(t.completedAt).toBeNull()
    expect(t.deletedAt).toBeNull()
    expect(t.id).toMatch(/[-0-9a-f]{20,}/)
  })

  it('lists todos by createdAt descending among pending', async () => {
    makeDb()
    const a = createTodo({ title: 'a', detail: '' })
    await wait(5)
    const b = createTodo({ title: 'b', detail: '' })
    const list = listTodos()
    expect(list.map((t) => t.id)).toEqual([b.id, a.id])
  })

  it('places done items after pending', async () => {
    makeDb()
    const a = createTodo({ title: 'a', detail: '' })
    await wait(5)
    const b = createTodo({ title: 'b', detail: '' })
    toggleTodo(b.id, true)
    const list = listTodos()
    expect(list[0].id).toBe(a.id)
    expect(list[1].id).toBe(b.id)
  })

  it('parses labels on create and exposes counts via listTodoLabels', () => {
    makeDb()
    createTodo({ title: 't1', detail: 'do #work and #meta' })
    createTodo({ title: 't2', detail: 'also #work' })
    const counts = listTodoLabels()
    const work = counts.find((c) => c.label === 'work')
    const meta = counts.find((c) => c.label === 'meta')
    expect(work?.count).toBe(2)
    expect(meta?.count).toBe(1)
  })

  it('re-parses labels on update', () => {
    makeDb()
    const t = createTodo({ title: 't', detail: '#work' })
    updateTodo(t.id, { detail: '#home #work' })
    const counts = listTodoLabels()
    expect(counts.map((c) => c.label).sort()).toEqual(['home', 'work'])
  })

  it('removes a label when detail no longer has it', () => {
    makeDb()
    const t = createTodo({ title: 't', detail: '#work #home' })
    updateTodo(t.id, { detail: 'no tags here' })
    expect(listTodoLabels()).toEqual([])
  })

  it('skips # inside urls when parsing labels', () => {
    makeDb()
    createTodo({ title: 't', detail: 'see https://x.io/p#sec and #real' })
    const counts = listTodoLabels()
    expect(counts.map((c) => c.label)).toEqual(['real'])
  })

  it('toggle true sets completedAt, false clears it', () => {
    makeDb()
    const t = createTodo({ title: 't', detail: '' })
    const done = toggleTodo(t.id, true)
    expect(done.completedAt).toBeTruthy()
    const undone = toggleTodo(t.id, false)
    expect(undone.completedAt).toBeNull()
  })

  it('softDelete removes from list and excludes from label counts', () => {
    makeDb()
    const t = createTodo({ title: 't', detail: '#work' })
    softDeleteTodo(t.id)
    expect(listTodos()).toEqual([])
    expect(listTodoLabels()).toEqual([])
  })

  it('filters by labels using OR (union)', () => {
    makeDb()
    const t1 = createTodo({ title: 't1', detail: '#work' })
    createTodo({ title: 't2', detail: '#home' })
    const t3 = createTodo({ title: 't3', detail: '#work #home' })
    const filtered = listTodos({ labels: ['work'] })
    expect(filtered.map((t) => t.id).sort()).toEqual([t1.id, t3.id].sort())
  })

  it('filter with multiple labels is union, not intersection', () => {
    makeDb()
    const t1 = createTodo({ title: 't1', detail: '#work' })
    const t2 = createTodo({ title: 't2', detail: '#home' })
    createTodo({ title: 't3', detail: '#meta' })
    const filtered = listTodos({ labels: ['work', 'home'] })
    expect(filtered.map((t) => t.id).sort()).toEqual([t1.id, t2.id].sort())
  })

  it('update throws on unknown id', () => {
    makeDb()
    expect(() => updateTodo('does-not-exist', { detail: 'x' })).toThrow()
  })

  it('omits completedAt column ordering: done sorted by completedAt desc', async () => {
    makeDb()
    const a = createTodo({ title: 'a', detail: '' })
    await wait(5)
    const b = createTodo({ title: 'b', detail: '' })
    toggleTodo(a.id, true)
    await wait(5)
    toggleTodo(b.id, true)
    const done = listTodos().filter((t) => t.completedAt)
    expect(done.map((t) => t.id)).toEqual([b.id, a.id])
  })
})