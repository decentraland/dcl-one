// Signed requests (ADR-44) carry the device's clock as their timestamp, and the servers reject one more than
// 60s off their own clock, in either direction. A device clock that is off therefore has every signed request
// refused: builder-server answers 401 "Unauthenticated", credits-server "Expired signature" (or "too far in the
// future" when the clock is ahead). This module measures how far the device
// clock is from the servers' and lets signing correct for it, so a wrong device clock no longer locks the
// creator out.
import { config } from '~/config'

/**
 * Drift below this is left alone: it fits inside the servers' 60s window with room for latency, and a
 * correction measured over a slow network would be noisier than the drift itself.
 */
const MAX_UNCORRECTED_DRIFT_MS = 30 * 1000

/**
 * A measured offset beyond this is not trusted: a clock that far off is not a drifting device, so the
 * reading is more likely wrong, and applying it would get every later signed request refused.
 */
const MAX_CORRECTION_MS = 7 * 24 * 60 * 60 * 1000

/**
 * How long a measurement (or a failed attempt) stands. builder-server also answers 401 for reasons that
 * aren't the clock (a publish polls it while the subgraph catches up), so refusals inside this window
 * reuse the last measurement instead of reading the server time again.
 */
const MEASUREMENT_TTL_MS = 60 * 1000

let offsetMs = 0
let measuredAt: number | null = null
let pending: Promise<void> | null = null

/** The correction currently applied to signing timestamps, in ms. */
export function serverClockOffset(): number {
  return offsetMs
}

/** The time to sign with: the device clock, corrected by the measured offset. */
export function serverNow(): number {
  return Date.now() + offsetMs
}

/** Reads credits-server's public `/status` (`currentTime`, CORS-enabled) and returns the offset it implies. */
async function measureOffset(): Promise<number | null> {
  const sentAt = Date.now()
  const response = await fetch(`${config.get('CREDITS_SERVER_URL')}/status`)
  const receivedAt = Date.now()
  const body = (await response.json().catch(() => null)) as { currentTime?: unknown } | null
  if (!response.ok || typeof body?.currentTime !== 'number' || !Number.isFinite(body.currentTime)) return null
  // The server read its clock somewhere during the round trip: its midpoint is the best estimate.
  return body.currentTime - (sentAt + receivedAt) / 2
}

/**
 * Brings the correction up to date after a signed request was refused. Fails closed: when the server time
 * can't be read, or reads implausibly, the correction stays as it was. Concurrent refusals (the page's
 * queries fail together) share one measurement, and a recent measurement is reused.
 * Compare `serverClockOffset()` before and after to know whether re-signing can now succeed.
 */
export function resyncServerClock(): Promise<void> {
  if (measuredAt !== null && Date.now() - measuredAt < MEASUREMENT_TTL_MS) return Promise.resolve()
  pending ??= (async () => {
    try {
      const measured = await measureOffset()
      if (measured === null || Math.abs(measured) > MAX_CORRECTION_MS) return
      offsetMs = Math.abs(measured) > MAX_UNCORRECTED_DRIFT_MS ? Math.round(measured) : 0
    } catch {
      // The server time couldn't be read: keep signing as before.
    } finally {
      // A failed read starts the window too, so an unreachable /status isn't asked again on every 401.
      measuredAt = Date.now()
      pending = null
    }
  })()
  return pending
}

/** Test seam: forget the measured offset. */
export function resetServerClock(): void {
  offsetMs = 0
  measuredAt = null
  pending = null
}
