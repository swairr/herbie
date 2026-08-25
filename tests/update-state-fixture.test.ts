import { describe, it, expect } from 'vitest'
import type { UpdateState, UpdateVersion } from '@shared/types'
import { UPDATE_SETTINGS_KEYS } from '@shared/types'
import fixture from './fixtures/update-state.json'

describe('update state fixture parity', () => {
  // 与 Rust side 的 `update::tests::serialized_ipc_shapes_match_shared_fixture` 读同一份
  // tests/fixtures/update-state.json:任一侧改动序列化形状或键名都必须同步该夹具,否则对拍报警。
  it.each(Object.entries(fixture.states))('serialized shape "%s" parses as UpdateState', (_tag, expected) => {
    const parsed = JSON.parse(expected) as UpdateState
    expect(typeof parsed.kind).toBe('string')
    // 逐字消费事件 payload:与 Rust `serde_json::to_string` 的输出保持一致。
    expect(JSON.stringify(parsed)).toBe(expected)
  })

  it('UpdateVersion shape has all renderer-consumed fields', () => {
    const parsed = JSON.parse(fixture.version) as UpdateVersion
    expect(typeof parsed.sha).toBe('string')
    expect(parsed.devBuild).toBe(false)
    expect(typeof parsed.lastCheck).toBe('string')
  })

  it('settings keys match TS constants (drift guard with Rust settings::KEY_*)', () => {
    expect([...UPDATE_SETTINGS_KEYS].sort()).toEqual([...fixture.settingsKeys].sort())
  })
})