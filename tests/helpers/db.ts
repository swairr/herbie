import Database from 'better-sqlite3'
import { initInMemory, closeDb } from '../../src/main/db-access'

export function makeDb(): Database.Database {
  closeDb() // ensure no leak between tests
  const db = new Database(':memory:')
  initInMemory(db)
  return db
}

export function resetDb(): void {
  closeDb()
}