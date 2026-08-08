import { describe, it, expect } from 'vitest'
import { splitAtMidnight } from '@shared/time'
import fixture from './fixtures/segments-split.json'

// absolute ISO -> 本机本地墙钟 "YYYY-MM-DDTHH:MM"(分钟精度)。
// 夹具内所有时间均为 naive 墙钟串;两侧都把 impl 的 absolute 输出转回本机本地墙钟再比,
// 故任意机器时区(CI ubuntu UTC / dev windows UTC+8)都能一致命中。
function toWall(iso: string): string {
  const d = new Date(iso)
  const pad = (n: number): string => (n < 10 ? '0' + n : String(n))
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

describe('splitAtMidnight fixture parity', () => {
  // 两侧(Rust cargo test 的 split_at_midnight_fixture_parity 与本测试)读取同一份
  // tests/fixtures/segments-split.json,任一侧改动切分逻辑都必须同步该夹具。
  it.each(fixture)('$localDate', (c) => {
    const out = splitAtMidnight(
      {
        id: 's',
        startAt: c.seg.start,
        endAt: c.seg.end,
        processName: '',
        title: '',
        note: '',
        todoId: null,
        kind: 'activity'
      },
      c.localDate,
      c.now ? new Date(c.now) : new Date()
    )
    if (c.expected === null) {
      expect(out).toBeNull()
    } else {
      expect(out).not.toBeNull()
      expect(toWall(out!.startAt)).toBe(c.expected.start)
      expect(toWall(out!.endAt)).toBe(c.expected.end)
    }
  })
})
