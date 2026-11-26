//! Rentals: the LAND/Estate rental-listing signature store (the old
//! `catalyrst-signatures`, itself the Rust port of
//! decentraland/signatures-server, moved here from `src/signatures` as
//! SHOP.md slice 3 asked), over the ONE pool (`signatures.*`,
//! `schemas/32_signatures.sql`) and the published land-authz index.
//!
//! Where each route reads (the stage-5 data-plane rule: every route names its
//! source, and a route whose source is unset answers the unavailable
//! envelope, never an empty page presented as the truth):
//! - `GET /v1/rentals-listings`, `GET /v1/rental-listings/prices`: local
//!   `signatures.rentals` / `rental_listings` / `rental_periods` /
//!   `rental_assets`. These are this node's own signed offers, so an empty
//!   answer IS the truth and the reads always serve.
//! - `POST /v1/rentals-listings`: local authority behind
//!   `DATA_WRITES_ENABLED` and signed fetch (ADR-44, the 6.x payload then the
//!   legacy one, as the old crate accepted); the asset's owner, category and
//!   LAND search columns on a node that walks NFTs come from a published land-authz snapshot joined
//!   to the indexed LAND data and proximity map. Until the index is ready,
//!   the route answers the unavailable envelope for the missing publication. On a node
//!   that does not walk NFTs, the configured marketplace subgraph supplies the asset.
//!   A listing must name the
//!   served Ethereum chain and its Rentals contract (stored as the
//!   contract's address); an asset whose last rent at that contract is
//!   still running answers 409, and an asset the contract still holds may
//!   be listed by that rent's lessor once the `rentals` index is ready (the
//!   unavailable envelope until then). The index walks only the mainnet
//!   contract, so off mainnet no rent is read and an asset the contract
//!   holds is refused as not the lessor's (401).
//! - `PATCH /v1/rentals-listings/{id}`: the same reader refreshes the asset
//!   row, unless its row is older than the stored one, and syncs the
//!   listing's status ([`status::sync_listing`]: its own
//!   Rentals logs while the `rentals` index is published, then expiry, a
//!   transfer away from the lessor and a dissolved estate); behind the
//!   write switch; the same envelope without a reader (the old crate
//!   answered the stored row unchanged, which presents stale metadata as a
//!   refresh).
//! - Rental status sync (`open -> executed | cancelled | claimed`): the
//!   `rentals` chain index ([`index`], `shop.rental_event`, the Rentals
//!   contract's own logs instead of the rentals subgraph the old crate
//!   configured and never ran), on whenever a chain-1 endpoint is configured
//!   unless `SHOP_RENTALS_INDEX_ENABLED=false`;
//!   `GET /v1/rentals-index` reports its cursor. On NFT walkers for either catalogue pair,
//!   owner pages that can include LAND return the `rentals-index` 503
//!   while this publication is absent or stale. Until it publishes, a
//!   listing keeps the status it was posted with and the rent quote
//!   (`shop::mana`) reads the nonces on chain. The leased
//!   `rental-listing-status` job ([`status`]) cancels expired open
//!   listings, and, from the land-authz index while it is published, those
//!   whose asset left the lessor or whose estate was dissolved.
//! - `GET /v1/rentals/{address}?chainId=1`: current LAND and Estate rentals
//!   from the published index for the requested chain. The tenant's list
//!   ends at expiry, and the lessor's includes unclaimed expired rentals.
//!   Sepolia (`11155111`) publishes a separate history.
//! - The rent, claim and cancel calls ([`calls`]) are pure builders the
//!   `shop::mana` quote and action routes serve over the chain readers.
//!
//! Not carried: the per-service `/ping`, `/health`, `/health/live` (the
//! content server owns `/status` and `/health` at the root),
//! `AUTH_EXPIRATION_SECONDS` (the shared verifier's window is 60 seconds
//! for every surface), `DAPPS_PG_COMPONENT_PSQL_*` (no squid),
//! `SIGNATURES_PG_CONNECTION_STRING` (the ONE pool), the `updates` table and
//! the `update_type` enum (nothing read or wrote them).
//!
//! Mount: `.merge(shop::rentals::router(state))` at the root (the old
//! `signatures-api.` vhost served its root there; `/v1/rentals-listings` and
//! `/v1/rental-listings/prices` collide with nothing) and
//! `.nest("/signatures-api", shop::rentals::router(state))` for a per-vhost
//! proxy; the create checks the signature over the request's own path first
//! and the route-local one second.

mod active;
pub mod calls;
pub mod db;
pub mod handlers;
pub mod http;
pub mod index;
pub mod nft;
mod nft_graph;
pub mod notices;
pub mod signature;
pub mod status;
pub mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::shared::routing::{erase, get, patch, post};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use sqlx::PgPool;

use crate::auth::eip1654::Eip1654Validator;
use crate::market::MarketplaceState;
use crate::shared::signed_fetch::{self, PayloadPolicy, SignedFetchError};
use crate::shared::writes::DataWrites;

use http::ApiError;

/// The old crate's body cap for the creation JSON.
pub const MAX_BODY_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct Settings {
    /// `DATA_WRITES_ENABLED`.
    pub writes_enabled: bool,
}

pub struct State {
    pub settings: Settings,
    pub db: db::Database,
    graph_cache: nft_graph::Cache,
    /// The served network and marketplace rollout settings.
    market: Option<Arc<MarketplaceState>>,
    eip1654: Option<Arc<dyn Eip1654Validator>>,
}

pub type SharedState = Arc<State>;

pub use crate::shared::signed_fetch::SignedPath;

pub(crate) fn signed_path(original: &axum::http::Uri, local: impl Into<String>) -> SignedPath {
    SignedPath::at(original, local)
}
/// The 401 bodies the old service emitted: the malformed-chain and the
/// invalid-signature details survive, every other refusal is its `Display`.
pub fn wire_message(err: &SignedFetchError) -> String {
    match err {
        SignedFetchError::MalformedChain { detail } => format!("Invalid Auth Chain: {detail}"),
        SignedFetchError::InvalidSignature(detail) => format!("Invalid signature: {detail}"),
        other => other.to_string(),
    }
}

impl State {
    pub fn new(
        pool: PgPool,
        market: Option<Arc<MarketplaceState>>,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        Self {
            settings,
            db: db::Database::new(pool),
            graph_cache: nft_graph::cache(),
            market,
            eip1654,
        }
    }

    /// Whether the asset reader is configured (the create and refresh
    /// routes' source).
    pub fn reader_configured(&self) -> bool {
        self.market
            .as_ref()
            .is_some_and(|m| Self::walks_nfts(m) || m.settings.marketplace_subgraph_url.is_some())
    }

    fn walks_nfts(market: &MarketplaceState) -> bool {
        crate::market::owned_items::walks_here(market)
    }

    fn served_chain(&self) -> Option<i64> {
        self.market
            .as_ref()
            .map(|m| m.settings.market.indexes.chain_id_ethereum)
    }

    pub(crate) async fn nft(
        &self,
        chain_id: i64,
        contract_address: &str,
        token_id: &str,
    ) -> Result<Option<nft::LandNft>, nft::NftReadError> {
        Ok(self
            .listing_asset(chain_id, contract_address, token_id, None)
            .await?
            .map(|(asset, _)| asset))
    }

    async fn listing_asset(
        &self,
        chain_id: i64,
        contract_address: &str,
        token_id: &str,
        custody_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Option<(nft::LandNft, Option<index::RentEvidence>)>, nft::NftReadError> {
        let market = self.market.as_ref().ok_or_else(|| {
            nft::NftReadError::Subgraph("no marketplace reader is configured".into())
        })?;
        if Some(chain_id) != self.served_chain() {
            return Err(nft::NftReadError::Unavailable);
        }
        let net = nft::listing_network(market.land_network(), chain_id)?;
        if !Self::walks_nfts(market) {
            let Some(asset) =
                nft_graph::read(market, &self.graph_cache, net, contract_address, token_id).await?
            else {
                return Ok(None);
            };
            let evidence = match custody_at {
                Some(now) if chain_id == 1 => Some(
                    index::rent_evidence(
                        self.db.pool(),
                        index::RentalsChain::Mainnet,
                        index::RENTALS_MAINNET,
                        contract_address,
                        token_id,
                        now,
                    )
                    .await?,
                ),
                _ => None,
            };
            return Ok(Some((asset, evidence)));
        }
        nft::nft_by_contract_token(
            self.db.pool(),
            net,
            chain_id,
            contract_address,
            token_id,
            custody_at,
        )
        .await
    }

    async fn verify(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &SignedPath,
    ) -> Result<String, SignedFetchError> {
        signed_fetch::require_signer_at(
            headers,
            method,
            path,
            PayloadPolicy::V6ThenLegacy,
            self.eip1654.as_deref(),
        )
        .await
        .map(|signer| signer.into_string())
    }

    /// A rental listing is the Marketplace's: refused while its rollout is off
    /// (`market::features`), and when no marketplace state is there to say.
    pub(crate) fn require_marketplace(&self) -> Result<(), ApiError> {
        if self
            .market
            .as_ref()
            .is_some_and(|m| m.marketplace_features())
        {
            return Ok(());
        }
        Err(ApiError::forbidden(crate::market::features::refusal()))
    }

    /// The lowercased signer of a write; every refusal is the old 401 with
    /// [`wire_message`], except a contract-wallet check the client source's
    /// budget refused (429).
    pub async fn require_signer(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &SignedPath,
    ) -> Result<String, ApiError> {
        self.verify(headers, method, path).await.map_err(|e| {
            tracing::debug!(error = %e, "auth chain verification failed on a rentals route");
            if matches!(e, SignedFetchError::RateLimited) {
                return ApiError::http(429, e.to_string());
            }
            ApiError::unauthorized(wire_message(&e))
        })
    }
}

/// The `/v1` rental-listings routes, wrapped in `DataWrites::guard` with the
/// state's `DATA_WRITES_ENABLED`.
pub fn router(state: SharedState) -> Router {
    let writes_enabled = state.settings.writes_enabled;
    let v1 = Router::new()
        .route(
            "/rentals-listings",
            post(handlers::create_rentals_listing).get(erase(handlers::get_rentals_listings)),
        )
        .route(
            "/rentals-listings/{id}",
            patch(handlers::refresh_rentals_listing),
        )
        .route(
            "/rental-listings/prices",
            get(handlers::get_rental_listings_prices),
        )
        .route("/rentals-index", get(handlers::get_rentals_index));
    let router = Router::new()
        .nest("/v1", v1)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state.clone());
    // Outside the gate, so a refused write still carries the CORS headers.
    DataWrites::guard(router, writes_enabled)
        .layer(axum::middleware::from_fn(
            crate::shared::cors::decentraland_cors,
        ))
        .merge(
            Router::new()
                .route("/v1/rentals/{address}", get(active::get_wallet_rentals))
                .layer(axum::middleware::from_fn(
                    crate::shared::cors::cors_without_credentials,
                ))
                .with_state(state),
        )
}
