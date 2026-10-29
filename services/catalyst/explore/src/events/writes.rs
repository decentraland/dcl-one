//! The events write routes: `POST /api/events`, `PATCH`/`DELETE
//! /api/events/{id}` and the RSVPs `POST`/`DELETE /api/events/{id}/attendees`.
//!
//! Two callers, as before: the events admin bearer
//! (`CATALYRST_EVENTS_ADMIN_TOKEN`) creates as `admin`, moderates with the
//! old admin body and soft-deletes as the admin, behind
//! `OPS_WRITES_ENABLED`; a signed wallet creates its own event (pending
//! approval), edits and deletes its own only (403 otherwise) and RSVPs,
//! behind `DATA_WRITES_ENABLED`. An owner's edit of an approved event's
//! content (or its featured item) sends it back to approval. Every write
//! lands in `explore.event` with its `explore.events_local` audit in one
//! transaction (`events::moderation::store`). Only events created here
//! (`origin = 'local'`) take their owner's edit and delete: on a mirrored
//! event those answer 409 naming the upstream (the mirror writes its
//! fields). The admin's moderation and delete of a mirrored event are local
//! policy (`explore.event_moderation`, re-applied by every pass; the delete
//! hides it from the public reads), and an RSVP to it is a local row the
//! count adds; the mirror deletes both with the event.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Method};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::moderation::{
    self, AdminPatch, CreateEvent, MAX_EVENT_DURATION_MS, PAST_FINISH_AT_MESSAGE,
    PAST_RECURRENT_UNTIL_MESSAGE, finish_at_of, is_soft_deleted, max_duration_message,
    normalized_series, parse_rfc3339, series_may_have_future_dates, validate_featured_item,
};
use super::{EventAttendeeRecord, EventRecord};
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiError};

const ADMIN_SIGNER: &str = "admin";
const MAX_RECURRENT_PAST_ITERATIONS: i64 = 50_000;
const DATE_FIELDS: [&str; 11] = [
    "start_at",
    "duration",
    "recurrent",
    "recurrent_frequency",
    "recurrent_interval",
    "recurrent_count",
    "recurrent_until",
    "recurrent_setpos",
    "recurrent_monthday",
    "recurrent_weekday_mask",
    "recurrent_month_mask",
];

fn deserialize_some<'de, T, D>(d: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(d).map(Some)
}

/// The owner's edit body (unknown fields refused; the moderation fields are
/// accepted and ignored, as before).
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateEventBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub image_vertical: Option<String>,
    #[serde(default)]
    pub start_at: Option<String>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub all_day: Option<bool>,
    #[serde(default)]
    pub recurrent: Option<bool>,
    #[serde(default)]
    pub recurrent_frequency: Option<String>,
    #[serde(default)]
    pub recurrent_setpos: Option<i64>,
    #[serde(default)]
    pub recurrent_monthday: Option<i64>,
    #[serde(default)]
    pub recurrent_weekday_mask: Option<i64>,
    #[serde(default)]
    pub recurrent_month_mask: Option<i64>,
    #[serde(default)]
    pub recurrent_interval: Option<i64>,
    #[serde(default)]
    pub recurrent_count: Option<i64>,
    #[serde(default)]
    pub recurrent_until: Option<String>,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub server: Option<Option<String>>,
    #[serde(default)]
    pub contact: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub schedules: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub world: Option<Option<bool>>,
    #[serde(default)]
    pub community_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub featured_item: Option<Option<String>>,
    #[serde(default)]
    pub approved: Option<bool>,
    #[serde(default)]
    pub rejected: Option<bool>,
    #[serde(default)]
    pub rejection_reason: Option<String>,
    #[serde(default)]
    pub highlighted: Option<bool>,
    #[serde(default)]
    pub trending: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Serialize)]
pub struct DeleteEventBody {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
}

fn normalize_featured_item(value: Option<&str>) -> Result<Option<String>, ApiError> {
    match value.filter(|v| !v.is_empty()) {
        Some(urn) => {
            validate_featured_item(urn).map_err(ApiError::bad_request)?;
            Ok(Some(urn.to_owned()))
        }
        None => Ok(None),
    }
}

fn dedup_schedules(schedules: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    schedules
        .into_iter()
        .filter(|s| seen.insert(s.clone()))
        .collect()
}

fn location_touched(body: &UpdateEventBody) -> bool {
    body.x.is_some() || body.y.is_some() || body.server.is_some() || body.world.is_some()
}

fn recurrence_touched(body: &UpdateEventBody) -> bool {
    body.recurrent.is_some()
        || body.recurrent_frequency.is_some()
        || body.recurrent_setpos.is_some()
        || body.recurrent_monthday.is_some()
        || body.recurrent_weekday_mask.is_some()
        || body.recurrent_month_mask.is_some()
        || body.recurrent_interval.is_some()
        || body.recurrent_count.is_some()
        || body.recurrent_until.is_some()
        || body.start_at.is_some()
        || body.duration.is_some()
}

fn content_touched(body: &UpdateEventBody) -> bool {
    body.name.is_some()
        || body.description.is_some()
        || body.image.is_some()
        || body.image_vertical.is_some()
        || body.url.is_some()
        || body.categories.is_some()
        || body.schedules.is_some()
        || body.community_id.is_some()
        || location_touched(body)
        || recurrence_touched(body)
}

fn featured_item_changed(raw: &Map<String, Value>, body: &UpdateEventBody) -> bool {
    let Some(next) = &body.featured_item else {
        return false;
    };
    next.as_deref().filter(|v| !v.is_empty()) != raw.get("featured_item").and_then(Value::as_str)
}

fn date_key_present(body: &[u8]) -> bool {
    serde_json::from_slice::<Map<String, Value>>(body)
        .map(|m| DATE_FIELDS.iter().any(|k| m.contains_key(*k)))
        .unwrap_or(false)
}

fn set_opt<T: Serialize>(map: &mut Map<String, Value>, key: &str, val: &Option<T>) {
    if let Some(v) = val {
        map.insert(key.into(), json!(v));
    }
}

/// The old owner edit onto the stored document.
pub fn apply_owner_edit(
    raw: &mut Map<String, Value>,
    body: &UpdateEventBody,
    date_key_present: bool,
) -> Result<(), ApiError> {
    if let Some(name) = &body.name {
        if name.chars().count() > 150 {
            return Err(ApiError::bad_request("name must be at most 150 characters"));
        }
        raw.insert("name".into(), json!(name));
    }
    set_opt(raw, "description", &body.description);
    for (key, img) in [
        ("image", &body.image),
        ("image_vertical", &body.image_vertical),
    ] {
        if let Some(img) = img {
            let img = crate::sanitize::sanitize_asset_url(img).map_err(ApiError::bad_request)?;
            raw.insert(key.into(), json!(img));
        }
    }
    if let Some(server) = &body.server {
        raw.insert("server".into(), json!(server));
    }
    set_opt(raw, "contact", &body.contact);
    set_opt(raw, "details", &body.details);
    set_opt(raw, "recurrent_frequency", &body.recurrent_frequency);
    set_opt(raw, "recurrent_until", &body.recurrent_until);
    set_opt(raw, "community_id", &body.community_id);
    if let Some(item) = &body.featured_item {
        let item = normalize_featured_item(item.as_deref())?;
        raw.insert("featured_item".into(), json!(item));
    }
    set_opt(raw, "all_day", &body.all_day);
    set_opt(raw, "recurrent", &body.recurrent);
    if let Some(world) = body.world {
        raw.insert("world".into(), json!(world.unwrap_or(false)));
    }
    set_opt(raw, "recurrent_setpos", &body.recurrent_setpos);
    set_opt(raw, "recurrent_monthday", &body.recurrent_monthday);
    set_opt(raw, "recurrent_weekday_mask", &body.recurrent_weekday_mask);
    set_opt(raw, "recurrent_month_mask", &body.recurrent_month_mask);
    set_opt(raw, "recurrent_interval", &body.recurrent_interval);
    set_opt(raw, "recurrent_count", &body.recurrent_count);
    if let Some(duration) = body.duration {
        let stored = raw.get("duration").and_then(Value::as_i64).unwrap_or(0);
        if duration > stored.max(MAX_EVENT_DURATION_MS) {
            return Err(ApiError::bad_request(max_duration_message()));
        }
        raw.insert("duration".into(), json!(duration));
    }
    if let Some(url) = &body.url {
        let url = crate::sanitize::validate_event_url(url).map_err(ApiError::bad_request)?;
        raw.insert("url".into(), json!(url));
    }
    if let Some(cats) = &body.categories {
        raw.insert("categories".into(), json!(cats));
    }
    if let Some(scheds) = &body.schedules {
        raw.insert("schedules".into(), json!(dedup_schedules(scheds.clone())));
    }
    for (key, v) in [("x", body.x), ("y", body.y)] {
        if let Some(v) = v {
            if !(-170..=170).contains(&v) {
                return Err(ApiError::bad_request("coordinates out of range"));
            }
            raw.insert(key.into(), json!(v));
        }
    }
    if let Some(start_at) = &body.start_at {
        let parsed =
            parse_rfc3339(start_at).ok_or_else(|| ApiError::bad_request("invalid start_at"))?;
        raw.insert("start_at".into(), json!(parsed.to_rfc3339()));
    }
    if recurrence_touched(body) || date_key_present {
        recompute_occurrences(raw)?;
        reject_past_dates(raw)?;
    }
    let x = raw.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
    let y = raw.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
    raw.insert("position".into(), json!([x, y]));
    raw.insert("coordinates".into(), json!([x, y]));
    Ok(())
}

fn recompute_occurrences(raw: &mut Map<String, Value>) -> Result<(), ApiError> {
    let start_at = raw
        .get("start_at")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339);
    let duration = raw.get("duration").and_then(Value::as_i64).unwrap_or(0);
    if raw
        .get("recurrent_count")
        .and_then(Value::as_i64)
        .is_some_and(|c| c > MAX_RECURRENT_PAST_ITERATIONS)
    {
        return Err(ApiError::bad_request(
            "recurrence would exceed the maximum number of occurrences",
        ));
    }
    if let Some(start) = start_at {
        let finish_at = finish_at_of(start, duration)
            .ok_or_else(|| ApiError::bad_request(max_duration_message()))?;
        raw.insert("recurrent_dates".into(), json!([start.to_rfc3339()]));
        raw.insert("next_start_at".into(), json!(start.to_rfc3339()));
        raw.insert("next_finish_at".into(), json!(finish_at.to_rfc3339()));
        raw.insert("finish_at".into(), json!(finish_at.to_rfc3339()));
    }
    Ok(())
}

fn reject_past_dates(raw: &Map<String, Value>) -> Result<(), ApiError> {
    let now = Utc::now();
    let recurrent = raw
        .get("recurrent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let frequency = raw.get("recurrent_frequency").and_then(Value::as_str);
    let count = raw.get("recurrent_count").and_then(Value::as_i64);
    let until = raw
        .get("recurrent_until")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339);
    if !series_may_have_future_dates(recurrent, frequency, count, until, now)
        && raw
            .get("finish_at")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339)
            .is_some_and(|f| f <= now)
    {
        return Err(ApiError::bad_request(PAST_FINISH_AT_MESSAGE));
    }
    if normalized_series(recurrent, frequency, count, until) && until.is_some_and(|u| u <= now) {
        return Err(ApiError::bad_request(PAST_RECURRENT_UNTIL_MESSAGE));
    }
    Ok(())
}

/// The stored document, its owner (the document's `user`, else the column)
/// and whether it is mirrored. The owner never writes a mirrored event: that
/// answers 409 naming the upstream (`owner_write`); the operator's delete of
/// one is local moderation ([`moderation::hide_mirrored`]).
async fn load_raw(
    state: &SharedState,
    id: &str,
    owner_write: bool,
) -> Result<Option<(Map<String, Value>, String, bool)>, ApiError> {
    super::mirror::reclaim_local(&state.pool).await?;
    let row: Option<(Value, Option<String>, String)> =
        sqlx::query_as("SELECT raw, user_creator, origin FROM explore.event WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let mirrored = row
        .as_ref()
        .is_some_and(|(_, _, origin)| origin != moderation::LOCAL);
    if mirrored && owner_write {
        return Err(moderation::mirrored_event(&state.pool, id).await);
    }
    Ok(row.map(|(raw, creator, _)| {
        let owner = raw
            .get("user")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(creator)
            .unwrap_or_default()
            .to_lowercase();
        let map = match raw {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        (map, owner, mirrored)
    }))
}

async fn answer(state: &SharedState, id: &str) -> Result<Json<ApiData<EventRecord>>, ApiError> {
    let record = state
        .events
        .get(id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Not found event \"{id}\"")))?;
    Ok(Json(ApiData::ok(record)))
}

fn not_found(id: &str) -> ApiError {
    ApiError::not_found(format!("Not found event \"{id}\""))
}

/// `POST /api/events`.
pub async fn create_event(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiData<EventRecord>>, ApiError> {
    let admin = super::handlers::is_admin_request(&headers, &state);
    let signer = if admin {
        state.operator_writes()?;
        ADMIN_SIGNER.to_string()
    } else {
        state.user_writes()?;
        state.signer(&headers, method.as_str(), uri.path()).await?
    };
    let body: CreateEvent = serde_json::from_slice(&body)
        .map_err(|e| ApiError::bad_request(format!("invalid event body: {e}")))?;
    let id = moderation::create_as(&state.pool, &body, &signer, &signer).await?;
    answer(&state, &id).await
}

/// `PATCH /api/events/{id}`: the admin's moderation or the owner's edit.
pub async fn patch_event(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(event_id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<EventRecord>>, ApiError> {
    if super::handlers::is_admin_request(&headers, &state) {
        state.operator_writes()?;
        let patch: AdminPatch = serde_json::from_slice(&body)
            .map_err(|e| ApiError::bad_request(format!("invalid patch body: {e}")))?;
        if patch.actor.as_ref().is_some_and(|a| a.chars().count() > 42) {
            return Err(ApiError::bad_request("actor too long"));
        }
        let actor = patch
            .actor
            .clone()
            .unwrap_or_else(|| ADMIN_SIGNER.to_string());
        if !moderation::moderate(&state.pool, &event_id, &patch, &actor).await? {
            return Err(not_found(&event_id));
        }
        return answer(&state, &event_id).await;
    }
    state.user_writes()?;
    let signer = state.signer(&headers, method.as_str(), uri.path()).await?;
    let dates_present = date_key_present(&body);
    let edit: UpdateEventBody = serde_json::from_slice(&body)
        .map_err(|e| ApiError::bad_request(format!("invalid patch body: {e}")))?;
    let Some((mut raw, owner, _)) = load_raw(&state, &event_id, true).await? else {
        return Err(not_found(&event_id));
    };
    if is_soft_deleted(&raw) {
        return Err(not_found(&event_id));
    }
    if owner != signer {
        return Err(ApiError::forbidden(
            "You don't have permission to edit this event",
        ));
    }
    let was_approved = raw
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let featured_changed = featured_item_changed(&raw, &edit);
    apply_owner_edit(&mut raw, &edit, dates_present)?;
    if location_touched(&edit) {
        let world = raw.get("world").and_then(Value::as_bool).unwrap_or(false);
        let server = raw
            .get("server")
            .and_then(Value::as_str)
            .map(str::to_string);
        let x = raw.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
        let y = raw.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
        let (place_id, world) =
            moderation::resolve_location(&state.pool, world, server.as_deref(), x, y).await?;
        raw.insert(
            "place_id".into(),
            place_id.map(Value::String).unwrap_or(Value::Null),
        );
        raw.insert("world".into(), json!(world));
    }
    if was_approved && (content_touched(&edit) || featured_changed) {
        raw.insert("approved".into(), json!(false));
    }
    raw.insert("updated_at".into(), json!(Utc::now().to_rfc3339()));
    moderation::store(
        &state.pool,
        &event_id,
        raw,
        &signer,
        json!({ "edit": edit }),
    )
    .await?;
    answer(&state, &event_id).await
}

/// `DELETE /api/events/{id}`: a soft delete, by the admin or the owner.
pub async fn delete_event(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(event_id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<EventRecord>>, ApiError> {
    let admin = super::handlers::is_admin_request(&headers, &state);
    let signer = if admin {
        state.operator_writes()?;
        None
    } else {
        state.user_writes()?;
        Some(state.signer(&headers, method.as_str(), uri.path()).await?)
    };
    let Some((mut raw, owner, mirrored)) = load_raw(&state, &event_id, signer.is_some()).await?
    else {
        return Err(not_found(&event_id));
    };
    if is_soft_deleted(&raw) {
        return answer(&state, &event_id).await;
    }
    let now = Utc::now().to_rfc3339();
    let (provenance, payload) = match signer {
        None => {
            let body: DeleteEventBody = if body.is_empty() {
                DeleteEventBody::default()
            } else {
                serde_json::from_slice(&body)
                    .map_err(|e| ApiError::bad_request(format!("invalid delete body: {e}")))?
            };
            raw.insert("deleted_by_admin".into(), json!(true));
            raw.insert("deleted_by".into(), json!(body.actor));
            raw.insert("deleted_reason".into(), json!(body.reason));
            (
                body.actor.clone().unwrap_or_else(|| ADMIN_SIGNER.into()),
                json!({ "delete": body }),
            )
        }
        Some(signer) => {
            if owner != signer {
                return Err(ApiError::forbidden(
                    "You don't have permission to delete this event",
                ));
            }
            raw.insert("deleted_by_user".into(), json!(true));
            raw.insert("deleted_by".into(), json!(signer));
            (signer, json!({ "delete": {} }))
        }
    };
    raw.insert("deleted_at".into(), json!(now));
    raw.insert("updated_at".into(), json!(now));
    if mirrored {
        moderation::hide_mirrored(&state.pool, &event_id, raw, &provenance).await?;
    } else {
        moderation::store(&state.pool, &event_id, raw, &provenance, payload).await?;
    }
    answer(&state, &event_id).await
}

/// Visible to `signer`: approved and not rejected, or theirs; never a deleted one.
async fn exists_visible(
    state: &SharedState,
    event_id: &str,
    signer: &str,
) -> Result<bool, ApiError> {
    let row: Option<(bool, Option<String>, Value)> =
        sqlx::query_as("SELECT approved, user_creator, raw FROM explore.event WHERE id = $1")
            .bind(event_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((approved, creator, raw)) = row else {
        return Ok(false);
    };
    let map = raw.as_object().cloned().unwrap_or_default();
    if is_soft_deleted(&map) {
        return Ok(false);
    }
    let rejected = map
        .get("rejected")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let owner = map
        .get("user")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(creator)
        .map(|u| u.to_lowercase());
    Ok((approved && !rejected) || owner.as_deref() == Some(signer))
}

async fn rsvp(
    state: SharedState,
    method: Method,
    path: &str,
    headers: HeaderMap,
    event_id: String,
    going: bool,
) -> Result<Json<ApiData<Vec<EventAttendeeRecord>>>, ApiError> {
    state.user_writes()?;
    let signer = state.signer(&headers, method.as_str(), path).await?;
    if !exists_visible(&state, &event_id, &signer).await? {
        return Err(not_found(&event_id));
    }
    let list = if going {
        let user_name = headers
            .get(crate::shared::signed_fetch::AUTH_METADATA_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|m| m.get("name").and_then(Value::as_str).map(str::to_string));
        super::attendees::rsvp_going(&state.pool, &event_id, &signer, user_name.as_deref()).await?
    } else {
        super::attendees::rsvp_cancel(&state.pool, &event_id, &signer).await?
    };
    Ok(Json(ApiData::ok(list)))
}

/// `POST /api/events/{id}/attendees`: the signer is going.
pub async fn create_event_attendee(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(event_id): Path<String>,
) -> Result<Json<ApiData<Vec<EventAttendeeRecord>>>, ApiError> {
    rsvp(state, method, uri.path(), headers, event_id, true).await
}

/// `DELETE /api/events/{id}/attendees`: the signer is not.
pub async fn delete_event_attendee(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(event_id): Path<String>,
) -> Result<Json<ApiData<Vec<EventAttendeeRecord>>>, ApiError> {
    rsvp(state, method, uri.path(), headers, event_id, false).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Map<String, Value> {
        json!({ "name": "e", "x": 1, "y": 2, "duration": 3_600_000, "start_at": "2099-01-01T00:00:00+00:00", "approved": true }).as_object().unwrap().clone()
    }

    #[test]
    fn an_owner_edit_rewrites_the_fields_it_names_and_the_position() {
        let mut r = raw();
        let body: UpdateEventBody = serde_json::from_value(json!({ "name": "renamed", "x": 10, "categories": ["art"], "schedules": ["a", "a", "b"] })).unwrap();
        apply_owner_edit(&mut r, &body, false).unwrap();
        assert_eq!(
            (
                r["name"].clone(),
                r["position"].clone(),
                r["schedules"].clone()
            ),
            (json!("renamed"), json!([10, 2]), json!(["a", "b"]))
        );
        assert!(content_touched(&body) && location_touched(&body));
    }

    #[test]
    fn an_owner_edit_is_validated() {
        let too_long: UpdateEventBody =
            serde_json::from_value(json!({ "name": "x".repeat(151) })).unwrap();
        assert!(apply_owner_edit(&mut raw(), &too_long, false).is_err());
        let off_map: UpdateEventBody = serde_json::from_value(json!({ "y": 171 })).unwrap();
        assert!(apply_owner_edit(&mut raw(), &off_map, false).is_err());
        let past: UpdateEventBody =
            serde_json::from_value(json!({ "start_at": "2000-01-01T00:00:00Z" })).unwrap();
        assert_eq!(
            apply_owner_edit(&mut raw(), &past, true)
                .unwrap_err()
                .to_string(),
            PAST_FINISH_AT_MESSAGE
        );
        let longer: UpdateEventBody =
            serde_json::from_value(json!({ "duration": MAX_EVENT_DURATION_MS + 1 })).unwrap();
        assert!(apply_owner_edit(&mut raw(), &longer, false).is_err());
        assert!(
            serde_json::from_value::<UpdateEventBody>(json!({ "nope": 1 })).is_err(),
            "unknown fields are refused"
        );
    }

    #[test]
    fn a_rescheduled_event_recomputes_its_occurrence() {
        let mut r = raw();
        let body: UpdateEventBody = serde_json::from_value(
            json!({ "start_at": "2099-02-01T10:00:00Z", "duration": 1_800_000 }),
        )
        .unwrap();
        apply_owner_edit(&mut r, &body, true).unwrap();
        assert_eq!(r["next_start_at"], json!("2099-02-01T10:00:00+00:00"));
        assert_eq!(r["finish_at"], json!("2099-02-01T10:30:00+00:00"));
        assert_eq!(r["recurrent_dates"], json!(["2099-02-01T10:00:00+00:00"]));
        assert!(
            date_key_present(br#"{"recurrent_count": 2}"#)
                && !date_key_present(br#"{"name": "x"}"#)
        );
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
        auth: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        match auth {
            Some("signed") => {
                let signed = path.split('?').next().unwrap();
                for (k, v) in
                    signed_headers(&method.to_lowercase(), signed, r#"{"name":"Alice"}"#, true)
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
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())),
        )
    }

    fn new_event() -> Value {
        json!({ "name": "Launch party", "start_at": "2099-06-01T18:00:00Z", "duration": 3_600_000, "x": 10, "y": -20, "description": "<b>come</b>" })
    }

    /// The events write surface over the real schema: a wallet creates,
    /// edits and deletes its own event and RSVPs; another wallet may not;
    /// the admin bearer moderates; the switches answer 503; the sitemaps and
    /// the v1 reads see the result.
    #[tokio::test]
    async fn events_are_written_by_their_owner_and_moderated_by_the_admin() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let settings = Settings {
            user_writes_enabled: true,
            operator_writes_enabled: true,
            events_admin_token: Some("events-admin".into()),
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
                user_writes_enabled: false,
                ..settings
            },
            None,
        )));

        let (status, _) = call(
            &closed,
            "POST",
            "/api/events",
            Some("signed"),
            Some(new_event()),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let (status, _) = call(&app, "POST", "/api/events", None, Some(new_event())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, body) = call(
            &app,
            "POST",
            "/api/events",
            Some("signed"),
            Some(json!({ "name": "x", "nope": 1 })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, body) = call(
            &app,
            "POST",
            "/api/events",
            Some("signed"),
            Some(new_event()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let id = body["data"]["id"].as_str().unwrap().to_string();
        assert_eq!(
            (
                body["data"]["user"].clone(),
                body["data"]["approved"].clone()
            ),
            (json!(address()), json!(false))
        );

        // the admin approves, the owner's content edit sends it back to approval
        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/events/{id}"),
            Some("events-admin"),
            Some(json!({ "action": "approve", "actor": "0xmod" })),
        )
        .await;
        assert_eq!(
            (status, body["data"]["approved"].clone()),
            (StatusCode::OK, json!(true)),
            "{body}"
        );
        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/events/{id}"),
            Some("signed"),
            Some(json!({ "name": "Launch party II" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            (
                body["data"]["name"].clone(),
                body["data"]["approved"].clone()
            ),
            (json!("Launch party II"), json!(false))
        );
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/api/events/{id}"),
            Some("events-admin"),
            Some(json!({ "approved": true })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // another owner's event is not editable; an unknown one is a 404
        sqlx::query("UPDATE explore.event SET raw = raw || '{\"user\": \"0x0000000000000000000000000000000000000abc\"}', user_creator = '0x0000000000000000000000000000000000000abc' WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        let (status, body) = call(
            &app,
            "PATCH",
            &format!("/api/events/{id}"),
            Some("signed"),
            Some(json!({ "name": "hijack" })),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::FORBIDDEN,
                json!("You don't have permission to edit this event")
            )
        );
        sqlx::query("UPDATE explore.event SET raw = raw || jsonb_build_object('user', $2::text), user_creator = $2 WHERE id = $1").bind(&id).bind(address()).execute(&pool).await.unwrap();
        let (status, _) = call(
            &app,
            "PATCH",
            "/api/events/00000000-0000-0000-0000-000000000000",
            Some("signed"),
            Some(json!({ "name": "x" })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, body) = call(
            &app,
            "POST",
            &format!("/api/events/{id}/attendees"),
            Some("signed"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            (
                body["data"][0]["user"].clone(),
                body["data"][0]["user_name"].clone()
            ),
            (json!(address()), json!("Alice"))
        );
        let (_, body) = call(
            &app,
            "DELETE",
            &format!("/api/events/{id}/attendees"),
            Some("signed"),
            None,
        )
        .await;
        assert_eq!(body["data"], json!([]));

        let (status, body) = call(&app, "GET", &format!("/v1/events/{id}"), None, None).await;
        assert_eq!(
            (status, body["data"]["id"].clone()),
            (StatusCode::OK, json!(id))
        );
        let (status, body) = call(&app, "GET", "/v1/events/not-a-uuid", None, None).await;
        assert_eq!(
            (status, body["message"].clone()),
            (StatusCode::NOT_FOUND, json!("Event not found: not-a-uuid"))
        );
        let (status, body) = call(&app, "GET", "/events/sitemap.xml", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.as_str().unwrap().contains("sitemap.events.xml?page=0"),
            "{body}"
        );
        let (_, body) = call(&app, "GET", "/events/sitemap.events.xml?page=0", None, None).await;
        assert!(
            body.as_str().unwrap().contains(&format!("/event/?id={id}")),
            "{body}"
        );
        let (_, body) = call(
            &app,
            "GET",
            "/events/sitemap.events.xml?page=00",
            None,
            None,
        )
        .await;
        assert!(
            !body.as_str().unwrap().contains("<url>"),
            "a non-canonical page number answers an empty set"
        );

        // the owner deletes; a delete again answers the deleted record
        let (status, body) = call(
            &app,
            "DELETE",
            &format!("/api/events/{id}"),
            Some("signed"),
            None,
        )
        .await;
        assert_eq!(
            (status, body["data"]["deleted_by_user"].clone()),
            (StatusCode::OK, json!(true)),
            "{body}"
        );
        let (status, _) = call(
            &app,
            "DELETE",
            &format!("/api/events/{id}"),
            Some("signed"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = call(
            &app,
            "POST",
            &format!("/api/events/{id}/attendees"),
            Some("signed"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "no RSVP to a deleted event");
        let overlay: String =
            sqlx::query_scalar("SELECT signer FROM explore.events_local WHERE id = $1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(overlay, address());
        let (status, body) = call(&app, "GET", "/api/profiles/subscriptions", None, None).await;
        assert_eq!(status, StatusCode::GONE, "{body}");
    }

    /// A mirrored event is the upstream's: its owner's edit and delete (and
    /// an admin patch of its name) answer 409 and write nothing, so the next
    /// pass still writes it. The admin's moderation and delete are local
    /// policy the pass keeps: a rejected or deleted event leaves the public
    /// list and detail while the admin still reads it. An RSVP is a local row
    /// the count adds to the upstream's.
    #[tokio::test]
    async fn a_mirrored_event_refuses_its_owner_keeps_the_admins_moderation_and_counts_local_rsvps()
    {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let settings = Settings {
            user_writes_enabled: true,
            operator_writes_enabled: true,
            events_admin_token: Some("events-admin".into()),
            ..Settings::for_tests()
        };
        let app = router(Arc::new(ExploreState::new(pool.clone(), settings, None)));
        let id = uuid::Uuid::new_v4().to_string();
        let community = uuid::Uuid::new_v4().to_string();
        let upstream = |name: &str| {
            json!({ "id": id, "name": name, "user": address(), "approved": true,
                    "community_id": community,
                    "start_at": "2099-06-01T18:00:00Z", "finish_at": "2099-06-01T19:00:00Z",
                    "duration": 3_600_000, "total_attendees": 7,
                    "latest_attendees": ["0x00000000000000000000000000000000000000a7"] })
        };
        let pass = |name: &'static str| {
            let pool = pool.clone();
            let doc = upstream(name);
            async move {
                assert!(
                    super::super::mirror::upsert_event(&pool, &doc)
                        .await
                        .unwrap()
                );
            }
        };
        pass("Upstream party").await;

        let path = format!("/api/events/{id}");
        let listed = || {
            let app = app.clone();
            let community = community.clone();
            async move {
                let (_, body) = call(
                    &app,
                    "GET",
                    &format!("/api/events?list=all&community_id={community}"),
                    None,
                    None,
                )
                .await;
                let shown = body["data"]["events"].as_array().map_or(0, Vec::len);
                assert_eq!(body["data"]["total"], json!(shown), "{body}");
                shown
            }
        };
        assert_eq!(listed().await, 1);
        let edit = json!({ "name": "renamed here" });
        let refusals = [
            call(&app, "PATCH", &path, Some("signed"), Some(edit.clone())).await,
            call(&app, "DELETE", &path, Some("signed"), None).await,
            call(
                &app,
                "PATCH",
                &path,
                Some("events-admin"),
                Some(edit.clone()),
            )
            .await,
        ];
        for (status, body) in refusals {
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body["code"], json!("conflict"));
            assert!(
                body["message"].as_str().unwrap().contains("mirrored from"),
                "{body}"
            );
        }
        let (name, raw, audits, overlays): (String, Value, i64, i64) = sqlx::query_as(
            "SELECT name, raw, (SELECT count(*) FROM explore.events_local WHERE id = $1), \
                    (SELECT count(*) FROM explore.event_moderation WHERE event_id = $1) \
             FROM explore.event WHERE id = $1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (
                name.as_str(),
                raw.get("deleted_by_user"),
                raw.get("rejected"),
                audits,
                overlays
            ),
            ("Upstream party", None, None, 0, 0)
        );

        // the admin rejects it: gone from the public reads, kept by a pass
        let (status, body) = call(
            &app,
            "PATCH",
            &path,
            Some("events-admin"),
            Some(json!({ "action": "reject" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        pass("Renamed upstream").await;
        assert_eq!(listed().await, 0, "a rejected event is not listed");
        let (status, _) = call(&app, "GET", &path, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, body) = call(&app, "GET", &path, Some("events-admin"), None).await;
        assert_eq!(
            (
                status,
                body["data"]["name"].clone(),
                body["data"]["rejected"].clone()
            ),
            (StatusCode::OK, json!("Renamed upstream"), json!(true)),
            "{body}"
        );
        let (status, body) = call(
            &app,
            "PATCH",
            &path,
            Some("events-admin"),
            Some(json!({ "action": "approve" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(listed().await, 1);

        let (status, body) = call(
            &app,
            "POST",
            &format!("{path}/attendees"),
            Some("signed"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (_, body) = call(&app, "GET", &path, None, None).await;
        assert_eq!(
            body["data"]["total_attendees"],
            json!(8),
            "the upstream's 7 and this node's 1: {body}"
        );
        pass("Renamed again").await;
        let (_, body) = call(&app, "GET", &path, None, None).await;
        assert_eq!(
            (
                body["data"]["name"].clone(),
                body["data"]["total_attendees"].clone()
            ),
            (json!("Renamed again"), json!(8)),
            "neither the RSVP nor the moderation freezes the upstream's fields"
        );

        // the admin deletes it: hidden from every public read across passes
        let operator = "0x00000000000000000000000000000000000000ad";
        let (status, body) = call(
            &app,
            "DELETE",
            &path,
            Some("events-admin"),
            Some(json!({ "reason": "duplicate", "actor": operator })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        pass("Renamed once more").await;
        assert_eq!(listed().await, 0, "a hidden event is not listed");
        let (status, _) = call(&app, "GET", &path, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, body) = call(&app, "GET", &path, Some("events-admin"), None).await;
        assert_eq!(
            (
                status,
                body["data"]["name"].clone(),
                body["data"]["deleted_by_admin"].clone(),
                body["data"]["deleted_by"].clone(),
                body["data"]["deleted_reason"].clone()
            ),
            (
                StatusCode::OK,
                json!("Renamed once more"),
                json!(true),
                json!(operator),
                json!("duplicate")
            ),
            "the console still reads it, with the delete's actor and reason: {body}"
        );
        let (hidden, approved, audits): (Option<bool>, Option<bool>, i64) = sqlx::query_as(
            "SELECT hidden, approved, (SELECT count(*) FROM explore.events_local WHERE id = $1) \
             FROM explore.event_moderation WHERE event_id = $1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((hidden, approved, audits), (Some(true), Some(true), 0));

        sqlx::query("DELETE FROM explore.event_attendance_local WHERE event_id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event_moderation WHERE event_id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }

    #[test]
    fn the_featured_item_change_is_detected() {
        let mut r = raw();
        r.insert(
            "featured_item".into(),
            json!(
                "urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000001:1"
            ),
        );
        let same: UpdateEventBody = serde_json::from_value(json!({ "featured_item": "urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000001:1" })).unwrap();
        let cleared: UpdateEventBody =
            serde_json::from_value(json!({ "featured_item": null })).unwrap();
        assert!(!featured_item_changed(&r, &same));
        assert!(featured_item_changed(&r, &cleared));
        assert!(!featured_item_changed(&r, &UpdateEventBody::default()));
    }
}
