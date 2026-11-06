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
    /// `COLLECTIONS_CHAIN_ID`: which book of venues the process settles on
    /// ([`settlement_book`]).
    pub collections_chain_id: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            writes_enabled: false,
            track_interval: Duration::from_secs(15),
            collections_chain_id: ChainId::POLYGON.0,
        }
    }
}

/// The venues the process settles on, chosen once from `COLLECTIONS_CHAIN_ID`:
/// Polygon (137) is the production book, unchanged; Amoy (80002) is the
/// testnet book. Any other value is refused rather than read as production.
/// The testnet book also needs a network the rest of the node agrees with:
/// `ETH_NETWORK` other than mainnet, and the trade-events index walking Amoy,
/// since a trade's verdict and a settlement's confirmation are read from the
/// `matic` index cursor, which must not be one of chain 137. On either book the
/// catalogue must name the same Polygon chain (`POLYGON_CHAIN_ID`): it is the
/// chain the Shop's pages ask these venues to settle on.
pub fn settlement_book(
    collections_chain_id: u64,
    market: &MarketplaceState,
) -> anyhow::Result<venues::Book> {
    settlement_book_for(
        collections_chain_id,
        crate::market::wire::polygon_chain_id().into(),
        market,
    )
}

fn settlement_book_for(
    collections_chain_id: u64,
    catalogue_polygon: u64,
    market: &MarketplaceState,
) -> anyhow::Result<venues::Book> {
    let rpc = |chain_id| market.venue_rpc_url(chain_id);
    if matches!(
        ChainId(collections_chain_id),
        ChainId::POLYGON | ChainId::AMOY
    ) {
        anyhow::ensure!(
            catalogue_polygon == collections_chain_id,
            "COLLECTIONS_CHAIN_ID is {collections_chain_id} but the catalogue names Polygon as chain \
             {catalogue_polygon} (POLYGON_CHAIN_ID): its items would ask to settle on a chain this book lacks"
        );
    }
    match ChainId(collections_chain_id) {
        ChainId::POLYGON => Ok(venues::Book::production(rpc)),
        ChainId::AMOY => {
            anyhow::ensure!(
                market.settings.eth_network.trim() != "mainnet",
                "COLLECTIONS_CHAIN_ID is Amoy (80002) but ETH_NETWORK is mainnet: \
                 the Shop's testnet venues need a testnet node"
            );
            let indexed = market
                .settings
                .market
                .indexes
                .chain_id(crate::market::chain_walk::ChainNet::Matic);
            anyhow::ensure!(
                indexed == ChainId::AMOY.0 as i64,
                "COLLECTIONS_CHAIN_ID is Amoy (80002) but MARKETPLACE_INDEX_CHAIN_ID_MATIC is {indexed}: \
                 an index of another chain would confirm Amoy's trades"
            );
            Ok(venues::Book::testnet(rpc))
        }
        other => anyhow::bail!(
            "COLLECTIONS_CHAIN_ID is {}: the Shop settles MANA only on Polygon (137) or Amoy (80002)",
            other.0
        ),
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
    /// The process's state: the venues book [`settlement_book`] picks over
    /// the market's per-chain endpoints (`TRADE_RPC_URLS`, `ETH_RPC_URL` for
    /// Ethereum on mainnet). Fails on a network the Shop does not settle on.
    pub fn new(
        pool: PgPool,
        http: Outbound,
        market: Arc<MarketplaceState>,
        rentals: Arc<crate::rentals::State>,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> anyhow::Result<Self> {
        let venues = settlement_book(settings.collections_chain_id, &market)?;
        Ok(Self {
            pool,
            http,
            settings,
            venues,
            market,
            rentals,
            eip1654,
        })
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

    /// The chain's reader: None when no endpoint is configured, a refusal
    /// when the endpoint serves another chain than the book's.
    pub async fn reader(
        &self,
        chain_id: ChainId,
    ) -> Result<Option<chain::Reader>, chain::EndpointRefusal> {
        let Some(url) = self
            .venues
            .chain(chain_id)
            .and_then(|c| c.rpc_url.as_deref())
        else {
            return Ok(None);
        };
        chain::verified_reader(self.http.clone(), url, chain_id.0)
            .await
            .map(Some)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::tests::support::state_without_db_settings;

    fn market(eth_network: &str, indexed_matic: i64) -> Arc<MarketplaceState> {
        let mut settings = crate::market::Settings::for_tests();
        settings.eth_network = eth_network.to_string();
        settings.market.indexes.chain_id_matic = indexed_matic;
        settings.market.trade_rpc_urls = crate::market::listings::RpcEndpoints::from_env(Some(
            "137=https://polygon.test,80002=https://amoy.test",
        ));
        state_without_db_settings(settings)
    }

    fn ids(book: &venues::Book) -> Vec<u64> {
        let mut ids: Vec<u64> = book.chains().map(|c| c.chain_id.0).collect();
        ids.sort();
        ids
    }

    #[tokio::test]
    async fn polygon_keeps_the_production_book_exactly() {
        let market = market("mainnet", 137);
        let book = settlement_book_for(137, 137, &market).unwrap();
        let production = venues::Book::production(|id| market.venue_rpc_url(id));
        assert_eq!(ids(&book), [1, 137]);
        for chain in production.chains() {
            assert_eq!(book.chain(chain.chain_id), Some(chain));
        }
        assert!(book.chain(ChainId::AMOY).is_none());
    }

    #[tokio::test]
    async fn amoy_selects_the_testnet_book_over_amoys_own_endpoint() {
        let market = market("sepolia", 80_002);
        let book = settlement_book_for(80_002, 80_002, &market).unwrap();
        assert_eq!(ids(&book), [80_002]);
        assert_eq!(
            book.chain(ChainId::AMOY).unwrap().rpc_url.as_deref(),
            Some("https://amoy.test"),
            "Amoy reads its own TRADE_RPC_URLS entry, never Polygon's"
        );
    }

    #[tokio::test]
    async fn amoy_without_its_own_endpoint_reads_nothing() {
        let mut settings = crate::market::Settings::for_tests();
        settings.eth_network = "sepolia".into();
        settings.market.indexes.chain_id_matic = 80_002;
        settings.market.trade_rpc_urls =
            crate::market::listings::RpcEndpoints::from_env(Some("137=https://polygon.test"));
        settings.polygon_rpc_url = Some("https://polygon-fallback.test".into());
        settings.eth_rpc_url = Some("https://eth.test".into());
        let market = state_without_db_settings(settings);
        let book = settlement_book_for(80_002, 80_002, &market).unwrap();
        assert_eq!(book.chain(ChainId::AMOY).unwrap().rpc_url, None);
    }

    #[tokio::test]
    async fn an_inconsistent_or_unsupported_network_is_refused_at_startup() {
        let err = settlement_book_for(80_002, 80_002, &market("mainnet", 80_002)).unwrap_err();
        assert!(err.to_string().contains("ETH_NETWORK is mainnet"), "{err}");
        let err = settlement_book_for(80_002, 80_002, &market("sepolia", 137)).unwrap_err();
        assert!(
            err.to_string()
                .contains("MARKETPLACE_INDEX_CHAIN_ID_MATIC is 137"),
            "{err}"
        );
        let err = settlement_book_for(80_002, 137, &market("sepolia", 80_002)).unwrap_err();
        assert!(err.to_string().contains("POLYGON_CHAIN_ID"), "{err}");
        let err = settlement_book_for(137, 80_002, &market("mainnet", 137)).unwrap_err();
        assert!(err.to_string().contains("POLYGON_CHAIN_ID"), "{err}");
        for other in [1, 11_155_111, 10] {
            let err = settlement_book_for(other, 137, &market("mainnet", 137)).unwrap_err();
            assert!(
                err.to_string().contains("settles MANA only on"),
                "{other}: {err}"
            );
        }
    }

    async fn rpc_serving(chain_hex: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stub = axum::Router::new().route(
            "/",
            post(
                move |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    axum::Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": chain_hex,
                    }))
                },
            ),
        );
        tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn an_endpoint_serving_another_chain_is_refused_before_any_read() {
        let http = Outbound::for_tests().allowing_private_hosts();
        let polygon = rpc_serving("0x89").await;
        assert_eq!(
            chain::verified_reader(http.clone(), &polygon, 80_002)
                .await
                .unwrap_err(),
            chain::EndpointRefusal::WrongChain {
                expected: 80_002,
                served: 137
            }
        );
        let amoy = rpc_serving("0x13882").await;
        assert!(
            chain::verified_reader(http.clone(), &amoy, 80_002)
                .await
                .is_ok()
        );
        let unreachable = "http://127.0.0.1:1";
        assert!(matches!(
            chain::verified_reader(http, unreachable, 80_002).await,
            Err(chain::EndpointRefusal::Unknown { .. })
        ));
    }
}
