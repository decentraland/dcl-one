//! Camera reel: the player photo gallery behind `camera-reel-service`:
//! uploads (multipart and base64 JSON) with a generated 640x360 thumbnail,
//! per-user galleries and counts under `MAX_IMAGES_PER_USER`, public place
//! and world galleries, the public photos of people wearing an item, owner
//! visibility and deletion, the moderator's review status and delete.
//! Schema `camera_reel` (`schemas/50_create.sql`).
//!
//! What it reuses rather than duplicates: the ONE pool ([`db`]), the ONE
//! blob store (`storage`) for the image and thumbnail bytes by
//! `auth::hash_bytes_v1` hash (the old per-service disk tree is gone; the
//! wire's `url`/`thumbnailUrl` render from the stored hashes under
//! `api_url`), signed fetch through the shared verifier
//! (`shared::signed_fetch`, the 6.x payload then the legacy one, as the old
//! crate's `accept_legacy_payload(&[])` did) with the process's EIP-1654
//! validator, and `explore.place_indexed` for a world name's place ids (the
//! old HTTP hop to the places API is a local read, [`db::Database::world_place_ids`]).
//!
//! Every write goes behind `CREATE_WRITES_ENABLED` ([`writes`]; default
//! false, 503 with `Retry-After`, built the way `social::writes` gates the
//! social writes); the reads (galleries, metadata, blobs, the batch place
//! gallery behind `POST /api/places/images`) serve either way.
//!
//! Authorization contracts kept exactly: an upload's `metadata.userAddress`
//! must be the signer (and the row's CHECK holds the pair together);
//! private rows are listed only to their owner (an unverifiable chain reads
//! as anonymous); delete and visibility are the owner's (403 for anyone
//! else, 404 for an absent row); the admin routes take the bearer
//! `CATALYRST_CAMERA_REEL_ADMIN_TOKEN` (403 unset, 401 wrong); a `rejected`
//! row answers 404 from every reel read, and its blobs answer 404 on every
//! route of the node that serves the ONE store: the reel's
//! `GET /api/images/{hash}` (from `review_status`), the content server's
//! `/contents/{hash}` at the root and under the worlds nest and the
//! builder's `/contents/{hash}` (from `camera_reel.withheld_blobs`, written
//! in the review's and the delete's transaction and mirrored into the shared
//! in-memory `content::Denylist`, refilled at boot by
//! [`State::load_withheld`]). The old service could not leak a rejected or
//! deleted photo through another route because its store was private; over
//! the ONE store the withheld record is what keeps that contract. A hash is
//! withheld only while no non-rejected row and no content file or builder
//! upload names it, so an owner's delete cannot hide a deployed file of the
//! same bytes. What stays as the old hash-as-capability rule: a private,
//! non-rejected photo is served to whoever holds its hash, by the reel route
//! as before and now by the content routes too. Two deliberate deviations:
//! a delete never unlinks a blob (the store is
//! shared with content, worlds and builder; a store-wide mark sweep is the
//! candidate), and `GET /api/images/{hash}` serves only a hash some
//! non-rejected row names (an "orphan stays servable" rule would expose any
//! content file through this route). Beside the offset pages: a keyset page
//! (`?cursor=`) over the same order, `nextCursor` on the wire only when
//! asked for.
//!
//! Mount: `.nest("/camera-reel-service", camera_reel::router(state))`, the
//! prefix the old gateway stripped (`catalyrst/nixos/web-gateway.nix`);
//! the router's own paths are `/api/...`, `/admin/...`, `/health`. It is not
//! merged at the root: `/api/places/{id}/...` is the explore places
//! surface there. A per-vhost proxy for `camera-reel-service.<domain>`
//! forwards `/x` to `/camera-reel-service/x`; a client signs the path it
//! requested, which is what the shared verifier checks
//! (`signed_fetch::candidate_paths`, the ONE path rule: the request's own
//! path, and the `x-original-path` the proxy sets when it ends with the
//! route).
//!
//! Not carried (the plan): `dcl-scene-cli` and what only it consumes
//! (`dcl-scene-authoring`, `dcl-preview-tunnel`,
//! `dcl-preview-tunnel-protocol`) and `catalyrst-deploy-signer`; of this
//! crate, the `ts` feature (ts-rs bindings), the per-service `/metrics` and
//! the places-API client with its LRU (`PLACES_API_URL`, `PLACES_CACHE_*`).

// The crates this one builds on, under their module names (AGENTS.md).
#[cfg(test)]
pub(crate) use catalyst_builder as builder;
pub(crate) use catalyst_content as content;
pub(crate) use catalyst_shared as shared;
pub(crate) use catalyst_storage as storage;
pub(crate) use dcl_auth as auth;

pub mod admin;
pub mod db;
pub mod docs;
pub mod dto;
pub mod handlers;
pub mod http;
pub mod writes;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::routing::{delete, get, patch, post};
use sqlx::PgPool;

use crate::auth::eip1654::Eip1654Validator;
use crate::content::Denylist;
use crate::shared::signed_fetch::{self, PayloadPolicy, SignedFetchError, Signer};
use crate::storage::Storage;

use http::ApiError;

pub const DEFAULT_MAX_IMAGES_PER_USER: u64 = 500;

/// The body cap, applied to the whole router.
pub const MAX_BODY_BYTES: usize = 25 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Settings {
    /// Base of the rendered `url`/`thumbnailUrl` (old `API_URL`): the public
    /// URL of this router, `<PUBLIC_URL>/camera-reel-service`.
    pub api_url: String,
    /// Old `BUCKET_URL`: when set, `GET /api/images/{hash}` redirects there.
    pub bucket_url: Option<String>,
    /// Old `MAX_IMAGES_PER_USER` (500).
    pub max_images_per_user: u64,
    /// Old `CATALYRST_CAMERA_REEL_ADMIN_TOKEN`; unset refuses the admin routes.
    pub admin_token: Option<String>,
    /// `CREATE_WRITES_ENABLED`.
    pub writes_enabled: bool,
}

impl Settings {
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into().trim_end_matches('/').to_string(),
            bucket_url: None,
            max_images_per_user: DEFAULT_MAX_IMAGES_PER_USER,
            admin_token: None,
            writes_enabled: false,
        }
    }
}

pub struct State {
    pub settings: Settings,
    pub db: db::Database,
    pub storage: Arc<Storage>,
    /// The store's denylist (`storage.denylist()`, the `Arc` the content
    /// state shares): what a rejection or a delete withholds here the store
    /// withholds on every route that reads it by hash, this module's
    /// `/api/images/{hash}` included.
    pub denylist: Arc<Denylist>,
    eip1654: Option<Arc<dyn Eip1654Validator>>,
}

pub type SharedState = Arc<State>;

pub use crate::shared::signed_fetch::SignedPath;

pub(crate) fn signed_path(original: &axum::http::Uri, local: impl Into<String>) -> SignedPath {
    SignedPath::at(original, local)
}
impl State {
    pub fn new(
        pool: PgPool,
        storage: Arc<Storage>,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        let db = db::Database::new(pool, settings.api_url.clone());
        let denylist = storage.denylist().clone();
        Self {
            settings,
            db,
            storage,
            denylist,
            eip1654,
        }
    }

    /// Fills the shared denylist from `camera_reel.withheld_blobs`; `serve`
    /// runs it once before listening, so a restart keeps every rejected and
    /// deleted photo's bytes off the content routes. Returns the count.
    pub async fn load_withheld(&self) -> Result<usize, sqlx::Error> {
        let hashes = self.db.withheld_hashes().await?;
        for hash in &hashes {
            self.denylist.add(hash);
        }
        Ok(hashes.len())
    }

    /// Mirrors what a review or delete wrote to `withheld_blobs`.
    pub(crate) fn apply_verdict(&self, verdict: &db::BlobVerdict) {
        for hash in &verdict.withheld {
            self.denylist.add(hash);
        }
        for hash in &verdict.released {
            self.denylist.remove(hash);
        }
    }

    async fn verify(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &SignedPath,
    ) -> Result<Signer, SignedFetchError> {
        signed_fetch::require_signer_at(
            headers,
            method,
            path,
            PayloadPolicy::V6ThenLegacy,
            self.eip1654.as_deref(),
        )
        .await
    }

    /// The verified signer of a write; every refusal is 401.
    pub async fn require_auth(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &SignedPath,
    ) -> Result<Signer, ApiError> {
        self.verify(headers, method, path).await.map_err(|e| {
            tracing::debug!(error = %e, "auth chain verification failed");
            ApiError::Unauthorized
        })
    }

    /// The signer of a read that widens for its owner: no chain, or one that
    /// does not verify, reads as anonymous.
    pub async fn optional_auth(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &SignedPath,
    ) -> Option<Signer> {
        headers.get(format!("{}0", signed_fetch::AUTH_CHAIN_HEADER_PREFIX))?;
        match self.verify(headers, method, path).await {
            Ok(signer) => Some(signer),
            Err(e) => {
                tracing::debug!(error = %e, "optional auth chain verification failed");
                None
            }
        }
    }

    pub fn authorize_admin(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        admin::check_admin(self.settings.admin_token.as_deref(), headers)
    }
}

async fn health() -> axum::Json<&'static str> {
    axum::Json("alive")
}

/// The merged `/api` and `/admin` trees plus the executable's `/health`,
/// wrapped in the [`writes::guard`] with the state's `CREATE_WRITES_ENABLED`.
pub fn router(state: SharedState) -> Router {
    let api = Router::new()
        .route("/images", post(handlers::images::upload_image))
        .route("/images-json", post(handlers::images::upload_image_json))
        .route(
            "/images/{image_id}",
            get(handlers::images::get_image).delete(handlers::images::delete_image),
        )
        .route(
            "/images/{image_id}/visibility",
            patch(handlers::images::update_image_visibility),
        )
        .route(
            "/images/{image_id}/metadata",
            get(handlers::images::get_metadata),
        )
        .route("/users/{user_address}", get(handlers::users::get_user_data))
        .route(
            "/users/{user_address}/images",
            get(handlers::users::get_user_images),
        )
        .route(
            "/places/{place_id}/images",
            get(handlers::places::get_place_images),
        )
        .route(
            "/places/images",
            post(handlers::places::get_multiple_places_images),
        )
        .route(
            "/wearables/{item}/images",
            get(handlers::wearables::get_wearable_images),
        )
        .route("/docs/openapi.json", get(docs::openapi_json))
        .route("/docs/ui", get(docs::swagger_ui))
        .route("/docs/ui/{*rest}", get(docs::swagger_ui));

    let admin = Router::new()
        .route(
            "/admin/images/{image_id}",
            delete(handlers::images::admin_delete_image),
        )
        .route(
            "/admin/images/{image_id}/review",
            patch(handlers::images::admin_update_image_review),
        );

    let writes_enabled = state.settings.writes_enabled;
    let router = Router::new()
        .route("/health", get(health))
        .route("/health/live", get(health))
        .nest("/api", api)
        .merge(admin)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state);
    writes::guard(router, writes_enabled)
}
