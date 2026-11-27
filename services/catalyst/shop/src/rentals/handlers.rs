//! The four routes: the listing page and the price histogram read the local
//! tables. Create and refresh read the published LAND and NFT indexes on
//! mainnet NFT walkers and the configured marketplace subgraph elsewhere.
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
use super::types::{
    ADDRESS_ZERO, ContractRentalListing, RentalListingCreation, RentalListingPeriodInput,
};
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
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "two hex digits make a byte"
                )]
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
    if let NftReadError::Subgraph(detail) = err {
        tracing::warn!(error = %detail, "marketplace subgraph reader unavailable on a rentals route");
        return unavailable(
            Source::MarketplaceSubgraph,
            "The marketplace subgraph is unavailable for the listing asset",
        );
    }
    if matches!(err, NftReadError::MetadataUnavailable) {
        return unavailable(
            Source::NftIndex,
            "The NFT index has not published the listing asset metadata",
        );
    }
    if let NftReadError::Database(e) = err {
        tracing::warn!(error = %e, "failed to read the indexed rental asset");
        return ApiError::from(e).into_response();
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
        metadata_block: nft.metadata_block,
        updated_at: naive_secs(nft.updated_at, now),
    }
}

/// An indexed venue can establish custody once its publication and its rent
/// both cover the transfer into the venue.
pub(super) fn held_for_lessor(
    evidence: Option<&super::index::RentEvidence>,
    transfer_block: Option<i64>,
    observed_block: Option<i64>,
    transferred_at: i64,
    signer: &str,
) -> Option<bool> {
    let Some(evidence) = evidence else {
        return Some(false);
    };
    match evidence.coverage(transfer_block, observed_block, transferred_at) {
        super::index::RentCoverage::Pending => None,
        super::index::RentCoverage::NoCoveringRent => Some(false),
        super::index::RentCoverage::Covered => Some(
            evidence
                .signer
                .as_deref()
                .is_some_and(|lessor| lessor.eq_ignore_ascii_case(signer)),
        ),
    }
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
    if !period_days_fit(&body.periods) {
        return Err(ApiError::bad_request(
            "minDays and maxDays must fit a 32-bit integer",
        ));
    }

    // One chain is served: a listing signed for another is not fillable here.
    let served_chain = body.chain_id;
    if Some(served_chain) != state.served_chain() || body.network != "ETHEREUM" {
        return Err(ApiError::bad_request(
            "The chain id and network of the listing are not the ones supported by the server",
        ));
    }
    // The stored address drives the status sync, so it is the served contract's, never the caller's.
    let Some(rentals) = u64::try_from(served_chain).ok().and_then(rentals_contract) else {
        return Err(ApiError::bad_request(format!(
            "There is no rentals contract for chain id {served_chain}"
        )));
    };
    let chain = super::index::RentalsChain::from_id(served_chain)
        .ok_or_else(|| ApiError::bad_request("Unsupported Rentals chain"))?;
    if !body.rental_contract_address.eq_ignore_ascii_case(rentals) {
        return Err(ApiError::bad_request(
            "The rental contract address does not match the Rentals contract of the given chain",
        ));
    }
    body.rental_contract_address = rentals.to_string();

    let contract = ContractRentalListing::from_creation(&signer, &body);
    let chain_id = u64::try_from(body.chain_id)
        .map_err(|_| ApiError::bad_request("Unsupported Rentals chain"))?;
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
    let (nft, rent_evidence) = match state
        .listing_asset(
            body.chain_id,
            &body.contract_address,
            &body.token_id,
            Some(now),
        )
        .await
    {
        Ok(Some(nft)) => nft,
        Ok(None) => return Err(ApiError::not_found("NFT not found")),
        Err(e) => return Ok(reader_unavailable(e)),
    };

    // A raw active rent can conservatively block a duplicate listing while
    // the Rentals cursor is unavailable; only published evidence can name
    // the lessor or authorize custody.
    if rent_evidence
        .as_ref()
        .and_then(|r| r.observed_ends_at)
        .is_some_and(|ends_at| ends_at > now.timestamp())
    {
        return Err(ApiError::conflict(
            "An open rental already exists for this token",
        ));
    }
    let rentals_unavailable = rent_evidence
        .as_ref()
        .is_some_and(|evidence| evidence.published_through.is_none());
    let venue_owned = nft.owner_address.eq_ignore_ascii_case(rentals);
    let signer_owns = nft.owner_address.eq_ignore_ascii_case(&signer);
    let indexed_third_party = nft.land_cursor_updated_at.is_some() && !venue_owned && !signer_owns;

    // An already-open listing takes precedence during a Rentals outage.
    // A third-party LAND owner needs no Rentals check unless that conflict exists.
    if (rentals_unavailable || indexed_third_party)
        && let Some(conflict) = state.db.open_listing_conflict(&body).await?
        && (rentals_unavailable || !super::index::ready_on(state.db.pool(), chain, now).await)
    {
        return Err(ApiError::conflict(conflict.message()));
    }
    if rentals_unavailable {
        return Ok(unavailable(
            Source::RentalsIndex,
            "The Rentals index for the listing chain is not ready",
        ));
    }

    let relist = if venue_owned {
        let Some(held) = held_for_lessor(
            rent_evidence.as_ref(),
            nft.transfer_block,
            Some(nft.metadata_block),
            nft.transferred_at,
            &signer,
        ) else {
            return Ok(unavailable(
                Source::RentalsIndex,
                "The asset is held by the Rentals contract and the rentals index is not ready to name its lessor",
            ));
        };
        held
    } else {
        false
    };
    if !signer_owns && !relist {
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
    let mut tx = state.db.pool().begin().await?;
    let write_time = chrono::Utc::now();
    if let Some(expected) = nft.land_cursor_updated_at
        && !super::status::land_publication_matches(&mut tx, chain.land(), expected).await?
    {
        return Ok(reader_unavailable(NftReadError::Unavailable));
    }
    if signer_owns
        && !venue_owned
        && nft.land_cursor_updated_at.is_some()
        && let Some(transfer_block) = nft.transfer_block
        && super::index::observed_running_rent(
            &mut tx,
            chain,
            &body.contract_address,
            &body.token_id,
            transfer_block,
            write_time,
        )
        .await?
    {
        return Err(ApiError::conflict(
            "An open rental already exists for this token",
        ));
    }
    if let Some(evidence) = &rent_evidence {
        let cursor = super::index::lock_cursor(&mut tx, chain).await?;
        if cursor != evidence.cursor
            || !cursor.as_ref().is_some_and(|c| {
                crate::market::chain_walk::fresh_fold_on(c, chain.id(), write_time).is_some()
            })
        {
            return Ok(unavailable(
                Source::RentalsIndex,
                "The Rentals publication changed before the listing could be created",
            ));
        }
    }
    let inserted = Database::insert_listing_on(
        &mut tx,
        &nft.nft_id,
        &listing_metadata(&nft, now_naive),
        naive_secs(nft.created_at, now_naive),
        &body,
        &signer,
        now_naive,
    )
    .await;

    match inserted {
        Ok(listing) => {
            tx.commit().await?;
            Ok(Ok2(StatusCode::CREATED, listing).into_response())
        }
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
        use super::status::SyncOutcome;
        match super::status::sync_listing(state.db.pool(), uuid, nft.as_ref(), clock).await? {
            SyncOutcome::Applied => {}
            SyncOutcome::LandUnavailable => {
                return Ok(reader_unavailable(NftReadError::Unavailable));
            }
            SyncOutcome::RentalsUnavailable => {
                return Ok(unavailable(
                    Source::RentalsIndex,
                    "The Rentals index for the listing chain is not ready",
                ));
            }
        }
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
            tracing::debug!(rental_id = %id, "NFT not found by the listing asset reader during refresh; metadata unchanged");
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

/// Whether every period's day bounds fit their `integer` columns.
fn period_days_fit(periods: &[RentalListingPeriodInput]) -> bool {
    periods
        .iter()
        .all(|p| i32::try_from(p.min_days).is_ok() && i32::try_from(p.max_days).is_ok())
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
    fn period_days_beyond_the_integer_columns_do_not_fit() {
        let period = |min_days, max_days| RentalListingPeriodInput {
            min_days,
            max_days,
            price_per_day: "1".to_string(),
        };
        assert!(period_days_fit(&[period(1, 30)]));
        assert!(!period_days_fit(&[
            period(1, 30),
            period(1, i64::from(i32::MAX) + 1)
        ]));
        assert!(!period_days_fit(&[period(i64::from(i32::MIN) - 1, 1)]));
    }

    #[test]
    fn database_reader_errors_are_server_errors_not_index_readiness() {
        let response = reader_unavailable(NftReadError::Database(sqlx::Error::RowNotFound));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
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
