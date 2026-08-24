import type { Todo } from '@shared/types'

export const DEFAULT_SHORTCUT = 'Ctrl+Shift+Space'

export type KeyboardCapture = {
  key: string
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
}

export type CaptureResult =
  | { status: 'captured'; accelerator: string }
  | { status: 'cancel' }
  | { status: 'waiting' }

function mapKey(key: string): string | null {
  switch (key) {
    case ' ':
    case 'Spacebar':
      return 'Space'
    case 'ArrowUp':
      return 'Up'
    case 'ArrowDown':
      return 'Down'
    case 'ArrowLeft':
      return 'Left'
    case 'ArrowRight':
      return 'Right'
    case 'Enter':
    case 'Return':
      return 'Enter'
    case 'Tab':
      return 'Tab'
    case 'Backspace':
      return 'Backspace'
    case 'Delete':
    case 'Del':
      return 'Delete'
    case 'Insert':
      return 'Insert'
    case 'Home':
      return 'Home'
    case 'End':
      return 'End'
    case 'PageUp':
      return 'PageUp'
    case 'PageDown':
      return 'PageDown'
    case 'PrintScreen':
      return 'PrintScreen'
    case 'CapsLock':
      return 'CapsLock'
    case 'Meta':
    case 'Control':
    case 'Shift':
    case 'Alt':
      return null
    case '+':
      return 'Plus'
    case '-':
      return 'Minus'
    case ',':
      return 'Comma'
    case '.':
      return 'Period'
    case '/':
      return 'Slash'
    case ';':
      return 'Semicolon'
    case "'":
      return 'Quote'
    case '[':
      return 'BracketLeft'
    case ']':
      return 'BracketRight'
    case '\\':
      return 'Backslash'
    case '`':
      return 'Backquote'
    default:
      if (/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) return key
      if (/^[a-zA-Z0-9]$/.test(key)) return key.toUpperCase()
      return null
  }
}

export function captureAccelerator(e: KeyboardCapture): CaptureResult {
  if (e.key === 'Escape') return { status: 'cancel' }
  const mods: string[] = []
  if (e.ctrlKey) mods.push('Ctrl')
  if (e.altKey) mods.push('Alt')
  if (e.shiftKey) mods.push('Shift')
  if (e.metaKey) mods.push('Super')
  const key = mapKey(e.key)
  if (!key) return { status: 'waiting' }
  return { status: 'captured', accelerator: [...mods, key].join('+') }
}

export function groupItems(todos: Todo[]): { pending: Todo[]; done: Todo[] } {
  const pending: Todo[] = []
  const done: Todo[] = []
  for (const t of todos) {
    if (t.deletedAt) continue
    if (t.completedAt) done.push(t)
    else pending.push(t)
  }
  return { pending, done }
}

export type Segment = { type: 'text'; value: string } | { type: 'url'; value: string }

export function splitLinks(text: string): Segment[] {
  const urlRe = /\bhttps?:\/\/[^\s]+/g
  const out: Segment[] = []
  let last = 0
  let m: RegExpExecArray | null
  while ((m = urlRe.exec(text)) !== null) {
    if (m.index > last) out.push({ type: 'text', value: text.slice(last, m.index) })
    out.push({ type: 'url', value: m[0] })
    last = m.index + m[0].length
  }
  if (last < text.length) out.push({ type: 'text', value: text.slice(last) })
  return out
}