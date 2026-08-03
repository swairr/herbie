import sql0001 from '../../migrations/0001.sql?raw'
import sql0002 from '../../migrations/0002.sql?raw'
import Database from 'better-sqlite3'

interface Migration {
  version: number
  sql: string
}

const migrations: Migration[] = [
  { version: 1, sql: sql0001 },
  { version: 2, sql: sql0002 }
]

export function runMigrations(db: Database.Database): number {
  db.exec(`
    CREATE TABLE IF NOT EXISTS migrations (
      version   INTEGER PRIMARY KEY,
      appliedAt TEXT NOT NULL
    );
  `)

  const applied = new Set(
    db.prepare('SELECT version FROM migrations').all().map((r) => (r as { version: number }).version)
  )

  const insertApplied = db.prepare('INSERT INTO migrations (version, appliedAt) VALUES (?, ?)')

  const apply = db.transaction((m: Migration) => {
    db.exec(m.sql)
    insertApplied.run(m.version, new Date().toISOString())
  })

  let last = 0
  for (const m of migrations) {
    if (applied.has(m.version)) {
      last = Math.max(last, m.version)
      continue
    }
    apply(m)
    last = m.version
  }
  return last
}