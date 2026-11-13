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
//! (`shared::signed_fetch`, the 6.x payload then the legacy one) with the
//! process's EIP-1654 validator, and `explore.place_indexed` for a world
//! name's place ids (the old HTTP hop to the places API is a local read,
//! [`db::Database::world_place_ids`]; a failed lookup is upstream's 502).
//!
//! Every write route goes behind `CREATE_WRITES_ENABLED` ([`writes`];
//! default false, 503 with `Retry-After`, built the way `social::writes`
//! gates the social writes); the reads (galleries, metadata, blobs, the
//! batch place gallery behind `POST /api/places/images`) serve either way.
//! CORS is actix-cors's as upstream configured it ([`cors`]); the query and
//! JSON body rejections are actix's plain-text 400/413 ([`extract`]); a path
//! or method no route serves is an empty 404.
//!
//! Photo events: camera-reel-service published `photo-taken` and
//! `photo-privacy-changed` to SNS; here the domains call each other in
//! process, with no broker. Each event is recorded, with the upstream body,
//! in the outbox `camera_reel.photo_events` ([`events`]) in the transaction
//! of the change it reports. The community photo counts that
//! social-service-ea derived from `photo-taken` are applied directly in the
//! upload's finalising transaction (`social::communities::ranking::record_photo`);
//! the credits program reads the outbox for its photo goals once it has one
//! (today `shop::credits` has no goals program). `photo-privacy-changed`
//! has no consumer.
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
//! Deliberate differences from camera-reel-service @ 3e8bd0b:
//! - images are addressed by blob hash, so the legacy `{uuid}-{file}` URLs
//!   (`/api/images/{uuid}-{ts}.jpg`) do not resolve; `GET /api/images/{hash}`
//!   answers 200 with the bytes unless `CAMERA_REEL_BUCKET_URL` is set (then
//!   307 there), and 404 for a hash no non-rejected row names;
//! - a refused signature is 401 JSON `{"message":"Unauthorized"}` (upstream:
//!   text/plain `Unathorized`);
//! - the shared signed-fetch verifier's policy is not upstream's
//!   (dcl-crypto-middleware-rs 0.3.0): a symmetric 5 minute timestamp window
//!   (upstream: up to 60 s old, any future one), `x-identity-metadata`
//!   optional but a JSON object when present (upstream: required, any
//!   string), and the scene signer `decentraland-kernel-scene` refused;
//! - an upload's `metadata.userAddress` matches the signer whatever its
//!   case (upstream compares it with the lowercase signer exactly), and so
//!   does the owner check of the user routes;
//! - multipart framework errors are 400 JSON `{message}` (upstream: actix's
//!   text/plain messages); the image part is capped at 5 MiB and the whole
//!   request at 25 MiB (upstream: actix-multipart's 2 MiB in-memory form
//!   limit);
//! - `HEAD` is answered on the `GET` routes, `GET /health` beside
//!   `/health/live`, and the Swagger page loads swagger-ui from unpkg.
//!
//! Not carried: `dcl-scene-cli` and what only it consumes
//! (`dcl-scene-authoring`, `dcl-preview-tunnel`,
//! `dcl-preview-tunnel-protocol`) and `catalyrst-deploy-signer`; of this
//! crate, the per-service `/metrics` (the process exporter serves the node's
//! metrics) and the places-API client with its LRU (`PLACES_API_URL`,
//! `PLACES_CACHE_*`).

// The crates this one builds on, under their module names (AGENTS.md).
#[cfg(test)]
pub(crate) use catalyst_builder as builder;
pub(crate) use catalyst_content as content;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_shared as shared;
pub(crate) use catalyst_social as social;
pub(crate) use catalyst_storage as storage;
pub(crate) use dcl_auth as auth;

pub mod admin;
pub mod cors;
pub mod db;
pub mod docs;
pub mod dto;
pub mod events;
pub mod extract;
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
    /// When set, `GET /api/images/{hash}` redirects to `<bucket_url>/<hash>`:
    /// a bucket that holds the ONE store's blobs by hash (not the legacy
    /// `BUCKET_URL` bucket, whose keys are `{uuid}-{file}`).
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

async fn not_found() -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_FOUND
}

/// axum lists the served methods in `Allow` on its method fallback; the 404
/// that fallback answers here carries no such header, as upstream's.
async fn without_allow(mut res: axum::response::Response) -> axum::response::Response {
    if res.status() == axum::http::StatusCode::NOT_FOUND {
        res.headers_mut().remove(axum::http::header::ALLOW);
    }
    res
}

/// The merged `/api` and `/admin` trees plus the executable's `/health`.
/// Each write route's methods sit behind [`writes::route`] with the state's
/// `CREATE_WRITES_ENABLED`; a path or a method no route serves is an empty
/// 404, as upstream's actix default service answers; [`cors::layer`] wraps
/// it all, refusals and 404s included.
pub fn router(state: SharedState) -> Router {
    let on = state.settings.writes_enabled;
    let gated = |route| writes::route(route, on);
    let api = Router::new()
        .route("/images", gated(post(handlers::images::upload_image)))
        .route(
            "/images-json",
            gated(post(handlers::images::upload_image_json)),
        )
        .route(
            "/images/{image_id}",
            get(handlers::images::get_image).merge(gated(delete(handlers::images::delete_image))),
        )
        .route(
            "/images/{image_id}/visibility",
            gated(patch(handlers::images::update_image_visibility)),
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
        .route("/docs/ui/", get(docs::swagger_ui))
        .route("/docs/ui/{*rest}", get(docs::swagger_ui));

    let admin = Router::new()
        .route(
            "/admin/images/{image_id}",
            gated(delete(handlers::images::admin_delete_image)),
        )
        .route(
            "/admin/images/{image_id}/review",
            gated(patch(handlers::images::admin_update_image_review)),
        );

    let router = Router::new()
        .route("/health", get(health))
        .route("/health/live", get(health))
        .nest("/api", api)
        .merge(admin)
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state);
    // Wrapped whole (a layer on `router` itself would run inside each route's
    // method dispatch, before the `Allow` header is added).
    let router = Router::new()
        .fallback_service(router)
        .layer(axum::middleware::map_response(without_allow));
    cors::layer(router)
}
