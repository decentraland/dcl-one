use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::SharedState;
use crate::auth::{require_signer, signed_path};
use crate::drafts::{CollectionDraft, CollectionListingOut, ItemDraft};
use crate::http::{ApiData, ApiError};
use crate::items::CollectionMetaOut;
use crate::publication::PublicationPreparation;
use crate::shared::signed_fetch::Signer;

/// The route's verified signer: the request path first (what a client behind
/// `/builder-api` signed), the route-local path second.
pub(super) async fn signer(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    original: &Uri,
    local: &Uri,
) -> Result<Signer, ApiError> {
    require_signer(
        headers,
        method,
        &signed_path(original, local),
        state.eip1654.as_deref(),
    )
    .await
}

#[derive(Deserialize)]
pub struct CollectionBody {
    collection: CollectionDraft,
}
#[derive(Deserialize)]
pub struct ItemBody {
    item: ItemDraft,
}

/// Local drafts use the verified signer; upstream global lists belong to the committee.
pub async fn get_collection_drafts(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<Vec<CollectionListingOut>>>, ApiError> {
    let owner = signer(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state.items.collection_drafts(owner.as_str()).await?,
    )))
}

pub async fn get_item_drafts(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<Vec<Value>>>, ApiError> {
    let owner = signer(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state.items.item_drafts(owner.as_str()).await?,
    )))
}

pub async fn put_collection(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<CollectionBody>,
) -> Result<Json<ApiData<CollectionMetaOut>>, ApiError> {
    let owner = signer(&state, &headers, "put", &original, &uri).await?;
    body.collection.validate(id, owner.as_str())?;
    Ok(Json(ApiData::ok(
        state
            .items
            .save_collection_draft(owner.as_str(), &body.collection)
            .await?,
    )))
}

pub async fn put_item(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<ItemBody>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let owner = signer(&state, &headers, "put", &original, &uri).await?;
    body.item.validate(id, owner.as_str())?;
    Ok(Json(ApiData::ok(
        state
            .items
            .save_item_draft(owner.as_str(), &body.item)
            .await?,
    )))
}

pub async fn delete_item(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let owner = signer(&state, &headers, "delete", &original, &uri).await?;
    state.items.delete_item_draft(owner.as_str(), id).await?;
    Ok(Json(ApiData::ok(serde_json::json!({ "id": id }))))
}

pub async fn get_item(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let owner = signer(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state.items.item_draft(owner.as_str(), id).await?,
    )))
}

pub async fn post_item_files(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<ApiData<BTreeMap<String, String>>>, ApiError> {
    let owner = signer(&state, &headers, "post", &original, &uri).await?;
    state.items.item_draft(owner.as_str(), id).await?;
    let mut files = Vec::new();
    let mut size = 0;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::bad_request("Invalid multipart upload"))?
    {
        let name = field
            .file_name()
            .ok_or_else(|| ApiError::bad_request("Filename is required"))?
            .to_owned();
        if !crate::drafts::valid_filename(&name) || files.len() >= 100 {
            return Err(ApiError::bad_request("Invalid filename or too many files"));
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| ApiError::bad_request("File upload exceeds 20 MB or is incomplete"))?;
        size += bytes.len();
        if size > 20 * 1024 * 1024 {
            return Err(ApiError::bad_request("Upload exceeds 20 MB"));
        }
        files.push((name, bytes.to_vec()));
    }
    Ok(Json(ApiData::ok(
        state
            .items
            .save_item_files(owner.as_str(), id, files)
            .await?,
    )))
}

pub async fn get_publication(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<PublicationPreparation>>, ApiError> {
    let owner = signer(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        state.items.prepare_publication(owner.as_str(), id).await?,
    )))
}
