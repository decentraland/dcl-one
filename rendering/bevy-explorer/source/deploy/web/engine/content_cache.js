// The page side of the content cache's upkeep (crates/ipfs/src/web_cache.rs): the engine's workers
// ask through the page functions below, and the service worker, which owns the cache, does the work
// on its own thread. Defined on the app page (engine.js) and on the scene server's frame
// (headless.js): each is the page of its own engine's workers.

export const CONTENT_CACHE_TRIM = 'content-cache-trim';
const REPLY_TIMEOUT_MS = 30_000;

/**
 * Posts `message` to the controlling service worker and resolves with its reply, or null when
 * the page has no controller or no reply comes within `timeoutMs` (a worker without the listener).
 * @param {object} message
 * @param {ServiceWorkerContainer | undefined} container
 * @param {number} timeoutMs
 * @returns {Promise<object | null>}
 */
export function askServiceWorker(message, container = navigator.serviceWorker, timeoutMs = REPLY_TIMEOUT_MS) {
  const worker = container?.controller;
  if (!worker) return Promise.resolve(null);
  return new Promise((resolve) => {
    const channel = new MessageChannel();
    const timer = setTimeout(() => {
      channel.port1.close();
      resolve(null);
    }, timeoutMs);
    channel.port1.onmessage = ({ data }) => {
      clearTimeout(timer);
      channel.port1.close();
      resolve(data ?? null);
    };
    worker.postMessage(message, [channel.port2]);
  });
}

/** `__contentCacheTrim`: trim to `capBytes`, freeing at least a quarter after a quota error. */
export function contentCacheTrim(capBytes, quotaError, container, timeoutMs) {
  return askServiceWorker({ type: CONTENT_CACHE_TRIM, capBytes, quotaError: quotaError === true }, container, timeoutMs);
}

/**
 * Asks once for persistent storage, so the browser does not clear this origin under storage
 * pressure. The browser decides without prompting.
 * @param {StorageManager} storage
 * @returns {Promise<boolean>}
 */
export async function requestPersistentStorage(storage = navigator.storage) {
  if (await storage.persisted()) return true;
  return storage.persist();
}

/** Defines the page functions on `target` (a window). */
export function installContentCachePageFunctions(target = window) {
  target.__contentCacheTrim = (capBytes, quotaError) => contentCacheTrim(capBytes, quotaError);
}
