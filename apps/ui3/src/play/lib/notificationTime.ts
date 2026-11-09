/** Epoch ms of a notification timestamp. The service sends a string of epoch ms ("1700000000123");
 *  an ISO date is accepted too. */
export function notificationTimeMs(ts: string): number | undefined {
  const n = /^\d+$/.test(ts) ? Number(ts) : Date.parse(ts)
  return Number.isFinite(n) ? n : undefined
}
