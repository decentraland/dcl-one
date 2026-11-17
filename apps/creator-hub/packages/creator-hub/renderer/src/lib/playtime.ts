const MINUTES_PER_HOUR = 60;

/** A total playtime in the unit it reads best in. */
export type PlaytimeAmount = { unit: 'minutes' | 'hours'; value: number };

/**
 * Whole minutes up to an hour, then hours to one decimal.
 *
 * Rounds to the minute before picking the unit, so 60.4 minutes still reads as 60 minutes and
 * 61 minutes as 1 hour.
 */
export function playtimeAmount(minutes: number): PlaytimeAmount {
  const whole = Math.round(minutes);
  if (whole <= MINUTES_PER_HOUR) return { unit: 'minutes', value: whole };
  return { unit: 'hours', value: Math.round((whole / MINUTES_PER_HOUR) * 10) / 10 };
}
