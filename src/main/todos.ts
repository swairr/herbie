import { getDb } from './db-access'
import { parseLabels } from '@shared/labels'
import { nowIso } from '@shared/time'
import { updateTodoLabels } from './labels-store'
import type { Todo, TodoInput, TodoPatch, TodoFilter, LabelCount } from '@shared/types'
import { randomUUID } from 'node:crypto'

export function rowToTodo(r: {
  id: string
  title: string
  detail: string
  createdAt: string
  updatedAt: string
  completedAt: string | null
  deletedAt: string | null
}): Todo {
  return {
    id: r.id,
    title: r.title,
    detail: r.detail,
    createdAt: r.createdAt,
    updatedAt: r.updatedAt,
    completedAt: r.completedAt,
    deletedAt: r.deletedAt
  }
}

export function listTodos(filter?: TodoFilter): Todo[] {
  const db = getDb()
  let sql = 'SELECT * FROM todos WHERE deletedAt IS NULL'
  const params: string[] = []
  if (filter?.labels && filter.labels.length > 0) {
    const placeholders = filter.labels.map(() => '?').join(',')
    sql += ` AND id IN (SELECT todoId FROM todo_labels WHERE label IN (${placeholders}))`
    params.push(...filter.labels)
  }
  sql +=
    ' ORDER BY (completedAt IS NULL) DESC, completedAt DESC, createdAt DESC'
  const rows = db.prepare(sql).all(...params) as any[]
  return rows.map(rowToTodo)
}

export function listTodoLabels(): LabelCount[] {
  const rows = getDb()
    .prepare(
      `SELECT label, COUNT(DISTINCT tl.todoId) AS count
       FROM todo_labels tl
       JOIN todos t ON t.id = tl.todoId
       WHERE t.deletedAt IS NULL
       GROUP BY label
       ORDER BY count DESC, label ASC`
    )
    .all() as { label: string; count: number }[]
  return rows
}

export function createTodo(input: TodoInput): Todo {
  const db = getDb()
  const id = randomUUID()
  const now = nowIso()
  const title = input.title.trim()
  const detail = input.detail
  const stmt = db.prepare(
    `INSERT INTO todos (id, title, detail, createdAt, updatedAt, completedAt, deletedAt)
     VALUES (?, ?, ?, ?, ?, NULL, NULL)`
  )
  const tx = db.transaction(() => {
    stmt.run(id, title, detail, now, now)
    updateTodoLabels(id, parseLabels(detail))
  })
  tx()
  const row = db.prepare('SELECT * FROM todos WHERE id = ?').get(id) as any
  return rowToTodo(row)
}

export function updateTodo(id: string, patch: TodoPatch): Todo {
  const db = getDb()
  const existing = db.prepare('SELECT * FROM todos WHERE id = ?').get(id) as any
  if (!existing) throw new Error('todo not found: ' + id)
  const title = patch.title != null ? patch.title.trim() : existing.title
  const detail = patch.detail != null ? patch.detail : existing.detail
  const now = nowIso()
  const tx = db.transaction(() => {
    db.prepare(
      'UPDATE todos SET title = ?, detail = ?, updatedAt = ? WHERE id = ?'
    ).run(title, detail, now, id)
    updateTodoLabels(id, parseLabels(detail))
  })
  tx()
  const row = db.prepare('SELECT * FROM todos WHERE id = ?').get(id) as any
  return rowToTodo(row)
}

export function toggleTodo(id: string, done: boolean): Todo {
  const db = getDb()
  const now = nowIso()
  const completedAt = done ? nowIso() : null
  db.prepare('UPDATE todos SET completedAt = ?, updatedAt = ? WHERE id = ?').run(
    completedAt,
    now,
    id
  )
  const row = db.prepare('SELECT * FROM todos WHERE id = ?').get(id) as any
  return rowToTodo(row)
}

export function softDeleteTodo(id: string): void {
  const db = getDb()
  db.prepare('UPDATE todos SET deletedAt = ?, updatedAt = ? WHERE id = ?').run(
    nowIso(),
    nowIso(),
    id
  )
}