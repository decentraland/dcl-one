use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::SharedState;
use crate::auth::{authorize_admin, signed_path};
use crate::http::{ApiData, ApiError};
use crate::onchain::{self, CommitteeMemberOut, ReviewRowOut};

/// `data` payload of `GET /v1/collections/curation`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CurationCollectionsOut {
    pub collections: Vec<ReviewRowOut>,
    pub committee: Vec<CommitteeMemberOut>,
}

/// `data` payload of `PATCH /v1/collections/{id}/items/{item}/status`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ItemStatusPatchOut {
    pub collection_id: String,
    pub id: String,
    pub status: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated: u64,
}

/// `data` payload of `PATCH /v1/collections/{id}/items/status`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct BulkItemStatusPatchOut {
    pub collection_id: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub requested: u64,
    pub status: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated: u64,
}

const CURATION_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];
const MAX_BULK_ITEMS: usize = 1000;

async fn admin(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    original: &Uri,
    local: &Uri,
) -> Result<(), ApiError> {
    authorize_admin(
        state.settings.admin_token.as_deref(),
        &state.settings.admin_addresses,
        state.eip1654.as_deref(),
        headers,
        method,
        &signed_path(original, local),
    )
    .await
}

fn validate_status(status: &str) -> Result<(), ApiError> {
    if CURATION_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(ApiError::bad_request("Invalid Status provided"))
    }
}

fn parse_uuid(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw.trim()).map_err(|_| ApiError::not_found("Not found"))
}

/// The curation queue: the committee (memoised five minutes) and the
/// collections under review, both from the collections subgraphs; empty
/// lists without a reader, as the old node answered without its side pool.
pub async fn get_curation_collections(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<CurationCollectionsOut>>, ApiError> {
    admin(&state, &headers, "get", &original, &uri).await?;

    let (committee, collections) = match state
        .market
        .as_deref()
        .filter(|m| onchain::configured(Some(m)))
    {
        Some(market) => {
            let committee = state.committee_memo.get_or_fetch((), || async {
                onchain::committee_members(market)
                    .await
                    .map(std::sync::Arc::new)
            });
            let (committee, collections) =
                tokio::try_join!(committee, onchain::collections_under_review(market))?;
            (committee.as_ref().clone(), collections)
        }
        None => (Vec::new(), Vec::new()),
    };

    Ok(Json(ApiData::ok(CurationCollectionsOut {
        collections,
        committee,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ItemStatusBody {
    pub status: String,
}

pub async fn patch_item_status(
    State(state): State<SharedState>,
    Path((id, item)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<ItemStatusBody>,
) -> Result<Json<ApiData<ItemStatusPatchOut>>, ApiError> {
    admin(&state, &headers, "patch", &original, &uri).await?;
    validate_status(&body.status)?;

    let collection_id = parse_uuid(&id)?;
    let item_id = parse_uuid(&item)?;

    let updated = state
        .items
        .set_item_curation_status(&collection_id, &item_id, &body.status)
        .await?;

    if updated == 0 {
        return Err(ApiError::not_found("Not found"));
    }

    Ok(Json(ApiData::ok(ItemStatusPatchOut {
        collection_id: id,
        id: item,
        status: body.status,
        updated,
    })))
}

#[derive(Debug, Deserialize)]
pub struct BulkItemStatusBody {
    pub status: String,
    #[serde(rename = "itemIds", alias = "item_ids")]
    pub item_ids: Vec<String>,
}

pub async fn patch_items_status_bulk(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<BulkItemStatusBody>,
) -> Result<Json<ApiData<BulkItemStatusPatchOut>>, ApiError> {
    admin(&state, &headers, "patch", &original, &uri).await?;
    validate_status(&body.status)?;

    if body.item_ids.is_empty() {
        return Err(ApiError::bad_request("itemIds must not be empty"));
    }
    if body.item_ids.len() > MAX_BULK_ITEMS {
        return Err(ApiError::bad_request("Too many items in a single request"));
    }

    let collection_id = parse_uuid(&id)?;
    let mut item_ids = Vec::with_capacity(body.item_ids.len());
    for raw in &body.item_ids {
        item_ids.push(parse_uuid(raw)?);
    }

    let updated = state
        .items
        .set_items_curation_status(&collection_id, &item_ids, &body.status)
        .await?;

    Ok(Json(ApiData::ok(BulkItemStatusPatchOut {
        collection_id: id,
        requested: body.item_ids.len() as u64,
        status: body.status,
        updated,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_status_accepts_known_and_rejects_unknown() {
        assert!(validate_status("approved").is_ok());
        assert!(validate_status("rejected").is_ok());
        assert!(validate_status("pending").is_ok());
        assert!(validate_status("bogus").is_err());
    }

    /// Compared as parsed JSON: object key order is not part of the contract and flips
    /// with serde_json's preserve_order feature.
    #[test]
    fn curation_wire_bytes_match_the_old_json_macro() {
        use serde_json::json;

        let envelope = ApiData::ok(CurationCollectionsOut {
            collections: Vec::new(),
            committee: Vec::new(),
        });
        let old = json!({
            "committee": [],
            "collections": [],
        });
        assert_eq!(
            serde_json::to_value(&envelope).unwrap(),
            json!({ "ok": true, "data": old }),
        );

        let item = ItemStatusPatchOut {
            collection_id: "col-1".into(),
            id: "item-1".into(),
            status: "approved".into(),
            updated: 1,
        };
        let old_item = json!({
            "id": "item-1",
            "collection_id": "col-1",
            "status": "approved",
            "updated": 1u64,
        });
        assert_eq!(serde_json::to_value(&item).unwrap(), old_item);

        let bulk = BulkItemStatusPatchOut {
            collection_id: "col-1".into(),
            requested: 2,
            status: "rejected".into(),
            updated: 2,
        };
        let old_bulk = json!({
            "collection_id": "col-1",
            "status": "rejected",
            "requested": 2u64,
            "updated": 2u64,
        });
        assert_eq!(serde_json::to_value(&bulk).unwrap(), old_bulk);
    }
}
