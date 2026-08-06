import type { WinHookEvent } from './segments'

export function normalizeWinHookEvent(
  eventOrType: WinHookEvent | WinHookEvent['type'],
  hwnd?: number,
  processName?: string,
  title?: string
): WinHookEvent {
  if (typeof eventOrType === 'object' && eventOrType !== null) {
    return eventOrType
  }
  if (typeof hwnd !== 'number' || typeof processName !== 'string' || typeof title !== 'string') {
    throw new TypeError('Invalid herbie-winhook event payload')
  }
  return { type: eventOrType, hwnd, processName, title }
}
