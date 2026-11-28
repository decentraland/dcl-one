//! The node's own draft listings ("my collections", "my item drafts").

use serde::Serialize;
use serde_json::Value;

use crate::access::Wallet;
use crate::chain_state::{self, ReadView};

use super::http::ApiError;
use super::items::{CollectionMetaOut, CollectionMetaRow, ItemsComponent, collection_meta_columns};

#[derive(Debug, sqlx::FromRow)]
struct CollectionListingRow {
    #[sqlx(flatten)]
    collection: CollectionMetaRow,
    item_count: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CollectionListingOut {
    #[serde(flatten)]
    pub collection: CollectionMetaOut,
    pub item_count: i64,
}

pub fn valid_filename(file: &str) -> bool {
    !file.is_empty()
        && file.len() <= 256
        && !file.contains(['\\', '\0'])
        && file
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn collection_listing(view: &ReadView) -> String {
    format!(
        "SELECT {}, (SELECT count(*) FROM builder.items i WHERE i.collection_id=c.id) AS item_count \
         FROM {}",
        collection_meta_columns(view),
        view.collections()
    )
}

fn draft_predicate(view: &ReadView) -> String {
    format!("(NOT {} OR c.third_party_id IS NOT NULL)", view.published())
}

/// The newest drafts one listing answers, of collections or of items.
pub const DRAFTS_CAP: i64 = 5_000;

const ITEM_DRAFT_LISTING: &str = "SELECT jsonb_build_object('id', id, 'name', name, 'type', type, 'collection_id', collection_id) FROM builder.items";

fn item_draft_predicate(view: &ReadView) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM builder.collections c WHERE c.id=items.collection_id \
         AND (c.publication_pending OR {}))",
        view.published_subquery()
    )
}

fn listing_out(rows: &[CollectionListingRow]) -> Vec<CollectionListingOut> {
    rows.iter()
        .map(|row| CollectionListingOut {
            collection: CollectionMetaOut::from(&row.collection),
            item_count: row.item_count,
        })
        .collect()
}

impl ItemsComponent {
    /// "My collections", the newest [`DRAFTS_CAP`]: the owner's unpublished standard collections
    /// (`collections_owner_idx`), and the unpublished and published linked
    /// collections of the providers it manages.
    pub async fn collection_drafts(
        &self,
        wallet: &Wallet,
    ) -> Result<Vec<CollectionListingOut>, ApiError> {
        let mut conn = self.pool.acquire().await?;
        let view = chain_state::for_read(&mut conn).await?;
        let rows = sqlx::query_as::<_, CollectionListingRow>(sqlx::AssertSqlSafe(format!(
            "{} WHERE ((c.eth_address = $1 AND c.third_party_id IS NULL) OR c.third_party_id = ANY($2)) \
             AND {} ORDER BY c.updated_at DESC LIMIT $3",
            collection_listing(&view),
            draft_predicate(&view)
        )))
        .bind(wallet.address())
        .bind(wallet.provider_ids())
        .bind(DRAFTS_CAP)
        .fetch_all(&mut *conn)
        .await?;
        Ok(listing_out(&rows))
    }

    /// "My item drafts", the newest [`DRAFTS_CAP`]: the wallet's unpublished items with no collection
    /// (`items_owner_updated_idx`), and those of the draft standard
    /// collections it owns (`collections_owner_idx`) and of the providers it
    /// manages.
    pub async fn item_drafts(&self, wallet: &Wallet) -> Result<Vec<Value>, ApiError> {
        let mut conn = self.pool.acquire().await?;
        let view = chain_state::for_read(&mut conn).await?;
        Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "{ITEM_DRAFT_LISTING} WHERE NOT is_published AND {} \
             AND ((collection_id IS NULL AND eth_address = $1) OR collection_id IN (SELECT c.id \
               FROM builder.collections c WHERE (c.eth_address = $1 AND c.third_party_id IS NULL) \
               OR c.third_party_id = ANY($2))) ORDER BY updated_at DESC LIMIT $3",
            item_draft_predicate(&view)
        )))
        .bind(wallet.address())
        .bind(wallet.provider_ids())
        .bind(DRAFTS_CAP)
        .fetch_all(&mut *conn)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_paths_allow_subdirectories_without_escape() {
        assert!(valid_filename("textures/skin.png"));
        for bad in [
            "",
            "../skin.png",
            "/skin.png",
            "a/../b",
            "a\\b",
            "a//b",
            "a\0b",
        ] {
            assert!(!valid_filename(bad), "{bad}");
        }
    }
}
