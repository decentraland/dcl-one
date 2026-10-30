//! What the images are made from, read from the ONE database instead of the
//! old `GET {content}/contents/{entity}` hop: the profile entity's own
//! snapshot files (`face256.png`, `body.png` in `content.content_files`),
//! and, for the renderer, the first avatar of its metadata.

use bytes::Bytes;
use serde_json::Value;
use sqlx::PgPool;

use super::cache::ImageKind;
use crate::storage::Storage;

pub enum ResolveResult {
    Avatar(Value),
    NotFound,
    Error(String),
}

/// The deployed snapshot for `kind`, when the entity is a profile this node
/// holds, it lists that file, the blob is present and the store withholds
/// neither the entity nor the hash (`Storage::retrieve_of`). `Ok(None)` is
/// a definite miss; `Err` is a store the answer cannot be read from.
pub async fn snapshot_from_content(
    pool: &PgPool,
    storage: &Storage,
    entity: &str,
    kind: ImageKind,
) -> anyhow::Result<Option<Bytes>> {
    // One probe on deployments_entity_id_key, then the (deployment, key)
    // unique index for the entity's handful of files; the key match is
    // case-insensitive in Rust over those few rows.
    let files: Vec<(String, String)> = sqlx::query_as(
        "SELECT cf.key, cf.content_hash \
         FROM content.deployments d \
         JOIN content.content_files cf ON cf.deployment = d.id \
         WHERE d.entity_id = $1 AND d.entity_type = 'profile'",
    )
    .bind(entity)
    .fetch_all(pool)
    .await?;
    let wanted = kind.content_key();
    let Some((_, hash)) = files
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(wanted))
    else {
        return Ok(None);
    };
    Ok(storage.retrieve_of(entity, hash).await?)
}

/// The first avatar of the profile's metadata, for the renderer.
pub async fn resolve_avatar(pool: &PgPool, entity: &str) -> ResolveResult {
    let row: Result<Option<Value>, sqlx::Error> = sqlx::query_scalar(
        "SELECT entity_metadata->'v' FROM content.deployments WHERE entity_id = $1 AND entity_type = 'profile'",
    )
    .bind(entity)
    .fetch_optional(pool)
    .await;
    match row {
        Ok(Some(metadata)) => extract_avatar(&metadata),
        Ok(None) => ResolveResult::NotFound,
        Err(e) => ResolveResult::Error(format!("profile lookup failed: {e}")),
    }
}

/// `metadata.avatars[0].avatar`, the object the renderer's payload carries.
pub fn extract_avatar(metadata: &Value) -> ResolveResult {
    let avatar = metadata
        .get("avatars")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|a0| a0.get("avatar"))
        .cloned();
    match avatar {
        Some(v) if v.is_object() => ResolveResult::Avatar(v),
        _ => ResolveResult::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_avatar_from_profile_metadata() {
        let m = json!({ "avatars": [ { "avatar": { "bodyShape": "x", "wearables": [] } } ] });
        match extract_avatar(&m) {
            ResolveResult::Avatar(v) => assert_eq!(v["bodyShape"], "x"),
            _ => panic!("expected avatar"),
        }
    }

    #[test]
    fn missing_avatar_is_not_found() {
        let m = json!({ "avatars": [] });
        assert!(matches!(extract_avatar(&m), ResolveResult::NotFound));
        assert!(matches!(
            extract_avatar(&json!({})),
            ResolveResult::NotFound
        ));
        assert!(matches!(
            extract_avatar(&json!({ "avatars": [ { "avatar": "x" } ] })),
            ResolveResult::NotFound
        ));
    }
}
