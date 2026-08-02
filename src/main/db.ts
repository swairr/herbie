import Database from 'better-sqlite3'
import { app } from 'electron'
import { join } from 'node:path'
import { runMigrations } from './migrations'
import { setDb } from './db-access'

// Electron-bound initialization. Opens the on-disk database in the user data dir,
// applies migrations, and registers the singleton in db-access (shared with tests).

export function initDb(): Database.Database {
  const dir = app.getPath('userData')
  const file = join(dir, 'herbie.db')
  const db = new Database(file)
  db.pragma('journal_mode = WAL')
  db.pragma('foreign_keys = ON')
  runMigrations(db)
  setDb(db)
  return db
}

export { getDb, closeDb } from './db-access'