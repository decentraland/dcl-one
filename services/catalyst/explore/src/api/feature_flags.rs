// from: catalyrst/crates/catalyrst-explorer-api/src/modules/feature_flags.rs (document and route shapes; the served document is the mirrored upstream one with feature_flags_local applied, the embedded asset only the fallback before the first pass)
//! `GET /<app>.json` and `GET /flags/{name}`. Upstream serves the full flag set
//! (including `explorer-alfa-minimum-requirements`) only to the
//! `referer: https://decentraland.org` hostname strategy, so the mirror pass
//! sends that header and this node serves the full set to everyone. The
//! document is kept as received (`explore_api.feature_flags`, one row per app)
//! so a key upstream adds reaches the client; `feature_flags_local` overrides
//! individual flags.

use std::sync::Arc;

use anyhow::Context as _;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use parking_lot::RwLock;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::jobs::JobContext;

pub const MIRROR: &str = "feature-flags";
const BODY_CAP: usize = 4 << 20;
const EMBEDDED_FLAGS: &str = include_str!("feature-flags.explorer.json");

pub fn embedded_document() -> Value {
    serde_json::from_str(EMBEDDED_FLAGS).expect("feature-flags.explorer.json parses")
}

/// A flags document: an object with `flags` (name -> bool) and `variants` objects.
pub fn validate(doc: &Value) -> anyhow::Result<()> {
    let flags = doc
        .get("flags")
        .and_then(Value::as_object)
        .context("document has no `flags` object")?;
    if let Some((name, value)) = flags.iter().find(|(_, v)| !v.is_boolean()) {
        anyhow::bail!("flag {name} is not a boolean: {value}");
    }
    doc.get("variants")
        .and_then(Value::as_object)
        .context("document has no `variants` object")?;
    Ok(())
}

pub struct Snapshot {
    doc: RwLock<Arc<Value>>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            doc: RwLock::new(Arc::new(embedded_document())),
        }
    }
}

impl Snapshot {
    pub fn document(&self) -> Arc<Value> {
        self.doc.read().clone()
    }

    /// The mirrored row for `app` (the embedded asset when there is none yet)
    /// with every local override applied to `flags`.
    pub async fn reload(&self, pool: &PgPool, app: &str) -> sqlx::Result<()> {
        let row: Option<(Value,)> =
            sqlx::query_as("SELECT document FROM explore_api.feature_flags WHERE app = $1")
                .bind(app)
                .fetch_optional(pool)
                .await?;
        let mut doc = row.map(|(d,)| d).unwrap_or_else(embedded_document);
        let overrides: Vec<(String, bool)> = sqlx::query_as(
            "SELECT name, value FROM explore_api.feature_flags_local WHERE app = $1 ORDER BY name",
        )
        .bind(app)
        .fetch_all(pool)
        .await?;
        if !overrides.is_empty() {
            let flags = doc
                .as_object_mut()
                .map(|o| o.entry("flags").or_insert_with(|| json!({})))
                .and_then(Value::as_object_mut);
            if let Some(flags) = flags {
                for (name, value) in overrides {
                    flags.insert(name, Value::Bool(value));
                }
            }
        }
        *self.doc.write() = Arc::new(doc);
        Ok(())
    }
}

pub async fn set_local_flag(pool: &PgPool, app: &str, name: &str, value: bool) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.feature_flags_local (app, name, value)
         VALUES ($1, $2, $3)
         ON CONFLICT (app, name) DO UPDATE
         SET value = EXCLUDED.value, updated_at = timezone('UTC', now())",
    )
    .bind(app)
    .bind(name)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_local_flag(pool: &PgPool, app: &str, name: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM explore_api.feature_flags_local WHERE app = $1 AND name = $2")
        .bind(app)
        .bind(name)
        .execute(pool)
        .await?;
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

/// One pass: the document with the referer strategy header, validated and
/// stored as received; `If-None-Match` from the stored etag makes an
/// unchanged upstream a 304 that only refreshes `fetched_at`.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let app = state.settings.feature_flags_app.as_str();
        let url = state.settings.sources.feature_flags.as_str();
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
        state
            .flags
            .reload(&state.pool, app)
            .await
            .context("reloading the feature flags")?;
        let doc = state.flags.document();
        let count = |key: &str| doc.get(key).and_then(Value::as_object).map_or(0, |o| o.len()) as i64;
        Ok(mirror::PassOutcome {
            items: count("flags") + count("variants"),
            cursor: fetched.etag.or(etag),
        })
    })
    .await
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
