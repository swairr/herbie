import { describe, it, expect } from 'vitest'
import { captureAccelerator, type KeyboardCapture } from '../src/renderer/src/utils'

function ev(key: string, mods: Partial<KeyboardCapture> = {}): KeyboardCapture {
  return {
    key,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...mods
  }
}

describe('captureAccelerator', () => {
  it('captures a plain combination', () => {
    expect(captureAccelerator(ev('v', { ctrlKey: true }))).toEqual({
      status: 'captured',
      accelerator: 'Ctrl+V'
    })
  })

  it('orders modifiers as Ctrl, Alt, Shift, Super', () => {
    expect(captureAccelerator(ev('a', { ctrlKey: true, altKey: true }))).toEqual({
      status: 'captured',
      accelerator: 'Ctrl+Alt+A'
    })
    expect(
      captureAccelerator(ev('q', { ctrlKey: true, altKey: true, shiftKey: true, metaKey: true }))
    ).toEqual({ status: 'captured', accelerator: 'Ctrl+Alt+Shift+Super+Q' })
  })

  it('maps space to Space', () => {
    expect(captureAccelerator(ev(' ', { ctrlKey: true, shiftKey: true }))).toEqual({
      status: 'captured',
      accelerator: 'Ctrl+Shift+Space'
    })
  })

  it('maps arrow keys', () => {
    expect(captureAccelerator(ev('ArrowLeft', { altKey: true }))).toEqual({
      status: 'captured',
      accelerator: 'Alt+Left'
    })
  })

  it('uppercases letters', () => {
    expect(captureAccelerator(ev('m', { ctrlKey: true, shiftKey: true }))).toEqual({
      status: 'captured',
      accelerator: 'Ctrl+Shift+M'
    })
  })

  it('keeps digits, F-keys and named keys', () => {
    expect(captureAccelerator(ev('5'))).toEqual({ status: 'captured', accelerator: '5' })
    expect(captureAccelerator(ev('F5'))).toEqual({ status: 'captured', accelerator: 'F5' })
    expect(captureAccelerator(ev('Enter'))).toEqual({ status: 'captured', accelerator: 'Enter' })
    expect(captureAccelerator(ev('PageUp'))).toEqual({ status: 'captured', accelerator: 'PageUp' })
  })

  it('escapes modifier-looking symbols', () => {
    expect(captureAccelerator(ev('+', { ctrlKey: true }))).toEqual({
      status: 'captured',
      accelerator: 'Ctrl+Plus'
    })
  })

  it('waits when only a modifier is pressed', () => {
    expect(captureAccelerator(ev('Control', { ctrlKey: true }))).toEqual({ status: 'waiting' })
    expect(captureAccelerator(ev('Shift', { shiftKey: true }))).toEqual({ status: 'waiting' })
  })

  it('waits on unmapped keys', () => {
    expect(captureAccelerator(ev('Unidentified'))).toEqual({ status: 'waiting' })
  })

  it('cancels on Escape', () => {
    expect(captureAccelerator(ev('Escape'))).toEqual({ status: 'cancel' })
  })
})