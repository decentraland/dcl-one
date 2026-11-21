use axum::Json;
use axum::extract::{Path, State};
use bytes::Bytes;
use serde_json::{Value, json};

use crate::SharedState;
use crate::access;
use crate::auth::Signed;
use crate::collections::approval;
use crate::collections::lifecycle;
use crate::collections::publish_third_party;
use crate::curation::forum_posts;
use crate::curation::slack::Notice;
use crate::http::{ApiData, ApiError, iso};
use crate::terms;

/// `POST /v1/collections/:id/lock` (`lockCollection`): the lock's date.
pub async fn lock(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<String>>, ApiError> {
    let lock = access::lock_collection(&state, &signer, &id).await?;
    let at = lifecycle::lock(&state, &lock).await?;
    Ok(Json(ApiData::ok(iso(&at))))
}

/// `DELETE /v1/collections/:id` (`deleteCollection`): `data` is `true`.
pub async fn delete(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<bool>>, ApiError> {
    let manage = access::manage_collection(&state, &signer, &id).await?;
    lifecycle::delete(&state, &manage).await?;
    Ok(Json(ApiData::ok(true)))
}

/// `POST /v1/collections/:id/owner/sync` (`syncCollectionOwner`): the
/// collection's full body, whose owner the node reads from the chain; it
/// stores nothing.
pub async fn owner_sync(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let sync = access::sync_collection(&state, &signer, &id).await?;
    if sync.third_party() {
        return Err(
            ApiError::bad_request("Third party collections have no on-chain owner")
                .with_data(json!({ "id": sync.id() })),
        );
    }
    Ok(Json(ApiData::ok(
        state.items.collection(&sync.full_read()).await?,
    )))
}

/// `POST /v1/collections/:id/tos` (`saveTOS`): who may, then the body,
/// then the acceptance stored and forwarded; answers `{ok: true}` without
/// data.
pub async fn terms(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let manage = access::manage_collection(&state, &signer, &id).await?;
    let body: Value =
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("Invalid request body"))?;
    terms::accept(&state, &manage, terms::parse(&body)?).await?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /v1/collections/:id/publish` (`publishCollection`): a standard
/// collection answers `{collection, items}` once the chain holds it; a
/// third-party one submits the items its body names with their slot cheque
/// and answers `{collection, items, itemCurations}`.
pub async fn publish(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let sync = access::sync_collection(&state, &signer, &id).await?;
    let published = match access::publish_third_party(sync) {
        Ok(grant) => {
            let request = publish_third_party::parse(&body)?;
            let published = publish_third_party::publish(&state, &grant, &request).await?;
            let name = published.collection["name"].as_str().unwrap_or_default();
            state.notices.send(
                &state.guarded,
                grant.id(),
                name,
                grant.signer(),
                Notice::ThirdPartyPublished {
                    items: published.items.len(),
                },
            );
            let items: Vec<uuid::Uuid> = published
                .item_curations
                .iter()
                .map(|curation| curation.item_id)
                .collect();
            forum_posts::publish_topic(&state, &grant, &items).await;
            serde_json::to_value(published)
        }
        Err(sync) => serde_json::to_value(lifecycle::publish(&state, &sync).await?),
    };
    Ok(Json(ApiData::ok(published.map_err(|e| {
        ApiError::internal(format!("publish answer: {e}"))
    })?)))
}

/// `GET /v1/collections/:id/approvalData` (`getApprovalData`): what the
/// committee's registry transaction needs to approve a third-party
/// collection's submitted items.
pub async fn approval_data(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<approval::ApprovalData>>, ApiError> {
    let read = access::read_approval(&state, &signer, &id).await?;
    Ok(Json(ApiData::ok(
        approval::approval_data(&state, &read).await?,
    )))
}
