//! The places admin HTTP routes (the web tier's moderation page and the
//! Foundation's tools), with the old service's three credentials and their
//! exact answers:
//!
//! - the admin bearer (`PLACES_ADMIN_AUTH_TOKEN`): the reports queue, the
//!   POIs, `PATCH .../disable` (403 "Admin token not configured" or
//!   "Invalid admin credentials"), featured and the ranking exclusion
//!   (gatsby `withBearerToken`: 401 "Missing Authorization", 400 "Invalid
//!   Authorization", 403 "Invalid Bearer Token", 401 "Invalid Bearer Token"
//!   while unset);
//! - the admin bearer or the data team's (`DATA_TEAM_AUTH_TOKEN`): the
//!   rankings (the same refusals), where only the admin bearer may move a
//!   highlighted or excluded row's ranking (403 with the editorial message),
//!   and the admin bearer's non-zero ranking of a row carrying neither flag
//!   is a 400 (the nightly `PUT /destinations/ranking` would clear it; 0 and
//!   `null` survive that clear and pass);
//! - a signed fetch by a `PLACES_ADMIN_ADDRESSES` wallet, never the bearer
//!   (wkc `withAuth`): highlight, rating and `PUT .../disable` (400 "Invalid
//!   Auth Chain" without one, 403 "Only admin allowed to update {what}").
//!
//! Every route checks in the old order: its credential, then the path params
//! (a place id must be a uuid, `Place/schemas.ts:5-11`), then the body
//! (`createWkcValidator`, `shared/validate.ts:17-36`: the gatsby ajv 400
//! `Invalid data was sent to the server` with `data.errors`, `data.messages`
//! and the validated `data.body`), then `isAdmin` where a wallet signs, then
//! the row (`updateRating.ts:35-50`, `updateRanking.ts:32-45`,
//! `featured.ts:21-33`, `rankingExclusion.ts:21-31`, `replaceRanking.ts:36-40`).
//! The answer is `findByIdWithAggregates`'s row (`Place/model.ts:513-553`:
//! `p.*` plus the signer's `user_*` flags, no headcount; `World/model.ts:284-335`:
//! `w.*`, `user_visits: 0`, no `user_count`) with the changed field overwritten.
//!
//! Every write is an operator decision behind `OPS_WRITES_ENABLED`: disable,
//! highlight and rating through `places::moderation` (the console's writer),
//! the rankings through `places::ranking`; both are overlays the mirror
//! re-applies. A POI is its parcel in the `dcl-lists` `pois` list, as the
//! console keeps it (`explore::api::lists`): the old row's title,
//! description and entity id have nowhere to be served and answer `null`.
//! The content rating body takes the old route's codes (`PR`, `E`, `T`,
//! `A`, `R`, places `Place/schemas.ts:343`) and the stored pending code
//! `RP`; `RP` is what is stored and answered (`places::moderation`). As the
//! old `updatePlace` (`Place/model.ts:928-931`), a rating, highlight,
//! featured, ranking or ranking exclusion of a disabled place writes nothing
//! and still answers the row with the requested value; a disable always
//! lands and persists `disabled_at`, `disabled_reason` and `updated_at`
//! (`Place/model.ts:727-742`), so its answer is the next read's.

use std::collections::HashSet;

use axum::Json;
use axum::extract::{FromRequestParts, OriginalUri, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::moderation::{self, Decision, Target};
use super::ranking::{self, RankingEntry, ReplaceRankingResult, ScoreRankingOutcome};
use super::{PlaceRow, WorldRow};
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiError};
use crate::validate::{self, Flavour, ObjectSchema};

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
    secret_matches(admin, bearer_token(headers).map(str::trim)) == Some(true)
}

/// gatsby `withBearerToken({tokens, optional: false})`
/// (`Auth/routes/withBearerToken.js:9-38`), in its order: no configured token
/// → 401 "Invalid Bearer Token"; no `Authorization` → 401 "Missing
/// Authorization"; another scheme → 400 "Invalid Authorization"; a token not
/// in the list → 403 "Invalid Bearer Token". The token is trimmed (`:30`) and
/// compared in constant time. Captures `places.put_featured.no_bearer`,
/// `.wrong_bearer`, `.invalid_uuid_wrong_bearer`, `places.put_ranking.*`.
fn with_bearer_token(headers: &HeaderMap, tokens: &[&str]) -> Result<(), ApiError> {
    let tokens: Vec<&str> = tokens.iter().copied().filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return Err(ApiError::unauthorized("Invalid Bearer Token"));
    }
    let Some(authorization) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return Err(ApiError::unauthorized("Missing Authorization"));
    };
    let Some(presented) = authorization.strip_prefix("Bearer ") else {
        return Err(ApiError::bad_request("Invalid Authorization"));
    };
    let presented = presented.trim();
    if tokens
        .iter()
        .any(|t| crate::auth::admin_bearer::timing_safe_eq(presented.as_bytes(), t.as_bytes()))
    {
        Ok(())
    } else {
        Err(ApiError::forbidden("Invalid Bearer Token"))
    }
}

/// The featured and ranking-exclusion routes' bearer: the admin token alone
/// (`Place/routes/featured.ts:14-19`, `rankingExclusion.ts`).
pub fn require_bearer_token(headers: &HeaderMap, admin: Option<&str>) -> Result<(), ApiError> {
    with_bearer_token(headers, &[admin.unwrap_or("")])
}

/// The ranking routes' bearer: the data team's token or the admin's
/// (`entities/shared/auth.ts:12-21`).
pub fn require_ranking_token(
    headers: &HeaderMap,
    data_team: Option<&str>,
    admin: Option<&str>,
) -> Result<(), ApiError> {
    with_bearer_token(headers, &[data_team.unwrap_or(""), admin.unwrap_or("")])
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
    match secret_matches(admin, bearer_token(headers).map(str::trim)) {
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

/// gatsby `isAdmin` (`Auth/isAdmin.js`) after wkc `withAuth` and the two
/// validators (`updateRating.ts:35-44`, `updateHighlight.ts:29-38`,
/// `updateDisabled.ts:31-40`, `updateWorldRating.ts:49-58`,
/// `updateWorldHighlight.ts`): a signer outside `PLACES_ADMIN_ADDRESSES` is
/// the 403 only once params and body validated (fixture
/// `places.put_rating.non_admin_invalid_body`). The bearer is never read on
/// these routes, so it answers the chain refusal like any unsigned request
/// (capture `places.put_rating.admin_bearer_refused`). The audit actor is
/// the wallet.
fn require_admin_wallet(
    state: &SharedState,
    user: String,
    action: &str,
) -> Result<String, ApiError> {
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

/// `findByIdWithAggregates(id, {user})` (`Place/model.ts:513-553`): the
/// `places` row by id with no `world` or `disabled` filter, so a world-backed
/// row is addressed by its UUID here (fixtures
/// `places.put_rating.world_backed_place`, `places.put_featured.world_backed_place`);
/// the `user_*` flags are the signer's on the wallet routes and `false` on
/// the bearer ones (`user: undefined`).
async fn fetch_place(
    state: &SharedState,
    place_id: &str,
    viewer: Option<&str>,
) -> Result<PlaceRow, ApiError> {
    state
        .places
        .find_by_id_for(place_id, viewer)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Not found place \"{place_id}\"")))
}

async fn fetch_world(
    state: &SharedState,
    world_id: &str,
    viewer: Option<&str>,
) -> Result<PlaceRow, ApiError> {
    state
        .places
        .find_world_by_id_for(world_id, viewer)
        .await?
        .filter(PlaceRow::is_world_catalogue_row)
        .ok_or_else(|| ApiError::not_found(format!("Not found world \"{world_id}\"")))
}

/// `createWkcValidator(getPlaceParamsSchema)(ctx.params)`
/// (`Place/schemas.ts:5-11`): a place id is a uuid, and the 400's
/// `data.body` is the params object (fixtures `places.put_*.invalid_uuid`).
fn validate_place_params(place_id: &str) -> Result<(), ApiError> {
    validate::validate(
        &validate::PLACE_PARAMS,
        &json!({ "place_id": place_id }),
        Flavour::Gatsby,
    )
}

/// `World/schemas.ts:240-245` and its copies: a plain string, so it never
/// refuses; kept so the world routes read in the old order.
fn validate_world_params(world_id: &str) -> Result<(), ApiError> {
    validate::validate(
        &validate::WORLD_PARAMS,
        &json!({ "world_id": world_id }),
        Flavour::Gatsby,
    )
}

/// gatsby `createValidator` reads an absent body as `{}`
/// (`Route/validate.js:35-37`), so a bodiless PUT fails on its required
/// property, not on its type.
fn body_object(body: Option<Json<Value>>) -> Value {
    body.map(|Json(v)| v).unwrap_or_else(|| json!({}))
}

fn validate_body(schema: &ObjectSchema, body: &Value) -> Result<(), ApiError> {
    validate::validate(schema, body, Flavour::Gatsby)
}

/// `updateRankingBodySchema` (`Place/schemas.ts:352-365`,
/// `World/schemas.ts:315-328`): `{ ranking: number | null }`.
fn body_ranking(body: &Value) -> Result<Option<f64>, ApiError> {
    validate_body(&validate::UPDATE_RANKING_BODY, body)?;
    Ok(body["ranking"].as_f64())
}

/// `updateDisabledBodySchema` (`Place/schemas.ts:380-392`).
fn body_disabled(body: &Value) -> Result<bool, ApiError> {
    validate_body(&validate::UPDATE_DISABLED_BODY, body)?;
    Ok(body["disabled"].as_bool().unwrap_or_default())
}

/// `updateHighlightBodySchema` (`Place/schemas.ts:367-378`,
/// `World/schemas.ts:295-306`).
fn body_highlighted(body: &Value) -> Result<bool, ApiError> {
    validate_body(&validate::UPDATE_HIGHLIGHT_BODY, body)?;
    Ok(body["highlighted"].as_bool().unwrap_or_default())
}

/// `updateRatingBodySchema` (`Place/schemas.ts:334-350`,
/// `World/schemas.ts:337-353`): `{ content_rating, comment? }`, the decision
/// and the audit comment (`comment || null`). The stored pending code `RP`
/// is accepted beside the route's `PR`: it is checked as `PR`,
/// so every other refusal keeps upstream's `allowedValues`, and the echoed
/// `data.body` stays the client's.
fn body_rating(body: &Value) -> Result<(Decision, Option<String>), ApiError> {
    let mut checked = body.clone();
    if let Some(code) = checked.get_mut("content_rating")
        && code.as_str() == Some("RP")
    {
        *code = json!("PR");
    }
    let errors = validate::errors(&validate::UPDATE_RATING_BODY, &checked);
    if !errors.is_empty() {
        return Err(validate::validation_error(errors, body, Flavour::Gatsby));
    }
    let raw = body["content_rating"].as_str().unwrap_or_default();
    let comment = body["comment"]
        .as_str()
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    Ok((Decision::rating(raw)?, comment))
}

/// The old answer: the aggregate row with the changed field overwritten,
/// which carries no headcount (`Place/model.ts:513-553`: `p.*` and the
/// `user_*` flags) and, for a world, `user_visits: 0` without `user_count`
/// (`World/model.ts:284-335`). Fixtures `places.put_rating.admin_wallet`,
/// `worlds.put_rating.admin_wallet` (`item_keys`).
fn answer(row: impl Serialize, without: &[&str]) -> Json<ApiData<Value>> {
    let mut value = serde_json::to_value(row).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        for key in without {
            object.remove(*key);
        }
    }
    Json(ApiData::ok(value))
}

fn place_answer(row: PlaceRow) -> Json<ApiData<Value>> {
    answer(row, &["user_count", "user_visits"])
}

fn world_answer(row: PlaceRow) -> Json<ApiData<Value>> {
    answer(WorldRow::from(row), &["user_count"])
}

/// The moderation webhook capture: on with operator writes and a configured
/// endpoint (`places::moderation::Context::notify`).
fn notify(state: &SharedState) -> Option<&str> {
    let configured = state
        .settings
        .places_moderation_webhook
        .as_deref()
        .is_some_and(|w| !w.trim().is_empty());
    (state.settings.operator_writes_enabled && configured)
        .then_some(state.settings.places_base_url.as_str())
}

/// `updatePlace`'s predicate on the row a places route addresses
/// (`Place/model.ts:928-935`): `disabled is false AND world is false` on a
/// genesis place, `world is true AND (disabled IS FALSE OR disabled_reason =
/// 'opt_out')` on a world-backed row; a skipped write still answers the row
/// with the value. The worlds routes' model has no predicate.
fn silently_skipped(row: &PlaceRow, places_route: bool) -> bool {
    places_route
        && row.disabled
        && !(row.world && row.disabled_reason.as_deref() == Some("opt_out"))
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

/// `PUT /api/places/{id}/rating` (`updateRating.ts:32-82`): `withAuth`,
/// params, body, `isAdmin`, the row.
pub async fn put_place_rating(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    let user = state
        .wkc_signer(&headers, method.as_str(), uri.path())
        .await?;
    validate_place_params(&place_id)?;
    let (decision, comment) = body_rating(&body_object(body))?;
    let actor = require_admin_wallet(&state, user, "rating")?;
    let mut place = fetch_place(&state, &place_id, Some(&actor)).await?;
    let ctx = moderation::Context {
        comment: comment.as_deref(),
        notify: notify(&state),
    };
    moderation::apply_with(
        &state.pool,
        Target::Place(&place.id),
        &decision,
        &actor,
        ctx,
    )
    .await?
    .ok_or_else(|| ApiError::not_found(format!("Not found place \"{place_id}\"")))?;
    if let Decision::Rating(r) = decision {
        place.content_rating = Some(r);
    }
    Ok(place_answer(place))
}

/// `PUT /api/worlds/{id}/rating` (`updateWorldRating.ts:40-90`).
pub async fn put_world_rating(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    let user = state
        .wkc_signer(&headers, method.as_str(), uri.path())
        .await?;
    validate_world_params(&world_id)?;
    let (decision, comment) = body_rating(&body_object(body))?;
    let actor = require_admin_wallet(&state, user, "rating")?;
    let mut world = fetch_world(&state, &world_id, Some(&actor)).await?;
    let ctx = moderation::Context {
        comment: comment.as_deref(),
        notify: notify(&state),
    };
    moderation::apply_with(
        &state.pool,
        Target::World(&world.id),
        &decision,
        &actor,
        ctx,
    )
    .await?
    .ok_or_else(|| ApiError::not_found(format!("Not found world \"{world_id}\"")))?;
    if let Decision::Rating(r) = decision {
        world.content_rating = Some(r);
    }
    Ok(world_answer(world))
}

/// `PUT /api/places/{id}/highlight` (`updateHighlight.ts:26-62`).
pub async fn put_place_highlight(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    let user = state
        .wkc_signer(&headers, method.as_str(), uri.path())
        .await?;
    validate_place_params(&place_id)?;
    let highlighted = body_highlighted(&body_object(body))?;
    let actor = require_admin_wallet(&state, user, "highlight")?;
    let mut place = fetch_place(&state, &place_id, Some(&actor)).await?;
    decide(
        &state,
        Target::Place(&place.id),
        &Decision::Highlighted(highlighted),
        &actor,
        || ApiError::not_found(format!("Not found place \"{place_id}\"")),
    )
    .await?;
    place.highlighted = highlighted;
    Ok(place_answer(place))
}

/// `PUT /api/worlds/{id}/highlight` (`updateWorldHighlight.ts`).
pub async fn put_world_highlight(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    let user = state
        .wkc_signer(&headers, method.as_str(), uri.path())
        .await?;
    validate_world_params(&world_id)?;
    let highlighted = body_highlighted(&body_object(body))?;
    let actor = require_admin_wallet(&state, user, "highlight")?;
    let mut world = fetch_world(&state, &world_id, Some(&actor)).await?;
    decide(
        &state,
        Target::World(&world.id),
        &Decision::Highlighted(highlighted),
        &actor,
        || ApiError::not_found(format!("Not found world \"{world_id}\"")),
    )
    .await?;
    world.highlighted = highlighted;
    Ok(world_answer(world))
}

/// `PUT /api/places/{id}/disable` `{ disabled }`: an admin wallet.
/// `updateDisabled.ts:53-73`: `disabled_at` and `disabled_reason` set
/// (`moderation`) or cleared, `updated_at` moved, a webhook notice on a
/// disable; the answer carries the stored instant.
pub async fn put_place_disable(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    let user = state
        .wkc_signer(&headers, method.as_str(), uri.path())
        .await?;
    validate_place_params(&place_id)?;
    let disabled = body_disabled(&body_object(body))?;
    let actor = require_admin_wallet(&state, user, "disabled")?;
    let mut place = fetch_place(&state, &place_id, Some(&actor)).await?;
    let decision = Decision::Disabled {
        disabled,
        reason: disabled.then(|| "moderation".to_string()),
    };
    let ctx = moderation::Context {
        comment: None,
        notify: notify(&state),
    };
    let applied = moderation::apply_with(
        &state.pool,
        Target::Place(&place.id),
        &decision,
        &actor,
        ctx,
    )
    .await?
    .ok_or_else(|| ApiError::not_found(format!("Not found place \"{place_id}\"")))?;
    place.disabled = disabled;
    place.disabled_at = disabled.then_some(applied.at);
    place.disabled_reason = disabled.then(|| "moderation".to_string());
    place.updated_at = Some(applied.at);
    Ok(place_answer(place))
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
    // A disable without a reason is the PUT's `moderation`.
    let reason = reason
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .or_else(|| disabled.then(|| "moderation".to_string()));
    let ctx = moderation::Context {
        comment: None,
        notify: notify(&state),
    };
    moderation::apply_with(
        &state.pool,
        Target::Place(&place_id),
        &Decision::Disabled { disabled, reason },
        ADMIN_TOKEN_ACTOR,
        ctx,
    )
    .await?
    .ok_or_else(|| ApiError::not_found(format!("Not found place \"{place_id}\"")))?;
    Ok(Json(
        json!({ "ok": true, "data": { "id": place_id, "disabled": disabled } }),
    ))
}

/// `featured.ts:21-49` (places and worlds alike): the admin bearer, the
/// params, the row read with `user: undefined`.
async fn set_featured(
    state: &SharedState,
    headers: &HeaderMap,
    target: Target<'_>,
    featured: bool,
) -> Result<PlaceRow, ApiError> {
    state.operator_writes()?;
    require_bearer_token(headers, state.settings.places_admin_token.as_deref())?;
    let mut row = match target {
        Target::Place(id) => {
            validate_place_params(id)?;
            fetch_place(state, id, None).await?
        }
        Target::World(id) => {
            validate_world_params(id)?;
            fetch_world(state, id, None).await?
        }
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
) -> Result<Json<ApiData<Value>>, ApiError> {
    Ok(place_answer(
        set_featured(&state, &headers, Target::Place(&place_id), true).await?,
    ))
}

pub async fn delete_place_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    Ok(place_answer(
        set_featured(&state, &headers, Target::Place(&place_id), false).await?,
    ))
}

pub async fn put_world_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    Ok(world_answer(
        set_featured(&state, &headers, Target::World(&world_id), true).await?,
    ))
}

pub async fn delete_world_featured(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    Ok(world_answer(
        set_featured(&state, &headers, Target::World(&world_id), false).await?,
    ))
}

async fn set_ranking(
    state: &SharedState,
    headers: &HeaderMap,
    row: &mut PlaceRow,
    ranking: Option<f64>,
    editorial: &'static str,
    places_route: bool,
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
    if silently_skipped(row, places_route) {
        // `updatePlace` matched nothing (`Place/model.ts:928-935`): the
        // admin token's write is a silent echo (`updateRanking.ts:59-63`),
        // the data team's `updateRankingFromScore` reported 0 rows and
        // answered the editorial 403 (`updateRanking.ts:65-75`).
        if !is_admin_token(headers, admin) {
            return Err(ApiError::forbidden(editorial));
        }
        row.ranking = ranking;
        return Ok(());
    }
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

/// `PUT /api/places/{id}/ranking` (`updateRanking.ts:30-78`): the ranking
/// bearer, the params, the body, the row (`user: undefined`), the curation
/// rule. The 201 is the router layer's.
pub async fn put_place_ranking(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    require_ranking_token(
        &headers,
        state.settings.data_team_token.as_deref(),
        state.settings.places_admin_token.as_deref(),
    )?;
    validate_place_params(&place_id)?;
    let ranking = body_ranking(&body_object(body))?;
    let mut place = fetch_place(&state, &place_id, None).await?;
    set_ranking(
        &state,
        &headers,
        &mut place,
        ranking,
        PLACE_RANKING_IS_EDITORIAL,
        true,
    )
    .await?;
    Ok(place_answer(place))
}

/// `PUT /api/worlds/{id}/ranking` (`updateWorldRanking.ts`).
pub async fn put_world_ranking(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    require_ranking_token(
        &headers,
        state.settings.data_team_token.as_deref(),
        state.settings.places_admin_token.as_deref(),
    )?;
    validate_world_params(&world_id)?;
    let ranking = body_ranking(&body_object(body))?;
    let mut world = fetch_world(&state, &world_id, None).await?;
    set_ranking(
        &state,
        &headers,
        &mut world,
        ranking,
        WORLD_RANKING_IS_EDITORIAL,
        false,
    )
    .await?;
    Ok(world_answer(world))
}

async fn set_exclusion(
    state: &SharedState,
    row: &mut PlaceRow,
    exclude: bool,
    places_route: bool,
) -> Result<(), ApiError> {
    // `rankingExclusion.ts:46-57` through `updatePlace`: silent on a disabled place.
    if !silently_skipped(row, places_route)
        && ranking::set_exclusion(&state.pool, &row.id, exclude, ADMIN_TOKEN_ACTOR).await? == 0
    {
        return Err(ApiError::service_unavailable(EXCLUSION_NOT_WRITABLE));
    }
    row.exclude_from_ranking = exclude;
    if exclude {
        row.ranking = Some(0.0);
    }
    Ok(())
}

/// `rankingExclusion.ts:21-31`: the admin bearer, the params, the row.
async fn place_exclusion(
    state: SharedState,
    headers: HeaderMap,
    place_id: String,
    exclude: bool,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    require_bearer_token(&headers, state.settings.places_admin_token.as_deref())?;
    validate_place_params(&place_id)?;
    let mut place = fetch_place(&state, &place_id, None).await?;
    set_exclusion(&state, &mut place, exclude, true).await?;
    Ok(place_answer(place))
}

async fn world_exclusion(
    state: SharedState,
    headers: HeaderMap,
    world_id: String,
    exclude: bool,
) -> Result<Json<ApiData<Value>>, ApiError> {
    state.operator_writes()?;
    require_bearer_token(&headers, state.settings.places_admin_token.as_deref())?;
    validate_world_params(&world_id)?;
    let mut world = fetch_world(&state, &world_id, None).await?;
    set_exclusion(&state, &mut world, exclude, false).await?;
    Ok(world_answer(world))
}

pub async fn put_place_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    place_exclusion(state, headers, place_id, true).await
}

pub async fn delete_place_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    place_exclusion(state, headers, place_id, false).await
}

pub async fn put_world_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    world_exclusion(state, headers, world_id, true).await
}

pub async fn delete_world_ranking_exclusion(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    world_exclusion(state, headers, world_id, false).await
}

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

#[derive(Debug)]
struct SubmittedEntry {
    kind: Kind,
    id: String,
    ranking: f64,
}

#[derive(Debug)]
struct RankingBody {
    replaces: Option<Vec<Kind>>,
    entries: Vec<SubmittedEntry>,
}

fn entry_key(entry: &SubmittedEntry) -> String {
    format!("{}:{}", entry.kind.label(), entry.id)
}

/// `replaceRankingBodySchema` (`Destination/schemas.ts:144-196`): the ajv
/// 400 for a malformed body (`entries` required, at most 5000, each
/// `{entity_type, id, ranking >= 0}` closed; `replaces` at most both types,
/// each once, nullable), then the body read; `null` `replaces` is absent.
fn parse_ranking_body(body: &Value) -> Result<RankingBody, ApiError> {
    validate_body(&validate::REPLACE_RANKING_BODY, body)?;
    let replaces = body["replaces"]
        .as_array()
        .map(|items| items.iter().filter_map(|v| Kind::parse(Some(v))).collect());
    let entries = body["entries"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|entry| {
                    Some(SubmittedEntry {
                        kind: Kind::parse(entry.get("entity_type"))?,
                        id: entry["id"].as_str()?.to_string(),
                        ranking: entry["ranking"].as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(RankingBody { replaces, entries })
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
/// admin bearer no override). A 400 writes nothing: the ajv body refusal,
/// then (`replaceRanking.ts:36-71`, route messages) a duplicate, an entry
/// outside `replaces`, or a type whose every named destination is unknown.
pub async fn put_destinations_ranking(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<ReplaceRankingResult>>, ApiError> {
    state.operator_writes()?;
    let admin = state.settings.places_admin_token.as_deref();
    require_ranking_token(&headers, state.settings.data_team_token.as_deref(), admin)?;
    let RankingBody { replaces, entries } = parse_ranking_body(&body_object(body))?;
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
    Ok(Json(ApiData::ok(result)))
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
    use axum::http::StatusCode;

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
        // gatsby withBearerToken.js:9-38; captures places.put_ranking.wrong_bearer,
        // places.put_featured.no_bearer, places.put_destinations_ranking.*
        assert_eq!(
            status_message(
                require_ranking_token(&bearer("nope"), Some("data-team"), Some("admin"))
                    .unwrap_err()
            ),
            (403, "Invalid Bearer Token".to_string())
        );
        assert_eq!(
            status_message(require_ranking_token(&bearer("anything"), None, None).unwrap_err()),
            (401, "Invalid Bearer Token".to_string())
        );
        assert_eq!(
            status_message(
                require_ranking_token(&bearer("anything"), Some(""), Some("")).unwrap_err()
            ),
            (401, "Invalid Bearer Token".to_string()),
            "an empty token configures nothing"
        );
        assert_eq!(
            status_message(
                require_ranking_token(&HeaderMap::new(), Some("data-team"), Some("admin"))
                    .unwrap_err()
            ),
            (401, "Missing Authorization".to_string())
        );
        let mut basic = HeaderMap::new();
        basic.insert("authorization", "Basic abc".parse().unwrap());
        assert_eq!(
            status_message(require_ranking_token(&basic, Some("data-team"), None).unwrap_err()),
            (400, "Invalid Authorization".to_string())
        );
        assert_eq!(
            status_message(require_bearer_token(&bearer("data-team"), Some("admin")).unwrap_err()),
            (403, "Invalid Bearer Token".to_string()),
            "featured takes the admin token only (capture places.put_featured.data_team_bearer)"
        );
        assert!(
            require_bearer_token(&bearer("admin "), Some("admin")).is_ok(),
            "the token is trimmed"
        );
        assert!(require_bearer_token(&bearer("admin"), Some("admin")).is_ok());
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
        assert!(
            require_admin_bearer(&bearer("secret "), Some("secret")).is_ok(),
            "trimmed like the other places checks (`withBearerToken.js:30`)"
        );
        assert_eq!(
            status_message(require_admin_bearer(&bearer("  "), Some("secret")).unwrap_err()),
            (403, "Invalid admin credentials".into()),
            "the shared parse keeps a blank token as a (wrong) credential"
        );
    }

    /// `Place/schemas.ts:334-350`: `content_rating` from the route enum
    /// (`PR` pending) or the stored `RP`, an optional string `comment`.
    /// The gatsby ajv 400 (`Route/validate.js:9-31`) as `(status, error,
    /// data)`.
    fn refusal(err: ApiError) -> (u16, String, Value) {
        match err {
            ApiError::HttpData {
                status,
                message,
                data,
            } => (status, message, data),
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    /// Fixtures `places.put_rating.missing_field`, `.extra_key`,
    /// `places.put_rating.invalid_enum` (upstream's enum; `RP` accepted and
    /// stored as itself).
    #[test]
    fn the_rating_body_takes_both_pending_codes_and_a_comment() {
        let (decision, comment) =
            body_rating(&json!({ "content_rating": "PR", "comment": "reviewed" })).unwrap();
        assert_eq!(
            (decision, comment),
            (Decision::Rating("RP".into()), Some("reviewed".into()))
        );
        let (decision, comment) = body_rating(&json!({ "content_rating": "RP" })).unwrap();
        assert_eq!((decision, comment), (Decision::Rating("RP".into()), None));
        assert_eq!(
            body_rating(&json!({ "content_rating": "T", "comment": "" }))
                .unwrap()
                .1,
            None,
            "comment || null"
        );
        let (status, message, data) = refusal(body_rating(&body_object(None)).unwrap_err());
        assert_eq!(
            (status, message.as_str()),
            (400, validate::GATSBY_INVALID_DATA)
        );
        assert_eq!(
            data,
            json!({
                "errors": [{
                    "instancePath": "",
                    "schemaPath": "#/required",
                    "keyword": "required",
                    "params": { "missingProperty": "content_rating" },
                    "message": "must have required property 'content_rating'"
                }],
                "messages": ["must have required property 'content_rating' {\"missingProperty\":\"content_rating\"}"],
                "body": {}
            })
        );
        let (_, _, data) =
            refusal(body_rating(&json!({ "content_rating": "T", "foo": 1 })).unwrap_err());
        assert_eq!(
            data["errors"][0],
            json!({
                "instancePath": "",
                "schemaPath": "#/additionalProperties",
                "keyword": "additionalProperties",
                "params": { "additionalProperty": "foo" },
                "message": "must NOT have additional properties"
            })
        );
        assert_eq!(data["body"], json!({ "content_rating": "T", "foo": 1 }));
        let (_, _, data) = refusal(body_rating(&json!({ "content_rating": "X" })).unwrap_err());
        assert_eq!(
            data["errors"][0],
            json!({
                "instancePath": "/content_rating",
                "schemaPath": "#/properties/content_rating/enum",
                "keyword": "enum",
                "params": { "allowedValues": ["PR", "E", "T", "A", "R"] },
                "message": "must be equal to one of the allowed values"
            })
        );
        let (_, _, data) =
            refusal(body_rating(&json!({ "content_rating": "RP", "foo": 1 })).unwrap_err());
        assert_eq!(
            data["body"],
            json!({ "content_rating": "RP", "foo": 1 }),
            "the echoed body is the client's"
        );
        assert_eq!(
            refusal(body_rating(&json!({ "content_rating": 1 })).unwrap_err()).2["messages"],
            json!(["content_rating must be string {\"type\":\"string\"}"])
        );
        assert_eq!(
            refusal(body_rating(&json!({ "content_rating": "T", "comment": 1 })).unwrap_err()).2["messages"],
            json!(["comment must be string {\"type\":\"string\"}"])
        );
    }

    /// Fixtures `places.put_disable.missing_field`,
    /// `places.put_highlight.missing_field`, `places.put_ranking.missing_field`,
    /// `.string_value`; `places.put_*.invalid_uuid`.
    #[test]
    fn disable_highlight_ranking_bodies_and_params_are_validated() {
        assert!(body_disabled(&json!({ "disabled": true })).unwrap());
        assert_eq!(
            refusal(body_disabled(&body_object(None)).unwrap_err()).2["messages"],
            json!(["must have required property 'disabled' {\"missingProperty\":\"disabled\"}"])
        );
        assert_eq!(
            refusal(body_disabled(&json!({ "disabled": "true" })).unwrap_err()).2["messages"],
            json!(["disabled must be boolean {\"type\":\"boolean\"}"])
        );
        assert_eq!(
            refusal(body_disabled(&json!({ "disabled": null })).unwrap_err()).2["messages"],
            json!(["disabled must be boolean {\"type\":\"boolean\"}"])
        );
        assert!(body_highlighted(&json!({ "highlighted": false })).is_ok_and(|h| !h));
        assert_eq!(
            refusal(body_highlighted(&json!({})).unwrap_err()).2["messages"],
            json!([
                "must have required property 'highlighted' {\"missingProperty\":\"highlighted\"}"
            ])
        );
        assert_eq!(body_ranking(&json!({ "ranking": null })).unwrap(), None);
        assert_eq!(body_ranking(&json!({ "ranking": 2.5 })).unwrap(), Some(2.5));
        let (status, message, data) =
            refusal(body_ranking(&json!({ "ranking": "high" })).unwrap_err());
        assert_eq!(
            (status, message.as_str()),
            (400, validate::GATSBY_INVALID_DATA)
        );
        assert_eq!(
            data,
            json!({
                "errors": [{
                    "instancePath": "/ranking",
                    "schemaPath": "#/properties/ranking/type",
                    "keyword": "type",
                    "params": { "type": "number,null" },
                    "message": "must be number,null"
                }],
                "messages": ["ranking must be number,null {\"type\":\"number,null\"}"],
                "body": { "ranking": "high" }
            })
        );
        assert_eq!(
            refusal(body_ranking(&json!({})).unwrap_err()).2["messages"],
            json!(["must have required property 'ranking' {\"missingProperty\":\"ranking\"}"])
        );
        assert_eq!(
            refusal(body_ranking(&json!({ "ranking": 1, "x": 1 })).unwrap_err()).2["messages"],
            json!(["must NOT have additional properties {\"additionalProperty\":\"x\"}"])
        );

        assert!(validate_place_params("3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d").is_ok());
        let (status, message, data) = refusal(validate_place_params("not-a-uuid").unwrap_err());
        assert_eq!(
            (status, message.as_str()),
            (400, validate::GATSBY_INVALID_DATA)
        );
        assert_eq!(
            data,
            json!({
                "errors": [{
                    "instancePath": "/place_id",
                    "schemaPath": "#/properties/place_id/format",
                    "keyword": "format",
                    "params": { "format": "uuid" },
                    "message": "must match format \"uuid\""
                }],
                "messages": ["place_id must match format \"uuid\" {\"format\":\"uuid\"}"],
                "body": { "place_id": "not-a-uuid" }
            })
        );
        assert!(
            validate_world_params("Brai.dcl.eth").is_ok(),
            "a plain string"
        );
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
        // wkc withAuth never reads the bearer (capture places.put_highlight.admin_bearer_refused)
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("Invalid Auth Chain"))
        );
        assert!(body.get("message").is_none(), "upstream's envelope: {body}");
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
            (StatusCode::CREATED, json!("T")),
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
            (status, body["error"].clone()),
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
        assert_eq!(status, StatusCode::CREATED, "wkc answers every PUT 201");
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/ranking"),
            Some("data"),
            Some(json!({ "ranking": 1 })),
        )
        .await;
        assert_eq!(
            (status, body["error"].clone()),
            (
                StatusCode::FORBIDDEN,
                json!(HIGHLIGHTED_RANKING_IS_EDITORIAL)
            )
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{place}/featured"),
            Some("data"),
            None,
        )
        .await;
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::FORBIDDEN, json!("Invalid Bearer Token")),
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
            (StatusCode::CREATED, json!(0.0), json!(true))
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
            (status, body["error"].clone()),
            (
                StatusCode::BAD_REQUEST,
                json!(
                    "None of the 1 place destinations in this request exist, so it would clear every place ranking rather than replace it"
                )
            )
        );
        let (status, body) = call(&app, "PUT", "/api/destinations/ranking", Some("data"), Some(json!({ "replaces": ["world"], "entries": [{ "entity_type": "place", "id": plain, "ranking": 3 }] }))).await;
        assert_eq!(
            (status, body["error"].clone()),
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

        // the rating body: the route's `PR` is stored and answered as `RP`,
        // the comment reaches the audit log (fixture places.put_rating.admin_wallet)
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/rating"),
            Some("signed"),
            Some(json!({ "content_rating": "PR", "comment": "reviewed" })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["content_rating"].clone()),
            (StatusCode::CREATED, json!("RP")),
            "{body}"
        );
        let (_, body) = call(&app, "GET", &format!("/api/places/{plain}"), None, None).await;
        assert_eq!(body["data"]["content_rating"], json!("RP"));
        let audit: Vec<(String, Option<String>, String, Option<String>)> = sqlx::query_as(
            "SELECT entity_type, original_rating, update_rating, comment FROM explore.place_content_ratings WHERE entity_id = $1 ORDER BY created_at",
        )
        .bind(&plain)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            audit,
            vec![(
                "place".into(),
                Some("E".into()),
                "RP".into(),
                Some("reviewed".into())
            )]
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/rating"),
            Some("signed"),
            Some(json!({ "content_rating": "M" })),
        )
        .await;
        // the ajv refusal keeps upstream's enum (Place/schemas.ts:340-344)
        assert_eq!(
            (
                status,
                body["error"].clone(),
                body["data"]["messages"].clone()
            ),
            (
                StatusCode::BAD_REQUEST,
                json!(validate::GATSBY_INVALID_DATA),
                json!([
                    "content_rating must be equal to one of the allowed values {\"allowedValues\":[\"PR\",\"E\",\"T\",\"A\",\"R\"]}"
                ])
            )
        );

        // a disable persists what it answers (fixtures places.put_disable.*):
        // the next read carries the same disabled_at, disabled_reason and updated_at
        let (status, put) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/disable"),
            Some("signed"),
            Some(json!({ "disabled": true })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{put}");
        let (_, got) = call(&app, "GET", &format!("/api/places/{plain}"), None, None).await;
        for key in ["disabled", "disabled_at", "disabled_reason", "updated_at"] {
            assert_eq!(put["data"][key], got["data"][key], "{key}: {put} vs {got}");
        }
        assert_eq!(got["data"]["disabled_reason"], json!("moderation"));
        assert!(got["data"]["disabled_at"].is_string(), "{got}");
        assert_eq!(got["data"]["disabled_at"], got["data"]["updated_at"]);

        // on a disabled place the old `updatePlace` matched nothing: rating,
        // featured, ranking exclusion and the admin ranking echo the value
        // and change nothing; the data team's ranking is the editorial 403
        // (fixtures places.put_rating.disabled_place_silent,
        // places.put_featured.disabled_place_silent,
        // places.put_ranking.admin_disabled_place_silent,
        // places.put_ranking.data_team_race_or_disabled)
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/rating"),
            Some("signed"),
            Some(json!({ "content_rating": "A" })),
        )
        .await;
        assert_eq!(
            (
                status,
                body["data"]["content_rating"].clone(),
                body["data"]["disabled"].clone()
            ),
            (StatusCode::CREATED, json!("A"), json!(true))
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/featured"),
            Some("admin"),
            None,
        )
        .await;
        assert_eq!(
            (status, body["data"]["highlighted"].clone()),
            (StatusCode::CREATED, json!(true))
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/ranking-exclusion"),
            Some("admin"),
            None,
        )
        .await;
        assert_eq!(
            (status, body["data"]["exclude_from_ranking"].clone()),
            (StatusCode::CREATED, json!(true))
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/ranking"),
            Some("admin"),
            Some(json!({ "ranking": 0 })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["ranking"].clone()),
            (StatusCode::CREATED, json!(0.0))
        );
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/ranking"),
            Some("data"),
            Some(json!({ "ranking": 42 })),
        )
        .await;
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::FORBIDDEN, json!(PLACE_RANKING_IS_EDITORIAL))
        );
        let (_, got) = call(&app, "GET", &format!("/api/places/{plain}"), None, None).await;
        assert_eq!(
            (
                got["data"]["content_rating"].clone(),
                got["data"]["highlighted"].clone(),
                got["data"]["exclude_from_ranking"].clone(),
                got["data"]["ranking"].clone()
            ),
            (json!("RP"), json!(false), json!(false), json!(6.0)),
            "nothing was written: {got}"
        );
        let audit: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.place_content_ratings WHERE entity_id = $1",
        )
        .bind(&plain)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            audit, 2,
            "the audit row is written even when the row took nothing"
        );

        // re-enabled: cleared keys, the next read agrees
        let (status, put) = call(
            &app,
            "PUT",
            &format!("/api/places/{plain}/disable"),
            Some("signed"),
            Some(json!({ "disabled": false })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let (_, got) = call(&app, "GET", &format!("/api/places/{plain}"), None, None).await;
        assert_eq!(
            (
                got["data"]["disabled"].clone(),
                got["data"]["disabled_at"].clone(),
                got["data"]["disabled_reason"].clone(),
                got["data"]["updated_at"].clone()
            ),
            (
                json!(false),
                Value::Null,
                Value::Null,
                put["data"]["updated_at"].clone()
            )
        );
        assert!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM explore.moderation_hooks WHERE channel = 'places' AND subject = $1"
            )
            .bind(&plain)
            .fetch_one(&pool)
            .await
            .unwrap()
                == 0,
            "no webhook configured: no notice captured"
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
        let (_, got) = call(&app, "GET", &format!("/api/places/{place}"), None, None).await;
        assert_eq!(
            (
                got["data"]["disabled"].clone(),
                got["data"]["disabled_reason"].clone()
            ),
            (json!(true), json!("spam")),
            "the PATCH's free-text reason is the stored disabled_reason"
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
            (status, body["error"].clone()),
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
        assert_eq!(status, StatusCode::CREATED, "{body}");
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
        // wkc withAuth: no chain is the 400 (fixture report.post.no_auth)
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, body) = call(
            &app,
            "PUT",
            &upload,
            None,
            Some(json!({ "reason": "scam_or_spam" })),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, json!({ "ok": true })));
        // The reporter can still use signed fetch without a capability.
        let (status, body) = call(
            &app,
            "PUT",
            &without_token,
            Some("signed"),
            Some(json!({ "reason": "scam_or_spam", "signed": true })),
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
            json!({ "reason": "scam_or_spam", "signed": true })
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

    /// `Destination/schemas.ts:144-196`; fixtures `destinations.put_ranking.
    /// missing_entries`, `.too_many_entries`, `.negative_ranking`,
    /// `.bad_entity_type`, `.entry_missing_id`, `.replaces_invalid`, `.extra_key`.
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
        let entries: Vec<Value> = (0..5001)
            .map(|i| json!({ "entity_type": "place", "id": format!("p{i}"), "ranking": 1 }))
            .collect();
        for (bad, line) in [
            (json!([]), "must be object {\"type\":\"object\"}"),
            (
                json!({ "entries": [], "dry_run": true }),
                "must NOT have additional properties {\"additionalProperty\":\"dry_run\"}",
            ),
            (
                json!({ "replaces": ["place"] }),
                "must have required property 'entries' {\"missingProperty\":\"entries\"}",
            ),
            (
                json!({ "replaces": "place", "entries": [] }),
                "replaces must be array,null {\"type\":\"array,null\"}",
            ),
            (
                json!({ "replaces": ["scene"], "entries": [] }),
                "replaces/0 must be equal to one of the allowed values {\"allowedValues\":[\"place\",\"world\"]}",
            ),
            (
                json!({ "replaces": ["place", "place"], "entries": [] }),
                "replaces must NOT have duplicate items (items ## 0 and 1 are identical) {\"i\":1,\"j\":0}",
            ),
            (
                json!({ "replaces": ["place", "world", "place"], "entries": [] }),
                "replaces must NOT have more than 2 items {\"limit\":2}",
            ),
            (
                json!({ "entries": [{ "entity_type": "scene", "id": "p", "ranking": 1 }] }),
                "entries/0/entity_type must be equal to one of the allowed values {\"allowedValues\":[\"place\",\"world\"]}",
            ),
            (
                json!({ "entries": [{ "entity_type": "place", "id": "p", "ranking": 1, "x": 1 }] }),
                "entries/0 must NOT have additional properties {\"additionalProperty\":\"x\"}",
            ),
            (
                json!({ "entries": [{ "entity_type": "place", "ranking": 1 }] }),
                "entries/0 must have required property 'id' {\"missingProperty\":\"id\"}",
            ),
            (
                json!({ "entries": [{ "entity_type": "place", "id": "", "ranking": 1 }] }),
                "entries/0/id must NOT have fewer than 1 characters {\"limit\":1}",
            ),
            (
                json!({ "entries": [{ "entity_type": "place", "id": "p", "ranking": -1 }] }),
                "entries/0/ranking must be >= 0 {\"comparison\":\">=\",\"limit\":0}",
            ),
            (
                json!({ "replaces": ["place"], "entries": entries }),
                "entries must NOT have more than 5000 items {\"limit\":5000}",
            ),
        ] {
            let (status, message, data) = refusal(parse_ranking_body(&bad).unwrap_err());
            assert_eq!(
                (status, message.as_str(), data["messages"].clone()),
                (400, validate::GATSBY_INVALID_DATA, json!([line])),
                "{bad}"
            );
            assert_eq!(data["body"], bad);
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

    /// A pool nothing answers: every refusal below lands before a row is read.
    fn offline_pool() -> sqlx::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(50))
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap()
    }

    /// The old order on every admin route (`updateRating.ts:35-44`,
    /// `updateRanking.ts:32-35`, `featured.ts:21-33`, `rankingExclusion.ts:21-31`,
    /// `replaceRanking.ts:36-38`, `Report/routes.ts:29`): credential, params,
    /// body, `isAdmin`, then the row. Fixtures `places.put_rating.invalid_uuid`,
    /// `.missing_field`, `.non_admin_invalid_body`, `places.put_featured.invalid_uuid`,
    /// `places.put_ranking.invalid_uuid`, `.missing_field`, `.string_value`,
    /// `places.put_ranking_exclusion.invalid_uuid`, `places.put_highlight.missing_field`,
    /// `places.put_disable.missing_field`, `destinations.put_ranking.extra_key`,
    /// `report.post.no_auth`.
    /// `findByIdWithAggregates` has no `world` filter and `updatePlace` writes a
    /// `world` row behind `world is true AND (disabled IS FALSE OR
    /// disabled_reason = 'opt_out')` (`Place/model.ts:513-553,932-935`;
    /// fixtures `places.put_rating.world_backed_place`,
    /// `places.put_featured.world_backed_place`).
    #[tokio::test]
    async fn world_backed_place_rows_are_addressed_by_uuid_on_the_places_routes() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let mut rows = Vec::new();
        for (disabled, reason) in [
            (false, None),
            (true, Some("moderation")),
            (true, Some("opt_out")),
        ] {
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO explore.place (id, base_position, title, content_rating, disabled, raw) \
                 VALUES ($1, '0,0', 'harbour scene', 'E', $2, $3)",
            )
            .bind(&id)
            .bind(disabled)
            .bind(json!({
                "id": id, "world": true, "world_name": format!("{id}.dcl.eth"),
                "disabled": disabled, "disabled_reason": reason, "content_rating": "E"
            }))
            .execute(&pool)
            .await
            .unwrap();
            rows.push((id, disabled, reason));
        }
        let app = router(Arc::new(ExploreState::new(
            pool.clone(),
            Settings {
                operator_writes_enabled: true,
                user_writes_enabled: true,
                places_admin_token: Some("admin".into()),
                places_admin_addresses: vec![address()],
                ..Settings::for_tests()
            },
            None,
        )));
        for (id, disabled, reason) in &rows {
            let written = !disabled || *reason == Some("opt_out");
            let (status, body) = call(
                &app,
                "PUT",
                &format!("/api/places/{id}/rating"),
                Some("signed"),
                Some(json!({ "content_rating": "T" })),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
            assert_eq!(body["data"]["content_rating"], "T");
            assert_eq!(body["data"]["world"], true);
            assert_eq!(body["data"]["disabled"], *disabled);
            let (status, body) = call(
                &app,
                "PUT",
                &format!("/api/places/{id}/featured"),
                Some("admin"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
            assert_eq!(body["data"]["highlighted"], true);
            let (rating, highlighted): (String, bool) = sqlx::query_as(
                "SELECT content_rating, highlighted FROM explore.place WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                (rating.as_str(), highlighted),
                if written { ("T", true) } else { ("E", false) },
                "disabled={disabled} reason={reason:?}"
            );
            let (status, body) = call(
                &app,
                "GET",
                &format!("/api/worlds/{id}.dcl.eth"),
                None,
                None,
            )
            .await;
            assert!(
                status.is_success() || status == StatusCode::NOT_FOUND,
                "{body}"
            );
        }
    }

    #[tokio::test]
    async fn admin_routes_check_credential_params_body_then_admin_before_any_row() {
        let settings = Settings {
            operator_writes_enabled: true,
            user_writes_enabled: true,
            places_admin_token: Some("admin".into()),
            data_team_token: Some("data".into()),
            places_admin_addresses: Vec::new(),
            ..Settings::for_tests()
        };
        let app = router(Arc::new(ExploreState::new(offline_pool(), settings, None)));
        let place = "/api/places/3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d";
        let uuid_refusal = json!({
            "ok": false,
            "error": validate::GATSBY_INVALID_DATA,
            "code": "bad_request",
            "data": {
                "errors": [{
                    "instancePath": "/place_id",
                    "schemaPath": "#/properties/place_id/format",
                    "keyword": "format",
                    "params": { "format": "uuid" },
                    "message": "must match format \"uuid\""
                }],
                "messages": ["place_id must match format \"uuid\" {\"format\":\"uuid\"}"],
                "body": { "place_id": "not-a-uuid" }
            }
        });
        let required = |key: &str, body: Value| {
            json!({
                "ok": false,
                "error": validate::GATSBY_INVALID_DATA,
                "code": "bad_request",
                "data": {
                    "errors": [{
                        "instancePath": "",
                        "schemaPath": "#/required",
                        "keyword": "required",
                        "params": { "missingProperty": key },
                        "message": format!("must have required property '{key}'")
                    }],
                    "messages": [format!("must have required property '{key}' {{\"missingProperty\":\"{key}\"}}")],
                    "body": body
                }
            })
        };

        // wallet routes: a non-admin signer gets the params and body 400s before the 403
        for (route, body, expected) in [
            (
                "/api/places/not-a-uuid/rating".to_string(),
                Some(json!({ "content_rating": "T" })),
                uuid_refusal.clone(),
            ),
            (
                format!("{place}/rating"),
                Some(json!({})),
                required("content_rating", json!({})),
            ),
            (
                format!("{place}/rating"),
                None,
                required("content_rating", json!({})),
            ),
            (
                format!("{place}/highlight"),
                Some(json!({})),
                required("highlighted", json!({})),
            ),
            (
                format!("{place}/disable"),
                Some(json!({})),
                required("disabled", json!({})),
            ),
            (
                "/api/places/not-a-uuid/disable".to_string(),
                None,
                uuid_refusal.clone(),
            ),
        ] {
            let (status, answer) = call(&app, "PUT", &route, Some("signed"), body.clone()).await;
            assert_eq!(
                (status, answer),
                (StatusCode::BAD_REQUEST, expected),
                "{route} {body:?}"
            );
        }
        let (status, answer) = call(
            &app,
            "PUT",
            &format!("{place}/rating"),
            Some("signed"),
            Some(json!({ "content_rating": "T" })),
        )
        .await;
        assert_eq!(
            (status, answer["error"].clone()),
            (
                StatusCode::FORBIDDEN,
                json!("Only admin allowed to update rating")
            ),
            "isAdmin after a valid body, before the row"
        );
        let (status, answer) = call(
            &app,
            "PUT",
            &format!("{place}/rating"),
            Some("admin"),
            Some(json!({})),
        )
        .await;
        assert_eq!(
            (status, answer["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("Invalid Auth Chain")),
            "the bearer is not a chain; the chain refusal comes before the body"
        );

        // bearer routes: the token, then the params, then the body
        for (method, route, token, body, expected) in [
            (
                "PUT",
                "/api/places/not-a-uuid/featured".to_string(),
                "admin",
                None,
                uuid_refusal.clone(),
            ),
            (
                "DELETE",
                "/api/places/not-a-uuid/featured".to_string(),
                "admin",
                None,
                uuid_refusal.clone(),
            ),
            (
                "PUT",
                "/api/places/not-a-uuid/ranking-exclusion".to_string(),
                "admin",
                None,
                uuid_refusal.clone(),
            ),
            (
                "DELETE",
                "/api/places/not-a-uuid/ranking-exclusion".to_string(),
                "admin",
                None,
                uuid_refusal.clone(),
            ),
            (
                "PUT",
                "/api/places/not-a-uuid/ranking".to_string(),
                "data",
                Some(json!({ "ranking": 1 })),
                uuid_refusal.clone(),
            ),
            (
                "PUT",
                format!("{place}/ranking"),
                "data",
                Some(json!({})),
                required("ranking", json!({})),
            ),
            (
                "PUT",
                format!("{place}/ranking"),
                "data",
                None,
                required("ranking", json!({})),
            ),
            (
                "PUT",
                "/api/destinations/ranking".to_string(),
                "data",
                Some(json!({ "entries": [], "dry_run": true })),
                json!({
                    "ok": false,
                    "error": validate::GATSBY_INVALID_DATA,
                    "code": "bad_request",
                    "data": {
                        "errors": [{
                            "instancePath": "",
                            "schemaPath": "#/additionalProperties",
                            "keyword": "additionalProperties",
                            "params": { "additionalProperty": "dry_run" },
                            "message": "must NOT have additional properties"
                        }],
                        "messages": ["must NOT have additional properties {\"additionalProperty\":\"dry_run\"}"],
                        "body": { "entries": [], "dry_run": true }
                    }
                }),
            ),
            (
                "PUT",
                "/api/destinations/ranking".to_string(),
                "data",
                None,
                required("entries", json!({})),
            ),
        ] {
            let (status, answer) = call(&app, method, &route, Some(token), body.clone()).await;
            assert_eq!(
                (status, answer),
                (StatusCode::BAD_REQUEST, expected),
                "{method} {route} {body:?}"
            );
        }
        let (status, answer) = call(
            &app,
            "PUT",
            "/api/places/not-a-uuid/ranking",
            Some("wrong"),
            Some(json!({ "ranking": 1 })),
        )
        .await;
        assert_eq!(
            (status, answer["error"].clone()),
            (StatusCode::FORBIDDEN, json!("Invalid Bearer Token")),
            "the bearer before the params (capture places.put_featured.invalid_uuid_wrong_bearer)"
        );
        let (status, answer) = call(
            &app,
            "PUT",
            &format!("{place}/ranking"),
            Some("data"),
            Some(json!({ "ranking": "high" })),
        )
        .await;
        assert_eq!(
            (status, answer["data"]["messages"].clone()),
            (
                StatusCode::BAD_REQUEST,
                json!(["ranking must be number,null {\"type\":\"number,null\"}"])
            )
        );

        // the report routes answer wkc withAuth's refusals
        for (method, route) in [
            ("POST", "/api/report"),
            ("PUT", "/api/report/upload/abcdef01deadbeef.json"),
        ] {
            let (status, answer) = call(&app, method, route, None, Some(json!({}))).await;
            assert_eq!(
                (status, answer),
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "ok": false, "error": "Invalid Auth Chain", "code": "bad_request" })
                ),
                "{method} {route}"
            );
        }
    }
}
