//! No source for the registry lives in this tree: it is decentraland/abgen,
//! an external dependency this node only calls; the wire shapes below come
//! from the live asset-bundle-registry.decentraland.org service.
//!
//! The asset-bundle registry reads unity-explorer makes at boot, as a cached
//! reader over the upstream registry: `POST /entities/active` (the content
//! server's shape plus `bundles`, `status`, `versions` and `deployer`, only
//! for entities the registry has converted), `POST /entities/versions`,
//! `POST /profiles` (the lambdas profile shape with a string `timestamp`),
//! `POST /profiles/metadata` (upstream's `getMetadata` projection: `pointer`,
//! `hasClaimedName`, `name`, `nameColor`, `thumbnailUrl`, no avatars), and
//! `GET /status`. The bundle state is
//! upstream's alone (the bundles themselves are served from upstream's CDN),
//! so the answer is upstream's, cached per pointer, id or address so a
//! radius load or a repeated avatar costs one upstream call per key per TTL.

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Shared, json_error, mirror};
use crate::shared::cache::TtlMap;

pub const MAX_KEYS: usize = 1000;
const ENTITY_TTL: Duration = Duration::from_secs(300);
const PROFILE_TTL: Duration = Duration::from_secs(60);
const ENTITIES_BODY_CAP: usize = 64 << 20;
const PROFILES_BODY_CAP: usize = 16 << 20;

pub struct Cache {
    /// `<selector>:<world>:<key>` -> the entity id it resolves to (`None`: the registry has nothing).
    keys: TtlMap<String, Option<String>>,
    entities: TtlMap<String, Arc<Bytes>>,
    /// `<variant>:<address>` -> the profile document.
    profiles: TtlMap<String, Option<Arc<Bytes>>>,
    /// Sorted pointer set -> the whole `/entities/versions` answer.
    versions: TtlMap<String, Arc<Bytes>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self {
            keys: TtlMap::bounded(ENTITY_TTL, 50_000),
            entities: TtlMap::bounded(ENTITY_TTL, 5_000),
            profiles: TtlMap::bounded(PROFILE_TTL, 20_000),
            versions: TtlMap::bounded(ENTITY_TTL, 2_000),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SelectorBody {
    #[serde(default)]
    pub pointers: Option<Vec<String>>,
    #[serde(default)]
    pub ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct WorldQuery {
    pub world_name: Option<String>,
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/entities/active", post(entities_active))
        .route("/entities/versions", post(entities_versions))
        .route("/profiles", post(profiles))
        .route("/profiles/metadata", post(profiles_metadata))
        .route("/status", get(status))
        .route("/health", get(health))
}

enum Selector {
    Pointers,
    Ids,
}

impl Selector {
    fn field(&self) -> &'static str {
        match self {
            Selector::Pointers => "pointers",
            Selector::Ids => "ids",
        }
    }
}

/// Exactly one non-empty selector of at most `MAX_KEYS` values; pointers are
/// compared case-insensitively like the content server does.
fn select(body: SelectorBody) -> Result<(Selector, Vec<String>), Response> {
    let (selector, values) = match (body.pointers, body.ids) {
        (Some(p), None) if !p.is_empty() => (
            Selector::Pointers,
            p.iter().map(|s| s.to_lowercase()).collect(),
        ),
        (None, Some(i)) if !i.is_empty() => (Selector::Ids, i),
        _ => {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "ids or pointers must be present in the request body, and only one of them",
            ));
        }
    };
    if values.len() > MAX_KEYS {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("at most {MAX_KEYS} ids or pointers per request"),
        ));
    }
    let mut seen = HashSet::new();
    let values: Vec<String> = values
        .into_iter()
        .filter(|v| seen.insert(v.clone()))
        .collect();
    Ok((selector, values))
}

fn json_array(docs: &[Arc<Bytes>]) -> Response {
    let mut out = Vec::with_capacity(docs.iter().map(|d| d.len() + 1).sum::<usize>() + 2);
    out.push(b'[');
    for (i, doc) in docs.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        out.extend_from_slice(doc);
    }
    out.push(b']');
    ([(CONTENT_TYPE, "application/json")], out).into_response()
}

/// The detail (which names the upstream URL) goes to the log; the client
/// gets the route that failed.
fn upstream_error(what: &str, err: anyhow::Error) -> Response {
    tracing::warn!(error = %format!("{err:#}"), "asset-bundle registry upstream: {what}");
    json_error(
        StatusCode::BAD_GATEWAY,
        format!("asset-bundle registry upstream: {what} failed"),
    )
}

async fn post_upstream(
    state: &Shared,
    path: &str,
    query: &[(&str, &str)],
    body: Value,
    cap: usize,
) -> anyhow::Result<Vec<u8>> {
    // No registry to read: it knows no entity, version or profile.
    if state.settings.sources.registry.is_empty() {
        return Ok(b"[]".to_vec());
    }
    let mut url = format!("{}{path}", state.settings.sources.registry);
    for (i, (k, v)) in query.iter().enumerate() {
        url.push(if i == 0 { '?' } else { '&' });
        url.push_str(k);
        url.push('=');
        url.push_str(&urlencoding::encode(v));
    }
    Ok(mirror::fetch(
        &state.http,
        &state.settings,
        reqwest::Method::POST,
        &url,
        Some(&body),
        &[],
        cap,
    )
    .await?
    .body)
}

/// `POST /entities/active[?world_name=]`: every key answered from the cache;
/// the misses in one upstream call whose entities then fill the cache under
/// each of their pointers, and whose absent keys are remembered as absent.
async fn entities_active(
    State(state): State<Shared>,
    Query(q): Query<WorldQuery>,
    Json(body): Json<SelectorBody>,
) -> Response {
    let (selector, keys) = match select(body) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let world = q
        .world_name
        .as_deref()
        .map(str::to_lowercase)
        .unwrap_or_default();
    let scope = format!("{}:{world}:", selector.field());
    let cache = &state.registry;

    let mut resolved: HashMap<String, Option<String>> = HashMap::new();
    let mut misses = Vec::new();
    for key in &keys {
        match cache.keys.get_fresh(&format!("{scope}{key}")) {
            Some(hit) => {
                resolved.insert(key.clone(), hit);
            }
            None => misses.push(key.clone()),
        }
    }

    if !misses.is_empty() {
        let query: Vec<(&str, &str)> = q
            .world_name
            .as_deref()
            .map(|w| ("world_name", w))
            .into_iter()
            .collect();
        let bytes = match post_upstream(
            &state,
            "/entities/active",
            &query,
            json!({ selector.field(): misses }),
            ENTITIES_BODY_CAP,
        )
        .await
        {
            Ok(b) => b,
            Err(err) => return upstream_error("entities/active", err),
        };
        let docs: Vec<Value> = match serde_json::from_slice(&bytes) {
            Ok(d) => d,
            Err(err) => {
                return upstream_error(
                    "entities/active",
                    anyhow::anyhow!("body is not an entity array: {err}"),
                );
            }
        };
        for doc in docs {
            let Some(id) = doc.get("id").and_then(Value::as_str).map(str::to_string) else {
                continue;
            };
            let raw = Arc::new(Bytes::from(serde_json::to_vec(&doc).unwrap_or_default()));
            cache.entities.insert(id.clone(), raw);
            let owned: Vec<String> = match selector {
                Selector::Ids => vec![id.clone()],
                Selector::Pointers => doc
                    .get("pointers")
                    .and_then(Value::as_array)
                    .map(|ps| {
                        ps.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_lowercase)
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            for key in owned {
                cache.keys.insert(format!("{scope}{key}"), Some(id.clone()));
                if misses.contains(&key) {
                    resolved.insert(key, Some(id.clone()));
                }
            }
        }
        for key in misses {
            if let Entry::Vacant(slot) = resolved.entry(key) {
                cache.keys.insert(format!("{scope}{}", slot.key()), None);
                slot.insert(None);
            }
        }
    }

    let mut ids_seen = HashSet::new();
    let mut docs = Vec::new();
    for key in &keys {
        let Some(Some(id)) = resolved.get(key) else {
            continue;
        };
        if !ids_seen.insert(id.clone()) {
            continue;
        }
        match cache.entities.get_fresh(id) {
            Some(doc) => docs.push(doc),
            None => {
                // The entity aged out between the key lookup and now; drop the
                // key so the next request refetches instead of answering short.
                cache.keys.insert(format!("{scope}{key}"), None);
            }
        }
    }
    json_array(&docs)
}

/// `POST /entities/versions`: the whole answer cached per pointer set.
async fn entities_versions(
    State(state): State<Shared>,
    Json(body): Json<SelectorBody>,
) -> Response {
    let (selector, mut keys) = match select(body) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    keys.sort();
    let cache_key = format!("{}:{}", selector.field(), keys.join("\n"));
    if let Some(hit) = state.registry.versions.get_fresh(&cache_key) {
        return ([(CONTENT_TYPE, "application/json")], hit.as_ref().clone()).into_response();
    }
    match post_upstream(
        &state,
        "/entities/versions",
        &[],
        json!({ selector.field(): keys }),
        ENTITIES_BODY_CAP,
    )
    .await
    {
        Ok(bytes) => {
            if serde_json::from_slice::<Value>(&bytes).is_err() {
                return upstream_error("entities/versions", anyhow::anyhow!("body is not JSON"));
            }
            let body = Arc::new(Bytes::from(bytes));
            state.registry.versions.insert(cache_key, body.clone());
            ([(CONTENT_TYPE, "application/json")], body.as_ref().clone()).into_response()
        }
        Err(err) => upstream_error("entities/versions", err),
    }
}

#[derive(Debug, Deserialize)]
pub struct ProfilesBody {
    #[serde(default)]
    pub ids: Option<Vec<String>>,
}

/// The address a document answers for: the full profile carries it in its
/// first avatar, the metadata projection as `pointer`.
fn profile_address(variant: &str, doc: &Value) -> Option<String> {
    if variant == "metadata" {
        return doc
            .get("pointer")
            .and_then(Value::as_str)
            .map(str::to_lowercase);
    }
    let avatar = doc.get("avatars")?.as_array()?.first()?;
    avatar
        .get("ethAddress")
        .or_else(|| avatar.get("userId"))
        .and_then(Value::as_str)
        .map(str::to_lowercase)
}

/// `POST /profiles` and `/profiles/metadata` (`{"ids": [addresses]}`): one
/// document per known address in request order, cached per address for a
/// minute; misses go upstream in one call and unknown addresses are
/// remembered as unknown.
async fn profiles_at(state: Shared, variant: &'static str, body: ProfilesBody) -> Response {
    let ids: Vec<String> = match body.ids {
        Some(ids) if !ids.is_empty() && ids.len() <= MAX_KEYS => {
            ids.iter().map(|s| s.trim().to_lowercase()).collect()
        }
        Some(ids) if ids.len() > MAX_KEYS => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("at most {MAX_KEYS} ids per request"),
            );
        }
        _ => return json_error(StatusCode::BAD_REQUEST, "ids must be a non-empty array"),
    };
    let mut seen = HashSet::new();
    let ids: Vec<String> = ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect();
    let cache = &state.registry.profiles;
    let key = |id: &str| format!("{variant}:{id}");

    let mut found: HashMap<String, Option<Arc<Bytes>>> = HashMap::new();
    let mut misses = Vec::new();
    for id in &ids {
        match cache.get_fresh(&key(id)) {
            Some(hit) => {
                found.insert(id.clone(), hit);
            }
            None => misses.push(id.clone()),
        }
    }
    if !misses.is_empty() {
        let path = if variant == "metadata" {
            "/profiles/metadata"
        } else {
            "/profiles"
        };
        let bytes = match post_upstream(
            &state,
            path,
            &[],
            json!({ "ids": misses }),
            PROFILES_BODY_CAP,
        )
        .await
        {
            Ok(b) => b,
            Err(err) => return upstream_error(path.trim_start_matches('/'), err),
        };
        let docs: Vec<Value> = match serde_json::from_slice(&bytes) {
            Ok(d) => d,
            Err(err) => {
                return upstream_error(
                    path.trim_start_matches('/'),
                    anyhow::anyhow!("body is not a profile array: {err}"),
                );
            }
        };
        for doc in docs {
            let Some(address) = profile_address(variant, &doc) else {
                continue;
            };
            let raw = Arc::new(Bytes::from(serde_json::to_vec(&doc).unwrap_or_default()));
            cache.insert(key(&address), Some(raw.clone()));
            found.insert(address, Some(raw));
        }
        for id in misses {
            if let Entry::Vacant(slot) = found.entry(id) {
                cache.insert(key(slot.key()), None);
                slot.insert(None);
            }
        }
    }
    let docs: Vec<Arc<Bytes>> = ids
        .iter()
        .filter_map(|id| found.get(id).cloned().flatten())
        .collect();
    json_array(&docs)
}

async fn profiles(State(state): State<Shared>, Json(body): Json<ProfilesBody>) -> Response {
    profiles_at(state, "full", body).await
}

async fn profiles_metadata(
    State(state): State<Shared>,
    Json(body): Json<ProfilesBody>,
) -> Response {
    profiles_at(state, "metadata", body).await
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn status(State(state): State<Shared>) -> Response {
    Json(json!({
        "data": {
            "version": state.settings.version,
            "currentTime": chrono::Utc::now().timestamp_millis(),
            "commitHash": state.settings.commit_hash,
        }
    }))
    .into_response()
}
