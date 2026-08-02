import { describe, it, expect, afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import { createTodo, toggleTodo } from '../src/main/todos'
import { buildExportContent } from '../src/main/export'

afterEach(resetDb)

describe('export content integration', () => {
  it('renders pending and done todos from the database', () => {
    makeDb()
    createTodo({ title: 'First', detail: '#work' })
    createTodo({ title: 'Second', detail: 'see https://x.io/p#sec' })
    const content = buildExportContent()
    expect(content).toContain('First')
    expect(content).toContain('Second')
    expect(content).toMatch(/- \[ \] First/)
  })

  it('marks done todos with [x] and emits Completed line', () => {
    makeDb()
    const t = createTodo({ title: 'Done item', detail: '' })
    toggleTodo(t.id, true)
    const content = buildExportContent()
    expect(content).toMatch(/- \[x\] Done item <!-- id:/)
    expect(content).toContain('Completed:')
  })

  it('excludes soft-deleted todos', async () => {
    makeDb()
    const { softDeleteTodo } = await import('../src/main/todos')
    createTodo({ title: 'Keep', detail: '' })
    const drop = createTodo({ title: 'Drop', detail: '' })
    softDeleteTodo(drop.id)
    const content = buildExportContent()
    expect(content).toContain('Keep')
    expect(content).not.toContain('Drop')
  })
})