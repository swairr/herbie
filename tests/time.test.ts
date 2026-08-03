import { describe, it, expect } from 'vitest'
import {
  formatLocalShort,
  nowIso,
  localDateString,
  splitAtMidnight,
  formatDuration,
  durationMs
} from '@shared/time'
import type { Segment } from '@shared/types'

function seg(start: string, end: string | null, id = 's'): Segment {
  return {
    id,
    startAt: start,
    endAt: end,
    processName: 'app.exe',
    title: '',
    note: '',
    todoId: null,
    kind: 'activity'
  }
}

describe('time utils', () => {
  it('formatLocalShort produces YYYY-MM-DD HH:mm', () => {
    const iso = '2026-08-03T00:14:00+08:00'
    const s = formatLocalShort(iso)
    expect(s).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
  })

  it('formatLocalShort returns empty for null', () => {
    expect(formatLocalShort(null)).toBe('')
  })

  it('formatLocalShort returns empty for invalid', () => {
    expect(formatLocalShort('not-a-date')).toBe('')
  })

  it('nowIso is parseable ISO', () => {
    const d = new Date(nowIso())
    expect(Number.isNaN(d.getTime())).toBe(false)
  })

  it('nowIso has millisecond precision and time designator', () => {
    expect(nowIso()).toContain('T')
  })

  it('formatLocalShort converts to local timezone representation', () => {
    const s = formatLocalShort('2026-08-03T00:14:00Z')
    expect(s.startsWith('2026')).toBe(true)
  })

  it('formatLocalShort handles negative-timezone offsets', () => {
    const s = formatLocalShort('2026-08-02T23:00:00-05:00')
    expect(s).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
  })
})

describe('localDateString', () => {
  it('returns YYYY-MM-DD in local tz', () => {
    expect(localDateString('2026-08-03T00:14:00Z')).toMatch(/^\d{4}-\d{2}-\d{2}$/)
  })
  it('returns empty for invalid', () => {
    expect(localDateString('nope')).toBe('')
  })
})

describe('splitAtMidnight', () => {
  it('returns segment unchanged when fully within the day', () => {
    const s = seg('2026-08-03T10:00:00', '2026-08-03T11:30:00')
    const out = splitAtMidnight(s, '2026-08-03')
    expect(out).not.toBeNull()
    expect(new Date(out!.startAt).getHours()).toBe(10)
    expect(new Date(out!.endAt).getHours()).toBe(11)
    expect(new Date(out!.endAt).getMinutes()).toBe(30)
    expect(out!.id).toBe('s')
  })

  it('slices a cross-midnight segment into the day it starts', () => {
    const s = seg('2026-08-03T23:30:00', '2026-08-04T00:30:00')
    const startSlice = splitAtMidnight(s, '2026-08-03')
    expect(startSlice).not.toBeNull()
    expect(new Date(startSlice!.startAt).getHours()).toBe(23)
    expect(new Date(startSlice!.endAt).getHours()).toBe(0)
    expect(new Date(startSlice!.endAt).getDate()).toBe(4)
  })

  it('slices a cross-midnight segment into the day it ends', () => {
    const s = seg('2026-08-03T23:30:00', '2026-08-04T00:30:00')
    const endSlice = splitAtMidnight(s, '2026-08-04')
    expect(endSlice).not.toBeNull()
    expect(new Date(endSlice!.startAt).getHours()).toBe(0)
    expect(new Date(endSlice!.endAt).getHours()).toBe(0)
    expect(new Date(endSlice!.endAt).getMinutes()).toBe(30)
  })

  it('returns null when segment is on a different day', () => {
    const s = seg('2026-08-03T10:00:00', '2026-08-03T11:00:00')
    expect(splitAtMidnight(s, '2026-08-05')).toBeNull()
  })

  it('clamps open segment to now', () => {
    const now = new Date(2026, 7, 3, 15, 0, 0)
    const s = seg('2026-08-03T10:00:00', null)
    const out = splitAtMidnight(s, '2026-08-03', now)
    expect(out).not.toBeNull()
    expect(out!.endAt).toBe(now.toISOString())
  })

  it('open segment started yesterday appears in today as midnight..now', () => {
    const now = new Date(2026, 7, 4, 9, 0, 0)
    const s = seg('2026-08-03T22:00:00', null)
    const out = splitAtMidnight(s, '2026-08-04', now)
    expect(out).not.toBeNull()
    expect(new Date(out!.startAt).getHours()).toBe(0)
    expect(out!.endAt).toBe(now.toISOString())
  })

  it('handles malformed date string as null', () => {
    const s = seg('not-a-date', '2026-08-03T11:00:00')
    expect(splitAtMidnight(s, '2026-08-03')).toBeNull()
  })

  it('handles segment boundary exactly at midnight', () => {
    const s = seg('2026-08-03T00:00:00', '2026-08-04T00:00:00')
    const out = splitAtMidnight(s, '2026-08-03')
    expect(out).not.toBeNull()
    expect(new Date(out!.startAt).getTime()).toBe(new Date('2026-08-03T00:00:00').getTime())
  })
})

describe('formatDuration', () => {
  it('formats hours and minutes', () => {
    const r = formatDuration(2 * 3600_000 + 30 * 60_000)
    expect(r).toContain('2h')
    expect(r).toContain('30m')
  })
  it('formats sub-hour minutes', () => {
    const r = formatDuration(45 * 60_000)
    expect(r).toContain('45m')
    expect(r).not.toContain('h')
  })
  it('formats sub-minute as <1m', () => {
    expect(formatDuration(30_000)).toBe('<1m')
    expect(formatDuration(0)).toBe('<1m')
  })
  it('formats pure hours without minutes', () => {
    const r = formatDuration(3 * 3600_000)
    expect(r).toContain('3h')
    expect(r).not.toContain('m')
  })
  it('clamps negatives', () => {
    expect(formatDuration(-100)).toBe('<1m')
  })
})

describe('durationMs', () => {
  it('computes milliseconds between bounds', () => {
    expect(durationMs('2026-08-03T10:00:00', '2026-08-03T10:30:00')).toBe(30 * 60_000)
  })
  it('falls back to now for open end', () => {
    const now = new Date('2026-08-03T11:00:00Z')
    expect(durationMs('2026-08-03T10:00:00Z', null, now)).toBe(3600_000)
  })
  it('clamps negative to 0', () => {
    expect(durationMs('2026-08-03T11:00:00Z', '2026-08-03T10:00:00Z')).toBe(0)
  })
})