import { describe, it, expect, afterEach } from 'vitest'
import { makeDb, resetDb } from './helpers/db'
import {
  createJournal,
  listJournals,
  updateJournal,
  softDeleteJournal
} from '../src/main/journals'
import { labelsForJournal } from '../src/main/labels-store'

afterEach(resetDb)

function wait(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms))
}

describe('journals repo', () => {
  it('creates a journal entry with required body and defaults date to today', () => {
    makeDb()
    const e = createJournal({ body: 'First note #work' })
    expect(e.id).toMatch(/[-0-9a-f]{20,}/)
    expect(e.title).toBeNull()
    expect(e.body).toBe('First note #work')
    expect(e.date).toMatch(/^\d{4}-\d{2}-\d{2}$/)
    expect(e.deletedAt).toBeNull()
  })

  it('stores a trimmed title when provided, null when blank', () => {
    makeDb()
    const a = createJournal({ title: '  Meeting  ', body: 'body' })
    expect(a.title).toBe('Meeting')
    const b = createJournal({ title: '   ', body: 'body' })
    expect(b.title).toBeNull()
  })

  it('rejects an empty body on create', () => {
    makeDb()
    expect(() => createJournal({ body: '   ' })).toThrow()
    expect(() => createJournal({ body: '' })).toThrow()
  })

  it('rejects an invalid date on create', () => {
    makeDb()
    expect(() => createJournal({ body: 'x', date: 'not-a-date' })).toThrow()
  })

  it('lists entries for a day ordered by createdAt ascending', async () => {
    makeDb()
    const a = createJournal({ body: 'a' })
    await wait(5)
    const b = createJournal({ body: 'b' })
    const list = listJournals(a.date)
    expect(list.map((e) => e.id)).toEqual([a.id, b.id])
  })

  it('only lists entries of the requested day', () => {
    makeDb()
    const today = createJournal({ body: 'today' })
    const other = createJournal({ body: 'past', date: '2020-01-01' })
    expect(listJournals(today.date).map((e) => e.id)).toEqual([today.id])
    expect(listJournals('2020-01-01').map((e) => e.id)).toEqual([other.id])
  })

  it('re-parses labels from body on create and update', () => {
    makeDb()
    const e = createJournal({ body: 'do #work and #meeting' })
    expect(labelsForJournal(e.id)).toEqual(['meeting', 'work'])
    updateJournal(e.id, { body: 'now only #work' })
    expect(labelsForJournal(e.id)).toEqual(['work'])
  })

  it('update edits title/body/date and bumps updatedAt', async () => {
    makeDb()
    const e = createJournal({ body: 'orig' })
    await wait(5)
    const updated = updateJournal(e.id, {
      title: 'T',
      body: 'new #tag',
      date: '2020-06-06'
    })
    expect(updated.title).toBe('T')
    expect(updated.body).toBe('new #tag')
    expect(updated.date).toBe('2020-06-06')
    expect(updated.updatedAt).not.toBe(e.updatedAt)
    expect(labelsForJournal(e.id)).toEqual(['tag'])
  })

  it('update rejects empty body when body is provided', () => {
    makeDb()
    const e = createJournal({ body: 'orig' })
    expect(() => updateJournal(e.id, { body: '   ' })).toThrow()
  })

  it('update clears title when set to null', () => {
    makeDb()
    const e = createJournal({ title: 'T', body: 'orig' })
    const updated = updateJournal(e.id, { title: null })
    expect(updated.title).toBeNull()
  })

  it('update throws on unknown id', () => {
    makeDb()
    expect(() => updateJournal('nope', { body: 'x' })).toThrow()
  })

  it('softDelete hides entry from list', () => {
    makeDb()
    const e = createJournal({ body: 'gone' })
    softDeleteJournal(e.id)
    expect(listJournals(e.date)).toEqual([])
  })

  it('reschedules entry via date patch to a different day', () => {
    makeDb()
    const e = createJournal({ body: 'x', date: '2020-01-01' })
    updateJournal(e.id, { date: '2020-02-02' })
    expect(listJournals('2020-01-01').map((x) => x.id)).toEqual([])
    expect(listJournals('2020-02-02').map((x) => x.id)).toEqual([e.id])
  })
})