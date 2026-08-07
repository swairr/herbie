import { describe, it, expect } from 'vitest'
import { parseLabels } from '@shared/labels'
import fixture from './fixtures/labels.json'

describe('parseLabels fixture parity', () => {
  // 两侧(Rust cargo test 的 labels::fixture_parity_with_ts 与本测试)读取同一份
  // tests/fixtures/labels.json,任一侧改动解析逻辑都必须同步该夹具,否则互漂报警。
  it.each(fixture)('input $input -> $expected', (c) => {
    expect(parseLabels(c.input)).toEqual(c.expected)
  })
})