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
// Set on responses answered from the cache, to the key the response came from: the engine records
// that entry as used.
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
// failed for quota. The trim runs here, off the engine's threads, evicts the least recently used
// entries first (oldest written first for those with no recorded use), and only ever opens CACHE_NAME.
const CONTENT_CACHE_TRIM = 'content-cache-trim';
const CONTENT_CACHE_TOUCH = 'content-cache-touch';
// When each entry was last read or written, as the engine reported it: eviction goes least
// recently used first. The browser may drop it at any time; without it, oldest written first.
const CONTENT_CACHE_INDEX_DB = 'dcl-content-cache-index-v1';
const CONTENT_CACHE_INDEX_STORE = 'lastUsed';
// Only this many keys of a message are looked at: the engine sends at most 1024. A key is an
// http(s) url of at most this many characters, as the engine builds them. The engine does not send
// longer keys (crates/ipfs/src/web_cache.rs `USED_KEY_MAX`); such entries rank as never used.
const CONTENT_CACHE_TOUCH_MAX = 4096;
const CONTENT_CACHE_KEY_MAX = 2048;
// The index is an aid: an operation on it that has not finished by then is given up, so a trim
// never waits on it and falls back to oldest written first.
const CONTENT_CACHE_INDEX_TIMEOUT_MS = 5000;
// A write waits on a quota-error trim: that trim gives the index this long in all, then evicts
// oldest written first, and does not wait for the index to be pruned.
const CONTENT_CACHE_QUOTA_INDEX_TIMEOUT_MS = 1000;
// Half the origin's quota stays for its other storage (OPFS, IndexedDB).
const CONTENT_CACHE_QUOTA_SHARE = 0.5;
// A trim goes down to this share of the cap, so the next one waits for real growth.
const CONTENT_CACHE_LOW_WATER = 0.9;
// After a quota error, keep at most this share of the cache, even under the cap.
const CONTENT_CACHE_QUOTA_ERROR_KEEP = 0.75;
// Entries sized (one `match` each) per step while choosing what to evict.
const CONTENT_CACHE_SIZE_BATCH = 64;
// Requests waiting their turn, uses and trims alike, one at a time in arrival order: a trim sees
// what the one before it left and the uses sent before it. A quota-error trim goes ahead of the
// regular requests, and a running regular trim stops for it: the engine holds a write until it is
// answered.
const contentCacheRequests = [];
let contentCacheRunning = false;

const contentCacheForcedWaiting = () => contentCacheRequests.some((request) => request.quotaError);

async function runContentCacheRequests() {
    if (contentCacheRunning) return;
    contentCacheRunning = true;
    while (contentCacheRequests.length > 0) {
        const { task, resolve, reject } = contentCacheRequests.shift();
        await task().then(resolve, reject);
    }
    contentCacheRunning = false;
}

self.addEventListener('message', (event) => {
    const data = event.data;
    let task;
    let quotaError = false;
    // work a task leaves running after its reply (a quota-error trim's prune): kept alive below
    const background = [];
    if (data?.type === CONTENT_CACHE_TRIM) {
        quotaError = data.quotaError === true;
        // the uses the engine has not sent yet: a quota-error trim carries them instead of queueing
        // a touch ahead of itself
        const usedNow = quotaError ? validContentCacheKeys(data.usedNow) : [];
        task = () => trimContentCache(data.capBytes, quotaError, usedNow, (work) => background.push(work));
    } else if (data?.type === CONTENT_CACHE_TOUCH) {
        task = () => recordContentCacheUse(data.keys);
    } else {
        return;
    }
    const port = event.ports?.[0];
    const run = new Promise((resolve, reject) => {
        const request = { task, quotaError, resolve, reject };
        const regular = contentCacheRequests.findIndex((waiting) => !waiting.quotaError);
        if (quotaError && regular >= 0) contentCacheRequests.splice(regular, 0, request);
        else contentCacheRequests.push(request);
    });
    runContentCacheRequests();
    event.waitUntil(run.then(
        (report) => port?.postMessage(report),
        (e) => port?.postMessage({ error: String(e?.message ?? e) }),
    ).then(() => Promise.all(background)));
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

// The engine stores every entry with Content-Length. The body is then never read: release it.
async function contentCacheEntrySize(response) {
    const length = Number(response.headers.get('Content-Length'));
    if (response.headers.has('Content-Length') && Number.isFinite(length)) {
        response.body?.cancel().catch(() => {});
        return length;
    }
    return (await response.blob()).size;
}

// Entries without a recorded use first, in `keys()` order (oldest written first), then by last use.
function contentCacheEvictionOrder(requests, lastUsed) {
    const url = (request) => request.url ?? request;
    const ranked = requests.map((request, position) => ({ request, position, used: lastUsed.get(url(request)) }));
    ranked.sort((a, b) => {
        if (a.used === undefined || b.used === undefined) {
            if (a.used !== b.used) return a.used === undefined ? -1 : 1;
        } else if (a.used !== b.used) {
            return a.used - b.used;
        }
        return a.position - b.position;
    });
    return ranked.map(({ request }) => request);
}

function openContentCacheIndex() {
    return new Promise((resolve, reject) => {
        const open = indexedDB.open(CONTENT_CACHE_INDEX_DB, 1);
        // an open that succeeds after it was given up has nobody to close it
        const giveUp = (error) => {
            clearTimeout(timer);
            open.onsuccess = () => open.result.close();
            reject(error);
        };
        const timer = setTimeout(() => giveUp(new Error('content cache index open timed out')), CONTENT_CACHE_INDEX_TIMEOUT_MS);
        open.onupgradeneeded = () => open.result.createObjectStore(CONTENT_CACHE_INDEX_STORE);
        open.onsuccess = () => {
            clearTimeout(timer);
            // a page asking for another version must not wait on a connection held for one operation
            open.result.onversionchange = () => open.result.close();
            resolve(open.result);
        };
        open.onerror = () => giveUp(open.error);
        open.onblocked = () => giveUp(new Error('content cache index blocked'));
    });
}

// Gives up on `promise` after `timeoutMs`.
function withinIndexTimeout(promise, timeoutMs = CONTENT_CACHE_INDEX_TIMEOUT_MS) {
    let timer;
    const timeout = new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('content cache index timed out')), timeoutMs);
    });
    return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

function finished(transaction) {
    return new Promise((resolve, reject) => {
        transaction.oncomplete = () => resolve();
        transaction.onerror = () => reject(transaction.error ?? new Error('content cache index transaction failed'));
        transaction.onabort = () => reject(transaction.error ?? new Error('content cache index transaction failed'));
    });
}

function result(request) {
    return new Promise((resolve, reject) => {
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}

// Every recorded use, or none when the index is gone, unreadable or not read within `timeoutMs`.
async function readContentCacheUses(timeoutMs = 2 * CONTENT_CACHE_INDEX_TIMEOUT_MS) {
    const read = (async () => {
        let db;
        try {
            db = await openContentCacheIndex();
            const store = db.transaction(CONTENT_CACHE_INDEX_STORE).objectStore(CONTENT_CACHE_INDEX_STORE);
            const [keys, times] = await withinIndexTimeout(Promise.all([result(store.getAllKeys()), result(store.getAll())]));
            return new Map(keys.map((key, i) => [key, times[i]]));
        } finally {
            db?.close();
        }
    })();
    try {
        return await withinIndexTimeout(read, timeoutMs);
    } catch (e) {
        // a read given up on still settles later: nobody waits for it
        read.catch(() => {});
        console.warn('[IPFS Cache Service Worker]: content cache index unavailable, evicting oldest written first', e);
        return new Map();
    }
}

// Applies `put` (keys used now) and `remove` (keys gone) in one transaction.
async function writeContentCacheUses(put, remove) {
    if (put.length === 0 && remove.length === 0) return;
    const db = await openContentCacheIndex();
    try {
        const transaction = db.transaction(CONTENT_CACHE_INDEX_STORE, 'readwrite');
        const store = transaction.objectStore(CONTENT_CACHE_INDEX_STORE);
        const now = Date.now();
        for (const key of put) store.put(now, key);
        for (const key of remove) store.delete(key);
        await withinIndexTimeout(finished(transaction));
    } finally {
        db.close();
    }
}

// The keys of a message the worker looks at: the first CONTENT_CACHE_TOUCH_MAX that are short
// http(s) urls; none when `keys` is not an array.
function validContentCacheKeys(keys) {
    if (!Array.isArray(keys)) return [];
    return keys
        .slice(0, CONTENT_CACHE_TOUCH_MAX)
        .filter((key) => typeof key === 'string' && key.length <= CONTENT_CACHE_KEY_MAX && /^https?:\/\//.test(key));
}

async function recordContentCacheUse(keys) {
    if (!Array.isArray(keys)) throw new Error('keys must be an array');
    const valid = validContentCacheKeys(keys);
    await writeContentCacheUses(valid, []);
    return { recorded: valid.length };
}

// `usedNow`: keys used since the engine's last touch, ranked as used now without being recorded.
// `keepAlive(promise)`: work to keep the worker alive for after the reply.
async function trimContentCache(capBytes, quotaError, usedNow = [], keepAlive = () => {}) {
    if (!(typeof capBytes === 'number' && capBytes >= 0)) throw new Error('capBytes must be a non-negative number');
    const { usage = 0, quota = Infinity, usageDetails } = await navigator.storage.estimate();
    // usageDetails leaves out a storage system with no usage
    const cachesUsage = usageDetails ? (usageDetails.caches ?? 0) : usage;
    // only for the report: a browser without it, or one that refuses, must not stop the trim
    const persisted = (await navigator.storage.persisted?.().catch(() => false)) ?? false;
    const { cap, bytesToFree } = contentCacheTarget({ capBytes, quota, cachesUsage, quotaError });
    const report = {
        capBytes: cap, usage, quota, cachesUsage, evictedEntries: 0, evictedBytes: 0, persisted,
        targetUnreachable: false, preempted: false,
    };
    let toFree = bytesToFree;
    if (toFree <= 0) return report;

    const cache = await caches.open(CACHE_NAME);
    // insertion order: oldest written first (a put over a key moves it to the end)
    const requests = await cache.keys();
    const lastUsed = await readContentCacheUses(quotaError ? CONTENT_CACHE_QUOTA_INDEX_TIMEOUT_MS : undefined);
    const ranked = new Map(lastUsed);
    const now = Date.now();
    for (const key of usedNow) ranked.set(key, now);
    const ordered = contentCacheEvictionOrder(requests, ranked);
    report.indexedEntries = requests.filter((r) => ranked.has(r.url)).length;
    // Size before deleting: `cachesUsage` also counts other caches (and, without usageDetails, all
    // storage), which no eviction here reaches.
    const sized = [];
    let contentBytes = 0;
    for (let i = 0; i < ordered.length && contentBytes < toFree; i += CONTENT_CACHE_SIZE_BATCH) {
        if (!quotaError && contentCacheForcedWaiting()) {
            report.preempted = true;
            return report;
        }
        const batch = await Promise.all(ordered.slice(i, i + CONTENT_CACHE_SIZE_BATCH).map(async (key) => {
            const response = await cache.match(key);
            return { key, size: response ? await contentCacheEntrySize(response) : 0 };
        }));
        for (const entry of batch) contentBytes += entry.size;
        sized.push(...batch);
    }
    if (contentBytes < toFree) {
        // the whole content cache would not reach the target: hold it to the cap by itself
        report.targetUnreachable = true;
        toFree = contentCacheTarget({ capBytes, quota, cachesUsage: contentBytes, quotaError }).bytesToFree;
    }
    const { picked } = pickEvictions(sized, toFree);
    if (picked.length === 0) return report;
    // An entry written again while this trim sized moved to the newest end, behind a key that
    // followed it in the first listing: keep it, and skip keys gone meanwhile. This covers the
    // sizing only, not a write between this listing and the deletes.
    const first = new Map(requests.map((key, i) => [key.url, i]));
    const listed = new Set();
    const rewritten = new Set();
    let latest = -1;
    for (const key of await cache.keys()) {
        const i = first.get(key.url);
        if (i === undefined) continue;
        listed.add(key.url);
        if (i < latest) rewritten.add(key.url);
        else latest = i;
    }
    const sizes = new Map(sized.map(({ key, size }) => [key.url, size]));
    const evicted = picked.filter((key) => listed.has(key.url) && !rewritten.has(key.url));
    await Promise.all(evicted.map((key) => cache.delete(key)));
    report.evictedEntries = evicted.length;
    report.evictedBytes = evicted.reduce((sum, key) => sum + sizes.get(key.url), 0);
    const present = new Set(requests.map((r) => r.url));
    const stale = [...lastUsed.keys()].filter((key) => !present.has(key));
    const pruned = writeContentCacheUses([], [...evicted.map((key) => key.url), ...stale])
        .catch((e) => console.warn('[IPFS Cache Service Worker]: content cache index not pruned', e));
    if (quotaError) keepAlive(pruned);
    else await pruned;
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
        // the engine records this key as used
        headers.set(CACHED_HEADER, sharedResponse ? sharedKey : originKey);
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
