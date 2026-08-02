// Pure time formatting helpers. Storage uses ISO 8601; display uses local short form.

export function nowIso(): string {
  return new Date().toISOString()
}

function pad(n: number): string {
  return n < 10 ? '0' + n : String(n)
}

// Format an ISO timestamp as local "YYYY-MM-DD HH:mm".
export function formatLocalShort(iso: string | null): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  const y = d.getFullYear()
  const mo = pad(d.getMonth() + 1)
  const da = pad(d.getDate())
  const h = pad(d.getHours())
  const mi = pad(d.getMinutes())
  return `${y}-${mo}-${da} ${h}:${mi}`
}