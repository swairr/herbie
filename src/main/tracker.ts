import { closeOpen, openSegment, type HookNotifier } from './segments'
import { nowIso } from '@shared/time'
import { getSettingWithDefault } from './settings'

// Dependency-injected surface for idle polling + power events. Keeping these injectable
// lets the tracker state machine be unit-tested with a fake clock and fake power monitor,
// mirroring the db-access / segments split (no `electron` import in the testable logic).
export interface TrackerDeps {
  getIdle: () => number
  onPowerEvent: (name: PowerEventName, cb: () => void) => void
  setInterval: (cb: () => void, ms: number) => unknown
  clearInterval: (handle: unknown) => void
  now: () => Date
}

type PowerEventName = 'suspend' | 'resume' | 'lock' | 'unlock'

const DEFAULT_INTERVAL_MS = 20_000
const IDLE_PROCESS = '[idle]'
const DEFAULT_THRESHOLD_SEC = 300

export interface Tracker {
  start(): void
  stop(): void
  reconfigure(): void
  setOffWork(on: boolean): void
  getOffWork(): boolean
  gateNotifier(real: HookNotifier): HookNotifier
}

// Process-wide holder so the IPC layer can reach the live tracker without threading it
// through registerIpcHandlers. The testable state machine still lives in createTracker.
let instance: Tracker | null = null
export function setTrackerInstance(t: Tracker | null): void {
  instance = t
}
export function getTracker(): Tracker | null {
  return instance
}

export function createTracker(deps: TrackerDeps): Tracker {
  let offWork = false
  let wasIdle = false
  let handle: unknown = null

  function threshold(): number {
    const raw = getSettingWithDefault('idleThresholdSec')
    const n = Number(raw)
    return Number.isFinite(n) && n > 0 ? n : DEFAULT_THRESHOLD_SEC
  }

  function poll(): void {
    const idleSec = deps.getIdle()
    const now = deps.now()
    if (offWork) {
      // No segments written while off-work; only detect return-to-work via idle->0.
      if (idleSec === 0 && wasIdle) {
        offWork = false
        wasIdle = false
      } else if (idleSec > 0) {
        wasIdle = true
      }
      return
    }
    const thr = threshold()
    if (idleSec >= thr && !wasIdle) {
      // Just crossed into idle: close the activity segment at the last-input moment and
      // open an idle segment starting at that same instant.
      const lastInputMs = now.getTime() - idleSec * 1000
      const lastInputIso = new Date(lastInputMs).toISOString()
      closeOpen(lastInputIso)
      openSegment({ processName: IDLE_PROCESS, title: '', kind: 'idle', startAt: lastInputIso })
      wasIdle = true
    } else if (idleSec === 0 && wasIdle) {
      // Back from idle: close the open idle segment now.
      closeOpen(now.toISOString())
      wasIdle = false
    }
  }

  function onPower(name: PowerEventName): void {
    if (name === 'suspend' || name === 'lock') {
      closeOpen(deps.now().toISOString())
      wasIdle = false
    } else {
      // resume / unlock: do not open anything; next foreground event opens naturally.
      wasIdle = false
    }
  }

  return {
    start() {
      if (handle != null) return
      for (const n of ['suspend', 'resume', 'lock', 'unlock'] as PowerEventName[]) {
        deps.onPowerEvent(n, () => onPower(n))
      }
      handle = deps.setInterval(poll, DEFAULT_INTERVAL_MS)
    },
    stop() {
      if (handle != null) {
        deps.clearInterval(handle)
        handle = null
      }
      closeOpen(nowIso())
    },
    reconfigure() {
      // Threshold is read live each poll; nothing to rewire here. Kept as a hook so the
      // settings listener can call it without us reaching into internals later.
    },
    setOffWork(on) {
      if (on && !offWork) {
        closeOpen(deps.now().toISOString())
        offWork = true
        wasIdle = false
      } else if (!on && offWork) {
        offWork = false
      }
    },
    getOffWork() {
      return offWork
    },
    gateNotifier(real) {
      return {
        start(cb) {
          real.start((e) => {
            // Any real foreground activity returns to work, then opens a fresh segment.
            // closeOpen during the off-work transition is a no-op (we already closed on
            // entering off-work), so cb's own close+open produces a clean new segment.
            if (offWork) offWork = false
            // A foreground event closes any open idle segment and opens an activity one,
            // so the open segment is no longer an idle one — clear the staleness flag or
            // the next idle->0 poll would wrongly close the new activity segment.
            wasIdle = false
            cb(e)
          })
        },
        stop() {
          real.stop()
        }
      }
    }
  }
}