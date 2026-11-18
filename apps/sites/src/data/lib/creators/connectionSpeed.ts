// Measures the link with one fixed, incompressible download. Resource timings are no substitute: on a
// warm cache every bundle is a 304 (300 bytes on the wire) or a cache hit (0 bytes), so nothing is measurable,
// and the CDN sends no Timing-Allow-Origin, so the probe's own entry has no response timings cross-origin.
export const PROBE_URL = `${import.meta.env.BASE_URL}speed-probe.bin`
// A probe still running after this long is on a slow link whatever the final number would have been.
export const PROBE_TIMEOUT_MS = 2_000
// Below this the transfer is latency, not throughput.
const MIN_PROBE_BYTES = 30_000

/**
 * Downlink in Mbps, timed over the second half of the body only: the first half absorbs the round trip,
 * the time to first byte and most of TCP slow start, which otherwise read a fast high-latency link as slow.
 * `0` when the probe timed out (that slow is slow); `null` when it could not run (offline flap, ad blocker,
 * a bad deploy), which stays optimistic because none of those says anything about the link.
 */
export async function measureDownlinkMbps(): Promise<number | null> {
  try {
    const response = await fetch(PROBE_URL, { cache: 'no-store', signal: AbortSignal.timeout(PROBE_TIMEOUT_MS) })
    if (!response.ok || !response.body) {
      await response.body?.cancel()
      return null
    }
    const total = Number(response.headers.get('content-length')) || 0
    const reader = response.body.getReader()
    let received = 0
    let halfAt: number | null = null
    let bytesAfterHalf = 0
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      received += value.byteLength
      if (halfAt === null) {
        if (received >= total / 2) halfAt = performance.now()
      } else {
        bytesAfterHalf += value.byteLength
      }
    }
    if (received < MIN_PROBE_BYTES || halfAt === null || bytesAfterHalf === 0) return null
    // A single read delivering the whole rest clamps to 1 ms; that reads "fast", which it is.
    const seconds = Math.max(performance.now() - halfAt, 1) / 1000
    return (bytesAfterHalf * 8) / 1e6 / seconds
  } catch (error) {
    return error instanceof DOMException && error.name === 'TimeoutError' ? 0 : null
  }
}
