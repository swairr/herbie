import { describe, it, expect } from 'vitest'
import { splitLinks, groupItems } from '../src/renderer/src/utils'
import type { Todo } from '@shared/types'

function make(partial: Partial<Todo>): Todo {
  return {
    id: 'x',
    title: 'T',
    detail: '',
    createdAt: '2026-08-03T00:00:00Z',
    updatedAt: '2026-08-03T00:00:00Z',
    completedAt: null,
    deletedAt: null,
    ...partial
  }
}

describe('splitLinks', () => {
  it('returns a single text segment when no url', () => {
    expect(splitLinks('plain text')).toEqual([{ type: 'text', value: 'plain text' }])
  })

  it('splits a leading url', () => {
    expect(splitLinks('https://a.b/x rest')).toEqual([
      { type: 'url', value: 'https://a.b/x' },
      { type: 'text', value: ' rest' }
    ])
  })

  it('splits a trailing url', () => {
    expect(splitLinks('see https://a.b')).toEqual([
      { type: 'text', value: 'see ' },
      { type: 'url', value: 'https://a.b' }
    ])
  })

  it('handles multiple urls', () => {
    const segs = splitLinks('a https://1.io b http://2.io c')
    expect(segs).toEqual([
      { type: 'text', value: 'a ' },
      { type: 'url', value: 'https://1.io' },
      { type: 'text', value: ' b ' },
      { type: 'url', value: 'http://2.io' },
      { type: 'text', value: ' c' }
    ])
  })

  it('returns empty array for empty string', () => {
    expect(splitLinks('')).toEqual([])
  })

  it('does not treat bare text as url', () => {
    expect(splitLinks('foo bar')).toHaveLength(1)
  })
})

describe('groupItems', () => {
  it('separates pending and done', () => {
    const g = groupItems([
      make({ id: 'p1' }),
      make({ id: 'd1', completedAt: '2026-08-03T01:00:00Z' }),
      make({ id: 'p2' })
    ])
    expect(g.pending.map((t) => t.id)).toEqual(['p1', 'p2'])
    expect(g.done.map((t) => t.id)).toEqual(['d1'])
  })

  it('excludes soft-deleted from both groups', () => {
    const g = groupItems([
      make({ id: 'del', deletedAt: '2026-08-03T01:00:00Z' }),
      make({ id: 'p' })
    ])
    expect(g.pending).toHaveLength(1)
    expect(g.done).toHaveLength(0)
  })

  it('handles empty input', () => {
    expect(groupItems([])).toEqual({ pending: [], done: [] })
  })
})