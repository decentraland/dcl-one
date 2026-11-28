//! builder-server's review-flow routes: `POST /v1/collections/:id/validations`,
//! `POST .../curation/appeal`, `GET .../events` and `POST .../comments`.
//!
//! Each answers only with the review flow on ([`review::enabled`]): after
//! the signature and the route's own budget per signer
//! ([`crate::limits::spend_review`], whatever the flag, as builder-server),
//! the flag is read and, off, the route answers 404 `Not
//! found` as an absent route does and touches nothing else. Then
//! `builder::access` decides, then the body is read.

use axum::Json;
use axum::extract::{Path, RawQuery, State};
use bytes::Bytes;
use serde::Deserialize;
use uuid::Uuid;

use crate::SharedState;
use crate::access;
use crate::auth::Signed;
use crate::curation::events::EventOut;
use crate::curation::rows::CollectionCurationOut;
use crate::http::{ApiData, ApiError};
use crate::review::{self, EventsPage, MAX_TEXT};

async fn flow_on(state: &SharedState) -> Result<(), ApiError> {
    let mut conn = state.items.pool().acquire().await?;
    if review::enabled(&mut conn, state).await? {
        Ok(())
    } else {
        Err(ApiError::not_found("Not found"))
    }
}

fn invalid_body() -> ApiError {
    ApiError::bad_request("Invalid request body")
}

fn body_of<T: for<'de> Deserialize<'de>>(body: &Bytes) -> Result<T, ApiError> {
    let body: &[u8] = if body.is_empty() { b"{}" } else { body };
    serde_json::from_slice(body).map_err(|_| invalid_body())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppealBody {
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CommentBody {
    message: String,
    item_id: Option<Uuid>,
}

/// `POST /v1/collections/:id/validations`: another automated review.
pub async fn validations(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<EventOut>>, ApiError> {
    crate::limits::spend_review(&state, signer.as_str(), false)?;
    flow_on(&state).await?;
    let creator = access::review_creator(&state, &signer, &id).await?;
    Ok(Json(ApiData::ok(review::retry(&state, creator).await?)))
}

/// `POST /v1/collections/:id/curation/appeal` (`{note?}`): a rejection
/// sent to a curator.
pub async fn appeal(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<CollectionCurationOut>>, ApiError> {
    crate::limits::spend_review(&state, signer.as_str(), false)?;
    flow_on(&state).await?;
    let creator = access::review_creator(&state, &signer, &id).await?;
    let AppealBody { note } = body_of(&body)?;
    let note = note.unwrap_or_default();
    if note.chars().count() > MAX_TEXT {
        return Err(invalid_body());
    }
    Ok(Json(ApiData::ok(
        review::appeal(&state, creator, note.trim().to_string()).await?,
    )))
}

/// `GET /v1/collections/:id/events?page&limit`: the review timeline.
pub async fn events(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Json<ApiData<EventsPage>>, ApiError> {
    crate::limits::spend_review(&state, signer.as_str(), true)?;
    flow_on(&state).await?;
    let read = access::read_review(&state, &signer, &id).await?;
    let (page, limit) = review::pagination(query.as_deref());
    Ok(Json(ApiData::ok(
        review::timeline(&state, &read, page, limit).await?,
    )))
}

/// `POST /v1/collections/:id/comments` (`{message, itemId?}`).
pub async fn comments(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<EventOut>>, ApiError> {
    crate::limits::spend_review(&state, signer.as_str(), false)?;
    flow_on(&state).await?;
    let member = access::review_member(&state, &signer, &id).await?;
    let CommentBody { message, item_id } = body_of(&body)?;
    let message = message.trim().to_string();
    let length = message.chars().count();
    if length == 0 || length > MAX_TEXT {
        return Err(ApiError::bad_request(format!(
            "The message must have between 1 and {MAX_TEXT} characters"
        ))
        .with_data(serde_json::json!({ "id": member.collection() })));
    }
    Ok(Json(ApiData::ok(
        review::comment(&state, member, message, item_id).await?,
    )))
}
