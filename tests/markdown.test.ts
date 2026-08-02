import { describe, it, expect } from 'vitest'
import { exportMarkdown } from '@shared/markdown'
import type { Todo } from '@shared/types'

function make(partial: Partial<Todo>): Todo {
  return {
    id: 'id-1',
    title: 'T',
    detail: '',
    createdAt: '2026-08-03T00:00:00+08:00',
    updatedAt: '2026-08-03T00:00:00+08:00',
    completedAt: null,
    deletedAt: null,
    ...partial
  }
}

describe('exportMarkdown', () => {
  it('renders pending item with id comment and Created', () => {
    const out = exportMarkdown([make({ id: 'abc', title: 'Buy milk' })])
    expect(out).toContain('- [ ] Buy milk <!-- id:abc -->')
    expect(out).toMatch(/Created: \d{4}-\d{2}-\d{2} \d{2}:\d{2}/)
  })

  it('renders done item with [x] and Completed line', () => {
    const out = exportMarkdown([
      make({
        id: 'd1',
        title: 'Done',
        completedAt: '2026-08-03T09:30:00+08:00'
      })
    ])
    expect(out).toContain('- [x] Done <!-- id:d1 -->')
    expect(out).toMatch(/Completed: \d{4}-\d{2}-\d{2} \d{2}:\d{2}/)
  })

  it('indents multi-line detail by 4 spaces', () => {
    const out = exportMarkdown([
      make({ id: 'm', title: 'Multi', detail: 'line1\nline2' })
    ])
    expect(out).toContain('    line1')
    expect(out).toContain('    line2')
  })

  it('omits detail block when detail empty', () => {
    const out = exportMarkdown([make({ id: 'e', title: 'No detail', detail: '' })])
    expect(out).not.toContain('    line')
    expect(out).toContain('Created:')
  })

  it('excludes soft-deleted todos', () => {
    const out = exportMarkdown([make({ id: 'del', title: 'X', deletedAt: '2026-08-03T00:00:00+08:00' })])
    expect(out).not.toContain('id:del')
  })

  it('orders pending before done', () => {
    const out = exportMarkdown([
      make({ id: 'done', title: 'Done', completedAt: '2026-08-03T09:00:00+08:00' }),
      make({ id: 'p', title: 'Pending' })
    ])
    const pendingIdx = out.indexOf('id:p')
    const doneIdx = out.indexOf('id:done')
    expect(pendingIdx).toBeGreaterThan(-1)
    expect(doneIdx).toBeGreaterThan(-1)
    expect(pendingIdx).toBeLessThan(doneIdx)
  })

  it('renders only a header for empty input', () => {
    const out = exportMarkdown([])
    expect(out.trim()).toBe('# Todos')
  })

  it('sorts within pending by createdAt descending', () => {
    const out = exportMarkdown([
      make({ id: 'old', title: 'Old', createdAt: '2026-01-01T00:00:00Z' }),
      make({ id: 'new', title: 'New', createdAt: '2026-06-01T00:00:00Z' })
    ])
    expect(out.indexOf('id:new')).toBeLessThan(out.indexOf('id:old'))
  })

  it('sorts done by completedAt descending', () => {
    const out = exportMarkdown([
      make({ id: 'first', title: 'First', completedAt: '2026-01-01T00:00:00Z' }),
      make({ id: 'last', title: 'Last', completedAt: '2026-06-01T00:00:00Z' })
    ])
    expect(out.indexOf('id:last')).toBeLessThan(out.indexOf('id:first'))
  })

  it('renders title with markdown special chars verbatim', () => {
    const out = exportMarkdown([make({ id: 's', title: 'Check [x] and <b>', detail: '' })])
    expect(out).toContain('Check [x] and <b>')
  })

  it('preserves blank lines inside multi-line detail', () => {
    const out = exportMarkdown([make({ id: 'm', title: 'M', detail: 'a\n\nb' })])
    expect(out).toContain('    a')
    expect(out).toContain('    b')
  })

  it('does not emit Completed line for pending', () => {
    const out = exportMarkdown([make({ id: 'p', title: 'P' })])
    expect(out).not.toContain('Completed:')
  })
})