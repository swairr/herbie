import type Database from 'better-sqlite3'
import { runMigrations } from './migrations'

// Electron-free holder of the better-sqlite3 singleton so the repos can be unit-tested
// in pure Node. `src/main/db.ts` (which imports `electron`) is the only caller that
// opens a real file and seeds this via `setDb`.

let dbInstance: Database.Database | null = null

export function getDb(): Database.Database {
  if (!dbInstance) {
    throw new Error('DB not initialized. Call setDb() (or initDb in main) first.')
  }
  return dbInstance
}

export function setDb(db: Database.Database): void {
  dbInstance = db
}

export function closeDb(): void {
  if (dbInstance) {
    dbInstance.close()
    dbInstance = null
  }
}

// Helper for tests: open an in-memory database and apply migrations.
export function initInMemory(db: Database.Database): void {
  db.pragma('foreign_keys = ON')
  runMigrations(db)
  setDb(db)
}