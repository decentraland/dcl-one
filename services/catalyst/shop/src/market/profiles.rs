//! `/lambdas/profiles`: the profile entity comes from `content.deployments`
//! (peer sync), every wearable and emote it names is checked against the
//! collections subgraphs and `hasClaimedName` against the marketplace
//! subgraph. With no reader configured the profile is served unfiltered,
//! as the old handler did with `squid_pool: None`; a reader that is
//! configured but unreachable fails the request closed (503) rather than
//! serving cosmetics nobody proved.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::content::queries::active_entity_docs_by_pointers;

use super::graph::{self, normalize_urn};
use super::{
    MarketplaceState, SharedState, bad_request, internal_server_error, not_found,
    service_unavailable,
};

/// Bounds staleness for changes that arrive on chain rather than through a
/// deployment (a wearable or name bought, sold or transferred).
pub const PROFILE_CACHE_TTL: Duration = Duration::from_secs(10);
pub const PROFILE_CACHE_MAX_ENTRIES: usize = 50_000;
const PROFILE_BATCH_MAX: usize = 50;
const PROFILE_IDS_MAX: usize = 1000;
const MISSING_PROFILE_IDS_MSG: &str =
    "No profile ids were specified. Expected ids:string[] in body";

fn is_base_wearable(urn: &str) -> bool {
    urn.contains("base-avatars")
}

/// Base-emote classification from @dcl/schemas 27.2.0 `isBaseEmote`.
pub fn is_base_emote(urn: &str) -> bool {
    let parts: Vec<_> = urn.split(':').collect();
    parts.len() == 5
        && parts[1] == "decentraland"
        && matches!(parts[3], "base-emotes" | "base-scene-emotes")
}

fn split_urn_and_token_id(urn: &str) -> (&str, Option<&str>) {
    let segment_count = urn.split(':').count();
    if segment_count == 7
        && !urn.contains("collections-thirdparty")
        && let Some(last_colon) = urn.rfind(':')
    {
        return (&urn[..last_colon], Some(&urn[last_colon + 1..]));
    }
    (urn, None)
}

fn collect_ownership_urns(metadata: &Value) -> Vec<String> {
    let mut to_check: Vec<String> = Vec::new();
    let Some(avatars) = metadata.get("avatars").and_then(|v| v.as_array()) else {
        return to_check;
    };
    for avatar_val in avatars {
        let Some(avatar_obj) = avatar_val.get("avatar") else {
            continue;
        };
        if let Some(wearables) = avatar_obj.get("wearables").and_then(|w| w.as_array()) {
            for wearable in wearables.iter().filter_map(Value::as_str) {
                if is_base_wearable(wearable) {
                    continue;
                }
                let (urn, _token_id) = split_urn_and_token_id(wearable);
                to_check.push(normalize_urn(urn));
            }
        }
        if let Some(emotes) = avatar_obj.get("emotes").and_then(|e| e.as_array()) {
            for emote_urn in emotes
                .iter()
                .filter_map(|e| e.get("urn").and_then(|u| u.as_str()))
            {
                if !emote_urn.contains(':') || is_base_emote(emote_urn) {
                    continue;
                }
                let (urn, _token_id) = split_urn_and_token_id(emote_urn);
                to_check.push(normalize_urn(urn));
            }
        }
    }
    to_check
}

fn filter_avatars_by_ownership(avatars: &mut [Value], owned: &HashSet<String>) {
    for avatar_val in avatars.iter_mut() {
        let Some(avatar_obj) = avatar_val.get_mut("avatar") else {
            continue;
        };
        if let Some(wearables) = avatar_obj
            .get("wearables")
            .and_then(|w| w.as_array())
            .cloned()
        {
            let validated: Vec<Value> = wearables
                .into_iter()
                .filter(|w| {
                    let Some(wearable) = w.as_str() else {
                        return false;
                    };
                    if is_base_wearable(wearable) {
                        return true;
                    }
                    let (urn, _) = split_urn_and_token_id(wearable);
                    owned.contains(&normalize_urn(urn))
                })
                .collect();
            avatar_obj["wearables"] = Value::Array(validated);
        }
        if let Some(emotes) = avatar_obj.get("emotes").and_then(|e| e.as_array()).cloned() {
            let validated: Vec<Value> = emotes
                .into_iter()
                .filter(|e| {
                    let Some(emote_urn) = e.get("urn").and_then(|u| u.as_str()) else {
                        return true;
                    };
                    if !emote_urn.contains(':') || is_base_emote(emote_urn) {
                        return true;
                    }
                    let (urn, _) = split_urn_and_token_id(emote_urn);
                    owned.contains(&normalize_urn(urn))
                })
                .collect();
            avatar_obj["emotes"] = Value::Array(validated);
        }
    }
}

pub fn apply_claimed_name(metadata: &mut Value, owned_names: &[String]) {
    let Some(avatars) = metadata.get_mut("avatars").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for avatar_val in avatars.iter_mut() {
        let claimed = avatar_val
            .get("name")
            .and_then(|v| v.as_str())
            .map(|name| owned_names.iter().any(|owned| owned == name))
            .unwrap_or(false);
        if let Some(obj) = avatar_val.as_object_mut() {
            obj.insert("hasClaimedName".to_string(), Value::Bool(claimed));
        }
    }
}

/// With `PROFILE_CDN_BASE_URL` the snapshots point at the CDN's
/// `entities/<id>/{face,body}.png`; without it the face and body point at this
/// node's `profile-images/entities/<id>/{face,body}.png`, uploaded or not:
/// sync and the read-through never copy a profile's snapshot files, so their
/// `contents/<hash>` would 404, while `/profile-images` serves the uploaded
/// file when this node holds it and renders or fetches the image otherwise.
/// Any other snapshot hash becomes this node's `contents/<hash>` URL (the
/// reference lambdas' behaviour).
pub fn rewrite_snapshot_urls(
    entity_id: &str,
    metadata: &mut Value,
    cdn_base: Option<&str>,
    content_public_url: &str,
    profile_images_public_url: &str,
) {
    let Some(avatars) = metadata.get_mut("avatars").and_then(|v| v.as_array_mut()) else {
        return;
    };
    let with_slash = |base: &str| {
        if base.ends_with('/') {
            base.to_string()
        } else {
            format!("{base}/")
        }
    };
    for avatar_val in avatars.iter_mut() {
        let Some(avatar_obj) = avatar_val.get_mut("avatar").and_then(|a| a.as_object_mut()) else {
            continue;
        };
        match cdn_base {
            Some(cdn) => {
                let base = with_slash(cdn);
                avatar_obj.insert(
                    "snapshots".to_string(),
                    json!({
                        "face256": format!("{base}entities/{entity_id}/face.png"),
                        "body": format!("{base}entities/{entity_id}/body.png"),
                    }),
                );
            }
            None => {
                let base = with_slash(content_public_url);
                let rendered = with_slash(profile_images_public_url);
                if !avatar_obj.get("snapshots").is_some_and(Value::is_object) {
                    avatar_obj.insert("snapshots".to_string(), json!({}));
                }
                if let Some(snapshots) = avatar_obj
                    .get_mut("snapshots")
                    .and_then(|s| s.as_object_mut())
                {
                    for value in snapshots.values_mut() {
                        if let Some(hash) = value
                            .as_str()
                            .filter(|h| !h.is_empty() && !h.contains("://"))
                        {
                            *value = Value::String(format!("{base}contents/{hash}"));
                        }
                    }
                    for (key, file) in [("face256", "face.png"), ("body", "body.png")] {
                        let url = snapshots
                            .get(key)
                            .and_then(Value::as_str)
                            .is_some_and(|v| v.contains("://") && !v.starts_with(&base));
                        if !url {
                            snapshots.insert(
                                key.to_string(),
                                Value::String(format!("{rendered}entities/{entity_id}/{file}")),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn link_url_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)^(?:https?)://[^\s/$.?#].[^\s]*$").expect("LinkUrl regex is valid")
    })
}

pub fn sanitize_links(metadata: &mut Value) {
    let Some(avatars) = metadata.get_mut("avatars").and_then(|v| v.as_array_mut()) else {
        return;
    };
    let re = link_url_regex();
    for avatar_val in avatars.iter_mut() {
        let Some(links) = avatar_val.get_mut("links").and_then(|l| l.as_array_mut()) else {
            continue;
        };
        let mut sanitized: Vec<Value> = Vec::new();
        for link in links.iter() {
            let Some(url) = link.get("url").and_then(|u| u.as_str()) else {
                continue;
            };
            if re.is_match(url) && crate::http::is_safe_http_url(url) {
                sanitized.push(link.clone());
                continue;
            }
            if let Ok(decoded) = urlencoding::decode(url)
                && re.is_match(&decoded)
                && crate::http::is_safe_http_url(&decoded)
            {
                let mut link_clone = link.clone();
                if let Some(obj) = link_clone.as_object_mut() {
                    obj.insert("url".to_string(), Value::String(decoded.into_owned()));
                }
                sanitized.push(link_clone);
            }
        }
        avatar_val["links"] = Value::Array(sanitized);
    }
}

pub fn ensure_profile_shape(entity: &Value, metadata: &mut Value) {
    if metadata.get("timestamp").is_none()
        && let Some(ts) = entity.get("timestamp")
    {
        metadata["timestamp"] = ts.clone();
    }
    if metadata.get("avatars").is_none() {
        metadata["avatars"] = Value::Array(vec![]);
    }
}

fn apply_pointer_identity(metadata: &mut Value, eth_address: &str) {
    if let Some(avatars) = metadata.get_mut("avatars").and_then(|v| v.as_array_mut()) {
        for avatar in avatars.iter_mut().filter(|a| a.is_object()) {
            avatar["userId"] = Value::String(eth_address.to_string());
            avatar["ethAddress"] = Value::String(eth_address.to_string());
        }
    }
}

pub fn entity_eth_address(entity: &Value) -> Option<String> {
    entity
        .get("pointers")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase())
}

/// One slot per entity (`None` without metadata): the pure passes, then one
/// ownership batch and one names batch per chunk of 128 when a reader is
/// configured. `Err` is a reader failure.
pub async fn process_profiles_positional(
    state: &MarketplaceState,
    entities: &[Value],
) -> Result<Vec<Option<Value>>, String> {
    let readers = graph::any_collections_reader(&state.index)
        || state.settings.marketplace_subgraph_url.is_some();
    state.warm_faces(
        entities
            .iter()
            .filter_map(|e| e.get("id").and_then(Value::as_str).map(str::to_string)),
    );
    let mut profiles = Vec::with_capacity(entities.len());
    for chunk in entities.chunks(128) {
        let mut prepared: Vec<Option<(String, Value)>> = chunk
            .iter()
            .map(|entity| {
                let mut metadata = entity.get("metadata")?.clone();
                let eid = entity.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let address = entity_eth_address(entity).unwrap_or_default();
                ensure_profile_shape(entity, &mut metadata);
                sanitize_links(&mut metadata);
                rewrite_snapshot_urls(
                    eid,
                    &mut metadata,
                    state.settings.profile_cdn_base_url.as_deref(),
                    &state.settings.content_public_url,
                    &state.settings.profile_images_public_url,
                );
                if !address.is_empty() && !address.starts_with("default") {
                    apply_pointer_identity(&mut metadata, &address);
                }
                Some((address, metadata))
            })
            .collect();
        if readers {
            let mut addresses: Vec<String> = Vec::new();
            let mut requested: HashSet<(String, String)> = HashSet::new();
            for (address, metadata) in prepared.iter().flatten() {
                if !address.starts_with("default") && !addresses.contains(address) {
                    addresses.push(address.clone());
                    requested.extend(
                        collect_ownership_urns(metadata)
                            .into_iter()
                            .map(|urn| (address.clone(), urn)),
                    );
                }
            }
            if !addresses.is_empty() {
                let names_futs = addresses
                    .iter()
                    .map(|a| graph::owned_names(&state.index, a));
                let (owned, names) = tokio::join!(
                    graph::owned_urns(&state.index, &requested),
                    futures::future::join_all(names_futs)
                );
                let owned = owned?;
                let mut names_by_address: HashMap<String, Vec<String>> = HashMap::new();
                for (address, names) in addresses.iter().zip(names) {
                    names_by_address.insert(
                        address.clone(),
                        names?.iter().map(|n| n.name.clone()).collect(),
                    );
                }
                let empty = HashSet::new();
                for (address, metadata) in prepared.iter_mut().flatten() {
                    if address.starts_with("default") {
                        continue;
                    }
                    if graph::any_collections_reader(&state.index)
                        && let Some(avatars) =
                            metadata.get_mut("avatars").and_then(Value::as_array_mut)
                    {
                        filter_avatars_by_ownership(avatars, owned.get(address).unwrap_or(&empty));
                    }
                    if state.settings.marketplace_subgraph_url.is_some() {
                        apply_claimed_name(
                            metadata,
                            names_by_address
                                .get(address)
                                .map(Vec::as_slice)
                                .unwrap_or(&[]),
                        );
                    }
                }
            }
        }
        profiles.extend(
            prepared
                .into_iter()
                .map(|p| p.map(|(_, metadata)| metadata)),
        );
    }
    Ok(profiles)
}

async fn active_entities(state: &MarketplaceState, pointers: &[String]) -> Result<Vec<Value>, ()> {
    let docs = match &state.read_through {
        Some(read_through) => read_through.active_docs(false, pointers).await,
        None => active_entity_docs_by_pointers(&state.pool, pointers).await,
    }
    .map_err(|e| {
        tracing::error!(error = %e, "profiles: active entities query failed");
    })?;
    Ok(docs.iter().filter_map(|d| d.to_value()).collect())
}

/// Processes a batch and fills the per-id cache (`Null` for absent pointers).
async fn process_and_cache_profiles(
    state: &MarketplaceState,
    pointers: &[String],
    entities: &[Value],
) -> Result<Vec<(String, Value)>, String> {
    let processed = process_profiles_positional(state, entities).await?;
    let mut out = Vec::with_capacity(entities.len());
    let mut filled: HashSet<String> = Default::default();
    for (entity, profile) in entities.iter().zip(processed) {
        let key = entity_eth_address(entity).unwrap_or_default();
        let profile = profile.unwrap_or(Value::Null);
        if !key.is_empty() && filled.insert(key.clone()) {
            state.profile_cache.insert(key.clone(), profile.clone());
        }
        if !profile.is_null() {
            out.push((key, profile));
        }
    }
    for pointer in pointers {
        if filled.insert(pointer.clone()) {
            state.profile_cache.insert(pointer.clone(), Value::Null);
        }
    }
    Ok(out)
}

enum Failure {
    Db,
    Reader(String),
}

impl Failure {
    fn into_response(self) -> Response {
        match self {
            Failure::Db => internal_server_error(),
            Failure::Reader(e) => {
                tracing::warn!(error = %e, "profiles: ownership reader failed");
                service_unavailable("ownership readers unavailable")
            }
        }
    }
}

/// Per-id hits are reused across id sets; only misses are fetched; answer in request order.
async fn profiles_via_cache(
    state: &MarketplaceState,
    pointers: &[String],
) -> Result<Vec<Value>, Failure> {
    let mut wanted: Vec<String> = Vec::with_capacity(pointers.len());
    for pointer in pointers {
        if !wanted.contains(pointer) {
            wanted.push(pointer.clone());
        }
    }
    let mut found: HashMap<String, Value> = Default::default();
    let mut missing: Vec<String> = Vec::new();
    for pointer in &wanted {
        match state.profile_cache.get_fresh(pointer) {
            Some(v) => {
                found.insert(pointer.clone(), v);
            }
            None => missing.push(pointer.clone()),
        }
    }
    if !missing.is_empty() {
        let entities = active_entities(state, &missing)
            .await
            .map_err(|()| Failure::Db)?;
        for (key, profile) in process_and_cache_profiles(state, &missing, &entities)
            .await
            .map_err(Failure::Reader)?
        {
            found.entry(key).or_insert(profile);
        }
    }
    Ok(wanted
        .iter()
        .filter_map(|p| found.get(p))
        .filter(|v| !v.is_null())
        .cloned()
        .collect())
}

fn is_default_profile_id(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    id == "default"
        || id
            .strip_prefix("default")
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

fn synthetic_default_profile(id: &str) -> Value {
    json!({
        "avatars": [{
            "userId": id,
            "name": id,
            "hasClaimedName": false,
            "ethAddress": id,
            "version": 0,
            "tutorialStep": 0,
            "avatar": {
                "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
                "wearables": [
                    "urn:decentraland:off-chain:base-avatars:eyebrows_00",
                    "urn:decentraland:off-chain:base-avatars:mouth_00",
                    "urn:decentraland:off-chain:base-avatars:casual_hair_01",
                    "urn:decentraland:off-chain:base-avatars:green_hoodie",
                    "urn:decentraland:off-chain:base-avatars:brown_pants",
                    "urn:decentraland:off-chain:base-avatars:sneakers"
                ],
                "emotes": [],
                "snapshots": { "face256": "", "body": "" },
                "eyes": { "color": { "r": 0.37, "g": 0.22, "b": 0.19 } },
                "hair": { "color": { "r": 0.6, "g": 0.46, "b": 0.27 } },
                "skin": { "color": { "r": 0.94, "g": 0.76, "b": 0.6 } }
            }
        }],
        "timestamp": 0
    })
}

async fn fetch_profile_for_id(
    state: &MarketplaceState,
    id: &str,
) -> Result<Option<Value>, Failure> {
    if is_default_profile_id(id) {
        return Ok(Some(synthetic_default_profile(id)));
    }
    let key = id.to_lowercase();
    let cached = state
        .profile_cache
        .get_or_fetch(key.clone(), || async {
            let entities = active_entities(state, std::slice::from_ref(&key))
                .await
                .map_err(|()| Failure::Db)?;
            let Some(entity) = entities.into_iter().next() else {
                return Ok(Value::Null);
            };
            let processed = process_profiles_positional(state, std::slice::from_ref(&entity))
                .await
                .map_err(Failure::Reader)?;
            Ok(processed
                .into_iter()
                .next()
                .flatten()
                .unwrap_or(Value::Null))
        })
        .await?;
    Ok(if cached.is_null() { None } else { Some(cached) })
}

#[derive(Debug, Deserialize)]
pub struct ProfilesRequest {
    #[serde(default)]
    pub ids: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Eq)]
enum ProfileIdsCheck {
    MissingOrEmpty,
    Valid,
    TooMany,
}

fn validate_profile_ids(ids: Option<&[String]>) -> ProfileIdsCheck {
    match ids {
        None | Some([]) => ProfileIdsCheck::MissingOrEmpty,
        Some(ids) if ids.len() > PROFILE_IDS_MAX => ProfileIdsCheck::TooMany,
        Some(_) => ProfileIdsCheck::Valid,
    }
}

fn early_profiles_response(check: ProfileIdsCheck) -> Option<Response> {
    match check {
        ProfileIdsCheck::MissingOrEmpty => Some(bad_request(MISSING_PROFILE_IDS_MSG)),
        ProfileIdsCheck::TooMany => Some(bad_request(&format!(
            "Too many profile ids were specified. Maximum allowed is {PROFILE_IDS_MAX}"
        ))),
        ProfileIdsCheck::Valid => None,
    }
}

pub async fn profiles(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let body: ProfilesRequest = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => return bad_request(&format!("Failed to parse the request body as JSON: {e}")),
    };
    if let Some(resp) = early_profiles_response(validate_profile_ids(body.ids.as_deref())) {
        return resp;
    }
    let ids = body
        .ids
        .expect("validate_profile_ids guarantees a non-empty id list");

    let modified_since: Option<i64> = headers
        .get("if-modified-since")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            chrono::DateTime::parse_from_rfc2822(s)
                .ok()
                .map(|dt| dt.timestamp_millis())
        });

    let pointers: Vec<String> = ids.iter().map(|id| id.to_lowercase()).collect();

    let cache_eligible = modified_since.is_none() && pointers.len() <= PROFILE_BATCH_MAX;
    if cache_eligible {
        return match profiles_via_cache(&state, &pointers).await {
            Ok(profiles) => Json(Value::Array(profiles)).into_response(),
            Err(f) => f.into_response(),
        };
    }
    let entities = match active_entities(&state, &pointers).await {
        Ok(e) => e,
        Err(()) => return internal_server_error(),
    };

    if let Some(threshold) = modified_since {
        let all_stale = entities.iter().all(|entity| {
            entity
                .get("timestamp")
                .and_then(|t| t.as_f64())
                .map(|ts| (ts as i64) <= threshold)
                .unwrap_or(false)
        });
        if all_stale && !entities.is_empty() {
            return StatusCode::NOT_MODIFIED.into_response();
        }
    }

    match process_and_cache_profiles(&state, &pointers, &entities).await {
        Ok(profiles) => {
            let profiles: Vec<Value> = profiles.into_iter().map(|(_, profile)| profile).collect();
            Json(Value::Array(profiles)).into_response()
        }
        Err(e) => Failure::Reader(e).into_response(),
    }
}

pub async fn profile_by_id(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    match fetch_profile_for_id(&state, &id).await {
        Ok(Some(metadata)) => Json(metadata).into_response(),
        Ok(None) => not_found("Profile not found"),
        Err(f) => f.into_response(),
    }
}

/// The singular alias never 404s: an unknown address is an empty profile.
pub async fn profile_alias(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    match fetch_profile_for_id(&state, &id).await {
        Ok(Some(processed)) => Json(processed).into_response(),
        Ok(None) => Json(json!({ "avatars": [], "timestamp": 0 })).into_response(),
        Err(f) => f.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A peer that holds nothing and counts the `/entities/active` reads it is asked.
    async fn counting_peer() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use axum::routing::post;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = calls.clone();
        let app = axum::Router::new().route(
            "/content/entities/active",
            post(move || async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Json(json!([]))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}/content"), calls)
    }

    #[tokio::test]
    async fn a_profile_this_node_does_not_hold_is_asked_of_the_read_through() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let (source, calls) = counting_peer().await;
        let tmp = std::env::temp_dir().join(format!(
            "catalyst-shop-read-through-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(crate::storage::Storage::from_config(&tmp).await.unwrap());
        let read_through = Arc::new(crate::content::read_through::ReadThrough::new(
            pool.clone(),
            storage,
            reqwest::Client::new(),
            vec![source],
            None,
            crate::content::read_through::Fills::new(1, std::time::Duration::from_secs(30)),
        ));
        let address = format!("0x{}{}", uuid::Uuid::new_v4().simple(), "00000000");

        let without = MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
        );
        assert!(
            active_entities(&without, std::slice::from_ref(&address))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);

        let with = without.with_read_through(Some(read_through));
        assert!(
            active_entities(&with, std::slice::from_ref(&address))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let _ = std::fs::remove_dir_all(tmp);
    }

    fn owned_set(urns: &[&str]) -> HashSet<String> {
        urns.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn split_urn_strips_token_id_for_7segment_collections_v2() {
        let (urn, tok) = split_urn_and_token_id("urn:decentraland:matic:collections-v2:0xabc:0:42");
        assert_eq!(urn, "urn:decentraland:matic:collections-v2:0xabc:0");
        assert_eq!(tok, Some("42"));
        let (urn, tok) = split_urn_and_token_id("urn:decentraland:matic:collections-v2:0xabc:0");
        assert_eq!(urn, "urn:decentraland:matic:collections-v2:0xabc:0");
        assert_eq!(tok, None);
        let tp = "urn:decentraland:matic:collections-thirdparty:tp:coll:item";
        assert_eq!(split_urn_and_token_id(tp).1, None);
    }

    #[test]
    fn schemas_27_2_base_emote_classification() {
        for collection in ["base-emotes", "base-scene-emotes"] {
            assert!(is_base_emote(&format!(
                "urn:decentraland:off-chain:{collection}:wave"
            )));
        }
        for urn in [
            "urn:decentraland:off-chain:base-avatars:wave",
            "urn:decentraland:off-chain:base-scene-emotes-fake:wave",
            "urn:decentraland:off-chain:base-scene-emotes:wave:extra",
            "urn:other:off-chain:base-scene-emotes:wave",
            "invalid",
        ] {
            assert!(!is_base_emote(urn), "{urn}");
        }
    }

    #[test]
    fn ownership_filter_keeps_base_keeps_owned_drops_unowned() {
        let owned = owned_set(&["urn:decentraland:matic:collections-v2:0xowned:0"]);
        let mut avatars = vec![json!({
            "avatar": {
                "wearables": [
                    "urn:decentraland:off-chain:base-avatars:eyebrows_00",
                    "urn:decentraland:matic:collections-v2:0xowned:0",
                    "urn:decentraland:matic:collections-v2:0xowned:0:99",
                    "urn:decentraland:matic:collections-v2:0xnope:0"
                ]
            }
        })];
        filter_avatars_by_ownership(&mut avatars, &owned);
        let ws: Vec<&str> = avatars[0]["avatar"]["wearables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            ws.len(),
            3,
            "unowned must be stripped, base+owned kept: {ws:?}"
        );
        assert!(
            ws.iter().all(|w| !w.contains("0xnope")),
            "unowned leaked: {ws:?}"
        );
        assert!(ws.contains(&"urn:decentraland:off-chain:base-avatars:eyebrows_00"));
    }

    #[test]
    fn ownership_filter_handles_emotes_and_mainnet_normalization() {
        let owned = owned_set(&["urn:decentraland:ethereum:collections-v1:0xc:dance"]);
        let mut avatars = vec![json!({
            "avatar": {
                "emotes": [
                    {"slot": 0, "urn": "handsair"},
                    {"slot": 1, "urn": "urn:decentraland:off-chain:base-emotes:wave"},
                    {"slot": 2, "urn": "urn:decentraland:mainnet:collections-v1:0xc:dance"},
                    {"slot": 3, "urn": "urn:decentraland:matic:collections-v2:0xx:1"}
                ]
            }
        })];
        filter_avatars_by_ownership(&mut avatars, &owned);
        let es = avatars[0]["avatar"]["emotes"].as_array().unwrap();
        let urns: Vec<&str> = es.iter().map(|e| e["urn"].as_str().unwrap()).collect();
        assert_eq!(es.len(), 3, "unowned emote must be stripped: {urns:?}");
        assert!(
            urns.iter().all(|u| !u.contains("0xx")),
            "unowned emote leaked: {urns:?}"
        );
        assert!(
            urns.contains(&"handsair")
                && urns.contains(&"urn:decentraland:off-chain:base-emotes:wave")
        );
    }

    #[test]
    fn collect_ownership_urns_skips_base_items_and_strips_token_ids() {
        let metadata = json!({ "avatars": [{ "avatar": {
            "wearables": [
                "urn:decentraland:off-chain:base-avatars:eyebrows_00",
                "urn:decentraland:mainnet:collections-v1:0xc:hat",
                "urn:decentraland:matic:collections-v2:0xabc:0:42"
            ],
            "emotes": [ {"urn": "wave"}, {"urn": "urn:decentraland:off-chain:base-emotes:wave"}, {"urn": "urn:decentraland:matic:collections-v2:0xabc:9"} ]
        } }] });
        assert_eq!(
            collect_ownership_urns(&metadata),
            vec![
                "urn:decentraland:ethereum:collections-v1:0xc:hat",
                "urn:decentraland:matic:collections-v2:0xabc:0",
                "urn:decentraland:matic:collections-v2:0xabc:9",
            ]
        );
    }

    #[test]
    fn claimed_name_follows_the_owned_names() {
        let mut metadata =
            json!({ "avatars": [{ "name": "alice" }, { "name": "mallory" }, { "x": 1 }] });
        apply_claimed_name(&mut metadata, &["alice".to_string()]);
        assert_eq!(metadata["avatars"][0]["hasClaimedName"], true);
        assert_eq!(metadata["avatars"][1]["hasClaimedName"], false);
        assert_eq!(metadata["avatars"][2]["hasClaimedName"], false);
    }

    #[test]
    fn snapshots_point_at_the_cdn_or_at_our_profile_images() {
        let mut m = json!({ "avatars": [{ "avatar": { "snapshots": { "face256": "bafyface", "body": "bafybody" } } }] });
        rewrite_snapshot_urls(
            "Qm1",
            &mut m,
            Some("https://cdn.example"),
            "http://localhost/content/",
            "http://localhost/profile-images/",
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["face256"],
            "https://cdn.example/entities/Qm1/face.png"
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["body"],
            "https://cdn.example/entities/Qm1/body.png"
        );

        let mut m = json!({ "avatars": [{ "avatar": { "snapshots": { "face256": "bafyface", "body": "https://x/already.png" } } }] });
        rewrite_snapshot_urls(
            "Qm1",
            &mut m,
            None,
            "http://localhost/content",
            "http://localhost/profile-images",
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["face256"],
            "http://localhost/profile-images/entities/Qm1/face.png",
            "an uploaded snapshot is served by /profile-images, which holds or derives it"
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["body"], "https://x/already.png",
            "a full URL is left alone"
        );

        let mut m =
            json!({ "avatars": [{ "avatar": { "snapshots": { "face128": "bafyface128" } } }] });
        rewrite_snapshot_urls(
            "Qm1",
            &mut m,
            None,
            "http://localhost/content",
            "http://localhost/profile-images",
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["face128"],
            "http://localhost/content/contents/bafyface128"
        );
    }

    #[test]
    fn snapshots_a_profile_never_uploaded_point_at_our_renders() {
        let mut m = json!({ "avatars": [{ "avatar": { "bodyShape": "urn:x" } }] });
        rewrite_snapshot_urls(
            "Qm1",
            &mut m,
            None,
            "http://localhost/content/",
            "http://localhost/profile-images/",
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["face256"],
            "http://localhost/profile-images/entities/Qm1/face.png"
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["body"],
            "http://localhost/profile-images/entities/Qm1/body.png"
        );

        let mut m = json!({ "avatars": [{ "avatar": { "snapshots": { "face256": "bafyface", "body": "" } } }] });
        rewrite_snapshot_urls(
            "Qm1",
            &mut m,
            None,
            "http://localhost/content/",
            "http://localhost/profile-images/",
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["face256"],
            "http://localhost/profile-images/entities/Qm1/face.png"
        );
        assert_eq!(
            m["avatars"][0]["avatar"]["snapshots"]["body"],
            "http://localhost/profile-images/entities/Qm1/body.png"
        );
    }

    #[test]
    fn links_keep_only_safe_http_targets() {
        let mut m = json!({ "avatars": [{ "links": [
            { "title": "ok", "url": "https://decentraland.org/x" },
            { "title": "encoded", "url": "https%3A%2F%2Fdecentraland.org%2Fy" },
            { "title": "js", "url": "javascript:alert(1)" },
            { "title": "internal", "url": "http://127.0.0.1/" },
            { "title": "none" }
        ] }] });
        sanitize_links(&mut m);
        let links = m["avatars"][0]["links"].as_array().unwrap();
        assert_eq!(links.len(), 2, "{links:?}");
        assert_eq!(links[1]["url"], "https://decentraland.org/y");
    }

    #[test]
    fn validate_profile_ids_boundaries() {
        assert_eq!(validate_profile_ids(None), ProfileIdsCheck::MissingOrEmpty);
        let empty: Vec<String> = Vec::new();
        assert_eq!(
            validate_profile_ids(Some(&empty)),
            ProfileIdsCheck::MissingOrEmpty
        );
        let ids = vec!["0xabc".to_string()];
        assert_eq!(validate_profile_ids(Some(&ids)), ProfileIdsCheck::Valid);
        let ids: Vec<String> = (0..PROFILE_IDS_MAX).map(|i| format!("0x{i}")).collect();
        assert_eq!(validate_profile_ids(Some(&ids)), ProfileIdsCheck::Valid);
        let ids: Vec<String> = (0..=PROFILE_IDS_MAX).map(|i| format!("0x{i}")).collect();
        assert_eq!(validate_profile_ids(Some(&ids)), ProfileIdsCheck::TooMany);
        const { assert!(PROFILE_BATCH_MAX < PROFILE_IDS_MAX) };
        let ids: Vec<String> = (0..=PROFILE_BATCH_MAX).map(|i| format!("0x{i}")).collect();
        assert_eq!(
            validate_profile_ids(Some(&ids)),
            ProfileIdsCheck::Valid,
            "the batch max is only cache eligibility"
        );
    }

    #[test]
    fn early_profiles_response_is_400_for_missing_and_empty_ids() {
        assert_eq!(
            early_profiles_response(validate_profile_ids(Some(&[])))
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            early_profiles_response(validate_profile_ids(None))
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        let ids = vec!["0xabc".to_string()];
        assert!(early_profiles_response(validate_profile_ids(Some(&ids))).is_none());
    }

    #[test]
    fn default_profile_ids() {
        assert!(is_default_profile_id("default"));
        assert!(is_default_profile_id("Default12"));
        assert!(!is_default_profile_id("default_x"));
        assert!(!is_default_profile_id("0xabc"));
        assert_eq!(
            synthetic_default_profile("default3")["avatars"][0]["userId"],
            "default3"
        );
    }
}
