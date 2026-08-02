import { describe, it, expect, afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import { getSetting, setSetting, getSettingWithDefault, getAllSettings } from '../src/main/settings'

afterEach(resetDb)

describe('settings repo', () => {
  it('returns null for unset key', () => {
    makeDb()
    expect(getSetting('shortcut')).toBeNull()
  })

  it('writes and reads back a value', () => {
    makeDb()
    setSetting('shortcut', 'Alt+Space')
    expect(getSetting('shortcut')).toBe('Alt+Space')
  })

  it('upserts on conflict', () => {
    makeDb()
    setSetting('exportDir', '/a')
    setSetting('exportDir', '/b')
    expect(getSetting('exportDir')).toBe('/b')
  })

  it('returns default when unset via getSettingWithDefault', () => {
    makeDb()
    expect(getSettingWithDefault('shortcut')).toBe('Ctrl+Shift+Space')
  })

  it('overrides default once set', () => {
    makeDb()
    setSetting('shortcut', 'Ctrl+K')
    expect(getSettingWithDefault('shortcut')).toBe('Ctrl+K')
  })

  it('getAllSettings returns empty object when nothing set', () => {
    makeDb()
    expect(getAllSettings()).toEqual({})
  })

  it('getAllSettings returns all set keys', () => {
    makeDb()
    setSetting('shortcut', 'Ctrl+K')
    setSetting('exportDir', '/x')
    const all = getAllSettings()
    expect(all.shortcut).toBe('Ctrl+K')
    expect(all.exportDir).toBe('/x')
  })
})