//! `POST /v1/items/:id/files`: a multipart upload of files an item names,
//! each part's field name the content hash the item names for it (as the
//! old builder web sends them). The decision runs before the body is read;
//! the body is read part by part, within its deadline and the idle bound
//! ([`super::places`]), and refused once it passes a bound, and each
//! file is hashed and compared with the hash it was sent under before
//! anything is stored. File names and part content types are never read.
//! Every hash must be one the item names under a name other than its
//! video's (the video comes through `/videos` and its MP4 check); then the
//! bytes go to the blob store, with no connection or lock held. In one
//! transaction with the item locked, the decision and the names are checked
//! again, and each file gets a record of who uploaded it (`builder.uploaded_contents`, one
//! row per hash and uploader) under the store's shared lock
//! ([`lock_store_shared`], 503 past its wait), only once it is still stored
//! (503 otherwise, as content's deployer answers a file collected in
//! between). A file no item names is never stored; a stored file no record
//! names (an upload refused once its bytes were stored) is the collector's,
//! as is one whose records go once no item names it
//! ([`super::save::forget_unnamed`]).

use axum::extract::Multipart;
use axum::extract::multipart::MultipartError;
use axum::http::StatusCode;
use bytes::{Bytes, BytesMut};
use serde_json::json;

use crate::access::{self, ItemChange};
use crate::chain_state;
use crate::http::ApiError;
use crate::storage::gc::{self, GcError};
use dcl_auth::{hash_bytes_v1, is_canonical_cid};

use super::ItemsComponent;
use super::places::Admission;
use super::save::lock_item;
use super::schema::EMPTY_CONTENT_HASH;
use super::video::VIDEO_FILE;

/// Parts in one upload, empty ones included: above the most files one item
/// names in the reference data (198).
pub const MAX_FILES: usize = 256;

/// The longest an upload's body may take to arrive: its largest body at
/// about 0.6 Mbit/s.
pub const DEADLINE: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Bytes in one upload, every file together and so each file: under the
/// gateway's 21 MiB request bound with room for the multipart framing.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

/// The store lock's shared side for a blob writer (`gc::lock_store_shared`), waited for at most
/// `gc::SHARED_LOCK_WAIT`: past it the request answers 503 instead of holding its connection and
/// row locks while unlink batches run.
pub(super) async fn lock_store_shared(conn: &mut sqlx::PgConnection) -> Result<(), ApiError> {
    gc::lock_store_shared_within(conn, gc::SHARED_LOCK_WAIT)
        .await
        .map_err(|e| match e {
            GcError::LockTimeout => store_collected(),
            GcError::Database(e) => e.into(),
            e => ApiError::internal(e.to_string()),
        })
}

pub(super) fn store_collected() -> ApiError {
    ApiError::service_unavailable("The content store is being collected; retry shortly")
}

pub(crate) fn too_large() -> ApiError {
    ApiError::too_large("The upload is too large").with_data(json!({ "maxBytes": MAX_BYTES }))
}

/// The files of `multipart`, read through `place` ([`Admission::read`],
/// [`Admission::arrived`]), each verified against the hash it was sent
/// under, within [`MAX_FILES`] and [`MAX_BYTES`]; an empty file (a folder
/// entry or a 0-byte file, which a save drops from `contents`) is skipped.
pub async fn read(
    mut multipart: Multipart,
    place: &mut Admission,
) -> Result<Vec<(String, Bytes)>, ApiError> {
    let invalid = |e: MultipartError| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            too_large()
        } else {
            ApiError::bad_request("Invalid multipart upload")
        }
    };
    let mut files = Vec::new();
    let (mut parts, mut total) = (0usize, 0usize);
    while let Some(mut field) = place.read(multipart.next_field()).await?.map_err(invalid)? {
        parts += 1;
        if parts > MAX_FILES {
            return Err(
                ApiError::bad_request("Too many files").with_data(json!({ "max": MAX_FILES }))
            );
        }
        let hash = field.name().unwrap_or_default().to_string();
        if !is_canonical_cid(&hash) {
            return Err(ApiError::bad_request(
                "Each file is sent under its content hash",
            ));
        }
        let mut bytes = BytesMut::new();
        while let Some(chunk) = place.read(field.chunk()).await?.map_err(invalid)? {
            place.arrived(chunk.len())?;
            total += chunk.len();
            if total > MAX_BYTES {
                return Err(too_large());
            }
            bytes.extend_from_slice(&chunk);
        }
        let bytes = bytes.freeze();
        if hash_bytes_v1(&bytes) != hash {
            return Err(ApiError::bad_request("The file does not match its hash")
                .with_data(json!({ "hash": hash })));
        }
        if hash != EMPTY_CONTENT_HASH {
            files.push((hash, bytes));
        }
    }
    Ok(files)
}

impl ItemsComponent {
    /// Stores `files`, each a hash the item names, then records them for the
    /// item `change` was decided for, under the store lock's shared side,
    /// once the decision and the names hold again; a file the collector
    /// removed in between answers 503 and records nothing.
    pub async fn save_item_files(
        &self,
        change: &ItemChange,
        files: Vec<(String, Bytes)>,
    ) -> Result<(), ApiError> {
        let id = change.id();
        let hashes: Vec<String> = files.iter().map(|(hash, _)| hash.clone()).collect();
        {
            let mut conn = self.pool.acquire().await?;
            named(&mut conn, id, &hashes).await?;
        }
        let mut sizes = Vec::with_capacity(files.len());
        for (hash, bytes) in files {
            sizes.push(
                i64::try_from(bytes.len())
                    .map_err(|_| ApiError::internal(format!("{hash} is too large to record")))?,
            );
            if !self.exists(&hash).await? {
                self.blobs
                    .store(&hash, bytes)
                    .await
                    .map_err(|e| ApiError::internal(format!("storing {hash}: {e}")))?;
            }
        }
        let mut tx = self.pool.begin().await?;
        let view = chain_state::for_decision(&mut tx, &self.clock).await?;
        lock_item(&mut tx, id, None).await?;
        access::confirm_item_change(&mut tx, &view, change).await?;
        lock_store_shared(&mut tx).await?;
        named(&mut tx, id, &hashes).await?;
        for hash in &hashes {
            if !self.exists(hash).await? {
                return Err(store_collected());
            }
        }
        sqlx::query(
            "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) \
             SELECT hash, size, $3 FROM unnest($1::text[], $2::bigint[]) AS u (hash, size) \
             ON CONFLICT (hash, uploaded_by) DO NOTHING",
        )
        .bind(&hashes)
        .bind(&sizes)
        .bind(change.signer())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn exists(&self, hash: &str) -> Result<bool, ApiError> {
        self.blobs
            .exist(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))
    }
}

/// The item names each of `hashes` under a name other than its video's;
/// 400 for the first that it doesn't.
async fn named(
    conn: &mut sqlx::PgConnection,
    id: uuid::Uuid,
    hashes: &[String],
) -> Result<(), ApiError> {
    let found: Vec<(String, bool, bool)> = sqlx::query_as(
        "SELECT h.hash, count(c.hash) > 0, count(c.hash) FILTER (WHERE c.file <> $3) > 0 \
         FROM unnest($2::text[]) WITH ORDINALITY AS h (hash, place) \
         LEFT JOIN builder.item_contents c ON c.item_id = $1 AND c.hash = h.hash \
         GROUP BY h.hash, h.place ORDER BY h.place",
    )
    .bind(id)
    .bind(hashes)
    .bind(VIDEO_FILE)
    .fetch_all(conn)
    .await?;
    for (hash, named, as_file) in found {
        if !named {
            return Err(ApiError::bad_request("The item does not name this file")
                .with_data(json!({ "hash": hash })));
        }
        if !as_file {
            return Err(
                ApiError::bad_request("The video is uploaded through the videos route")
                    .with_data(json!({ "hash": hash })),
            );
        }
    }
    Ok(())
}
