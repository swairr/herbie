import { describe, it, expect, beforeEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import {
  openSegment,
  closeOpen,
  startTracking,
  stopTracking,
  listAllSegments,
  type HookNotifier,
  type WinHookEvent
} from '../src/main/segments'

beforeEach(() => {
  makeDb()
})

afterEach(resetDb)

import { afterEach } from 'vitest'

function fakeNotifier(): HookNotifier & {
  emit(e: WinHookEvent): void
  starts: number
  stops: number
} {
  let cb: ((e: WinHookEvent) => void) | null = null
  return {
    starts: 0,
    stops: 0,
    start(c) {
      this.starts++
      cb = c
    },
    stop() {
      this.stops++
      cb = null
    },
    emit(e) {
      if (cb) cb(e)
    }
  }
}

describe('segments business layer', () => {
  it('openSegment inserts an open (endAt NULL) activity row', () => {
    const s = openSegment({ processName: 'app.exe', title: 'Main' })
    expect(s.endAt).toBeNull()
    expect(s.processName).toBe('app.exe')
    expect(s.title).toBe('Main')
    expect(s.kind).toBe('activity')
  })

  it('closeOpen sets endAt on the open row and is idempotent', () => {
    openSegment({ processName: 'a', title: 't' })
    const at = '2026-08-03T10:30:00Z'
    const first = closeOpen(at)
    expect(first).toBe(1)
    expect(closeOpen(at)).toBe(0)
  })

  it('startTracking opens a new segment and closes the previous on each foreground event', () => {
    const hook = fakeNotifier()
    startTracking(hook)
    expect(hook.starts).toBe(1)

    hook.emit({ type: 'foreground', hwnd: 1, processName: 'a.exe', title: 'A' })
    hook.emit({ type: 'foreground', hwnd: 2, processName: 'b.exe', title: 'B' })

    const rows = listAllSegments()
    expect(rows).toHaveLength(2)
    expect(rows[0].processName).toBe('a.exe')
    expect(rows[0].endAt).not.toBeNull()
    expect(rows[1].processName).toBe('b.exe')
    expect(rows[1].endAt).toBeNull()
  })

  it('namechange for the current foreground hwnd updates title via close+open', () => {
    const hook = fakeNotifier()
    startTracking(hook)
    hook.emit({ type: 'foreground', hwnd: 5, processName: 'c.exe', title: 'Doc1' })
    hook.emit({ type: 'namechange', hwnd: 5, processName: 'c.exe', title: 'Doc2' })
    const rows = listAllSegments()
    expect(rows).toHaveLength(2)
    expect(rows[0].title).toBe('Doc1')
    expect(rows[1].title).toBe('Doc2')
    expect(rows[0].endAt).not.toBeNull()
    expect(rows[1].endAt).toBeNull()
  })

  it('namechange for a non-foreground hwnd is ignored', () => {
    const hook = fakeNotifier()
    startTracking(hook)
    hook.emit({ type: 'foreground', hwnd: 7, processName: 'c.exe', title: 'X' })
    const before = listAllSegments()
    hook.emit({ type: 'namechange', hwnd: 99, processName: 'other.exe', title: 'Y' })
    const after = listAllSegments()
    expect(after).toHaveLength(before.length)
    expect(after[after.length - 1].title).toBe('X')
    expect(after[after.length - 1].endAt).toBeNull()
  })

  it('stopTracking closes the open segment and stops the notifier', () => {
    const hook = fakeNotifier()
    startTracking(hook)
    hook.emit({ type: 'foreground', hwnd: 1, processName: 'a.exe', title: 'A' })
    stopTracking(hook, '2026-08-03T12:00:00Z')
    expect(hook.stops).toBe(1)
    const rows = listAllSegments()
    expect(rows[0].endAt).toBe('2026-08-03T12:00:00Z')
  })
})