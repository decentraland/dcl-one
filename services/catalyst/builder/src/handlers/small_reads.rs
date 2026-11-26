//! The small public reads, unsigned: `GET /v1/info`, the rarities, a minted
//! item's utility and contents by contract and on-chain id, and the
//! published collections' contract addresses.

use axum::Json;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::SharedState;
use crate::access;
use crate::collections::listing::{ListQuery, PublishedAddresses};
use crate::http::{ApiData, ApiError};
use crate::items::minted::contents_answer;
use crate::rarities::Rarity;

const ADDRESSES_CACHE: &str = "public,max-age=300,s-maxage=300";

/// `GET /v1/info`: builder-server's body as its deployment answers it, with
/// no package version.
pub async fn get_info() -> Json<ApiData<Value>> {
    Json(ApiData::ok(json!({ "version": "" })))
}

/// `GET /v1/rarities`.
pub async fn get_rarities(
    State(state): State<SharedState>,
) -> Result<Json<ApiData<Vec<Rarity>>>, ApiError> {
    let rarities = state.rarities.all(&state).await?;
    Ok(Json(ApiData::ok(rarities.as_ref().clone())))
}

/// `GET /v1/rarities/:name`.
pub async fn get_rarity(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Json<ApiData<Rarity>>, ApiError> {
    Ok(Json(ApiData::ok(
        state.rarities.named(&state, &name).await?,
    )))
}

/// `GET /v1/published-collections/:address/items/:id/utility`: the path
/// compared exactly, as builder-server compares it.
pub async fn get_item_utility(
    State(state): State<SharedState>,
    Path((address, id)): Path<(String, String)>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let Some(item) = access::minted_item(&state, &address, &id).await? else {
        return Err(ApiError::not_found("The item doesn't exist.")
            .with_data(json!({ "id": format!("{address}-{id}") })));
    };
    let utility = state.items.utility(&item).await?;
    Ok(Json(ApiData::ok(json!({ "utility": utility }))))
}

/// `ethers.utils.isAddress` of a lowercased value: 40 hex digits, `0x`
/// optional.
fn is_address(lowercased: &str) -> bool {
    let hex = lowercased.strip_prefix("0x").unwrap_or(lowercased);
    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `GET /v1/items/:collectionAddress/:itemId/contents`: both lowercased,
/// then checked as builder-server checks them; the entries anyone may read.
pub async fn get_item_contents(
    State(state): State<SharedState>,
    Path((address, id)): Path<(String, String)>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let (address, id) = (address.to_lowercase(), id.to_lowercase());
    if !is_address(&address) {
        return Err(ApiError::bad_request(format!("Invalid address {address}"))
            .with_data(json!({ "contractAddress": address })));
    }
    if crate::collections::listing::js_int(&id).is_none() {
        return Err(ApiError::bad_request(format!("Invalid Item ID {id}"))
            .with_data(json!({ "itemId": id })));
    }
    let item = access::minted_item(&state, &address, &id)
        .await?
        .ok_or_else(ApiError::item_does_not_exist)?;
    let kind = state.items.minted_kind(&item).await?;
    let video = match kind.video {
        Some(hash) if kind.smart => access::read_file(&state, None, &hash)
            .await?
            .map(|_| Some(hash)),
        _ => Some(None),
    };
    let open = access::public_entries(&state, &item).await?;
    Ok(Json(ApiData::ok(contents_answer(open, kind.smart, video))))
}

/// `GET /v1/addresses`: kept five minutes by any cache, as builder-server
/// sends it.
pub async fn get_addresses(
    State(state): State<SharedState>,
    RawQuery(raw): RawQuery,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    let q = ListQuery::parse(raw.as_deref());
    let request = PublishedAddresses::new(&q)?;
    let data = state.items.published_addresses(&request).await?;
    crate::limits::charge(&state, &headers, None, crate::limits::ADDRESSES_TOKENS);
    let mut response = Json(ApiData::ok(data)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(ADDRESSES_CACHE),
    );
    Ok(response)
}
