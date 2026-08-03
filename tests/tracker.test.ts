import { describe, it, expect, beforeEach } from 'vitest'
import { afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import { createTracker, type TrackerDeps } from '../src/main/tracker'
import {
  closeOpen,
  openSegment,
  listAllSegments,
  type HookNotifier,
  type WinHookEvent
} from '../src/main/segments'
import { setSetting } from '../src/main/settings'

beforeEach(() => {
  makeDb()
})
afterEach(resetDb)

function fakeDeps(opts: Partial<TrackerDeps> = {}): TrackerDeps & {
  fire: (name: 'suspend' | 'resume' | 'lock' | 'unlock') => void
  setIdle: (s: number) => void
  advance: (ms: number) => void
  nowMs: number
} {
  let idle = 0
  const powerCbs = new Map<string, () => void>()
  let nowMs = Date.UTC(2026, 7, 3, 10, 0, 0)
  const now = () => new Date(nowMs)
  return {
    getIdle: opts.getIdle ?? (() => idle),
    onPowerEvent: opts.onPowerEvent ?? ((name, cb) => powerCbs.set(name, cb)),
    setInterval: opts.setInterval ?? ((cb) => {
      ;(globalThis as any).__trackerPoll = cb
      return 'handle'
    }),
    clearInterval: opts.clearInterval ?? (() => {}),
    now: opts.now ?? now,
    fire(name) {
      powerCbs.get(name)?.()
    },
    setIdle(s) {
      idle = s
    },
    advance(ms) {
      nowMs += ms
    },
    get nowMs() {
      return nowMs
    }
  }
}

function pollManually(): void {
  ;(globalThis as any).__trackerPoll()
}

function fakeNotifier(): HookNotifier & { emit(e: WinHookEvent): void } {
  let cb: ((e: WinHookEvent) => void) | null = null
  return {
    start(c) {
      cb = c
    },
    stop() {
      cb = null
    },
    emit(e) {
      cb?.(e)
    }
  }
}

describe('createTracker idle handling', () => {
  it('opens an idle segment when idle crosses threshold', () => {
    const deps = fakeDeps()
    setSetting('idleThresholdSec', '10')
    const t = createTracker(deps)
    t.start()
    deps.setIdle(12)
    deps.advance(12_000)
    pollManually()
    const rows = listAllSegments()
    expect(rows).toHaveLength(1)
    expect(rows[0].kind).toBe('idle')
    expect(rows[0].processName).toBe('[idle]')
    expect(rows[0].endAt).toBeNull()
  })

  it('closes the idle segment when idle returns to 0', () => {
    const deps = fakeDeps()
    setSetting('idleThresholdSec', '10')
    const t = createTracker(deps)
    t.start()
    deps.setIdle(12)
    deps.advance(12_000)
    pollManually()
    expect(listAllSegments().length).toBe(1)
    deps.setIdle(0)
    deps.advance(5_000)
    pollManually()
    const rows = listAllSegments()
    expect(rows).toHaveLength(1)
    expect(rows[0].endAt).not.toBeNull()
  })

  it('suspend/lock close the open segment without creating an idle segment', () => {
    const deps = fakeDeps()
    const t = createTracker(deps)
    t.start()
    deps.fire('lock')
    deps.fire('suspend')
    expect(listAllSegments()).toHaveLength(0)
  })
})

describe('createTracker off-work', () => {
  it('setOffWork(true) closes any open segment; a gated foreground event clears off-work', () => {
    const deps = fakeDeps()
    const t = createTracker(deps)
    const hook = fakeNotifier()
    const gated = t.gateNotifier(hook)
    let received: WinHookEvent | null = null
    gated.start((e) => {
      received = e
    })
    t.start()
    t.setOffWork(true)
    expect(t.getOffWork()).toBe(true)
    hook.emit({ type: 'foreground', hwnd: 1, processName: 'a.exe', title: 'A' })
    expect(t.getOffWork()).toBe(false)
    expect(received).not.toBeNull()
  })

  it('idle poll during off-work writes nothing', () => {
    const deps = fakeDeps()
    setSetting('idleThresholdSec', '10')
    const t = createTracker(deps)
    t.start()
    t.setOffWork(true)
    deps.setIdle(60)
    deps.advance(60_000)
    pollManually()
    expect(listAllSegments()).toHaveLength(0)
    // returning to input clears off-work
    deps.setIdle(0)
    pollManually()
    expect(t.getOffWork()).toBe(false)
  })

  it('gateNotifier opens a new segment after ending off-work on a foreground event', () => {
    const deps = fakeDeps()
    const t = createTracker(deps)
    const hook = fakeNotifier()
    const gated = t.gateNotifier(hook)
    // Wire startTracking manually (segments.ts pattern)
    let currentHwnd = -1
    gated.start((e) => {
      if (e.type === 'namechange' && currentHwnd !== e.hwnd) return
      currentHwnd = e.hwnd
      closeOpen(new Date().toISOString())
      openSegment({ processName: e.processName, title: e.title })
    })
    t.start()
    t.setOffWork(true)
    hook.emit({ type: 'foreground', hwnd: 1, processName: 'a.exe', title: 'A' })
    const rows = listAllSegments()
    expect(rows).toHaveLength(1)
    expect(rows[0].processName).toBe('a.exe')
  })

it('returning from idle via a foreground event does not let the next poll close the new activity segment', () => {
  const deps = fakeDeps()
  setSetting('idleThresholdSec', '10')
  const t = createTracker(deps)
  const hook = fakeNotifier()
  const gated = t.gateNotifier(hook)
  let currentHwnd = -1
  gated.start((e) => {
    if (e.type === 'namechange' && currentHwnd !== e.hwnd) return
    currentHwnd = e.hwnd
    closeOpen(deps.now().toISOString())
    openSegment({ processName: e.processName, title: e.title })
  })
  t.start()
  // 1) user goes idle: poll opens an idle segment
  deps.setIdle(15)
  deps.advance(15_000)
  pollManually()
  let rows = listAllSegments()
  expect(rows).toHaveLength(1)
  expect(rows[0].kind).toBe('idle')
  expect(rows[0].endAt).toBeNull()
  // 2) user returns via a foreground event: idle closed + activity opened
  hook.emit({ type: 'foreground', hwnd: 2, processName: 'a.exe', title: 'A' })
  rows = listAllSegments()
  expect(rows).toHaveLength(2)
  const idleSeg = rows.find((r) => r.kind === 'idle')!
  const actSeg = rows.find((r) => r.kind === 'activity')!
  expect(idleSeg.endAt).not.toBeNull()
  expect(actSeg.endAt).toBeNull()
  // 3) next poll, user active (idle 0): the new activity segment must stay open
  deps.setIdle(0)
  deps.advance(5_000)
  pollManually()
  rows = listAllSegments()
  expect(rows).toHaveLength(2)
  const actSegAfter = rows.find((r) => r.kind === 'activity')!
  expect(actSegAfter.endAt).toBeNull()
})
})