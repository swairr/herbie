import { app, BrowserWindow, shell } from 'electron'
import { join } from 'node:path'
import { is } from '@electron-toolkit/utils'

let mainWindow: BrowserWindow | null = null
let quickAddWindow: BrowserWindow | null = null

function outDir(): string {
  return join(app.getAppPath(), 'out')
}

const preloadPath = () => join(outDir(), 'preload', 'index.js')
const rendererHtml = () => join(outDir(), 'renderer', 'index.html')

function baseWebPrefs() {
  return {
    preload: preloadPath(),
    sandbox: true,
    contextIsolation: true,
    nodeIntegration: false
  }
}

export function createMainWindow(): BrowserWindow {
  mainWindow = new BrowserWindow({
    title: 'Herbie',
    width: 900,
    height: 680,
    minWidth: 600,
    minHeight: 480,
    show: false,
    autoHideMenuBar: true,
    webPreferences: baseWebPrefs() as any
  })

  mainWindow.on('ready-to-show', () => mainWindow?.show())

  mainWindow.webContents.setWindowOpenHandler((details) => {
    void shell.openExternal(details.url)
    return { action: 'deny' }
  })

  if (is.dev && process.env['ELECTRON_RENDERER_URL']) {
    void mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    void mainWindow.loadFile(rendererHtml())
  }

  return mainWindow
}

export function getMainWindow(): BrowserWindow | null {
  return mainWindow
}

export function getOrCreateQuickAddWindow(): BrowserWindow {
  if (quickAddWindow && !quickAddWindow.isDestroyed()) return quickAddWindow
  quickAddWindow = new BrowserWindow({
    title: 'Quick Add',
    width: 460,
    height: 360,
    frame: false,
    resizable: false,
    show: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    webPreferences: baseWebPrefs() as any
  })

  quickAddWindow.on('blur', () => {
    // Notify the renderer so it can flush its draft, then it requests hide.
    quickAddWindow?.webContents.send('quickadd:blur')
  })

  if (is.dev && process.env['ELECTRON_RENDERER_URL']) {
    void quickAddWindow.loadURL(`${process.env['ELECTRON_RENDERER_URL']}/#/quickadd`)
  } else {
    void quickAddWindow.loadFile(rendererHtml(), { hash: 'quickadd' })
  }

  return quickAddWindow
}

export function showQuickAdd(): void {
  const w = getOrCreateQuickAddWindow()
  w.show()
  w.focus()
  w.webContents.send('quickadd:show')
}

export function hideQuickAdd(): void {
  quickAddWindow?.webContents.send('quickadd:hide')
  quickAddWindow?.hide()
}

export function getQuickAddWindow(): BrowserWindow | null {
  return quickAddWindow
}