//! A viewer's favorite and like of a place or a world: `PATCH
//! /api/{places,worlds}/{id}/{favorites,likes}` and `PUT`/`DELETE
//! /v1/destinations/{id}/{favorites,likes}`, signed fetch, behind
//! `DATA_WRITES_ENABLED`.
//!
//! The facts are `explore.user_favorites` and `explore.user_likes` (one row
//! per viewer and entity, local authority). The counters the routes answer
//! are upstream's numbers (`explore.place.{favorites,likes,dislikes}`, the
//! mirror's) plus this node's own, kept in
//! `explore.place_interaction_counts`: derived from the two fact tables,
//! written in the same transaction as the fact under a lock on the entity's
//! counter row (so two writes on one entity serialise), and rebuildable
//! from them ([`REBUILD_COUNTS`]). The old service wrote its local count
//! over the mirrored column, and the next pass wrote upstream's back.
//! `like_rate` and `like_score` stay upstream's: the old node's local
//! weighting by a Snapshot activity score is not carried (its like writes
//! record `user_activity` 0).

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Method};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use super::PlaceRow;
use crate::SharedState;
use crate::fed;
use crate::shared::errors::{ApiData, ApiError};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct FavoritesResult {
    pub favorites: i32,
    pub user_favorite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct LikesResult {
    pub likes: i32,
    pub dislikes: i32,
    pub user_like: bool,
    pub user_dislike: bool,
}

/// The six counters `/v1/destinations/{id}/{favorites,likes}` answer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct InteractionSummary {
    pub likes: i32,
    pub dislikes: i32,
    pub favorites: i32,
    pub user_like: bool,
    pub user_dislike: bool,
    pub user_favorite: bool,
}

impl From<&PlaceRow> for InteractionSummary {
    fn from(p: &PlaceRow) -> Self {
        Self {
            likes: p.likes,
            dislikes: p.dislikes,
            favorites: p.favorites,
            user_like: p.user_like,
            user_dislike: p.user_dislike,
            user_favorite: p.user_favorite,
        }
    }
}

/// Rebuilds every counter from the facts (the derived table's rebuild rule).
pub const REBUILD_COUNTS: &str = r#"
    WITH facts AS (
        SELECT entity_id, sum(f)::int AS favorites, sum(l)::int AS likes, sum(d)::int AS dislikes
        FROM (SELECT entity_id, 1 AS f, 0 AS l, 0 AS d FROM explore.user_favorites
              UNION ALL
              SELECT entity_id, 0, ("like")::int, (NOT "like")::int FROM explore.user_likes) u
        GROUP BY entity_id
    ), zeroed AS (
        UPDATE explore.place_interaction_counts c SET favorites = 0, likes = 0, dislikes = 0, updated_at = $1
        WHERE NOT EXISTS (SELECT 1 FROM facts WHERE facts.entity_id = c.entity_id)
    )
    INSERT INTO explore.place_interaction_counts (entity_id, favorites, likes, dislikes, updated_at)
    SELECT entity_id, favorites, likes, dislikes, $1 FROM facts
    ON CONFLICT (entity_id) DO UPDATE SET
        favorites = EXCLUDED.favorites, likes = EXCLUDED.likes, dislikes = EXCLUDED.dislikes,
        updated_at = EXCLUDED.updated_at"#;

/// A catalogue place id is a UUID; anything else names a world.
pub fn is_place_id(s: &str) -> bool {
    s.len() == 36 && uuid::Uuid::parse_str(s).is_ok()
}

fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// Locks the entity's counter row for the rest of the transaction.
async fn lock_counts(tx: &mut PgConnection, entity_id: &str) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO explore.place_interaction_counts (entity_id, updated_at) VALUES ($1, $2) ON CONFLICT (entity_id) DO NOTHING")
        .bind(entity_id)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT 1 FROM explore.place_interaction_counts WHERE entity_id = $1 FOR UPDATE")
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

/// Sets `user`'s favorite of `entity_id` (the catalogue row id). Idempotent.
pub async fn set_favorite(
    pool: &PgPool,
    entity_id: &str,
    user: &str,
    favorite: bool,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    set_favorite_in(&mut tx, entity_id, user, favorite).await?;
    tx.commit().await?;
    Ok(())
}

/// [`set_favorite`] inside the caller's transaction.
pub async fn set_favorite_in(
    tx: &mut PgConnection,
    entity_id: &str,
    user: &str,
    favorite: bool,
) -> Result<(), ApiError> {
    let user = user.to_lowercase();
    lock_counts(&mut *tx, entity_id).await?;
    let changed = if favorite {
        sqlx::query(r#"INSERT INTO explore.user_favorites ("user", entity_id, created_at) VALUES ($1, $2, $3) ON CONFLICT ("user", entity_id) DO NOTHING"#)
            .bind(&user)
            .bind(entity_id)
            .bind(now())
            .execute(&mut *tx)
            .await?
            .rows_affected()
    } else {
        sqlx::query(r#"DELETE FROM explore.user_favorites WHERE "user" = $1 AND entity_id = $2"#)
            .bind(&user)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
    };
    if changed > 0 {
        let delta: i32 = if favorite { 1 } else { -1 };
        sqlx::query("UPDATE explore.place_interaction_counts SET favorites = greatest(favorites + $2, 0), updated_at = $3 WHERE entity_id = $1")
            .bind(entity_id)
            .bind(delta)
            .bind(now())
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

/// Sets `user`'s like (`Some(true)`), dislike (`Some(false)`) or neither
/// (`None`) of `entity_id`. Idempotent.
pub async fn set_like(
    pool: &PgPool,
    entity_id: &str,
    user: &str,
    like: Option<bool>,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    set_like_in(&mut tx, entity_id, user, like).await?;
    tx.commit().await?;
    Ok(())
}

/// [`set_like`] inside the caller's transaction.
pub async fn set_like_in(
    tx: &mut PgConnection,
    entity_id: &str,
    user: &str,
    like: Option<bool>,
) -> Result<(), ApiError> {
    let user = user.to_lowercase();
    lock_counts(&mut *tx, entity_id).await?;
    let previous: Option<bool> = sqlx::query_scalar(
        r#"SELECT "like" FROM explore.user_likes WHERE "user" = $1 AND entity_id = $2"#,
    )
    .bind(&user)
    .bind(entity_id)
    .fetch_optional(&mut *tx)
    .await?;
    if previous == like {
        return Ok(());
    }
    match like {
        Some(value) => {
            sqlx::query(
                r#"INSERT INTO explore.user_likes ("user", entity_id, "like", user_activity, created_at, updated_at)
                   VALUES ($1, $2, $3, 0, $4, $4)
                   ON CONFLICT ("user", entity_id) DO UPDATE SET "like" = EXCLUDED."like", updated_at = EXCLUDED.updated_at"#,
            )
            .bind(&user)
            .bind(entity_id)
            .bind(value)
            .bind(now())
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(r#"DELETE FROM explore.user_likes WHERE "user" = $1 AND entity_id = $2"#)
                .bind(&user)
                .bind(entity_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    let count = |v: Option<bool>, want: bool| i32::from(v == Some(want));
    let likes = count(like, true) - count(previous, true);
    let dislikes = count(like, false) - count(previous, false);
    sqlx::query(
        "UPDATE explore.place_interaction_counts SET likes = greatest(likes + $2, 0), dislikes = greatest(dislikes + $3, 0), updated_at = $4 WHERE entity_id = $1",
    )
    .bind(entity_id)
    .bind(likes)
    .bind(dislikes)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// The old lookup: a world by its id or name; a place by its id, falling
/// back to a world of that name unless the id is a place UUID.
pub async fn lookup_entity_for(
    state: &SharedState,
    entity_id: &str,
    is_world: bool,
    viewer: Option<&str>,
) -> Result<Option<PlaceRow>, ApiError> {
    if is_world {
        return state.places.find_world_by_id_for(entity_id, viewer).await;
    }
    if let Some(place) = state.places.find_by_id_for(entity_id, viewer).await? {
        return Ok(Some(place));
    }
    if is_place_id(entity_id) {
        return Ok(None);
    }
    state.places.find_world_by_id_for(entity_id, viewer).await
}

async fn resolve_entity(
    state: &SharedState,
    entity_id: &str,
    is_world: bool,
    viewer: &str,
) -> Result<PlaceRow, ApiError> {
    match lookup_entity_for(state, entity_id, is_world, Some(viewer)).await? {
        Some(entity) => Ok(entity),
        None if is_world => Err(ApiError::not_found(format!(
            "Not found world \"{entity_id}\""
        ))),
        None => Err(ApiError::not_found(format!(
            "Not found entity \"{entity_id}\""
        ))),
    }
}

/// Re-reads the entity after a write, so the answer is what the next read shows.
async fn reread(
    state: &SharedState,
    entity: &PlaceRow,
    viewer: &str,
) -> Result<PlaceRow, ApiError> {
    Ok(lookup_entity_for(
        state,
        &entity.id,
        entity.world && !is_place_id(&entity.id),
        Some(viewer),
    )
    .await?
    .unwrap_or_else(|| entity.clone()))
}

fn body_bool(body: &Option<Json<Value>>, key: &str) -> Option<bool> {
    body.as_ref()
        .and_then(|Json(v)| v.get(key))
        .and_then(Value::as_bool)
}

fn body_like(body: &Option<Json<Value>>) -> Option<Option<bool>> {
    let v = body.as_ref()?.0.get("like")?;
    if v.is_null() {
        Some(None)
    } else {
        v.as_bool().map(Some)
    }
}

fn current_like(row: &PlaceRow) -> Option<bool> {
    if row.user_like {
        Some(true)
    } else if row.user_dislike {
        Some(false)
    } else {
        None
    }
}

async fn patch_favorites(
    state: SharedState,
    method: Method,
    path: &str,
    headers: HeaderMap,
    entity_id: String,
    body: Option<Json<Value>>,
    is_world: bool,
) -> Result<Json<ApiData<FavoritesResult>>, ApiError> {
    state.user_writes()?;
    let user = state.signer(&headers, method.as_str(), path).await?;
    let wanted = body_bool(&body, "favorites").ok_or_else(|| {
        ApiError::bad_request("Invalid favorites body. Expected { favorites: boolean }.")
    })?;
    let entity = resolve_entity(&state, &entity_id, is_world, &user).await?;
    let row = if wanted == entity.user_favorite {
        entity
    } else {
        set_favorite(&state.pool, &entity.id, &user, wanted).await?;
        reread(&state, &entity, &user).await?
    };
    Ok(Json(ApiData::ok(FavoritesResult {
        favorites: row.favorites,
        user_favorite: row.user_favorite,
    })))
}

async fn patch_likes(
    state: SharedState,
    method: Method,
    path: &str,
    headers: HeaderMap,
    entity_id: String,
    body: Option<Json<Value>>,
    is_world: bool,
) -> Result<Json<ApiData<LikesResult>>, ApiError> {
    state.user_writes()?;
    let user = state.signer(&headers, method.as_str(), path).await?;
    let wanted = body_like(&body).ok_or_else(|| {
        ApiError::bad_request("Invalid likes body. Expected { like: boolean|null }.")
    })?;
    let entity = resolve_entity(&state, &entity_id, is_world, &user).await?;
    let row = if current_like(&entity) == wanted {
        entity
    } else {
        set_like(&state.pool, &entity.id, &user, wanted).await?;
        reread(&state, &entity, &user).await?
    };
    Ok(Json(ApiData::ok(LikesResult {
        likes: row.likes,
        dislikes: row.dislikes,
        user_like: row.user_like,
        user_dislike: row.user_dislike,
    })))
}

fn refuse_place_uuid_as_world(world_id: &str, route: &str) -> Result<(), ApiError> {
    if is_place_id(world_id) {
        return Err(ApiError::bad_request(format!(
            "Invalid world ID \"{world_id}\". Use /places/:entity_id/{route} for place entities."
        )));
    }
    Ok(())
}

fn envelope(body: &Option<Json<Value>>) -> Option<Value> {
    body.as_ref()
        .map(|Json(v)| v)
        .filter(|v| catalyst_fed::is_envelope(v))
        .cloned()
}

/// A federation envelope's favorite (`PlaceFavorite`): the action logged
/// and applied in one transaction, answered with its digest.
async fn fed_favorites(
    state: &SharedState,
    headers: &HeaderMap,
    entity_id: &str,
    body: Value,
) -> Result<Response, ApiError> {
    state.user_writes()?;
    let (signed, signer) =
        fed::preflight::<fed::PlaceFavorite>(state, headers, body, fed::Scope::Places).await?;
    if signed.message.place_id != entity_id {
        return Err(ApiError::bad_request(
            "place_id in body does not match path",
        ));
    }
    let place = state
        .places
        .find_by_id_for(entity_id, Some(&signer))
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Not found place \"{entity_id}\"")))?;
    let favorite = signed.message.action == fed::PlaceFavoriteAction::Add;
    let hash = signed.signature_hash();
    let payload = json!({ "place_id": signed.message.place_id, "action": if favorite { "add" } else { "remove" }, "signed_at": signed.message.signed_at });
    let mut tx = state.pool.begin().await?;
    fed::record_action(
        &mut tx,
        &fed::Action {
            scope: fed::Scope::Places,
            signature_hash: &hash,
            signer: &signer,
            subject_id: Some(entity_id),
            action_type: if favorite { "favorite" } else { "unfavorite" },
            payload: &payload,
            signed_at: signed.signed_at,
        },
    )
    .await?;
    set_favorite_in(&mut tx, &place.id, &signer, favorite).await?;
    tx.commit().await?;
    let row = reread(state, &place, &signer).await?;
    Ok(Json(fed::SignedApiData::ok(
        hash,
        FavoritesResult {
            favorites: row.favorites,
            user_favorite: row.user_favorite,
        },
    ))
    .into_response())
}

/// A federation envelope's vote (`PlaceVote`: a positive score likes, a
/// negative one dislikes, zero withdraws).
async fn fed_likes(
    state: &SharedState,
    headers: &HeaderMap,
    entity_id: &str,
    body: Value,
) -> Result<Response, ApiError> {
    state.user_writes()?;
    let (signed, signer) =
        fed::preflight::<fed::PlaceVote>(state, headers, body, fed::Scope::Places).await?;
    if signed.message.place_id != entity_id {
        return Err(ApiError::bad_request(
            "place_id in body does not match path",
        ));
    }
    let place = state
        .places
        .find_by_id_for(entity_id, Some(&signer))
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Not found place \"{entity_id}\"")))?;
    let like = match signed.message.score {
        s if s > 0 => Some(true),
        s if s < 0 => Some(false),
        _ => None,
    };
    let action_type = match like {
        Some(true) => "vote_up",
        Some(false) => "vote_down",
        None => "vote_withdraw",
    };
    let hash = signed.signature_hash();
    let payload = json!({ "place_id": signed.message.place_id, "score": signed.message.score, "signed_at": signed.message.signed_at });
    let mut tx = state.pool.begin().await?;
    fed::record_action(
        &mut tx,
        &fed::Action {
            scope: fed::Scope::Places,
            signature_hash: &hash,
            signer: &signer,
            subject_id: Some(entity_id),
            action_type,
            payload: &payload,
            signed_at: signed.signed_at,
        },
    )
    .await?;
    set_like_in(&mut tx, &place.id, &signer, like).await?;
    tx.commit().await?;
    let row = reread(state, &place, &signer).await?;
    Ok(Json(fed::SignedApiData::ok(
        hash,
        LikesResult {
            likes: row.likes,
            dislikes: row.dislikes,
            user_like: row.user_like,
            user_dislike: row.user_dislike,
        },
    ))
    .into_response())
}

/// `PATCH /api/places/{entity_id}/favorites` `{ favorites: boolean }`, or a `PlaceFavorite` envelope.
pub async fn patch_place_favorites(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(entity_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Response, ApiError> {
    if let Some(env) = envelope(&body) {
        return fed_favorites(&state, &headers, &entity_id, env).await;
    }
    Ok(
        patch_favorites(state, method, uri.path(), headers, entity_id, body, false)
            .await?
            .into_response(),
    )
}

/// `PATCH /api/places/{entity_id}/likes` `{ like: boolean|null }`, or a `PlaceVote` envelope.
pub async fn patch_place_likes(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(entity_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Response, ApiError> {
    if let Some(env) = envelope(&body) {
        return fed_likes(&state, &headers, &entity_id, env).await;
    }
    Ok(
        patch_likes(state, method, uri.path(), headers, entity_id, body, false)
            .await?
            .into_response(),
    )
}

/// `PATCH /api/worlds/{world_id}/favorites`: a place UUID is refused first (envelopes pass).
pub async fn patch_world_favorites(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Response, ApiError> {
    if let Some(env) = envelope(&body) {
        return fed_favorites(&state, &headers, &world_id, env).await;
    }
    refuse_place_uuid_as_world(&world_id, "favorites")?;
    Ok(
        patch_favorites(state, method, uri.path(), headers, world_id, body, true)
            .await?
            .into_response(),
    )
}

/// `PATCH /api/worlds/{world_id}/likes`.
pub async fn patch_world_likes(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Response, ApiError> {
    if let Some(env) = envelope(&body) {
        return fed_likes(&state, &headers, &world_id, env).await;
    }
    refuse_place_uuid_as_world(&world_id, "likes")?;
    Ok(
        patch_likes(state, method, uri.path(), headers, world_id, body, true)
            .await?
            .into_response(),
    )
}

async fn resolve_destination_entity(
    state: &SharedState,
    entity_id: &str,
    user: &str,
) -> Result<PlaceRow, ApiError> {
    let is_world = !is_place_id(entity_id);
    let lookup = if is_world {
        entity_id.to_string()
    } else {
        entity_id.to_lowercase()
    };
    resolve_entity(state, &lookup, is_world, user).await
}

async fn destination_favorite(
    state: &SharedState,
    entity_id: &str,
    user: &str,
    favorite: bool,
) -> Result<InteractionSummary, ApiError> {
    let entity = resolve_destination_entity(state, entity_id, user).await?;
    if favorite == entity.user_favorite {
        return Ok(InteractionSummary::from(&entity));
    }
    set_favorite(&state.pool, &entity.id, user, favorite).await?;
    Ok(InteractionSummary::from(
        &reread(state, &entity, user).await?,
    ))
}

async fn destination_like(
    state: &SharedState,
    entity_id: &str,
    user: &str,
    like: Option<bool>,
) -> Result<InteractionSummary, ApiError> {
    let entity = resolve_destination_entity(state, entity_id, user).await?;
    if current_like(&entity) == like {
        return Ok(InteractionSummary::from(&entity));
    }
    set_like(&state.pool, &entity.id, user, like).await?;
    Ok(InteractionSummary::from(
        &reread(state, &entity, user).await?,
    ))
}

/// `{ like: boolean }` and nothing else.
pub fn parse_like_body(body: Option<&Value>) -> Result<bool, ApiError> {
    let invalid = || ApiError::bad_request("Invalid likes body. Expected { like: boolean }.");
    let object = body.and_then(Value::as_object).ok_or_else(invalid)?;
    if object.len() != 1 {
        return Err(invalid());
    }
    object
        .get("like")
        .and_then(Value::as_bool)
        .ok_or_else(invalid)
}

async fn v1_actor(
    state: &SharedState,
    headers: &HeaderMap,
    method: &Method,
    path: &str,
) -> Result<String, ApiError> {
    state.user_writes()?;
    state.signer(headers, method.as_str(), path).await
}

pub async fn put_v1_destination_favorites(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiData<InteractionSummary>>, ApiError> {
    let user = v1_actor(&state, &headers, &method, uri.path()).await?;
    Ok(Json(ApiData::ok(
        destination_favorite(&state, &id, &user, true).await?,
    )))
}

pub async fn delete_v1_destination_favorites(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiData<InteractionSummary>>, ApiError> {
    let user = v1_actor(&state, &headers, &method, uri.path()).await?;
    Ok(Json(ApiData::ok(
        destination_favorite(&state, &id, &user, false).await?,
    )))
}

pub async fn put_v1_destination_likes(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<ApiData<InteractionSummary>>, ApiError> {
    let user = v1_actor(&state, &headers, &method, uri.path()).await?;
    let like = parse_like_body(body.as_ref().map(|Json(v)| v))?;
    Ok(Json(ApiData::ok(
        destination_like(&state, &id, &user, Some(like)).await?,
    )))
}

pub async fn delete_v1_destination_likes(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiData<InteractionSummary>>, ApiError> {
    let user = v1_actor(&state, &headers, &method, uri.path()).await?;
    Ok(Json(ApiData::ok(
        destination_like(&state, &id, &user, None).await?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn like_body_accepts_exactly_a_boolean_like() {
        assert!(parse_like_body(Some(&json!({ "like": true }))).unwrap());
        assert!(!parse_like_body(Some(&json!({ "like": false }))).unwrap());
    }

    #[test]
    fn like_body_rejects_everything_else() {
        assert!(parse_like_body(None).is_err());
        assert!(parse_like_body(Some(&json!({}))).is_err());
        assert!(parse_like_body(Some(&json!({ "like": "yes" }))).is_err());
        assert!(parse_like_body(Some(&json!({ "like": null }))).is_err());
        assert!(parse_like_body(Some(&json!({ "like": true, "extra": 1 }))).is_err());
        assert!(parse_like_body(Some(&json!([true]))).is_err());
    }

    #[test]
    fn place_uuid_guard() {
        assert!(is_place_id("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!is_place_id("my-world.dcl.eth"));
        assert!(!is_place_id("123e4567e89b12d3a456426614174000"));
        assert!(!is_place_id("123e4567-e89b-12d3-a456-42661417400g"));
        assert!(!is_place_id(""));
    }

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::shared::signed_fetch::test_support::{address, signed_headers};
    use crate::{ExploreState, Settings, router};

    async fn call(
        app: &axum::Router,
        method: &str,
        path: &str,
        body: Option<Value>,
        signed: bool,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if signed {
            for (k, v) in signed_headers(&method.to_lowercase(), path, "{}", true)
                .await
                .iter()
            {
                req = req.header(k, v);
            }
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

    /// Favorites and likes of a mirrored place and a world: the counters are
    /// upstream's plus this node's, idempotent, rebuildable from the facts;
    /// the switch answers 503 and an unsigned write 401.
    #[tokio::test]
    async fn interactions_count_on_top_of_upstream_and_rebuild_from_the_facts() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let place = uuid::Uuid::new_v4().to_string();
        let world = format!(
            "fav{}.dcl.eth",
            &uuid::Uuid::new_v4().simple().to_string()[..10]
        );
        let world_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO explore.place (id, base_position, title, favorites, likes, dislikes, raw) VALUES ($1, '1,1', 'p', 10, 5, 2, $2)")
            .bind(&place)
            .bind(json!({ "positions": ["1,1"] }))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.place (id, base_position, title, favorites, raw) VALUES ($1, '0,0', 'w', 3, $2)")
            .bind(&world_id)
            .bind(json!({ "world": true, "world_name": world }))
            .execute(&pool)
            .await
            .unwrap();

        let closed = router(Arc::new(ExploreState::new(
            pool.clone(),
            Settings::for_tests(),
            None,
        )));
        let (status, body) = call(
            &closed,
            "PATCH",
            &format!("/api/places/{place}/favorites"),
            Some(json!({ "favorites": true })),
            true,
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["code"], json!("service_unavailable"));

        let settings = Settings {
            user_writes_enabled: true,
            ..Settings::for_tests()
        };
        let app = router(Arc::new(ExploreState::new(pool.clone(), settings, None)));
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/api/places/{place}/favorites"),
            Some(json!({ "favorites": true })),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/places/{place}/favorites"),
            Some(json!({ "favorites": "yes" })),
            true,
        )
        .await;
        assert_eq!(
            (status, &body["message"]),
            (
                StatusCode::BAD_REQUEST,
                &json!("Invalid favorites body. Expected { favorites: boolean }.")
            )
        );

        for _ in 0..2 {
            let (status, body) = call(
                &app,
                "PATCH",
                &format!("/api/places/{place}/favorites"),
                Some(json!({ "favorites": true })),
                true,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(
                body["data"],
                json!({ "favorites": 11, "user_favorite": true }),
                "idempotent"
            );
        }
        let (_, body) = call(
            &app,
            "PATCH",
            &format!("/api/places/{place}/likes"),
            Some(json!({ "like": false })),
            true,
        )
        .await;
        assert_eq!(
            body["data"],
            json!({ "likes": 5, "dislikes": 3, "user_like": false, "user_dislike": true })
        );
        let (_, body) = call(
            &app,
            "PATCH",
            &format!("/api/places/{place}/likes"),
            Some(json!({ "like": true })),
            true,
        )
        .await;
        assert_eq!(
            body["data"],
            json!({ "likes": 6, "dislikes": 2, "user_like": true, "user_dislike": false })
        );

        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/worlds/{place}/likes"),
            Some(json!({ "like": true })),
            true,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "a place UUID on the worlds route: {body}"
        );
        let (_, body) = call(
            &app,
            "PATCH",
            &format!("/api/worlds/{}/favorites", world.to_uppercase()),
            Some(json!({ "favorites": true })),
            true,
        )
        .await;
        assert_eq!(
            body["data"],
            json!({ "favorites": 4, "user_favorite": true })
        );

        let (status, body) = call(
            &app,
            "PUT",
            &format!("/v1/destinations/{world}/likes"),
            Some(json!({ "like": true })),
            true,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["data"],
            json!({ "likes": 1, "dislikes": 0, "favorites": 4, "user_like": true, "user_dislike": false, "user_favorite": true })
        );
        let (_, body) = call(
            &app,
            "DELETE",
            &format!("/v1/destinations/{world}/favorites"),
            None,
            true,
        )
        .await;
        assert_eq!(body["data"]["favorites"], json!(3));
        let (status, _) = call(
            &app,
            "PUT",
            &format!("/v1/destinations/{world}/likes"),
            Some(json!({ "like": null })),
            true,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = call(
            &app,
            "PUT",
            &format!("/v1/destinations/{}/favorites", uuid::Uuid::new_v4()),
            None,
            true,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // the reads show the same numbers, the viewer's flags when signed
        let (_, body) = call(&app, "GET", &format!("/api/places/{place}"), None, true).await;
        assert_eq!(
            (
                body["data"]["favorites"].clone(),
                body["data"]["likes"].clone(),
                body["data"]["user_favorite"].clone()
            ),
            (json!(11), json!(6), json!(true))
        );
        let (_, body) = call(
            &app,
            "GET",
            &format!("/v1/destinations/{world}"),
            None,
            false,
        )
        .await;
        assert_eq!(
            (
                body["data"]["favorites"].clone(),
                body["data"]["likes"].clone()
            ),
            (json!(3), json!(1))
        );

        // the counters equal their rebuild from the facts
        let counts = || async {
            let mut rows: Vec<(String, i32, i32, i32)> = sqlx::query_as("SELECT entity_id, favorites, likes, dislikes FROM explore.place_interaction_counts WHERE entity_id = ANY($1)")
                .bind(vec![place.clone(), world_id.clone()])
                .fetch_all(&pool)
                .await
                .unwrap();
            rows.sort();
            rows
        };
        let mut expected = vec![(place.clone(), 1, 1, 0), (world_id.clone(), 0, 1, 0)];
        expected.sort();
        assert_eq!(counts().await, expected);
        sqlx::query("UPDATE explore.place_interaction_counts SET favorites = 99, dislikes = 7 WHERE entity_id = ANY($1)").bind(vec![place.clone(), world_id.clone()]).execute(&pool).await.unwrap();
        sqlx::query(REBUILD_COUNTS)
            .bind(now())
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            counts().await,
            expected,
            "the rebuild restores the counters from the facts"
        );
        let rows: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM explore.user_favorites WHERE "user" = $1 AND entity_id = $2"#,
        )
        .bind(address())
        .bind(&place)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn the_patch_bodies_read_their_one_key() {
        assert_eq!(
            body_bool(&Some(Json(json!({ "favorites": true }))), "favorites"),
            Some(true)
        );
        assert_eq!(
            body_bool(&Some(Json(json!({ "favorites": "true" }))), "favorites"),
            None
        );
        assert_eq!(body_like(&Some(Json(json!({ "like": null })))), Some(None));
        assert_eq!(
            body_like(&Some(Json(json!({ "like": false })))),
            Some(Some(false))
        );
        assert_eq!(body_like(&Some(Json(json!({})))), None);
        assert_eq!(body_like(&None), None);
    }
}
