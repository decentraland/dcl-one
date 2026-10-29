//! Creator-signed collection discount coupons (`/v1/coupons`) and the
//! CouponManager state reader. A coupon is local authority
//! (`shop.coupons`, written by `POST /v1/coupons` behind
//! `DATA_WRITES_ENABLED`): the caller must be its signer (the signed-fetch
//! chain with signer `dcl:marketplace|dcl:builder` and intent
//! `dcl:create-coupon`), the EIP-712 signature must verify against one of
//! the chain's CouponManagers, the signer must be the creator of every
//! collection it covers (the collections mirror), and the manager is asked
//! at creation whether it accepts the coupon contract, at which signature
//! indexes it stands and what the coupon's slot already holds.
//! `shop.coupon_state` is the manager's answer as last read: the
//! `coupon-state-refresh` job re-reads the live and upcoming coupons every
//! `MARKETPLACE_COUPON_REFRESH_INTERVAL_SECS` over `TRADE_RPC_URLS` (the
//! chain's entry), `eth_call` through the ONE client.
//!
//! What changed against the old port: one pool (the old component read the
//! squid for collection creators), the creator check reads
//! `shop.collections`, timestamps are `timestamp without time zone`
//! UTC and the due set takes the time as a parameter. A node without an RPC
//! endpoint for the coupon's chain answers the unavailable envelope on
//! `POST` (the reads still serve the stored coupons and their last state).

pub mod chain;
pub mod component;
pub mod contracts;
pub mod errors;
pub mod merkle;
pub mod schema;
pub mod signature;
pub mod types;

#[cfg(test)]
pub(crate) mod component_tests;

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::Serialize;

pub use chain::{ChainReadError, CouponChainReader, RpcCouponChainReader};
pub use component::{CouponsComponent, to_coupon, validate_creation};
pub use contracts::{CouponContracts, CouponMarketplace, coupon_contracts, find_coupon_contracts};
pub use errors::CouponError;
pub use schema::validate_creation_schema;
pub use types::{
    Coupon, CouponCreation, CouponPagination, CouponState, CouponStatus, DbCouponWithState,
};

use crate::jobs::{Jobs, PeriodicCfg};
use crate::market::signed::{MARKETPLACE_AUTH_SIGNERS, require_auth_metadata, require_signer};
use crate::market::wire::{ApiError, get_pagination_params};
use crate::market::{MarketplaceState, SharedState, Source, is_valid_eth_address, unavailable};

pub const CREATE_COUPON_INTENT: &str = "dcl:create-coupon";

const COULD_NOT_CREATE: &str = "Coupon could not be created";
const COULD_NOT_FETCH_ONE: &str = "Could not fetch the coupon";
const COULD_NOT_FETCH_MANY: &str = "Could not fetch the coupons";
const SIGNER_REQUIRED: &str = "A valid signer address is required";

impl MarketplaceState {
    /// The coupon component over this state's pool, client and endpoints.
    pub fn coupons(&self) -> CouponsComponent {
        CouponsComponent::new(
            self.pool.clone(),
            Arc::new(RpcCouponChainReader::new(
                self.http.clone(),
                self.settings.market.trade_rpc_urls.clone(),
            )),
        )
    }
}

fn error_response(status: u16, message: String) -> Response {
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(serde_json::json!({ "ok": false, "message": message })),
    )
        .into_response()
}

/// Upstream answers 400 for every refusal a creator can act on, including
/// the signer mismatch the analogous trade route answers 401 for; ported as
/// it stands.
fn coupon_error_response(e: CouponError) -> Response {
    let message = e.to_string();
    match e {
        CouponError::Duplicate => error_response(409, message),
        CouponError::NotFound(_) => error_response(404, message),
        CouponError::Db(_) | CouponError::Internal(_) => {
            tracing::error!(error = %message, "coupon request failed");
            error_response(500, COULD_NOT_CREATE.to_string())
        }
        _ => error_response(400, message),
    }
}

#[derive(Debug, Serialize)]
pub struct CouponEnvelope {
    pub ok: bool,
    pub data: Coupon,
}

#[derive(Debug, Serialize)]
pub struct CouponsEnvelope {
    pub ok: bool,
    pub data: Vec<Coupon>,
}

/// `POST /v1/coupons` (behind `DATA_WRITES_ENABLED`, `DataWrites::guard`).
pub async fn add_coupon(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    if let Err(e) = require_auth_metadata(
        &headers,
        MARKETPLACE_AUTH_SIGNERS,
        Some(CREATE_COUPON_INTENT),
    ) {
        return e.into_response();
    }
    let recovered = match require_signer(&headers, "post", "/v1/coupons").await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let raw: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return ApiError::bad_request(format!("invalid coupon body: {e}")).into_response();
        }
    };
    if let Err(e) = validate_creation_schema(&raw) {
        return coupon_error_response(e);
    }
    let coupon: CouponCreation = match serde_json::from_value(raw) {
        Ok(c) => c,
        Err(e) => {
            return ApiError::bad_request(format!("invalid coupon body: {e}")).into_response();
        }
    };
    if state
        .settings
        .market
        .trade_rpc_urls
        .for_chain(coupon.chain_id)
        .is_none()
        && !coupon_contracts(coupon.chain_id).is_empty()
    {
        return unavailable(
            Source::CouponManager,
            &format!(
                "no JSON-RPC endpoint is configured for chain {}: the coupon manager cannot be asked",
                coupon.chain_id
            ),
        );
    }
    match state
        .coupons()
        .add_coupon(coupon, &recovered, Utc::now().timestamp_millis())
        .await
    {
        Ok(data) => (StatusCode::CREATED, Json(CouponEnvelope { ok: true, data })).into_response(),
        Err(e) => coupon_error_response(e),
    }
}

/// `GET /v1/coupons?signer=&limit=&offset=`: a creator's coupons, newest first.
pub async fn get_coupons(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let Some(signer) = pairs
        .iter()
        .find(|(k, _)| k == "signer")
        .map(|(_, v)| v.as_str())
        .filter(|v| is_valid_eth_address(v))
    else {
        return error_response(400, SIGNER_REQUIRED.to_string());
    };
    let page = get_pagination_params(&pairs);
    match state
        .coupons()
        .get_coupons_by_signer(
            signer,
            CouponPagination {
                limit: Some(page.limit),
                offset: Some(page.offset),
            },
        )
        .await
    {
        Ok(data) => Json(CouponsEnvelope { ok: true, data }).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "could not fetch the coupons");
            error_response(500, COULD_NOT_FETCH_MANY.to_string())
        }
    }
}

/// `GET /v1/coupons/{id}`: one coupon; a non-uuid id is the 404.
pub async fn get_coupon(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    if uuid::Uuid::parse_str(&id).is_err() {
        return error_response(404, CouponError::NotFound(id).to_string());
    }
    match state.coupons().get_coupon(&id).await {
        Ok(data) => Json(CouponEnvelope { ok: true, data }).into_response(),
        Err(CouponError::NotFound(id)) => {
            error_response(404, CouponError::NotFound(id).to_string())
        }
        Err(e) => {
            tracing::error!(error = %e, "could not fetch the coupon");
            error_response(500, COULD_NOT_FETCH_ONE.to_string())
        }
    }
}

/// Attaches `coupon-state-refresh` when an RPC endpoint serves a coupon
/// chain: one bounded batch of the due coupons per tick.
pub fn spawn_job(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    let served = state
        .settings
        .market
        .trade_rpc_urls
        .iter()
        .any(|(chain, _)| !coupon_contracts(chain).is_empty());
    if !served {
        tracing::info!(
            "no TRADE_RPC_URLS entry serves a coupon chain: coupon-state-refresh is off; stored coupon states stay as last read"
        );
        return;
    }
    let state = state.clone();
    let interval = state
        .settings
        .market
        .indexes
        .coupon_refresh_interval
        .max(Duration::from_secs(30));
    jobs.spawn_periodic_leased(
        "coupon-state-refresh",
        PeriodicCfg::every(interval)
            .with_jitter(Duration::from_secs(30))
            .with_error_backoff(Duration::from_secs(1800)),
        move |ctx| {
            let state = state.clone();
            async move {
                let refreshed = state
                    .coupons()
                    .refresh_state()
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                ctx.progress(format!("refreshed {refreshed} coupon state(s)"));
                Ok(())
            }
        },
    );
}
