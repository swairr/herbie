import { describe, it, expect } from 'vitest'
import { parseLabels } from '@shared/labels'

describe('parseLabels', () => {
  it('extracts unicode labels from detail', () => {
    expect(parseLabels('今天 #工作 很忙 #项目-x')).toEqual(['工作', '项目-x'])
  })

  it('skips # inside URLs', () => {
    expect(parseLabels('见 https://example.com/page#section 和 #工作')).toEqual(['工作'])
  })

  it('skips # that is part of an http url token', () => {
    expect(parseLabels('https://git.io/abc#def #todo')).toEqual(['todo'])
  })

  it('dedupes same-named labels', () => {
    expect(parseLabels('#a #a #b')).toEqual(['a', 'b'])
  })

  it('is case-sensitive', () => {
    expect(parseLabels('#Work #WORK')).toEqual(['Work', 'WORK'])
  })

  it('returns empty for empty detail', () => {
    expect(parseLabels('')).toEqual([])
  })

  it('only matches valid label chars (1..60)', () => {
    expect(parseLabels('#')).toEqual([])
    expect(parseLabels('#a')).toEqual(['a'])
  })

  it('does not treat plain words as labels', () => {
    expect(parseLabels('just some text no tags')).toEqual([])
  })

  it('handles labels adjacent to punctuation', () => {
    expect(parseLabels('do #work, then #home.')).toEqual(['work', 'home'])
  })

  it('ignores # not followed by valid label char', () => {
    expect(parseLabels('#! and # work')).toEqual([])
  })

  it('parses labels across multiple lines', () => {
    expect(parseLabels('line1 #a\nline2 #b')).toEqual(['a', 'b'])
  })

  it('skips multiple URLs and labels outside them', () => {
    expect(parseLabels('https://a.io/x#y https://b.io/z#w #keep')).toEqual(['keep'])
  })

  it('does not capture # inside a URL with trailing label', () => {
    expect(parseLabels('https://a.io#x#y #ok')).toEqual(['ok'])
  })

  it('preserves unicode and underscore/dash combos', () => {
    expect(parseLabels('#项目_1 #A-B #测试-2')).toEqual(['项目_1', 'A-B', '测试-2'])
  })
})