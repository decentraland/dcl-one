import type { Cycle } from '/shared/types/creator-fund';

import type { DayRange } from '/@/lib/metricsApi';

const DAY_MS = 24 * 60 * 60 * 1000;

const utcDay = (time: number) => new Date(time).toISOString().slice(0, 10);

/**
 * Formats a cycle as `Oct 1 - Oct 15, 2026` in UTC days.
 * @param cycle - its `end_at` is exclusive, so the label ends the day before.
 * @param locale - a BCP 47 locale for the month and day names.
 */
export function formatCycleLabel(cycle: Pick<Cycle, 'start_at' | 'end_at'>, locale: string) {
  const start = new Date(cycle.start_at);
  const end = new Date(new Date(cycle.end_at).getTime() - DAY_MS);
  const day = new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric', timeZone: 'UTC' });
  const dayWithYear = new Intl.DateTimeFormat(locale, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    timeZone: 'UTC',
  });
  const sameYear = start.getUTCFullYear() === end.getUTCFullYear();
  const from = sameYear ? day.format(start) : dayWithYear.format(start);
  return `${from} - ${dayWithYear.format(end)}`;
}

/**
 * Formats a cycle as `Oct 1 - Oct 15`: the cycle label without its year.
 * @param cycle - its `end_at` is exclusive, so the label ends the day before.
 * @param locale - a BCP 47 locale for the month and day names.
 */
export function formatCycleDays(cycle: Pick<Cycle, 'start_at' | 'end_at'>, locale: string) {
  const day = new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric', timeZone: 'UTC' });
  const end = new Date(new Date(cycle.end_at).getTime() - DAY_MS);
  return `${day.format(new Date(cycle.start_at))} - ${day.format(end)}`;
}

/**
 * The UTC days a cycle's metrics are summed over: from the day `start_at` falls on through the day
 * before `end_at`, the same days `formatCycleLabel` names. The analytics API sums whole UTC days, so
 * a cycle bounded at 03:00 UTC is read as the half-month it is labelled with.
 */
export function cycleDayRange(cycle: Pick<Cycle, 'start_at' | 'end_at'>): DayRange {
  return {
    from: utcDay(new Date(cycle.start_at).getTime()),
    to: utcDay(new Date(cycle.end_at).getTime() - DAY_MS),
  };
}
