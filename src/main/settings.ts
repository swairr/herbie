import { getDb } from './db-access'
import type { Settings, SettingsKey } from '@shared/types'

const DEFAULTS: Settings = {
  shortcut: 'Ctrl+Shift+Space',
  exportDir: '',
  draft: '',
  idleThresholdSec: '300',
  shortcutError: null
}

export function getSetting(key: SettingsKey): string | null {
  const row = getDb()
    .prepare('SELECT value FROM settings WHERE key = ?')
    .get(key) as { value: string } | undefined
  return row ? row.value : null
}

export function setSetting(key: SettingsKey, value: string): void {
  getDb()
    .prepare(
      'INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value'
    )
    .run(key, value)
}

export function getDefault(key: SettingsKey): string {
  const v = DEFAULTS[key]
  return v == null ? '' : v
}

export function getSettingWithDefault(key: SettingsKey): string {
  return getSetting(key) ?? getDefault(key)
}

export function getAllSettings(): Partial<Settings> {
  const rows = getDb().prepare('SELECT key, value FROM settings').all() as { key: string; value: string }[]
  const out: Record<string, string> = {}
  for (const r of rows) out[r.key] = r.value
  return out as Partial<Settings>
}