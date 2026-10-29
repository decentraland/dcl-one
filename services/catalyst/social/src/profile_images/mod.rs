//! Profile images (the old `profile-images.decentraland.org`, ported as
//! `catalyrst-profile-images`): `GET /entities/{cid}/face.png` and
//! `/body.png`, the avatar thumbnails the explorer and the lambdas'
//! `snapshots` URLs point at (`PROFILE_CDN_BASE_URL` in
//! `marketplace::profiles`).
//!
//! The first source is the ONE database and blob store: a profile deployed
//! to this node carries its own `face256.png` and `body.png` content files, so
//! the image is a keyed lookup plus a blob read ([`resolver`]). Sync does not
//! fetch those files for profiles deployed elsewhere, so for them the image is
//! rendered from the avatar in the entity's metadata, as the old crate did:
//! behind `PROFILE_IMAGES_RENDER_ENABLED` ([`render`], [`queue`]) with the
//! engine's headless `avatar_render` (`PROFILE_IMAGES_RENDERER=bevy`, the
//! default) or the godot-explorer client on a throwaway X display (`godot`).
//! The proxy of an upstream profile-images service sits behind
//! `PROFILE_IMAGES_ORIGIN_URL` ([`origin`]). Both keep what they produce in
//! the ONE blob store ([`cache`], `social.profile_images`). A profile entity
//! this node holds no deployment of is filled through the content
//! read-through (`READ_THROUGH_ENABLED`) when the origin has no image, so the
//! renderer finds its avatar and an id the sources name as another type
//! answers 404. [`produce`] runs that once per image for every concurrent
//! reader, under one budget, and warms the faces of listed profiles.
//!
//! Deviation from the old `PROFILE_IMAGES_BACKEND=render|proxy|disabled`:
//! the content path is always on, and every configured source is tried in
//! order (the snapshot file, the origin, the renderer) before the answer is
//! "no image"; the old `PROFILE_IMAGES_RENDER_FALLBACK_PROXY` is implied.

pub mod cache;
pub mod images;
pub mod origin;
pub mod produce;
pub mod queue;
pub mod render;
pub mod resolver;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::bail;
use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

use crate::content::read_through::ReadThrough;
use crate::http::Outbound;
use crate::storage::Storage;

use cache::ImageCache;
use origin::Origin;
use queue::RenderQueue;
use render::{AvatarRenderer, RenderBackend, RenderSettings};

pub const DEFAULT_CACHE_DIR: &str = "/var/lib/catalyst/profile-images";
pub const DEFAULT_CACHE_TTL_SECONDS: u64 = 86_400;
pub const DEFAULT_RENDER_TIMEOUT_SECONDS: u64 = 120;

/// The environment as `Config` hands it over: `PROFILE_IMAGES_*` (the old names).
#[derive(Debug, Clone, Default)]
pub struct RawSettings {
    /// `PROFILE_IMAGES_CACHE_DIR` (empty: [`DEFAULT_CACHE_DIR`]): render scratch only.
    pub cache_dir: String,
    /// `PROFILE_IMAGES_CACHE_TTL_SECONDS` (86400; 0 = never expires).
    pub cache_ttl_seconds: u64,
    /// `PROFILE_IMAGES_CACHE_MAX_BYTES` (0 = unbounded): the images' blob budget.
    pub cache_max_bytes: u64,
    /// `PROFILE_IMAGES_ORIGIN_URL`: upstream profile-images service (proxy / render fallback).
    pub origin_url: String,
    /// `PROFILE_IMAGES_RENDER_ENABLED` (false).
    pub render_enabled: bool,
    /// `PROFILE_IMAGES_RENDERER`: `bevy` (empty) or `godot`.
    pub renderer: String,
    /// `PROFILE_IMAGES_AVATAR_RENDER_BIN`: the engine's `avatar_render`, required
    /// with the bevy renderer.
    pub avatar_render_bin: String,
    /// `PROFILE_IMAGES_GODOT_BIN`: required with the godot renderer.
    pub godot_bin: String,
    /// `PROFILE_IMAGES_GODOT_PROJECT` (empty: derived from the binary path).
    pub godot_project: String,
    /// `PROFILE_IMAGES_RENDERING_METHOD` (gl_compatibility).
    pub rendering_method: String,
    /// `PROFILE_IMAGES_RENDERING_DRIVER` (opengl3).
    pub rendering_driver: String,
    /// `PROFILE_IMAGES_DCLENV`.
    pub dclenv: String,
    /// `PROFILE_IMAGES_GODOT_HEADLESS`: refused with the godot renderer (see [`Settings::parse`]).
    pub godot_headless: bool,
    /// `PROFILE_IMAGES_GODOT_DISPLAY`.
    pub godot_display: String,
    /// `PROFILE_IMAGES_GODOT_EXTRA_ARGS`: whitespace-separated.
    pub godot_extra_args: String,
    /// `PROFILE_IMAGES_RENDER_TIMEOUT_SECONDS` (120).
    pub render_timeout_seconds: u64,
    /// `PROFILE_IMAGES_RENDER_MAX_CONCURRENT` (1).
    pub render_max_concurrent: u64,
    /// `PROFILE_IMAGES_RENDER_WORKDIR` (empty: `<cache dir>/.render-tmp`).
    pub render_workdir: String,
    /// This node's public content URL (`content.settings.content_public_url`):
    /// the renderer downloads wearables from it.
    pub content_public_url: String,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub cache_dir: PathBuf,
    pub cache_ttl_seconds: u64,
    /// None = unbounded. With a byte budget the cache drops its oldest rows
    /// to stay under it; every image is re-derivable, so an eviction costs
    /// one re-render or re-fetch.
    pub cache_max_bytes: Option<u64>,
    pub origin_url: Option<String>,
    pub render: Option<RenderSettings>,
    pub content_public_url: String,
    /// How long one image's production may take ([`produce::BUDGET`]).
    pub budget: std::time::Duration,
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

impl Settings {
    /// Whether `/profile-images` can produce a face the profile never
    /// uploaded (or this node does not hold): an origin or a renderer.
    pub fn derives_faces(&self) -> bool {
        self.origin_url.is_some() || self.render.is_some()
    }

    /// The old `Config::from_env` checks over [`RawSettings`].
    pub fn parse(raw: RawSettings) -> anyhow::Result<Self> {
        let cache_dir = non_empty(&raw.cache_dir).unwrap_or_else(|| DEFAULT_CACHE_DIR.to_string());
        let origin_url = non_empty(&raw.origin_url).map(|u| u.trim_end_matches('/').to_string());
        if let Some(url) = &origin_url
            && !crate::http::is_safe_http_url(url)
        {
            bail!("PROFILE_IMAGES_ORIGIN_URL must be a public http(s) URL");
        }
        let backend = match raw.renderer.trim() {
            "" | "bevy" => RenderBackend::Bevy,
            "godot" => RenderBackend::Godot,
            other => bail!("PROFILE_IMAGES_RENDERER must be bevy or godot, not {other:?}"),
        };
        let render = if raw.render_enabled {
            let (bin, work_root) = match backend {
                RenderBackend::Bevy => {
                    let Some(bin) = non_empty(&raw.avatar_render_bin) else {
                        bail!(
                            "PROFILE_IMAGES_RENDER_ENABLED=true with the bevy renderer requires \
                             PROFILE_IMAGES_AVATAR_RENDER_BIN (the engine's avatar_render, \
                             packages.batch-tools)"
                        );
                    };
                    (bin, None)
                }
                RenderBackend::Godot => {
                    let Some(godot_bin) = non_empty(&raw.godot_bin) else {
                        bail!(
                            "PROFILE_IMAGES_RENDERER=godot requires PROFILE_IMAGES_GODOT_BIN \
                             (path to decentraland.godot.client.x86_64)"
                        );
                    };
                    if raw.godot_headless {
                        bail!(
                            "PROFILE_IMAGES_GODOT_HEADLESS is incompatible with the renderer: godot's \
                             --headless selects a dummy rendering server that cannot produce an image, \
                             with or without a display. Use the godot-explorer package's \
                             decentraland-godot-client-xvfb binary, which brings its own."
                        );
                    }
                    let work_root = match non_empty(&raw.godot_project) {
                        Some(p) => p,
                        None => std::path::Path::new(&godot_bin)
                            .parent()
                            .and_then(|p| p.parent())
                            .map(|p| p.to_string_lossy().into_owned())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "could not derive PROFILE_IMAGES_GODOT_PROJECT from the godot bin path"
                                )
                            })?,
                    };
                    (godot_bin, Some(work_root))
                }
            };
            Some(RenderSettings {
                backend,
                bin,
                work_root,
                rendering_method: non_empty(&raw.rendering_method)
                    .unwrap_or_else(|| "gl_compatibility".to_string()),
                rendering_driver: non_empty(&raw.rendering_driver)
                    .unwrap_or_else(|| "opengl3".to_string()),
                dclenv: non_empty(&raw.dclenv),
                display: non_empty(&raw.godot_display),
                extra_args: raw
                    .godot_extra_args
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
                timeout_seconds: if raw.render_timeout_seconds == 0 {
                    DEFAULT_RENDER_TIMEOUT_SECONDS
                } else {
                    raw.render_timeout_seconds
                },
                max_concurrent: raw.render_max_concurrent.max(1) as usize,
                workdir_root: non_empty(&raw.render_workdir)
                    .unwrap_or_else(|| format!("{cache_dir}/.render-tmp")),
            })
        } else {
            None
        };
        Ok(Self {
            cache_dir: PathBuf::from(cache_dir),
            cache_ttl_seconds: raw.cache_ttl_seconds,
            cache_max_bytes: (raw.cache_max_bytes > 0).then_some(raw.cache_max_bytes),
            origin_url,
            render,
            content_public_url: raw.content_public_url.trim_end_matches('/').to_string(),
            budget: produce::BUDGET,
        })
    }

    pub fn for_tests(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            cache_ttl_seconds: 0,
            cache_max_bytes: None,
            origin_url: None,
            render: None,
            content_public_url: "http://127.0.0.1:5210/content".to_string(),
            budget: produce::BUDGET,
        }
    }
}

pub struct ProfileImagesState {
    pub pool: PgPool,
    pub storage: Arc<Storage>,
    pub settings: Settings,
    pub cache: ImageCache,
    pub render_queue: Option<RenderQueue>,
    pub origin: Option<Origin>,
    /// The content read-through that fills a profile entity this node holds
    /// no deployment of.
    pub read_through: Option<Arc<ReadThrough>>,
    pub producer: produce::Producer,
}

pub type SharedState = Arc<ProfileImagesState>;

impl ProfileImagesState {
    pub fn new(pool: PgPool, storage: Arc<Storage>, http: Outbound, settings: Settings) -> Self {
        let cache = ImageCache::new(
            pool.clone(),
            storage.clone(),
            settings.cache_ttl_seconds,
            settings.cache_max_bytes,
        );
        let render_queue = settings.render.clone().map(|rc| {
            RenderQueue::new(
                cache.clone(),
                pool.clone(),
                AvatarRenderer::new(rc.clone()),
                settings.content_public_url.clone(),
                rc.max_concurrent,
                rc.workdir_root.clone(),
            )
        });
        let origin = settings
            .origin_url
            .clone()
            .map(|url| Origin::new(http, url));
        let producer = produce::Producer::new(settings.budget);
        Self {
            pool,
            storage,
            settings,
            cache,
            render_queue,
            origin,
            read_through: None,
            producer,
        }
    }

    pub fn with_read_through(mut self, read_through: Option<Arc<ReadThrough>>) -> Self {
        self.read_through = read_through;
        self
    }

    /// `content` | `content+render` | `content+proxy` | `content+render+proxy`, for logs and status.
    pub fn sources_label(&self) -> String {
        let mut parts = vec!["content"];
        if self.render_queue.is_some() {
            parts.push("render");
        }
        if self.origin.is_some() {
            parts.push("proxy");
        }
        parts.join("+")
    }
}

/// The two image routes, served at the root (the `profile-images.` vhost)
/// and under the old gateway prefix `/profile-images` by the caller.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/entities/{entity}/face.png", get(images::face))
        .route("/entities/{entity}/body.png", get(images::body))
        .layer(axum::middleware::from_fn(
            crate::shared::nul_guard::nul_guard_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::shared::cors::cors_middleware,
        ))
        .with_state(state)
}

#[cfg(test)]
mod tests;
