//! The four routes: the listing page and the price histogram read the local
//! tables. Create and refresh read the listing asset from a published
//! land-authz snapshot and return its unavailable envelope until ready.
//! Both writes require `DATA_WRITES_ENABLED`. Creation also requires signed fetch.

use std::collections::HashMap;

use axum::extract::{OriginalUri, Path, Query, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::db::{
    Database, ListingFilters, ListingMetadata, ListingQuery, PriceFilters, STATUS_VALUES,
    is_decimal,
};
use super::http::{ApiError, Ok2};
use super::nft::{LandNft, NftReadError};
use super::signature::{has_valid_v, rentals_contract, verify_rentals_listing_signature};
use super::types::{ADDRESS_ZERO, ContractRentalListing, RentalListingCreation};
use super::{SharedState, signed_path};
use crate::market::{Source, is_valid_eth_address, unavailable};

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 100;
pub const ROUTE_LISTINGS: &str = "/v1/rentals-listings";

fn pagination(params: &HashMap<String, String>) -> (i64, i64) {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|l| l.clamp(1, MAX_LIMIT))
        .unwrap_or(DEFAULT_LIMIT);
    let offset = params
        .get("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|o| *o >= 0)
        .unwrap_or(0);
    (limit, offset)
}

/// Every value of a repeated query key (`?status=open&status=executed`),
/// which the `HashMap` extractor collapses.
pub(crate) fn all_values(raw_query: &str, key: &str) -> Vec<String> {
    raw_query
        .split('&')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            let k = it.next()?;
            if k != key {
                return None;
            }
            Some(urldecode(it.next().unwrap_or("")))
        })
        .filter(|s| !s.is_empty())
        .collect()
}

pub(crate) fn urldecode(s: &str) -> String {
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(((hi << 4) | lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn num(params: &HashMap<String, String>, k: &str) -> Result<Option<i32>, ApiError> {
    match params.get(k) {
        Some(v) => v
            .parse::<i32>()
            .map(Some)
            .map_err(|_| ApiError::bad_request(format!("{k} must be a number"))),
        None => Ok(None),
    }
}

fn rental_days(raw: &str) -> Vec<i32> {
    all_values(raw, "rentalDays")
        .iter()
        .filter_map(|s| s.parse::<i32>().ok())
        .filter(|d| *d != 0)
        .collect()
}

pub async fn get_rentals_listings(
    State(state): State<SharedState>,
    RawQuery(raw): RawQuery,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let raw = raw.unwrap_or_default();
    let (limit, offset) = pagination(&params);

    let boolean = |k: &str| -> Result<Option<bool>, ApiError> {
        match params.get(k).map(String::as_str) {
            Some("true") => Ok(Some(true)),
            Some("false") => Ok(Some(false)),
            Some(_) => Err(ApiError::bad_request(format!("{k} must be a boolean"))),
            None => Ok(None),
        }
    };
    let status = all_values(&raw, "status");
    if let Some(bad) = status.iter().find(|s| !STATUS_VALUES.contains(&s.as_str())) {
        return Err(ApiError::bad_request(format!(
            "status {bad} is not one of {}",
            STATUS_VALUES.join(", ")
        )));
    }
    let token_id = params.get("tokenId").cloned();
    if token_id.as_deref().is_some_and(|t| !is_decimal(t)) {
        return Err(ApiError::bad_request("tokenId must be a number"));
    }
    for (key, value) in [
        ("minPricePerDay", params.get("minPricePerDay")),
        ("maxPricePerDay", params.get("maxPricePerDay")),
    ] {
        if value.is_some_and(|v| !is_decimal(v)) {
            return Err(ApiError::bad_request(format!("{key} must be a number")));
        }
    }

    let filter = ListingFilters {
        category: params.get("category").cloned(),
        text: params.get("text").cloned(),
        lessor: params.get("lessor").cloned(),
        tenant: params.get("tenant").cloned(),
        status,
        token_id,
        contract_addresses: all_values(&raw, "contractAddresses"),
        nft_ids: all_values(&raw, "nftIds"),
        network: params.get("network").cloned(),
        updated_after: params.get("updatedAfter").and_then(|s| s.parse().ok()),
        target: Some(
            params
                .get("target")
                .cloned()
                .unwrap_or_else(|| ADDRESS_ZERO.to_string()),
        ),
        min_price_per_day: params.get("minPricePerDay").cloned(),
        max_price_per_day: params.get("maxPricePerDay").cloned(),
        min_distance_to_plaza: num(&params, "minDistanceToPlaza")?,
        max_distance_to_plaza: num(&params, "maxDistanceToPlaza")?,
        min_estate_size: num(&params, "minEstateSize")?,
        max_estate_size: num(&params, "maxEstateSize")?,
        adjacent_to_road: boolean("adjacentToRoad")?,
        rental_days: rental_days(&raw),
    };

    let query = ListingQuery {
        sort_by: params.get("sortBy").cloned(),
        sort_direction: params.get("sortDirection").cloned(),
        offset,
        limit,
        filter,
        history: params.get("history").map(|s| s == "true").unwrap_or(false),
    };

    let page = state.db.get_listings(&query).await?;
    Ok(Ok2(StatusCode::OK, page).into_response())
}

fn reader_unavailable(err: NftReadError) -> Response {
    if matches!(err, NftReadError::MetadataUnavailable) {
        return unavailable(
            Source::NftIndex,
            "The NFT index has not published the listing asset metadata",
        );
    }
    if let NftReadError::Database(e) = err {
        tracing::warn!(error = %e, "failed to read the indexed rental asset");
    }
    crate::market::nft_reads::land_unavailable()
}

fn naive_secs(secs: i64, fallback: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .map(|d| d.naive_utc())
        .unwrap_or(fallback)
}

pub(super) fn listing_metadata(nft: &LandNft, now: chrono::NaiveDateTime) -> ListingMetadata<'_> {
    ListingMetadata {
        category: &nft.category,
        search_text: &nft.search_text,
        distance_to_plaza: nft.distance_to_plaza,
        adjacent_to_road: nft.adjacent_to_road,
        estate_size: nft.estate_size,
        membership_block: nft.membership_block,
        updated_at: naive_secs(nft.updated_at, now),
    }
}

/// An indexed venue can establish custody once its publication and its rent
/// both cover the transfer into the venue.
pub(super) fn held_for_lessor(
    evidence: Option<&super::index::RentEvidence>,
    transfer_block: i64,
    signer: &str,
) -> Option<bool> {
    let Some(evidence) = evidence else {
        return Some(false);
    };
    if !evidence.covers_transfer(transfer_block) {
        return None;
    }
    Some(
        evidence
            .signer
            .as_deref()
            .is_some_and(|lessor| lessor.eq_ignore_ascii_case(signer)),
    )
}

pub async fn create_rentals_listing(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    axum::extract::Json(mut body): axum::extract::Json<RentalListingCreation>,
) -> Result<Response, ApiError> {
    let signer = state
        .require_signer(&headers, "post", &signed_path(&original, ROUTE_LISTINGS))
        .await?;
    // After the signature, so an unsigned request keeps the old 401. A refresh (below) only re-reads the chain for a
    // listing that exists, and stays open.
    state.require_marketplace()?;

    let now = chrono::Utc::now();
    if body.expiration < now.timestamp_millis() {
        return Err(ApiError::bad_request("The rental listing has expired"));
    }
    if !is_valid_eth_address(&body.contract_address) {
        return Err(ApiError::bad_request("contractAddress must be an address"));
    }
    if !is_decimal(&body.token_id) {
        return Err(ApiError::bad_request("tokenId must be a number"));
    }
    if body.periods.is_empty() {
        return Err(ApiError::bad_request(
            "The rental listing needs at least one period",
        ));
    }

    // One chain is served: a listing signed for another is not fillable here.
    let served_chain = crate::market::catalog::served_chains().0;
    if body.chain_id != served_chain || body.network != "ETHEREUM" {
        return Err(ApiError::bad_request(
            "The chain id and network of the listing are not the ones supported by the server",
        ));
    }
    // The stored address drives the status sync, so it is the served contract's, never the caller's.
    let Some(rentals) = rentals_contract(served_chain as u64) else {
        return Err(ApiError::bad_request(format!(
            "There is no rentals contract for chain id {served_chain}"
        )));
    };
    if !body.rental_contract_address.eq_ignore_ascii_case(rentals) {
        return Err(ApiError::bad_request(
            "The rental contract address does not match the Rentals contract of the given chain",
        ));
    }
    body.rental_contract_address = rentals.to_string();

    let contract = ContractRentalListing::from_creation(&signer, &body);
    let chain_id = body.chain_id as u64;
    match verify_rentals_listing_signature(&contract, chain_id) {
        Ok(true) => {}
        Ok(false) => {
            let msg = if !has_valid_v(&body.signature) {
                "The server does not accept ECDSA signatures with V as 0 or 1"
            } else {
                "The provided signature is invalid"
            };
            return Err(ApiError::bad_request(msg));
        }
        Err(e) => return Err(ApiError::bad_request(e.to_string())),
    }

    let nft = match state
        .nft(body.chain_id, &body.contract_address, &body.token_id)
        .await
    {
        Ok(Some(nft)) => nft,
        Ok(None) => return Err(ApiError::not_found("NFT not found")),
        Err(e) => return Ok(reader_unavailable(e)),
    };

    // Only the indexed contract has rents to read; elsewhere a rented asset is the contract's, so the owner check refuses it.
    let rent_evidence = if super::index::walks(rentals) {
        Some(
            super::index::rent_evidence(
                state.db.pool(),
                rentals,
                &body.contract_address,
                &body.token_id,
            )
            .await?,
        )
    } else {
        None
    };
    if rent_evidence
        .as_ref()
        .and_then(|r| r.ends_at)
        .is_some_and(|ends_at| ends_at > now.timestamp())
    {
        return Err(ApiError::conflict(
            "An open rental already exists for this token",
        ));
    }

    let relist = if nft.owner_address.eq_ignore_ascii_case(rentals) {
        let Some(held) = held_for_lessor(rent_evidence.as_ref(), nft.transfer_block, &signer)
        else {
            return Ok(unavailable(
                Source::RentalsIndex,
                "The asset is held by the Rentals contract and the rentals index is not ready to name its lessor",
            ));
        };
        held
    } else {
        false
    };
    if !nft.owner_address.eq_ignore_ascii_case(&signer) && !relist {
        return Err(ApiError::unauthorized(format!(
            "The owner of the NFT {} is not the lessor {}",
            nft.owner_address, signer
        )));
    }
    if nft.category != "parcel" && nft.category != "estate" {
        return Err(ApiError::bad_request(
            "The provided NFT is not a LAND parcel or an Estate",
        ));
    }
    if nft.category == "estate" && nft.estate_size.unwrap_or(0) == 0 {
        return Err(ApiError::bad_request(
            "The provided Estate does not have any parcels",
        ));
    }

    let now_naive = now.naive_utc();
    let inserted = state
        .db
        .insert_listing(
            &nft.nft_id,
            &listing_metadata(&nft, now_naive),
            naive_secs(nft.created_at, now_naive),
            &body,
            &signer,
            now_naive,
        )
        .await;

    match inserted {
        Ok(listing) => Ok(Ok2(StatusCode::CREATED, listing).into_response()),
        Err(e) => match Database::conflict(&e) {
            Some(conflict) => Err(ApiError::conflict(conflict.message())),
            None => Err(ApiError::from(e)),
        },
    }
}

pub async fn refresh_rentals_listing(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let not_found = || ApiError::not_found("Rental listing was not found");
    let (chain_id, contract_address, token_id) =
        state.db.listing_asset(&id).await?.ok_or_else(not_found)?;

    let clock = chrono::Utc::now();
    let now = clock.naive_utc();
    let nft = state.nft(chain_id, &contract_address, &token_id).await;
    if let (Ok(nft), Ok(uuid)) = (&nft, uuid::Uuid::parse_str(&id)) {
        super::status::sync_listing(state.db.pool(), uuid, nft.as_ref(), clock).await?;
    }
    match nft {
        Ok(Some(nft)) => {
            if let Some(refreshed) = state
                .db
                .refresh_metadata_for_rental(&id, &listing_metadata(&nft, now), now)
                .await?
            {
                return Ok(Ok2(StatusCode::OK, refreshed).into_response());
            }
            // No row: the reader's is older than the stored one, which stays.
        }
        Ok(None) => {
            tracing::debug!(rental_id = %id, "NFT not found in the land-authz index during refresh; metadata unchanged");
        }
        Err(e) => return Ok(reader_unavailable(e)),
    }

    let listing = state
        .db
        .get_listing_by_id(&id)
        .await?
        .ok_or_else(not_found)?;
    Ok(Ok2(StatusCode::OK, listing).into_response())
}

pub async fn get_rental_listings_prices(
    State(state): State<SharedState>,
    RawQuery(raw): RawQuery,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let raw = raw.unwrap_or_default();
    let boolean = |k: &str| -> Option<bool> {
        match params.get(k).map(String::as_str) {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        }
    };
    let filters = PriceFilters {
        category: params.get("category").cloned(),
        adjacent_to_road: boolean("adjacentToRoad"),
        min_distance_to_plaza: num(&params, "minDistanceToPlaza")?,
        max_distance_to_plaza: num(&params, "maxDistanceToPlaza")?,
        min_estate_size: num(&params, "minEstateSize")?,
        max_estate_size: num(&params, "maxEstateSize")?,
        rental_days: rental_days(&raw),
    };

    let prices = state.db.get_prices(&filters).await?;
    let map: serde_json::Map<String, serde_json::Value> =
        prices.into_iter().map(|(p, c)| (p, json!(c))).collect();
    Ok(Ok2(StatusCode::OK, serde_json::Value::Object(map)).into_response())
}

/// `GET /v1/rentals-index`: the `rentals` chain index's cursor (published,
/// last block, rewinds), so a client waiting for a status change knows
/// whether the statuses are live or as posted.
pub async fn get_rentals_index(State(state): State<SharedState>) -> Response {
    Ok2(StatusCode::OK, super::index::status(state.db.pool()).await).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_keys_are_collected_and_decoded() {
        let raw = "status=open&status=executed&nftIds=0xabc-1&nftIds=0xabc%2D2&rentalDays=7&rentalDays=x&rentalDays=0&other=1";
        assert_eq!(all_values(raw, "status"), vec!["open", "executed"]);
        assert_eq!(all_values(raw, "nftIds"), vec!["0xabc-1", "0xabc-2"]);
        assert_eq!(rental_days(raw), vec![7]);
        assert!(all_values(raw, "missing").is_empty());
        assert_eq!(urldecode("a+b%20c%zz"), "a b c%zz");
    }

    #[test]
    fn pagination_clamps_to_the_old_bounds() {
        let p = |limit: Option<&str>, offset: Option<&str>| {
            let mut m = HashMap::new();
            if let Some(l) = limit {
                m.insert("limit".to_string(), l.to_string());
            }
            if let Some(o) = offset {
                m.insert("offset".to_string(), o.to_string());
            }
            pagination(&m)
        };
        assert_eq!(p(None, None), (100, 0));
        assert_eq!(p(Some("500"), Some("-3")), (100, 0));
        assert_eq!(p(Some("0"), Some("20")), (1, 20));
        assert_eq!(p(Some("x"), Some("y")), (100, 0));
    }
}
