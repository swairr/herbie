// Pure time formatting helpers. Storage uses ISO 8601; display uses local short form.

export function nowIso(): string {
  return new Date().toISOString()
}

function pad(n: number): string {
  return n < 10 ? '0' + n : String(n)
}

// Format an ISO timestamp as local "YYYY-MM-DD HH:mm".
export function formatLocalShort(iso: string | null): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  const y = d.getFullYear()
  const mo = pad(d.getMonth() + 1)
  const da = pad(d.getDate())
  const h = pad(d.getHours())
  const mi = pad(d.getMinutes())
  return `${y}-${mo}-${da} ${h}:${mi}`
}

// Local calendar date as "YYYY-MM-DD". Uses the runtime local timezone so ISO timestamps
// are bucketed into the same natural day the user actually experienced them.
export function localDateString(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

// A segment-like record carrying start (required) and end (null = still open).
export interface IntervalLike {
  startAt: string
  endAt: string | null
}

// Local-day [start, end) bounds for a "YYYY-MM-DD" string, in the runtime local timezone.
// Single source of truth shared by splitAtMidnight (in-memory clip) and the SQL day
// prefilter in segments-query so the two can never drift apart. Returns null on malformed
// input.
export interface DayBounds {
  startMs: number
  endMs: number
  startIso: string
  endIso: string
}

export function dayBounds(localDate: string): DayBounds | null {
  const parts = localDate.split('-').map(Number)
  if (parts.length !== 3 || parts.some((n) => Number.isNaN(n))) return null
  const [y, m, d] = parts
  const start = new Date(y, m - 1, d, 0, 0, 0, 0)
  const end = new Date(y, m - 1, d + 1, 0, 0, 0, 0)
  const startMs = start.getTime()
  const endMs = end.getTime()
  if (Number.isNaN(startMs) || Number.isNaN(endMs)) return null
  return { startMs, endMs, startIso: start.toISOString(), endIso: end.toISOString() }
}

// Slice `seg` to the local natural day `localDate` ("YYYY-MM-DD"). Returns the sub-segment
// overlapping [day 00:00, next day 00:00) or null when there is no overlap. Open segments
// (endAt === null) are clamped to `now` so aggregation reflects only elapsed time.
// Segments crossing midnight produce one slice per day; each slice keeps every other
// field of `seg` verbatim. Total: never throws on malformed input.
export function splitAtMidnight<T extends IntervalLike>(
  seg: T,
  localDate: string,
  now: Date = new Date()
): (T & { endAt: string }) | null {
  const bounds = dayBounds(localDate)
  if (!bounds) return null
  const { startMs: dayStart, endMs: dayEnd } = bounds

  const startMs = new Date(seg.startAt).getTime()
  if (Number.isNaN(startMs)) return null
  const rawEnd = seg.endAt == null ? now.toISOString() : seg.endAt
  const endMs = new Date(rawEnd).getTime()
  if (Number.isNaN(endMs)) return null
  if (endMs < startMs) return null

  const lo = Math.max(startMs, dayStart)
  const hi = Math.min(endMs, dayEnd)
  if (hi <= lo) return null
  return { ...seg, startAt: new Date(lo).toISOString(), endAt: new Date(hi).toISOString() }
}

// Format a duration in milliseconds as a compact "Hh Mm" / "Mm" / "<1m" label.
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) ms = 0
  const totalMin = Math.floor(ms / 60000)
  const h = Math.floor(totalMin / 60)
  const m = totalMin % 60
  if (h > 0 && m > 0) return `${h}h ${m}m`
  if (h > 0) return `${h}h`
  if (m > 0) return `${m}m`
  return '<1m'
}

// Difference in milliseconds between two ISO timestamps; null/undefined end falls back to
// `now`. Negative or NaN results are clamped to 0.
export function durationMs(startAt: string, endAt: string | null | undefined, now: Date = new Date()): number {
  const s = new Date(startAt).getTime()
  if (Number.isNaN(s)) return 0
  const e = endAt == null ? now.getTime() : new Date(endAt).getTime()
  if (Number.isNaN(e)) return 0
  return Math.max(0, e - s)
}