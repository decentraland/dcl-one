import type { FunnelOs } from "./download-funnel";

// How /download_success hands the installer to the browser and when it calls the delivery done, after the legacy
// site's `streamOrFallback.ts`. macOS takes the native anchor, so the DMG keeps the download URL the launcher reads,
// then holds for an estimate of the transfer from the file's size. Windows takes the same anchor with the legacy
// fixed hold; the legacy streamed transfer (`delivery_mode: "streamed"`) needs a progress screen this page lacks.

export const FALLBACK_HOLD_MS = 4000;
export const MIN_HOLD_MS = 800;
export const MAX_HOLD_MS = 4000;
const ASSUMED_DOWNLINK_MBPS = 100;
const SAFETY_MARGIN_MS = 500;

export type Delivery = { mode: "anchor_native" | "anchor_fallback" };

/** The file name the URL ends in, "launcher" when it ends in none. */
export function installerFilename(url: string): string {
  let last = url.split("/").pop() ?? "";
  try {
    last = new URL(url).pathname.split("/").pop() ?? "";
  } catch {
    // Not absolute: the raw last segment.
  }
  return last.includes(".") ? last : "launcher";
}

/** Resolves after `ms`, or at once when the attempt is aborted. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) return resolve();
    const id = setTimeout(resolve, ms);
    signal.addEventListener("abort", () => (clearTimeout(id), resolve()), { once: true });
  });
}

async function contentLength(url: string, signal: AbortSignal): Promise<number | null> {
  try {
    const response = await fetch(url, { method: "HEAD", mode: "cors", credentials: "omit", cache: "no-store", signal });
    if (!response.ok) return null;
    const size = parseInt(response.headers.get("content-length") ?? "", 10);
    return Number.isFinite(size) && size > 0 ? size : null;
  } catch {
    return null;
  }
}

/** The hold for a file of the HEAD's size at an assumed broadband speed, clamped; the minimum when unknown. */
export async function estimateHoldMs(url: string, signal: AbortSignal): Promise<number> {
  const size = await contentLength(url, signal);
  if (size === null) return MIN_HOLD_MS;
  const transferMs = (size / ((ASSUMED_DOWNLINK_MBPS * 1024 * 1024) / 8)) * 1000;
  return Math.min(MAX_HOLD_MS, Math.max(MIN_HOLD_MS, transferMs + SAFETY_MARGIN_MS));
}

/**
 * Requests the installer through a hidden anchor and resolves when the hold for its delivery has passed. Aborting
 * `signal` cancels the HEAD request and the hold; the caller then reports no outcome, as the legacy page did.
 */
export async function deliverInstaller(url: string, os: FunnelOs, dispatch: (url: string) => void, signal: AbortSignal): Promise<Delivery> {
  const startedAt = Date.now();
  dispatch(url);
  if (os === "macOS") {
    const hold = await estimateHoldMs(url, signal);
    await sleep(Math.max(0, hold - (Date.now() - startedAt)), signal);
    return { mode: "anchor_native" };
  }
  await sleep(FALLBACK_HOLD_MS, signal);
  return { mode: "anchor_fallback" };
}
