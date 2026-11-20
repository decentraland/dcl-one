const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3_600_000],
  ['month', 30 * 24 * 3_600_000],
  ['week', 7 * 24 * 3_600_000],
  ['day', 24 * 3_600_000],
  ['hour', 3_600_000],
  ['minute', 60_000]
]

/** "3 weeks ago" / "hace 3 semanas" — localized largest-unit relative time. Sub-minute clamps to 1 minute. */
export function formatTimeAgo(timestamp: number, locale: string, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'always' })
  const elapsed = Math.max(0, now - timestamp)
  for (const [unit, ms] of UNITS) {
    if (elapsed >= ms) return rtf.format(-Math.round(elapsed / ms), unit)
  }
  return rtf.format(-1, 'minute')
}

/** "February 11, 2026" / "11 de febrero de 2026"; empty for a missing or unparsable date. */
export function formatLongDate(iso: string | null | undefined, locale: string): string {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  // A bare `YYYY-MM-DD` parses as UTC midnight, which is the previous day west of Greenwich.
  const timeZone = /^\d{4}-\d{2}-\d{2}$/.test(iso) ? 'UTC' : undefined
  return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'long', day: 'numeric', timeZone }).format(date)
}
