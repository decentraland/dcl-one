//! The asset-bundle CDN the explorer loads bundles from, served as a
//! bounded read-through cache of the upstream CDN (`ab-cdn.decentraland.org`,
//! `ABGEN_UPSTREAM_AB_CDN` in the old bundle): `GET`/`HEAD /ab-cdn/<path>`
//! for the shapes the boot corpus hits, `LOD/{n}/{cid}_{n}_{platform}`,
//! `manifest/{cid}_{platform}.json`, `v{n}/{cid}_{platform}`,
//! `v{n}/{cid}/{file}` (`scene.json`, `main.crdt`, `bin/index.js`, the
//! bundle itself) and `v{n}/assets/{file}`. The old gateway rewrote
//! `v{n}/assets/x` to `v{n}/x` for the abgen server's own layout; the
//! upstream CDN keeps the `assets/` segment (probed 2026-09-25: the boot
//! corpus' `v49/assets/{cid}_{hash}_mac` answers 200 there and `v49/{cid}_
//! {hash}_mac` 404), so the request path is the upstream path and the cache
//! key, no rewrite. Every object is content-addressed by its
//! name, so a fetched body is kept immutable under
//! [`crate::filecache`] (its own directory beside the ONE blob
//! store: the key is upstream's path, not a hash of the bytes) and served
//! with a year-long immutable cache policy; an upstream 404 is remembered
//! for a minute; an upstream failure answers 502 without caching.
//!
//! The rest of the old bundle, the abgen registry write side, JIT
//! conversion, LOD manifest building and its denylist/queues admin, is a
//! prebuilt external binary with no source in this tree and stays out
//! (`explore::api::registry` keeps the cached upstream reader for the
//! registry reads).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use super::Shared;
use crate::filecache::{FileCache, Served, Source, Supply, valid_rel};

pub const DEFAULT_UPSTREAM: &str = "https://ab-cdn.decentraland.org";
pub const DEFAULT_CACHE_DIR: &str = "/var/lib/catalyst/ab-cdn";
pub const DEFAULT_CACHE_MAX_BYTES: u64 = 21_474_836_480;
/// Served bytes memoised in memory across requests.
const MEM_CACHE_BYTES: usize = 64 << 20;
/// A bundle is tens of MB at most; anything past this is not a bundle.
pub const BODY_CAP: usize = 256 << 20;
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(120);
pub const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(600);
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub struct Cache {
    pub store: Arc<FileCache>,
    supply: Supply<String>,
}

impl Cache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> anyhow::Result<Self> {
        let store = Arc::new(FileCache::new(dir.clone(), max_bytes));
        store
            .init()
            .with_context(|| format!("initializing the ab-cdn cache at {}", dir.display()))?;
        Ok(Self {
            supply: Supply::new(store.clone(), MEM_CACHE_BYTES),
            store,
        })
    }
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/{*path}", get(object))
}

/// The cache key of a request path: the request path itself (the upstream
/// CDN's layout, `assets/` included), admitted only for the three families
/// the CDN serves, each a plain relative path with an object name after the
/// family segment.
pub fn object_key(path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let mut segments = path.split('/');
    let first = segments.next()?;
    let rest: Vec<&str> = segments.collect();
    let versioned = first
        .strip_prefix('v')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    if !(versioned || first == "LOD" || first == "manifest") {
        return None;
    }
    let object = match rest.split_first() {
        Some((&"assets", tail)) if versioned => tail,
        Some(_) => &rest[..],
        None => return None,
    };
    if object.is_empty() {
        return None;
    }
    valid_rel(path).then(|| path.to_string())
}

/// The type from the name; a bundle (`{cid}_{platform}`, no extension:
/// the LOD, versioned and `assets/` objects) is `application/wasm`, as the
/// upstream CDN answers for every platform suffix.
fn content_type_for(key: &str) -> &'static str {
    let name = key.rsplit('/').next().unwrap_or(key);
    if !name.contains('.')
        && ["_mac", "_windows", "_webgl", "_linux", "_android", "_ios"]
            .iter()
            .any(|p| name.ends_with(p))
    {
        return "application/wasm";
    }
    match name.rsplit('.').next() {
        Some("json") => "application/json",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

async fn fetch_upstream(state: &Shared, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
    // No upstream CDN: what this node and abgen hold is all there is.
    if state.settings.sources.ab_cdn.is_empty() {
        return Ok(None);
    }
    let url = format!("{}/{key}", state.settings.sources.ab_cdn);
    let target = state.settings.guarded_url(&url)?;
    let shown = super::mirror::shown(&url);
    let resp = state
        .http
        .get(target)
        .timeout(UPSTREAM_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("{shown}: request failed"))?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::FORBIDDEN {
        return Ok(None);
    }
    if !status.is_success() {
        anyhow::bail!("{shown} answered {status}");
    }
    let body = crate::http::read_body_capped(resp, BODY_CAP)
        .await
        .with_context(|| format!("{shown}: reading the body"))?;
    Ok(Some(body))
}

async fn object(State(state): State<Shared>, Path(path): Path<String>) -> Response {
    let Some(key) = object_key(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !state.settings.ab_cdn_enabled {
        return (
            StatusCode::NOT_FOUND,
            "asset-bundle CDN disabled on this node",
        )
            .into_response();
    }
    let cache = &state.ab_cdn;
    let served = cache
        .supply
        .get(&key, &key, || fetch_upstream(&state, &key), |_| Ok(()))
        .await;
    match served {
        Ok(Served::Hit(bytes, source)) => {
            let mut resp = (
                [
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(content_type_for(&key)),
                    ),
                    (header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE)),
                ],
                bytes,
            )
                .into_response();
            let source = match source {
                Source::Store => "store",
                Source::Upstream => "upstream",
            };
            resp.headers_mut()
                .insert("x-ab-cdn-source", HeaderValue::from_static(source));
            resp
        }
        Ok(Served::Miss) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::warn!(error = %format!("{err:#}"), key, "ab-cdn read-through failed");
            (StatusCode::BAD_GATEWAY, "asset-bundle CDN upstream failed").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_are_the_upstream_paths_of_the_three_families_and_refuse_the_rest() {
        assert_eq!(
            object_key("LOD/0/bafkreib_0_mac").as_deref(),
            Some("LOD/0/bafkreib_0_mac")
        );
        assert_eq!(
            object_key("/manifest/bafk_mac.json").as_deref(),
            Some("manifest/bafk_mac.json")
        );
        assert_eq!(
            object_key("v16/bafybe_mac").as_deref(),
            Some("v16/bafybe_mac")
        );
        assert_eq!(
            object_key("v49/bafk/bin/index.js").as_deref(),
            Some("v49/bafk/bin/index.js")
        );
        assert_eq!(
            object_key("v49/assets/bafk_31d_mac").as_deref(),
            Some("v49/assets/bafk_31d_mac"),
            "the CDN keeps the assets/ segment"
        );
        assert_eq!(
            object_key("v49/assets/dir/x").as_deref(),
            Some("v49/assets/dir/x")
        );
        for bad in [
            "",
            "v49",
            "v/x",
            "vx/y",
            "assets/x",
            "content/x",
            "v49/../x",
            "LOD//x",
            "v49/assets",
            "v49/assets/",
            "LOD",
            "manifest",
        ] {
            assert_eq!(object_key(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn content_types_come_from_the_name() {
        assert_eq!(content_type_for("v49/bafk/scene.json"), "application/json");
        assert_eq!(
            content_type_for("v49/bafk/bin/index.js"),
            "application/javascript"
        );
        assert_eq!(
            content_type_for("v49/bafk/main.crdt"),
            "application/octet-stream"
        );
        assert_eq!(
            content_type_for("LOD/0/bafk_0_mac"),
            "application/wasm",
            "a bundle, as the CDN answers"
        );
        assert_eq!(
            content_type_for("v49/assets/bafk_31d_mac"),
            "application/wasm"
        );
        assert_eq!(content_type_for("v16/bafybe_windows"), "application/wasm");
        assert_eq!(
            content_type_for("manifest/bafk_mac.json"),
            "application/json"
        );
    }
}
