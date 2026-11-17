// from: catalyrst/crates/catalyrst-explorer-api/src/modules/feature_flags.rs (document and route shapes; the served document is the mirrored upstream one with feature_flags_local applied, the embedded asset only the fallback before the first pass)
//! `GET /<app>.json` and `GET /flags/{name}`. Upstream serves the full flag set
//! (including `explorer-alfa-minimum-requirements`) only to the
//! `referer: https://decentraland.org` hostname strategy, so the mirror pass
//! sends that header and this node serves the full set to everyone. The
//! document is kept as received (`explore_api.feature_flags`, one row per app)
//! so a key upstream adds reaches the client; `feature_flags_local` overrides
//! individual flags.

use anyhow::Context as _;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::jobs::JobContext;

use crate::shared::feature_flags::BODY_CAP;
pub use crate::shared::feature_flags::{
    Snapshot, clear_local_flag, fetch_document, set_local_flag, validate, with_local_overrides,
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
    let doc = row.map(|(d,)| d).unwrap_or_else(|| {
        if app == "explorer" {
            embedded_document()
        } else {
            json!({"flags": {}, "variants": {}})
        }
    });
    let doc = with_local_overrides(pool, app, doc).await?;
    snapshot.replace(doc);
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

/// The sibling document of the same feature-flags service: the source names
/// one `<app>.json`; `events.json` lives beside it.
fn sibling_document(url: &str, app: &str) -> Option<String> {
    let (dir, file) = url.rsplit_once('/')?;
    if dir.is_empty() || !file.ends_with(".json") {
        return None;
    }
    Some(format!("{dir}/{app}.json"))
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

/// One pass: the explorer document (served by this mirror's routes) and the
/// `events` document beside it, which `events::foundation` reads for the
/// Foundation display names (upstream `refreshFoundationAddresses` fetched
/// `events.json` itself; review architecture-1 explore-change-4). Either
/// document failing fails the pass.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let app = state.settings.feature_flags_app.as_str();
        let url = state.settings.sources.feature_flags.as_str();
        let etag = mirror_document(state, app, url).await?;
        // The explorer document is served from memory: reloaded as soon as
        // it is stored, before the events sibling can fail the pass.
        reload(&state.flags, &state.pool, app)
            .await
            .context("reloading the feature flags")?;
        let mut items = count_entries(&state.flags.document());
        let events_app = crate::events::foundation::APP;
        if events_app != app
            && let Some(events_url) = sibling_document(url, events_app)
        {
            mirror_document(state, events_app, &events_url).await?;
            let (events_doc,): (Value,) =
                sqlx::query_as("SELECT document FROM explore_api.feature_flags WHERE app = $1")
                    .bind(events_app)
                    .fetch_one(&state.pool)
                    .await
                    .context("reading the stored events flags")?;
            items += count_entries(&events_doc);
        }
        Ok(mirror::PassOutcome {
            items,
            cursor: etag,
        })
    })
    .await
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

#[cfg(test)]
mod tests {
    use super::sibling_document;

    #[test]
    fn the_events_document_sits_beside_the_explorer_one() {
        assert_eq!(
            sibling_document(
                "https://feature-flags.decentraland.org/explorer.json",
                "events"
            ),
            Some("https://feature-flags.decentraland.org/events.json".into())
        );
        assert_eq!(
            sibling_document("http://127.0.0.1:9/flags/explorer.json", "events"),
            Some("http://127.0.0.1:9/flags/events.json".into())
        );
        assert_eq!(sibling_document("explorer.json", "events"), None);
        assert_eq!(sibling_document("https://x.example/flags", "events"), None);
    }
}
