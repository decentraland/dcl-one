//! Builder: the pre-publish draft store behind `builder-api.decentraland.org`
//! (the old `catalyrst-builder` member of the `catalyrst-create` bundle):
//! item and collection drafts, uploaded content, the committee's item
//! curation, standard collection publication (Polygon factory parameters
//! prepared here, signed and submitted by the client, the receipt verified
//! here) and linked (third-party) publication (frozen snapshot, stable
//! nonce, the owner-signed provider-slot cheque, the Foundation review),
//! plus the asset-packs catalogue, scene projects (upstream's Project,
//! Manifest and per-project AssetPack routes, and the content-addressed
//! file set `dcl-scene-cli sync` mirrors: [`projects`], [`project_packs`])
//! and the newsletter opt-in. Schema `builder` (`schemas/50_create.sql`).
//!
//! What it reuses: the ONE pool; the ONE `http::Outbound` client for the
//! content bucket pull-through and HEAD probes, the Polygon RPC and the
//! newsletter forward; the ONE blob store (`storage`) for uploaded item
//! files and asset-pack files (both hash-addressed: the old
//! `uploaded_contents.bytes` column and the LRU pull cache are gone); signed
//! fetch through `src/auth` and the shared verifier the explore and social
//! routes use (`shared::signed_fetch`, [`auth`]); the marketplace subgraph
//! readers (`marketplace::graph`) where the old code read the squid
//! ([`onchain`]); `shared::cache::TtlMap` for the in-process memos. The one
//! client it does not share is the Foundation review's
//! (`linked_publication_review::foundation_client`: guarded as the shared
//! one, but no redirects; the reason is in that module). Every write goes behind
//! `CREATE_WRITES_ENABLED` ([`writes`], default false, 503 with
//! `Retry-After`, built the way `social::writes` gates the social writes).
//!
//! Authorization contracts, kept exactly: owner = `collections.eth_address`
//! / `items.eth_address` equals the signer on every draft, upload and
//! publication write (the rows are lowercase by CHECK, the signer is
//! lowercased by the verifier); a non-owner's collection read and the
//! curation routes need the admin bearer (`CATALYRST_BUILDER_ADMIN_TOKEN`,
//! constant-time, fails closed) or a `BUILDER_ADMIN_ADDRESSES` signer; the
//! on-chain address reads stay anonymous (the old crate's recorded
//! divergence); chain writes are prepared here and signed and submitted by
//! the client, the receipt verified over the Polygon RPC; the linked cheque
//! is verified only; v2 never holds a signing key.
//!
//! Deviations from the old crate, all deliberate: the signed-fetch window is
//! the shared verifier's 5 minutes (the old required path allowed 30); the
//! subgraph readers replace the squid side pool (503 on the address routes
//! and empty curation listings without a collections subgraph, as the old
//! node answered without its pool); `BUILDER_CONTENT_BUCKET_URL` is
//! optional (unset: an unknown storage hash answers 404 instead of the 301,
//! and `/contents/{hash}` serves the store only); `BUILDER_POLYGON_RPC_URL`
//! must be https to a public host (a plaintext verifier could be fed a fake
//! receipt); `is_approved` is the declared projection of `curation_status`
//! (`linked_publication_review::finish_linked_publication`); the item
//! contents FK replaces the per-file "upload before saving" probe (same 400).
//!
//! Not carried, by the plan: `dcl-scene-cli` and what only it consumes
//! (`dcl-scene-authoring`, `dcl-preview-tunnel`,
//! `dcl-preview-tunnel-protocol`) and `catalyrst-deploy-signer` (a local
//! one-shot tool). Left out with the port: the old `ts-rs` bindings export
//! (the sites generate their types from the old crate until the web tier
//! moves), and the per-service `/ping` at the root (it answers under the
//! `/builder-api` prefix; the content server owns the root).
//!
//! Mount: `serve` merges [`root_router`] at the root (the old
//! `builder-api.` vhost paths, minus `/contents/{hash}`, which the content
//! server owns there) and nests [`router`] under `/builder-api` (the old
//! gateway prefix, stripped by nginx before it reached the bundle). The
//! operator commands are [`catalog::build`] (`catalyst builder catalog
//! <packs-dir>`) and [`catalog::seed`].

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
pub(crate) use catalyst_content as content;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_shared as shared;
pub(crate) use catalyst_storage as storage;

pub mod auth;
pub mod catalog;
pub mod drafts;
pub mod handlers;
pub mod http;
pub mod items;
pub mod linked_publication;
pub mod linked_publication_review;
pub mod linked_publication_store;
pub mod onchain;
pub mod project_packs;
pub mod projects;
pub mod publication;
pub mod publication_chain;
pub mod publication_store;
pub mod writes;

#[cfg(test)]
pub(crate) mod tests;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::handler::Handler;
use axum::routing::{get, patch, post};
use sqlx::PgPool;

use crate::chain_index::ChainIndex;
use crate::shared::cache::TtlMap;
use crate::storage::Storage;
use dcl_auth::eip1654::Eip1654Validator;
use dcl_http::Outbound;

use catalog::{CatalogStore, PullSource};
use items::{ItemsComponent, NewsletterComponent};
use onchain::CommitteeMemberOut;

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_BUILDER_ADMIN_TOKEN";
const COMMITTEE_MEMO_TTL: Duration = Duration::from_secs(300);
const EXISTS_TTL: Duration = Duration::from_secs(300);
const EXISTS_CACHE_MAX: usize = 8192;
/// One 20 MiB upload with its framing; also the cap of a project manifest
/// (the editor's composite) and of a file set (5000 paths).
const UPLOAD_BODY_LIMIT: usize = 21 * 1024 * 1024;

/// The newsletter SaaS forward (`NEWSLETTER_SERVICE_URL`,
/// `NEWSLETTER_PUBLICATION_ID`, `NEWSLETTER_SERVICE_API_KEY`), configured
/// only when all three are set.
#[derive(Clone, Debug)]
pub struct NewsletterForward {
    pub url: String,
    pub publication_id: String,
    pub api_key: String,
}

/// The environment as `Config` hands it over (the old env names).
#[derive(Clone, Debug, Default)]
pub struct RawSettings {
    /// `CREATE_WRITES_ENABLED` (false).
    pub writes_enabled: bool,
    /// `BUILDER_CONTENT_BUCKET_URL`: the item content bucket the storage
    /// route redirects to and `/contents/{hash}` pulls through from.
    pub content_bucket_url: Option<String>,
    /// `BUILDER_ASSET_PACKS_BUCKET_URL`: the asset-packs bucket
    /// `/contents/{hash}` pulls from when the item bucket misses.
    pub asset_packs_bucket_url: Option<String>,
    /// `BUILDER_POLYGON_RPC_URL`: the Polygon JSON-RPC that verifies
    /// publication receipts; unset refuses begin/claim/verify with 503.
    pub polygon_rpc_url: Option<String>,
    /// `BUILDER_ADMIN_ADDRESSES`: comma-separated admin wallets.
    pub admin_addresses: String,
    /// `CATALYRST_BUILDER_ADMIN_TOKEN`: bearer of the curation routes.
    pub admin_token: Option<String>,
    pub newsletter_service_url: Option<String>,
    pub newsletter_publication_id: Option<String>,
    pub newsletter_api_key: Option<String>,
    /// `BUILDER_FOUNDATION_URL`: the Foundation builder API a linked
    /// publication is verified against; unset answers 503 there.
    pub foundation_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub writes_enabled: bool,
    pub content_bucket_url: Option<String>,
    pub asset_packs_bucket_url: Option<String>,
    pub polygon_rpc_url: Option<String>,
    pub admin_addresses: Vec<String>,
    pub admin_token: Option<String>,
    pub newsletter: Option<NewsletterForward>,
    pub foundation_url: Option<String>,
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Settings {
    /// Lowercased, trimmed, blanks dropped: the old `Config::from_env` rule.
    pub fn admin_addresses_from(raw: &str) -> Vec<String> {
        raw.split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// The old `Config::from_env` checks over [`RawSettings`], plus the
    /// upstream guards every configured endpoint passes in v2.
    pub fn parse(raw: RawSettings) -> anyhow::Result<Self> {
        let content_bucket_url =
            non_empty(&raw.content_bucket_url).map(|u| u.trim_end_matches('/').to_string());
        if let Some(url) = &content_bucket_url
            && !dcl_http::is_safe_http_url(url)
        {
            bail!("BUILDER_CONTENT_BUCKET_URL must be a public http(s) URL");
        }
        let asset_packs_bucket_url =
            non_empty(&raw.asset_packs_bucket_url).map(|u| u.trim_end_matches('/').to_string());
        if let Some(url) = &asset_packs_bucket_url
            && !dcl_http::is_safe_http_url(url)
        {
            bail!("BUILDER_ASSET_PACKS_BUCKET_URL must be a public http(s) URL");
        }
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
        let newsletter = match (
            non_empty(&raw.newsletter_service_url),
            non_empty(&raw.newsletter_publication_id),
            non_empty(&raw.newsletter_api_key),
        ) {
            (Some(url), Some(publication_id), Some(api_key)) => {
                if !dcl_http::is_safe_http_url(&url) {
                    bail!("NEWSLETTER_SERVICE_URL must be a public http(s) URL");
                }
                Some(NewsletterForward {
                    url,
                    publication_id,
                    api_key,
                })
            }
            _ => None,
        };
        let foundation_url =
            non_empty(&raw.foundation_url).map(|u| u.trim_end_matches('/').to_string());
        if let Some(url) = &foundation_url
            && !dcl_http::is_safe_http_url(url)
        {
            bail!("BUILDER_FOUNDATION_URL must be a public http(s) URL");
        }
        Ok(Self {
            writes_enabled: raw.writes_enabled,
            content_bucket_url,
            asset_packs_bucket_url,
            polygon_rpc_url,
            admin_addresses: Self::admin_addresses_from(&raw.admin_addresses),
            admin_token: non_empty(&raw.admin_token),
            newsletter,
            foundation_url,
        })
    }
}

pub struct State {
    pub items: ItemsComponent,
    pub newsletter: NewsletterComponent,
    pub catalog: CatalogStore,
    /// The marketplace readers the on-chain routes read; `None` (or one
    /// without a collections subgraph) answers 503 there.
    pub market: Option<Arc<ChainIndex>>,
    pub settings: Settings,
    pub http: Outbound,
    /// The guarded, no-redirect client of the Foundation review reads.
    pub foundation: Outbound,
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The content state's denylist (the same `Arc`): a hash the camera reel
    /// withheld (rejected or deleted photo) is a 404 on `/contents/{hash}`
    /// here too, not only on the content server's routes.
    pub denylist: Arc<crate::content::Denylist>,
    pub(crate) committee_memo: TtlMap<(), Arc<Vec<CommitteeMemberOut>>>,
    pub(crate) exists_memo: TtlMap<String, bool>,
}

pub type SharedState = Arc<State>;

impl State {
    pub fn new(
        pool: PgPool,
        blobs: Arc<Storage>,
        denylist: Arc<crate::content::Denylist>,
        http: Outbound,
        market: Option<Arc<ChainIndex>>,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        settings: Settings,
    ) -> anyhow::Result<Self> {
        let buckets: Vec<String> = [
            &settings.content_bucket_url,
            &settings.asset_packs_bucket_url,
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
        let pull = (!buckets.is_empty()).then(|| PullSource {
            buckets,
            http: http.clone(),
        });
        let catalog =
            CatalogStore::new(pool.clone(), blobs.clone(), pull).context("builder catalogue")?;
        Ok(Self {
            items: ItemsComponent::new(pool.clone(), blobs),
            newsletter: NewsletterComponent::new(pool),
            catalog,
            market,
            settings,
            http,
            foundation: linked_publication_review::foundation_client(),
            eip1654,
            denylist,
            committee_memo: TtlMap::new(COMMITTEE_MEMO_TTL),
            exists_memo: TtlMap::bounded(EXISTS_TTL, EXISTS_CACHE_MAX),
        })
    }

    /// What `serve` logs at startup.
    pub fn sources_label(&self) -> String {
        format!(
            "bucket={} asset_packs_bucket={} polygon_rpc={} collections_reader={} admins={} newsletter={}",
            self.settings.content_bucket_url.is_some(),
            self.settings.asset_packs_bucket_url.is_some(),
            self.settings.polygon_rpc_url.is_some(),
            onchain::configured(self.market.as_deref()),
            self.settings.admin_addresses.len(),
            self.settings.newsletter.is_some(),
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
            "/v1/collections/{id}/linked-publication",
            get(handlers::linked_publication::prepare)
                .post(handlers::linked_publication::begin)
                .patch(handlers::linked_publication::claim)
                .put(handlers::linked_publication::authorize)
                .delete(handlers::linked_publication::cancel)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/collections/{id}/linked-publication/status",
            get(handlers::linked_publication::status)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/collections/{id}/linked-publication/verify",
            post(handlers::linked_publication::verify)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/drafts/collections",
            get(handlers::drafts::get_collection_drafts)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/drafts/items",
            get(handlers::drafts::get_item_drafts)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/collections",
            get(handlers::collections::get_collections),
        )
        .route("/v1/items", get(handlers::collections::get_items))
        .route(
            "/v1/items/{id}",
            get(handlers::drafts::get_item)
                .put(handlers::drafts::put_item)
                .delete(handlers::drafts::delete_item),
        )
        .route(
            "/v1/collections/{id}/items",
            get(handlers::collections::get_collection_items),
        )
        .route(
            "/v1/collections/{id}",
            get(handlers::collections::get_collection).put(handlers::drafts::put_collection),
        )
        .route(
            "/v1/collections/curation",
            get(handlers::curation::get_curation_collections),
        )
        .route(
            "/v1/{address}/collections",
            get(handlers::collections::get_address_collections),
        )
        .route(
            "/v1/{address}/items",
            get(handlers::collections::get_address_items),
        )
        .route(
            "/v1/storage/contents/{hash}",
            get(handlers::storage::get_storage_content)
                .head(handlers::storage::get_storage_content),
        )
        .route(
            "/v1/storage/contents/{hash}/exists",
            get(handlers::storage::head_storage_content_exists),
        )
        .route("/v1/assetPacks", get(handlers::catalog::get_asset_packs))
        .route(
            "/v1/projects",
            get(handlers::projects::list)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/projects/{id}",
            get(handlers::projects::get)
                .put(handlers::projects::put)
                .delete(handlers::projects::delete)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/projects/{id}/public",
            get(handlers::projects::get_public),
        )
        .route(
            "/v1/projects/{id}/coords",
            axum::routing::delete(handlers::projects::delete_coords),
        )
        .route(
            "/v1/projects/{id}/manifest",
            get(handlers::projects::get_manifest)
                .put(
                    handlers::projects::put_manifest
                        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
                )
                .delete(handlers::projects::delete_manifest)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/manifests",
            get(handlers::projects::get_manifests)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/projects/{id}/files",
            get(handlers::projects::get_files)
                .put(handlers::projects::put_files.layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)))
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/projects/{id}/blobs/{hash}",
            axum::routing::put(handlers::projects::put_blob)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/v1/projects/{id}/assetPacks",
            get(handlers::projects::get_asset_packs)
                .layer(axum::middleware::map_response(http::private_response)),
        )
        .route(
            "/v1/projects/{id}/assetPacks/{pack}",
            axum::routing::put(handlers::projects::put_asset_pack)
                .delete(handlers::projects::delete_asset_pack),
        )
        .route(
            "/v1/newsletter",
            post(handlers::newsletter::post_newsletter),
        )
        .route(
            "/v1/collections/{id}/items/{item}/status",
            patch(handlers::curation::patch_item_status),
        )
        .route(
            "/v1/collections/{id}/items/status",
            patch(handlers::curation::patch_items_status_bulk),
        )
}

fn finish(routes: Router<SharedState>, state: SharedState) -> Router {
    let writes_enabled = state.settings.writes_enabled;
    let router = routes
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state);
    writes::guard(router, writes_enabled)
}

/// The whole builder surface, for the `/builder-api` nest: the API routes,
/// `/ping` and `/contents/{hash}` (the asset-pack files).
pub fn router(state: SharedState) -> Router {
    finish(
        api_routes()
            .route("/ping", get(handlers::ping))
            .route("/contents/{hash}", get(handlers::catalog::get_content)),
        state,
    )
}

/// The routes safe to merge at the root beside the content server (which
/// owns `/contents/{hashId}` and the root liveness route there).
pub fn root_router(state: SharedState) -> Router {
    finish(api_routes(), state)
}

#[cfg(test)]
impl State {
    /// A copy of this state pulling `/contents/{hash}` misses from `bucket`
    /// (a loopback stub) through the test client.
    pub(crate) async fn with_bucket(&self, bucket: String) -> SharedState {
        self.with_buckets(vec![bucket]).await
    }

    /// As [`State::with_bucket`], trying `buckets` in order.
    pub(crate) async fn with_buckets(&self, buckets: Vec<String>) -> SharedState {
        let http = Outbound::for_tests();
        let settings = Settings {
            content_bucket_url: buckets.first().cloned(),
            asset_packs_bucket_url: buckets.get(1).cloned(),
            ..self.settings.clone()
        };
        let catalog = CatalogStore::new(
            self.items.pool().clone(),
            self.items.blobs.clone(),
            Some(PullSource {
                buckets,
                http: http.clone(),
            }),
        )
        .unwrap();
        Arc::new(State {
            items: ItemsComponent::new(self.items.pool().clone(), self.items.blobs.clone()),
            newsletter: NewsletterComponent::new(self.items.pool().clone()),
            catalog,
            market: self.market.clone(),
            settings,
            http,
            foundation: self.foundation.clone(),
            eip1654: None,
            denylist: self.denylist.clone(),
            committee_memo: TtlMap::new(COMMITTEE_MEMO_TTL),
            exists_memo: TtlMap::bounded(EXISTS_TTL, EXISTS_CACHE_MAX),
        })
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;

    #[test]
    fn settings_parse_the_old_env_rules_and_refuse_unsafe_endpoints() {
        let s = Settings::parse(RawSettings {
            admin_addresses: " 0xABC, ,0xdef ".into(),
            admin_token: Some("  ".into()),
            content_bucket_url: Some("https://builder-items.decentraland.org/".into()),
            asset_packs_bucket_url: Some("https://builder-api.decentraland.org/v1/storage/".into()),
            polygon_rpc_url: Some("https://polygon-rpc.com".into()),
            newsletter_service_url: Some("https://api.beehiiv.com/v2".into()),
            newsletter_publication_id: Some("pub_1".into()),
            newsletter_api_key: None,
            ..RawSettings::default()
        })
        .unwrap();
        assert_eq!(s.admin_addresses, vec!["0xabc", "0xdef"]);
        assert_eq!(s.admin_token, None, "a blank token is unset");
        assert_eq!(
            s.content_bucket_url.as_deref(),
            Some("https://builder-items.decentraland.org")
        );
        assert_eq!(
            s.asset_packs_bucket_url.as_deref(),
            Some("https://builder-api.decentraland.org/v1/storage")
        );
        assert!(
            s.newsletter.is_none(),
            "the forward needs all three settings"
        );
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
                content_bucket_url: Some("http://127.0.0.1:9/".into()),
                ..RawSettings::default()
            })
            .is_err()
        );
        assert!(
            Settings::parse(RawSettings {
                asset_packs_bucket_url: Some("http://127.0.0.1:9/".into()),
                ..RawSettings::default()
            })
            .is_err()
        );
        assert!(
            Settings::parse(RawSettings {
                polygon_rpc_url: Some("https://127.0.0.1/".into()),
                ..RawSettings::default()
            })
            .is_err()
        );
    }
}
