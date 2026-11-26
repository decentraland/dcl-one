//! Pending re-creations after a contract signature index bump.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::Value;
use sqlx::types::Json as DbJson;

use crate::shared::signed_fetch::AUTH_CHAIN_HEADER_PREFIX;

use super::chain_walk::ChainNet;
use super::signed::{MARKETPLACE_AUTH_SIGNERS, require_auth_metadata, require_signer};
use super::wire::{ApiError, DataTotal, Params, strict_page};
use super::{
    MarketplaceState, SharedState, Source, listings, nft_reads, owned_items, trade_events, trades,
    unavailable,
};

const PATH: &str = "/v1/cancelled-trades";
const REASON: &str = "contract_signature_index_bump";
const TYPES: &[&str] = &["bid", "public_nft_order", "public_item_order"];

pub(super) async fn get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if !headers.contains_key(format!("{AUTH_CHAIN_HEADER_PREFIX}0").as_str()) {
        return ApiError::unauthorized("Unauthorized").into_response();
    }
    let signer = match require_signer(state.eip1654.as_deref(), &headers, "get", PATH).await {
        Ok(signer) => signer,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = require_auth_metadata(&headers, MARKETPLACE_AUTH_SIGNERS, None) {
        return e.into_response();
    }
    let p = Params::new(&pairs);
    if p.get_string("reason", None).as_deref() != Some(REASON) {
        return ApiError::bad_request(format!("reason must be one of: {REASON}")).into_response();
    }
    let types = p.get_list("type", &[]);
    if types.iter().any(|t| !TYPES.contains(&t.as_str())) {
        return ApiError::bad_request(format!("type must be one of: {}", TYPES.join(", ")))
            .into_response();
    }
    let (first, skip) = match strict_page(&p) {
        Ok(page) => page,
        Err(e) => return e.into_response(),
    };
    match read(&state, &signer, &types, first, skip).await {
        Ok(page) => Json(page).into_response(),
        Err(response) => response,
    }
}

async fn read(
    state: &MarketplaceState,
    signer: &str,
    types: &[String],
    first: i64,
    skip: i64,
) -> Result<DataTotal<Value>, Response> {
    if !owned_items::walks_here(state) {
        return Err(owned_items::not_ready(
            "this node does not walk the nft index",
        ));
    }
    let catalogue = state.settings.catalogue();
    let mut tx = owned_items::snapshot(
        &state.pool,
        catalogue,
        &ChainNet::ALL,
        owned_items::Rule::Fresh,
        true,
    )
    .await
    .map_err(|error| match error {
        owned_items::ReadError::Db(e) => nft_reads::read_error(e),
        owned_items::ReadError::NotReady(source, message) => unavailable(source, &message),
    })?;
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(format!(
            "{}ms",
            nft_reads::READ_STATEMENT_TIMEOUT.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(nft_reads::read_error)?;
    let now = Utc::now();
    trades::readiness_on(state, &mut tx, now)
        .await
        .map_err(trades::not_ready_response)?;
    trade_events::require_current_on(&mut tx, now).await?;
    let land = state
        .land_network()
        .ok_or_else(nft_reads::land_unavailable)?;
    let land_ready = crate::land_authz::indexer::cursor_observed(&mut *tx, land.network)
        .await
        .map_err(nft_reads::read_error)?
        .is_some_and(|cursor| {
            crate::land_authz::indexer::observation_ready(
                &cursor,
                land.land_registry,
                land.estate_registry,
            )
        });
    let ethereum = catalogue.chain_id(ChainNet::Ethereum);
    let matic = catalogue.chain_id(ChainNet::Matic);
    let digest_venues = [ethereum, matic]
        .into_iter()
        .filter_map(listings::offchain_marketplace_v3)
        .map(|venue| venue.address)
        .collect::<Vec<_>>();
    let (DbJson(data), total, needs_land, digest_missing): (DbJson<Vec<Value>>, i64, bool, bool) =
        sqlx::query_as(include_str!("cancelled_trades.sql"))
            .bind(signer)
            .bind(types)
            .bind(now.naive_utc())
            .bind(ethereum)
            .bind(matic)
            .bind(land.land_registry)
            .bind(land.estate_registry)
            .bind(land_ready)
            .bind(first)
            .bind(skip)
            .bind(digest_venues)
            .bind(REASON)
            .fetch_one(&mut *tx)
            .await
            .map_err(nft_reads::read_error)?;
    if needs_land && !land_ready {
        return Err(nft_reads::land_unavailable());
    }
    if digest_missing {
        return Err(unavailable(
            Source::TradeEventsIndex,
            "a trade's signature digest is still needed to verify its cancellation reason",
        ));
    }
    Ok(DataTotal { data, total })
}
