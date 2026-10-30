//! Autotranslate (the old `autotranslate-server.decentraland.org`, ported as
//! `catalyrst-media`): a LibreTranslate-compatible `POST /translate` for the
//! explorer's chat and community text, single or batch, over an external
//! translation engine reached on the network ([`backend`]) with the answers
//! cached per `(backend, target_lang, sha256(text))` in
//! `social.translation_cache` ([`cache`]).
//!
//! The engine is an external runtime: LibreTranslate (`TRANSLATE_BACKEND=http`)
//! or an OpenAI-compatible chat endpoint (`llm`). Without one the route
//! answers 503 `translation backend not configured` instead of the old
//! default, which silently echoed the source text back as its "translation"
//! (`mock`, still selectable for tests and demos). The cache never expired in
//! the old service; `TRANSLATE_CACHE_TTL_DAYS` bounds it with a periodic
//! prune ([`MediaState::spawn_jobs`]), 0 keeping the old forever.
//!
//! `GET /convert` and `/media/convert` ([`convert`]) are the old crate's
//! stand-in for `metamorph-api.decentraland.org`: a proxy-and-resize of
//! public http(s) URLs through the guarded client, which the web tier's
//! image derivatives call.

pub mod backend;
pub mod cache;
pub mod convert;
pub mod translate;

use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::FromRequest;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::json;
use sqlx::PgPool;

use crate::http::Outbound;
use crate::jobs::{Jobs, PeriodicCfg};

use backend::Backend;

pub const DEFAULT_LLM_MODEL: &str = "gpt-4o-mini";
pub const DEFAULT_TRANSLATE_CHAR_LIMIT: u64 = 5000;
pub const DEFAULT_TRANSLATE_BATCH_LIMIT: u64 = 100;
pub const DEFAULT_TRANSLATE_REQUEST_TIMEOUT_SECS: u64 = 30;

/// How often the cache prune runs when `TRANSLATE_CACHE_TTL_DAYS` is set.
pub const CACHE_PRUNE_INTERVAL: Duration = Duration::from_secs(6 * 3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// No engine: `POST /translate` answers 503.
    Disabled,
    /// Echoes the text back (tests, demos).
    Mock,
    /// LibreTranslate-compatible `POST {url}/translate`.
    Http,
    /// OpenAI-compatible `POST {url}/v1/chat/completions`.
    Llm,
}

impl BackendKind {
    pub fn label(self) -> &'static str {
        match self {
            BackendKind::Disabled => "disabled",
            BackendKind::Mock => "mock",
            BackendKind::Http => "http",
            BackendKind::Llm => "llm",
        }
    }
}

/// The environment as `Config` hands it over: `TRANSLATE_*` (the old names).
#[derive(Debug, Clone, Default)]
pub struct RawSettings {
    /// `TRANSLATE_BACKEND`: disabled | mock | http | llm. Empty: http when
    /// `TRANSLATE_BACKEND_URL` is set, llm when `TRANSLATE_LLM_BASE_URL` is,
    /// else disabled.
    pub backend: String,
    /// `TRANSLATE_BACKEND_URL`
    pub backend_url: String,
    /// `TRANSLATE_BACKEND_API_KEY`
    pub backend_api_key: String,
    /// `TRANSLATE_LLM_BASE_URL`
    pub llm_base_url: String,
    /// `TRANSLATE_LLM_API_KEY`
    pub llm_api_key: String,
    /// `TRANSLATE_LLM_MODEL` (empty: [`DEFAULT_LLM_MODEL`])
    pub llm_model: String,
    /// `TRANSLATE_CHAR_LIMIT` (5000): characters per request, summed across a batch.
    pub char_limit: u64,
    /// `TRANSLATE_BATCH_LIMIT` (100): items per batch.
    pub batch_limit: u64,
    /// `TRANSLATE_REQUEST_TIMEOUT_SECS` (30): deadline over the whole engine call of one request.
    pub request_timeout_secs: u64,
    /// `TRANSLATE_CACHE_TTL_DAYS` (0 = never prune).
    pub cache_ttl_days: u64,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub backend: BackendKind,
    pub backend_url: Option<String>,
    pub backend_api_key: Option<String>,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: String,
    pub char_limit: usize,
    pub batch_limit: usize,
    pub request_timeout: Duration,
    pub cache_ttl: Option<Duration>,
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
    /// The old `Config::from_env` checks over [`RawSettings`].
    pub fn parse(raw: RawSettings) -> anyhow::Result<Self> {
        let backend_url = non_empty(&raw.backend_url).map(|u| u.trim_end_matches('/').to_string());
        let llm_base_url =
            non_empty(&raw.llm_base_url).map(|u| u.trim_end_matches('/').to_string());
        let backend = match raw.backend.trim() {
            "disabled" | "off" => BackendKind::Disabled,
            "mock" => BackendKind::Mock,
            "http" => BackendKind::Http,
            "llm" => BackendKind::Llm,
            "" if backend_url.is_some() => BackendKind::Http,
            "" if llm_base_url.is_some() => BackendKind::Llm,
            "" => BackendKind::Disabled,
            other => bail!("unknown TRANSLATE_BACKEND={other} (disabled | mock | http | llm)"),
        };
        if backend == BackendKind::Http && backend_url.is_none() {
            bail!("TRANSLATE_BACKEND=http requires TRANSLATE_BACKEND_URL");
        }
        if backend == BackendKind::Llm && llm_base_url.is_none() {
            bail!(
                "TRANSLATE_BACKEND=llm requires TRANSLATE_LLM_BASE_URL (e.g. https://llm.decent.dev)"
            );
        }
        let char_limit = if raw.char_limit == 0 {
            DEFAULT_TRANSLATE_CHAR_LIMIT
        } else {
            raw.char_limit
        };
        let batch_limit = if raw.batch_limit == 0 {
            DEFAULT_TRANSLATE_BATCH_LIMIT
        } else {
            raw.batch_limit
        };
        let timeout_secs = if raw.request_timeout_secs == 0 {
            DEFAULT_TRANSLATE_REQUEST_TIMEOUT_SECS
        } else {
            raw.request_timeout_secs
        };
        Ok(Self {
            backend,
            backend_url,
            backend_api_key: non_empty(&raw.backend_api_key),
            llm_base_url,
            llm_api_key: non_empty(&raw.llm_api_key),
            llm_model: non_empty(&raw.llm_model).unwrap_or_else(|| DEFAULT_LLM_MODEL.to_string()),
            char_limit: char_limit as usize,
            batch_limit: batch_limit as usize,
            request_timeout: Duration::from_secs(timeout_secs),
            cache_ttl: (raw.cache_ttl_days > 0)
                .then(|| Duration::from_secs(raw.cache_ttl_days * 86_400)),
        })
    }

    pub fn for_tests(backend: BackendKind) -> Self {
        Self {
            backend,
            backend_url: None,
            backend_api_key: None,
            llm_base_url: None,
            llm_api_key: None,
            llm_model: DEFAULT_LLM_MODEL.to_string(),
            char_limit: DEFAULT_TRANSLATE_CHAR_LIMIT as usize,
            batch_limit: DEFAULT_TRANSLATE_BATCH_LIMIT as usize,
            request_timeout: Duration::from_secs(DEFAULT_TRANSLATE_REQUEST_TIMEOUT_SECS),
            cache_ttl: None,
        }
    }
}

pub struct MediaState {
    pub pool: PgPool,
    pub settings: Settings,
    pub backend: Backend,
    /// The guarded client `/convert` fetches user-named URLs through.
    pub http: Outbound,
    pub convert_cache: crate::shared::cache::TtlMap<String, convert::Converted>,
    pub convert_slots: convert::Slots,
    pub convert_sources: ConvertSources,
}

/// Where `/convert` reads a source from. The default reads nothing: every
/// URL is refused.
#[derive(Clone, Default)]
pub struct ConvertSources {
    /// Any public URL, through the guarded client
    /// (`MEDIA_CONVERT_REMOTE_SOURCES`, set by the mirror posture alone).
    pub remote: bool,
    /// This node's own origin (`PUBLIC_URL`): `<own>/content/contents/<hash>`
    /// is read from `storage`, whatever `remote` says, and never fetched.
    pub own_url: Option<url::Url>,
    pub storage: Option<Arc<crate::storage::Storage>>,
    /// `READ_THROUGH_ENABLED`: a content file the store lacks is filled
    /// (hash-verified, kept) from `SYNC_SOURCE` before any host is asked.
    pub read_through: Option<Arc<crate::content::read_through::ReadThrough>>,
}

impl ConvertSources {
    /// The content hash a `.../content/contents/<cid>` URL names on any
    /// host: the same bytes wherever they are read, so this node's copy, or
    /// its sync sources', serves them.
    pub fn content_hash(source: &url::Url) -> Option<String> {
        if source.query().is_some() {
            return None;
        }
        let (dir, hash) = source.path().rsplit_once('/')?;
        (dir.ends_with("/content/contents") && crate::auth::is_canonical_cid(hash))
            .then(|| hash.to_string())
    }

    /// The blob hash a URL names under this node's `/content/contents/`.
    pub fn own_content_hash(&self, source: &url::Url) -> Option<String> {
        let own = self.own_url.as_ref()?;
        if source.origin() != own.origin() || source.query().is_some() {
            return None;
        }
        let prefix = format!("{}/content/contents/", own.path().trim_end_matches('/'));
        let hash = source.path().strip_prefix(&prefix)?;
        (!hash.is_empty() && hash.bytes().all(|b| b.is_ascii_alphanumeric()))
            .then(|| hash.to_string())
    }
}

pub type SharedState = Arc<MediaState>;

impl MediaState {
    /// The engine is operator-configured (like `ETH_RPC_URL`, a loopback
    /// LibreTranslate is a normal deployment), so it is called through the
    /// shared client as is, never a user-influenced URL.
    pub fn new(pool: PgPool, http: Outbound, settings: Settings) -> Self {
        let backend = Backend::build(&settings, http.raw().clone());
        Self {
            pool,
            settings,
            backend,
            http,
            convert_cache: convert::cache(),
            convert_slots: convert::slots(),
            convert_sources: ConvertSources::default(),
        }
    }

    /// With a state built by tests over a stub engine.
    pub fn with_backend(pool: PgPool, settings: Settings, backend: Backend) -> Self {
        Self {
            pool,
            settings,
            backend,
            http: Outbound::for_tests(),
            convert_cache: convert::cache(),
            convert_slots: convert::slots(),
            convert_sources: ConvertSources::default(),
        }
    }

    pub fn with_convert_sources(mut self, sources: ConvertSources) -> Self {
        self.convert_sources = sources;
        self
    }

    /// Tests: `/convert` thumbnails through `slots` at once, `queue` more in line.
    pub fn with_convert_slots(mut self, slots: usize, queue: usize) -> Self {
        self.convert_slots = convert::Slots::new(slots, queue);
        self
    }

    /// Tests: `/convert` through `http` (a guarded client, or one admitting a loopback stub).
    pub fn with_http(mut self, http: Outbound) -> Self {
        self.http = http;
        self
    }

    pub fn backend_label(&self) -> &'static str {
        self.settings.backend.label()
    }

    /// `translation-cache-prune`: with `TRANSLATE_CACHE_TTL_DAYS` set, deletes
    /// rows older than the TTL in bounded batches every
    /// [`CACHE_PRUNE_INTERVAL`]; the current time is a bind, never an index
    /// predicate.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let Some(ttl) = self.settings.cache_ttl else {
            tracing::info!("TRANSLATE_CACHE_TTL_DAYS=0: translation cache rows are kept forever");
            return;
        };
        let state = self.clone();
        let cfg = PeriodicCfg::every(CACHE_PRUNE_INTERVAL)
            .with_jitter(Duration::from_secs(60))
            .with_error_backoff(Duration::from_secs(6 * 3600));
        jobs.spawn_periodic_leased("translation-cache-prune", cfg, move |ctx| {
            let state = state.clone();
            async move {
                let now = chrono::Utc::now().naive_utc();
                let deleted = cache::prune_older_than(&state.pool, now, ttl, &ctx).await?;
                if deleted > 0 {
                    tracing::info!(
                        deleted,
                        ttl_days = ttl.as_secs() / 86_400,
                        "translation cache pruned"
                    );
                }
                Ok(())
            }
        });
    }
}

/// `POST /translate`, `GET /convert` and `/media/convert`, served at the
/// root (the `autotranslate-server.` vhost, nginx's `/metamorph-api` strip)
/// and under the old gateway prefix `/autotranslate-server` by the caller.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/translate", post(translate::translate))
        .route("/convert", get(convert::convert))
        .route("/media/convert", get(convert::convert))
        .layer(axum::middleware::from_fn(
            crate::shared::nul_guard::nul_guard_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::shared::cors::cors_middleware,
        ))
        .with_state(state)
}

/// `{"ok":false,"error":msg,"message":msg}` (catalyrst-types' `ApiErrorBody`,
/// which the old media routes emitted without the places `code` key).
#[derive(Debug)]
pub enum ApiError {
    Http { status: u16, message: String },
    Internal(String),
}

impl ApiError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::http(400, msg)
    }
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::http(503, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Http { status, message } => (status, message),
            ApiError::Internal(m) => {
                tracing::error!(error = %m, "internal error on a media route");
                (500, "internal error".to_string())
            }
        };
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            Json(json!({ "ok": false, "error": message, "message": message })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(format!("{e:#}"))
    }
}

/// A JSON body with the old media rejections: 400 in the envelope above for
/// a wrong content type, an unreadable or oversized body, or invalid JSON.
pub struct JsonBody<T>(pub T);

const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

impl<T, S> FromRequest<S> for JsonBody<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        req: Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_ascii_lowercase());
        let is_json = match content_type {
            Some(ct) => ct.starts_with("application/json") || ct.starts_with("application/+json"),
            None => false,
        };
        if !is_json {
            return Err(ApiError::bad_request(
                "Invalid request: expected application/json body",
            ));
        }
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|_| ApiError::bad_request("Invalid request: could not read request body"))?;
        if bytes.len() > MAX_BODY_BYTES {
            return Err(ApiError::bad_request(
                "Invalid request: request body too large",
            ));
        }
        let value = serde_json::from_slice::<T>(&bytes)
            .map_err(|e| ApiError::bad_request(format!("Invalid request: {e}")))?;
        Ok(JsonBody(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> RawSettings {
        RawSettings::default()
    }

    #[test]
    fn an_unset_backend_is_disabled_not_mock() {
        let s = Settings::parse(raw()).unwrap();
        assert_eq!(s.backend, BackendKind::Disabled);
        assert_eq!(s.char_limit, 5000);
        assert_eq!(s.batch_limit, 100);
        assert_eq!(s.request_timeout, Duration::from_secs(30));
        assert!(s.cache_ttl.is_none());
    }

    #[test]
    fn a_backend_url_selects_http_and_an_llm_url_selects_llm() {
        let s = Settings::parse(RawSettings {
            backend_url: "http://127.0.0.1:5000/".into(),
            ..raw()
        })
        .unwrap();
        assert_eq!(s.backend, BackendKind::Http);
        assert_eq!(s.backend_url.as_deref(), Some("http://127.0.0.1:5000"));
        let s = Settings::parse(RawSettings {
            llm_base_url: "https://llm.example/".into(),
            ..raw()
        })
        .unwrap();
        assert_eq!(s.backend, BackendKind::Llm);
        assert_eq!(s.llm_model, DEFAULT_LLM_MODEL);
    }

    #[test]
    fn an_explicit_kind_needs_its_url() {
        assert!(
            Settings::parse(RawSettings {
                backend: "http".into(),
                ..raw()
            })
            .is_err()
        );
        assert!(
            Settings::parse(RawSettings {
                backend: "llm".into(),
                ..raw()
            })
            .is_err()
        );
        assert!(
            Settings::parse(RawSettings {
                backend: "smoke".into(),
                ..raw()
            })
            .is_err()
        );
        assert_eq!(
            Settings::parse(RawSettings {
                backend: "mock".into(),
                ..raw()
            })
            .unwrap()
            .backend,
            BackendKind::Mock
        );
    }

    #[test]
    fn a_ttl_in_days_becomes_a_prune_budget() {
        let s = Settings::parse(RawSettings {
            cache_ttl_days: 30,
            ..raw()
        })
        .unwrap();
        assert_eq!(s.cache_ttl, Some(Duration::from_secs(30 * 86_400)));
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::bad_request("missing q parameter").into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "missing q parameter", "message": "missing q parameter" })
        );
    }
}
