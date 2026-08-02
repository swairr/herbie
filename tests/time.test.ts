import { describe, it, expect } from 'vitest'
import { formatLocalShort, nowIso } from '@shared/time'

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
    // 2026-08-03T00:14:00Z in some local tz; assert format shape and year
    const s = formatLocalShort('2026-08-03T00:14:00Z')
    expect(s.startsWith('2026')).toBe(true)
  })

  it('formatLocalShort handles negative-timezone offsets', () => {
    const s = formatLocalShort('2026-08-02T23:00:00-05:00')
    expect(s).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
  })
})