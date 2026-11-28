import { PROBE_BYTES } from "@web-core/connection-speed";

// getRandomValues fills at most 64 KiB per call.
const CHUNK = 64 * 1024;

let probe: Uint8Array<ArrayBuffer> | undefined;

// Random bytes, so no compression along the way shortens the download being timed.
function probeBody(): Uint8Array<ArrayBuffer> {
  if (!probe) {
    probe = new Uint8Array(PROBE_BYTES);
    for (let at = 0; at < PROBE_BYTES; at += CHUNK) crypto.getRandomValues(probe.subarray(at, at + CHUNK));
  }
  return probe;
}

/**
 * The fixed download timed to choose a 3D preview renderer, by the creators item editor and by the Shop's previews (item
 * detail, fitting room, outfits, the home promo). Both read it through `PROBE_URL` in library/web-core/connection-speed.ts:
 * renaming this route means changing that constant with it.
 *
 * `no-transform` keeps a proxy from re-encoding it: Cloudflare otherwise compresses it and streams it chunked, which
 * drops the Content-Length the timing halves by (the reader then falls back to `PROBE_BYTES`).
 */
export function loader() {
  return new Response(probeBody(), {
    headers: {
      "content-type": "application/octet-stream",
      "content-length": String(PROBE_BYTES),
      "cache-control": "no-store, no-transform",
    },
  });
}
