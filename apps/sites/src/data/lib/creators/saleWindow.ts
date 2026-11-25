// When a discount runs, as the Shop's discount modal decides it: a length from now, or two calendar days.

const HOUR_MS = 60 * 60 * 1000

/** A length from now (a preset), or two calendar days picked on the calendar. */
export type SaleWhen<Key extends string = string> =
  { kind: 'preset'; key: Key; hours: number } | { kind: 'range'; from: number; to: number }

/** The local midnight that starts `ms`'s day. */
export function startOfDay(ms: number): number {
  const d = new Date(ms)
  d.setHours(0, 0, 0, 0)
  return d.getTime()
}

/** The last millisecond of `ms`'s local day. */
export function endOfDay(ms: number): number {
  const d = new Date(ms)
  d.setHours(23, 59, 59, 999)
  return d.getTime()
}

/**
 * The window the signed terms carry. A preset runs from now; a range starting today starts now, not at a midnight
 * already gone, one starting later starts at its first day's midnight, and every range runs to the end of its last day.
 */
export function windowOf(when: SaleWhen, now: number): { startsAtMs: number | undefined; endsAtMs: number } {
  if (when.kind === 'preset') return { startsAtMs: undefined, endsAtMs: now + when.hours * HOUR_MS }
  const start = startOfDay(when.from)
  return { startsAtMs: start > now ? start : undefined, endsAtMs: endOfDay(when.to) }
}
