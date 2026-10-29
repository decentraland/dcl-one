use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use serde::Deserialize;
use uuid::Uuid;

use crate::SharedState;
use crate::http::{ApiData, ApiError};
use crate::linked_publication::LinkedPublicationCheque;
use crate::linked_publication_review::{FoundationLinkedReview, FoundationReviewSignatures};
use crate::linked_publication_store::{LinkedPublicationPreparation, LinkedPublicationState};

use super::drafts::signer;

#[derive(Deserialize)]
pub struct Revision {
    revision: String,
}

pub async fn prepare(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<LinkedPublicationPreparation>>, ApiError> {
    let owner = signer(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state
            .items
            .prepare_linked_publication(owner.as_str(), id)
            .await?,
    )))
}

pub async fn status(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<Option<LinkedPublicationState>>>, ApiError> {
    let owner = signer(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state
            .items
            .linked_publication_state(owner.as_str(), id)
            .await?,
    )))
}

pub async fn begin(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Revision>,
) -> Result<Json<ApiData<LinkedPublicationState>>, ApiError> {
    let owner = signer(&state, &headers, "post", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state
            .items
            .begin_linked_publication(owner.as_str(), id, &body.revision)
            .await?,
    )))
}

pub async fn claim(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Revision>,
) -> Result<Json<ApiData<LinkedPublicationState>>, ApiError> {
    let owner = signer(&state, &headers, "patch", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state
            .items
            .claim_linked_publication(owner.as_str(), id, &body.revision)
            .await?,
    )))
}

pub async fn authorize(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<LinkedPublicationCheque>,
) -> Result<Json<ApiData<LinkedPublicationState>>, ApiError> {
    let owner = signer(&state, &headers, "put", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state
            .items
            .authorize_linked_publication(owner.as_str(), id, &body)
            .await?,
    )))
}

pub async fn cancel(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<bool>>, ApiError> {
    let owner = signer(&state, &headers, "delete", &original, &uri).await?;
    state
        .items
        .cancel_linked_publication(owner.as_str(), id)
        .await?;
    Ok(Json(ApiData::ok(true)))
}

pub async fn verify(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<FoundationReviewSignatures>,
) -> Result<Json<ApiData<LinkedPublicationState>>, ApiError> {
    let Some(verifier) = FoundationLinkedReview::new(
        state.foundation.clone(),
        state.settings.foundation_url.as_deref(),
    ) else {
        return Err(ApiError::service_unavailable(
            "Foundation review is off on this node: BUILDER_FOUNDATION_URL names no Foundation",
        ));
    };
    let owner = signer(&state, &headers, "post", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        verifier
            .verify(&state.items, owner.as_str(), id, &body)
            .await?,
    )))
}
