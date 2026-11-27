// Measures the link with one fixed, incompressible download, for the hosts that choose a 3D preview renderer by it
// (the creators item editor in Sites, the Shop's previews). Resource timings are no substitute: on a warm cache every
// bundle is a 304 (300 bytes on the wire) or a cache hit (0 bytes), and the small chunks a page loads are latency,
// not throughput, so a fast link reads slow. The CDN sends no Timing-Allow-Origin, so the probe's own entry has no
// response timings cross-origin either.

/** Sites serves it (routes/internal.creators-speed-probe): `PROBE_BYTES` of random bytes, never cached. */
export const PROBE_URL = "/internal/creators-speed-probe";
/** The probe's size, which the route serves and the timing halves by. */
export const PROBE_BYTES = 512 * 1024;
// A probe still running after this long is on a slow link whatever the final number would have been.
export const PROBE_TIMEOUT_MS = 2_000;
// Below this the transfer is latency, not throughput.
const MIN_PROBE_BYTES = 30_000;

/**
 * Downlink in Mbps, timed over the second half of the body only: the first half absorbs the round trip,
 * the time to first byte and most of TCP slow start, which otherwise read a fast high-latency link as slow.
 * `0` when the probe timed out (that slow is slow); `null` when it could not run (offline flap, ad blocker,
 * a bad deploy), which stays optimistic because none of those says anything about the link.
 */
export async function measureDownlinkMbps(): Promise<number | null> {
  try {
    const response = await fetch(PROBE_URL, { cache: "no-store", signal: AbortSignal.timeout(PROBE_TIMEOUT_MS) });
    if (!response.ok || !response.body) {
      await response.body?.cancel();
      return null;
    }
    // Halved by the probe's fixed size, never by the response's Content-Length: a proxy that streams the body chunked
    // (Cloudflare does) drops it, and a compressed response states its encoded length, not the bytes read here.
    // Without the size the "second half" would start at the first chunk and time the slow start it exists to leave out.
    const total = PROBE_BYTES;
    const reader = response.body.getReader();
    let received = 0;
    let halfAt: number | null = null;
    let bytesAfterHalf = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      received += value.byteLength;
      if (halfAt === null) {
        if (received >= total / 2) halfAt = performance.now();
      } else {
        bytesAfterHalf += value.byteLength;
      }
    }
    if (received < MIN_PROBE_BYTES || halfAt === null || bytesAfterHalf === 0) return null;
    // A single read delivering the whole rest clamps to 1 ms; that reads "fast", which it is.
    const seconds = Math.max(performance.now() - halfAt, 1) / 1000;
    return (bytesAfterHalf * 8) / 1e6 / seconds;
  } catch (error) {
    return error instanceof DOMException && error.name === "TimeoutError" ? 0 : null;
  }
}
