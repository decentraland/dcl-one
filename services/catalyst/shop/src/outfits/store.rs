// from: shop-server@dc1a8be96:src/adapters/db/outfits.ts (listOutfits, getOutfit, createOutfit, replaceOutfit, removeOutfit, putThumbnail, getThumbnail)
//! The outfits rows (`shop.outfits`, `shop.outfit_thumbnails`): every
//! SELECT/RETURNING reads the same column set; timestamps are the
//! millisecond integers the wire exposes; the thumbnail row maps the sha256
//! wire id to the blob store cid that holds the bytes.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;

use super::OutfitRequest;

/// A wearable reference that outlives listings: contract + item, never a trade id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutfitItemRef {
    pub contract_address: String,
    pub item_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Outfit {
    pub id: String,
    pub name: String,
    /// sha256 hex of the thumbnail bytes, '' on drafts.
    pub thumbnail_hash: String,
    pub items: Vec<OutfitItemRef>,
    pub body_shape: String,
    pub gradient_from: String,
    pub gradient_to: String,
    pub author_address: String,
    pub published: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutfitThumbnail {
    pub hash: String,
    pub blob_cid: String,
    pub content_type: String,
    pub size: i32,
}

type Row = (
    String,
    String,
    String,
    Json<Vec<OutfitItemRef>>,
    String,
    String,
    String,
    String,
    bool,
    i64,
    i64,
);

const COLUMNS: &str = "id, name, thumbnail_hash, items, body_shape, gradient_from, gradient_to, author_address, published, created_at, updated_at";

fn to_outfit(r: Row) -> Outfit {
    Outfit {
        id: r.0,
        name: r.1,
        thumbnail_hash: r.2,
        items: r.3.0,
        body_shape: r.4,
        gradient_from: r.5,
        gradient_to: r.6,
        author_address: r.7,
        published: r.8,
        created_at: r.9,
        updated_at: r.10,
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Clone)]
pub struct Outfits {
    pool: PgPool,
}

impl Outfits {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Newest first; `published_only = false` is the authoring view. The
    /// limit is clamped to 1..=50, the offset to >= 0.
    pub async fn list(
        &self,
        published_only: bool,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<Outfit>> {
        let limit = limit.clamp(1, super::OUTFITS_MAX_LIMIT);
        let offset = offset.max(0);
        let sql = if published_only {
            format!(
                "SELECT {COLUMNS} FROM shop.outfits WHERE published ORDER BY created_at DESC LIMIT $1 OFFSET $2"
            )
        } else {
            format!(
                "SELECT {COLUMNS} FROM shop.outfits ORDER BY created_at DESC LIMIT $1 OFFSET $2"
            )
        };
        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(to_outfit).collect())
    }

    pub async fn get(&self, id: &str) -> sqlx::Result<Option<Outfit>> {
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM shop.outfits WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(to_outfit))
    }

    /// Idempotent on the client-generated id: a replay returns the stored
    /// row, whatever it holds, with `inserted = false`.
    pub async fn create(
        &self,
        input: &OutfitRequest,
        author: &str,
    ) -> sqlx::Result<(Outfit, bool)> {
        let now = now_ms();
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.outfits (id, name, thumbnail_hash, items, body_shape, gradient_from, gradient_to, author_address, published, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10) \
             ON CONFLICT (id) DO NOTHING \
             RETURNING {COLUMNS}"
        )))
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.thumbnail_hash)
        .bind(Json(&input.items))
        .bind(&input.body_shape)
        .bind(&input.gradient_from)
        .bind(&input.gradient_to)
        .bind(author.to_ascii_lowercase())
        .bind(input.published)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = row {
            return Ok((to_outfit(row), true));
        }
        match self.get(&input.id).await? {
            Some(existing) => Ok((existing, false)),
            None => Err(sqlx::Error::RowNotFound),
        }
    }

    /// Full replace, the author kept; `None` when the id does not exist.
    pub async fn replace(&self, input: &OutfitRequest) -> sqlx::Result<Option<Outfit>> {
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "UPDATE shop.outfits \
             SET name = $2, thumbnail_hash = $3, items = $4, body_shape = $5, gradient_from = $6, gradient_to = $7, published = $8, updated_at = $9 \
             WHERE id = $1 \
             RETURNING {COLUMNS}"
        )))
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.thumbnail_hash)
        .bind(Json(&input.items))
        .bind(&input.body_shape)
        .bind(&input.gradient_from)
        .bind(&input.gradient_to)
        .bind(input.published)
        .bind(now_ms())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(to_outfit))
    }

    /// Idempotent; true when a row was deleted.
    pub async fn remove(&self, id: &str) -> sqlx::Result<bool> {
        let res = sqlx::query("DELETE FROM shop.outfits WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Content-addressed: a hash collision is the same bytes, so a conflict is a no-op.
    pub async fn put_thumbnail(
        &self,
        thumbnail: &OutfitThumbnail,
        author: &str,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO shop.outfit_thumbnails (hash, blob_cid, content_type, size, author_address, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (hash) DO NOTHING",
        )
        .bind(&thumbnail.hash)
        .bind(&thumbnail.blob_cid)
        .bind(&thumbnail.content_type)
        .bind(thumbnail.size)
        .bind(author.to_ascii_lowercase())
        .bind(now_ms())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_thumbnail(&self, hash: &str) -> sqlx::Result<Option<OutfitThumbnail>> {
        let row: Option<(String, String, String, i32)> = sqlx::query_as(
            "SELECT hash, blob_cid, content_type, size FROM shop.outfit_thumbnails WHERE hash = $1",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(
            row.map(|(hash, blob_cid, content_type, size)| OutfitThumbnail {
                hash,
                blob_cid,
                content_type,
                size,
            }),
        )
    }
}
