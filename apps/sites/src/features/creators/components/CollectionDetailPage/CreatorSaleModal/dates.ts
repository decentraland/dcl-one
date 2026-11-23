/** A moment as a date and a time in the viewer's locale, e.g. "Oct 19, 2026, 3:40 PM". */
export function formatDateTime(ms: number, locale?: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(ms))
}
