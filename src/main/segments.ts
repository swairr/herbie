import { getDb } from './db-access'
import { nowIso } from '@shared/time'
import type { Segment, SegmentKind } from '@shared/types'
import { randomUUID } from 'node:crypto'
import { rowToSegment } from './segments-row'

// Event shape delivered by the native winhook module (T2). The native side already
// filters NAMECHANGE to the current foreground hwnd; we also re-check here so a stray
// late namechange for a previous window never corrupts the open segment.
export interface WinHookEvent {
  type: 'foreground' | 'namechange'
  hwnd: number
  processName: string
  title: string
}

export interface HookNotifier {
  start(cb: (e: WinHookEvent) => void): void
  stop(): void
}

export interface OpenSegmentInput {
  processName: string
  title: string
  kind?: SegmentKind
  todoId?: string | null
  startAt?: string
}

export function openSegment(input: OpenSegmentInput): Segment {
  const db = getDb()
  const id = randomUUID()
  const startAt = input.startAt ?? nowIso()
  const kind: SegmentKind = input.kind ?? 'activity'
  const todoId = input.todoId ?? null
  db.prepare(
    `INSERT INTO segments (id, startAt, endAt, processName, title, note, todoId, kind)
     VALUES (?, ?, NULL, ?, ?, '', ?, ?)`
  ).run(id, startAt, input.processName, input.title, todoId, kind)
  // Return the segment assembled from the known inputs — avoids a redundant SELECT after
  // the INSERT on the high-frequency tracking path; callers that need the row get it free.
  return {
    id,
    startAt,
    endAt: null,
    processName: input.processName,
    title: input.title,
    note: '',
    todoId,
    kind
  }
}

// Close the currently open segment (if any) by setting endAt. Idempotent — no-op when
// nothing is open. Returns the number of rows closed (0 or 1).
export function closeOpen(at: string): number {
  const db = getDb()
  const res = db.prepare('UPDATE segments SET endAt = ? WHERE endAt IS NULL').run(at)
  return res.changes
}

export function stopTracking(notifier: HookNotifier, at: string = nowIso()): void {
  closeOpen(at)
  notifier.stop()
}

// Register the native hook and translate events into segment open/close transitions.
// The notifier is injected so the state machine is unit-testable with a fake hook.
export function startTracking(notifier: HookNotifier): void {
  let currentHwnd = -1
  let openProcess = ''
  let openTitle = ''
  notifier.start((e) => {
    try {
      // NAMECHANGE is only meaningful for the window currently in the foreground; ignore
      // stray events for the previous foreground hwnd that may arrive late.
      if (e.type === 'namechange' && currentHwnd !== e.hwnd) return
      // Dedup: a chatty foreground window can emit identical (process,title) events
      // repeatedly. Don't close+reopen a segment when nothing changed — avoids write
      // amplification and thousands of 1-row fragments.
      if (
        e.type === 'namechange' &&
        e.processName === openProcess &&
        (e.title === openTitle || e.title === '')
      ) {
        return
      }
      currentHwnd = e.hwnd
      closeOpen(nowIso())
      openSegment({ processName: e.processName, title: e.title })
      openProcess = e.processName
      openTitle = e.title
    } catch (error) {
      // Native ThreadSafeFunction callbacks must not let business errors escape into N-API.
      console.error('[segments] failed to process native window event', error)
    }
  })
}

export function listAllSegments(): Segment[] {
  const rows = getDb().prepare('SELECT * FROM segments ORDER BY startAt ASC').all() as any[]
  return rows.map(rowToSegment)
}
