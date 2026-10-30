//! `GET /world-manifest.json`: the genesis `WorldManifest.json` explorers
//! resolve for every realm that is not decentraland.org itself (its
//! `occupied` set drives genesis terrain generation and gates scene loading;
//! the web client's fetch shim rewrites
//! `places-dcf8abb.s3.amazonaws.com/WorldManifest.json` to this path). This
//! realm mirrors Genesis City, so the canonical manifest is copied verbatim
//! from decentraland's places pipeline: the roads/occupied/empty split cannot
//! be derived from deployments alone.
//!
//! The old `catalyrst-world-manifest-mirror` timer curled it into the www
//! root nginx served; here the `world-manifest` pass (a periodic job, daily
//! by default) sends the held copy's validators, keeps the bytes in the ONE
//! blob store and keys them in the one-row `explore.world_manifest`. A body
//! that is not a JSON object whose `occupied`, `roads` and `empty` are arrays
//! (or absent), or that has no occupied parcel, never replaces the held copy
//! (explorers treat `occupied` as the whole city). Before the first pass the
//! route answers the unavailable envelope instead of the old bare 404.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use chrono::NaiveDateTime;
use parking_lot::RwLock;
use serde_json::Value;
use sqlx::PgPool;

use super::Shared;
use super::mirror::{self, Conditional};
use crate::jobs::JobContext;
use crate::storage::Storage;

pub const MIRROR: &str = "world-manifest";
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 3600);

const BODY_CAP: usize = 32 << 20;
const FETCH_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_AGE_SECS: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub occupied: i32,
    pub roads: i32,
    pub empty: i32,
}

/// The old jq gate: an object whose three lists are arrays or absent, with
/// at least one occupied parcel.
pub fn validate(body: &[u8]) -> anyhow::Result<Counts> {
    let doc: Value = serde_json::from_slice(body).context("manifest is not JSON")?;
    let obj = doc
        .as_object()
        .context("manifest is not a JSON object with occupied/roads/empty arrays")?;
    let len = |key: &str| -> anyhow::Result<i32> {
        match obj.get(key) {
            None | Some(Value::Null) => Ok(0),
            Some(Value::Array(a)) => Ok(i32::try_from(a.len()).unwrap_or(i32::MAX)),
            Some(_) => anyhow::bail!("manifest `{key}` is not an array"),
        }
    };
    let counts = Counts {
        occupied: len("occupied")?,
        roads: len("roads")?,
        empty: len("empty")?,
    };
    anyhow::ensure!(
        counts.occupied > 0,
        "manifest has no occupied parcels; refusing to publish"
    );
    Ok(counts)
}

pub struct Held {
    pub body: Bytes,
    pub cid: String,
    pub counts: Counts,
    pub fetched_at: NaiveDateTime,
}

#[derive(Default)]
pub struct WorldManifestState {
    current: RwLock<Option<Arc<Held>>>,
}

impl WorldManifestState {
    pub fn snapshot(&self) -> Option<Arc<Held>> {
        self.current.read().clone()
    }

    fn set(&self, held: Held) {
        *self.current.write() = Some(Arc::new(held));
    }

    /// The persisted copy, if a pass ever stored one and the store still
    /// holds its bytes (a lost blob is refetched unconditionally by the next pass).
    pub async fn load(&self, pool: &PgPool, storage: &Storage) -> anyhow::Result<()> {
        let row: Option<(String, i32, i32, i32, NaiveDateTime)> = sqlx::query_as(
            "SELECT blob_cid, occupied_count, roads_count, empty_count, fetched_at FROM explore.world_manifest WHERE id = 1",
        )
        .fetch_optional(pool)
        .await
        .context("reading explore.world_manifest")?;
        let Some((cid, occupied, roads, empty, fetched_at)) = row else {
            return Ok(());
        };
        match storage
            .retrieve(&cid)
            .await
            .context("reading the world manifest blob")?
        {
            Some(body) => self.set(Held {
                body,
                cid,
                counts: Counts {
                    occupied,
                    roads,
                    empty,
                },
                fetched_at,
            }),
            None => {
                tracing::warn!(blob = %cid, "the world manifest blob is missing from the store; the next pass fetches it again")
            }
        }
        Ok(())
    }
}

/// One pass: a conditional GET against the held copy's validators.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let counts = pass_body(state).await?;
        Ok(mirror::PassOutcome {
            items: counts.occupied as i64,
            cursor: state.world_manifest.snapshot().map(|h| h.cid.clone()),
        })
    })
    .await
}

/// etag, last_modified, occupied, roads, empty.
type HeldValidators = (Option<String>, Option<String>, i32, i32, i32);

pub async fn pass_body(state: &Shared) -> anyhow::Result<Counts> {
    let url = state.settings.world_manifest_upstream.clone();
    let row: Option<HeldValidators> =
        sqlx::query_as("SELECT etag, last_modified, occupied_count, roads_count, empty_count FROM explore.world_manifest WHERE id = 1")
            .fetch_optional(&state.pool)
            .await
            .context("reading explore.world_manifest")?;
    let held = state.world_manifest.snapshot();
    let (etag, last_modified) = match (&row, &held) {
        (Some((etag, last_modified, ..)), Some(_)) => (etag.clone(), last_modified.clone()),
        _ => (None, None),
    };
    match mirror::fetch_conditional(
        &state.http,
        &state.settings,
        &url,
        etag.as_deref(),
        last_modified.as_deref(),
        BODY_CAP,
        FETCH_TIMEOUT,
        mirror::Reach::Public,
    )
    .await?
    {
        Conditional::NotModified => {
            let (_, _, occupied, roads, empty) = row.context("304 without a held manifest")?;
            sqlx::query("UPDATE explore.world_manifest SET checked_at = timezone('UTC', now()) WHERE id = 1")
                .execute(&state.pool)
                .await
                .context("restamping explore.world_manifest")?;
            Ok(Counts {
                occupied,
                roads,
                empty,
            })
        }
        Conditional::Missing(status) => anyhow::bail!(
            "{} answered {status}; keeping the held copy",
            mirror::shown(&url)
        ),
        Conditional::Fetched {
            body,
            etag,
            last_modified,
            ..
        } => {
            let body = Bytes::from(body);
            let (counts, cid) = {
                let body = body.clone();
                tokio::task::spawn_blocking(move || {
                    (validate(&body), crate::auth::hash::hash_bytes_v1(&body))
                })
                .await
                .context("manifest verify task")?
            };
            let counts = counts.with_context(|| mirror::shown(&url))?;
            let storage = &state.content.storage;
            if !storage
                .exist(&cid)
                .await
                .context("probing the manifest blob")?
            {
                storage
                    .store(&cid, body.clone())
                    .await
                    .context("storing the manifest blob")?;
            }
            let (fetched_at,): (NaiveDateTime,) = sqlx::query_as(
                "INSERT INTO explore.world_manifest
                     (id, blob_cid, byte_size, occupied_count, roads_count, empty_count, etag, last_modified, fetched_at, checked_at)
                 VALUES (1, $1, $2, $3, $4, $5, $6, $7, timezone('UTC', now()), timezone('UTC', now()))
                 ON CONFLICT (id) DO UPDATE
                 SET blob_cid = EXCLUDED.blob_cid, byte_size = EXCLUDED.byte_size,
                     occupied_count = EXCLUDED.occupied_count, roads_count = EXCLUDED.roads_count,
                     empty_count = EXCLUDED.empty_count, etag = EXCLUDED.etag,
                     last_modified = EXCLUDED.last_modified, fetched_at = EXCLUDED.fetched_at,
                     checked_at = EXCLUDED.checked_at
                 RETURNING fetched_at",
            )
            .bind(&cid)
            .bind(body.len() as i64)
            .bind(counts.occupied)
            .bind(counts.roads)
            .bind(counts.empty)
            .bind(etag)
            .bind(last_modified)
            .fetch_one(&state.pool)
            .await
            .context("recording explore.world_manifest")?;
            tracing::info!(
                occupied = counts.occupied,
                roads = counts.roads,
                empty = counts.empty,
                bytes = body.len(),
                "world-manifest.json mirrored"
            );
            state.world_manifest.set(Held {
                body,
                cid,
                counts,
                fetched_at,
            });
            Ok(counts)
        }
    }
}

fn http_date(t: chrono::DateTime<chrono::Utc>) -> String {
    t.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

/// `GET /world-manifest.json`: the held bytes, `max-age` one day (the old
/// `expires 1d`), the blob id as the ETag.
pub async fn get(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let Some(held) = state.world_manifest.snapshot() else {
        return super::unavailable(
            MIRROR,
            "EXPLORER_MIRRORS_ENABLED",
            "the world manifest has not been mirrored yet (the world-manifest pass runs with the explorer mirrors)",
        );
    };
    let etag = format!("\"{}\"", held.cid);
    let now = chrono::Utc::now();
    let mut resp = if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == etag || t.trim() == "*"))
    {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mut r = Response::new(axum::body::Body::from(held.body.clone()));
        r.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        r
    };
    let h = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(header::ETAG, v);
    }
    if let Ok(v) = HeaderValue::from_str(&http_date(held.fetched_at.and_utc())) {
        h.insert(header::LAST_MODIFIED, v);
    }
    if let Ok(v) = HeaderValue::from_str(&http_date(now + chrono::Duration::seconds(MAX_AGE_SECS)))
    {
        h.insert(header::EXPIRES, v);
    }
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("max-age=86400"),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_is_the_old_jq_rule() {
        assert_eq!(
            validate(br#"{"occupied":[[0,0],[1,0]],"roads":[[2,0]],"empty":[]}"#).unwrap(),
            Counts {
                occupied: 2,
                roads: 1,
                empty: 0
            }
        );
        assert_eq!(
            validate(br#"{"occupied":[[0,0]]}"#).unwrap(),
            Counts {
                occupied: 1,
                roads: 0,
                empty: 0
            },
            "absent lists count as empty"
        );
        assert_eq!(
            validate(br#"{"occupied":[[0,0]],"roads":null}"#)
                .unwrap()
                .roads,
            0
        );
        for bad in [
            &br#"{"occupied":[],"roads":[[1,1]]}"#[..],
            br#"{"roads":[[1,1]]}"#,
            br#"[[0,0]]"#,
            br#"{"occupied":"0,0"}"#,
            br#"{"occupied":[[0,0]],"empty":{}}"#,
            b"not json",
            br#"{"occupied":[[0,0]"#,
        ] {
            assert!(validate(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn http_dates_are_imf_fixdate() {
        let t = chrono::DateTime::parse_from_rfc3339("2026-09-25T11:57:32Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(http_date(t), "Fri, 25 Sep 2026 11:57:32 GMT");
    }
}
