// from: catalyrst/crates/catalyrst-explorer-api/src/modules/feature_flags.rs (document and route shapes; the served document is the mirrored upstream one with feature_flags_local applied, the embedded asset only the fallback before the first pass)
//! `GET /<app>.json` and `GET /flags/{name}`. Upstream serves the full flag set
//! (including `explorer-alfa-minimum-requirements`) only to the
//! `referer: https://decentraland.org` hostname strategy, so the mirror pass
//! sends that header and this node serves the full set to everyone. The
//! document is kept as received (`explore_api.feature_flags`, one row per app)
//! so a key upstream adds reaches the client; `feature_flags_local` overrides
//! individual flags.

use crate::shared::routing::get;
use anyhow::Context as _;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::jobs::JobContext;

use crate::shared::feature_flags::BODY_CAP;
pub use crate::shared::feature_flags::{
    Snapshot, clear_local_flag, fetch_document, fetch_document_with, set_local_flag, validate,
    with_local_overrides,
};

pub const MIRROR: &str = "feature-flags";
const EMBEDDED_FLAGS: &str = include_str!("feature-flags.explorer.json");

pub fn embedded_document() -> Value {
    serde_json::from_str(EMBEDDED_FLAGS).expect("feature-flags.explorer.json parses")
}

/// The mirrored row for `app` (the embedded asset when there is none yet)
/// with every local override applied to `flags`, into `snapshot`.
pub async fn reload(snapshot: &Snapshot, pool: &PgPool, app: &str) -> sqlx::Result<()> {
    let row: Option<(Value,)> =
        sqlx::query_as("SELECT document FROM explore_api.feature_flags WHERE app = $1")
            .bind(app)
            .fetch_optional(pool)
            .await?;
    let sourced = row.is_some();
    let doc = row.map(|(d,)| d).unwrap_or_else(|| {
        if app == "explorer" {
            embedded_document()
        } else {
            json!({"flags": {}, "variants": {}})
        }
    });
    let doc = with_local_overrides(pool, app, doc).await?;
    snapshot.replace_sourced(doc, sourced);
    Ok(())
}

pub async fn store(pool: &PgPool, app: &str, doc: &Value, etag: Option<&str>) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.feature_flags (app, document, etag, fetched_at)
         VALUES ($1, $2, $3, timezone('UTC', now()))
         ON CONFLICT (app) DO UPDATE
         SET document = EXCLUDED.document, etag = EXCLUDED.etag, fetched_at = EXCLUDED.fetched_at",
    )
    .bind(app)
    .bind(doc)
    .bind(etag)
    .execute(pool)
    .await?;
    Ok(())
}

/// One document: fetched with the referer strategy header, validated and
/// stored as received under `app`; `If-None-Match` from the stored etag makes
/// an unchanged upstream a 304 that only refreshes `fetched_at`. Answers the
/// stored document's etag.
async fn mirror_document(state: &Shared, app: &str, url: &str) -> anyhow::Result<Option<String>> {
    let stored: Option<(Option<String>,)> =
        sqlx::query_as("SELECT etag FROM explore_api.feature_flags WHERE app = $1")
            .bind(app)
            .fetch_optional(&state.pool)
            .await
            .context("reading the stored feature flags etag")?;
    let etag = stored.and_then(|(e,)| e);
    let mut headers: Vec<(&str, &str)> = vec![
        ("referer", "https://decentraland.org"),
        ("x-debug", "false"),
    ];
    if let Some(etag) = etag.as_deref() {
        headers.push(("if-none-match", etag));
    }
    let fetched = mirror::fetch(
        &state.http,
        &state.settings,
        reqwest::Method::GET,
        url,
        None,
        &headers,
        BODY_CAP,
    )
    .await?;
    if fetched.status == reqwest::StatusCode::NOT_MODIFIED {
        sqlx::query("UPDATE explore_api.feature_flags SET fetched_at = timezone('UTC', now()) WHERE app = $1")
            .bind(app)
            .execute(&state.pool)
            .await
            .context("touching the feature flags row")?;
    } else {
        let doc: Value = serde_json::from_slice(&fetched.body)
            .with_context(|| format!("{}: body is not JSON", mirror::shown(url)))?;
        validate(&doc).with_context(|| format!("{}: not a flags document", mirror::shown(url)))?;
        store(&state.pool, app, &doc, fetched.etag.as_deref())
            .await
            .context("storing explore_api.feature_flags")?;
    }
    Ok(fetched.etag.or(etag))
}

/// Refresh a separate application's document through the same outbound client,
/// bounded body reader and persisted flag store as Explorer. A failed fetch or
/// validation leaves both the persisted and process snapshots unchanged.
pub async fn refresh_app(
    state: &Shared,
    snapshot: &Snapshot,
    app: &str,
    url: &str,
    referer: &str,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    let target = state.settings.guarded_url(url)?;
    let doc = fetch_document(&state.http, target, referer, timeout).await?;
    store(&state.pool, app, &doc, None).await?;
    reload(snapshot, &state.pool, app).await?;
    Ok(())
}

/// One pass of the explorer document, served by this mirror's routes.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let app = state.settings.feature_flags_app.as_str();
        let url = state.settings.sources.feature_flags.as_str();
        let etag = mirror_document(state, app, url).await?;
        reload(&state.flags, &state.pool, app)
            .await
            .context("reloading the feature flags")?;
        Ok(mirror::PassOutcome {
            items: count_entries(&state.flags.document()),
            cursor: etag,
        })
    })
    .await
}

/// The job that keeps the `events` document `events::foundation` reads.
pub const EVENTS_JOB: &str = "events-foundation-flags";
const EVENTS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// The site the `events` flags are asked for (the service answers by Referer).
const EVENTS_REFERER: &str = "https://decentraland.org";

/// One read of `<EVENTS_FEATURE_FLAGS_URL>/events.json`, stored as the `events`
/// row. The service answers the Foundation variant only to the decentraland.org
/// Referer; `X-Address-Hash` is added when a user id is set. A failure leaves
/// the stored row as it was.
pub async fn refresh_events(state: &Shared) -> anyhow::Result<()> {
    let app = crate::events::foundation::APP;
    let url = format!("{}/{app}.json", state.settings.events_flags_url);
    let target = state
        .settings
        .guarded_url(&url)
        .context("EVENTS_FEATURE_FLAGS_URL")?;
    let mut headers = vec![("referer", EVENTS_REFERER)];
    if let Some(id) = state.settings.events_flags_user_id.as_deref() {
        headers.push(("x-address-hash", id));
    }
    let doc = fetch_document_with(&state.http, target, &headers, EVENTS_TIMEOUT).await?;
    store(&state.pool, app, &doc, None)
        .await
        .context("storing the events feature flags")?;
    Ok(())
}

fn count_entries(doc: &Value) -> i64 {
    let count = |key: &str| {
        doc.get(key)
            .and_then(Value::as_object)
            .map_or(0, |o| o.len()) as i64
    };
    count("flags") + count("variants")
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/flags/{name}", get(get_flag))
        .route("/{app_name}", get(get_app_json))
}

/// Any app name answers the mirrored document, as the old service did;
/// upstream's `cache-control: private,no-store` is kept.
async fn get_app_json(State(state): State<Shared>, Path(_app_name): Path<String>) -> Response {
    (
        StatusCode::OK,
        [("cache-control", "private,no-store,max-age=0")],
        Json(state.flags.document()),
    )
        .into_response()
}

async fn get_flag(State(state): State<Shared>, Path(name): Path<String>) -> Response {
    let doc = state.flags.document();
    if let Some(variant) = doc.get("variants").and_then(|v| v.get(&name)) {
        return (StatusCode::OK, Json(variant.clone())).into_response();
    }
    if let Some(flag) = doc.get("flags").and_then(|f| f.get(&name)) {
        return (StatusCode::OK, Json(flag.clone())).into_response();
    }
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "flag_not_found", "name": name })),
    )
        .into_response()
}
