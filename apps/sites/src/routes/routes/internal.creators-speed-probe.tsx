const PROBE_BYTES = 512 * 1024;
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

/** The fixed download the creators item editor times to choose its preview renderer. */
export function loader() {
  return new Response(probeBody(), {
    headers: {
      "content-type": "application/octet-stream",
      "content-length": String(PROBE_BYTES),
      "cache-control": "no-store",
    },
  });
}
