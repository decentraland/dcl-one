// MUST stay in sync with crates/ipfs/src/web_cache.rs (cache name, both keys, the cached header):
// this worker only answers from the cache; the engine fills it (content it has checked against
// its hash under the shared key, a url's response under the url), and its asset processor writes
// processed bytes back over the entry it read.
// v2: v1 entries were shared across content servers on the path alone, with nothing checked.
const CACHE_NAME = 'ipfs-path-cache-v2';
const CUSTOM_HEADER = 'X-IPFS';
// Stand-in origin for entries shared across content servers. `.invalid` is reserved, so no real
// server's own entries can land on these keys.
const SHARED_ORIGIN = 'https://shared.invalid';
// Set on responses answered from the cache.
const CACHED_HEADER = 'X-IPFS-Cached';

// The scene editor's preview realm (PREVIEW_REALM.md): urls under <scope>preview/ are answered
// from this cache alone. Scenes can't write it: the sandbox worker deletes `caches`.
const PREVIEW_CACHE_NAME = 'dcl-editor-preview-v1';
const PREVIEW_ROOT = new URL('preview/', self.registration.scope).href;
// The editor package's scene, staged by the page from checked bytes (preview_realm.js).
const EDITOR_SCENE_CACHE_NAME = 'dcl-editor-scene-v1';
const EDITOR_SCENE_ROOT = new URL('editor-scene/', self.registration.scope).href;
// A preview realm's storage, for its scene server and the page (preview_realm.js). Never synced anywhere.
const PREVIEW_STORAGE_CACHE_NAME = 'dcl-editor-storage-v1';
let previewRealm = null;
try {
    importScripts('preview_realm.js');
    previewRealm = self.dclPreviewRealm;
} catch (e) {
    // keep the worker installable: the page needs its COEP rewrite and X-IPFS cache regardless
    console.warn('[IPFS Cache Service Worker]: preview realm unavailable', e);
}

self.addEventListener('install', (event) => {
    // Force the waiting service worker to become the active service worker.
    self.skipWaiting();
});

self.addEventListener('activate', (event) => {
    console.log('[IPFS Cache Service Worker]: Active');
    
    // An array of cache names that are "allowed" to exist.
    const cacheWhitelist = [CACHE_NAME, PREVIEW_CACHE_NAME, EDITOR_SCENE_CACHE_NAME, PREVIEW_STORAGE_CACHE_NAME];

    event.waitUntil(
        // Get all the cache keys (names) that exist.
        caches.keys().then(cacheNames => {
            return Promise.all(
                cacheNames.map(cacheName => {
                    // If a cache name is NOT in our whitelist...
                    if (cacheWhitelist.indexOf(cacheName) === -1) {
                        console.log(`SW: Deleting old cache: ${cacheName}`);
                        // ...delete it.
                        return caches.delete(cacheName);
                    }
                })
            );
        }).then(() => {
            // After cleanup is done, claim the clients.
            return self.clients.claim();
        })
    );
});

self.addEventListener('fetch', (event) => {
    const request = event.request;

    if (request.url.startsWith(PREVIEW_ROOT)) {
        event.respondWith(servePreviewRealm(request, event.clientId));
        return;
    }
    if (request.url.startsWith(EDITOR_SCENE_ROOT)) {
        event.respondWith(serveEditorScene(request, event.clientId));
        return;
    }

    // Check if the request has our custom header.
    if (request.headers.has(CUSTOM_HEADER)) {
        // If it does, use our caching strategy.
        event.respondWith(cacheFirstStrategy(request));
        return;
    }

    // Same-origin page loads: isolate the app, and keep the shell that embeds it unisolated
    if (request.mode === 'navigate' || request.destination === 'document') {
        event.respondWith(addCrossOriginIsolationHeaders(request));
    }
});

// The document or worker that sent a request, as preview_realm.js tells scenes' sandboxes apart.
async function requestClient(clientId) {
    const client = clientId ? await self.clients.get(clientId) : undefined;
    return client ? { url: client.url, type: client.type } : undefined;
}

async function serveEditorScene(request, clientId) {
    if (!previewRealm) return unavailable();
    const [cache, client] = await Promise.all([caches.open(EDITOR_SCENE_CACHE_NAME), requestClient(clientId)]);
    return previewRealm.handleEditorScene(request, EDITOR_SCENE_ROOT, cache, client);
}

function unavailable() {
    return new Response(null, {
        status: 503,
        headers: {
            'Content-Type': 'application/octet-stream',
            'X-Content-Type-Options': 'nosniff',
            'Content-Security-Policy': 'sandbox',
            'Cross-Origin-Resource-Policy': 'same-origin',
            'Cache-Control': 'no-store',
        },
    });
}

async function servePreviewRealm(request, clientId) {
    if (!previewRealm) return unavailable();
    const [cache, storage, client] = await Promise.all([
        caches.open(PREVIEW_CACHE_NAME),
        caches.open(PREVIEW_STORAGE_CACHE_NAME),
        requestClient(clientId),
    ]);
    return previewRealm.handle(request, PREVIEW_ROOT, cache, storage, client);
}

// The content cache's upkeep: the engine asks with a `content-cache-trim` message
// (engine/content_cache.js) after it has written enough, when its cap changes, and after a write
// failed for quota. The trim runs here, off the engine's threads, and only ever opens CACHE_NAME.
const CONTENT_CACHE_TRIM = 'content-cache-trim';
// Half the origin's quota stays for its other storage (OPFS, IndexedDB).
const CONTENT_CACHE_QUOTA_SHARE = 0.5;
// A trim goes down to this share of the cap, so the next one waits for real growth.
const CONTENT_CACHE_LOW_WATER = 0.9;
// After a quota error, keep at most this share of the cache, even under the cap.
const CONTENT_CACHE_QUOTA_ERROR_KEEP = 0.75;
// Entries sized (one `match` each) per step while choosing what to evict.
const CONTENT_CACHE_SIZE_BATCH = 64;
let contentCacheQueue = Promise.resolve();

self.addEventListener('message', (event) => {
    const data = event.data;
    if (data?.type !== CONTENT_CACHE_TRIM) return;
    const port = event.ports?.[0];
    const task = () => trimContentCache(data.capBytes, data.quotaError === true);
    // one trim at a time: a second request sees what the first left
    const run = contentCacheQueue.then(task, task);
    contentCacheQueue = run.catch(() => {});
    event.waitUntil(run.then(
        (report) => port?.postMessage(report),
        (e) => port?.postMessage({ error: String(e?.message ?? e) }),
    ));
});

// How much to free. `cachesUsage` is the origin's Cache Storage usage; `quota` bounds the cap.
function contentCacheTarget({ capBytes, quota, cachesUsage, quotaError }) {
    const cap = Math.max(0, Math.min(capBytes, Math.floor(quota * CONTENT_CACHE_QUOTA_SHARE)));
    let bytesToFree = cachesUsage > cap ? cachesUsage - Math.floor(cap * CONTENT_CACHE_LOW_WATER) : 0;
    if (quotaError) {
        const keep = Math.floor(Math.min(cap, cachesUsage) * CONTENT_CACHE_QUOTA_ERROR_KEEP);
        bytesToFree = Math.max(bytesToFree, cachesUsage - keep);
    }
    return { cap, bytesToFree };
}

// The leading entries of `sized` ({ key, size }, eviction order) that free `bytesToFree`.
function pickEvictions(sized, bytesToFree) {
    const picked = [];
    let freed = 0;
    for (const { key, size } of sized) {
        if (freed >= bytesToFree) break;
        picked.push(key);
        freed += size;
    }
    return { picked, freed };
}

// The engine stores every entry with Content-Length.
async function contentCacheEntrySize(response) {
    const length = Number(response.headers.get('Content-Length'));
    if (response.headers.has('Content-Length') && Number.isFinite(length)) return length;
    return (await response.blob()).size;
}

async function trimContentCache(capBytes, quotaError) {
    if (!(typeof capBytes === 'number' && capBytes >= 0)) throw new Error('capBytes must be a non-negative number');
    const { usage = 0, quota = Infinity, usageDetails } = await navigator.storage.estimate();
    // usageDetails leaves out a storage system with no usage
    const cachesUsage = usageDetails ? (usageDetails.caches ?? 0) : usage;
    // only for the report: a browser without it, or one that refuses, must not stop the trim
    const persisted = (await navigator.storage.persisted?.().catch(() => false)) ?? false;
    const { cap, bytesToFree } = contentCacheTarget({ capBytes, quota, cachesUsage, quotaError });
    const report = { capBytes: cap, usage, quota, cachesUsage, evictedEntries: 0, evictedBytes: 0, persisted };
    if (bytesToFree <= 0) return report;

    const cache = await caches.open(CACHE_NAME);
    // insertion order: oldest written first (a put over a key moves it to the end)
    const requests = await cache.keys();
    for (let i = 0; i < requests.length && report.evictedBytes < bytesToFree; i += CONTENT_CACHE_SIZE_BATCH) {
        const sized = await Promise.all(requests.slice(i, i + CONTENT_CACHE_SIZE_BATCH).map(async (key) => {
            const response = await cache.match(key);
            return { key, size: response ? await contentCacheEntrySize(response) : 0 };
        }));
        const { picked, freed } = pickEvictions(sized, bytesToFree - report.evictedBytes);
        await Promise.all(picked.map((key) => cache.delete(key)));
        report.evictedEntries += picked.length;
        report.evictedBytes += freed;
    }
    return report;
}

/**
 * Fetches a page and sets its isolation headers. The app (app.html: HUD + engine) gets
 * Document-Isolation-Policy, which makes it cross-origin isolated (SharedArrayBuffer) in a process
 * of its own. The shell that embeds it (the scope's index page) loses the host's COEP: an isolated
 * top-level page would take the app's process back in with it. Any other page (another host's
 * engine page, say) gets COOP/COEP credentialless to enable SharedArrayBuffer.
 */
async function addCrossOriginIsolationHeaders(request) {
    const response = await fetch(request);

    // Only modify same-origin responses
    if (response.type === 'basic') {
        const newHeaders = new Headers(response.headers);
        const path = new URL(request.url).pathname;
        const scope = new URL(self.registration.scope).pathname;
        newHeaders.set('Cross-Origin-Opener-Policy', 'same-origin');
        // the scene server's frame (engine/headless.js) takes the app's policy: the app scripts it,
        // which only documents with the same policy may do
        if (path === scope + 'app.html' || path === scope + 'engine/headless.html') {
            newHeaders.delete('Cross-Origin-Embedder-Policy');
            newHeaders.set('Document-Isolation-Policy', 'isolate-and-credentialless');
        } else if (path === scope || path === scope + 'index.html') {
            newHeaders.delete('Cross-Origin-Embedder-Policy');
        } else {
            newHeaders.set('Cross-Origin-Embedder-Policy', 'credentialless');
        }

        return new Response(response.body, {
            status: response.status,
            statusText: response.statusText,
            headers: newHeaders
        });
    }

    return response;
}

async function cacheFirstStrategy(request) {
    // DEV: never cache LOCAL preview content. sdk-commands' `start` server hashes mutable files by a
    // CONSTANT id (the file path), so cache-first would serve the first build of the local bridge scene
    // forever — every rebuild would be invisible. Always go to network for localhost so scene edits show.
    const reqUrl = new URL(request.url);
    if (reqUrl.hostname === 'localhost' || reqUrl.hostname === '127.0.0.1') {
        return fetch(stripCustomHeader(request));
    }

    // Cache entries are keyed by url alone, so only a GET may be answered from one.
    if (request.method !== 'GET') {
        return fetch(stripCustomHeader(request));
    }

    // Two keys. The shared one is the hash alone (the last path segment), so every content server
    // serving a hash hits the same entry. This worker writes neither: any client (a scene
    // included) can make it fetch, so what it fetched would have to stay under the url it came
    // from, and the engine already stores what it downloads, under the hash once it has checked
    // the bytes against it. Storing here would also hold the response until the whole body was
    // in, which on a slow link makes a large file look like a request that never sent headers.
    const sharedKey = SHARED_ORIGIN + '/' + reqUrl.pathname.split('/').pop();
    const originKey = reqUrl.origin + reqUrl.pathname + reqUrl.search;

    //Open the cache
    const cache = await caches.open(CACHE_NAME);

    //Try to find a response in the cache
    const sharedResponse = await cache.match(sharedKey);
    if (sharedResponse) {
        // an origin's own copy is dead weight once the shared entry exists (one can be left behind
        // when the engine never got to check it). Not awaited: nothing depends on it.
        cache.delete(originKey).catch(() => {});
    }
    const cachedResponse = sharedResponse ?? (await cache.match(originKey));
    if (cachedResponse) {
        const headers = new Headers(cachedResponse.headers);
        headers.set(CACHED_HEADER, '1');
        return new Response(cachedResponse.body, {
            status: cachedResponse.status,
            statusText: cachedResponse.statusText,
            headers
        });
    }

    //If not in cache, fetch from network
    return fetch(stripCustomHeader(request));
}

function stripCustomHeader(request) {
    const newHeaders = new Headers(request.headers);
    newHeaders.delete(CUSTOM_HEADER);
    
    // A headers override resets referrer fields unless the forward carries them too.
    return new Request(request, {
        headers: newHeaders,
        referrer: request.referrer,
        referrerPolicy: request.referrerPolicy,
    });
}
