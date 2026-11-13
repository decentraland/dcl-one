//! `catalyst camera-reel import`: copies camera-reel-service's photos into
//! `camera_reel.images` (`services/catalyst/docs/camera-reel.md` is the
//! cutover runbook).
//!
//! The source is the old service's Postgres table `images` (`id`,
//! `user_address`, `url`, `thumbnail_url`, `metadata`, `created_at`,
//! `is_public`; camera-reel-service @ 3e8bd0b `migrations/`), read in pages
//! by its `(created_at, id)` keyset, and its public bucket, which holds each
//! photo at the key ending its `url` (`{id}-{file}`) and the thumbnail at the
//! key ending its `thumbnail_url` (`{id}-thumbnail-{file}`). A photo keeps
//! its id, owner (lowercased), document, visibility and `created_at`; its
//! bytes land in the ONE store by hash, and its legacy URLs keep resolving
//! through `GET /api/images/{key}` ([`crate::db::LegacyKey`]), so the file
//! name is not stored.
//!
//! What an imported row is not: an upload. It is inserted with
//! `review_status = 'ok'` and `ranking_published = true` (the old service's
//! photos were counted when they were taken, so the community photo counts
//! do not count them again) and records no `camera_reel.photo_events` row.
//! `MAX_IMAGES_PER_USER` is not applied: the old service enforced it when the
//! photos were taken.
//!
//! Re-running is the resume: a photo already here is not downloaded again;
//! its `is_public` and document are refreshed from the source (the source is
//! the authority until cutover), but only while the row is one this import
//! wrote (`camera_reel.imported_images`) and its `review_status` is `ok`, so
//! a moderator's verdict and a photo uploaded here are never touched.
//! `prune` deletes the imported rows the source no longer has, through the
//! owner-delete path (the blobs are withheld as for any delete).
//!
//! A source row that the local constraints refuse, whose objects are
//! missing, or that cannot be fetched is skipped and reported by id and
//! reason; the run continues. Neither the source database URL nor the bucket
//! URL appears in the report or a log line (an HTTP error is reported
//! without its URL).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use anyhow::Context as _;
use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::StreamExt as _;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::dto::Metadata;
use crate::storage::Storage;
use dcl_http::{Outbound, RetryDecision, SendError, read_body_capped, retry_with_backoff};

/// The largest object fetched from the bucket; the old service took images
/// of at most 15 MiB (its base64 route) and stored thumbnails smaller still.
pub const MAX_OBJECT_BYTES: usize = 32 * 1024 * 1024;
/// Attempts per object for a transport failure or a 5xx; a 429 gets the
/// shared ladder's own budget.
const FETCH_ATTEMPTS: u32 = 4;
const FETCH_BACKOFF_MS: u64 = 500;
/// The whole-request budget of one object fetch (the command's client).
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// The old service's `BUCKET_URL`: objects are read at `<bucket_url>/<key>`.
    pub bucket_url: url::Url,
    /// Source rows per page.
    pub batch: i64,
    /// Photos fetched and stored at once within a page.
    pub concurrency: usize,
    /// Fetch and check everything, write nothing (no blob, no row, no prune).
    pub dry_run: bool,
    /// After a complete pass, delete the imported rows whose id the source
    /// no longer has.
    pub prune: bool,
}

/// Why a source row was not imported or refreshed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// `created_at` is NULL in the source; the local column is NOT NULL.
    CreatedAtMissing,
    /// `metadata` is not a JSON object (`images_metadata_object_check`).
    MetadataNotObject,
    /// `metadata` does not read as the photo document every gallery decodes.
    MetadataShape,
    /// `metadata.userAddress` is not the row's owner
    /// (`images_metadata_owner_check`).
    OwnerMismatch,
    /// `url` does not end with this photo's key `{id}-{file}`.
    ImageKey,
    /// `thumbnail_url` is set and does not end with a key of this photo.
    ThumbnailKey,
    /// The bucket answered 403/404/410 for the image.
    ImageMissing,
    /// The image could not be fetched after the retries, or is over
    /// [`MAX_OBJECT_BYTES`].
    ImageFetchFailed,
    /// The thumbnail could not be fetched after the retries (a missing one
    /// is generated instead).
    ThumbnailFetchFailed,
    /// The thumbnail had to be generated and the image does not decode.
    ImageUndecodable,
    /// A photo uploaded here has this id; it is not the source's to change.
    LocalPhoto,
    /// The local table refused the row (a `CHECK` named in the detail).
    Refused,
    /// The blob store refused a write.
    StoreFailed,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAtMissing => "created_at_missing",
            Self::MetadataNotObject => "metadata_not_object",
            Self::MetadataShape => "metadata_shape",
            Self::OwnerMismatch => "owner_mismatch",
            Self::ImageKey => "image_key",
            Self::ThumbnailKey => "thumbnail_key",
            Self::ImageMissing => "image_missing",
            Self::ImageFetchFailed => "image_fetch_failed",
            Self::ThumbnailFetchFailed => "thumbnail_fetch_failed",
            Self::ImageUndecodable => "image_undecodable",
            Self::LocalPhoto => "local_photo",
            Self::Refused => "refused",
            Self::StoreFailed => "store_failed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Skip {
    pub id: Uuid,
    pub reason: SkipReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// What one run did; printed as JSON by the command.
#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub dry_run: bool,
    /// Source rows read.
    pub read: u64,
    /// New rows inserted (counted, not written, on a dry run).
    pub imported: u64,
    /// Imported rows whose `is_public` or document changed in the source.
    pub updated: u64,
    /// Imported rows already equal to the source.
    pub unchanged: u64,
    /// Imported rows a moderator set to `flagged` or `rejected`; left alone.
    pub kept_moderated: u64,
    /// Imported photos whose thumbnail was generated (no `thumbnail_url`, or
    /// the bucket had no object for it).
    pub thumbnails_generated: u64,
    /// Bytes fetched from the bucket.
    pub bytes_fetched: u64,
    /// Bytes written to the store (blobs it already held are not counted).
    pub bytes_stored: u64,
    /// Skipped rows by reason.
    pub skipped: BTreeMap<&'static str, u64>,
    /// Every skipped row.
    pub skips: Vec<Skip>,
    /// Imported rows deleted because the source no longer has them
    /// (counted, not deleted, on a dry run); `None` without `prune`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pruned: Option<u64>,
}

impl ImportReport {
    fn skip(&mut self, skip: Skip) {
        tracing::warn!(id = %skip.id, reason = skip.reason.as_str(), detail = ?skip.detail, "camera reel import skipped a photo");
        *self.skipped.entry(skip.reason.as_str()).or_default() += 1;
        self.skips.push(skip);
    }
}

#[derive(sqlx::FromRow)]
struct SourceRow {
    id: Uuid,
    user_address: String,
    url: String,
    thumbnail_url: String,
    metadata: Json<Value>,
    created_at: NaiveDateTime,
    is_public: bool,
}

#[derive(sqlx::FromRow)]
struct LocalRow {
    id: Uuid,
    review_status: String,
    is_public: bool,
    metadata: Json<Value>,
    imported: bool,
}

/// One pass over the source; with `prune`, then the prune.
pub async fn run(
    source: &PgPool,
    target: &PgPool,
    storage: &Storage,
    http: &Outbound,
    options: &ImportOptions,
) -> anyhow::Result<ImportReport> {
    let mut report = ImportReport {
        dry_run: options.dry_run,
        ..Default::default()
    };
    let mut seen: HashSet<Uuid> = HashSet::new();

    // Rows without `created_at` are outside the keyset; the local column
    // is NOT NULL and inventing a time would misorder the galleries.
    let undated: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM images WHERE created_at IS NULL ORDER BY id")
            .fetch_all(source)
            .await
            .context("reading the source's undated rows")?;
    for id in undated {
        report.read += 1;
        seen.insert(id);
        report.skip(Skip {
            id,
            reason: SkipReason::CreatedAtMissing,
            detail: None,
        });
    }

    let mut after: Option<(NaiveDateTime, Uuid)> = None;
    loop {
        let rows: Vec<SourceRow> = sqlx::query_as(
            "SELECT id, user_address, url, thumbnail_url, metadata, created_at, is_public \
             FROM images \
             WHERE created_at IS NOT NULL \
               AND ($1::timestamp IS NULL OR (created_at, id) > ($1, $2)) \
             ORDER BY created_at, id LIMIT $3",
        )
        .bind(after.map(|(at, _)| at))
        .bind(after.map(|(_, id)| id).unwrap_or_else(Uuid::nil))
        .bind(options.batch.max(1))
        .fetch_all(source)
        .await
        .context("reading a page of the source's images")?;
        let Some(last) = rows.last() else {
            break;
        };
        after = Some((last.created_at, last.id));
        report.read += rows.len() as u64;
        seen.extend(rows.iter().map(|r| r.id));
        import_page(target, storage, http, options, rows, &mut report).await?;
    }

    if options.prune {
        report.pruned = Some(prune(target, &seen, options.dry_run).await?);
    }
    Ok(report)
}

/// The checks the local table and the galleries' decoding apply, before
/// anything is fetched.
fn check(row: &SourceRow) -> Result<(), Skip> {
    let skip = |reason, detail: Option<String>| Skip {
        id: row.id,
        reason,
        detail,
    };
    let Value::Object(document) = &row.metadata.0 else {
        return Err(skip(SkipReason::MetadataNotObject, None));
    };
    if let Err(e) = serde_json::from_value::<Metadata>(row.metadata.0.clone()) {
        return Err(skip(SkipReason::MetadataShape, Some(e.to_string())));
    }
    let owner = document.get("userAddress").and_then(Value::as_str);
    if owner.map(str::to_lowercase) != Some(row.user_address.to_lowercase()) {
        return Err(skip(SkipReason::OwnerMismatch, None));
    }
    Ok(())
}

/// The bucket key a stored URL ends with, when it is one of this photo's
/// (`{id}-...` with a non-empty rest).
fn key_of(id: Uuid, url: &str) -> Option<&str> {
    let key = url.rsplit('/').next()?;
    let rest = key.strip_prefix(&id.to_string())?.strip_prefix('-')?;
    (!rest.is_empty() && !rest.contains('\\')).then_some(key)
}

async fn import_page(
    target: &PgPool,
    storage: &Storage,
    http: &Outbound,
    options: &ImportOptions,
    rows: Vec<SourceRow>,
    report: &mut ImportReport,
) -> anyhow::Result<()> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let local: HashMap<Uuid, LocalRow> = sqlx::query_as::<_, LocalRow>(
        "SELECT i.id, i.review_status, i.is_public, i.metadata, (m.id IS NOT NULL) AS imported \
         FROM camera_reel.images i LEFT JOIN camera_reel.imported_images m ON m.id = i.id \
         WHERE i.id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(target)
    .await
    .context("reading the page's local rows")?
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

    let mut new_rows = Vec::new();
    for row in rows {
        if let Err(skip) = check(&row) {
            report.skip(skip);
            continue;
        }
        match local.get(&row.id) {
            None => new_rows.push(row),
            Some(here) if !here.imported => report.skip(Skip {
                id: row.id,
                reason: SkipReason::LocalPhoto,
                detail: None,
            }),
            Some(here) if here.review_status != "ok" => report.kept_moderated += 1,
            Some(here) if here.is_public == row.is_public && here.metadata.0 == row.metadata.0 => {
                report.unchanged += 1
            }
            Some(_) if options.dry_run => report.updated += 1,
            Some(_) => match refresh(target, &row).await {
                Ok(true) => report.updated += 1,
                Ok(false) => report.unchanged += 1,
                Err(e) => match refused(&e) {
                    Some(constraint) => report.skip(Skip {
                        id: row.id,
                        reason: SkipReason::Refused,
                        detail: Some(constraint),
                    }),
                    None => return Err(e).context("refreshing an imported row"),
                },
            },
        }
    }

    let mut outcomes = futures::stream::iter(new_rows)
        .map(|row| import_one(target, storage, http, options, row))
        .buffer_unordered(options.concurrency.max(1));
    while let Some(outcome) = outcomes.next().await {
        let outcome = outcome?;
        report.bytes_fetched += outcome.fetched;
        report.bytes_stored += outcome.stored;
        report.thumbnails_generated += u64::from(outcome.generated);
        match outcome.result {
            Imported::Inserted => report.imported += 1,
            Imported::AlreadyThere => report.unchanged += 1,
            Imported::Skipped(skip) => report.skip(skip),
        }
    }
    Ok(())
}

/// The source's visibility and document onto an imported, unmoderated row.
async fn refresh(target: &PgPool, row: &SourceRow) -> anyhow::Result<bool> {
    let updated = sqlx::query(
        "UPDATE camera_reel.images i SET is_public = $2, metadata = $3 \
         WHERE i.id = $1 AND i.review_status = 'ok' \
           AND EXISTS (SELECT 1 FROM camera_reel.imported_images m WHERE m.id = i.id) \
           AND (i.is_public IS DISTINCT FROM $2 OR i.metadata IS DISTINCT FROM $3)",
    )
    .bind(row.id)
    .bind(row.is_public)
    .bind(&row.metadata)
    .execute(target)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// The constraint a refused write names, for a `CHECK` violation.
fn refused(error: &anyhow::Error) -> Option<String> {
    let db = error.downcast_ref::<sqlx::Error>()?.as_database_error()?;
    (db.code().as_deref() == Some("23514")).then(|| db.constraint().unwrap_or("check").to_string())
}

enum Imported {
    Inserted,
    AlreadyThere,
    Skipped(Skip),
}

struct Outcome {
    result: Imported,
    fetched: u64,
    stored: u64,
    generated: bool,
}

enum Fetched {
    Bytes(Bytes),
    Missing,
    Failed(String),
}

async fn import_one(
    target: &PgPool,
    storage: &Storage,
    http: &Outbound,
    options: &ImportOptions,
    row: SourceRow,
) -> anyhow::Result<Outcome> {
    let mut outcome = Outcome {
        result: Imported::Inserted,
        fetched: 0,
        stored: 0,
        generated: false,
    };
    let skip = |reason, detail: Option<String>| {
        Imported::Skipped(Skip {
            id: row.id,
            reason,
            detail,
        })
    };
    let Some(image_key) = key_of(row.id, &row.url) else {
        outcome.result = skip(SkipReason::ImageKey, None);
        return Ok(outcome);
    };
    let thumbnail_key = if row.thumbnail_url.is_empty() {
        None
    } else {
        match key_of(row.id, &row.thumbnail_url) {
            Some(key) => Some(key),
            None => {
                outcome.result = skip(SkipReason::ThumbnailKey, None);
                return Ok(outcome);
            }
        }
    };

    let image = match fetch(http, &options.bucket_url, image_key).await {
        Fetched::Bytes(bytes) => bytes,
        Fetched::Missing => {
            outcome.result = skip(SkipReason::ImageMissing, None);
            return Ok(outcome);
        }
        Fetched::Failed(why) => {
            outcome.result = skip(SkipReason::ImageFetchFailed, Some(why));
            return Ok(outcome);
        }
    };
    outcome.fetched += image.len() as u64;
    let fetched_thumbnail = match thumbnail_key {
        None => Fetched::Missing,
        Some(key) => fetch(http, &options.bucket_url, key).await,
    };
    let thumbnail = match fetched_thumbnail {
        Fetched::Bytes(bytes) => {
            outcome.fetched += bytes.len() as u64;
            bytes
        }
        Fetched::Missing => {
            let image = image.clone();
            match tokio::task::spawn_blocking(move || crate::handlers::images::thumbnail_of(&image))
                .await?
            {
                Ok(bytes) => {
                    outcome.generated = true;
                    bytes
                }
                Err(_) => {
                    outcome.result = skip(SkipReason::ImageUndecodable, None);
                    return Ok(outcome);
                }
            }
        }
        Fetched::Failed(why) => {
            outcome.result = skip(SkipReason::ThumbnailFetchFailed, Some(why));
            return Ok(outcome);
        }
    };
    if options.dry_run {
        return Ok(outcome);
    }

    let image_hash = crate::auth::hash_bytes_v1(&image);
    let thumbnail_hash = crate::auth::hash_bytes_v1(&thumbnail);
    // The blobs land before the row, as an upload's do: a row never names
    // a blob the store lacks.
    for (hash, bytes) in [(&image_hash, image), (&thumbnail_hash, thumbnail)] {
        match storage.exist(hash).await {
            Ok(true) => {}
            Ok(false) => {
                let len = bytes.len() as u64;
                if let Err(e) = storage.store(hash, bytes).await {
                    outcome.result = skip(SkipReason::StoreFailed, Some(e.to_string()));
                    return Ok(outcome);
                }
                outcome.stored += len;
            }
            Err(e) => {
                outcome.result = skip(SkipReason::StoreFailed, Some(e.to_string()));
                return Ok(outcome);
            }
        }
    }

    match insert(target, &row, &image_hash, &thumbnail_hash).await {
        Ok(true) => {}
        Ok(false) => outcome.result = Imported::AlreadyThere,
        Err(e) => match refused(&e) {
            Some(constraint) => outcome.result = skip(SkipReason::Refused, Some(constraint)),
            None => return Err(e).context("inserting an imported row"),
        },
    }
    Ok(outcome)
}

/// The row and its provenance in one transaction; `false` when the id is
/// already taken (another run inserted it first).
async fn insert(
    target: &PgPool,
    row: &SourceRow,
    image_hash: &str,
    thumbnail_hash: &str,
) -> anyhow::Result<bool> {
    let mut tx = target.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO camera_reel.images \
         (id, user_address, image_hash, thumbnail_hash, is_public, review_status, \
          ranking_published, metadata, created_at) \
         VALUES ($1, $2, $3, $4, $5, 'ok', true, $6, $7) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(row.id)
    .bind(row.user_address.to_lowercase())
    .bind(image_hash)
    .bind(thumbnail_hash)
    .bind(row.is_public)
    .bind(&row.metadata)
    .bind(row.created_at)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if inserted {
        sqlx::query("INSERT INTO camera_reel.imported_images (id) VALUES ($1)")
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(inserted)
}

/// `GET <bucket_url>/<key>` with the key as one path segment (encoded);
/// 403/404/410 is a missing object (a public S3 bucket answers 403 for a
/// key it does not have), a transport failure, 429 or 5xx is retried.
async fn fetch(http: &Outbound, bucket_url: &url::Url, key: &str) -> Fetched {
    let mut url = bucket_url.clone();
    match url.path_segments_mut() {
        Ok(mut segments) => {
            segments.pop_if_empty().push(key);
        }
        Err(()) => return Fetched::Failed("the bucket URL cannot take a path".into()),
    }
    let last_failure = std::sync::Mutex::new(String::new());
    let failed = |why: String| *last_failure.lock().unwrap() = why;
    let attempt = || {
        let url = url.clone();
        let failed = &failed;
        async move {
            let response = match http.get(url).send().await {
                Ok(response) => response,
                Err(e) => {
                    failed(e.to_string());
                    return Ok(match e {
                        SendError::Http(ref h) if is_retryable(h) => RetryDecision::Retry {
                            after: None,
                            rate_limited: false,
                        },
                        _ => RetryDecision::Done(Fetched::Failed(e.to_string())),
                    });
                }
            };
            let status = response.status();
            if status.is_success() {
                return Ok(match read_body_capped(response, MAX_OBJECT_BYTES).await {
                    Ok(body) => RetryDecision::Done(Fetched::Bytes(Bytes::from(body))),
                    Err(e) => {
                        failed(e.to_string());
                        RetryDecision::Retry {
                            after: None,
                            rate_limited: false,
                        }
                    }
                });
            }
            if matches!(status.as_u16(), 403 | 404 | 410) {
                return Ok(RetryDecision::Done(Fetched::Missing));
            }
            failed(format!("status {status}"));
            if dcl_http::is_transient_status(status) {
                let after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(dcl_http::parse_retry_after);
                return Ok(RetryDecision::Retry {
                    after,
                    rate_limited: status == reqwest::StatusCode::TOO_MANY_REQUESTS,
                });
            }
            Ok(RetryDecision::Done(Fetched::Failed(format!(
                "status {status}"
            ))))
        }
    };
    match retry_with_backoff(
        "camera reel bucket object",
        FETCH_ATTEMPTS,
        FETCH_BACKOFF_MS,
        attempt,
    )
    .await
    {
        Ok(Some(fetched)) => fetched,
        Ok(None) => Fetched::Failed(format!(
            "gave up after retries: {}",
            last_failure.lock().unwrap()
        )),
        Err(e) => Fetched::Failed(e.to_string()),
    }
}

fn is_retryable(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request() || e.is_body()
}

/// Deletes every imported row whose id the complete pass did not read,
/// through the delete path (the blobs are withheld, as for an owner's
/// delete). Returns how many it deleted, or would delete on a dry run.
async fn prune(target: &PgPool, seen: &HashSet<Uuid>, dry_run: bool) -> anyhow::Result<u64> {
    let db = crate::db::Database::new(target.clone(), "");
    let mut pruned = 0u64;
    let mut after = Uuid::nil();
    loop {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM camera_reel.imported_images WHERE id > $1 ORDER BY id LIMIT 1000",
        )
        .bind(after)
        .fetch_all(target)
        .await
        .context("reading the imported rows")?;
        let Some(last) = ids.last() else {
            break;
        };
        after = *last;
        for id in ids.into_iter().filter(|id| !seen.contains(id)) {
            if dry_run {
                pruned += 1;
                continue;
            }
            let deleted = db
                .delete_image_owned(&id.to_string(), None, chrono::Utc::now().naive_utc())
                .await
                .context("deleting an imported row the source no longer has")?;
            if deleted.is_some_and(|d| d.deleted) {
                pruned += 1;
            }
        }
    }
    Ok(pruned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_the_last_url_segment_of_this_photo() {
        let id: Uuid = "e9659dd5-0f87-4169-a9a6-1ec0d5faf051".parse().unwrap();
        let base = "https://camera-reel-service.decentraland.org/api/images";
        assert_eq!(
            key_of(id, &format!("{base}/{id}-1790074770.jpg")),
            Some(format!("{id}-1790074770.jpg").as_str())
        );
        assert_eq!(
            key_of(id, &format!("{base}/{id}-thumbnail-1790074770.jpg")),
            Some(format!("{id}-thumbnail-1790074770.jpg").as_str())
        );
        assert_eq!(key_of(id, &format!("{base}/{id}-")), None);
        assert_eq!(key_of(id, &format!("{base}/{id}")), None);
        assert_eq!(
            key_of(id, &format!("{base}/{}-a.jpg", Uuid::new_v4())),
            None
        );
        assert_eq!(key_of(id, ""), None);
    }
}
