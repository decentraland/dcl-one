//! The places admin HTTP routes (the web tier's moderation page and the
//! Foundation's tools), with the old service's three credentials and their
//! exact answers:
//!
//! - the admin bearer (`PLACES_ADMIN_AUTH_TOKEN`): the reports queue, the
//!   POIs, `PATCH .../disable` (403 "Admin token not configured" or
//!   "Invalid admin credentials"), featured and the ranking exclusion (401
//!   "Invalid authentication");
//! - the admin bearer or the data team's (`DATA_TEAM_AUTH_TOKEN`): the
//!   rankings (401 otherwise), where only the admin bearer may move a
//!   highlighted or excluded row's ranking (403 with the editorial message),
//!   and the admin bearer's non-zero ranking of a row carrying neither flag
//!   is a 400 (the nightly `PUT /destinations/ranking` would clear it; 0 and
//!   `null` survive that clear and pass);
//! - the admin bearer or a signed fetch by a `PLACES_ADMIN_ADDRESSES`
//!   wallet: highlight, rating and `PUT .../disable` (403 "Only admin allowed
//!   to update {what}").
//!
//! Every write is an operator decision behind `OPS_WRITES_ENABLED`: disable,
//! highlight and rating through `places::moderation` (the console's writer),
//! the rankings through `places::ranking`; both are overlays the mirror
//! re-applies. A POI is its parcel in the `dcl-lists` `pois` list, as the
//! console keeps it (`explore::api::lists`): the old row's title,
//! description and entity id have nowhere to be served and answer `null`.
//! The content rating is one of v2's `RATINGS` (`RP`, the ESRB code the
//! catalogue carries, where the old route's list read `PR`).

use std::collections::HashSet;

use axum::Json;
use axum::extract::{FromRequestParts, OriginalUri, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::moderation::{self, Decision, Target};
use super::ranking::{self, RankingEntry, ReplaceRankingResult, ScoreRankingOutcome};
use super::{PlaceRow, WorldRow};
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiError};

pub const HIGHLIGHTED_RANKING_IS_EDITORIAL: &str =
    "The ranking of a highlighted entity is editorial and can only be changed with the admin token";
pub const EXCLUDED_RANKING_IS_EDITORIAL: &str = "This entity is excluded from the automated ranking and its ranking can only be changed with the admin token";
pub const PLACE_RANKING_IS_EDITORIAL: &str =
    "The ranking of this place is editorial and can only be changed with the admin token";
pub const WORLD_RANKING_IS_EDITORIAL: &str =
    "The ranking of this world is editorial and can only be changed with the admin token";
pub const EDITORIAL_RANKING_NEEDS_CURATION: &str = "An editorial ranking only holds on a curated entity. Feature it or set exclude_from_ranking first, otherwise the nightly replace clears this value.";
const RANKING_NOT_WRITABLE: &str = "ranking is not writable for this destination on this server";
const EXCLUSION_NOT_WRITABLE: &str =
    "ranking exclusion is not writable for this destination on this server";

/// The audit actor of a token-authorised write: the credential's name, never anything the client sent.
const ADMIN_TOKEN_ACTOR: &str = "token:PLACES_ADMIN_AUTH_TOKEN";
const DATA_TEAM_ACTOR: &str = "token:DATA_TEAM_AUTH_TOKEN";

pub use crate::auth::admin_bearer::bearer_token;

/// `Some(true)`: the presented secret is the configured one; `None`: none configured.
fn secret_matches(expected: Option<&str>, presented: Option<&str>) -> Option<bool> {
    match crate::auth::admin_bearer::check_token(expected, presented) {
        Ok(()) => Some(true),
        Err(crate::auth::admin_bearer::Refusal::NotConfigured) => None,
        Err(_) => Some(false),
    }
}

pub fn is_admin_token(headers: &HeaderMap, admin: Option<&str>) -> bool {
    secret_matches(admin, bearer_token(headers)) == Some(true)
}

pub fn require_bearer_token(headers: &HeaderMap, admin: Option<&str>) -> Result<(), ApiError> {
    if is_admin_token(headers, admin) {
        Ok(())
    } else {
        Err(ApiError::unauthorized("Invalid authentication"))
    }
}

pub fn require_ranking_token(
    headers: &HeaderMap,
    data_team: Option<&str>,
    admin: Option<&str>,
) -> Result<(), ApiError> {
    let presented = bearer_token(headers);
    if secret_matches(data_team, presented) == Some(true)
        || secret_matches(admin, presented) == Some(true)
    {
        Ok(())
    } else {
        Err(ApiError::unauthorized("Invalid authentication"))
    }
}

/// The replace recognises curation by the two flags alone, so the admin
/// token's hand-set value on an unflagged row would be written and gone by
/// the next nightly run: refused unless it is a clear (0 or `null`).
pub fn require_admin_token_for_curated_ranking(
    headers: &HeaderMap,
    admin: Option<&str>,
    highlighted: bool,
    exclude_from_ranking: bool,
    ranking: Option<f64>,
) -> Result<(), ApiError> {
    let admin_token = is_admin_token(headers, admin);
    if !highlighted && !exclude_from_ranking {
        if admin_token && ranking.is_some_and(|r| r != 0.0) {
            return Err(ApiError::bad_request(EDITORIAL_RANKING_NEEDS_CURATION));
        }
        return Ok(());
    }
    if admin_token {
        return Ok(());
    }
    Err(ApiError::forbidden(if highlighted {
        HIGHLIGHTED_RANKING_IS_EDITORIAL
    } else {
        EXCLUDED_RANKING_IS_EDITORIAL
    }))
}

pub fn require_admin_bearer(headers: &HeaderMap, admin: Option<&str>) -> Result<(), ApiError> {
    match secret_matches(admin, bearer_token(headers)) {
        Some(true) => Ok(()),
        None => Err(ApiError::forbidden("Admin token not configured")),
        Some(false) => Err(ApiError::forbidden("Invalid admin credentials")),
    }
}

/// The admin bearer, as an extractor (the reports queue and the POIs).
pub struct RequireAdmin(());

impl FromRequestParts<SharedState> for RequireAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        require_admin_bearer(&parts.headers, state.settings.places_admin_token.as_deref())?;
        Ok(RequireAdmin(()))
    }
}

/// The admin bearer when one is presented, else a signed fetch by an admin
/// wallet. The audit actor it answers is the credential's name or the wallet.
async fn require_admin(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    action: &str,
) -> Result<String, ApiError> {
    if bearer_token(headers).is_some() {
        require_admin_bearer(headers, state.settings.places_admin_token.as_deref())?;
        return Ok(ADMIN_TOKEN_ACTOR.to_string());
    }
    let user = state.signer(headers, method, path).await?;
    if state
        .settings
        .places_admin_addresses
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&user))
    {
        Ok(user)
    } else {
        Err(ApiError::forbidden(format!(
            "Only admin allowed to update {action}"
        )))
    }
}

fn ranking_actor(headers: &HeaderMap, admin: Option<&str>) -> &'static str {
    if is_admin_token(headers, admin) {
        ADMIN_TOKEN_ACTOR
    } else {
        DATA_TEAM_ACTOR
    }
}

async fn fetch_place(state: &SharedState, place_id: &str) -> Result<PlaceRow, ApiError> {
    state
        .places
        .find_by_id_for(place_id, None)
        .await?
        .filter(|p| !p.world)
        .ok_or_else(|| ApiError::not_found(format!("Not found place \"{place_id}\"")))
}

async fn fetch_world(state: &SharedState, world_id: &str) -> Result<PlaceRow, ApiError> {
    state
        .places
        .find_world_by_id_for(world_id, None)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Not found world \"{world_id}\"")))
}

fn body_field<'a>(body: &'a Option<Json<Value>>, key: &str) -> Option<&'a Value> {
    body.as_ref().and_then(|Json(v)| v.get(key))
}

pub fn body_ranking(body: &Option<Json<Value>>) -> Result<Option<f64>, ApiError> {
    let invalid =
        || ApiError::bad_request("Invalid ranking body. Expected { ranking: number|null }.");
    let v = body_field(body, "ranking").ok_or_else(invalid)?;
    if v.is_null() {
        Ok(None)
    } else {
        v.as_f64().map(Some).ok_or_else(invalid)
    }
}

pub fn body_disabled(body: &Option<Json<Value>>) -> Result<bool, ApiError> {
    body_field(body, "disabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            ApiError::bad_request("Invalid disable body. Expected { disabled: boolean }.")
        })
}

fn body_highlighted(body: &Option<Json<Value>>) -> Result<bool, ApiError> {
    body_field(body, "highlighted")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            ApiError::bad_request("Invalid highlight body. Expected { highlighted: boolean }.")
        })
}

fn body_rating(body: &Option<Json<Value>>) -> Result<Decision, ApiError> {
    let raw = body_field(body, "content_rating")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("content rating body needed"))?;
    Decision::rating(raw)
}

async fn decide(
    state: &SharedState,
    target: Target<'_>,
    decision: &Decision,
    actor: &str,
    not_found: impl FnOnce() -> ApiError,
) -> Result<(), ApiError> {
    match moderation::apply(&state.pool, target, decision, actor).await? {
        Some(_) => Ok(()),
        None => Err(not_found()),
    }
}

pub async fn put_place_rating(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    state.operator_writes()?;
    let actor = require_admin(&state, &headers, method.as_str(), uri.path(), "rating").await?;
    let decision = body_rating(&body)?;
    let mut place = fetch_place(&state, &place_id).await?;
    decide(&state, Target::Place(&place.id), &decision, &actor, || {
        ApiError::not_found(format!("Not found place \"{place_id}\""))
    })
    .await?;
    if let Decision::Rating(r) = decision {
        place.content_rating = Some(r);
    }
    Ok(Json(ApiData::ok(place)))
}

pub async fn put_world_rating(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    state.operator_writes()?;
    let actor = require_admin(&state, &headers, method.as_str(), uri.path(), "rating").await?;
    let decision = body_rating(&body)?;
    let mut world = fetch_world(&state, &world_id).await?;
    decide(&state, Target::World(&world.id), &decision, &actor, || {
        ApiError::not_found(format!("Not found world \"{world_id}\""))
    })
    .await?;
    if let Decision::Rating(r) = decision {
        world.content_rating = Some(r);
    }
    Ok(Json(ApiData::ok(WorldRow::from(world))))
}

pub async fn put_place_highlight(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    state.operator_writes()?;
    let actor = require_admin(&state, &headers, method.as_str(), uri.path(), "highlight").await?;
    let highlighted = body_highlighted(&body)?;
    let mut place = fetch_place(&state, &place_id).await?;
    decide(
        &state,
        Target::Place(&place.id),
        &Decision::Highlighted(highlighted),
        &actor,
        || ApiError::not_found(format!("Not found place \"{place_id}\"")),
    )
    .await?;
    place.highlighted = highlighted;
    Ok(Json(ApiData::ok(place)))
}

pub async fn put_world_highlight(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    state.operator_writes()?;
    let actor = require_admin(&state, &headers, method.as_str(), uri.path(), "highlight").await?;
    let highlighted = body_highlighted(&body)?;
    let mut world = fetch_world(&state, &world_id).await?;
    decide(
        &state,
        Target::World(&world.id),
        &Decision::Highlighted(highlighted),
        &actor,
        || ApiError::not_found(format!("Not found world \"{world_id}\"")),
    )
    .await?;
    world.highlighted = highlighted;
    Ok(Json(ApiData::ok(WorldRow::from(world))))
}

/// `PUT /api/places/{id}/disable` `{ disabled }`: an admin wallet or the bearer.
pub async fn put_place_disable(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    state.operator_writes()?;
    let actor = require_admin(&state, &headers, method.as_str(), uri.path(), "disabled").await?;
    let disabled = body_disabled(&body)?;
    let mut place = fetch_place(&state, &place_id).await?;
    let decision = Decision::Disabled {
        disabled,
        reason: disabled.then(|| "moderation".to_string()),
    };
    decide(&state, Target::Place(&place.id), &decision, &actor, || {
        ApiError::not_found(format!("Not found place \"{place_id}\""))
    })
    .await?;
    let now = Utc::now();
    place.disabled = disabled;
    place.disabled_at = disabled.then_some(now);
    place.disabled_reason = disabled.then(|| "moderation".to_string());
    place.updated_at = Some(now);
    Ok(Json(ApiData::ok(place)))
}

#[derive(Debug, Deserialize)]
pub struct DisablePlace {
    pub disabled: Option<bool>,
    pub reason: Option<String>,
}

/// `PATCH /api/places/{id}/disable` `{ disabled?, reason? }` (the admin bearer; no body disables).
pub async fn patch_place_disable(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    Path(place_id): Path<String>,
    body: Option<Json<DisablePlace>>,
) -> Result<Json<Value>, ApiError> {
    state.operator_writes()?;
    let (disabled, reason) = match body {
        Some(Json(b)) => (b.disabled.unwrap_or(true), b.reason),
        None => (true, None),
    };
    decide(
        &state,
        Target::Place(&place_id),
        &Decision::Disabled { disabled, reason },
        ADMIN_TOKEN_ACTOR,
        || ApiError::not_found(format!("Not found place \"{place_id}\"")),
    )
    .await?;
    Ok(Json(
        json!({ "ok": true, "data": { "id": place_id, "disabled": disabled } }),
    ))
}

async fn set_featured(
    state: &SharedState,
    headers: &HeaderMap,
    target: Target<'_>,
    featured: bool,
) -> Result<PlaceRow, ApiError> {
    state.operator_writes()?;
    require_bearer_token(headers, state.settings.places_admin_token.as_deref())?;
    let mut row = match target {
        Target::Place(id) => fetch_place(state, id).await?,
        Target::World(id) => fetch_world(state, id).await?,
    };
    let row_target = match target {
        Target::Place(_) => Target::Place(&row.id),
        Target::World(_) => Target::World(&row.id),
    };
    let missing = row.id.clone();
    decide(
        state,
        row_target,
        &Decision::Highlighted(featured),
        ADMIN_TOKEN_ACTOR,
        || ApiError::not_found(format!("Not found \"{missing}\"")),
    )
    .await?;
    row.highlighted = featured;
    Ok(row)
}

pub async fn put_place_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    Ok(Json(ApiData::ok(
        set_featured(&state, &headers, Target::Place(&place_id), true).await?,
    )))
}

pub async fn delete_place_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    Ok(Json(ApiData::ok(
        set_featured(&state, &headers, Target::Place(&place_id), false).await?,
    )))
}

pub async fn put_world_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    Ok(Json(ApiData::ok(WorldRow::from(
        set_featured(&state, &headers, Target::World(&world_id), true).await?,
    ))))
}

pub async fn delete_world_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    Ok(Json(ApiData::ok(WorldRow::from(
        set_featured(&state, &headers, Target::World(&world_id), false).await?,
    ))))
}

async fn set_ranking(
    state: &SharedState,
    headers: &HeaderMap,
    row: &mut PlaceRow,
    ranking: Option<f64>,
    editorial: &'static str,
) -> Result<(), ApiError> {
    let admin = state.settings.places_admin_token.as_deref();
    require_admin_token_for_curated_ranking(
        headers,
        admin,
        row.highlighted,
        row.exclude_from_ranking,
        ranking,
    )?;
    let actor = ranking_actor(headers, admin);
    if is_admin_token(headers, admin) {
        if ranking::set_ranking(&state.pool, &row.id, ranking, actor).await? == 0 {
            return Err(ApiError::service_unavailable(RANKING_NOT_WRITABLE));
        }
    } else {
        match ranking::set_ranking_from_score(&state.pool, &row.id, ranking, actor).await? {
            ScoreRankingOutcome::RefusedCurated => return Err(ApiError::forbidden(editorial)),
            ScoreRankingOutcome::NotWritable => {
                return Err(ApiError::service_unavailable(RANKING_NOT_WRITABLE));
            }
            ScoreRankingOutcome::Stored => {}
        }
    }
    row.ranking = ranking;
    Ok(())
}

pub async fn put_place_ranking(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<(StatusCode, Json<ApiData<PlaceRow>>), ApiError> {
    state.operator_writes()?;
    require_ranking_token(
        &headers,
        state.settings.data_team_token.as_deref(),
        state.settings.places_admin_token.as_deref(),
    )?;
    let ranking = body_ranking(&body)?;
    let mut place = fetch_place(&state, &place_id).await?;
    set_ranking(
        &state,
        &headers,
        &mut place,
        ranking,
        PLACE_RANKING_IS_EDITORIAL,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiData::ok(place))))
}

pub async fn put_world_ranking(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<(StatusCode, Json<ApiData<WorldRow>>), ApiError> {
    state.operator_writes()?;
    require_ranking_token(
        &headers,
        state.settings.data_team_token.as_deref(),
        state.settings.places_admin_token.as_deref(),
    )?;
    let ranking = body_ranking(&body)?;
    let mut world = fetch_world(&state, &world_id).await?;
    set_ranking(
        &state,
        &headers,
        &mut world,
        ranking,
        WORLD_RANKING_IS_EDITORIAL,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiData::ok(WorldRow::from(world))),
    ))
}

async fn set_exclusion(
    state: &SharedState,
    row: &mut PlaceRow,
    exclude: bool,
) -> Result<(), ApiError> {
    if ranking::set_exclusion(&state.pool, &row.id, exclude, ADMIN_TOKEN_ACTOR).await? == 0 {
        return Err(ApiError::service_unavailable(EXCLUSION_NOT_WRITABLE));
    }
    row.exclude_from_ranking = exclude;
    if exclude {
        row.ranking = Some(0.0);
    }
    Ok(())
}

async fn place_exclusion(
    state: SharedState,
    headers: HeaderMap,
    place_id: String,
    exclude: bool,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    state.operator_writes()?;
    require_bearer_token(&headers, state.settings.places_admin_token.as_deref())?;
    let mut place = fetch_place(&state, &place_id).await?;
    set_exclusion(&state, &mut place, exclude).await?;
    Ok(Json(ApiData::ok(place)))
}

async fn world_exclusion(
    state: SharedState,
    headers: HeaderMap,
    world_id: String,
    exclude: bool,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    state.operator_writes()?;
    require_bearer_token(&headers, state.settings.places_admin_token.as_deref())?;
    let mut world = fetch_world(&state, &world_id).await?;
    set_exclusion(&state, &mut world, exclude).await?;
    Ok(Json(ApiData::ok(WorldRow::from(world))))
}

pub async fn put_place_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    place_exclusion(state, headers, place_id, true).await
}

pub async fn delete_place_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    place_exclusion(state, headers, place_id, false).await
}

pub async fn put_world_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    world_exclusion(state, headers, world_id, true).await
}

pub async fn delete_world_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    world_exclusion(state, headers, world_id, false).await
}

pub const MAX_RANKING_ENTRIES: usize = 5000;
const MAX_ID_LEN: usize = 255;
const BODY_SHAPE: &str = "Invalid ranking body. Expected { replaces?: (\"place\"|\"world\")[], entries: [{ entity_type: \"place\"|\"world\", id: string, ranking: number }] }.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Place,
    World,
}

impl Kind {
    fn parse(value: Option<&Value>) -> Option<Kind> {
        match value.and_then(Value::as_str) {
            Some("place") => Some(Kind::Place),
            Some("world") => Some(Kind::World),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Place => "place",
            Kind::World => "world",
        }
    }
}

struct SubmittedEntry {
    kind: Kind,
    id: String,
    ranking: f64,
}

struct RankingBody {
    replaces: Option<Vec<Kind>>,
    entries: Vec<SubmittedEntry>,
}

fn entry_key(entry: &SubmittedEntry) -> String {
    format!("{}:{}", entry.kind.label(), entry.id)
}

fn parse_entry(value: &Value) -> Result<SubmittedEntry, ApiError> {
    let object = value
        .as_object()
        .ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
    if object
        .keys()
        .any(|k| !matches!(k.as_str(), "entity_type" | "id" | "ranking"))
    {
        return Err(ApiError::bad_request(BODY_SHAPE));
    }
    let kind =
        Kind::parse(object.get("entity_type")).ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
    let length = id.chars().count();
    if length == 0 || length > MAX_ID_LEN {
        return Err(ApiError::bad_request(format!(
            "A destination id must be 1 to {MAX_ID_LEN} characters"
        )));
    }
    let ranking = object
        .get("ranking")
        .and_then(Value::as_f64)
        .ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
    if !ranking.is_finite() || ranking < 0.0 {
        return Err(ApiError::bad_request(
            "A ranking must be a number greater than or equal to 0",
        ));
    }
    Ok(SubmittedEntry {
        kind,
        id: id.to_string(),
        ranking,
    })
}

/// `replaces`: at most both types, each once; `null` reads as absent.
fn parse_replaces(value: Option<&Value>) -> Result<Option<Vec<Kind>>, ApiError> {
    let items = match value {
        None | Some(Value::Null) => return Ok(None),
        Some(v) => v
            .as_array()
            .ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?,
    };
    if items.len() > 2 {
        return Err(ApiError::bad_request(BODY_SHAPE));
    }
    let mut kinds = Vec::with_capacity(items.len());
    for item in items {
        let kind = Kind::parse(Some(item)).ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
        if kinds.contains(&kind) {
            return Err(ApiError::bad_request(BODY_SHAPE));
        }
        kinds.push(kind);
    }
    Ok(Some(kinds))
}

fn parse_ranking_body(body: &Value) -> Result<RankingBody, ApiError> {
    let object = body
        .as_object()
        .ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
    if object.keys().any(|k| k != "entries" && k != "replaces") {
        return Err(ApiError::bad_request(BODY_SHAPE));
    }
    let replaces = parse_replaces(object.get("replaces"))?;
    let items = object
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request(BODY_SHAPE))?;
    if items.len() > MAX_RANKING_ENTRIES {
        return Err(ApiError::bad_request(format!(
            "Too many entries (max {MAX_RANKING_ENTRIES})"
        )));
    }
    Ok(RankingBody {
        replaces,
        entries: items.iter().map(parse_entry).collect::<Result<_, _>>()?,
    })
}

/// The legs a run may clear: the `replaces` it declares, else the types its
/// entries carry, so a one-type payload (or a run whose other half failed to
/// build) never empties the other table. An entry outside a declared scope
/// would write into a table the run declared out of it: refused.
fn replace_scope(
    replaces: Option<&[Kind]>,
    entries: &[SubmittedEntry],
) -> Result<ranking::ReplaceScope, ApiError> {
    let declared: Vec<Kind> = replaces
        .map(<[Kind]>::to_vec)
        .unwrap_or_else(|| entries.iter().map(|e| e.kind).collect());
    let mut outside: Vec<&str> = Vec::new();
    for entry in entries {
        if !declared.contains(&entry.kind) && !outside.contains(&entry.kind.label()) {
            outside.push(entry.kind.label());
        }
    }
    if !outside.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Entries name types this run does not declare in \"replaces\": {}",
            outside.join(", ")
        )));
    }
    Ok(ranking::ReplaceScope {
        places: declared.contains(&Kind::Place),
        worlds: declared.contains(&Kind::World),
    })
}

fn duplicated_keys(entries: &[SubmittedEntry]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut duplicated: Vec<String> = Vec::new();
    for key in entries.iter().map(entry_key) {
        if !seen.insert(key.clone()) && !duplicated.contains(&key) {
            duplicated.push(key);
        }
    }
    duplicated
}

fn leg(entries: &[SubmittedEntry], kind: Kind) -> Vec<RankingEntry> {
    entries
        .iter()
        .filter(|e| e.kind == kind)
        .map(|e| RankingEntry {
            id: e.id.clone(),
            ranking: e.ranking,
        })
        .collect()
}

/// `PUT /api/destinations/ranking`: the whole automated set of each type the
/// run replaces (`replaces`, else the types its entries carry), named
/// rankings written and every other automated one of that type cleared;
/// curated rows are never touched, by either token (this route buys the
/// admin bearer no override). A 400 writes nothing: a duplicate, an entry
/// outside `replaces`, or a type whose every named destination is unknown.
pub async fn put_destinations_ranking(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Result<(StatusCode, Json<ApiData<ReplaceRankingResult>>), ApiError> {
    state.operator_writes()?;
    let admin = state.settings.places_admin_token.as_deref();
    require_ranking_token(&headers, state.settings.data_team_token.as_deref(), admin)?;
    let body = body
        .map(|Json(v)| v)
        .unwrap_or_else(|| Value::Object(Default::default()));
    let RankingBody { replaces, entries } = parse_ranking_body(&body)?;
    let duplicated = duplicated_keys(&entries);
    if !duplicated.is_empty() {
        return Err(ApiError::bad_request(format!(
            "The same destination appears more than once: {}",
            duplicated.join(", ")
        )));
    }
    let scope = replace_scope(replaces.as_deref(), &entries)?;
    let result = ranking::replace_ranking(
        &state.pool,
        &leg(&entries, Kind::Place),
        &leg(&entries, Kind::World),
        scope,
        ranking_actor(&headers, admin),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiData::ok(result))))
}

#[derive(Debug, Clone, Serialize)]
pub struct PoiRow {
    pub position: String,
    pub entity_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_by: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

const POIS_LIST: &str = "pois";

async fn poi_row(state: &SharedState, position: &str) -> Result<Option<PoiRow>, ApiError> {
    let row: Option<(bool, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT present, updated_at FROM explore_api.lists_local WHERE list = $1 AND value = $2",
    )
    .bind(POIS_LIST)
    .bind(position)
    .fetch_optional(&state.pool)
    .await?;
    Ok(row.map(|(present, at)| PoiRow {
        position: position.to_string(),
        entity_id: None,
        title: None,
        description: None,
        enabled: present,
        created_by: None,
        created_at: at.and_utc(),
        updated_at: at.and_utc(),
    }))
}

pub async fn get_pois(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
) -> Result<Json<ApiData<Vec<PoiRow>>>, ApiError> {
    let rows: Vec<(String, bool, chrono::NaiveDateTime)> = sqlx::query_as("SELECT value, present, updated_at FROM explore_api.lists_local WHERE list = $1 ORDER BY value ASC")
        .bind(POIS_LIST)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(ApiData::ok(
        rows.into_iter()
            .map(|(position, enabled, at)| PoiRow {
                position,
                entity_id: None,
                title: None,
                description: None,
                enabled,
                created_by: None,
                created_at: at.and_utc(),
                updated_at: at.and_utc(),
            })
            .collect(),
    )))
}

#[derive(Debug, Deserialize)]
pub struct PoiCreate {
    pub position: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

pub async fn post_poi(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    Json(body): Json<PoiCreate>,
) -> Result<Json<ApiData<PoiRow>>, ApiError> {
    state.operator_writes()?;
    let position = body.position.trim();
    if position.is_empty() {
        return Err(ApiError::bad_request("position is required"));
    }
    crate::api::lists::set_local(
        &state.pool,
        POIS_LIST,
        position,
        body.enabled.unwrap_or(true),
    )
    .await?;
    let row = poi_row(&state, position)
        .await?
        .ok_or_else(|| ApiError::internal("the POI row vanished after its write"))?;
    Ok(Json(ApiData::ok(row)))
}

#[derive(Debug, Deserialize)]
pub struct PoiPatch {
    #[serde(default)]
    pub enabled: Option<bool>,
}

pub async fn patch_poi(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    Path(position): Path<String>,
    Json(body): Json<PoiPatch>,
) -> Result<Json<ApiData<PoiRow>>, ApiError> {
    state.operator_writes()?;
    let Some(current) = poi_row(&state, &position).await? else {
        return Err(ApiError::not_found(format!("POI \"{position}\" not found")));
    };
    if let Some(enabled) = body.enabled {
        crate::api::lists::set_local(&state.pool, POIS_LIST, &position, enabled).await?;
        return Ok(Json(ApiData::ok(
            poi_row(&state, &position).await?.unwrap_or(current),
        )));
    }
    Ok(Json(ApiData::ok(current)))
}

pub async fn delete_poi(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    Path(position): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.operator_writes()?;
    if poi_row(&state, &position).await?.is_none() {
        return Err(ApiError::not_found(format!("POI \"{position}\" not found")));
    }
    crate::api::lists::set_local(&state.pool, POIS_LIST, &position, false).await?;
    Ok(Json(
        json!({ "ok": true, "data": { "position": position } }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        headers
    }

    fn status_message(err: ApiError) -> (u16, String) {
        match err {
            ApiError::Http { status, message } => (status, message),
            other => panic!("expected an http error, got {other:?}"),
        }
    }

    #[test]
    fn ranking_accepts_either_token_and_refuses_the_rest() {
        assert!(
            require_ranking_token(&bearer("data-team"), Some("data-team"), Some("admin")).is_ok()
        );
        assert!(require_ranking_token(&bearer("data-team"), Some("data-team"), None).is_ok());
        assert!(require_ranking_token(&bearer("admin"), Some("data-team"), Some("admin")).is_ok());
        assert!(require_ranking_token(&bearer("admin"), None, Some("admin")).is_ok());
        assert_eq!(
            status_message(
                require_ranking_token(&bearer("nope"), Some("data-team"), Some("admin"))
                    .unwrap_err()
            )
            .0,
            401
        );
        assert_eq!(
            status_message(require_ranking_token(&bearer("anything"), None, None).unwrap_err()).0,
            401
        );
        assert_eq!(
            status_message(
                require_ranking_token(&HeaderMap::new(), Some("data-team"), Some("admin"))
                    .unwrap_err()
            )
            .0,
            401
        );
    }

    #[test]
    fn only_the_admin_token_moves_a_curated_ranking() {
        let err = require_admin_token_for_curated_ranking(
            &bearer("data-team"),
            Some("admin"),
            true,
            false,
            Some(42.0),
        )
        .unwrap_err();
        assert_eq!(
            status_message(err),
            (403, HIGHLIGHTED_RANKING_IS_EDITORIAL.to_string())
        );
        let err = require_admin_token_for_curated_ranking(
            &bearer("data-team"),
            Some("admin"),
            false,
            true,
            Some(42.0),
        )
        .unwrap_err();
        assert_eq!(
            status_message(err),
            (403, EXCLUDED_RANKING_IS_EDITORIAL.to_string())
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("data-team"),
                Some("admin"),
                false,
                false,
                Some(42.0)
            )
            .is_ok()
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("data-team"),
                None,
                false,
                false,
                Some(42.0)
            )
            .is_ok()
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                true,
                false,
                Some(42.0)
            )
            .is_ok()
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                false,
                true,
                Some(42.0)
            )
            .is_ok()
        );
        let err = require_admin_token_for_curated_ranking(
            &bearer("admin"),
            None,
            true,
            false,
            Some(42.0),
        )
        .unwrap_err();
        assert_eq!(status_message(err).1, HIGHLIGHTED_RANKING_IS_EDITORIAL);
        let err = require_admin_token_for_curated_ranking(
            &bearer("data-team"),
            Some("admin"),
            true,
            true,
            Some(42.0),
        )
        .unwrap_err();
        assert_eq!(
            status_message(err).1,
            HIGHLIGHTED_RANKING_IS_EDITORIAL,
            "highlighted wins over excluded"
        );
        assert!(!is_admin_token(&bearer("admin"), None));
        assert!(!is_admin_token(&HeaderMap::new(), Some("admin")));
    }

    #[test]
    fn the_admin_token_cannot_hand_rank_an_uncurated_row() {
        let err = require_admin_token_for_curated_ranking(
            &bearer("admin"),
            Some("admin"),
            false,
            false,
            Some(42.0),
        )
        .unwrap_err();
        assert_eq!(
            status_message(err),
            (400, EDITORIAL_RANKING_NEEDS_CURATION.to_string())
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                false,
                false,
                Some(0.0)
            )
            .is_ok(),
            "a zero survives the clear"
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                false,
                false,
                Some(-0.0)
            )
            .is_ok()
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                false,
                false,
                None
            )
            .is_ok(),
            "a null survives the clear"
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("data-team"),
                Some("admin"),
                false,
                false,
                Some(42.0)
            )
            .is_ok(),
            "the automated score is unaffected"
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                true,
                false,
                Some(42.0)
            )
            .is_ok()
        );
        assert!(
            require_admin_token_for_curated_ranking(
                &bearer("admin"),
                Some("admin"),
                false,
                true,
                Some(42.0)
            )
            .is_ok()
        );
    }

    #[test]
    fn the_admin_bearer_answers_the_old_403s() {
        assert_eq!(
            status_message(require_admin_bearer(&HeaderMap::new(), Some("secret")).unwrap_err()),
            (403, "Invalid admin credentials".into())
        );
        assert_eq!(
            status_message(require_admin_bearer(&bearer("anything"), None).unwrap_err()),
            (403, "Admin token not configured".into())
        );
        assert_eq!(
            status_message(require_admin_bearer(&bearer("nope"), Some("secret")).unwrap_err()),
            (403, "Invalid admin credentials".into())
        );
        assert!(require_admin_bearer(&bearer("secret"), Some("secret")).is_ok());
        assert_eq!(
            status_message(require_admin_bearer(&bearer("  "), Some("secret")).unwrap_err()),
            (403, "Invalid admin credentials".into()),
            "the shared parse keeps a blank token as a (wrong) credential"
        );
    }

    #[test]
    fn disable_and_ranking_bodies_are_validated() {
        assert!(body_disabled(&Some(Json(json!({ "disabled": true })))).unwrap());
        for body in [
            None,
            Some(Json(json!({}))),
            Some(Json(json!({ "disabled": "true" }))),
            Some(Json(json!({ "disabled": null }))),
        ] {
            assert_eq!(
                body_disabled(&body).unwrap_err().to_string(),
                "Invalid disable body. Expected { disabled: boolean }."
            );
        }
        assert_eq!(
            body_ranking(&Some(Json(json!({ "ranking": null })))).unwrap(),
            None
        );
        assert_eq!(
            body_ranking(&Some(Json(json!({ "ranking": 2.5 })))).unwrap(),
            Some(2.5)
        );
        assert!(body_ranking(&Some(Json(json!({ "ranking": "2" })))).is_err());
        assert!(body_ranking(&None).is_err());
    }

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::shared::signed_fetch::test_support::{address, signed_headers};
    use crate::{ExploreState, Settings, router};

    async fn call(
        app: &axum::Router,
        method: &str,
        path: &str,
        auth: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        match auth {
            Some("signed") => {
                for (k, v) in signed_headers(&method.to_lowercase(), path, "{}", true)
                    .await
                    .iter()
                {
                    req = req.header(k, v);
                }
            }
            Some(token) => req = req.header("authorization", format!("Bearer {token}")),
            None => {}
        }
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn mirror_row(id: &str, ranking: Option<f64>) -> super::super::mirror::MirrorRow {
        super::super::mirror::MirrorRow {
            id: id.to_string(),
            base_position: "3,3".into(),
            title: "ranked".into(),
            description: String::new(),
            creator_address: None,
            content_rating: Some("E".into()),
            categories: Vec::new(),
            likes: 0,
            dislikes: 0,
            favorites: 0,
            deployed_at: None,
            disabled: false,
            highlighted: false,
            raw: json!({ "id": id, "base_position": "3,3", "ranking": ranking }),
        }
    }

    /// The admin routes with their credentials, their switch, and the
    /// overlays a mirror pass re-applies; the reports' submit, upload and
    /// queue round trip.
    #[tokio::test]
    async fn the_admin_routes_decide_with_their_credentials_and_outlive_a_pass() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (place, other, plain) = (
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        );
        super::super::mirror::upsert_rows(
            &pool,
            &[
                mirror_row(&place, Some(4.0)),
                mirror_row(&other, Some(9.0)),
                mirror_row(&plain, Some(4.0)),
            ],
        )
        .await
        .unwrap();
        let world = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '0,0', 'ranked world', $2)")
            .bind(&world)
            .bind(json!({ "world": true, "world_name": format!("{world}.dcl.eth"), "ranking": 190 }))
            .execute(&pool)
            .await
            .unwrap();
        let stored_ranking = |id: String| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, Option<f64>>(
                    "SELECT (raw->>'ranking')::float8 FROM explore.place WHERE id = $1",
                )
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let settings = Settings {
            operator_writes_enabled: true,
            user_writes_enabled: true,
            places_admin_token: Some("admin".into()),
            data_team_token: Some("data".into()),
            places_admin_addresses: vec![address()],
            ..Settings::for_tests()
        };
        let app = router(Arc::new(ExploreState::new(
            pool.clone(),
            settings.clone(),
            None,
        )));
        let closed = router(Arc::new(ExploreState::new(
            pool.clone(),
            Settings {
                operator_writes_enabled: false,
                ..settings
            },
            None,
        )));

        let (status, _) = call(
            &closed,
            "PUT",
            &format!("/api/places/{place}/highlight"),
            Some("admin"),
            Some(json!({ "highlighted": true })),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/highlight"),
            Some("wrong"),
            Some(json!({ "highlighted": true })),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (StatusCode::FORBIDDEN, json!("Invalid admin credentials"))
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/rating"),
            Some("signed"),
            Some(json!({ "content_rating": "T" })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["content_rating"].clone()),
            (StatusCode::OK, json!("T")),
            "an admin wallet: {body}"
        );

        // rankings: the admin token only clears an uncurated row's ranking
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/ranking"),
            Some("admin"),
            Some(json!({ "ranking": 3 })),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::BAD_REQUEST,
                json!(EDITORIAL_RANKING_NEEDS_CURATION)
            )
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/ranking"),
            Some("admin"),
            Some(json!({ "ranking": 0 })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["ranking"].clone()),
            (StatusCode::CREATED, json!(0.0))
        );
        // the data team moves an uncurated row, not a highlighted one
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/ranking"),
            Some("data"),
            Some(json!({ "ranking": 7.5 })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["ranking"].clone()),
            (StatusCode::CREATED, json!(7.5))
        );
        let (status, _) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/featured"),
            Some("admin"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/ranking"),
            Some("data"),
            Some(json!({ "ranking": 1 })),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::FORBIDDEN,
                json!(HIGHLIGHTED_RANKING_IS_EDITORIAL)
            )
        );
        let (status, _) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/featured"),
            Some("data"),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "featured takes the admin bearer only"
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{other}/ranking-exclusion"),
            Some("admin"),
            None,
        )
        .await;
        assert_eq!(
            (
                status,
                body["data"]["ranking"].clone(),
                body["data"]["exclude_from_ranking"].clone()
            ),
            (StatusCode::OK, json!(0.0), json!(true))
        );

        // an upstream pass keeps every decision
        super::super::mirror::upsert_rows(
            &pool,
            &[mirror_row(&place, Some(4.0)), mirror_row(&other, Some(9.0))],
        )
        .await
        .unwrap();
        let (_, body) = call(&app, "GET", &format!("/api/places/{place}"), None, None).await;
        assert_eq!(
            (
                body["data"]["ranking"].clone(),
                body["data"]["highlighted"].clone(),
                body["data"]["content_rating"].clone()
            ),
            (json!(7.5), json!(true), json!("T"))
        );
        let (_, body) = call(&app, "GET", &format!("/api/places/{other}"), None, None).await;
        assert_eq!(
            (
                body["data"]["ranking"].clone(),
                body["data"]["exclude_from_ranking"].clone()
            ),
            (json!(0.0), json!(true))
        );

        // the replace skips both curated rows and reports what it did; a
        // places-only payload leaves the worlds' ranking alone
        let (status, body) = call(&app, "PUT", "/api/destinations/ranking", Some("data"), Some(json!({ "entries": [{ "entity_type": "place", "id": place, "ranking": 3 }, { "entity_type": "place", "id": "gone", "ranking": 1 }, { "entity_type": "place", "id": plain, "ranking": 6 }] }))).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["data"]["places"]["skipped_curated"], json!([place]));
        assert_eq!(body["data"]["places"]["skipped_missing"], json!(["gone"]));
        assert_eq!(
            (
                body["data"]["places"]["applied"].clone(),
                body["data"]["places"]["replaced"].clone()
            ),
            (json!(1), json!(true))
        );
        assert_eq!(
            (
                body["data"]["worlds"]["replaced"].clone(),
                body["data"]["worlds"]["cleared"].clone()
            ),
            (json!(false), json!(0))
        );
        assert_eq!(
            (
                stored_ranking(plain.clone()).await,
                stored_ranking(world.clone()).await
            ),
            (Some(6.0), Some(190.0))
        );

        // a type whose every destination is unknown, or an entry outside
        // `replaces`, is refused and writes nothing
        let (status, body) = call(&app, "PUT", "/api/destinations/ranking", Some("data"), Some(json!({ "entries": [{ "entity_type": "place", "id": uuid::Uuid::new_v4().to_string(), "ranking": 9 }] }))).await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::BAD_REQUEST,
                json!(
                    "None of the 1 place destinations in this request exist, so it would clear every place ranking rather than replace it"
                )
            )
        );
        let (status, body) = call(&app, "PUT", "/api/destinations/ranking", Some("data"), Some(json!({ "replaces": ["world"], "entries": [{ "entity_type": "place", "id": plain, "ranking": 3 }] }))).await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::BAD_REQUEST,
                json!("Entries name types this run does not declare in \"replaces\": place")
            )
        );
        assert_eq!(stored_ranking(plain.clone()).await, Some(6.0));

        // a declared empty type: nothing qualified today, clear that type only
        let (status, body) = call(
            &app,
            "PUT",
            "/api/destinations/ranking",
            Some("data"),
            Some(json!({ "replaces": ["world"], "entries": [] })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(
            (
                body["data"]["worlds"]["replaced"].clone(),
                body["data"]["places"]["replaced"].clone()
            ),
            (json!(true), json!(false))
        );
        assert_eq!(
            (
                stored_ranking(plain.clone()).await,
                stored_ranking(world.clone()).await
            ),
            (Some(6.0), Some(0.0))
        );

        // disable through the bearer PATCH, the web tier's call
        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/places/{place}/disable"),
            Some("admin"),
            Some(json!({ "reason": "spam" })),
        )
        .await;
        assert_eq!(
            (status, body["data"].clone()),
            (StatusCode::OK, json!({ "id": place, "disabled": true }))
        );
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/api/places/{}/disable", uuid::Uuid::new_v4()),
            Some("admin"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // POIs: the parcel in the dcl-lists pois list
        let (status, body) = call(
            &app,
            "POST",
            "/api/pois",
            Some("admin"),
            Some(json!({ "position": "12,-3", "title": "kept nowhere" })),
        )
        .await;
        assert_eq!(
            (
                status,
                body["data"]["position"].clone(),
                body["data"]["enabled"].clone()
            ),
            (StatusCode::OK, json!("12,-3"), json!(true))
        );
        let (_, body) = call(
            &app,
            "PATCH",
            "/api/pois/12,-3",
            Some("admin"),
            Some(json!({ "enabled": false })),
        )
        .await;
        assert_eq!(body["data"]["enabled"], json!(false));
        let (status, _) = call(&app, "DELETE", "/api/pois/99,99", Some("admin"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, body) = call(&app, "GET", "/api/pois", None, None).await;
        assert_eq!(
            (status, body["message"].clone()),
            (StatusCode::FORBIDDEN, json!("Invalid admin credentials"))
        );

        // a report: submit, upload by the reporter, the queue
        let (status, body) = call(
            &app,
            "POST",
            "/api/report",
            Some("signed"),
            Some(json!({ "place_id": place })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let url = body["data"]["signed_url"].as_str().unwrap().to_string();
        let upload = url[url.find("/api/report/upload/").unwrap()..].to_string();
        // Without the capability token an unsigned upload is refused.
        let without_token = upload.split('?').next().unwrap().to_string();
        let (status, _) = call(
            &app,
            "PUT",
            &without_token,
            None,
            Some(json!({ "reason": "scam_or_spam" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, body) = call(
            &app,
            "PUT",
            &upload,
            None,
            Some(json!({ "reason": "scam_or_spam" })),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, json!({ "ok": true })));
        let (status, body) = call(
            &app,
            "GET",
            &format!("/api/reports?entity_id={place}"),
            Some("admin"),
            None,
        )
        .await;
        assert_eq!((status, body["total"].clone()), (StatusCode::OK, json!(1)));
        assert_eq!(
            body["data"][0]["payload"],
            json!({ "reason": "scam_or_spam" })
        );
        let id = body["data"][0]["id"].as_i64().unwrap();
        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/reports/{id}"),
            Some("admin"),
            Some(json!({ "status": "actioned", "resolution": "content_removed" })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["status"].clone()),
            (StatusCode::OK, json!("actioned"))
        );
        assert!(body["data"]["resolved_at"].is_string());
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/api/reports/{id}"),
            Some("admin"),
            Some(json!({ "status": "gone" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn the_ranking_replace_body_is_strict() {
        let ok = parse_ranking_body(&json!({ "entries": [{ "entity_type": "place", "id": "p", "ranking": 1 }, { "entity_type": "world", "id": "w.dcl.eth", "ranking": 0 }] })).unwrap();
        assert_eq!((ok.entries.len(), ok.replaces), (2, None));
        assert_eq!(
            parse_ranking_body(&json!({ "replaces": ["world", "place"], "entries": [] }))
                .unwrap()
                .replaces,
            Some(vec![Kind::World, Kind::Place])
        );
        assert_eq!(
            parse_ranking_body(&json!({ "replaces": [], "entries": [] }))
                .unwrap()
                .replaces,
            Some(vec![])
        );
        assert_eq!(
            parse_ranking_body(&json!({ "replaces": null, "entries": [] }))
                .unwrap()
                .replaces,
            None
        );
        for bad in [
            json!([]),
            json!({ "entries": [], "extra": 1 }),
            json!({ "replaces": ["place"] }),
            json!({ "replaces": "place", "entries": [] }),
            json!({ "replaces": ["scene"], "entries": [] }),
            json!({ "replaces": ["place", "place"], "entries": [] }),
            json!({ "replaces": ["place", "world", "place"], "entries": [] }),
            json!({ "entries": [{ "entity_type": "scene", "id": "p", "ranking": 1 }] }),
            json!({ "entries": [{ "entity_type": "place", "id": "p", "ranking": 1, "x": 1 }] }),
            json!({ "entries": [{ "entity_type": "place", "id": "", "ranking": 1 }] }),
            json!({ "entries": [{ "entity_type": "place", "id": "p", "ranking": -1 }] }),
        ] {
            assert!(parse_ranking_body(&bad).is_err(), "{bad}");
        }
        let dup = parse_ranking_body(&json!({ "entries": [{ "entity_type": "place", "id": "p", "ranking": 1 }, { "entity_type": "place", "id": "p", "ranking": 2 }] })).unwrap();
        assert_eq!(duplicated_keys(&dup.entries), vec!["place:p".to_string()]);
    }

    #[test]
    fn the_replace_scope_is_declared_or_read_off_the_entries() {
        use ranking::ReplaceScope;
        let entries = |kinds: &[Kind]| {
            kinds
                .iter()
                .map(|&kind| SubmittedEntry {
                    kind,
                    id: "x".into(),
                    ranking: 1.0,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            replace_scope(None, &entries(&[Kind::Place])).unwrap(),
            ReplaceScope {
                places: true,
                worlds: false
            },
            "a one-type payload leaves the other table alone"
        );
        assert_eq!(
            replace_scope(None, &entries(&[])).unwrap(),
            ReplaceScope::default()
        );
        assert_eq!(
            replace_scope(None, &entries(&[Kind::World, Kind::Place])).unwrap(),
            ReplaceScope {
                places: true,
                worlds: true
            }
        );
        assert_eq!(
            replace_scope(Some(&[Kind::World]), &entries(&[])).unwrap(),
            ReplaceScope {
                places: false,
                worlds: true
            },
            "the declared empty set"
        );
        assert_eq!(
            replace_scope(Some(&[]), &entries(&[])).unwrap(),
            ReplaceScope::default()
        );
        let err = replace_scope(
            Some(&[Kind::World]),
            &entries(&[Kind::Place, Kind::World, Kind::Place]),
        )
        .unwrap_err();
        assert_eq!(
            status_message(err),
            (
                400,
                "Entries name types this run does not declare in \"replaces\": place".to_string()
            )
        );
        let err = replace_scope(Some(&[]), &entries(&[Kind::World, Kind::Place])).unwrap_err();
        assert_eq!(
            status_message(err).1,
            "Entries name types this run does not declare in \"replaces\": world, place"
        );
    }
}
