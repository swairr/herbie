import { describe, expect, it, vi } from 'vitest'

const { FakeBrowserWindow } = vi.hoisted(() => {
  class FakeBrowserWindow {
    static instances: FakeBrowserWindow[] = []
    destroyed = false
    readonly webContents = {
      setWindowOpenHandler: vi.fn(),
      loadFile: vi.fn(),
      loadURL: vi.fn()
    }

    constructor() {
      FakeBrowserWindow.instances.push(this)
    }

    isDestroyed(): boolean {
      return this.destroyed
    }

    on = vi.fn()
    show = vi.fn()
    focus = vi.fn()
    loadFile = vi.fn()
    loadURL = vi.fn()
  }

  return { FakeBrowserWindow }
})

vi.mock('electron', () => ({
  BrowserWindow: FakeBrowserWindow,
  app: { getAppPath: () => 'C:/herbie' },
  shell: { openExternal: vi.fn() }
}))

vi.mock('@electron-toolkit/utils', () => ({ is: { dev: false } }))

import { createMainWindow, getMainWindow, showMainWindow } from '../src/main/windows'

describe('main window lifecycle', () => {
  it('does not return a destroyed main window', () => {
    const window = createMainWindow() as unknown as InstanceType<typeof FakeBrowserWindow>
    window.destroyed = true

    expect(getMainWindow()).toBeNull()
  })

  it('recreates the main window when tray access follows a close', () => {
    const oldWindow = createMainWindow() as unknown as InstanceType<typeof FakeBrowserWindow>
    oldWindow.destroyed = true

    showMainWindow()

    const currentWindow = getMainWindow() as unknown as InstanceType<typeof FakeBrowserWindow>
    expect(currentWindow).not.toBe(oldWindow)
    expect(currentWindow).not.toBeNull()
  })
})
