import { describe, it, expect } from 'vitest'
import {
  MERGE_GAP_MS,
  actualMsByTodo,
  aggregateDaySegments,
  placeJournals,
  stableHue
} from '@shared/timeline'
import type { JournalEntry, Segment } from '@shared/types'

const DAY = '2026-11-09'

function at(hhmm: string, day = DAY): string {
  const [y, m, d] = day.split('-').map(Number)
  const [h, mi] = hhmm.split(':').map(Number)
  return new Date(y, m - 1, d, h, mi, 0, 0).toISOString()
}

function segment(start: string, end: string | null, partial: Partial<Segment> = {}): Segment {
  return {
    id: `${start}-${partial.processName ?? partial.todoId ?? 'x'}`,
    startAt: start,
    endAt: end,
    processName: 'Code.exe',
    title: '',
    note: '',
    todoId: null,
    kind: 'activity',
    ...partial
  }
}

function journal(partial: Partial<JournalEntry>): JournalEntry {
  return {
    id: 'j',
    title: null,
    body: 'body',
    date: DAY,
    time: null,
    createdAt: at('00:00'),
    updatedAt: at('00:00'),
    deletedAt: null,
    ...partial
  }
}

describe('aggregateDaySegments', () => {
  it('merges adjacent same-todo segments with a gap of at most two minutes', () => {
    const out = aggregateDaySegments(
      [
        segment(at('09:00'), at('09:20'), { todoId: 't1' }),
        segment(at('09:22'), at('09:40'), { todoId: 't1' })
      ],
      { t1: 'Plan A' },
      DAY
    )
    expect(out).toHaveLength(1)
    expect(out[0].key).toBe('todo:t1')
    expect(out[0].label).toBe('Plan A')
    expect(out[0].todoId).toBe('t1')
    expect(out[0].startMin).toBe(540)
    expect(out[0].endMin).toBe(580)
    expect(out[0].durationMs).toBe(38 * 60000)
    expect(out[0].segments).toHaveLength(2)
    expect(out[0].idle).toBe(false)
  })

  it('splits the same group when the gap exceeds the merge threshold', () => {
    const out = aggregateDaySegments(
      [
        segment(at('09:00'), at('09:20'), { todoId: 't1' }),
        segment(at('09:20'), at('09:21'), { todoId: 't1' }),
        segment(at('09:30'), at('09:40'), { todoId: 't1' })
      ],
      { t1: 'Plan A' },
      DAY
    )
    expect(out.map((b) => b.segments.length)).toEqual([2, 1])
    expect(MERGE_GAP_MS).toBe(120000)
    expect(out[0].id).not.toBe(out[1].id)
  })

  it('does not merge across another group even when the times touch', () => {
    const out = aggregateDaySegments(
      [
        segment(at('09:00'), at('09:10'), { todoId: 't1' }),
        segment(at('09:10'), at('09:20'), { todoId: 't2' }),
        segment(at('09:20'), at('09:30'), { todoId: 't1' })
      ],
      { t1: 'A', t2: 'B' },
      DAY
    )
    expect(out.map((b) => b.key)).toEqual(['todo:t1', 'todo:t2', 'todo:t1'])
  })

  it('groups unassigned activity by process name and ignores title churn', () => {
    const out = aggregateDaySegments(
      [
        segment(at('09:00'), at('09:10'), { title: 'index.ts' }),
        segment(at('09:10'), at('09:20'), { title: 'main.rs' })
      ],
      {},
      DAY
    )
    expect(out).toHaveLength(1)
    expect(out[0].key).toBe('process:Code.exe')
    expect(out[0].label).toBe('Code.exe')
    expect(out[0].detail).toBe('main.rs')
  })

  it('puts idle segments without a todo into one 空闲 block', () => {
    const out = aggregateDaySegments(
      [
        segment(at('12:00'), at('12:10'), { kind: 'idle', processName: '' }),
        segment(at('12:10'), at('12:20'), { kind: 'idle', processName: '' })
      ],
      {},
      DAY
    )
    expect(out).toHaveLength(1)
    expect(out[0].key).toBe('idle')
    expect(out[0].label).toBe('空闲')
    expect(out[0].idle).toBe(true)
  })

  it('attributes idle segments with a todoId to that todo', () => {
    const out = aggregateDaySegments(
      [segment(at('12:00'), at('12:10'), { kind: 'idle', todoId: 't1' })],
      { t1: 'Plan A' },
      DAY
    )
    expect(out[0].key).toBe('todo:t1')
    expect(out[0].label).toBe('Plan A')
    expect(out[0].idle).toBe(true)
  })

  it('clamps a segment that ends at the next local midnight to 24:00', () => {
    const out = aggregateDaySegments(
      [segment(at('23:30'), at('00:00', '2026-11-10'))],
      {},
      DAY
    )
    expect(out[0].startMin).toBe(1410)
    expect(out[0].endMin).toBe(1440)
  })

  it('uses now for a still-open segment', () => {
    const out = aggregateDaySegments([segment(at('09:00'), null)], {}, DAY, new Date(at('10:00')))
    expect(out[0].durationMs).toBe(60 * 60000)
    expect(out[0].endMin).toBe(600)
  })

  it('assigns lanes to overlapping aggregate blocks', () => {
    const out = aggregateDaySegments(
      [
        segment(at('09:00'), at('10:00'), { todoId: 't1' }),
        segment(at('09:30'), at('10:30'), { todoId: 't2' })
      ],
      { t1: 'A', t2: 'B' },
      DAY
    )
    expect(out.map((b) => b.lane)).toEqual([0, 1])
    expect(out.map((b) => b.lanes)).toEqual([2, 2])
  })
})

describe('actualMsByTodo', () => {
  it('sums activity and idle time per todo and ignores unassigned segments', () => {
    const out = actualMsByTodo([
      segment(at('09:00'), at('09:30'), { todoId: 't1' }),
      segment(at('09:30'), at('09:45'), { todoId: 't1', kind: 'idle' }),
      segment(at('09:45'), at('10:00'), { todoId: 't2' }),
      segment(at('10:00'), at('10:10'))
    ])
    expect(out).toEqual({ t1: 45 * 60000, t2: 15 * 60000 })
  })
})

describe('placeJournals', () => {
  it('splits timed entries into the rail and keeps the rest in the all-day zone', () => {
    const { timed, untimed } = placeJournals([
      journal({ id: 'late', time: '15:00', createdAt: at('15:00') }),
      journal({ id: 'none', time: null }),
      journal({ id: 'early', time: '09:00', createdAt: at('09:10') })
    ])
    expect(timed.map((t) => t.entry.id)).toEqual(['early', 'late'])
    expect(timed.map((t) => t.startMin)).toEqual([540, 900])
    expect(untimed.map((e) => e.id)).toEqual(['none'])
  })

  it('treats a malformed time as untimed', () => {
    const { timed, untimed } = placeJournals([journal({ id: 'bad', time: '25:00' })])
    expect(timed).toEqual([])
    expect(untimed.map((e) => e.id)).toEqual(['bad'])
  })
})

describe('stableHue', () => {
  it('is deterministic and stays in the hue range', () => {
    expect(stableHue('abc')).toBe(stableHue('abc'))
    expect(stableHue('t1')).toBeGreaterThanOrEqual(0)
    expect(stableHue('t1')).toBeLessThan(360)
  })
})
