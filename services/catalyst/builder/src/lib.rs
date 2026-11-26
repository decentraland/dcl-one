//! Builder: the pre-publish draft store behind `builder-api.decentraland.org`:
//! item and collection drafts, uploaded content, the committee's item
//! curation, standard collection publication (Polygon factory parameters
//! prepared here, signed and submitted by the client, the receipt verified
//! here), third-party publication (the items a provider manager submits for
//! curation with its signed slot cheque, [`cheques`]), the wearable
//! validator's review requests, which it pulls ([`validator`]), the review
//! flow creators, curators and the validator share ([`review`]), and the
//! newsletter opt-in. It keeps no scenes: no projects, manifests,
//! asset packs or scene files. Schema `builder` (`schemas/50_create.sql`).
//!
//! What it reuses: the ONE pool; the ONE `http::Outbound` client for the
//! Polygon RPC; the ONE blob store (`storage`) for uploaded item files, by
//! hash (an item's video streams there, [`items::video`]), which it serves
//! itself (who may read a file is [`access::read_file`]); signed fetch
//! through `src/auth` and the shared verifier the explore and social routes
//! use (`shared::signed_fetch`, [`auth`]); the marketplace subgraph readers
//! (`marketplace::graph`) for the curation console's listing ([`onchain`]);
//! the shop's copy of the Polygon collections subgraph, read by shared SQL,
//! for a collection's published and approved state ([`chain_state`]: 503
//! while the copy can't tell) and the chain facts a collection's body and
//! listings merge ([`collections`]). The one client it does not share is
//! [`guarded_client`]: guarded as the shared one, but it follows no
//! redirect, so a credential it carries never reaches another host; the
//! terms-of-service forward to the warehouse ([`terms`]), the newsletter
//! forward, the Slack notices and the forum take it. Every write goes behind
//! `CREATE_WRITES_ENABLED` ([`writes`], default false, 503 with
//! `Retry-After`, built the way `social::writes` gates the social writes).
//!
//! Authorization: [`access`] decides every read and write of a collection or
//! item (owner, on-chain manager, current committee member, third-party
//! manager, as builder-server defines them; 503 when a source it needs can't
//! tell), and the store functions take the grant it returns. The committee is
//! [`committee`]'s read of the Polygon collections subgraph.
//! The curation routes need the admin bearer (`CATALYRST_BUILDER_ADMIN_TOKEN`,
//! constant-time, fails closed) or a `BUILDER_ADMIN_ADDRESSES` signer; chain
//! writes are prepared here and signed and submitted by the client, the
//! receipt verified over the Polygon RPC; a slot cheque is verified only;
//! the node never holds a signing key. The signed-fetch window is the shared
//! verifier's 60 seconds, and `BUILDER_POLYGON_RPC_URL` must be https to a
//! public host, so a plaintext verifier can't be fed a fake receipt.
//!
//! Mount: `serve` merges [`root_router`] at the root (the
//! `builder-api.` host's paths) and nests [`router`] under `/builder-api`
//! (the gateway's prefix); `/ping` answers only under that prefix, because
//! the content server owns the root's.

pub(crate) use catalyst_chain_index as chain_index;
pub(crate) use catalyst_content as content;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_shared as shared;
pub(crate) use catalyst_storage as storage;

pub mod access;
pub mod auth;
pub mod chain_state;
pub mod collections;
pub mod committee;
pub mod cors;
pub mod curation;
pub mod drafts;
pub mod handlers;
pub mod http;
pub mod items;
pub mod kept;
pub mod limits;
pub mod network;
pub mod onchain;
pub mod third_parties;
pub mod writes;

#[cfg(test)]
pub(crate) mod tests;

use std::sync::Arc;

use anyhow::{Context as _, bail};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use sqlx::PgPool;

use crate::chain_index::ChainIndex;
use crate::storage::Storage;
use dcl_auth::eip1654::Eip1654Validator;
use dcl_http::{HttpClientCfg, Outbound};

use items::ItemsComponent;

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_BUILDER_ADMIN_TOKEN";
/// A request body on a route that takes none of the larger ones below.
pub const BODY_LIMIT: usize = 64 * 1024;
/// An item save: ten times the largest item document builder-server holds.
pub const ITEM_BODY_LIMIT: usize = 256 * 1024;
/// A collection save (its `data`), a terms acceptance (its hashes) and a
/// publication (its item ids).
pub const LARGE_BODY_LIMIT: usize = 2 * 1024 * 1024;
/// The environment as `Config` hands it over.
#[derive(Clone, Default)]
pub struct RawSettings {
    /// `ETH_NETWORK`: the Polygon chain it pairs is the builder's
    /// ([`network`]); unset is `mainnet`, `ETH_NETWORK`'s default.
    pub eth_network: Option<String>,
    /// `MATIC_COLLECTIONS_SUBGRAPH_URL` as set, empty included; unset reads
    /// the network's collections subgraph.
    pub matic_collections_subgraph_url: Option<String>,
    /// `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL` as set, empty included; unset
    /// reads the network's registry subgraph.
    pub third_party_registry_subgraph_url: Option<String>,
    /// `CREATE_WRITES_ENABLED` (false).
    pub writes_enabled: bool,
    /// `BUILDER_POLYGON_RPC_URL`: the Polygon JSON-RPC that verifies
    /// publication receipts and reads the rarities' MANA prices
    /// ([`rarities`]); unset refuses begin/claim/verify and the rarities
    /// routes with 503.
    pub polygon_rpc_url: Option<String>,
    /// `BUILDER_ADMIN_ADDRESSES`: comma-separated admin wallets.
    pub admin_addresses: String,
    /// `CATALYRST_BUILDER_ADMIN_TOKEN`: bearer of the curation routes.
    pub admin_token: Option<String>,
    pub newsletter_service_url: Option<String>,
    pub newsletter_publication_id: Option<String>,
    pub newsletter_api_key: Option<String>,
    pub warehouse_url: Option<String>,
    pub warehouse_token: Option<String>,
    pub warehouse_context_prefix: Option<String>,
    /// `SLACK_BOT_TOKEN`: the bot the curation notices are posted as, the
    /// one social's Slack notices use.
    pub slack_bot_token: Option<String>,
    /// `SLACK_CURATION_CHANNEL`: the curators' channel; notices are posted
    /// only when it is set, and it needs `SLACK_BOT_TOKEN`.
    pub slack_curation_channel: Option<String>,
    /// `CREATORS_URL`: the creators site a notice links to.
    pub creators_url: Option<String>,
    /// `BUILDER_URL`: the legacy builder a notice links to without
    /// `CREATORS_URL`.
    pub builder_url: Option<String>,
    /// `FORUM_URL`: the forum's origin a collection's topic is posted to.
    pub forum_url: Option<String>,
    /// `FORUM_API_KEY`: the forum API key; the forum needs all four.
    pub forum_api_key: Option<String>,
    /// `FORUM_API_USERNAME`: the forum account the node posts as.
    pub forum_api_username: Option<String>,
    /// `FORUM_CATEGORY`: the id of the category a topic is created in.
    pub forum_category: Option<String>,
    /// `BUILDER_SERVER_URL`: the public origin of these routes, which a
    /// forum post's thumbnails link into.
    pub builder_server_url: Option<String>,
    /// `WEARABLE_VALIDATOR_CALLBACK_SECRET`: the secret the wearable
    /// validator signs its calls with; unset, no validation is requested
    /// and the validator routes refuse.
    pub validator_secret: Option<String>,
    /// `CORS_ORIGIN`: the origins that may read the answers ([`cors`]).
    pub cors_origin: Option<String>,
    /// `CORS_METHOD`: the methods a preflight allows ([`cors`]).
    pub cors_method: Option<String>,
    /// `BUILDER_UNPUBLISHED_MAX_BYTES` ([`limits::NODE_BYTES`] unset).
    pub unpublished_max_bytes: Option<u64>,
    /// `BUILDER_UNPUBLISHED_MAX_FILES` ([`limits::NODE_FILES`] unset).
    pub unpublished_max_files: Option<u64>,
    /// `BUILDER_VIDEO_UPLOADS_MAX` ([`items::video::MAX_UPLOADS`] unset).
    pub video_uploads_max: Option<usize>,
}

#[derive(Clone, Default)]
pub struct Settings {
    /// The Polygon chain whose contracts, URNs and chain id the builder uses.
    pub network: network::Network,
    /// The network's collections subgraph, read while
    /// `MATIC_COLLECTIONS_SUBGRAPH_URL` is unset ([`State::collections_subgraph`]).
    pub default_collections_subgraph: Option<String>,
    /// The network's registry subgraph, read while
    /// `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL` is unset ([`State::registry_subgraph`]).
    pub default_registry_subgraph: Option<String>,
    pub writes_enabled: bool,
    pub polygon_rpc_url: Option<String>,
    pub admin_addresses: Vec<String>,
    pub admin_token: Option<String>,
    /// Who may read the answers from another origin.
    pub cors: cors::Cors,
    /// The node's bound of unpublished upload bytes, if not the default.
    pub unpublished_max_bytes: Option<u64>,
    /// The node's bound of unpublished upload files, if not the default.
    pub unpublished_max_files: Option<u64>,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("network", &self.network.chain_id)
            .field(
                "default_collections_subgraph",
                &self.default_collections_subgraph,
            )
            .field("default_registry_subgraph", &self.default_registry_subgraph)
            .field("writes_enabled", &self.writes_enabled)
            .field("polygon_rpc_url", &self.polygon_rpc_url.is_some())
            .field("admin_addresses", &self.admin_addresses)
            .field("admin_token", &self.admin_token.is_some())
            .field("cors", &self.cors)
            .field("unpublished_max_bytes", &self.unpublished_max_bytes)
            .field("unpublished_max_files", &self.unpublished_max_files)
            .finish()
    }
}

/// The shortest admin token or validator secret startup accepts.
pub const MIN_SECRET_BYTES: usize = 32;

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Settings {
    /// Lowercased, trimmed, blanks dropped.
    pub fn admin_addresses_from(raw: &str) -> Vec<String> {
        raw.split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// The settings from [`RawSettings`]; a configured value the builder
    /// can't use safely (an invalid or unsafe endpoint, a malformed id) is
    /// an error.
    pub fn parse(raw: RawSettings) -> anyhow::Result<Self> {
        let network = match raw.eth_network.as_deref() {
            None => network::POLYGON,
            Some(name) => network::for_eth_network(name).with_context(|| {
                format!("ETH_NETWORK={name} has no builder contracts (mainnet|sepolia)")
            })?,
        };
        let polygon_rpc_url = non_empty(&raw.polygon_rpc_url);
        if let Some(url) = &polygon_rpc_url {
            let parsed = url::Url::parse(url)
                .map_err(|e| anyhow::anyhow!("BUILDER_POLYGON_RPC_URL is not a valid URL: {e}"))?;
            if parsed.scheme() != "https" || !dcl_http::is_safe_http_url(url) {
                bail!(
                    "BUILDER_POLYGON_RPC_URL is {}:// to {}; publication receipts must be verified over a trusted TLS (https://) endpoint",
                    parsed.scheme(),
                    parsed.host_str().unwrap_or("?")
                );
            }
        }
        let cors = cors::Cors::parse(raw.cors_origin.as_deref(), raw.cors_method.as_deref())?;
        let admin_token = non_empty(&raw.admin_token);
        if admin_token
            .as_ref()
            .is_some_and(|s| s.len() < MIN_SECRET_BYTES)
        {
            bail!("CATALYRST_BUILDER_ADMIN_TOKEN must be at least {MIN_SECRET_BYTES} bytes");
        }
        let default_of = |set: &Option<String>, url: &str| set.is_none().then(|| url.to_string());
        Ok(Self {
            network,
            default_collections_subgraph: default_of(
                &raw.matic_collections_subgraph_url,
                network.collections_subgraph,
            ),
            default_registry_subgraph: default_of(
                &raw.third_party_registry_subgraph_url,
                network.registry_subgraph,
            ),
            cors,
            unpublished_max_bytes: raw.unpublished_max_bytes,
            unpublished_max_files: raw.unpublished_max_files,
            writes_enabled: raw.writes_enabled,
            polygon_rpc_url,
            admin_addresses: Self::admin_addresses_from(&raw.admin_addresses),
            admin_token,
        })
    }
}

pub struct State {
    pub items: ItemsComponent,
    /// The marketplace readers the on-chain routes read; `None` (or one
    /// without a collections subgraph) answers 503 there.
    pub market: Option<Arc<ChainIndex>>,
    pub settings: Settings,
    pub http: Outbound,
    /// [`guarded_client`]: the warehouse forward, the Slack notices and the
    /// forum, whose requests carry a credential.
    pub guarded: Outbound,
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The content state's denylist (the same `Arc`): a hash the camera reel
    /// withheld (rejected or deleted photo) is a 404 on the storage routes
    /// here too, not only on the content server's routes.
    pub denylist: Arc<crate::content::Denylist>,
    /// The third-party registry (`third_parties`), read per decision.
    pub third_parties: third_parties::ThirdParties,
    /// The curation committee (`committee`), read per decision.
    pub committee: committee::Committee,
    /// The node's clock, which a collection's lock is set and compared by.
    pub now: fn() -> chrono::DateTime<chrono::Utc>,
    /// What the Polygon RPC says of collection contracts, kept per contract.
    pub deployments: collections::deployment::Deployments,
    /// The content sync's state: whether this process is its own content
    /// authority, so an item URN its content lacks is unused.
    pub content_sync: Arc<crate::content::SyncStatus>,
    /// The request budgets per source, signer and route ([`limits`]).
    pub limits: limits::Limits,
}

pub type SharedState = Arc<State>;

/// The guarded client that follows no redirect, 20 s a request, built once
/// per process: the shared one follows redirects with the request's headers.
pub fn guarded_client() -> Outbound {
    Outbound::new(HttpClientCfg::default().with_total_timeout(std::time::Duration::from_secs(20)))
        .expect("the guarded client builds")
}

impl State {
    /// `blobs` is the node's blob store, which must hold the builder's
    /// [`Storage::item_files`] tree, the one the content routes never read.
    pub fn new(
        pool: PgPool,
        blobs: Arc<Storage>,
        denylist: Arc<crate::content::Denylist>,
        http: Outbound,
        market: Option<Arc<ChainIndex>>,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        settings: Settings,
    ) -> anyhow::Result<Self> {
        blobs
            .item_files()
            .context("the builder keeps its files in the blob store's item-files tree")?;
        let mut limits = limits::Limits::default();
        limits.node_bytes = settings.unpublished_max_bytes.unwrap_or(limits.node_bytes);
        limits.node_files = settings.unpublished_max_files.unwrap_or(limits.node_files);
        Ok(Self {
            items: ItemsComponent::new(pool.clone())
                .with_denylist(denylist.clone())
                .with_network(settings.network),
            market,
            settings,
            http,
            guarded: guarded_client(),
            eip1654,
            denylist,
            third_parties: third_parties::ThirdParties::default(),
            committee: committee::Committee::default(),
            now: chrono::Utc::now,
            deployments: collections::deployment::Deployments::default(),
            content_sync: Arc::default(),
            limits,
        })
    }

    /// The Polygon collections subgraph the builder reads:
    /// `MATIC_COLLECTIONS_SUBGRAPH_URL` when set, else its network's.
    pub fn collections_subgraph(&self) -> Option<String> {
        self.market
            .as_ref()
            .and_then(|m| m.settings.matic_collections_subgraph_url.clone())
            .or_else(|| self.settings.default_collections_subgraph.clone())
    }

    /// The third-party registry subgraph the builder reads:
    /// `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL` when set, else its network's.
    pub fn registry_subgraph(&self) -> Option<String> {
        self.market
            .as_ref()
            .and_then(|m| m.settings.third_party_registry_subgraph_url.clone())
            .or_else(|| self.settings.default_registry_subgraph.clone())
    }

    /// What `serve` logs at startup.
    pub fn sources_label(&self) -> String {
        format!(
            "polygon_chain={} polygon_rpc={} collections_reader={} admins={}",
            self.settings.network.chain_id,
            self.settings.polygon_rpc_url.is_some(),
            onchain::configured(self.market.as_deref()),
            self.settings.admin_addresses.len(),
        )
    }
}

fn api_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/v1/drafts/collections",
            get(handlers::drafts::get_collection_drafts),
        )
        .route("/v1/drafts/items", get(handlers::drafts::get_item_drafts))
        .route("/v1/items", get(handlers::collections::get_items))
        .route(
            "/v1/items/{id}",
            get(handlers::drafts::get_item)
                .put(handlers::drafts::put_item)
                .delete(handlers::drafts::delete_item)
                .layer(DefaultBodyLimit::max(ITEM_BODY_LIMIT)),
        )
        .route(
            "/v1/collections/{id}/items",
            get(handlers::collections::get_collection_items),
        )
        .route(
            "/v1/collections/{id}",
            get(handlers::collections::get_collection)
                .put(handlers::drafts::put_collection)
                .layer(DefaultBodyLimit::max(LARGE_BODY_LIMIT)),
        )
        .route(
            "/v1/{address}/collections",
            get(handlers::collections::get_address_collections),
        )
        .route(
            "/v1/{address}/items",
            get(handlers::collections::get_address_items),
        )
        .route_layer(axum::middleware::map_response(http::private_response))
        .route(
            "/v1/collections",
            get(handlers::collections::get_collections),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

fn finish(routes: Router<SharedState>, state: SharedState) -> Router {
    let writes_enabled = state.settings.writes_enabled;
    let cors = state.settings.cors.layer();
    let router = routes
        .route_layer(axum::middleware::map_response(http::enveloped))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            limits::per_source,
        ))
        .with_state(state);
    writes::guard(router, writes_enabled).layer(cors)
}

/// The whole builder surface, for the `/builder-api` nest: the API routes
/// and `/ping`.
pub fn router(state: SharedState) -> Router {
    finish(api_routes().route("/ping", get(handlers::ping)), state)
}

/// The routes safe to merge at the root beside the content server (which
/// owns the root liveness route there).
pub fn root_router(state: SharedState) -> Router {
    finish(api_routes(), state)
}

#[cfg(test)]
mod router_tests {
    use super::*;

    #[test]
    fn settings_parse_the_old_env_rules_and_refuse_unsafe_endpoints() {
        let s = Settings::parse(RawSettings {
            admin_addresses: " 0xABC, ,0xdef ".into(),
            admin_token: Some("  ".into()),
            polygon_rpc_url: Some("https://polygon-rpc.com".into()),
            ..RawSettings::default()
        })
        .unwrap();
        assert_eq!(s.admin_addresses, vec!["0xabc", "0xdef"]);
        assert_eq!(s.admin_token, None, "a blank token is unset");
        assert!(!s.writes_enabled);
        let err = Settings::parse(RawSettings {
            polygon_rpc_url: Some("http://rpc.example/v3/SECRET".into()),
            ..RawSettings::default()
        })
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("http:// to rpc.example") && !err.contains("SECRET"),
            "{err}"
        );
        assert!(
            Settings::parse(RawSettings {
                polygon_rpc_url: Some("https://127.0.0.1/".into()),
                ..RawSettings::default()
            })
            .is_err()
        );
    }

    #[test]
    fn no_secret_reaches_the_settings_printout() {
        let settings = Settings::parse(RawSettings {
            polygon_rpc_url: Some("https://rpc.example/v3/sk-example-rpc".into()),
            admin_token: Some("sk-example-admin-0123456789abcdef".into()),
            ..RawSettings::default()
        })
        .unwrap();
        let printed = format!("{settings:?}");
        for secret in ["sk-example-rpc", "sk-example-admin"] {
            assert!(!printed.contains(secret), "{secret} in {printed}");
        }
    }
}
