import { describe, expect, it } from 'vitest'
import { normalizeWinHookEvent } from '../src/main/win-hook-event'

describe('normalizeWinHookEvent', () => {
  it('converts the native positional callback payload to an event object', () => {
    expect(normalizeWinHookEvent('foreground', 123, 'explorer.exe', 'Desktop')).toEqual({
      type: 'foreground',
      hwnd: 123,
      processName: 'explorer.exe',
      title: 'Desktop'
    })
  })

  it('passes through the object payload used by the current native module', () => {
    const event = { type: 'namechange' as const, hwnd: 123, processName: 'editor.exe', title: 'Doc' }
    expect(normalizeWinHookEvent(event)).toBe(event)
  })

  it('rejects incomplete positional payloads before they reach SQLite', () => {
    expect(() => normalizeWinHookEvent('foreground', 123, undefined, 'Desktop')).toThrow(
      'Invalid herbie-winhook event payload'
    )
  })
})
