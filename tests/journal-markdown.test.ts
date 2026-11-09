import { describe, it, expect } from 'vitest'
import { exportJournalMarkdown } from '@shared/journal-markdown'
import type { JournalEntry } from '@shared/types'

function make(partial: Partial<JournalEntry>): JournalEntry {
  return {
    id: 'id-1',
    title: null,
    body: 'body',
    date: '2026-08-04',
    time: null,
    createdAt: '2026-08-04T00:00:00Z',
    updatedAt: '2026-08-04T00:00:00Z',
    deletedAt: null,
    ...partial
  }
}

describe('exportJournalMarkdown', () => {
  it('renders only the day header for empty input', () => {
    const out = exportJournalMarkdown('2026-08-04', [])
    expect(out.trim()).toBe('# 日志 2026-08-04')
  })

  it('renders an entry with a title, its body, and a stable id comment', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'abc', title: '会议纪要', body: '讨论了 A\n讨论了 B' })
    ])
    expect(out).toContain('## 会议纪要 <!-- id:abc -->')
    expect(out).toContain('讨论了 A')
    expect(out).toContain('讨论了 B')
  })

  it('uses the body first line as heading when there is no title', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'nt', title: null, body: '今日思考\n更多内容' })
    ])
    expect(out).toContain('## 今日思考 <!-- id:nt -->')
    expect(out).toContain('更多内容')
  })

  it('does not duplicate the first line into the body block for no-title entries', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'nt', title: null, body: '首行\n第二行' })
    ])
    expect((out.match(/首行/g) || []).length).toBe(1)
    expect(out).toContain('第二行')
  })

  it('falls back to a placeholder when title and body are both empty-ish', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'blank', title: null, body: '   ' })
    ])
    expect(out).toContain('## (无内容)')
    expect(out).toContain('id:blank')
  })

  it('orders entries by createdAt ascending', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'late', title: '晚', createdAt: '2026-08-04T10:00:00Z' }),
      make({ id: 'early', title: '早', createdAt: '2026-08-04T01:00:00Z' })
    ])
    expect(out.indexOf('id:early')).toBeLessThan(out.indexOf('id:late'))
  })

  it('renders a 时间 line under the heading when time is set', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'timed', title: '会议', body: '纪要', time: '14:30' })
    ])
    expect(out).toContain('## 会议 <!-- id:timed -->\n\n时间：14:30\n\n纪要')
  })

  it('omits the 时间 line when time is null', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'timed', title: '会议', body: '纪要', time: null })
    ])
    expect(out).not.toContain('时间：')
  })

  it('orders entries by explicit time before falling back to createdAt', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'afternoon', title: '下午', time: '15:00', createdAt: '2026-08-04T01:00:00Z' }),
      make({ id: 'morning', title: '上午', time: '09:00', createdAt: '2026-08-04T02:00:00Z' })
    ])
    expect(out.indexOf('id:morning')).toBeLessThan(out.indexOf('id:afternoon'))
  })

  it('orders untimed entries by the local clock time of createdAt', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'later', title: '后', createdAt: '2026-08-04T03:00:00Z' }),
      make({ id: 'earlier', title: '先', createdAt: '2026-08-04T01:00:00Z' })
    ])
    expect(out.indexOf('id:earlier')).toBeLessThan(out.indexOf('id:later'))
  })

  it('excludes soft-deleted entries', () => {
    const out = exportJournalMarkdown('2026-08-04', [
      make({ id: 'del', deletedAt: '2026-08-04T05:00:00Z', body: 'gone' })
    ])
    expect(out).not.toContain('id:del')
    expect(out).not.toContain('gone')
  })
})