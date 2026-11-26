//! The node's own draft listings ("my collections", "my item drafts"), the
//! shapes its publication snapshots read (`CollectionDraft`, `ItemDraft` and
//! their validation before a publication), and the reads of uploaded files
//! by hash (`builder.uploaded_contents`, the bytes in the ONE blob store).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::access::{FileRead, Wallet};
use crate::chain_state::{self, ReadView};
use crate::storage::OpenedContent;
use dcl_auth::{is_canonical_cid, is_eth_address};

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

#[derive(Debug, Deserialize)]
pub struct CollectionDraft {
    pub id: Uuid,
    pub name: String,
    pub eth_address: Option<String>,
    pub salt: Option<String>,
    pub contract_address: Option<String>,
    pub urn: Option<String>,
    #[serde(default)]
    pub is_published: bool,
    #[serde(default)]
    pub is_approved: bool,
}

#[derive(Debug, Deserialize)]
pub struct ItemDraft {
    pub id: Uuid,
    pub name: String,
    pub collection_id: Option<Uuid>,
    pub eth_address: Option<String>,
    #[serde(default)]
    pub description: String,
    pub thumbnail: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    pub rarity: Option<String>,
    pub price: Option<String>,
    pub beneficiary: Option<String>,
    #[serde(default)]
    pub data: Value,
    pub metrics: Option<Value>,
    #[serde(default)]
    pub contents: BTreeMap<String, String>,
    #[serde(default)]
    pub is_published: bool,
    #[serde(default)]
    pub is_approved: bool,
}

/// A publication snapshot's row belongs to the publishing wallet.
pub(crate) fn owned_by(owner: Option<&str>, signer: &str) -> Result<(), ApiError> {
    if owner.is_some_and(|owner| !owner.eq_ignore_ascii_case(signer)) {
        return Err(ApiError::forbidden(
            "The draft owner must match the signing wallet",
        ));
    }
    Ok(())
}

fn validate_name(name: &str, max: usize) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.encode_utf16().count() > max {
        return Err(ApiError::bad_request(format!(
            "Name must contain 1 to {max} characters"
        )));
    }
    Ok(())
}

impl CollectionDraft {
    pub fn validate(&self, id: Uuid) -> Result<(), ApiError> {
        if self.id != id {
            return Err(ApiError::bad_request(
                "The body and URL collection ids do not match",
            ));
        }
        validate_name(&self.name, 42)?;
        if let Some(urn) = &self.urn {
            let parts: Vec<_> = urn.split(':').collect();
            if urn.len() > 512
                || parts.len() != 6
                || parts[..4] != ["urn", "decentraland", "matic", "collections-thirdparty"]
                || parts[4..].iter().any(|part| {
                    part.is_empty() || part.chars().any(|c| c.is_whitespace() || c == '|')
                })
            {
                return Err(ApiError::bad_request("Invalid linked collection URN"));
            }
        }
        if self.is_published || self.is_approved || self.contract_address.is_some() {
            return Err(ApiError::bad_request(
                "Publication requires a verified blockchain receipt",
            ));
        }
        if self.salt.as_ref().is_some_and(|salt| salt.len() > 128) {
            return Err(ApiError::bad_request("Invalid collection salt"));
        }
        Ok(())
    }
}

impl ItemDraft {
    pub fn validate(&self, id: Uuid) -> Result<(), ApiError> {
        if self.id != id {
            return Err(ApiError::bad_request(
                "The body and URL item ids do not match",
            ));
        }
        validate_name(&self.name, 64)?;
        if !matches!(self.item_type.as_str(), "wearable" | "emote") || !self.data.is_object() {
            return Err(ApiError::bad_request("Invalid item type or data"));
        }
        if self.is_published || self.is_approved {
            return Err(ApiError::bad_request(
                "Draft writes cannot change publication or approval",
            ));
        }
        if self.metrics.as_ref().is_some_and(|metrics| {
            !metrics.is_object()
                || metrics.to_string().len() > 1024
                || metrics.as_object().is_some_and(|fields| {
                    fields
                        .values()
                        .any(|v| v.as_f64().is_none_or(|n| n < 0.0 || !n.is_finite()))
                })
        }) {
            return Err(ApiError::bad_request("Invalid item metrics"));
        }
        if self.description.len() > 4096
            || self.data.to_string().len() > 128 * 1024
            || self.contents.len() > 100
        {
            return Err(ApiError::bad_request("Item metadata is too large"));
        }
        if self.price.as_ref().is_some_and(|price| {
            price.is_empty() || price.len() > 78 || !price.bytes().all(|b| b.is_ascii_digit())
        }) {
            return Err(ApiError::bad_request(
                "Price must be an integer amount in MANA wei",
            ));
        }
        if self
            .beneficiary
            .as_ref()
            .is_some_and(|address| !is_eth_address(address))
        {
            return Err(ApiError::bad_request("Invalid beneficiary address"));
        }
        if self.rarity.as_deref().is_some_and(|rarity| {
            !matches!(
                rarity,
                "unique"
                    | "mythic"
                    | "exotic"
                    | "legendary"
                    | "epic"
                    | "rare"
                    | "uncommon"
                    | "common"
            )
        }) {
            return Err(ApiError::bad_request("Invalid rarity"));
        }
        for (file, hash) in &self.contents {
            if !valid_filename(file) || !is_canonical_cid(hash) {
                return Err(ApiError::bad_request("Invalid content filename or CID"));
            }
        }
        Ok(())
    }
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
        let view = chain_state::for_read(&mut conn, &self.clock).await?;
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
        let view = chain_state::for_read(&mut conn, &self.clock).await?;
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

    #[cfg(test)]
    pub async fn uploaded_content_exists(&self, hash: &str) -> Result<bool, ApiError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM builder.uploaded_contents WHERE hash=$1)",
        )
        .bind(hash)
        .fetch_one(&self.pool)
        .await?)
    }

    /// The accepted upload's bytes, whole.
    #[cfg(test)]
    pub async fn uploaded_content(&self, hash: &str) -> Result<Option<bytes::Bytes>, ApiError> {
        if !self.uploaded_content_exists(hash).await? {
            return Ok(None);
        }
        self.blobs
            .retrieve(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))
    }

    /// The file [`crate::access::read_file`] granted, opened once for its whole
    /// answer; `None` when the store holds no bytes for it.
    pub async fn open_file(&self, read: &FileRead) -> Result<Option<OpenedContent>, ApiError> {
        self.blobs
            .open(read.hash())
            .await
            .map_err(|e| ApiError::internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_draft_write_cannot_claim_publication_and_its_owner_field_is_ignored() {
        let id = Uuid::nil();
        let mut draft: CollectionDraft =
            serde_json::from_value(json!({"id": id, "name": "My collection"})).unwrap();
        assert!(draft.validate(id).is_ok());
        draft.eth_address = Some("0xother".into());
        assert!(draft.validate(id).is_ok());
        draft.is_published = true;
        assert!(draft.validate(id).is_err());
    }

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
