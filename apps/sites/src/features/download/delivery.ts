import type { DownloadOs } from "./params";

// Installer delivery as source Sites does it (decentraland/sites@cfdfa1a modules/streamOrFallback.ts
// and modules/file.ts, Apache-2.0; download contract §3). A transfer failure retries the SAME URL
// through a native anchor: it never switches to the CDN or drops the query (plan behavior 4b).

export type DeliveryMode = "streamed" | "anchor_fallback" | "anchor_native";

export type DeliveryResult = {
  deliveryMode: DeliveryMode;
  gatewayRequestId?: string;
  bytesTransferred?: number;
};

/** Windows fetch-error path: the HEAD estimate would fail for the same reason, so the hold is fixed. */
export const FALLBACK_HOLD_MS = 4000;
// navigator.connection.downlink is capped by Chrome and missing elsewhere; a static broadband
// assumption gives a better hold.
const ASSUMED_DOWNLINK_MBPS = 100;
const ESTIMATE_SAFETY_MARGIN_MS = 500;
export const ESTIMATE_MIN_HOLD_MS = 800;
export const ESTIMATE_MAX_HOLD_MS = 4000;

/** Resolves (never rejects) after `ms`, or early when `signal` aborts. */
function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal?.aborted) return resolve();
    const id = setTimeout(resolve, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(id);
        resolve();
      },
      { once: true },
    );
  });
}

function clickAnchor(href: string, downloadAttr: string | null): void {
  const a = document.createElement("a");
  a.href = href;
  if (downloadAttr !== null) a.setAttribute("download", downloadAttr);
  document.body.appendChild(a);
  a.click();
  // Removing the anchor synchronously after click() aborts the download in some Chromium versions.
  requestAnimationFrame(() => a.remove());
}

/**
 * Chromium ignores `download` on cross-origin URLs without Content-Disposition, and setting it has
 * dropped some .dmg downloads, so the attribute is only set for same-origin URLs.
 */
export function triggerFileDownload(link: string, filename = ""): void {
  let downloadAttr: string | null = null;
  try {
    if (new URL(link, window.location.href).origin === window.location.origin)
      downloadAttr = filename;
  } catch {
    // Invalid URL: the browser handles it.
  }
  clickAnchor(link, downloadAttr);
}

function triggerBlobDownload(blob: Blob, filename: string): void {
  const blobUrl = URL.createObjectURL(blob);
  clickAnchor(blobUrl, filename);
  requestAnimationFrame(() => URL.revokeObjectURL(blobUrl));
}

async function streamDownload(
  url: string,
  filename: string,
  onProgress: (loaded: number, total: number) => void,
  signal: AbortSignal,
): Promise<{ gatewayRequestId?: string }> {
  const response = await fetch(url, {
    method: "GET",
    mode: "cors",
    credentials: "omit",
    cache: "no-store",
    redirect: "follow",
    signal,
  });
  if (!response.ok)
    throw new Error(`Download failed with status ${response.status}`);
  if (!response.body) throw new Error("Response body is null");

  // Exposed by the gateway so download_success joins its server-side telemetry; the CDN has none.
  const gatewayRequestId = response.headers.get("x-request-id") ?? undefined;
  const total =
    Number.parseInt(response.headers.get("content-length") ?? "", 10) || 0;
  let loaded = 0;
  const progress = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      loaded += chunk.byteLength;
      onProgress(loaded, total);
      controller.enqueue(chunk);
    },
  });
  const blob = await new Response(response.body.pipeThrough(progress)).blob();
  triggerBlobDownload(blob, filename);
  return { gatewayRequestId };
}

async function fetchContentLength(
  url: string,
  signal?: AbortSignal,
): Promise<number | null> {
  try {
    const response = await fetch(url, {
      method: "HEAD",
      mode: "cors",
      credentials: "omit",
      cache: "no-store",
      signal,
    });
    if (!response.ok) return null;
    const size = Number.parseInt(
      response.headers.get("content-length") ?? "",
      10,
    );
    return Number.isFinite(size) && size > 0 ? size : null;
  } catch {
    return null;
  }
}

/** How long the "Downloading…" state stays up for a native anchor download, from the HEAD size. */
export async function estimateHoldMs(
  url: string,
  signal?: AbortSignal,
): Promise<number> {
  const size = await fetchContentLength(url, signal);
  if (size === null) return ESTIMATE_MIN_HOLD_MS;
  const transferMs =
    (size / ((ASSUMED_DOWNLINK_MBPS * 1024 * 1024) / 8)) * 1000;
  return Math.min(
    ESTIMATE_MAX_HOLD_MS,
    Math.max(ESTIMATE_MIN_HOLD_MS, transferMs + ESTIMATE_SAFETY_MARGIN_MS),
  );
}

/**
 * Starts the installer download and resolves once it has effectively landed.
 * - macOS: native anchor. The launcher reads `anon_user_id` from the DMG's kMDItemWhereFroms xattr,
 *   which a blob: URL would replace with the page origin.
 * - Windows: fetch + Blob, so progress is real (attribution is baked into the EXE). When the stream
 *   fails (network, CORS on preview hosts) the same URL goes through a native anchor.
 */
export async function deliverInstaller(args: {
  url: string;
  filename: string;
  os: DownloadOs;
  signal: AbortSignal;
  onProgress?: (percent: number) => void;
}): Promise<DeliveryResult> {
  const { url, filename, os, signal } = args;
  if (os === "macos") {
    const startedAt = Date.now();
    triggerFileDownload(url, filename);
    const holdMs = await estimateHoldMs(url, signal);
    if (!signal.aborted)
      await sleep(Math.max(0, holdMs - (Date.now() - startedAt)), signal);
    return { deliveryMode: "anchor_native" };
  }

  let bytesTransferred = 0;
  try {
    const { gatewayRequestId } = await streamDownload(
      url,
      filename,
      (loaded, total) => {
        bytesTransferred = loaded;
        // Capped at 99 while the blob is handed to the anchor.
        if (!signal.aborted && total > 0)
          args.onProgress?.(Math.min(99, Math.floor((loaded / total) * 100)));
      },
      signal,
    );
    return { deliveryMode: "streamed", gatewayRequestId, bytesTransferred };
  } catch (error) {
    if (signal.aborted) throw error;
    triggerFileDownload(url, filename);
    await sleep(FALLBACK_HOLD_MS, signal);
    return { deliveryMode: "anchor_fallback" };
  }
}
