// from: shop@94584a5c:app/src/lib/sale.ts
// Flash-sale helpers. saleEndsAt is epoch ms (the feed mappers convert the trade's seconds once).
export type SaleInfo = {
  priceCredits: number
  compareAtCredits?: number
  saleEndsAt?: number // epoch ms
}

// On sale = there's a real discount (compare-at strictly above the current price) AND, if a window is
// set, it hasn't closed yet. A missing saleEndsAt means "no hard end" (still a valid sale), but a
// discount that's already expired must not render as live, so an explicit past end kills it.
export function isSaleActive(sale: SaleInfo, now: number = Date.now()): boolean {
  const { priceCredits, compareAtCredits, saleEndsAt } = sale
  if (compareAtCredits == null || !(compareAtCredits > priceCredits)) return false
  if (saleEndsAt != null && saleEndsAt <= now) return false
  return true
}

// Whole-percent discount for the "SALE -X%" badge. Clamped to 1..99 so we never render "-0%" (a
// rounding artifact of a sub-1% cut) or a nonsensical "-100%". Returns 0 when there's no valid sale.
export function saleDiscountPct(compareAtCredits: number, priceCredits: number): number {
  if (!(compareAtCredits > 0) || !(priceCredits >= 0) || priceCredits >= compareAtCredits) return 0
  const pct = Math.round((1 - priceCredits / compareAtCredits) * 100)
  return Math.min(99, Math.max(1, pct))
}

// Milliseconds left until the sale ends, floored at 0. Infinity for an open-ended sale (no window).
export function saleTimeLeft(saleEndsAt: number | undefined, now: number = Date.now()): number {
  if (saleEndsAt == null) return Infinity
  return Math.max(0, saleEndsAt - now)
}

/** The remaining window split into whole days, hours, minutes and seconds. */
export function countdownParts(
  msLeft: number
): { days: number; hours: number; minutes: number; seconds: number } | null {
  if (!isFinite(msLeft) || msLeft <= 0) return null
  const totalSec = Math.floor(msLeft / 1000)
  return {
    days: Math.floor(totalSec / 86400),
    hours: Math.floor((totalSec % 86400) / 3600),
    minutes: Math.floor((totalSec % 3600) / 60),
    seconds: totalSec % 60
  }
}

// Compact, urgency-forward countdown: "2d 4h" → "4h 12m" → "12m 30s" → "45s". Seconds only surface
// under an hour, where they actually create pressure. Returns "" at/after zero so callers hide it.
export function formatCountdown(msLeft: number): string {
  if (!isFinite(msLeft) || msLeft <= 0) return ""
  const totalSec = Math.floor(msLeft / 1000)
  const d = Math.floor(totalSec / 86400)
  const h = Math.floor((totalSec % 86400) / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  if (d > 0) return h > 0 ? `${d}d ${h}h` : `${d}d`
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

// How often a countdown needs to repaint to stay accurate without burning renders: every second in
// the final hour (where seconds show), every minute otherwise. Used by <SaleCountdown>.
export function countdownTickMs(msLeft: number): number {
  if (!isFinite(msLeft) || msLeft <= 0) return 0
  return msLeft < 3600_000 ? 1000 : 60_000
}

/**
 * How many units are worth telling the buyer about, the "only N left at this price" hint, or null.
 *
 * Only a small number is urgency; "only 847 left" is noise dressed as pressure, so anything above the
 * threshold says nothing at all. Zero and below say nothing either: a sale with nothing left is over, and
 * the listing itself is what should disappear, not shrink to "0 left".
 *
 * The server sends the count already resolved (the sale's remaining uses capped by the listing's stock),
 * so this is only the display rule.
 */
export const SALE_UNITS_HINT_MAX = 10

export function saleUnitsHint(saleUnitsLeft: number | undefined): number | null {
  if (saleUnitsLeft == null || !Number.isFinite(saleUnitsLeft)) return null
  if (saleUnitsLeft <= 0 || saleUnitsLeft > SALE_UNITS_HINT_MAX) return null
  return Math.floor(saleUnitsLeft)
}
