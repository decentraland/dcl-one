//! `items-mirror`: a whole walk of the collections subgraphs' `items`
//! (`orderBy: id, where: { id_gt }`, the collections mirror's shape),
//! upserted into `shop.items` (PK `(network, contract_address,
//! item_id)`) every tick: about 13 pages of 1,000 on matic, one on
//! ethereum, some 20 s. Not an `updatedAt` keyset: the collections subgraph
//! does not bump `Item.updatedAt` on a mint (`handleIssue` decrements
//! `available` and increments `totalSupply` without it) or on a store-minter
//! change (`handleSetItemMinter` flips `searchIsStoreMinter` without it), so
//! an incremental walk never re-read those rows and `available`,
//! `totalSupply` and the catalog's `isOnSale` froze at their first-pass
//! values (probed 2026-09-25: the six newest mints on matic sit on items
//! whose `updatedAt` is months older than the mint). The wearable and emote
//! metadata legs are typed columns here, the old four-table join folded,
//! because every catalog filter is on them. The listing-derived projections
//! of the old squid (`min_price`, `max_listing_price`, `min_listing_price`,
//! `listings`, `owners`) come from on-chain orders the subgraph does not
//! carry per item; they stay NULL until the nft indexer.

use crate::shared::now_utc;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::jobs::JobContext;

use super::super::MarketplaceState;
use super::super::graph::Network;
use super::super::readers::{
    at, bool_of, int_of, opt_bool_of, opt_int_of, opt_str_of, str_of, strings_of,
};
use super::super::wire;
use super::{Mirror, Page, walk};

const QUERY: &str = r#"
query items($idFrom: ID!, $first: Int!, $verifyChain: Boolean!, $block: Block_height) {
  items(block: $block, first: $first, orderBy: id, orderDirection: asc, where: { id_gt: $idFrom }) {
    id
    blockchainId
    collection { id }
    creator
    beneficiary
    itemType
    rarity
    totalSupply
    maxSupply
    available
    price
    image
    urn
    createdAt
    updatedAt
    reviewedAt
    soldAt
    firstListedAt
    searchIsStoreMinter
    searchIsCollectionApproved
    searchText
    contentHash
    metadata {
      wearable { name description category bodyShapes }
      emote { name description category bodyShapes loop hasSound hasGeometry }
    }
  }
  _meta(block: $block) @include(if: $verifyChain) { block { number hash } }
}"#;

pub const ITEM_TYPES: [&str; 4] = [
    "wearable_v1",
    "wearable_v2",
    "smart_wearable_v1",
    "emote_v1",
];

pub(super) async fn run_pass(
    state: &MarketplaceState,
    ctx: &JobContext,
    network: Network,
    url: &str,
) -> anyhow::Result<()> {
    let pool = state.pool.clone();
    walk(
        state,
        ctx,
        Mirror::Items,
        network,
        url,
        false,
        |_, id, hash| async move {
            let data = super::read_catalogue_page(state, url, QUERY, id, hash).await?;
            let rows = super::super::readers::rows(&data, "items");
            let last_id = rows
                .last()
                .and_then(|r| r.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(Page {
                rows,
                last_updated_at: None,
                last_id,
            })
        },
        |rows, chain_id, verified| {
            let pool = pool.clone();
            async move { upsert(&pool, network, chain_id, verified, &rows).await }
        },
    )
    .await
}

/// One subgraph item as the `shop.items` row, `None` when it cannot
/// be keyed (no contract, a non-decimal blockchain id, an unknown type).
pub fn record(r: &Value, network: Network) -> Option<Value> {
    let contract = at(r, "collection/id")
        .and_then(Value::as_str)
        .map(str::to_lowercase)
        .or_else(|| str_of(r, "id").split('-').next().map(str::to_lowercase))?;
    if !super::super::is_valid_eth_address(&contract) {
        return None;
    }
    let item_id = str_of(r, "blockchainId");
    if item_id.is_empty() || !item_id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let item_type = str_of(r, "itemType");
    if !ITEM_TYPES.contains(&item_type.as_str()) {
        return None;
    }
    let is_emote = item_type == "emote_v1";
    let meta = if is_emote {
        at(r, "metadata/emote")
    } else {
        at(r, "metadata/wearable")
    };
    let meta = meta.cloned().unwrap_or(Value::Null);
    let urn = {
        let raw = str_of(r, "urn");
        if raw.is_empty() {
            format!(
                "urn:decentraland:{}:collections-v2:{contract}:{item_id}",
                network.db_name()
            )
        } else {
            wire::fix_urn(&raw)
        }
    };
    let numeric = |key: &str| -> String {
        let s = str_of(r, key);
        if s.bytes().all(|b| b.is_ascii_digit()) && !s.is_empty() {
            s
        } else {
            "0".to_string()
        }
    };
    Some(json!({
        "contract_address": contract,
        "item_id": item_id,
        "item_type": item_type,
        "urn": urn,
        "creator": str_of(r, "creator").to_lowercase(),
        "beneficiary": opt_str_of(r, "beneficiary").map(|b| b.to_lowercase()),
        "rarity": str_of(r, "rarity").to_lowercase(),
        "total_supply": numeric("totalSupply"),
        "max_supply": numeric("maxSupply"),
        "available": numeric("available"),
        "price": numeric("price"),
        "image": opt_str_of(r, "image"),
        "name": str_of(&meta, "name"),
        "description": opt_str_of(&meta, "description"),
        "category": str_of(&meta, "category").to_lowercase(),
        "body_shapes": strings_of(&meta, "bodyShapes"),
        "is_smart": item_type == "smart_wearable_v1",
        "emote_loop": if is_emote { opt_bool_of(&meta, "loop") } else { None },
        "emote_has_sound": if is_emote { opt_bool_of(&meta, "hasSound") } else { None },
        "emote_has_geometry": if is_emote { opt_bool_of(&meta, "hasGeometry") } else { None },
        // The production collections subgraphs have no `Emote.outcomeType` (probed 2026-09-25); the column stays NULL from this source.
        "emote_outcome_type": if is_emote { opt_str_of(&meta, "outcomeType") } else { None },
        "search_is_store_minter": bool_of(r, "searchIsStoreMinter"),
        "search_is_collection_approved": bool_of(r, "searchIsCollectionApproved"),
        "created_at": int_of(r, "createdAt"),
        "updated_at": int_of(r, "updatedAt"),
        "reviewed_at": int_of(r, "reviewedAt"),
        "sold_at": int_of(r, "soldAt"),
        "first_listed_at": opt_int_of(r, "firstListedAt"),
        "search_text": opt_str_of(r, "searchText"),
        "content_hash": opt_str_of(r, "contentHash"),
    }))
}

fn recordset(rows: &[Value], network: Network) -> Vec<Value> {
    let mut by_key: std::collections::BTreeMap<(String, String), Value> =
        std::collections::BTreeMap::new();
    let mut skipped = 0usize;
    for r in rows {
        match record(r, network) {
            Some(rec) => {
                let key = (
                    rec["contract_address"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    rec["item_id"].as_str().unwrap_or_default().to_string(),
                );
                by_key.insert(key, rec);
            }
            None => skipped += 1,
        }
    }
    if skipped > 0 {
        tracing::warn!(
            skipped,
            "items mirror: rows without a key or with an unknown item type skipped"
        );
    }
    by_key.into_values().collect()
}

pub async fn upsert(
    pool: &PgPool,
    network: Network,
    chain_id: i32,
    verified: bool,
    rows: &[Value],
) -> Result<i64, sqlx::Error> {
    let records = recordset(rows, network);
    if records.is_empty() {
        return Ok(0);
    }
    // The collections index's fold may own the network's rows: the page is
    // written only while it does not, under the network's catalogue lock.
    let net = super::super::chain_walk::ChainNet::parse(network.db_name())
        .expect("a mirrored network is a chain network");
    let mut tx = pool.begin().await?;
    super::super::collections_projection::lock(&mut tx, net, false).await?;
    if super::super::collections_projection::owned(&mut tx, net).await? {
        tracing::warn!(
            network = network.db_name(),
            "items mirror: the collections index owns this network's rows; the page is not written"
        );
        return Ok(0);
    }
    let done = sqlx::query(
        "INSERT INTO shop.items \
             (network, contract_address, item_id, chain_id, item_type, urn, creator, beneficiary, rarity, total_supply, \
              max_supply, available, price, image, name, description, category, body_shapes, is_smart, emote_loop, \
              emote_has_sound, emote_has_geometry, emote_outcome_type, search_is_store_minter, \
              search_is_collection_approved, created_at, updated_at, reviewed_at, sold_at, first_listed_at, mirrored_at, \
              search_text, content_hash, testnet_verified) \
         SELECT $1, r.contract_address, r.item_id::numeric, $2, r.item_type, r.urn, r.creator, r.beneficiary, r.rarity, \
                r.total_supply::numeric, r.max_supply::numeric, r.available::numeric, r.price::numeric, r.image, r.name, \
                r.description, r.category, r.body_shapes, r.is_smart, r.emote_loop, r.emote_has_sound, \
                r.emote_has_geometry, r.emote_outcome_type, r.search_is_store_minter, r.search_is_collection_approved, \
                r.created_at, r.updated_at, r.reviewed_at, r.sold_at, r.first_listed_at, $3, r.search_text, r.content_hash, $5 \
         FROM jsonb_to_recordset($4) AS r(contract_address text, item_id text, item_type text, urn text, creator text, \
              beneficiary text, rarity text, total_supply text, max_supply text, available text, price text, image text, \
              name text, description text, category text, body_shapes text[], is_smart boolean, emote_loop boolean, \
              emote_has_sound boolean, emote_has_geometry boolean, emote_outcome_type text, \
              search_is_store_minter boolean, search_is_collection_approved boolean, created_at bigint, \
              updated_at bigint, reviewed_at bigint, sold_at bigint, first_listed_at bigint, search_text text, \
              content_hash text) \
         ON CONFLICT (network, contract_address, item_id) DO UPDATE SET \
             testnet_verified = EXCLUDED.testnet_verified, chain_id = EXCLUDED.chain_id, item_type = EXCLUDED.item_type, urn = EXCLUDED.urn, \
             creator = EXCLUDED.creator, beneficiary = EXCLUDED.beneficiary, rarity = EXCLUDED.rarity, \
             total_supply = EXCLUDED.total_supply, max_supply = EXCLUDED.max_supply, available = EXCLUDED.available, \
             price = EXCLUDED.price, image = EXCLUDED.image, name = EXCLUDED.name, description = EXCLUDED.description, \
             category = EXCLUDED.category, body_shapes = EXCLUDED.body_shapes, is_smart = EXCLUDED.is_smart, \
             emote_loop = EXCLUDED.emote_loop, emote_has_sound = EXCLUDED.emote_has_sound, \
             emote_has_geometry = EXCLUDED.emote_has_geometry, emote_outcome_type = EXCLUDED.emote_outcome_type, \
             search_is_store_minter = EXCLUDED.search_is_store_minter, \
             search_is_collection_approved = EXCLUDED.search_is_collection_approved, \
             created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at, reviewed_at = EXCLUDED.reviewed_at, \
             sold_at = EXCLUDED.sold_at, first_listed_at = EXCLUDED.first_listed_at, mirrored_at = EXCLUDED.mirrored_at, \
             search_text = EXCLUDED.search_text, content_hash = EXCLUDED.content_hash",
    )
    .bind(network.db_name())
    .bind(chain_id)
    .bind(now_utc())
    .bind(Value::Array(records))
    .bind(verified)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(done.rows_affected() as i64)
}

#[cfg(test)]
pub(crate) fn fixture(contract: &str, item_id: &str, item_type: &str, name: &str) -> Value {
    let is_emote = item_type == "emote_v1";
    let metadata = if is_emote {
        json!({ "wearable": null, "emote": { "name": name, "description": "an emote", "category": "dance", "bodyShapes": ["BaseMale", "BaseFemale"], "loop": true, "hasSound": false, "hasGeometry": false, "outcomeType": null } })
    } else {
        json!({ "wearable": { "name": name, "description": "a hat", "category": "hat", "bodyShapes": ["BaseMale"] }, "emote": null })
    };
    json!({
        "id": format!("{contract}-{item_id}"), "blockchainId": item_id, "collection": { "id": contract },
        "creator": "0xcccccccccccccccccccccccccccccccccccccccc", "beneficiary": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "itemType": item_type, "rarity": "epic", "totalSupply": "10", "maxSupply": "100", "available": "90", "price": "5000000000000000000",
        "image": format!("https://peer.decentraland.org/lambdas/collections/contents/urn:decentraland:matic:collections-v2:{contract}:{item_id}/thumbnail"),
        "urn": format!("urn:decentraland:matic:collections-v2:{contract}:{item_id}"),
        "createdAt": "1700000000", "updatedAt": "1700000500", "reviewedAt": "1700000100", "soldAt": "1700000400", "firstListedAt": "1700000300",
        "searchIsStoreMinter": true, "searchIsCollectionApproved": true, "searchText": name.to_lowercase(), "metadata": metadata,
        "contentHash": format!("bafkreicontent{item_id}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subgraph_item_becomes_a_keyed_typed_row() {
        let c = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let m = Network::Matic;
        let hat = record(&fixture(c, "7", "smart_wearable_v1", "Hat"), m).unwrap();
        assert_eq!(hat["contract_address"], c);
        assert_eq!(hat["item_id"], "7");
        assert_eq!(hat["name"], "Hat");
        assert_eq!(hat["search_text"], "hat");
        assert_eq!(hat["category"], "hat");
        assert_eq!(hat["is_smart"], true);
        assert_eq!(hat["emote_loop"], Value::Null);
        assert_eq!(hat["body_shapes"], json!(["BaseMale"]));
        assert_eq!(hat["price"], "5000000000000000000");
        assert_eq!(hat["first_listed_at"], 1_700_000_300_i64);
        assert_eq!(hat["content_hash"], "bafkreicontent7");
        let dance = record(&fixture(c, "8", "emote_v1", "Dance"), m).unwrap();
        assert_eq!(dance["category"], "dance");
        assert_eq!(dance["emote_loop"], true);
        assert_eq!(dance["emote_has_sound"], false);
        assert_eq!(dance["is_smart"], false);
        assert!(
            record(&fixture(c, "x", "wearable_v2", "Bad"), m).is_none(),
            "a non-decimal blockchain id is not a key"
        );
        assert!(
            record(&fixture(c, "1", "wearable_v9", "Bad"), m).is_none(),
            "an unknown item type is refused by the CHECK, so it is skipped"
        );
        assert!(
            record(
                &json!({ "id": "nope-1", "blockchainId": "1", "itemType": "wearable_v2" }),
                m
            )
            .is_none()
        );
        let rows = recordset(
            &[
                fixture(c, "1", "wearable_v1", "A"),
                fixture(c, "1", "wearable_v1", "B"),
            ],
            m,
        );
        assert_eq!(
            rows.len(),
            1,
            "one page carrying an item twice keeps one row"
        );
        assert_eq!(rows[0]["name"], "B");
        let mut unhashed = fixture(c, "9", "wearable_v2", "Unhashed");
        unhashed["contentHash"] = Value::Null;
        assert_eq!(record(&unhashed, m).unwrap()["content_hash"], Value::Null);
    }

    #[tokio::test]
    async fn the_items_copy_keeps_the_on_chain_content_hash() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let contract = format!(
            "0x{}",
            uuid::Uuid::new_v4().simple().to_string()[..20].repeat(2)
        );
        upsert(
            &pool,
            Network::Matic,
            137,
            false,
            &[fixture(&contract, "3", "wearable_v2", "Hat")],
        )
        .await
        .unwrap();
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT content_hash FROM shop.items WHERE network = 'matic' AND contract_address = $1",
        )
        .bind(&contract)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM shop.items WHERE contract_address = $1")
            .bind(&contract)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(stored.as_deref(), Some("bafkreicontent3"));
    }
}
