//! The service worker's asset cache (deploy/web/service_worker.js), as the engine reaches it.
//! The cache name, the keys and the header here MUST match the worker's.
//!
//! The worker only answers from the cache; the engine fills it from what it downloads. An entry
//! under the shared key answers for every content server, so it is only written once the bytes
//! are checked against the hash (`content_hash`), and under that hash; a url's response goes
//! under the url.
//!
//! The cache is capped (`effective_cap`). The service worker trims it, least recently used
//! entries first (oldest written when it has no record of use), when the engine asks through
//! the `__contentCacheTrim` page function (deploy/web/engine/content_cache.js): after every
//! `trim_interval` bytes written, when the cap changes, and before retrying a write that failed
//! for quota.

// stand-in origin for shared entries: `.invalid` is reserved, so no server's own entries land here
const SHARED_ORIGIN: &str = "https://shared.invalid";

// The shared key, then the url's own. The shared key is the last path segment alone: the worker
// looks a request up by it, whatever the rest of the url says.
fn keys(url: &str) -> Option<[String; 2]> {
    let url = url::Url::parse(url).ok()?;
    let name = url.path().rsplit('/').next().unwrap_or_default();
    let query = url
        .query()
        .filter(|query| !query.is_empty())
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    Some([
        format!("{SHARED_ORIGIN}/{name}"),
        format!(
            "{}{}{query}",
            url.origin().ascii_serialization(),
            url.path()
        ),
    ])
}

// Whether a request for `url` is one the worker would answer from the shared entry for `hash`.
// A url is the server's to choose (its base url, then the hash), and the path of
// `…/contents/<other hash>#/contents/<hash>` ends at the other hash: sharing that response would
// file it under a hash nobody checked it against.
fn names(url: &str, hash: &str) -> bool {
    keys(url).is_some_and(|[shared, _]| shared == format!("{SHARED_ORIGIN}/{hash}"))
}

/// The most the content cache keeps on web, whatever `AppConfig::cache_bytes` says.
pub const WEB_CACHE_CAP_BYTES: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
// The worker leaves headroom below the cap, so a request every 1/16 of it is enough.
const TRIM_INTERVAL_MAX: u64 = 64 * MIB;
// A full origin is often full for reasons the cap does not see (OPFS, IndexedDB): forced trims
// are spaced so they cannot empty the cache a quarter at a time.
const FORCED_TRIM_SPACING_MS: u64 = 60_000;

/// The cap the worker trims to: a tester's session override, else the configured size, at most
/// [`WEB_CACHE_CAP_BYTES`].
pub fn effective_cap(configured: u64, override_bytes: Option<u64>) -> u64 {
    override_bytes.unwrap_or(configured.min(WEB_CACHE_CAP_BYTES))
}

/// Bytes written between two trim requests.
fn trim_interval(cap: u64) -> u64 {
    (cap / 16).clamp(MIB, TRIM_INTERVAL_MAX)
}

/// Whether writing `len` bytes after `before` bytes crosses a multiple of `interval`.
fn crosses_interval(before: u64, len: u64, interval: u64) -> bool {
    interval > 0 && before / interval != before.saturating_add(len) / interval
}

/// Whether a forced trim may run at `now_ms`, the last one having run at `last_ms` (0 = never).
fn forced_trim_allowed(last_ms: u64, now_ms: u64) -> bool {
    last_ms == 0 || now_ms.saturating_sub(last_ms) >= FORCED_TRIM_SPACING_MS
}

/// What the service worker reported for its last trim (deploy/web/service_worker.js
/// `trimContentCache`), in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrimReport {
    pub cap_bytes: f64,
    pub usage: f64,
    pub quota: f64,
    pub caches_usage: f64,
    pub evicted_entries: f64,
    pub evicted_bytes: f64,
    pub persisted: bool,
    /// Entries the worker ordered by recorded use; `None` when it did not enumerate the cache.
    pub indexed_entries: Option<f64>,
}

/// The engine side's readings, for the debug panel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WebCacheState {
    pub cap_bytes: u64,
    pub override_bytes: Option<u64>,
    pub written_bytes: u64,
    pub quota_retries: u64,
    pub trims: u64,
    pub last: Option<TrimReport>,
    pub pending_uses: usize,
}

fn mib(bytes: f64) -> f64 {
    (bytes / MIB as f64 * 10.0).round() / 10.0
}

/// The `/web_content_cache` reply.
pub fn state_json(state: &WebCacheState) -> serde_json::Value {
    let last = state.last;
    let reason = "The service worker has not answered a trim yet: it does not control this page, or none ran.";
    // a quota the browser leaves out reaches the page as infinity, which JSON writes as null
    let quota_reason = if last.is_some() {
        "The browser reported no quota."
    } else {
        reason
    };
    serde_json::json!({
        "capMiB": mib(state.cap_bytes as f64),
        "overrideMiB": state.override_bytes.map(|b| mib(b as f64)),
        "writtenMiB": mib(state.written_bytes as f64),
        "quotaRetries": state.quota_retries,
        "trims": state.trims,
        "workerCapMiB": last.map(|r| mib(r.cap_bytes)),
        "usageMiB": last.map(|r| mib(r.usage)),
        "quotaMiB": last.and_then(|r| r.quota.is_finite().then(|| mib(r.quota))),
        "cachesMiB": last.map(|r| mib(r.caches_usage)),
        "evictedEntries": last.map(|r| r.evicted_entries),
        "evictedMiB": last.map(|r| mib(r.evicted_bytes)),
        "persisted": last.map(|r| r.persisted),
        "pendingUses": state.pending_uses,
        "indexedEntries": last.and_then(|r| r.indexed_entries),
        "unavailable": {
            "overrideMiB": "No override: the configured cap applies.",
            "workerCapMiB": reason, "usageMiB": reason, "quotaMiB": quota_reason, "cachesMiB": reason,
            "evictedEntries": reason, "evictedMiB": reason, "persisted": reason,
            "indexedEntries": "No trim has enumerated the cache yet: it has stayed under its cap.",
        },
    })
}

#[cfg(target_arch = "wasm32")]
#[bevy::web_worker::page_functions]
#[wasm_bindgen::prelude::wasm_bindgen(js_namespace = self)]
extern "C" {
    /// deploy/web/engine/content_cache.js: asks the service worker to trim the cache to
    /// `cap_bytes`, freeing at least a quarter of it when `quota_error`. Resolves with the worker's
    /// report, or null when no worker answered.
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = "__contentCacheTrim")]
    fn page_content_cache_trim(cap_bytes: f64, quota_error: bool) -> js_sys::Promise;

    /// deploy/web/engine/content_cache.js: records `keys` (cache key urls) as used now.
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = "__contentCacheTouch")]
    fn page_content_cache_touch(keys: &js_sys::Array) -> js_sys::Promise;
}

// When this many distinct keys are waiting they are sent at once, so the set stays small.
const USED_KEYS_FLUSH_AT: usize = 1024;

/// Cache keys read or written since the last flush, each once.
#[derive(Default)]
struct UsedKeys(std::collections::HashSet<String>);

impl UsedKeys {
    /// Notes `key`; true when the set is full and should be flushed.
    fn note(&mut self, key: &str) -> bool {
        if !self.0.contains(key) {
            self.0.insert(key.to_owned());
        }
        self.0.len() >= USED_KEYS_FLUSH_AT
    }

    fn take(&mut self) -> Vec<String> {
        self.0.drain().collect()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(target_arch = "wasm32")]
pub use io::*;

#[cfg(target_arch = "wasm32")]
mod io {
    use super::{
        crosses_interval, forced_trim_allowed, keys, names, page_content_cache_touch,
        page_content_cache_trim, trim_interval, TrimReport, UsedKeys, WebCacheState,
        WEB_CACHE_CAP_BYTES,
    };
    use js_sys::Uint8Array;
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, PoisonError,
    };
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Cache, CacheStorage, Headers, Response, ResponseInit};

    const CACHE_NAME: &str = "ipfs-path-cache-v2";
    /// On a response the worker answered from its cache.
    pub const CACHED_HEADER: &str = "x-ipfs-cached";

    pub async fn open() -> Result<Cache, JsValue> {
        // a window or a worker
        let caches: CacheStorage =
            js_sys::Reflect::get(&js_sys::global(), &"caches".into())?.dyn_into()?;
        JsFuture::from(caches.open(CACHE_NAME)).await?.dyn_into()
    }

    /// The entry under `key`.
    pub async fn get(cache: &Cache, key: &str) -> Result<Option<Response>, JsValue> {
        let response = JsFuture::from(cache.match_with_str(key)).await?;
        if response.is_undefined() {
            return Ok(None);
        }
        Ok(Some(response.dyn_into()?))
    }

    /// The cached entry for `url`, and the key it is under.
    pub async fn find(cache: &Cache, url: &str) -> Result<(String, Response), JsValue> {
        for key in keys(url).ok_or("not a url")? {
            if let Some(response) = get(cache, &key).await? {
                note_used(&key);
                return Ok((key, response));
            }
        }
        Err("no previous cache item".into())
    }

    /// The cached bytes for `url`, under either key; `None` when there are none (or no cache).
    pub async fn read(url: &str) -> Option<Vec<u8>> {
        async fn read(url: &str) -> Result<Vec<u8>, JsValue> {
            let cache = open().await?;
            let (_, response) = find(&cache, url).await?;
            bytes(&response).await
        }
        read(url).await.ok()
    }

    /// The body of a cached response.
    pub async fn bytes(response: &Response) -> Result<Vec<u8>, JsValue> {
        let buffer: js_sys::ArrayBuffer =
            JsFuture::from(response.array_buffer()?).await?.dyn_into()?;
        let array = Uint8Array::new(&buffer);
        let mut data = vec![0u8; array.length() as usize];
        array.copy_to(&mut data);
        Ok(data)
    }

    async fn put_once(
        cache: &Cache,
        key: &str,
        old: &Response,
        data: &[u8],
    ) -> Result<(), JsValue> {
        let headers = old.headers();
        headers.set("Content-Length", &data.len().to_string())?;

        let init = ResponseInit::new();
        init.set_status(old.status());
        init.set_status_text(&old.status_text());
        init.set_headers(&headers);

        let body = Uint8Array::from(data);
        let response = Response::new_with_opt_buffer_source_and_init(Some(&body), &init)?;
        JsFuture::from(cache.put_with_str(key, &response)).await?;
        Ok(())
    }

    async fn store_once(cache: &Cache, key: &str, data: &[u8]) -> Result<(), JsValue> {
        let headers = Headers::new()?;
        headers.set("Content-Type", "application/octet-stream")?;
        headers.set("Content-Length", &data.len().to_string())?;

        let init = ResponseInit::new();
        init.set_status(200);
        init.set_headers(&headers);

        let body = Uint8Array::from(data);
        let response = Response::new_with_opt_buffer_source_and_init(Some(&body), &init)?;
        JsFuture::from(cache.put_with_str(key, &response)).await?;
        Ok(())
    }

    /// Stores `data`, the bytes `url` answered with, under the url's own key.
    pub async fn keep(url: &str, data: &[u8]) -> Result<(), JsValue> {
        let [_, own] = keys(url).ok_or("not a url")?;
        let cache = open().await?;
        store(&cache, &own, data).await
    }

    /// Drops the entry under the url's own key, if there is one.
    pub async fn forget(url: &str) -> Result<(), JsValue> {
        let [_, own] = keys(url).ok_or("not a url")?;
        let cache = open().await?;
        JsFuture::from(cache.delete_with_str(&own)).await?;
        Ok(())
    }

    /// Stores `data`, the bytes `url` answered with, under the shared key for `hash`, if
    /// `matches_hash`: whether they are what `hash` names. The url's own entry, if an older
    /// worker left one, goes: it is dead weight beside the shared one.
    pub async fn share(
        url: &str,
        hash: &str,
        data: &[u8],
        matches_hash: impl FnOnce() -> bool,
    ) -> Result<(), JsValue> {
        if !names(url, hash) || !matches_hash() {
            return Ok(());
        }
        let [shared, own] = keys(url).ok_or("not a url")?;
        let cache = open().await?;
        store(&cache, &shared, data).await?;
        JsFuture::from(cache.delete_with_str(&own)).await?;
        Ok(())
    }

    // Written from the asset loader and processor workers, which have no `World`; one wasm memory
    // is shared by every worker of an engine instance.
    static CAP_BYTES: AtomicU64 = AtomicU64::new(WEB_CACHE_CAP_BYTES);
    static WRITTEN_BYTES: AtomicU64 = AtomicU64::new(0);
    static LAST_FORCED_MS: AtomicU64 = AtomicU64::new(0);
    static QUOTA_RETRIES: AtomicU64 = AtomicU64::new(0);
    static TRIMS: AtomicU64 = AtomicU64::new(0);
    static LAST_REPORT: Mutex<Option<TrimReport>> = Mutex::new(None);
    static USED: std::sync::LazyLock<Mutex<UsedKeys>> = std::sync::LazyLock::new(Default::default);

    /// Replaces the body of the entry under `key`.
    pub async fn put(cache: &Cache, key: &str, old: &Response, data: &[u8]) -> Result<(), JsValue> {
        write_with_quota_retry(key, data.len(), || put_once(cache, key, old, data)).await
    }

    /// Stores `data` under `key` as a fresh response.
    async fn store(cache: &Cache, key: &str, data: &[u8]) -> Result<(), JsValue> {
        write_with_quota_retry(key, data.len(), || store_once(cache, key, data)).await
    }

    /// The cap the worker trims to.
    pub fn set_cap(cap_bytes: u64) {
        CAP_BYTES.store(cap_bytes, Ordering::Relaxed);
    }

    /// Asks the service worker for a trim to the current cap and keeps its report.
    pub async fn request_trim(quota_error: bool) -> Option<TrimReport> {
        // posted first, not awaited: the page and the worker keep arrival order, so the trim sees
        // these uses, and a write waiting on a quota trim never waits on them
        if let Some(touch) = post_used() {
            wasm_bindgen_futures::spawn_local(log_unrecorded(touch));
        }
        let cap = CAP_BYTES.load(Ordering::Relaxed) as f64;
        let reply = JsFuture::from(page_content_cache_trim(cap, quota_error))
            .await
            .map_err(|e| bevy::log::warn!("content cache trim failed: {e:?}"))
            .ok()?;
        let report = report_from_js(&reply)?;
        TRIMS.fetch_add(1, Ordering::Relaxed);
        *LAST_REPORT.lock().unwrap_or_else(PoisonError::into_inner) = Some(report);
        Some(report)
    }

    /// The engine side's readings, for the debug panel.
    pub fn state(override_bytes: Option<u64>) -> WebCacheState {
        WebCacheState {
            cap_bytes: CAP_BYTES.load(Ordering::Relaxed),
            override_bytes,
            written_bytes: WRITTEN_BYTES.load(Ordering::Relaxed),
            quota_retries: QUOTA_RETRIES.load(Ordering::Relaxed),
            trims: TRIMS.load(Ordering::Relaxed),
            last: *LAST_REPORT.lock().unwrap_or_else(PoisonError::into_inner),
            pending_uses: USED.lock().unwrap_or_else(PoisonError::into_inner).len(),
        }
    }

    // A failed `put` may have consumed its response's body: `write` builds a fresh one each call.
    async fn write_with_quota_retry<F, Fut>(key: &str, len: usize, write: F) -> Result<(), JsValue>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<(), JsValue>>,
    {
        let result = match write().await {
            Err(e) if is_quota_error(&e) && claim_forced_trim() => {
                QUOTA_RETRIES.fetch_add(1, Ordering::Relaxed);
                bevy::log::debug!("content cache full writing `{key}`: trimming and retrying");
                request_trim(true).await;
                write().await
            }
            other => other,
        };
        if result.is_ok() {
            note_used(key);
            note_written(len as u64);
        }
        result
    }

    fn is_quota_error(error: &JsValue) -> bool {
        js_sys::Reflect::get(error, &"name".into())
            .ok()
            .and_then(|name| name.as_string())
            .is_some_and(|name| name == "QuotaExceededError")
    }

    fn claim_forced_trim() -> bool {
        let now = js_sys::Date::now() as u64;
        let last = LAST_FORCED_MS.load(Ordering::Relaxed);
        forced_trim_allowed(last, now)
            && LAST_FORCED_MS
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
    }

    fn note_written(len: u64) {
        let interval = trim_interval(CAP_BYTES.load(Ordering::Relaxed));
        let before = WRITTEN_BYTES.fetch_add(len, Ordering::Relaxed);
        if crosses_interval(before, len, interval) {
            wasm_bindgen_futures::spawn_local(async {
                request_trim(false).await;
            });
        }
    }

    fn report_from_js(value: &JsValue) -> Option<TrimReport> {
        if value.is_null() || value.is_undefined() {
            return None;
        }
        let get = |name: &str| js_sys::Reflect::get(value, &name.into()).ok();
        if let Some(error) = get("error").and_then(|e| e.as_string()) {
            bevy::log::warn!("content cache trim failed in the service worker: {error}");
            return None;
        }
        let number = |name: &str| get(name).and_then(|v| v.as_f64());
        Some(TrimReport {
            cap_bytes: number("capBytes")?,
            usage: number("usage")?,
            quota: number("quota")?,
            caches_usage: number("cachesUsage")?,
            evicted_entries: number("evictedEntries")?,
            evicted_bytes: number("evictedBytes")?,
            persisted: get("persisted").and_then(|v| v.as_bool()).unwrap_or(false),
            indexed_entries: number("indexedEntries"),
        })
    }

    fn note_used(key: &str) {
        let full = USED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .note(key);
        if full {
            wasm_bindgen_futures::spawn_local(flush_used());
        }
    }

    /// Posts the keys used since the last post to the service worker's last-use index, without
    /// waiting for its reply.
    fn post_used() -> Option<js_sys::Promise> {
        let keys = USED.lock().unwrap_or_else(PoisonError::into_inner).take();
        if keys.is_empty() {
            return None;
        }
        let array: js_sys::Array = keys.iter().map(|key| JsValue::from_str(key)).collect();
        Some(page_content_cache_touch(&array))
    }

    async fn log_unrecorded(touch: js_sys::Promise) {
        if let Err(e) = JsFuture::from(touch).await {
            bevy::log::debug!("content cache uses not recorded: {e:?}");
        }
    }

    /// Sends the keys used since the last flush to the service worker's last-use index. A batch
    /// that is not delivered is dropped: those entries keep their previous record, or none, which
    /// ranks them first.
    pub async fn flush_used() {
        if let Some(touch) = post_used() {
            log_unrecorded(touch).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_key_is_the_hash_whichever_server_and_path() {
        let [catalyst, _] = keys("https://peer.example/content/contents/bafkreihash").unwrap();
        let [worlds, own] = keys("https://worlds.example/contents/bafkreihash?v=1").unwrap();
        assert_eq!(catalyst, "https://shared.invalid/bafkreihash");
        assert_eq!(worlds, catalyst);
        assert_eq!(own, "https://worlds.example/contents/bafkreihash?v=1");
    }

    #[test]
    fn a_url_only_names_the_hash_its_path_ends_with() {
        assert!(names(
            "https://peer.example/contents/bafkreihash",
            "bafkreihash"
        ));
        // the worker keys this request by `bafkreitarget`
        let url = "https://evil.example/contents/bafkreitarget#/contents/bafkreihash";
        assert!(!names(url, "bafkreihash"));
        assert_eq!(
            keys(url).unwrap(),
            [
                "https://shared.invalid/bafkreitarget".to_owned(),
                "https://evil.example/contents/bafkreitarget".to_owned()
            ]
        );
        assert!(!names(
            "https://evil.example/contents/bafkreitarget?/bafkreihash",
            "bafkreihash"
        ));
        assert!(!names(
            "https://peer.example/contents/bafkreihash/",
            "bafkreihash"
        ));
    }

    #[test]
    fn web_cap_is_at_most_a_gibibyte_whatever_the_config() {
        assert_eq!(
            effective_cap(common::structs::AppConfig::default().cache_bytes, None),
            WEB_CACHE_CAP_BYTES
        );
        assert_eq!(effective_cap(200 << 20, None), 200 << 20);
    }

    #[test]
    fn a_session_override_replaces_the_configured_cap() {
        assert_eq!(effective_cap(10 << 30, Some(50 << 20)), 50 << 20);
        assert_eq!(effective_cap(10 << 30, Some(4 << 30)), 4 << 30);
    }

    #[test]
    fn trims_are_requested_every_sixteenth_of_the_cap() {
        assert_eq!(trim_interval(1 << 30), 64 << 20);
        assert!(crosses_interval(63 << 20, 1 << 20, 64 << 20));
        assert!(!crosses_interval(0, (64 << 20) - 1, 64 << 20));
        assert!(!crosses_interval(10, 0, 64 << 20));
    }

    #[test]
    fn small_caps_trim_at_least_every_mebibyte_and_large_ones_every_64() {
        assert_eq!(trim_interval(4 << 20), 1 << 20);
        assert_eq!(trim_interval(16 << 30), 64 << 20);
        assert_eq!(trim_interval(50 << 20), (50 << 20) / 16);
    }

    #[test]
    fn forced_trims_are_a_minute_apart() {
        assert!(forced_trim_allowed(0, 5));
        assert!(!forced_trim_allowed(1_000, 60_999));
        assert!(forced_trim_allowed(1_000, 61_000));
        // the clock went back
        assert!(!forced_trim_allowed(70_000, 1_000));
    }

    #[test]
    fn state_without_a_report_names_why_the_worker_readings_are_missing() {
        let state = WebCacheState {
            cap_bytes: 1 << 30,
            ..Default::default()
        };
        let json: serde_json::Value =
            serde_json::from_str(&state_json(&state).to_string()).unwrap();
        assert_eq!(json["capMiB"], 1024.0);
        assert!(json["usageMiB"].is_null());
        assert!(json["unavailable"]["usageMiB"].is_string());
    }

    #[test]
    fn state_reports_the_last_trim_in_mebibytes() {
        let state = WebCacheState {
            last: Some(TrimReport {
                evicted_bytes: 3.0 * 1048576.0 + 52429.0,
                persisted: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let json: serde_json::Value =
            serde_json::from_str(&state_json(&state).to_string()).unwrap();
        assert_eq!(json["evictedMiB"], 3.1);
        assert_eq!(json["persisted"], true);
    }

    #[test]
    fn a_report_without_a_quota_says_the_browser_gave_none() {
        let state = WebCacheState {
            last: Some(TrimReport {
                usage: 1048576.0,
                quota: f64::INFINITY,
                ..Default::default()
            }),
            ..Default::default()
        };
        let json: serde_json::Value =
            serde_json::from_str(&state_json(&state).to_string()).unwrap();
        assert!(json["quotaMiB"].is_null());
        assert_eq!(
            json["unavailable"]["quotaMiB"],
            "The browser reported no quota."
        );
        // the other readings are the report's, not missing
        assert_eq!(json["usageMiB"], 1.0);
    }

    #[test]
    fn a_report_with_a_quota_shows_it() {
        let state = WebCacheState {
            last: Some(TrimReport {
                quota: 2.0 * 1048576.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let json: serde_json::Value =
            serde_json::from_str(&state_json(&state).to_string()).unwrap();
        assert_eq!(json["quotaMiB"], 2.0);
    }

    #[test]
    fn a_key_used_twice_is_sent_once() {
        let mut used = UsedKeys::default();
        used.note("a");
        used.note("a");
        used.note("b");
        let mut keys = used.take();
        keys.sort();
        assert_eq!(keys, ["a", "b"]);
        assert_eq!(used.len(), 0);
    }

    #[test]
    fn the_set_asks_for_a_flush_when_full() {
        let mut used = UsedKeys::default();
        for n in 0..USED_KEYS_FLUSH_AT - 1 {
            assert!(!used.note(&n.to_string()));
        }
        assert!(used.note("last"));
    }

    #[test]
    fn state_counts_waiting_uses_and_reports_ordered_entries() {
        let state = WebCacheState {
            pending_uses: 3,
            last: Some(TrimReport {
                indexed_entries: Some(7.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let json: serde_json::Value =
            serde_json::from_str(&state_json(&state).to_string()).unwrap();
        assert_eq!(json["pendingUses"], 3);
        assert_eq!(json["indexedEntries"], 7.0);

        let state = WebCacheState {
            last: Some(TrimReport::default()),
            ..Default::default()
        };
        let json: serde_json::Value =
            serde_json::from_str(&state_json(&state).to_string()).unwrap();
        assert!(json["indexedEntries"].is_null());
        assert!(json["unavailable"]["indexedEntries"].is_string());
    }
}
