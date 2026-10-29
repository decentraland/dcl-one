//! dcl-lists: `POST /pois` (the map's points of interest, parcel coords) and
//! `POST /banned-names`, each `{"data": [...]}`. The old node seeded its
//! tables from a shell script that pulled the upstream lists and pruned what
//! disappeared; here that is the `lists` mirror pass, over `explore_api.lists`
//! with the `lists_local` overlay on top.

use anyhow::Context as _;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::jobs::JobContext;

pub const LISTS: [&str; 2] = ["pois", "banned-names"];
pub const MIRROR: &str = "lists";
const BODY_CAP: usize = 4 << 20;

#[derive(Debug, Serialize, Deserialize)]
pub struct ListResponse {
    pub data: Vec<String>,
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/pois", post(post_pois))
        .route("/banned-names", post(post_banned_names))
}

/// The served set: mirrored values plus local additions, minus local removals, sorted.
pub async fn values(pool: &PgPool, list: &str) -> sqlx::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT value FROM (
             SELECT value FROM explore_api.lists WHERE list = $1
             UNION
             SELECT value FROM explore_api.lists_local WHERE list = $1 AND present
         ) v
         WHERE value NOT IN (SELECT value FROM explore_api.lists_local WHERE list = $1 AND NOT present)
         ORDER BY value COLLATE \"C\"",
    )
    .bind(list)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(v,)| v).collect())
}

/// `present = true` adds a value the upstream lacks, `false` hides a mirrored one.
pub async fn set_local(pool: &PgPool, list: &str, value: &str, present: bool) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.lists_local (list, value, present)
         VALUES ($1, $2, $3)
         ON CONFLICT (list, value) DO UPDATE
         SET present = EXCLUDED.present, updated_at = timezone('UTC', now())",
    )
    .bind(list)
    .bind(value)
    .bind(present)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_local(pool: &PgPool, list: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM explore_api.lists_local WHERE list = $1 AND value = $2")
        .bind(list)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}

/// Makes the mirror equal to `values` in one transaction: insert the new
/// ones, delete the ones that are gone; the overlay is not touched.
pub async fn replace(pool: &PgPool, list: &str, values: &[String]) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO explore_api.lists (list, value)
         SELECT $1, v FROM unnest($2::text[]) AS v
         ON CONFLICT (list, value) DO NOTHING",
    )
    .bind(list)
    .bind(values)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM explore_api.lists WHERE list = $1 AND value <> ALL($2::text[])")
        .bind(list)
        .bind(values)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// One pass: both lists pulled whole from the upstream (they are a few
/// hundred values each), then replaced. An empty upstream answer is refused
/// rather than mirrored, since it would prune every row on a bad day upstream.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let mut items = 0i64;
        for list in LISTS {
            let url = format!("{}/{list}", state.settings.sources.lists);
            let fetched = mirror::fetch(
                &state.http,
                &state.settings,
                reqwest::Method::POST,
                &url,
                None,
                &[],
                BODY_CAP,
            )
            .await?;
            let doc: ListResponse = serde_json::from_slice(&fetched.body).with_context(|| {
                format!("{}: body is not {{\"data\": [...]}}", mirror::shown(&url))
            })?;
            let mut values = doc.data;
            values.sort();
            values.dedup();
            anyhow::ensure!(
                !values.is_empty(),
                "{url}: upstream answered an empty list; refusing to prune the mirror"
            );
            replace(&state.pool, list, &values)
                .await
                .with_context(|| format!("replacing explore_api.lists {list}"))?;
            items += values.len() as i64;
        }
        Ok(mirror::PassOutcome {
            items,
            cursor: None,
        })
    })
    .await
}

async fn serve(state: &Shared, list: &str) -> Response {
    match values(&state.pool, list).await {
        Ok(data) => Json(ListResponse { data }).into_response(),
        Err(err) => super::internal_error("reading explore_api.lists", err),
    }
}

async fn post_pois(State(state): State<Shared>) -> Response {
    serve(&state, "pois").await
}

async fn post_banned_names(State(state): State<Shared>) -> Response {
    serve(&state, "banned-names").await
}
