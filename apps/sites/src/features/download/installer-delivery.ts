import type { FunnelOs } from "./download-funnel";

// How /download_success hands the installer to the browser and when it calls the delivery done, after the legacy
// site's `streamOrFallback.ts` and `file.ts`. macOS takes the native anchor, so the DMG keeps the download URL the
// launcher reads, then holds for an estimate of the transfer from the file's size. Windows streams the installer
// through fetch so the progress screen follows the real transfer (the gateway bakes the attribution into the EXE, so
// the blob's origin does not matter), and falls back to the native anchor with a fixed hold when the stream fails.

export const FALLBACK_HOLD_MS = 4000;
export const MIN_HOLD_MS = 800;
export const MAX_HOLD_MS = 4000;
const ASSUMED_DOWNLINK_MBPS = 100;
const SAFETY_MARGIN_MS = 500;

export type Delivery = { mode: "streamed" | "anchor_native" | "anchor_fallback"; bytes?: number; requestId?: string };

/** Hands a URL to the browser as a download; `filename` names a same-origin (blob) download. */
export type Dispatch = (url: string, filename?: string) => void;

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
 * Streams the installer, reporting each chunk, then saves the assembled bytes through a blob link. Throws on a
 * network error or a non-2xx answer. The gateway exposes `x-request-id` for the server-side join.
 */
async function streamInstaller(
  url: string,
  filename: string,
  signal: AbortSignal,
  dispatch: Dispatch,
  onChunk: (loaded: number, total: number) => void,
): Promise<{ requestId?: string }> {
  const response = await fetch(url, { method: "GET", mode: "cors", credentials: "omit", cache: "no-store", redirect: "follow", signal });
  if (!response.ok) throw new Error(`Download failed with status ${response.status}`);
  if (!response.body) throw new Error("Response body is null");
  const requestId = response.headers.get("x-request-id") ?? undefined;
  const total = parseInt(response.headers.get("content-length") ?? "", 10) || 0;
  let loaded = 0;
  const progress = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      loaded += chunk.byteLength;
      onChunk(loaded, total);
      controller.enqueue(chunk);
    },
  });
  const blob = await new Response(response.body.pipeThrough(progress)).blob();
  const blobUrl = URL.createObjectURL(blob);
  dispatch(blobUrl, filename);
  // The browser holds its own reference once the download is dispatched; revoking on the next frame frees memory.
  requestAnimationFrame(() => URL.revokeObjectURL(blobUrl));
  return { requestId };
}

/**
 * Hands the installer to the browser and resolves when its delivery is done (Windows) or the hold has passed (macOS,
 * and the Windows fallback). `onProgress` gets 0 to 99 while a stream with a known size runs. Aborting `signal`
 * cancels the request and the hold; the caller then reports no outcome, as the legacy page did.
 */
export async function deliverInstaller(
  url: string,
  os: FunnelOs,
  dispatch: Dispatch,
  signal: AbortSignal,
  { filename, onProgress }: { filename: string; onProgress: (percent: number) => void },
): Promise<Delivery> {
  if (os === "macOS") {
    const startedAt = Date.now();
    dispatch(url);
    const hold = await estimateHoldMs(url, signal);
    await sleep(Math.max(0, hold - (Date.now() - startedAt)), signal);
    return { mode: "anchor_native" };
  }
  let bytes = 0;
  try {
    const { requestId } = await streamInstaller(url, filename, signal, dispatch, (loaded, total) => {
      bytes = loaded;
      if (!signal.aborted && total > 0) onProgress(Math.min(99, Math.floor((loaded / total) * 100)));
    });
    return { mode: "streamed", bytes, ...(requestId ? { requestId } : {}) };
  } catch (error) {
    if (signal.aborted) throw error;
    dispatch(url);
    await sleep(FALLBACK_HOLD_MS, signal);
    return { mode: "anchor_fallback" };
  }
}
