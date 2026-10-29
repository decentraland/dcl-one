//! The unavailable envelope: the source a route reads (a subgraph reader, a
//! mirror, an index) and the 503 it answers while that source is unset or
//! not ready. The shop's routes and `land_authz` answer with it.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::graph;

/// The source a marketplace route reads; what [`unavailable`] names when the
/// source is not configured or not ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A collections-subgraph reader on one network.
    CollectionsSubgraph(graph::Network),
    /// The marketplace subgraph reader (LAND, estates, names).
    MarketplaceSubgraph,
    /// The land-manager subgraph reader (parcel operators, update operators).
    LandSubgraph,
    /// A per-chain JSON-RPC reader (`TRADE_RPC_URLS`, `ETH_RPC_URL`).
    Rpc,
    /// `shop.collections`, the `collections-mirror` job.
    CollectionsMirror,
    /// `shop.items`, the `items-mirror` job.
    ItemsMirror,
    /// `shop.trades`, the trades poller.
    TradesMirror,
    /// `land_authz.*` after a completed fold.
    LandAuthzIndex,
    /// `shop.nft*` after a published fold (`nft_index`).
    NftIndex,
    /// `shop.trade_event` after a published fold (`trade_events`).
    TradeEventsIndex,
    /// The CouponManager state reader (`coupons`).
    CouponManager,
    /// A source v2 does not have yet; the label names the missing indexer.
    Deferred(&'static str),
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Self::CollectionsSubgraph(graph::Network::Ethereum) => "collections-subgraph:ethereum",
            Self::CollectionsSubgraph(graph::Network::Matic) => "collections-subgraph:matic",
            Self::MarketplaceSubgraph => "marketplace-subgraph",
            Self::LandSubgraph => "land-subgraph",
            Self::Rpc => "json-rpc",
            Self::CollectionsMirror => "collections-mirror",
            Self::ItemsMirror => "items-mirror",
            Self::TradesMirror => "trades-mirror",
            Self::LandAuthzIndex => "land-authz-index",
            Self::NftIndex => "nft-index",
            Self::TradeEventsIndex => "trade-events-index",
            Self::CouponManager => "coupon-manager",
            Self::Deferred(label) => label,
        }
    }

    /// The variable an operator sets to make the source available; `None`
    /// for a deferred source (nothing to configure yet).
    pub fn configure(self) -> Option<&'static str> {
        match self {
            Self::CollectionsSubgraph(graph::Network::Ethereum) => {
                Some("ETH_COLLECTIONS_SUBGRAPH_URL")
            }
            Self::CollectionsSubgraph(graph::Network::Matic) => {
                Some("MATIC_COLLECTIONS_SUBGRAPH_URL")
            }
            Self::MarketplaceSubgraph => Some("MARKETPLACE_SUBGRAPH_URL"),
            Self::LandSubgraph => Some("LAND_SUBGRAPH_URL"),
            Self::Rpc => Some("TRADE_RPC_URLS"),
            Self::CollectionsMirror | Self::ItemsMirror => Some("MARKETPLACE_MIRRORS_ENABLED"),
            Self::TradesMirror => Some("TRADES_SYNC_UPSTREAM_URL"),
            Self::LandAuthzIndex => Some("LAND_AUTHZ_INDEX_ENABLED"),
            Self::NftIndex => Some("MARKETPLACE_NFT_INDEX_ENABLED"),
            Self::TradeEventsIndex => Some("MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED"),
            Self::CouponManager => Some("TRADE_RPC_URLS"),
            Self::Deferred(_) => None,
        }
    }
}

/// The unavailable envelope: a route whose source is unset, whose mirror has
/// no complete pass yet, or whose data v2 does not carry, answers this 503
/// with `Retry-After` instead of an empty page. The lambdas envelope plus
/// `source` (what is missing) and `configure` (the variable, when there is one).
pub fn unavailable(source: Source, message: &str) -> Response {
    let mut body =
        json!({ "error": "Service Unavailable", "message": message, "source": source.name() });
    if let Some(var) = source.configure() {
        body["configure"] = Value::String(var.to_string());
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "60"),
        ],
        body.to_string(),
    )
        .into_response()
}
