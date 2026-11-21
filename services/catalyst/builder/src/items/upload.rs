//! `POST /v1/items/:id/files`: a multipart upload of files an item names,
//! each part's field name the content hash the item names for it (as the
//! old builder web sends them). The decision runs before the body is read;
//! the body is read part by part and refused once it passes a bound, and each
//! file is hashed and compared with the hash it was sent under before
//! anything is stored. File names and part content types are never read.
//! Then, in one transaction with the item locked, the decision runs again,
//! every hash must be one the item names under a name other than its
//! video's (the video comes through `/videos` and its MP4 check), and the
//! bytes go to the blob store with a record of who uploaded them
//! (`builder.uploaded_contents`, one row per hash and uploader), under the
//! store's shared lock, so a collector's unlink batch either sees the record
//! or leaves the upload to store again. A file no item names is never
//! stored, and a file's records go once no item names it
//! ([`super::save::forget_unnamed`]).

use axum::extract::Multipart;
use axum::extract::multipart::MultipartError;
use axum::http::StatusCode;
use bytes::{Bytes, BytesMut};
use serde_json::json;

use crate::access::{self, ItemChange};
use crate::chain_state;
use crate::http::ApiError;
use dcl_auth::{hash_bytes_v1, is_canonical_cid};

use super::ItemsComponent;
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

pub(crate) fn too_large() -> ApiError {
    ApiError::http(413, "The upload is too large").with_data(json!({ "maxBytes": MAX_BYTES }))
}

/// The files of `multipart`, each verified against the hash it was sent
/// under, within [`MAX_FILES`] and [`MAX_BYTES`]; an empty file (a folder
/// entry or a 0-byte file, which a save drops from `contents`) is skipped.
pub async fn read(mut multipart: Multipart) -> Result<Vec<(String, Bytes)>, ApiError> {
    let invalid = |e: MultipartError| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            too_large()
        } else {
            ApiError::bad_request("Invalid multipart upload")
        }
    };
    let mut files = Vec::new();
    let (mut parts, mut total) = (0usize, 0usize);
    while let Some(mut field) = multipart.next_field().await.map_err(invalid)? {
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
        while let Some(chunk) = field.chunk().await.map_err(invalid)? {
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
    /// Stores `files`, each a hash the item names, for the item `change` was
    /// decided for.
    pub async fn save_item_files(
        &self,
        change: &ItemChange,
        files: Vec<(String, Bytes)>,
    ) -> Result<(), ApiError> {
        let id = change.id();
        let mut tx = self.pool.begin().await?;
        let view = chain_state::for_decision(&mut tx).await?;
        lock_item(&mut tx, id, None).await?;
        access::confirm_item_change(&mut tx, &view, change).await?;
        crate::storage::gc::lock_store_shared(&mut tx).await?;
        for (hash, bytes) in files {
            let (named, as_file): (bool, bool) = sqlx::query_as(
                "SELECT count(*) > 0, count(*) FILTER (WHERE file <> $3) > 0 \
                 FROM builder.item_contents WHERE item_id = $1 AND hash = $2",
            )
            .bind(id)
            .bind(&hash)
            .bind(VIDEO_FILE)
            .fetch_one(&mut *tx)
            .await?;
            if !named {
                return Err(ApiError::bad_request("The item does not name this file")
                    .with_data(json!({ "hash": hash })));
            }
            if !as_file {
                return Err(ApiError::bad_request(
                    "The video is uploaded through the videos route",
                )
                .with_data(json!({ "hash": hash })));
            }
            let size = bytes.len() as i64;
            if !self
                .blobs
                .exist(&hash)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?
            {
                self.blobs
                    .store(&hash, bytes)
                    .await
                    .map_err(|e| ApiError::internal(format!("storing {hash}: {e}")))?;
            }
            sqlx::query(
                "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, $2, $3) \
                 ON CONFLICT (hash, uploaded_by) DO NOTHING",
            )
            .bind(&hash)
            .bind(size)
            .bind(change.signer())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
