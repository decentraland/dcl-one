//! MANA sales, resale and LAND (SHOP.md slice 3): the server side of the
//! purchases that pay MANA directly, one rule for every one of them
//! (`shop::policy`), over the closure pass's indexes and this node's own
//! records, never over anything a request asserts.
//!
//! | route (under `/shop-api`) | what | source |
//! | --- | --- | --- |
//! | `POST /v1/purchase-offers` | the quote of every cart line: the rule's eligibility and quotes per method, and for MANA the venue, exact amount, allowance, calldata, gas and the wallet's balances ([`quote`]) | `shop.trades` + the trade-events verdict, `shop.nft_order`, `signatures.rentals` + the rentals index, the registrar; the chain through `TRADE_RPC_URLS` |
//! | `POST /v1/mana/actions` | the calldata of a seller's or owner's action: accept a bid, cancel a trade or a legacy order, transfer, claim or remove a rental, approve MANA ([`actions`]) | the same records, checked on chain |
//! | `POST /v1/executions`, `GET /v1/executions/{id}`, `GET /v1/executions` | a sent transaction reported, tracked to settlement by the `executions-track` job ([`executions`], [`tracker`]) | `shop.executions`; receipts; the indexes |
//!
//! Every write sits behind `DATA_WRITES_ENABLED` (the data plane's switch)
//! and signed fetch; the quote is open (a signed identity narrows the
//! presentation to its session). The venues are a static book per chain
//! ([`venues`]); a controlled deployment (the anvil tests) builds its own,
//! so no route reads an address from a request.

pub mod actions;
pub mod calls;
pub mod chain;
pub mod executions;
pub mod facts;
pub mod quote;
pub mod tracker;
pub mod venues;
pub mod verify;
pub mod wire;

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::Address;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::auth::eip1654::Eip1654Validator;
use crate::http::Outbound;
use crate::market::MarketplaceState;
use crate::shared::signed_fetch::{self, PayloadPolicy, SignedPath};
use crate::shared::writes::DataWrites;
use crate::types::ChainId;

#[derive(Debug, Clone)]
pub struct Settings {
    /// `DATA_WRITES_ENABLED`.
    pub writes_enabled: bool,
    /// Between two tracker passes (the market indexes' cadence by default).
    pub track_interval: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            writes_enabled: false,
            track_interval: Duration::from_secs(15),
        }
    }
}

pub struct State {
    pub pool: PgPool,
    pub http: Outbound,
    pub settings: Settings,
    pub venues: venues::Book,
    pub market: Arc<MarketplaceState>,
    pub rentals: Arc<crate::rentals::State>,
    eip1654: Option<Arc<dyn Eip1654Validator>>,
}

pub type SharedState = Arc<State>;

impl State {
    /// The production state: the venues book over the market's per-chain
    /// endpoints (`TRADE_RPC_URLS`, `ETH_RPC_URL` for Ethereum).
    pub fn new(
        pool: PgPool,
        http: Outbound,
        market: Arc<MarketplaceState>,
        rentals: Arc<crate::rentals::State>,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        let m = market.clone();
        let venues = venues::Book::production(move |chain_id| {
            let net = match chain_id {
                1 => crate::market::chain_walk::ChainNet::Ethereum,
                137 => crate::market::chain_walk::ChainNet::Matic,
                _ => return None,
            };
            m.chain_rpc_url(net)
        });
        Self {
            pool,
            http,
            settings,
            venues,
            market,
            rentals,
            eip1654,
        }
    }

    /// A state over a given book (the controlled deployments of the tests).
    pub fn with_venues(
        pool: PgPool,
        http: Outbound,
        market: Arc<MarketplaceState>,
        rentals: Arc<crate::rentals::State>,
        settings: Settings,
        venues: venues::Book,
    ) -> Self {
        Self {
            pool,
            http,
            settings,
            venues,
            market,
            rentals,
            eip1654: None,
        }
    }

    /// The chain's reader, when its endpoint is configured.
    pub fn reader(&self, chain_id: ChainId) -> Option<chain::Reader> {
        self.venues
            .chain(chain_id)
            .and_then(|c| c.rpc_url.as_deref())
            .map(|url| chain::Reader::new(self.http.clone(), url))
    }

    async fn verify(
        &self,
        headers: &HeaderMap,
        method: &str,
        original: &Uri,
        route: &str,
    ) -> Result<signed_fetch::Signer, signed_fetch::SignedFetchError> {
        signed_fetch::require_signer_at(
            headers,
            method,
            &SignedPath::at(original, route),
            PayloadPolicy::V6ThenLegacy,
            self.eip1654.as_deref(),
        )
        .await
    }

    /// The signer of a write, as an address; 401 with the lambdas envelope otherwise.
    pub async fn require_signer(
        &self,
        headers: &HeaderMap,
        method: &str,
        original: &Uri,
        route: &str,
    ) -> Result<Address, Response> {
        match self.verify(headers, method, original, route).await {
            Ok(signer) => signer.as_str().parse().map_err(|_| {
                error(
                    StatusCode::UNAUTHORIZED,
                    "Unauthorized",
                    "the signer is not an address",
                )
            }),
            Err(e) => {
                tracing::debug!(error = %e, "auth chain verification failed on a mana route");
                Err(error(
                    StatusCode::UNAUTHORIZED,
                    "Unauthorized",
                    &e.to_string(),
                ))
            }
        }
    }

    /// A read's optional identity: none without a chain, 401 for one that
    /// does not verify (never a silent downgrade to signed out).
    pub async fn optional_signer(
        &self,
        headers: &HeaderMap,
        method: &str,
        original: &Uri,
        route: &str,
    ) -> Result<Option<String>, Response> {
        if !headers.contains_key(format!("{}0", signed_fetch::AUTH_CHAIN_HEADER_PREFIX).as_str()) {
            return Ok(None);
        }
        self.require_signer(headers, method, original, route)
            .await
            .map(|a| Some(format!("{a:#x}")))
    }
}

/// The lambdas error envelope the market routes answer with.
pub fn error(status: StatusCode, label: &str, message: &str) -> Response {
    crate::market::lambdas_error(status, label, message)
}

/// The canonical routes, behind the data-plane write gate.
pub fn shop_router(state: SharedState) -> Router {
    let writes_enabled = state.settings.writes_enabled;
    let app = Router::new()
        .route(quote::ROUTE, post(quote::post_purchase_offers))
        .route(actions::ROUTE, post(actions::post_action))
        .route(
            executions::ROUTE,
            post(executions::post_execution).get(executions::list_executions),
        )
        .route(executions::ROUTE_ONE, get(executions::get_execution))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .layer(axum::middleware::from_fn(
            crate::shared::cors::cors_middleware,
        ))
        .with_state(state);
    DataWrites::guard_open(app, writes_enabled, OPEN_WHILE_DISABLED)
}

/// The execution tracker.
pub fn spawn_jobs(state: &SharedState, jobs: &crate::jobs::Jobs) {
    tracker::spawn(state, jobs);
}

/// The quote route stays open while writes are disabled: it changes
/// nothing and a web2 buyer's guidance depends on it.
pub const OPEN_WHILE_DISABLED: &[&str] = &[quote::ROUTE];
