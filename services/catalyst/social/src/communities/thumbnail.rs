// from: catalyrst/crates/catalyrst-social-service/src/rest/thumbnail_signature.rs (magic-byte validation, port of upstream social-service-ea image-signature.ts #444), catalyrst/crates/catalyrst-social-service/src/rest/handlers/client/mod.rs (store_thumbnail), catalyrst/crates/catalyrst-social-service/src/rest/content_store.rs (MAX_BODY_BYTES)
//! Community thumbnails: bounded, signature-checked PNG/JPEG/GIF/WebP bytes,
//! kept in the ONE blob store under their content hash with the hash in
//! `social.community_thumbnails` (has_thumbnail on the wire = the row
//! exists). Only fixed signature bytes are inspected, no container parsing.

use bytes::Bytes;
use sqlx::PgExecutor;

use crate::CommunityId;
use crate::storage::{Storage, StorageError};

/// Rejected before any storage or database work happens (upstream: 1KB floor).
pub const MIN_THUMBNAIL_BYTES: usize = 1024;

/// The old communities content store's body cap; validation and storage agree.
pub const MAX_THUMBNAIL_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMimeType {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageMimeType {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageMimeType::Png => "image/png",
            ImageMimeType::Jpeg => "image/jpeg",
            ImageMimeType::Gif => "image/gif",
            ImageMimeType::Webp => "image/webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailError {
    /// Fewer than [`MIN_THUMBNAIL_BYTES`] or more than [`MAX_THUMBNAIL_BYTES`].
    Size,
    UnsupportedSignature,
}

impl ThumbnailError {
    pub fn message(self) -> &'static str {
        match self {
            ThumbnailError::Size => "Thumbnail size must be between 1KB and 256KB",
            ThumbnailError::UnsupportedSignature => {
                "Thumbnail must start with a supported PNG, JPEG, GIF or WebP signature"
            }
        }
    }
}

const PNG: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const JPEG: [u8; 3] = [0xff, 0xd8, 0xff];
const GIF87A: [u8; 6] = [0x47, 0x49, 0x46, 0x38, 0x37, 0x61];
const GIF89A: [u8; 6] = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61];

/// Reads the media type a buffer's leading bytes announce, so the thumbnail
/// is stored and served as what it actually is.
pub fn detect_image_mime_type(buffer: &[u8]) -> Option<ImageMimeType> {
    if buffer.starts_with(&PNG) {
        return Some(ImageMimeType::Png);
    }
    if buffer.starts_with(&JPEG) {
        return Some(ImageMimeType::Jpeg);
    }
    if buffer.starts_with(&GIF87A) || buffer.starts_with(&GIF89A) {
        return Some(ImageMimeType::Gif);
    }
    if buffer.len() >= 12 && &buffer[0..4] == b"RIFF" && &buffer[8..12] == b"WEBP" {
        return Some(ImageMimeType::Webp);
    }
    None
}

/// Size bounds first, then signature. Callers run this before any
/// authorization or database write, so an unrecognised blob never reaches
/// the blob store.
pub fn validate_thumbnail(buffer: &[u8]) -> Result<ImageMimeType, ThumbnailError> {
    if buffer.len() < MIN_THUMBNAIL_BYTES || buffer.len() > MAX_THUMBNAIL_BYTES {
        return Err(ThumbnailError::Size);
    }
    detect_image_mime_type(buffer).ok_or(ThumbnailError::UnsupportedSignature)
}

/// The blob key of a thumbnail: the same content hash the content module
/// mints for a file, so the ONE store holds it beside every other blob.
pub fn thumbnail_hash(bytes: &[u8]) -> String {
    crate::auth::hash::hash_bytes(bytes)
}

#[derive(Debug, thiserror::Error)]
pub enum StoreThumbnailError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Writes the bytes to the blob store, then the hash row; the row write runs
/// on the caller's executor so a create/update transaction carries it.
pub async fn store_thumbnail<'a, E>(
    executor: E,
    storage: &Storage,
    community_id: CommunityId,
    bytes: &[u8],
) -> Result<String, StoreThumbnailError>
where
    E: PgExecutor<'a>,
{
    let hash = thumbnail_hash(bytes);
    storage.store(&hash, Bytes::copy_from_slice(bytes)).await?;
    sqlx::query(
        "INSERT INTO social.community_thumbnails (community_id, thumbnail_hash, updated_at) \
         VALUES ($1, $2, timezone('UTC', now())) \
         ON CONFLICT (community_id) DO UPDATE \
           SET thumbnail_hash = EXCLUDED.thumbnail_hash, updated_at = timezone('UTC', now())",
    )
    .bind(community_id)
    .bind(&hash)
    .execute(executor)
    .await?;
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padded(prefix: &[u8], len: usize) -> Vec<u8> {
        let mut v = prefix.to_vec();
        v.resize(len.max(prefix.len()), 0x00);
        v
    }

    #[test]
    fn detects_each_supported_signature() {
        assert_eq!(detect_image_mime_type(&PNG), Some(ImageMimeType::Png));
        assert_eq!(detect_image_mime_type(&JPEG), Some(ImageMimeType::Jpeg));
        assert_eq!(detect_image_mime_type(&GIF87A), Some(ImageMimeType::Gif));
        assert_eq!(detect_image_mime_type(&GIF89A), Some(ImageMimeType::Gif));
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBP");
        assert_eq!(detect_image_mime_type(&webp), Some(ImageMimeType::Webp));
    }

    #[test]
    fn rejects_non_image_bytes() {
        assert_eq!(detect_image_mime_type(b"<html></html>"), None);
        assert_eq!(detect_image_mime_type(b"#!/bin/sh\n"), None);
        assert_eq!(detect_image_mime_type(&[]), None);
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&[0, 0, 0, 0]);
        wav.extend_from_slice(b"WAVE");
        assert_eq!(detect_image_mime_type(&wav), None);
    }

    #[test]
    fn validate_enforces_size_and_signature() {
        assert_eq!(validate_thumbnail(&PNG), Err(ThumbnailError::Size));
        let blob = padded(b"not-an-image", MIN_THUMBNAIL_BYTES);
        assert_eq!(
            validate_thumbnail(&blob),
            Err(ThumbnailError::UnsupportedSignature)
        );
        let png = padded(&PNG, MIN_THUMBNAIL_BYTES);
        assert_eq!(validate_thumbnail(&png), Ok(ImageMimeType::Png));
        let png = padded(&PNG, MAX_THUMBNAIL_BYTES + 1);
        assert_eq!(validate_thumbnail(&png), Err(ThumbnailError::Size));
    }

    #[test]
    fn the_thumbnail_hash_is_the_content_hash_of_the_bytes() {
        let png = padded(&PNG, MIN_THUMBNAIL_BYTES);
        assert_eq!(thumbnail_hash(&png), crate::auth::hash::hash_bytes(&png));
        assert!(crate::auth::hash::verify_hash(&png, &thumbnail_hash(&png)));
    }
}
