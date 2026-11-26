use axum::Json;
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::SharedState;
use crate::access::{self, CurationScope};
use crate::auth::{Admin, Signed};
use crate::chain_state;
use crate::curation::rows::{
    self, CollectionCurationOpened, CollectionCurationOut, ItemCurationOpened, ItemCurationOut,
};
use crate::curation::transitions::Status;
use crate::curation::writes;
use crate::http::{ApiData, ApiError};
use crate::kept::Since;
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

pub(crate) const MAX_BULK_ITEMS: usize = 1000;

fn parse_status(status: &str) -> Result<Status, ApiError> {
    Status::parse(status).ok_or_else(|| ApiError::bad_request("Invalid Status provided"))
}

fn parse_uuid(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw.trim()).map_err(|_| ApiError::not_found("Not found"))
}

/// The curation queue: the committee's current members (503 when it can't
/// tell) and the collections under review from the collections subgraphs
/// (empty without a reader).
pub async fn get_curation_collections(
    State(state): State<SharedState>,
    _: Admin,
) -> Result<Json<ApiData<CurationCollectionsOut>>, ApiError> {
    let committee = state
        .committee
        .members(&state, Since::read())
        .await?
        .iter()
        .map(CommitteeMemberOut::from)
        .collect();
    let collections = match state
        .market
        .as_ref()
        .filter(|m| onchain::configured(Some(m)))
    {
        Some(market) => onchain::collections_under_review(market).await?,
        None => Vec::new(),
    };

    Ok(Json(ApiData::ok(CurationCollectionsOut {
        collections,
        committee,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemStatusBody {
    pub status: String,
}

pub async fn patch_item_status(
    State(state): State<SharedState>,
    admin: Admin,
    Path((id, item)): Path<(String, String)>,
    Json(body): Json<ItemStatusBody>,
) -> Result<Json<ApiData<ItemStatusPatchOut>>, ApiError> {
    let status = parse_status(&body.status)?;
    let collection_id = parse_uuid(&id)?;
    let item_id = parse_uuid(&item)?;
    let operator = access::operate(&state, &admin, collection_id).await?;
    let updated = writes::operate_items(&state, operator, &[item_id], status).await?;

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
#[serde(deny_unknown_fields)]
pub struct BulkItemStatusBody {
    pub status: String,
    #[serde(rename = "itemIds", alias = "item_ids")]
    pub item_ids: Vec<String>,
}

pub async fn patch_items_status_bulk(
    State(state): State<SharedState>,
    admin: Admin,
    Path(id): Path<String>,
    Json(body): Json<BulkItemStatusBody>,
) -> Result<Json<ApiData<BulkItemStatusPatchOut>>, ApiError> {
    let status = parse_status(&body.status)?;
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

    let operator = access::operate(&state, &admin, collection_id).await?;
    let updated = writes::operate_items(&state, operator, &item_ids, status).await?;

    Ok(Json(ApiData::ok(BulkItemStatusPatchOut {
        collection_id: id,
        requested: body.item_ids.len() as u64,
        status: body.status,
        updated,
    })))
}

/// `GET /v1/curations` (`getCollectionCurations`): the committee reads the
/// newest curation of every collection; anyone else the newest of the
/// collections it creates, manages or mints and of its providers'.
pub async fn curations(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    headers: HeaderMap,
) -> Result<Json<ApiData<Vec<CollectionCurationOut>>>, ApiError> {
    let scope = access::curation_scope(&state, &signer).await?;
    let mut conn = state.items.pool().acquire().await?;
    let rows = match scope {
        CurationScope::Every(_) => rows::newest_of_every_collection(&mut conn).await?,
        CurationScope::Own(wallet) => {
            chain_state::for_read(&mut conn).await?;
            rows::newest_of_wallet(&mut conn, wallet.address(), &wallet.provider_ids()).await?
        }
    };
    drop(conn);
    crate::limits::charge_rows(&state, &headers, Some(signer.as_str()), rows.len());
    Ok(Json(ApiData::ok(rows)))
}

/// `{ok: true}` with the row as `data`, or without `data` when there is none.
fn maybe<T: serde::Serialize>(row: Option<T>) -> Json<Value> {
    Json(match row {
        Some(row) => json!({ "ok": true, "data": row }),
        None => json!({ "ok": true }),
    })
}

/// `GET /v1/collections/:id/curation` (`getCollectionCuration`): the newest
/// curation of a published collection.
pub async fn collection_curation(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let curator = access::curate_collection(&state, &signer, &id, Since::read()).await?;
    let mut conn = state.items.pool().acquire().await?;
    Ok(maybe(
        rows::newest_of_collection(&mut conn, curator.collection()).await?,
    ))
}

/// The `itemIds` of a query string as builder-server's query parser reads
/// them: repeated, or bracketed (`itemIds[]`), they are a list; given once
/// without brackets, a string, which it refuses.
fn item_ids(query: Option<&str>) -> Result<Option<Vec<Uuid>>, ApiError> {
    let mut plain = Vec::new();
    let mut bracketed = Vec::new();
    for (key, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "itemIds" => plain.push(value.into_owned()),
            "itemIds[]" => bracketed.push(value.into_owned()),
            _ => {}
        }
    }
    if plain.len() == 1 && bracketed.is_empty() {
        return Err(ApiError::bad_request("Invalid itemIds parameter provided.")
            .with_data(json!({ "itemIds": plain[0] })));
    }
    let raw: Vec<String> = plain.into_iter().chain(bracketed).collect();
    if raw.is_empty() {
        return Ok(None);
    }
    if !i64::try_from(raw.len()).is_ok_and(|len| len <= rows::MAX_ROWS) {
        return Err(ApiError::bad_request("Invalid itemIds parameter provided.")
            .with_data(json!({ "itemIds": raw.len() })));
    }
    Ok(Some(
        raw.iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect(),
    ))
}

/// `GET /v1/collections/:id/itemCurations` (`getCollectionItemCurations`):
/// the newest curation of each of the collection's items, or of the items
/// `itemIds` names.
pub async fn collection_item_curations(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Json<ApiData<Vec<ItemCurationOut>>>, ApiError> {
    let items = item_ids(query.as_deref())?;
    let curator = access::curate_collection(&state, &signer, &id, Since::read()).await?;
    let mut conn = state.items.pool().acquire().await?;
    let rows =
        rows::newest_of_collection_items(&mut conn, curator.collection(), items.as_deref()).await?;
    drop(conn);
    crate::limits::charge_rows(&state, &headers, Some(signer.as_str()), rows.len());
    Ok(Json(ApiData::ok(rows)))
}

/// `GET /v1/items/:id/curation` (`getItemCuration`): the item's newest
/// curation.
pub async fn item_curation(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let curator = access::curate_item(&state, &signer, &id, Since::read()).await?;
    let mut conn = state.items.pool().acquire().await?;
    let item = curator.item().unwrap_or_default();
    Ok(maybe(rows::newest_of_item(&mut conn, item).await?))
}

/// A `PATCH` body's `curation`, which builder-server reads before anything
/// else.
fn curation_of(body: &Bytes) -> Result<Value, ApiError> {
    let body: Value =
        serde_json::from_slice(body).map_err(|_| ApiError::bad_request("Invalid request body"))?;
    body.get("curation")
        .filter(|c| !c.is_null())
        .cloned()
        .ok_or_else(|| ApiError::bad_request("Could not get curation from request"))
}

/// `PATCH /v1/collections/:id/curation` (`updateCollectionCuration`).
pub async fn update_collection_curation(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<CollectionCurationOut>>, ApiError> {
    let curation = curation_of(&body)?;
    let curator = access::curate_collection(&state, &signer, &id, Since::write()).await?;
    let update = writes::parse_update(&curation)?;
    if update.content_hash.is_some() {
        return Err(ApiError::bad_request("Invalid schema"));
    }
    Ok(Json(ApiData::ok(
        writes::update_collection(&state, curator, update).await?,
    )))
}

/// `POST /v1/collections/:id/curation` (`insertCollectionCuration`).
pub async fn request_collection_curation(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<CollectionCurationOpened>>, ApiError> {
    let curator = access::curate_collection(&state, &signer, &id, Since::write()).await?;
    let body: Value = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("Invalid request body"))?
    };
    let open = writes::parse_open(body.get("curation"))?;
    Ok(Json(ApiData::ok(
        writes::open_collection(&state, curator, open).await?,
    )))
}

/// `PATCH /v1/items/:id/curation` (`updateItemCuration`).
pub async fn update_item_curation(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<ItemCurationOut>>, ApiError> {
    let curation = curation_of(&body)?;
    let curator = access::curate_item(&state, &signer, &id, Since::write()).await?;
    let update = writes::parse_update(&curation)?;
    Ok(Json(ApiData::ok(
        writes::update_item(&state, curator, update).await?,
    )))
}

/// `POST /v1/items/:id/curation` (`insertItemCuration`): the body is not
/// read.
pub async fn request_item_curation(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<ItemCurationOpened>>, ApiError> {
    let curator = access::curate_item(&state, &signer, &id, Since::write()).await?;
    Ok(Json(ApiData::ok(writes::open_item(&state, curator).await?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_ids_are_a_list_only_when_repeated_or_bracketed_and_bounded() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        assert_eq!(item_ids(None).unwrap(), None);
        assert_eq!(item_ids(Some("other=1")).unwrap(), None);
        assert_eq!(
            item_ids(Some(&format!("itemIds%5B%5D={a}"))).unwrap(),
            Some(vec![a])
        );
        assert_eq!(
            item_ids(Some(&format!("itemIds={a}&itemIds={b}"))).unwrap(),
            Some(vec![a, b])
        );
        assert_eq!(
            item_ids(Some(&format!("itemIds[]=not-a-uuid&itemIds[]={b}"))).unwrap(),
            Some(vec![b])
        );
        assert_eq!(
            item_ids(Some(&format!("itemIds={a}")))
                .unwrap_err()
                .status(),
            400
        );
        let many: Vec<String> = (0..=rows::MAX_ROWS)
            .map(|_| format!("itemIds[]={a}"))
            .collect();
        assert_eq!(item_ids(Some(&many.join("&"))).unwrap_err().status(), 400);
        let most: Vec<String> = (0..rows::MAX_ROWS)
            .map(|_| format!("itemIds[]={a}"))
            .collect();
        assert!(item_ids(Some(&most.join("&"))).is_ok());
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
