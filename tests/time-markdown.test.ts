import { describe, it, expect } from 'vitest'
import { exportTimeMarkdown } from '@shared/time-markdown'
import type { Segment } from '@shared/types'

function mk(p: Partial<Segment>): Segment {
  return {
    id: p.id ?? 's1',
    startAt: p.startAt ?? '2026-08-03T10:00:00',
    endAt: p.endAt !== undefined ? p.endAt : '2026-08-03T11:00:00',
    processName: p.processName ?? 'app.exe',
    title: p.title ?? 'Title',
    note: p.note ?? '',
    todoId: p.todoId !== undefined ? p.todoId : null,
    kind: p.kind ?? 'activity'
  }
}

describe('exportTimeMarkdown', () => {
  it('renders the day heading and table headers', () => {
    const out = exportTimeMarkdown('2026-08-03', [])
    expect(out).toContain('# 时间记录 2026-08-03')
    expect(out).toContain('## 进程时长排行')
    expect(out).toContain('## Todo 时长排行')
    expect(out).toContain('## 片段清单')
  })

  it('aggregates process and todo rankings', () => {
    const out = exportTimeMarkdown(
      '2026-08-03',
      [
        mk({ id: 'a', processName: 'code.exe', todoId: 't1', startAt: '2026-08-03T09:00:00', endAt: '2026-08-03T10:00:00' }),
        mk({ id: 'b', processName: 'code.exe', todoId: 't1', startAt: '2026-08-03T10:00:00', endAt: '2026-08-03T10:30:00' }),
        mk({ id: 'c', processName: 'browser.exe', todoId: null, startAt: '2026-08-03T11:00:00', endAt: '2026-08-03T11:15:00' })
      ],
      { t1: 'Write plan' }
    )
    expect(out).toContain('code.exe')
    expect(out).toContain('browser.exe')
    expect(out).toContain('Write plan')
    expect(out).toContain('(未关联)')
  })

  it('excludes idle from process ranking but shows idle total line', () => {
    const out = exportTimeMarkdown(
      '2026-08-03',
      [
        mk({ id: 'i', kind: 'idle', processName: '[idle]', title: '', todoId: null, startAt: '2026-08-03T12:00:00', endAt: '2026-08-03T12:30:00' }),
        mk({ id: 'a', processName: 'app.exe', todoId: null, startAt: '2026-08-03T13:00:00', endAt: '2026-08-03T13:10:00' })
      ]
    )
    expect(out).toContain('空闲合计')
    expect(out).toMatch(/空闲合计：\S+/)
    // [idle] must not appear in the process ranking table (only app.exe)
    const ranking = out.split('## Todo 时长排行')[0]
    expect(ranking).not.toContain('[idle]')
  })

  it('prefers note over processName/title in the segment listing', () => {
    const out = exportTimeMarkdown(
      '2026-08-03',
      [mk({ note: '聚焦写作', title: 'Doc', processName: 'x.exe' })]
    )
    const listing = out.split('## 片段清单')[1]
    expect(listing).toContain('聚焦写作')
    expect(listing).not.toContain('Doc')
  })

  it('escapes pipe characters in cells', () => {
    const out = exportTimeMarkdown('2026-08-03', [mk({ title: 'a|b', note: '' })])
    expect(out).toContain('a\\|b')
  })
})