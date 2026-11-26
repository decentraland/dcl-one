//! Standard collection publication, the preparation half: the frozen
//! snapshot of a collection and its items (`SNAPSHOT_QUERY`, items in
//! creation order) validated and turned into the Polygon collection-manager
//! parameters the client's wallet signs (`PublicationPreparation`: the
//! node's Polygon chain ([`crate::network`]) with its manager, factory and
//! forwarder contracts and base URI, a stable salt, the symbol
//! and each item's on-chain metadata string). The `revision` is the CIDv1 of
//! the snapshot text, so any edit invalidates a review. Nothing here writes
//! or signs: `publication_store` records the attempt, `publication_chain`
//! verifies the receipt. A published collection is refused before its
//! snapshot is prepared ([`chain_state::published`]).

use alloy_primitives::{Address, B256, U256, keccak256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::access::Publisher;
use crate::network::Network;

use dcl_auth::hash_bytes_v1;

use super::chain_state;
use super::drafts::ItemDraft;
use super::http::ApiError;
use super::items::ItemsComponent;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct PublicationItem {
    pub id: String,
    pub rarity: String,
    pub price: String,
    pub beneficiary: String,
    pub metadata: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct PublicationPreparation {
    pub id: String,
    pub revision: String,
    pub chain_id: u32,
    pub manager: String,
    pub factory: String,
    pub forwarder: String,
    pub salt: String,
    pub name: String,
    pub symbol: String,
    pub base_uri: String,
    pub creator: String,
    pub items: Vec<PublicationItem>,
}

impl ItemsComponent {
    pub async fn prepare_publication(
        &self,
        publisher: &Publisher,
    ) -> Result<PublicationPreparation, ApiError> {
        let (signer, id) = (publisher.signer(), publisher.id());
        if let Some(state) = self.publication_state(publisher).await?
            && state.status != "reverted"
        {
            return Ok(state.preparation);
        }
        let mut conn = self.pool.acquire().await?;
        let view = chain_state::for_decision(&mut conn).await?;
        refuse_before_snapshot(&mut conn, &view, id, signer).await?;
        let mut snapshot: Value = sqlx::query_scalar(SNAPSHOT_QUERY)
            .bind(id)
            .bind(signer)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| ApiError::not_found("Collection not found"))?;
        mark_missing_bytes(&self.blobs, &mut snapshot).await?;
        prepare(snapshot, signer, &self.network)
    }
}

/// The most items a standard publication takes.
pub(crate) const MAX_ITEMS: usize = 50;

/// Refuses, before its snapshot loads item documents, probes the store or
/// locks item rows: an absent collection of `signer`'s, a published one,
/// and one holding no item or more than [`MAX_ITEMS`].
pub(super) async fn refuse_before_snapshot(
    conn: &mut sqlx::PgConnection,
    view: &chain_state::DecisionView,
    id: uuid::Uuid,
    signer: &str,
) -> Result<(), ApiError> {
    let items: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM (SELECT 1 FROM builder.items WHERE collection_id = c.id LIMIT $3) i) \
         FROM builder.collections c WHERE c.id = $1 AND c.eth_address = $2",
    )
    .bind(id)
    .bind(signer)
    .bind(MAX_ITEMS as i64 + 1)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    if chain_state::published(conn, view, id).await? {
        return Err(already_published());
    }
    if !(1..=MAX_ITEMS as i64).contains(&items) {
        return Err(outside_item_range());
    }
    Ok(())
}

fn outside_item_range() -> ApiError {
    ApiError::bad_request("Publish a collection containing 1 to 50 items")
}

/// Marks `missing_content` on each item of `snapshot` that names a hash
/// the store holds no bytes for.
pub(super) async fn mark_missing_bytes(
    blobs: &crate::storage::Storage,
    snapshot: &mut Value,
) -> Result<(), ApiError> {
    let mut held = std::collections::HashMap::new();
    for item in snapshot["items"].as_array_mut().into_iter().flatten() {
        let hashes: Vec<String> = item["contents"]
            .as_object()
            .into_iter()
            .flat_map(|contents| contents.values())
            .filter_map(|hash| hash.as_str().map(str::to_string))
            .collect();
        for hash in hashes {
            let stored = match held.get(&hash) {
                Some(stored) => *stored,
                None => {
                    let stored = blobs
                        .exist(&hash)
                        .await
                        .map_err(|e| ApiError::internal(e.to_string()))?;
                    held.insert(hash, stored);
                    stored
                }
            };
            if !stored {
                item["missing_content"] = Value::Bool(true);
                break;
            }
        }
    }
    Ok(())
}

fn already_published() -> ApiError {
    ApiError::bad_request("Collection has already been published")
}

pub(super) fn prepare(
    snapshot: Value,
    signer: &str,
    network: &Network,
) -> Result<PublicationPreparation, ApiError> {
    let collection = &snapshot["collection"];
    let id = text(collection, "id")?;
    let name = text(collection, "name")?;
    if !collection["third_party_id"].is_null() {
        return Err(ApiError::bad_request(
            "Linked collections use their registered provider's publication flow",
        ));
    }
    if name.trim().is_empty() || name.encode_utf16().count() > 42 {
        return Err(ApiError::bad_request(
            "Collection name must contain 1 to 42 characters",
        ));
    }
    let creator: Address = signer
        .parse()
        .map_err(|_| ApiError::bad_request("Invalid creator address"))?;
    if !text(collection, "eth_address")?.eq_ignore_ascii_case(signer) {
        return Err(ApiError::forbidden("Collection belongs to another wallet"));
    }
    let salt = match collection["salt"].as_str().filter(|s| !s.is_empty()) {
        Some(salt) => salt
            .parse::<B256>()
            .map_err(|_| ApiError::bad_request("Collection salt must be 32 bytes"))?,
        None => keccak256(id.as_bytes()),
    };
    let rows = snapshot["items"]
        .as_array()
        .ok_or_else(|| ApiError::bad_request("Collection items are missing"))?;
    if rows.is_empty() || rows.len() > MAX_ITEMS {
        return Err(outside_item_range());
    }
    let items = rows
        .iter()
        .map(|row| prepare_item(row, signer))
        .collect::<Result<_, _>>()?;
    Ok(PublicationPreparation {
        id: id.into(),
        revision: hash_bytes_v1(snapshot.to_string().as_bytes()),
        chain_id: u32::try_from(network.chain_id)
            .map_err(|_| ApiError::internal("the network's chain id is past u32"))?,
        manager: network.collection_manager.into(),
        factory: network.collection_factory.into(),
        forwarder: network.forwarder.into(),
        salt: format!("{salt:#x}"),
        name: name.into(),
        symbol: format!(
            "DCL-{}",
            name.chars()
                .filter(|c| !c.is_whitespace() && !"aeiou".contains(*c))
                .collect::<String>()
                .to_uppercase()
        ),
        base_uri: network.base_uri.into(),
        creator: format!("{creator:#x}"),
        items,
    })
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, ApiError> {
    value[field]
        .as_str()
        .ok_or_else(|| ApiError::bad_request(format!("Missing {field}")))
}

fn prepare_item(row: &Value, signer: &str) -> Result<PublicationItem, ApiError> {
    let item: ItemDraft = serde_json::from_value(row.clone())
        .map_err(|_| ApiError::bad_request("Invalid collection item"))?;
    item.validate(item.id)?;
    crate::drafts::owned_by(item.eth_address.as_deref(), signer)?;
    let invalid = |reason: &str| ApiError::bad_request(format!("{}: {reason}", item.name));
    if row["missing_content"] == true {
        return Err(invalid("upload missing content before publishing"));
    }
    if item.name.encode_utf16().count() > 32 || item.description.encode_utf16().count() > 64 {
        return Err(invalid(
            "publication requires a name of at most 32 characters and a description of at most 64 characters",
        ));
    }
    if item.name.contains(':') || item.description.contains(':') {
        return Err(invalid("name and description cannot contain colons"));
    }
    let rarity = item
        .rarity
        .as_ref()
        .ok_or_else(|| invalid("choose a rarity"))?;
    let price = item
        .price
        .as_ref()
        .ok_or_else(|| invalid("set an item price"))?;
    let price = U256::from_str_radix(price, 10).map_err(|_| invalid("price exceeds uint256"))?;
    let beneficiary: Address = (if price == U256::ZERO {
        "0x0000000000000000000000000000000000000000"
    } else {
        item.beneficiary.as_deref().unwrap_or(signer)
    })
    .parse()
    .map_err(|_| invalid("invalid beneficiary"))?;
    if price != U256::ZERO && beneficiary == Address::ZERO {
        return Err(invalid("a priced item needs a beneficiary"));
    }
    let thumbnail = item
        .thumbnail
        .as_deref()
        .ok_or_else(|| invalid("add a thumbnail"))?;
    if !item.contents.contains_key(thumbnail) {
        return Err(invalid("upload the thumbnail before publishing"));
    }
    let category = item.data["category"].as_str().unwrap_or_default();
    let categories = if item.item_type == "emote" {
        crate::items::schema::EMOTE_CATEGORIES
    } else {
        crate::items::schema::WEARABLE_CATEGORIES
    };
    if !categories.contains(&category) {
        return Err(invalid("choose a valid category"));
    }
    let reps = item.data["representations"]
        .as_array()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| invalid("add an avatar representation"))?;
    let mut shapes = Vec::new();
    for rep in reps {
        let files = rep["contents"]
            .as_array()
            .filter(|f| !f.is_empty())
            .ok_or_else(|| invalid("representation content is missing"))?;
        let main = rep["mainFile"]
            .as_str()
            .ok_or_else(|| invalid("representation main file is missing"))?;
        if !files.iter().any(|f| f.as_str() == Some(main))
            || files
                .iter()
                .any(|f| f.as_str().is_none_or(|f| !item.contents.contains_key(f)))
        {
            return Err(invalid("upload all representation files before publishing"));
        }
        let bodies = rep["bodyShapes"]
            .as_array()
            .filter(|b| !b.is_empty())
            .ok_or_else(|| invalid("choose avatar body shapes"))?;
        for body in bodies {
            let shape = match body.as_str() {
                Some("urn:decentraland:off-chain:base-avatars:BaseMale") => "BaseMale",
                Some("urn:decentraland:off-chain:base-avatars:BaseFemale") => "BaseFemale",
                _ => return Err(invalid("unsupported avatar body shape")),
            };
            if !shapes.contains(&shape) {
                shapes.push(shape);
            }
        }
    }
    let kind = if item.item_type == "emote" {
        "e"
    } else if item.contents.keys().any(|p| p.ends_with(".js")) {
        "sw"
    } else {
        "w"
    };
    let mut metadata = format!(
        "1:{kind}:{}:{}:{category}:{}",
        item.name,
        item.description,
        shapes.join(",")
    );
    if kind == "e" {
        let looped = item.data["loop"]
            .as_bool()
            .ok_or_else(|| invalid("choose an emote play mode"))?;
        metadata.push_str(if looped { ":1" } else { ":0" });
        let mut props = String::new();
        if item.contents.keys().any(|p| p.contains(".mp3")) {
            props.push('s');
        }
        if row["metrics"]["props"].as_u64().unwrap_or(0) > 0 {
            props.push('g');
        }
        if !props.is_empty() {
            metadata.push(':');
            metadata.push_str(&props);
        }
        if let Some(outcomes) = item.data["outcomes"].as_array().filter(|a| !a.is_empty()) {
            metadata.push_str(if outcomes.len() == 1 {
                ":so"
            } else if item.data["randomizeOutcomes"] == true {
                ":ro"
            } else {
                ":mo"
            });
        }
    }
    Ok(PublicationItem {
        id: item.id.to_string(),
        rarity: rarity.clone(),
        price: price.to_string(),
        beneficiary: format!("{beneficiary:#x}"),
        metadata,
    })
}

#[cfg(test)]
mod tests;

/// The frozen draft: the collection row and its items in creation order,
/// each with its `contents` map (`items_collection_created_idx`) and
/// `missing_content`: whether it names a file whose name's writer never
/// uploaded it (no `uploaded_contents` row for that hash by the entry's
/// `named_by`, the attribution the file routes read; for `video.mp4`, none
/// the videos route wrote), which every preparation and begin of both
/// publications refuses.
pub(super) const SNAPSHOT_QUERY: &str = "SELECT jsonb_build_object('collection', to_jsonb(c), 'items', \
             COALESCE((SELECT jsonb_agg(to_jsonb(i) || jsonb_build_object('contents', \
               COALESCE((SELECT jsonb_object_agg(ic.file, ic.hash) FROM builder.item_contents ic \
                 WHERE ic.item_id=i.id), '{}'::jsonb), 'missing_content', EXISTS (SELECT 1 \
                 FROM builder.item_contents ic WHERE ic.item_id=i.id AND NOT EXISTS (SELECT 1 \
                 FROM builder.uploaded_contents u WHERE u.hash=ic.hash AND u.uploaded_by=ic.named_by \
                 AND (u.video OR ic.file <> 'video.mp4')))) \
                 ORDER BY i.created_at, i.id) \
               FROM builder.items i WHERE i.collection_id=c.id), '[]'::jsonb)) \
             FROM builder.collections c WHERE c.id=$1 AND c.eth_address=$2";
