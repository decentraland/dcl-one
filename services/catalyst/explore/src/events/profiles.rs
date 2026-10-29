//! The events' signed-envelope writes and the profile settings reads:
//!
//! - `PATCH /api/profiles/me/settings` (a `ProfileSettingsUpdate` envelope
//!   for the signer itself) and `PATCH /api/profiles/{id}/settings` (a
//!   moderator's for anyone); `GET` of both and the moderators'
//!   `GET /api/profiles/settings` behind a signed fetch;
//! - `POST /api/schedules` and `PATCH /api/schedules/{id}` (a moderator's
//!   `ScheduleUpsert` envelope);
//! - `/api/profiles/subscriptions` answers 410 on every verb: web-push was
//!   retired before the port.
//!
//! A moderator is a wallet in `explore.moderators`. Each envelope is logged
//! in `explore.signed_actions` and applied in one transaction: the signer's
//! own settings behind `DATA_WRITES_ENABLED`, the moderator-only writes (the
//! schedules, another profile's settings) behind `OPS_WRITES_ENABLED`, the
//! switch of the node's moderation.

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Method};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use super::ScheduleRecord;
use crate::SharedState;
use crate::fed::{self, ProfileSettingsUpdate, ScheduleUpsert};
use crate::shared::errors::{ApiData, ApiError};

/// Moderators may read everyone's settings; the list is capped (moderator
/// reads of a table with one row per wallet that saved settings).
const SETTINGS_LIST_CAP: i64 = 10_000;

pub async fn is_moderator(pool: &PgPool, address: &str) -> Result<bool, ApiError> {
    let row: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM explore.moderators WHERE address = $1")
        .bind(address.to_ascii_lowercase())
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

pub async fn require_moderator(pool: &PgPool, signer: &str) -> Result<(), ApiError> {
    if is_moderator(pool, signer).await? {
        Ok(())
    } else {
        Err(ApiError::forbidden("Forbidden"))
    }
}

fn ok(data: Value) -> Json<Value> {
    Json(json!({ "ok": true, "data": data }))
}

fn envelope(body: Value) -> Result<Value, ApiError> {
    if catalyst_fed::is_envelope(&body) {
        Ok(body)
    } else {
        Err(ApiError::bad_request("missing signed body"))
    }
}

#[derive(sqlx::FromRow)]
struct SettingsRow {
    email: Option<String>,
    email_verified: bool,
    use_local_time: bool,
    notify_by_email: bool,
    notify_by_browser: bool,
    permissions: Value,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

fn stamp(t: chrono::NaiveDateTime) -> String {
    t.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The wallet's settings, or the defaults for one that never saved any.
pub async fn load_settings(conn: &mut PgConnection, user: &str) -> Result<Value, ApiError> {
    let user = user.to_ascii_lowercase();
    let row: Option<SettingsRow> = sqlx::query_as(
        r#"SELECT email, email_verified, use_local_time, notify_by_email, notify_by_browser, permissions, created_at, updated_at
           FROM explore.event_profile_settings WHERE "user" = $1"#,
    )
    .bind(&user)
    .fetch_optional(conn)
    .await?;
    Ok(match row {
        Some(r) => json!({
            "user": user,
            "email": r.email,
            "email_verified": r.email_verified,
            "use_local_time": r.use_local_time,
            "notify_by_email": r.notify_by_email,
            "notify_by_browser": r.notify_by_browser,
            "permissions": r.permissions,
            "created_at": stamp(r.created_at),
            "updated_at": stamp(r.updated_at),
        }),
        None => json!({
            "user": user,
            "email": null,
            "email_verified": false,
            "use_local_time": true,
            "notify_by_email": false,
            "notify_by_browser": false,
            "permissions": [],
            "created_at": "1970-01-01T00:00:00.000Z",
            "updated_at": "1970-01-01T00:00:00.000Z",
        }),
    })
}

async fn apply_profile_settings(
    pool: &PgPool,
    signed: &catalyst_fed::Signed<ProfileSettingsUpdate>,
    signer: &str,
) -> Result<Value, ApiError> {
    let m = &signed.message;
    let target = m.target.to_ascii_lowercase();
    let hash = signed.signature_hash();
    let payload = serde_json::to_value(m).unwrap_or_else(|_| json!({}));
    let now = chrono::Utc::now().naive_utc();
    let mut tx = pool.begin().await?;
    fed::record_action(
        &mut tx,
        &fed::Action {
            scope: fed::Scope::Events,
            signature_hash: &hash,
            signer,
            subject_id: Some(&target),
            action_type: "ProfileSettingsUpdate",
            payload: &payload,
            signed_at: signed.signed_at,
        },
    )
    .await?;
    sqlx::query(
        r#"INSERT INTO explore.event_profile_settings
               ("user", email, email_verified, use_local_time, notify_by_email, notify_by_browser, permissions, created_at, updated_at)
           VALUES ($1, $2, COALESCE($3, false), COALESCE($4, true), COALESCE($5, false), COALESCE($6, false), COALESCE($7, '[]'::jsonb), $8, $8)
           ON CONFLICT ("user") DO UPDATE SET
               email = COALESCE($2, explore.event_profile_settings.email),
               email_verified = COALESCE($3, explore.event_profile_settings.email_verified),
               use_local_time = COALESCE($4, explore.event_profile_settings.use_local_time),
               notify_by_email = COALESCE($5, explore.event_profile_settings.notify_by_email),
               notify_by_browser = COALESCE($6, explore.event_profile_settings.notify_by_browser),
               permissions = COALESCE($7, explore.event_profile_settings.permissions),
               updated_at = $8"#,
    )
    .bind(&target)
    .bind(m.email.as_deref())
    .bind(m.email_verified)
    .bind(m.use_local_time)
    .bind(m.notify_by_email)
    .bind(m.notify_by_browser)
    .bind(m.permissions.as_ref().map(|p| json!(p)))
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let settings = load_settings(&mut tx, &target).await?;
    tx.commit().await?;
    Ok(settings)
}

/// `GET /api/profiles/settings`: every saved profile, for a moderator.
pub async fn list_profile_settings(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = state.signer(&headers, method.as_str(), uri.path()).await?;
    require_moderator(&state.pool, &user).await?;
    let users: Vec<String> = sqlx::query_scalar(
        r#"SELECT "user" FROM explore.event_profile_settings ORDER BY "user" ASC LIMIT $1"#,
    )
    .bind(SETTINGS_LIST_CAP)
    .fetch_all(&state.pool)
    .await?;
    let mut conn = state.pool.acquire().await?;
    let mut out = Vec::with_capacity(users.len());
    for user in users {
        out.push(load_settings(&mut conn, &user).await?);
    }
    Ok(ok(json!(out)))
}

/// `GET /api/profiles/me/settings`: the signer's own, with the retired `subscriptions` empty.
pub async fn get_my_profile_settings(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = state.signer(&headers, method.as_str(), uri.path()).await?;
    let mut conn = state.pool.acquire().await?;
    let mut settings = load_settings(&mut conn, &user).await?;
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("subscriptions".into(), json!([]));
    }
    Ok(ok(settings))
}

/// `PATCH /api/profiles/me/settings`: a `ProfileSettingsUpdate` for the signer itself.
pub async fn update_my_profile_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    state.user_writes()?;
    let body = envelope(body)?;
    let (signed, signer) =
        fed::preflight::<ProfileSettingsUpdate>(&state, &headers, body, fed::Scope::Events).await?;
    if !signed.message.target.eq_ignore_ascii_case(&signer) {
        return Err(ApiError::forbidden(
            "me/settings only edits the signer's own profile",
        ));
    }
    Ok(ok(
        apply_profile_settings(&state.pool, &signed, &signer).await?
    ))
}

/// `GET /api/profiles/{profile_id}/settings`: a moderator's read.
pub async fn get_profile_settings(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user = state.signer(&headers, method.as_str(), uri.path()).await?;
    require_moderator(&state.pool, &user).await?;
    let mut conn = state.pool.acquire().await?;
    Ok(ok(load_settings(&mut conn, &profile_id).await?))
}

/// `PATCH /api/profiles/{profile_id}/settings`: a moderator's `ProfileSettingsUpdate`.
pub async fn update_profile_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    state.operator_writes()?;
    let body = envelope(body)?;
    let (signed, signer) =
        fed::preflight::<ProfileSettingsUpdate>(&state, &headers, body, fed::Scope::Events).await?;
    if !signed.message.target.eq_ignore_ascii_case(&profile_id) {
        return Err(ApiError::bad_request(
            "target does not match path profile_id",
        ));
    }
    require_moderator(&state.pool, &signer).await?;
    Ok(ok(
        apply_profile_settings(&state.pool, &signed, &signer).await?
    ))
}

const SUBSCRIPTIONS_RETIRED: &str = "Web-push subscriptions are deprecated and no longer supported";

/// `GET`/`POST`/`DELETE /api/profiles/subscriptions`: 410 Gone.
pub async fn profile_subscriptions() -> ApiError {
    ApiError::http(410, SUBSCRIPTIONS_RETIRED)
}

async fn apply_schedule(
    state: &SharedState,
    headers: &HeaderMap,
    body: Value,
    path_id: Option<String>,
) -> Result<Json<ApiData<ScheduleRecord>>, ApiError> {
    state.operator_writes()?;
    let body = envelope(body)?;
    let (signed, signer) =
        fed::preflight::<ScheduleUpsert>(state, headers, body, fed::Scope::Events).await?;
    if let Some(id) = &path_id
        && signed.message.schedule_id.as_ref() != Some(id)
    {
        return Err(ApiError::bad_request(
            "schedule_id in body does not match path",
        ));
    }
    require_moderator(&state.pool, &signer).await?;
    let m = &signed.message;
    let id = m
        .schedule_id
        .clone()
        .unwrap_or_else(|| fed::schedule_id(&signer, &m.name, &signed.nonce));
    let hash = signed.signature_hash();
    let payload = serde_json::to_value(m).unwrap_or_else(|_| json!({}));
    let at = |secs: i64| chrono::DateTime::from_timestamp(secs, 0).map(|t| t.naive_utc());
    let now = chrono::Utc::now().naive_utc();
    let mut tx = state.pool.begin().await?;
    fed::record_action(
        &mut tx,
        &fed::Action {
            scope: fed::Scope::Events,
            signature_hash: &hash,
            signer: &signer,
            subject_id: Some(&id),
            action_type: "ScheduleUpsert",
            payload: &payload,
            signed_at: signed.signed_at,
        },
    )
    .await?;
    sqlx::query(
        r#"INSERT INTO explore.schedules_local (id, name, description, image, theme, background, active_since, active_until, active, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
           ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name, description = EXCLUDED.description, image = EXCLUDED.image, theme = EXCLUDED.theme,
               background = EXCLUDED.background, active_since = EXCLUDED.active_since, active_until = EXCLUDED.active_until,
               active = EXCLUDED.active, updated_at = EXCLUDED.updated_at"#,
    )
    .bind(&id)
    .bind(&m.name)
    .bind(m.description.as_deref())
    .bind(m.image.as_deref())
    .bind(m.theme.as_deref())
    .bind(json!(m.background))
    .bind(at(m.active_since))
    .bind(at(m.active_until))
    .bind(m.active)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    state.schedules.invalidate();
    let schedule = state
        .schedules
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::internal("schedule vanished after write"))?;
    Ok(Json(ApiData::ok(schedule)))
}

/// `POST /api/schedules`: a moderator's `ScheduleUpsert` (an id minted when absent).
pub async fn create_schedule(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<ApiData<ScheduleRecord>>, ApiError> {
    apply_schedule(&state, &headers, body, None).await
}

/// `PATCH /api/schedules/{schedule_id}`: the envelope must name the same id.
pub async fn patch_schedule(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(schedule_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<ApiData<ScheduleRecord>>, ApiError> {
    apply_schedule(&state, &headers, body, Some(schedule_id)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_retired_subscriptions_answer_410() {
        use axum::response::IntoResponse;
        assert_eq!(profile_subscriptions().await.into_response().status(), 410);
    }

    #[test]
    fn a_body_without_the_envelope_keys_is_refused() {
        assert_eq!(
            envelope(json!({ "email": "a@b.c" }))
                .unwrap_err()
                .to_string(),
            "missing signed body"
        );
        assert!(envelope(json!({ "domain": {}, "message": {}, "signature": "0x" })).is_ok());
    }
}
