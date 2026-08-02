// Pure, dependency-free label parsing. Shared by main (persistence) and unit tests.
// §3 CONTEXT.md / requirements §3: labels come from `detail` text only; title is ignored.

const URL_RE = /\bhttps?:\/\/[^\s]+/g
const LABEL_RE = /#([\p{L}\p{N}_-]{1,60})/gu

function urlRanges(text: string): Array<[number, number]> {
  const ranges: Array<[number, number]> = []
  let m: RegExpExecArray | null
  URL_RE.lastIndex = 0
  while ((m = URL_RE.exec(text)) !== null) {
    ranges.push([m.index, m.index + m[0].length])
  }
  return ranges
}

function inRanges(idx: number, end: number, ranges: Array<[number, number]>): boolean {
  for (const [s, e] of ranges) {
    if (idx >= s && end <= e) return true
  }
  return false
}

export function parseLabels(detail: string): string[] {
  if (!detail) return []
  const ranges = urlRanges(detail)
  const seen = new Set<string>()
  const out: string[] = []
  let m: RegExpExecArray | null
  LABEL_RE.lastIndex = 0
  while ((m = LABEL_RE.exec(detail)) !== null) {
    if (inRanges(m.index, m.index + m[0].length, ranges)) continue
    const label = m[1]
    if (!seen.has(label)) {
      seen.add(label)
      out.push(label)
    }
  }
  return out
}