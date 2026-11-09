import { describe, it, expect } from 'vitest'
import {
  timeToMinutes,
  minutesToTime,
  snapMinutes,
  isSlotOverdue,
  addLocalDays,
  startOfWeek,
  weekDates,
  formatPlanDate,
  formatPlanLabel,
  weekdayLabel,
  layoutDay,
  gridRange,
  expandGridRange,
  DEFAULT_GRID_START_MIN,
  DEFAULT_GRID_END_MIN
} from '@shared/schedule'
import { localDateString } from '@shared/time'
import type { Todo } from '@shared/types'

function todo(partial: Partial<Todo> = {}): Todo {
  return {
    id: 't',
    title: 'T',
    detail: '',
    createdAt: '2026-11-02T00:00:00.000Z',
    updatedAt: '2026-11-02T00:00:00.000Z',
    completedAt: null,
    deletedAt: null,
    sortOrder: 0,
    plannedDate: null,
    plannedStart: null,
    plannedEnd: null,
    ...partial
  }
}

const NOW = new Date(2026, 10, 2, 12, 0, 0, 0)
const TODAY = localDateString(NOW.toISOString())

describe('plan time helpers', () => {
  it('parses wall-clock HH:MM', () => {
    expect(timeToMinutes('09:30')).toBe(570)
    expect(timeToMinutes('9:05')).toBe(545)
    expect(timeToMinutes('00:00')).toBe(0)
    expect(timeToMinutes('23:59')).toBe(1439)
  })

  it('rejects malformed or out-of-range times', () => {
    expect(timeToMinutes(null)).toBeNull()
    expect(timeToMinutes('')).toBeNull()
    expect(timeToMinutes('24:00')).toBeNull()
    expect(timeToMinutes('09:60')).toBeNull()
    expect(timeToMinutes('9')).toBeNull()
  })

  it('formats minutes and keeps 24:00 for the grid end', () => {
    expect(minutesToTime(0)).toBe('00:00')
    expect(minutesToTime(570)).toBe('09:30')
    expect(minutesToTime(1440)).toBe('24:00')
  })

  it('snaps to the nearest step and clamps to a day', () => {
    expect(snapMinutes(62)).toBe(60)
    expect(snapMinutes(68)).toBe(75)
    expect(snapMinutes(-5)).toBe(0)
    expect(snapMinutes(1450)).toBe(1440)
    expect(snapMinutes(62, 30)).toBe(60)
  })
})

describe('isSlotOverdue', () => {
  it('is overdue when a timed slot ended before now', () => {
    expect(isSlotOverdue(todo({ plannedDate: TODAY, plannedStart: '09:00', plannedEnd: '10:00' }), NOW)).toBe(true)
  })

  it('is not overdue while the slot is still running or later today', () => {
    expect(isSlotOverdue(todo({ plannedDate: TODAY, plannedStart: '11:00', plannedEnd: '13:00' }), NOW)).toBe(false)
    expect(isSlotOverdue(todo({ plannedDate: TODAY, plannedStart: '13:00', plannedEnd: '14:00' }), NOW)).toBe(false)
  })

  it('date-only slots are overdue only after the day ends', () => {
    const today = todo({ plannedDate: TODAY })
    expect(isSlotOverdue(today, NOW)).toBe(false)
    expect(isSlotOverdue({ ...today, plannedDate: addLocalDays(TODAY, -1) }, NOW)).toBe(true)
  })

  it('done or deleted todos are never overdue', () => {
    const late = {
      plannedDate: addLocalDays(TODAY, -2),
      plannedStart: '09:00',
      plannedEnd: '10:00'
    }
    expect(isSlotOverdue(todo({ ...late, completedAt: '2026-11-01T10:00:00.000Z' }), NOW)).toBe(false)
    expect(isSlotOverdue(todo({ ...late, deletedAt: '2026-11-01T10:00:00.000Z' }), NOW)).toBe(false)
  })

  it('is never overdue without a planned date', () => {
    expect(isSlotOverdue(todo(), NOW)).toBe(false)
  })
})

describe('local day helpers', () => {
  it('adds days across month and year boundaries', () => {
    expect(addLocalDays('2026-11-02', -1)).toBe('2026-11-01')
    expect(addLocalDays('2026-11-02', 30)).toBe('2026-12-02')
    expect(addLocalDays('2026-12-31', 1)).toBe('2027-01-01')
    expect(addLocalDays('bad', 1)).toBe('bad')
  })

  it('weeks start on Monday', () => {
    expect(startOfWeek('2026-11-02')).toBe('2026-11-02')
    expect(startOfWeek('2026-11-05')).toBe('2026-11-02')
    expect(startOfWeek('2026-11-08')).toBe('2026-11-02')
    expect(weekDates('2026-11-05')).toEqual([
      '2026-11-02',
      '2026-11-03',
      '2026-11-04',
      '2026-11-05',
      '2026-11-06',
      '2026-11-07',
      '2026-11-08'
    ])
  })

  it('labels weekdays Monday-first', () => {
    expect(weekdayLabel('2026-11-02')).toBe('周一')
    expect(weekdayLabel('2026-11-08')).toBe('周日')
  })

  it('formats dates relative to today', () => {
    expect(formatPlanDate(TODAY, TODAY)).toBe('今天')
    expect(formatPlanDate(addLocalDays(TODAY, 1), TODAY)).toBe('明天')
    expect(formatPlanDate(addLocalDays(TODAY, -1), TODAY)).toBe('昨天')
    expect(formatPlanDate('2026-12-25', TODAY)).toBe('12-25')
  })

  it('formats a todo plan chip', () => {
    expect(formatPlanLabel(todo(), TODAY)).toBe('')
    expect(formatPlanLabel(todo({ plannedDate: TODAY }), TODAY)).toBe('今天')
    expect(
      formatPlanLabel(todo({ plannedDate: TODAY, plannedStart: '09:00', plannedEnd: '10:30' }), TODAY)
    ).toBe('今天 09:00–10:30')
  })
})

describe('layoutDay', () => {
  it('orders blocks and separates overlaps into lanes', () => {
    const a = todo({ id: 'a', plannedDate: TODAY, plannedStart: '09:00', plannedEnd: '10:00' })
    const b = todo({ id: 'b', plannedDate: TODAY, plannedStart: '09:30', plannedEnd: '10:30' })
    const c = todo({ id: 'c', plannedDate: TODAY, plannedStart: '11:00', plannedEnd: '12:00' })
    const untimed = todo({ id: 'u', plannedDate: TODAY })

    const out = layoutDay([c, untimed, b, a])
    expect(out.map((x) => x.todo.id)).toEqual(['a', 'b', 'c'])
    expect(out.map((x) => x.lane)).toEqual([0, 1, 0])
    expect(out.map((x) => x.lanes)).toEqual([2, 2, 1])
    expect(out[0].startMin).toBe(540)
    expect(out[0].endMin).toBe(600)
  })

  it('skips untimed and malformed slots', () => {
    const out = layoutDay([
      todo({ id: 'u' }),
      todo({ id: 'bad', plannedStart: '10:00', plannedEnd: '09:00' })
    ])
    expect(out).toEqual([])
  })
})

describe('gridRange', () => {
  it('defaults to 06:00–24:00', () => {
    expect(gridRange([])).toEqual({
      startMin: DEFAULT_GRID_START_MIN,
      endMin: DEFAULT_GRID_END_MIN
    })
  })

  it('expands for early and late slots, rounded to the hour', () => {
    const out = gridRange([
      todo({ plannedStart: '05:30', plannedEnd: '06:30' }),
      todo({ plannedStart: '22:30', plannedEnd: '23:15' })
    ])
    expect(out).toEqual({ startMin: 5 * 60, endMin: 24 * 60 })
  })

  it('always keeps at least one hour', () => {
    expect(gridRange([todo({ plannedStart: '10:00', plannedEnd: '10:15' })])).toEqual({
      startMin: DEFAULT_GRID_START_MIN,
      endMin: DEFAULT_GRID_END_MIN
    })
  })

  it('expands for point intervals such as timed journal entries', () => {
    expect(expandGridRange([{ startMin: 5 * 60 + 10, endMin: 5 * 60 + 10 }])).toEqual({
      startMin: 5 * 60,
      endMin: DEFAULT_GRID_END_MIN
    })
  })
})
