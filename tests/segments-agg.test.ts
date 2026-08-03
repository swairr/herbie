import { describe, it, expect } from 'vitest'
import { aggregateByProcess, aggregateByTodo, aggregateIdle } from '@shared/segments-agg'
import type { Segment } from '@shared/types'

function mk(p: Partial<Segment>): Segment {
  return {
    id: p.id ?? 'x',
    startAt: p.startAt ?? '2026-08-03T10:00:00',
    endAt: p.endAt !== undefined ? p.endAt : '2026-08-03T10:30:00',
    processName: p.processName ?? '',
    title: p.title ?? '',
    note: p.note ?? '',
    todoId: p.todoId !== undefined ? p.todoId : null,
    kind: p.kind ?? 'activity'
  }
}

describe('aggregateByProcess', () => {
  it('sums durations per processName', () => {
    const rows = aggregateByProcess([
      mk({ processName: 'a.exe', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T11:00:00' }),
      mk({ processName: 'a.exe', startAt: '2026-08-03T12:00:00', endAt: '2026-08-03T12:30:00' }),
      mk({ processName: 'b.exe', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:15:00' })
    ])
    expect(rows).toHaveLength(2)
    const a = rows.find((r) => r.key === 'a.exe')!
    expect(a.ms).toBe(90 * 60_000)
    const b = rows.find((r) => r.key === 'b.exe')!
    expect(b.ms).toBe(15 * 60_000)
    expect(rows[0].key).toBe('a.exe') // sorted desc
  })

  it('excludes idle segments', () => {
    const rows = aggregateByProcess([
      mk({ kind: 'idle', processName: '[idle]', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T11:00:00' }),
      mk({ processName: 'a.exe', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:10:00' })
    ])
    expect(rows).toHaveLength(1)
    expect(rows[0].key).toBe('a.exe')
  })

  it('treats empty processName as (unknown)', () => {
    const rows = aggregateByProcess([
      mk({ processName: '', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:05:00' })
    ])
    expect(rows[0].key).toBe('(unknown)')
  })

  it('clamps open segment end to now', () => {
    const now = new Date(2026, 7, 3, 11, 0, 0)
    const rows = aggregateByProcess([
      mk({ processName: 'a.exe', startAt: '2026-08-03T10:00:00', endAt: null })
    ], now)
    expect(rows[0].ms).toBe(60 * 60_000)
  })
})

describe('aggregateByTodo', () => {
  it('groups by todoId and resolves titles', () => {
    const rows = aggregateByTodo(
      [
        mk({ todoId: 't1', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:20:00' }),
        mk({ todoId: 't1', startAt: '2026-08-03T11:00:00', endAt: '2026-08-03T11:10:00' }),
        mk({ todoId: 't2', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:05:00' })
      ],
      { t1: 'Write plan', t2: 'Review' }
    )
    expect(rows).toHaveLength(2)
    expect(rows[0].key).toBe('t1')
    expect(rows[0].label).toBe('Write plan')
    expect(rows[0].ms).toBe(30 * 60_000)
  })

  it('buckets unassociated segments under (未关联)', () => {
    const rows = aggregateByTodo([
      mk({ todoId: null, startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:15:00' })
    ], {})
    expect(rows).toHaveLength(1)
    expect(rows[0].label).toBe('(未关联)')
  })

  it('marks deleted todo as (已删除) when title missing', () => {
    const rows = aggregateByTodo(
      [mk({ todoId: 'gone', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:10:00' })],
      {}
    )
    expect(rows[0].label).toBe('(已删除)')
  })

  it('counts idle segments under todo bucket', () => {
    const rows = aggregateByTodo(
      [
        mk({ todoId: 't1', kind: 'idle', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:20:00' }),
        mk({ todoId: 't1', kind: 'activity', startAt: '2026-08-03T10:20:00', endAt: '2026-08-03T10:30:00' })
      ],
      { t1: 'X' }
    )
    expect(rows[0].ms).toBe(30 * 60_000)
  })
})

describe('aggregateIdle', () => {
  it('sums idle segment durations', () => {
    const total = aggregateIdle([
      mk({ kind: 'idle', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:05:00' }),
      mk({ kind: 'activity', processName: 'a.exe', startAt: '2026-08-03T10:05:00', endAt: '2026-08-03T10:30:00' }),
      mk({ kind: 'idle', startAt: '2026-08-03T11:00:00', endAt: '2026-08-03T11:02:00' })
    ])
    expect(total).toBe(7 * 60_000)
  })
})