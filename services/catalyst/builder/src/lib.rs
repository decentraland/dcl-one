//! Builder: the pre-publish draft store behind `builder-api.decentraland.org`:
//! item and collection drafts, their listings and the committee's
//! collection listing, uploaded item files and videos, standard collection
//! publication (Polygon factory parameters prepared here, signed and
//! submitted by the client, the receipt verified here), the small public
//! reads (rarities, committee, addresses, item contents, stored files) and
//! the newsletter opt-in. It keeps no scenes: no projects, manifests, asset
//! packs or scene files. Schema `builder` (`schemas/50_create.sql`).
//!
//! What it reuses: the ONE pool; the ONE `http::Outbound` client for the
//! Polygon RPC, which verifies publication receipts, reads the rarities'
//! MANA prices ([`rarities`]) and tells whether a collection's address holds
//! its contract ([`collections::deployment`]); the ONE blob store
//! (`storage`) for uploaded item files, by hash (an item's video streams
//! there, [`items::video`]), which it serves itself (who may read a file is
//! [`access::read_file`]); signed fetch through `src/auth` and the shared
//! verifier the explore and social routes use (`shared::signed_fetch`,
//! [`auth`]); the shop's copy of the Polygon collections subgraph, read by
//! shared SQL, for a collection's published and approved state
//! ([`chain_state`]: 503 while the copy can't tell) and the chain facts a
//! collection's body and listings merge ([`collections`]). The one client
//! it does not share is [`guarded_client`]: guarded as the shared one, but
//! it follows no redirect, so a credential it carries never reaches another
//! host; the terms-of-service forward to the warehouse ([`terms`]), the
//! newsletter forward, the Slack notices and the forum take it. Every write goes behind
//! `CREATE_WRITES_ENABLED` ([`writes`], default false, 503 with
//! `Retry-After`, built the way `social::writes` gates the social writes).
//!
//! Authorization: [`access`] decides every read and write of a collection or
//! item (owner, on-chain manager, current committee member, third-party
//! manager, as builder-server defines them; 503 when a source it needs can't
//! tell), and the store functions take the grant it returns. The committee is
//! [`committee`]'s read of the Polygon collections subgraph. Chain writes
//! are prepared here and signed and submitted by the client, the receipt
//! verified over the Polygon RPC; the node never holds a signing key. The
//! signed-fetch window is the shared verifier's 60 seconds, and
//! `BUILDER_POLYGON_RPC_URL` must be https to a public host, so a plaintext
//! verifier can't be fed a fake receipt.
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
pub mod cheques;
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
pub mod publication;
pub mod publication_chain;
pub mod publication_store;
pub mod rarities;
pub mod terms;
pub mod third_parties;
pub mod writes;

#[cfg(test)]
pub(crate) mod tests;

use std::sync::Arc;

use anyhow::{Context as _, bail};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::chain_index::ChainIndex;
use crate::storage::Storage;
use dcl_auth::eip1654::Eip1654Validator;
use dcl_http::{HttpClientCfg, Outbound};

use items::ItemsComponent;

/// One 20 MiB upload with its framing.
pub const UPLOAD_BODY_LIMIT: usize = 21 * 1024 * 1024;
/// A request body on a route that takes none of the larger ones below.
pub const BODY_LIMIT: usize = 64 * 1024;
/// An item save: ten times the largest item document builder-server holds.
pub const ITEM_BODY_LIMIT: usize = 256 * 1024;
/// A collection save (its `data`), a terms acceptance (its hashes) and a
/// publication (its item ids).
pub const LARGE_BODY_LIMIT: usize = 2 * 1024 * 1024;
/// File uploads the node reads at once, each up to [`UPLOAD_BODY_LIMIT`]
/// in memory; of them per wallet (the `/create` dApp sends one item's files
/// at a time) and per source address.
pub const FILE_UPLOADS: usize = 16;
pub const FILE_UPLOADS_PER_SIGNER: usize = 4;
pub const FILE_UPLOADS_PER_SOURCE: usize = 8;

/// The newsletter SaaS forward (`NEWSLETTER_SERVICE_URL`,
/// `NEWSLETTER_PUBLICATION_ID`, `NEWSLETTER_SERVICE_API_KEY`), configured
/// only when all three are set.
#[derive(Clone)]
pub struct NewsletterForward {
    pub url: String,
    pub publication_id: String,
    pub api_key: String,
}

impl std::fmt::Debug for NewsletterForward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewsletterForward")
            .field("url", &self.url)
            .field("publication_id", &self.publication_id)
            .finish_non_exhaustive()
    }
}

/// The warehouse a terms-of-service acceptance is forwarded to
/// (`WAREHOUSE_URL`, `WAREHOUSE_TOKEN` and the optional
/// `WAREHOUSE_CONTEXT_PREFIX`), configured only when the URL and the token
/// are set.
#[derive(Clone)]
pub struct WarehouseForward {
    pub url: String,
    pub token: String,
    pub context_prefix: Option<String>,
}

impl std::fmt::Debug for WarehouseForward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarehouseForward")
            .field("context_prefix", &self.context_prefix)
            .finish_non_exhaustive()
    }
}

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
    pub newsletter: Option<NewsletterForward>,
    pub warehouse: Option<WarehouseForward>,
    /// The curators' Slack channel; none sends no notice.
    pub slack: Option<curation::slack::Slack>,
    pub links: curation::slack::Links,
    /// The curators' forum; none posts nothing and answers 503.
    pub forum: Option<curation::forum::Forum>,
    /// Where a forum post's thumbnails link to.
    pub builder_server_url: String,
    /// Who may read the answers from another origin.
    pub cors: cors::Cors,
    /// The node's bound of unpublished upload bytes, if not the default.
    pub unpublished_max_bytes: Option<u64>,
    /// The node's bound of unpublished upload files, if not the default.
    pub unpublished_max_files: Option<u64>,
    /// The video uploads the node reads at once, if not the default.
    pub video_uploads_max: Option<usize>,
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
            .field("newsletter", &self.newsletter)
            .field("warehouse", &self.warehouse)
            .field("slack", &self.slack)
            .field("links", &self.links)
            .field("forum", &self.forum)
            .field("builder_server_url", &self.builder_server_url)
            .field("cors", &self.cors)
            .field("unpublished_max_bytes", &self.unpublished_max_bytes)
            .field("unpublished_max_files", &self.unpublished_max_files)
            .field("video_uploads_max", &self.video_uploads_max)
            .finish()
    }
}

/// The most video uploads `BUILDER_VIDEO_UPLOADS_MAX` may set: 64 GB of spool.
pub const MAX_VIDEO_UPLOADS_SETTING: usize = 256;

/// The settings of one integration, all set or none; one set without the
/// others refuses to start, naming the first missing.
fn all_or_none<const N: usize>(
    named: [(&str, &Option<String>); N],
) -> anyhow::Result<Option<[String; N]>> {
    let values = named.map(|(name, value)| (name, non_empty(value)));
    if values.iter().all(|(_, value)| value.is_none()) {
        return Ok(None);
    }
    if let Some((missing, _)) = values.iter().find(|(_, value)| value.is_none()) {
        let set: Vec<&str> = named.iter().map(|(name, _)| *name).collect();
        bail!("{} are set together; {missing} is unset", set.join(", "));
    }
    Ok(Some(values.map(|(_, value)| value.unwrap_or_default())))
}

/// Refuses a URL that carries a token unless it is a public https URL.
fn token_url(name: &str, url: &str) -> anyhow::Result<()> {
    if !url.starts_with("https://") || !dcl_http::is_safe_http_url(url) {
        bail!("{name} must be a public https URL: it carries a token");
    }
    Ok(())
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Settings {
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
        let newsletter = all_or_none([
            ("NEWSLETTER_SERVICE_URL", &raw.newsletter_service_url),
            ("NEWSLETTER_PUBLICATION_ID", &raw.newsletter_publication_id),
            ("NEWSLETTER_SERVICE_API_KEY", &raw.newsletter_api_key),
        ])?
        .map(|[url, publication_id, api_key]| {
            token_url("NEWSLETTER_SERVICE_URL", &url)?;
            if publication_id.len() > 128
                || !publication_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                bail!("NEWSLETTER_PUBLICATION_ID must be letters, digits, '_' or '-'");
            }
            Ok(NewsletterForward {
                url,
                publication_id,
                api_key,
            })
        })
        .transpose()?;
        let warehouse = all_or_none([
            ("WAREHOUSE_URL", &raw.warehouse_url),
            ("WAREHOUSE_TOKEN", &raw.warehouse_token),
        ])?
        .map(|[url, token]| {
            token_url("WAREHOUSE_URL", &url).map(|()| WarehouseForward {
                url,
                token,
                context_prefix: non_empty(&raw.warehouse_context_prefix),
            })
        })
        .transpose()?;
        let slack = match (
            non_empty(&raw.slack_bot_token),
            non_empty(&raw.slack_curation_channel),
        ) {
            (Some(token), Some(channel)) => Some(curation::slack::Slack {
                token,
                channel,
                api: curation::slack::API.to_string(),
            }),
            (None, Some(_)) => {
                bail!(
                    "SLACK_CURATION_CHANNEL is set without SLACK_BOT_TOKEN, the bot that posts to it"
                )
            }
            _ => None,
        };
        let links = curation::slack::Links {
            creators_url: non_empty(&raw.creators_url),
            builder_url: non_empty(&raw.builder_url),
        };
        let forum = match all_or_none([
            ("FORUM_URL", &raw.forum_url),
            ("FORUM_API_KEY", &raw.forum_api_key),
            ("FORUM_API_USERNAME", &raw.forum_api_username),
            ("FORUM_CATEGORY", &raw.forum_category),
        ])? {
            Some([url, api_key, api_username, category]) => curation::forum::Forum::parse(
                Some(url),
                Some(api_key),
                Some(api_username),
                Some(category),
            )?,
            None => None,
        };
        let builder_server_url = non_empty(&raw.builder_server_url);
        if forum.is_some() && builder_server_url.is_none() {
            bail!(
                "FORUM_URL is set without BUILDER_SERVER_URL, the origin of the thumbnails a topic shows"
            );
        }
        for (name, url) in [
            ("CREATORS_URL", &links.creators_url),
            ("BUILDER_URL", &links.builder_url),
            ("BUILDER_SERVER_URL", &builder_server_url),
        ] {
            if let Some(url) = url
                && !url::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
            {
                bail!("{name} must be an http(s) URL");
            }
        }
        let cors = cors::Cors::parse(raw.cors_origin.as_deref(), raw.cors_method.as_deref())?;
        if raw
            .video_uploads_max
            .is_some_and(|n| !(1..=MAX_VIDEO_UPLOADS_SETTING).contains(&n))
        {
            bail!("BUILDER_VIDEO_UPLOADS_MAX must be 1 to {MAX_VIDEO_UPLOADS_SETTING}");
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
            video_uploads_max: raw.video_uploads_max,
            slack,
            links,
            forum,
            builder_server_url: builder_server_url.unwrap_or_default(),
            writes_enabled: raw.writes_enabled,
            polygon_rpc_url,
            newsletter,
            warehouse,
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
    /// [`guarded_client`]: the warehouse forward, the newsletter forward,
    /// the Slack notices and the forum, whose requests carry a credential.
    pub guarded: Outbound,
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The content state's denylist (the same `Arc`): a hash the camera reel
    /// withheld (rejected or deleted photo) is a 404 on the storage routes
    /// here too, not only on the content server's routes.
    pub denylist: Arc<crate::content::Denylist>,
    /// The rarities and their prices (`rarities`), kept a minute.
    pub rarities: rarities::Rarities,
    /// The third-party registry (`third_parties`), read per decision.
    pub third_parties: third_parties::ThirdParties,
    /// The curation committee (`committee`), read per decision.
    pub committee: committee::Committee,
    /// The node's clock, which a collection's lock is set and compared by.
    pub now: fn() -> chrono::DateTime<chrono::Utc>,
    /// What the Polygon RPC says of collection contracts, kept per contract.
    pub deployments: collections::deployment::Deployments,
    /// The curators' Slack notices.
    pub notices: curation::slack::Notices,
    /// The content sync's state: whether this process is its own content
    /// authority, so an item URN its content lacks is unused.
    pub content_sync: Arc<crate::content::SyncStatus>,
    /// The item video uploads' bounds and those in flight.
    pub videos: items::video::Videos,
    /// The item file uploads in flight, each holding its body in memory:
    /// [`FILE_UPLOADS`] on the node, [`FILE_UPLOADS_PER_SIGNER`] of a wallet,
    /// [`FILE_UPLOADS_PER_SOURCE`] of a source address.
    pub file_uploads: items::places::Places,
    /// The longest a file upload's body may take to arrive
    /// ([`items::upload::DEADLINE`]).
    pub file_deadline: std::time::Duration,
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
    /// `blobs` is the node's blob store; the builder keeps its item files in
    /// its [`Storage::item_files`] tree, which the content routes never read.
    pub fn new(
        pool: PgPool,
        blobs: Arc<Storage>,
        denylist: Arc<crate::content::Denylist>,
        http: Outbound,
        market: Option<Arc<ChainIndex>>,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        settings: Settings,
    ) -> anyhow::Result<Self> {
        let notices = curation::slack::Notices::new(settings.slack.clone(), settings.links.clone());
        let blobs = blobs
            .item_files()
            .cloned()
            .context("the builder keeps its files in the blob store's item-files tree")?;
        let mut limits = limits::Limits::default();
        limits.node_bytes = settings.unpublished_max_bytes.unwrap_or(limits.node_bytes);
        limits.node_files = settings.unpublished_max_files.unwrap_or(limits.node_files);
        let mut videos = items::video::Videos::default();
        if let Some(places) = settings.video_uploads_max {
            videos.places = items::places::Places::new(
                "video uploads",
                places,
                items::video::MAX_UPLOADS_PER_SIGNER,
                items::video::MAX_UPLOADS_PER_SOURCE,
            );
        }
        Ok(Self {
            items: ItemsComponent::new(pool.clone(), blobs)
                .with_denylist(denylist.clone())
                .with_network(settings.network),
            market,
            settings,
            http,
            guarded: guarded_client(),
            eip1654,
            denylist,
            rarities: rarities::Rarities::default(),
            third_parties: third_parties::ThirdParties::default(),
            committee: committee::Committee::default(),
            now: chrono::Utc::now,
            deployments: collections::deployment::Deployments::default(),
            notices,
            content_sync: Arc::default(),
            videos,
            file_uploads: items::places::Places::new(
                "file uploads",
                FILE_UPLOADS,
                FILE_UPLOADS_PER_SIGNER,
                FILE_UPLOADS_PER_SOURCE,
            ),
            file_deadline: items::upload::DEADLINE,
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
            "polygon_chain={} polygon_rpc={} collections_reader={} newsletter={} warehouse={} slack={} forum={}",
            self.settings.network.chain_id,
            self.settings.polygon_rpc_url.is_some(),
            onchain::configured(self.market.as_deref()),
            self.settings.newsletter.is_some(),
            self.settings.warehouse.is_some(),
            self.settings.slack.is_some(),
            self.settings.forum.is_some(),
        )
    }
}

fn api_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/v1/items/{id}/files",
            post(handlers::drafts::post_item_files).layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/v1/items/{id}/videos",
            post(handlers::drafts::post_item_videos)
                .layer(DefaultBodyLimit::max(items::video::BODY_LIMIT)),
        )
        .route(
            "/v1/collections/{id}/publication",
            get(handlers::drafts::get_publication)
                .post(handlers::publication::begin)
                .put(handlers::publication::transaction)
                .delete(handlers::publication::cancel)
                .patch(handlers::publication::claim),
        )
        .route(
            "/v1/collections/{id}/publication/status",
            get(handlers::publication::status),
        )
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
                .delete(handlers::lifecycle::delete)
                .layer(DefaultBodyLimit::max(LARGE_BODY_LIMIT)),
        )
        .route("/v1/collections/{id}/lock", post(handlers::lifecycle::lock))
        .route(
            "/v1/collections/{id}/owner/sync",
            post(handlers::lifecycle::owner_sync),
        )
        .route(
            "/v1/collections/{id}/tos",
            post(handlers::lifecycle::terms).layer(DefaultBodyLimit::max(LARGE_BODY_LIMIT)),
        )
        .route(
            "/v1/collections/{id}/publish",
            post(handlers::lifecycle::publish).layer(DefaultBodyLimit::max(LARGE_BODY_LIMIT)),
        )
        .route(
            "/v1/collections/{id}/approvalData",
            get(handlers::lifecycle::approval_data),
        )
        .route(
            "/v1/collections/{id}/curation/post",
            post(handlers::forum::reply),
        )
        .route("/v1/collections/{id}/post", post(handlers::forum::post))
        .route(
            "/v1/{address}/collections",
            get(handlers::collections::get_address_collections),
        )
        .route(
            "/v1/{address}/items",
            get(handlers::collections::get_address_items),
        )
        .route("/v1/thirdParties", get(handlers::third_parties::list))
        .route(
            "/v1/thirdParties/{id}",
            get(handlers::third_parties::get)
                .delete(handlers::third_parties::delete)
                .patch(handlers::third_parties::update),
        )
        .route(
            "/v1/thirdParties/{id}/slots",
            get(handlers::third_parties::slots),
        )
        .route_layer(axum::middleware::map_response(http::private_response))
        .route(
            "/v1/collections",
            get(handlers::collections::get_collections),
        )
        .route("/v1/committee", get(handlers::collections::get_committee))
        .route("/v1/info", get(handlers::small_reads::get_info))
        .route("/v1/rarities", get(handlers::small_reads::get_rarities))
        .route(
            "/v1/rarities/{name}",
            get(handlers::small_reads::get_rarity),
        )
        .route(
            "/v1/published-collections/{address}/items/{id}/utility",
            get(handlers::small_reads::get_item_utility),
        )
        .route(
            "/v1/items/{id}/{item}/contents",
            get(handlers::small_reads::get_item_contents),
        )
        .route("/v1/addresses", get(handlers::small_reads::get_addresses))
        .route(
            "/v1/storage/contents/{hash}",
            get(handlers::storage::get_storage_content)
                .head(handlers::storage::get_storage_content),
        )
        .route(
            "/v1/storage/contents/{hash}/exists",
            get(handlers::storage::get_storage_content_exists),
        )
        .route(
            "/v1/newsletter",
            post(handlers::newsletter::post_newsletter),
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
            polygon_rpc_url: Some("https://polygon-rpc.com".into()),
            ..RawSettings::default()
        })
        .unwrap();
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
    fn an_integration_set_in_part_or_carrying_its_token_in_clear_refuses_to_start() {
        let some = |v: &str| Some(v.to_string());
        for (raw, named) in [
            (
                RawSettings {
                    newsletter_service_url: some("https://newsletter.example/v2"),
                    newsletter_publication_id: some("pub_1"),
                    ..RawSettings::default()
                },
                "NEWSLETTER_SERVICE_API_KEY",
            ),
            (
                RawSettings {
                    warehouse_url: some("https://warehouse.example/events"),
                    ..RawSettings::default()
                },
                "WAREHOUSE_TOKEN",
            ),
            (
                RawSettings {
                    slack_curation_channel: some("curation"),
                    ..RawSettings::default()
                },
                "SLACK_BOT_TOKEN",
            ),
            (
                RawSettings {
                    forum_url: some("https://forum.example"),
                    forum_api_key: some("sk-example123"),
                    ..RawSettings::default()
                },
                "FORUM_API_USERNAME",
            ),
            (
                RawSettings {
                    warehouse_url: some("http://warehouse.example/events"),
                    warehouse_token: some("sk-example123"),
                    ..RawSettings::default()
                },
                "WAREHOUSE_URL",
            ),
            (
                RawSettings {
                    newsletter_service_url: some("http://newsletter.example/v2"),
                    newsletter_publication_id: some("pub_1"),
                    newsletter_api_key: some("sk-example123"),
                    ..RawSettings::default()
                },
                "NEWSLETTER_SERVICE_URL",
            ),
            (
                RawSettings {
                    newsletter_service_url: some("https://newsletter.example/v2"),
                    newsletter_publication_id: some("pub_1/../admin"),
                    newsletter_api_key: some("sk-example123"),
                    ..RawSettings::default()
                },
                "NEWSLETTER_PUBLICATION_ID",
            ),
            (
                RawSettings {
                    forum_url: some("http://forum.example"),
                    forum_api_key: some("sk-example123"),
                    forum_api_username: some("builder"),
                    forum_category: some("12"),
                    builder_server_url: some("https://builder-api.example"),
                    ..RawSettings::default()
                },
                "FORUM_URL",
            ),
            (
                RawSettings {
                    forum_url: some("https://forum.example"),
                    forum_api_key: some("sk-example123"),
                    forum_api_username: some("builder"),
                    forum_category: some("12"),
                    ..RawSettings::default()
                },
                "BUILDER_SERVER_URL",
            ),
        ] {
            let err = Settings::parse(raw).unwrap_err().to_string();
            assert!(
                err.contains(named) && !err.contains("sk-example123"),
                "{named}: {err}"
            );
        }
    }

    #[tokio::test]
    async fn the_video_places_setting_sizes_the_node_and_refuses_what_it_cannot_hold() {
        for refused in [0, MAX_VIDEO_UPLOADS_SETTING + 1] {
            let err = Settings::parse(RawSettings {
                video_uploads_max: Some(refused),
                ..RawSettings::default()
            })
            .unwrap_err()
            .to_string();
            assert!(err.contains("BUILDER_VIDEO_UPLOADS_MAX"), "{err}");
        }
        let settings = Settings::parse(RawSettings {
            video_uploads_max: Some(2),
            ..RawSettings::default()
        })
        .unwrap();
        let state = State::new(
            crate::tests::support::lazy_pool(),
            crate::tests::support::temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            None,
            None,
            settings,
        )
        .unwrap();
        let mut held: Vec<_> = (0..2)
            .map(|n| state.videos.admit(&format!("0xwallet{n}"), None, None).ok())
            .collect();
        for upload in held.iter_mut().flatten() {
            upload.place().arrived(1_000_000).unwrap();
        }
        assert!(held.iter().all(Option::is_some));
        assert!(state.videos.admit("0xwallet2", None, None).is_err());
    }

    #[test]
    fn no_secret_reaches_the_settings_printout() {
        let settings = Settings::parse(RawSettings {
            polygon_rpc_url: Some("https://rpc.example/v3/sk-example-rpc".into()),
            newsletter_service_url: Some("https://newsletter.example/v2".into()),
            newsletter_publication_id: Some("pub-example".into()),
            newsletter_api_key: Some("sk-example-newsletter".into()),
            warehouse_url: Some("https://warehouse.example/?key=sk-example-warehouse-url".into()),
            warehouse_token: Some("sk-example-warehouse".into()),
            slack_bot_token: Some("xoxb-example-slack".into()),
            slack_curation_channel: Some("curation".into()),
            forum_url: Some("https://forum.example".into()),
            forum_api_key: Some("sk-example-forum".into()),
            forum_api_username: Some("builder".into()),
            forum_category: Some("7".into()),
            builder_server_url: Some("https://builder-api.example".into()),
            ..RawSettings::default()
        })
        .unwrap();
        let printed = format!("{settings:?}");
        for secret in [
            "sk-example-rpc",
            "sk-example-newsletter",
            "sk-example-warehouse",
            "xoxb-example-slack",
            "sk-example-forum",
        ] {
            assert!(!printed.contains(secret), "{secret} in {printed}");
        }
    }

    #[test]
    fn the_warehouse_needs_its_url_and_token_and_a_public_url() {
        let warehouse = |url: &str, token: Option<&str>, prefix: Option<&str>| {
            Settings::parse(RawSettings {
                warehouse_url: Some(url.into()),
                warehouse_token: token.map(str::to_string),
                warehouse_context_prefix: prefix.map(str::to_string),
                ..RawSettings::default()
            })
        };
        let forward = warehouse(
            "https://warehouse.example/events",
            Some("sk-example123"),
            Some(" prd "),
        )
        .unwrap()
        .warehouse
        .unwrap();
        assert_eq!(
            (
                forward.url.as_str(),
                forward.token.as_str(),
                forward.context_prefix.as_deref()
            ),
            (
                "https://warehouse.example/events",
                "sk-example123",
                Some("prd")
            )
        );
        assert!(warehouse("https://warehouse.example/events", None, None).is_err());
        assert!(warehouse("https://warehouse.example/events", Some(" "), None).is_err());
        assert!(
            warehouse(
                "https://warehouse.example/events",
                Some("sk-example123"),
                Some("")
            )
            .unwrap()
            .warehouse
            .unwrap()
            .context_prefix
            .is_none()
        );
        let settings = warehouse(
            "https://warehouse.example/events?key=sk-example456",
            Some("sk-example123"),
            None,
        )
        .unwrap();
        let printed = format!("{settings:?}");
        assert!(
            !printed.contains("sk-example123") && !printed.contains("sk-example456"),
            "{printed}"
        );
        let err = warehouse("http://127.0.0.1/events", Some("sk-example123"), None)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("WAREHOUSE_URL") && !err.contains("sk-example123"),
            "{err}"
        );
    }
}
