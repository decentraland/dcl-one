//! The `collections` index on Ethereum: the 42 collections-v1 contracts.
//!
//! Ethereum holds no collections v2. Its catalogue is the closed set of v1
//! collections (`catalyst_content::collections_v1`), each an
//! `ERC721BaseCollection` whose items are added by `AddWearable` (a wearable
//! id and its maximum issuance) and minted by `Issue`. The walk reads them
//! with a static filter, from the first one's deployment, into
//! `shop.collection_event` under the shared kinds:
//! - `AddWearable`: `add_item`, the wearable id in `metadata` and the
//!   maximum issuance in `max_supply` (no item id: items are numbered by the
//!   fold);
//! - `Issue`: `issue`, the wearable id in `metadata`, the issued id in
//!   `amount` and the beneficiary;
//! - `OwnershipTransferred`: `ownership_transferred`.
//!
//! The fold follows the collections subgraph's v1 handlers
//! (`decentraland/collections-graph` `handleAddItemV1`,
//! `handleTransferWearableV1`):
//! - a collection has a row from its first `AddWearable`: its static name
//!   and symbol, its owner at that block as owner and creator, completed and
//!   approved, never editable, its URN from the name;
//! - each `AddWearable` is the collection's next item (numbered from 1): the
//!   owner at its block as creator, the static representation of its
//!   wearable id ([`catalyst_content::collections_v1::representation`]) for
//!   the rarity, name, description, category and body shapes, its maximum
//!   issuance, no price and no beneficiary;
//! - an item's supply is its count of `Issue` logs (each mint emits one);
//!   `available` is the rest of the maximum.
//!
//! An `AddWearable` whose wearable id no static representation has cannot
//! be described: the item is left out of the catalogue, counted
//! ([`Projected::unrepresented`] and
//! `catalyst_index_collections_v1_unrepresented_items`) and logged. The
//! collection still counts it.
//!
//! The projection is the Polygon one's ([`super::collections_projection`]):
//! the same ownership marker, catalogue lock and publication hook. With so
//! few rows, a publication rebuilds the network's rows when an event was
//! walked past the block they stand at, and writes nothing otherwise; a
//! walk's first publication (each process or lease) rebuilds them whatever
//! was walked, so a change to the fold or to the vendored data reaches them.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};

use alloy_primitives::{B256, U256, hex};
use alloy_sol_types::{SolEvent, sol};
use anyhow::{Context, ensure};
use dcl_metrics::{Descriptor, GaugeFamily};
use serde_json::json;
use sqlx::{PgConnection, PgPool, Row};

use super::chain_walk::{self, ChainIndex, ChainNet, INDEXER_COLLECTIONS, LogGroup, Owner};
use super::collections_fold::{CollectionRow, IMAGE_BASE, ItemRow};
use super::collections_index::{
    CollectionEvent, KIND_ADD_ITEM, KIND_ISSUE, KIND_OWNERSHIP_TRANSFERRED, address, insert_events,
    text,
};
use super::collections_projection::Projected;
use crate::chain_index::rpc::RpcLog;
use crate::content::collections_v1::{self as catalogue, Collection};

/// The block the first v1 collection (`dcl://halloween_2019`) was deployed
/// in: no v1 collection has code before it (the collections subgraph's
/// `config/mainnet.json` starts its v1 templates here too).
pub const START_BLOCK: u64 = 8_828_687;

/// A v1 item's type.
pub const WEARABLE_V1: &str = "wearable_v1";

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

sol! {
    interface CollectionV1 {
        event AddWearable(bytes32 indexed _wearableIdKey, string _wearableId, uint256 _maxIssuance);
        event Issue(address indexed _beneficiary, uint256 indexed _tokenId, bytes32 indexed _wearableIdKey, string _wearableId, uint256 _issuedId);
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    }
}

/// The topics walked on every v1 collection.
pub const TOPICS: [B256; 3] = [
    CollectionV1::AddWearable::SIGNATURE_HASH,
    CollectionV1::Issue::SIGNATURE_HASH,
    CollectionV1::OwnershipTransferred::SIGNATURE_HASH,
];

/// Items the last projection left out for want of a static representation.
const UNREPRESENTED: GaugeFamily<0> = GaugeFamily::new(
    "catalyst_index_collections_v1_unrepresented_items",
    "Ethereum v1 items the collections projection left out: no static representation of their wearable id.",
    None,
    &[],
);

/// The v1 walk's families, declared for the index process's registry.
pub const FAMILIES: &[Descriptor] = &[UNREPRESENTED.descriptor];

/// One v1 log as a row: `Ok(None)` for a topic the index does not walk, an
/// error for a log of a walked topic that does not decode or that a
/// contract outside the catalogue emitted.
pub fn decode_log(log: &RpcLog, block_time: i64) -> anyhow::Result<Option<CollectionEvent>> {
    let (block, index) = chain_walk::log_position(log);
    ensure!(
        block >= 0 && block != i64::MAX && (0..=i64::from(i32::MAX)).contains(&index),
        "collections ethereum log without a valid position"
    );
    let topics = log
        .topics
        .iter()
        .map(|t| t.parse::<B256>())
        .collect::<Result<Vec<_>, _>>()
        .context("collections ethereum log topic")?;
    let Some(topic0) = topics.first().copied() else {
        return Ok(None);
    };
    if !TOPICS.contains(&topic0) {
        return Ok(None);
    }
    let emitter = log.address.to_ascii_lowercase();
    ensure!(
        catalogue::collection(&emitter).is_some(),
        "collections ethereum: block {block} log {index} from {emitter}, not a v1 collection"
    );
    let data =
        hex::decode(log.data.trim_start_matches("0x")).context("collections ethereum log data")?;
    let decoded = |what: &'static str| {
        move |e: alloy_sol_types::Error| anyhow::anyhow!("{what} at block {block} log {index}: {e}")
    };
    let base = |kind: &'static str| CollectionEvent {
        block_number: block,
        log_index: index as i32,
        block_time,
        kind,
        contract_address: emitter.clone(),
        ..Default::default()
    };
    Ok(Some(match topic0 {
        CollectionV1::AddWearable::SIGNATURE_HASH => {
            let e = CollectionV1::AddWearable::decode_raw_log(topics, &data)
                .map_err(decoded("AddWearable"))?;
            CollectionEvent {
                metadata: Some(text(e._wearableId)),
                max_supply: Some(e._maxIssuance.to_string()),
                ..base(KIND_ADD_ITEM)
            }
        }
        CollectionV1::Issue::SIGNATURE_HASH => {
            let e = CollectionV1::Issue::decode_raw_log(topics, &data).map_err(decoded("Issue"))?;
            CollectionEvent {
                metadata: Some(text(e._wearableId)),
                amount: Some(e._issuedId.to_string()),
                beneficiary: Some(address(e._beneficiary)),
                ..base(KIND_ISSUE)
            }
        }
        _ => {
            let e = CollectionV1::OwnershipTransferred::decode_raw_log(topics, &data)
                .map_err(decoded("OwnershipTransferred"))?;
            CollectionEvent {
                account: Some(address(e.newOwner)),
                previous_account: Some(address(e.previousOwner)),
                ..base(KIND_OWNERSHIP_TRANSFERRED)
            }
        }
    }))
}

/// The filter of every window: the 42 contracts and the walked topics.
pub fn group() -> LogGroup {
    let mut addresses: Vec<String> = catalogue::collections()
        .iter()
        .map(|c| c.address.clone())
        .collect();
    addresses.sort();
    LogGroup {
        addresses,
        topics: json!([TOPICS.iter().map(|t| format!("{t:#x}")).collect::<Vec<_>>()]),
    }
}

/// One collection folded: its row, its items' rows and the wearable ids of
/// the `AddWearable`s it left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folded {
    pub collection: CollectionRow,
    pub items: Vec<ItemRow>,
    pub unrepresented: Vec<String>,
}

/// `collection` folded from its `add_item` and `ownership_transferred`
/// events (in chain order) and its mints by wearable id; `None` before its
/// first `AddWearable`.
pub fn fold(
    collection: &Collection,
    events: &[CollectionEvent],
    issued: &HashMap<String, u64>,
) -> Option<Folded> {
    // The owner as the subgraph's `owner()` call at an event's block read
    // it: after the whole block.
    let owner_at = |block: i64| {
        events
            .iter()
            .filter(|e| e.kind == KIND_OWNERSHIP_TRANSFERRED && e.block_number <= block)
            .filter_map(|e| e.account.clone())
            .next_back()
            .unwrap_or_else(|| ZERO_ADDRESS.to_string())
    };
    let added: Vec<&CollectionEvent> = events.iter().filter(|e| e.kind == KIND_ADD_ITEM).collect();
    let first = added.first()?;
    let collection_urn = format!(
        "urn:decentraland:ethereum:collections-v1:{}",
        collection.urn_name()
    );
    let owner = owner_at(first.block_number);
    let mut items = Vec::new();
    let mut unrepresented = Vec::new();
    for (n, add) in added.iter().enumerate() {
        let wearable_id = add.metadata.clone().unwrap_or_default();
        let Some(rep) = catalogue::representation(&wearable_id) else {
            unrepresented.push(wearable_id);
            continue;
        };
        let max_supply = add
            .max_supply
            .as_deref()
            .and_then(|m| U256::from_str_radix(m, 10).ok())
            .unwrap_or_default();
        let total_supply = U256::from(issued.get(&wearable_id).copied().unwrap_or(0));
        let urn = format!("{collection_urn}:{}", rep.id);
        items.push(ItemRow {
            item_id: U256::from(n + 1),
            item_type: WEARABLE_V1,
            image: format!("{IMAGE_BASE}/{urn}/thumbnail"),
            urn,
            creator: owner_at(add.block_number),
            beneficiary: ZERO_ADDRESS.to_string(),
            rarity: rep.rarity.clone(),
            total_supply,
            max_supply,
            available: max_supply.saturating_sub(total_supply),
            price: U256::ZERO,
            name: rep.name.clone(),
            description: rep.description.clone(),
            category: rep.category.clone(),
            body_shapes: rep.body_shapes.clone(),
            is_smart: false,
            emote_loop: None,
            emote_has_sound: None,
            emote_has_geometry: None,
            emote_outcome_type: None,
            minters: Vec::new(),
            minter_allowances: Vec::new(),
            search_is_store_minter: false,
            search_is_collection_approved: true,
            search_text: format!("{} {}", rep.name, rep.description).to_ascii_lowercase(),
            created_at: add.block_time,
            updated_at: add.block_time,
            reviewed_at: add.block_time,
            sold_at: 0,
            first_listed_at: None,
        });
    }
    Some(Folded {
        collection: CollectionRow {
            contract_address: collection.address.clone(),
            name: collection.name.clone(),
            symbol: collection.symbol.clone(),
            creator: owner.clone(),
            owner,
            urn: collection_urn,
            is_completed: true,
            is_approved: true,
            is_editable: false,
            items_count: added.len() as i32,
            minters: Vec::new(),
            managers: Vec::new(),
            search_is_store_minter: false,
            search_text: collection.name.to_ascii_lowercase(),
            created_at: first.block_time,
            updated_at: first.block_time,
            reviewed_at: first.block_time,
            first_listed_at: None,
        },
        items,
        unrepresented,
    })
}

/// Folds Ethereum's log to `upto` into its catalogue rows, in the
/// projection's transaction (under its exclusive catalogue lock, with
/// `projected_through` read as `through`): every row again when an event
/// was walked past `through` (or none stands), nothing otherwise.
pub async fn project_in(
    conn: &mut PgConnection,
    through: Option<i64>,
    upto: i64,
) -> anyhow::Result<Projected> {
    let net = ChainNet::Ethereum;
    if let Some(through) = through.filter(|t| *t <= upto) {
        let moved: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM shop.collection_event \
             WHERE network = $1 AND block_number > $2 AND block_number <= $3)",
        )
        .bind(net.db_name())
        .bind(through)
        .bind(upto)
        .fetch_one(&mut *conn)
        .await?;
        if !moved {
            return Ok(Projected::default());
        }
    }
    let rows = sqlx::query(
        "SELECT block_number, log_index, block_time, kind, contract_address, account, max_supply::text AS max_supply, \
                metadata \
         FROM shop.collection_event \
         WHERE network = $1 AND kind IN ('add_item', 'ownership_transferred') AND block_number <= $2 \
         ORDER BY contract_address, block_number, log_index",
    )
    .bind(net.db_name())
    .bind(upto)
    .fetch_all(&mut *conn)
    .await?;
    let mut events: BTreeMap<String, Vec<CollectionEvent>> = BTreeMap::new();
    for row in &rows {
        let kind: String = row.get("kind");
        let event = CollectionEvent {
            block_number: row.get("block_number"),
            log_index: row.get("log_index"),
            block_time: row.get("block_time"),
            kind: if kind == KIND_ADD_ITEM {
                KIND_ADD_ITEM
            } else {
                KIND_OWNERSHIP_TRANSFERRED
            },
            contract_address: row.get("contract_address"),
            account: row.get("account"),
            max_supply: row.get("max_supply"),
            metadata: row.get("metadata"),
            ..Default::default()
        };
        events
            .entry(event.contract_address.clone())
            .or_default()
            .push(event);
    }
    let mut issued: HashMap<String, HashMap<String, u64>> = HashMap::new();
    for row in sqlx::query(
        "SELECT contract_address, metadata, count(*) AS issued FROM shop.collection_event \
         WHERE network = $1 AND kind = 'issue' AND block_number <= $2 \
         GROUP BY contract_address, metadata",
    )
    .bind(net.db_name())
    .bind(upto)
    .fetch_all(&mut *conn)
    .await?
    {
        let count: i64 = row.get("issued");
        issued
            .entry(row.get("contract_address"))
            .or_default()
            .insert(
                row.get::<Option<String>, _>("metadata").unwrap_or_default(),
                count as u64,
            );
    }
    let none = HashMap::new();
    let mut folded = Vec::new();
    let mut unrepresented = 0;
    for collection in catalogue::collections() {
        let Some(f) = fold(
            collection,
            events.get(&collection.address).map_or(&[], Vec::as_slice),
            issued.get(&collection.address).unwrap_or(&none),
        ) else {
            continue;
        };
        for id in &f.unrepresented {
            tracing::warn!(
                collection = %collection.address,
                wearable = %id,
                "collections ethereum: an AddWearable no static representation describes is left out"
            );
        }
        unrepresented += f.unrepresented.len();
        folded.push((f.collection, f.items));
    }
    sqlx::query("DELETE FROM shop.items WHERE network = $1")
        .bind(net.db_name())
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM shop.collections WHERE network = $1")
        .bind(net.db_name())
        .execute(&mut *conn)
        .await?;
    super::collections_projection::write(conn, net, &folded).await?;
    UNREPRESENTED.handle([]).set(unrepresented as f64);
    Ok(Projected {
        rebuilt: true,
        collections: folded.len(),
        replayed: folded.len(),
        waiting: 0,
        unrepresented,
    })
}

/// The Ethereum walk of the v1 collections.
#[derive(Default)]
pub struct CollectionsV1Index {
    /// Whether its publication projects the log into `shop.collections`
    /// and `shop.items` (`MARKETPLACE_COLLECTIONS_INDEX_OWNS` lists
    /// `ethereum`); otherwise it keeps the log alone.
    project: bool,
    /// Whether this instance's publications already rebuilt the rows once.
    /// The first one rebuilds them whatever was walked, so a change to the
    /// fold or to the vendored data reaches them with the next process (or
    /// lease), and the unrepresented gauge is set again.
    rebuilt: AtomicBool,
}

impl CollectionsV1Index {
    /// The walk, projecting the log into the catalogue when `project`.
    pub fn projecting(project: bool) -> Self {
        Self {
            project,
            ..Self::default()
        }
    }
}

#[async_trait::async_trait]
impl ChainIndex for CollectionsV1Index {
    fn indexer(&self) -> &'static str {
        INDEXER_COLLECTIONS
    }

    fn net(&self) -> ChainNet {
        ChainNet::Ethereum
    }

    fn start_block(&self) -> u64 {
        START_BLOCK
    }

    /// A pass that failed may not have committed its publication: the next
    /// one rebuilds the rows again.
    fn abandon(&self) {
        self.rebuilt.store(false, Ordering::SeqCst);
    }

    async fn groups(
        &self,
        _pool: &PgPool,
        _from: u64,
        _to: u64,
        _anchor: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        Ok(vec![group()])
    }

    async fn write(
        &self,
        conn: &mut PgConnection,
        logs: &[RpcLog],
        times: &HashMap<i64, i64>,
    ) -> anyhow::Result<usize> {
        let mut rows = Vec::with_capacity(logs.len());
        for log in logs {
            let block = chain_walk::log_position(log).0;
            let time = times.get(&block).copied().with_context(|| {
                format!("collections ethereum: no block time for block {block}")
            })?;
            if let Some(row) = decode_log(log, time)? {
                rows.push(row);
            }
        }
        insert_events(conn, ChainNet::Ethereum, &rows).await
    }

    /// Deletes the log past `to` and clears the projection's block, so the
    /// next publication rebuilds the rows.
    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM shop.collection_event WHERE network = $1 AND block_number > $2")
            .bind(ChainNet::Ethereum.db_name())
            .bind(to)
            .execute(&mut *conn)
            .await?;
        super::collections_projection::invalidate(conn, ChainNet::Ethereum).await?;
        Ok(())
    }

    /// The log is the fold: the catalogue is projected from it when the
    /// fold is published ([`ChainIndex::publish`]).
    async fn fold(
        &self,
        _pool: &PgPool,
        _since: Option<i64>,
        _upto: i64,
        _owner: &Owner,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    /// Projects the log into the catalogue when this process owns Ethereum's,
    /// in the transaction that publishes the fold at `upto`.
    async fn publish(&self, conn: &mut PgConnection, upto: i64) -> anyhow::Result<()> {
        if self.project {
            if !self.rebuilt.swap(true, Ordering::SeqCst) {
                // In the publishing transaction: no reader sees the block
                // cleared, only the rebuilt rows.
                super::collections_projection::invalidate(conn, ChainNet::Ethereum).await?;
            }
            let out =
                super::collections_projection::project_in(conn, ChainNet::Ethereum, upto).await?;
            if out.rebuilt {
                tracing::info!(
                    upto,
                    collections = out.collections,
                    unrepresented = out.unrepresented,
                    "collections ethereum: projected"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const MEME: &str = "0x1a57f6afc902d25792c53b8f19b7e17ef84222d5";
    const XMASH: &str = "0xdd9c7bc159dacb19c9f6b9d7e23948c87aa2397f";

    fn fixture(name: &str) -> Value {
        let path = format!(
            "{}/services/catalyst/tests/fixtures/shop/{name}",
            env!("DCL_ONE_ROOT")
        );
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    /// The real logs of `dcl://meme_dontbuythis` and `dcl://xmash_up_2020`
    /// through block 26,499,473, decoded with their block times.
    fn walked() -> Vec<CollectionEvent> {
        let f = fixture("collections-ethereum-v1-logs.json");
        let logs: Vec<RpcLog> = serde_json::from_value(f["logs"].clone()).unwrap();
        logs.iter()
            .map(|l| {
                let block = chain_walk::log_position(l).0;
                let time = f["times"][block.to_string()].as_i64().unwrap();
                decode_log(l, time).unwrap().unwrap()
            })
            .collect()
    }

    /// `events` folded per collection, as the projection folds them.
    fn folded(events: &[CollectionEvent]) -> BTreeMap<String, Folded> {
        let mut out = BTreeMap::new();
        for collection in catalogue::collections() {
            let mine: Vec<CollectionEvent> = events
                .iter()
                .filter(|e| e.contract_address == collection.address && e.kind != KIND_ISSUE)
                .cloned()
                .collect();
            let mut issued: HashMap<String, u64> = HashMap::new();
            for e in events
                .iter()
                .filter(|e| e.contract_address == collection.address && e.kind == KIND_ISSUE)
            {
                *issued.entry(e.metadata.clone().unwrap()).or_default() += 1;
            }
            if let Some(f) = fold(collection, &mine, &issued) {
                out.insert(collection.address.clone(), f);
            }
        }
        out
    }

    fn s(v: &Value) -> String {
        v.as_str().unwrap().to_string()
    }

    /// The fold of the real logs is the collections subgraph's rows for the
    /// same collections at the same block, field by field: a collection
    /// never minted, and one whose items sold out or were partly issued.
    #[test]
    fn the_real_logs_fold_to_the_subgraphs_rows() {
        let events = walked();
        let kinds: Vec<&str> = events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds.iter().filter(|k| **k == KIND_ADD_ITEM).count(), 9);
        assert_eq!(kinds.iter().filter(|k| **k == KIND_ISSUE).count(), 48);
        let folded = folded(&events);
        assert_eq!(folded.keys().collect::<Vec<_>>(), [MEME, XMASH]);
        let expected = fixture("collections-ethereum-v1-subgraph.json");
        for c in expected["collections"].as_array().unwrap() {
            let f = &folded[&s(&c["id"])];
            let row = &f.collection;
            assert_eq!(row.name, s(&c["name"]));
            assert_eq!(row.symbol, s(&c["symbol"]));
            assert_eq!(row.owner, s(&c["owner"]));
            assert_eq!(row.creator, s(&c["creator"]));
            assert_eq!(row.urn, s(&c["urn"]));
            assert_eq!(row.search_text, s(&c["searchText"]));
            assert_eq!(row.is_completed, c["isCompleted"].as_bool().unwrap());
            assert_eq!(row.is_approved, c["isApproved"].as_bool().unwrap());
            assert_eq!(row.items_count, c["itemsCount"].as_i64().unwrap() as i32);
            assert_eq!(row.created_at.to_string(), s(&c["createdAt"]));
            assert!(f.unrepresented.is_empty());
        }
        let items = expected["items"].as_array().unwrap();
        assert_eq!(
            items.len(),
            folded.values().map(|f| f.items.len()).sum::<usize>()
        );
        let (mut sold_out, mut partly, mut none) = (0, 0, 0);
        for i in items {
            let f = &folded[&s(&i["collection"]["id"])];
            let row = f
                .items
                .iter()
                .find(|r| r.item_id.to_string() == s(&i["blockchainId"]))
                .unwrap_or_else(|| panic!("no item {}", i["urn"]));
            let w = &i["metadata"]["wearable"];
            assert_eq!(row.urn, s(&i["urn"]));
            assert_eq!(row.item_type, s(&i["itemType"]));
            assert_eq!(
                row.max_supply.to_string(),
                s(&i["maxSupply"]),
                "{}",
                row.urn
            );
            assert_eq!(
                row.total_supply.to_string(),
                s(&i["totalSupply"]),
                "{}",
                row.urn
            );
            assert_eq!(row.available.to_string(), s(&i["available"]), "{}", row.urn);
            assert_eq!(row.rarity, s(&i["rarity"]));
            assert_eq!(row.creator, s(&i["creator"]));
            assert_eq!(row.beneficiary, s(&i["beneficiary"]));
            assert_eq!(row.price.to_string(), s(&i["price"]));
            assert_eq!(row.image, s(&i["image"]));
            assert_eq!(row.search_text, s(&i["searchText"]));
            assert_eq!(row.created_at.to_string(), s(&i["createdAt"]));
            assert_eq!(row.name, s(&w["name"]));
            assert_eq!(row.description, s(&w["description"]));
            assert_eq!(row.category, s(&w["category"]));
            assert_eq!(
                serde_json::to_value(&row.body_shapes).unwrap(),
                w["bodyShapes"]
            );
            match (row.total_supply, row.available) {
                (_, a) if a.is_zero() => sold_out += 1,
                (t, _) if t.is_zero() => none += 1,
                _ => partly += 1,
            }
        }
        assert_eq!((sold_out, partly, none), (3, 2, 4));
    }

    fn add(block: i64, id: &str) -> CollectionEvent {
        CollectionEvent {
            block_number: block,
            block_time: 1_600_000_000 + block,
            kind: KIND_ADD_ITEM,
            contract_address: XMASH.into(),
            metadata: Some(id.into()),
            max_supply: Some("10".into()),
            ..Default::default()
        }
    }

    fn owned_by(block: i64, owner: &str) -> CollectionEvent {
        CollectionEvent {
            block_number: block,
            block_time: 1_600_000_000 + block,
            kind: KIND_OWNERSHIP_TRANSFERRED,
            contract_address: XMASH.into(),
            account: Some(owner.into()),
            ..Default::default()
        }
    }

    /// An `AddWearable` no static representation describes is left out and
    /// reported, though the collection counts it and numbers the next item
    /// after it; the owner at each `AddWearable`'s block (after the whole
    /// block) creates its item, and the first one's owns the collection.
    #[test]
    fn an_undescribed_wearable_is_left_out_and_reported() {
        let (a, b) = (
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b2",
        );
        let collection = catalogue::collection(XMASH).unwrap();
        let events = [
            owned_by(10, a),
            add(11, "xmash_up_cap_hat"),
            add(12, "no_such_wearable"),
            add(13, "xmash_up_boots_feet"),
            owned_by(13, b),
        ];
        let f = fold(
            collection,
            &events,
            &HashMap::from([("xmash_up_cap_hat".into(), 10)]),
        )
        .unwrap();
        assert_eq!(f.unrepresented, ["no_such_wearable"]);
        assert_eq!(f.collection.items_count, 3);
        assert_eq!(
            (f.collection.owner.as_str(), f.collection.creator.as_str()),
            (a, a)
        );
        let ids: Vec<(String, &str, String)> = f
            .items
            .iter()
            .map(|i| {
                (
                    i.item_id.to_string(),
                    i.creator.as_str(),
                    i.available.to_string(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            [
                ("1".to_string(), a, "0".to_string()),
                ("3".to_string(), b, "10".to_string())
            ]
        );
        assert_eq!(
            fold(collection, &[owned_by(10, a)], &HashMap::new()),
            None,
            "no item yet"
        );
    }

    /// Only the walked topics of a v1 collection become rows: a `Transfer`
    /// is skipped, and a walked topic from another contract is refused.
    #[test]
    fn only_a_v1_collections_walked_logs_are_rows() {
        let f = fixture("collections-ethereum-v1-logs.json");
        let mut log: RpcLog = serde_json::from_value(f["logs"][0].clone()).unwrap();
        log.topics[0] = format!(
            "{:#x}",
            alloy_primitives::keccak256("Transfer(address,address,uint256)")
        );
        assert_eq!(decode_log(&log, 1).unwrap(), None);
        let mut other: RpcLog = serde_json::from_value(f["logs"][0].clone()).unwrap();
        other.address = "0x00000000000000000000000000000000000000ff".into();
        let err = decode_log(&other, 1).unwrap_err().to_string();
        assert!(err.contains("not a v1 collection"), "{err}");
        let group = group();
        assert_eq!(group.addresses.len(), 42);
    }

    async fn rows(pool: &PgPool) -> Vec<(String, String, String, String)> {
        sqlx::query_as(
            "SELECT contract_address, item_id::text, total_supply::text, available::text FROM shop.items \
             WHERE network = 'ethereum' ORDER BY contract_address, item_id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn project(pool: &PgPool, upto: i64) -> Projected {
        let mut tx = pool.begin().await.unwrap();
        let out =
            super::super::collections_projection::project_in(&mut tx, ChainNet::Ethereum, upto)
                .await
                .unwrap();
        tx.commit().await.unwrap();
        out
    }

    /// The projection of the walked log: it takes Ethereum's rows, writes
    /// nothing while no event is walked past them, folds a new mint, and
    /// after a rewind rebuilds the rows from what is left.
    #[tokio::test]
    async fn the_projection_follows_the_walked_log() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        // A mirror row the projection replaces.
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, symbol, creator, owner, urn, \
                 is_completed, is_approved, is_editable, items_count, minters, managers, search_is_store_minter, \
                 search_text, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('ethereum', '0xabababababababababababababababababababab', 1, 'mirror', 'M', '0x', '0x', 'urn', \
                 true, true, false, 0, '{}', '{}', false, 'mirror', 1, 1, 1, timezone('UTC', now()))",
        )
        .execute(&pool)
        .await
        .unwrap();
        let events = walked();
        let mut conn = pool.acquire().await.unwrap();
        insert_events(&mut conn, ChainNet::Ethereum, &events)
            .await
            .unwrap();
        drop(conn);
        let upto = 26_499_473;
        let out = project(&pool, upto).await;
        assert_eq!(
            (out.rebuilt, out.collections, out.unrepresented),
            (true, 2, 0)
        );
        let first = rows(&pool).await;
        assert_eq!(first.len(), 9);
        assert!(first.contains(&(XMASH.into(), "3".into(), "9".into(), "1".into())));
        let collections: Vec<String> = sqlx::query_scalar(
            "SELECT contract_address FROM shop.collections WHERE network = 'ethereum' ORDER BY 1",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(collections, [MEME, XMASH], "the mirror's row is gone");
        let (owned, through): (bool, Option<i64>) = sqlx::query_as(
            "SELECT owned_at IS NOT NULL, projected_through FROM shop.collection_projection WHERE network = 'ethereum'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((owned, through), (true, Some(upto)));

        assert_eq!(
            project(&pool, upto + 10).await,
            Projected::default(),
            "nothing walked since"
        );

        let last = events.iter().rev().find(|e| e.kind == KIND_ISSUE).unwrap();
        let mint = CollectionEvent {
            block_number: upto + 20,
            log_index: 0,
            metadata: Some("xmash_up_cap_hat".into()),
            amount: Some("10".into()),
            ..last.clone()
        };
        let mut conn = pool.acquire().await.unwrap();
        insert_events(&mut conn, ChainNet::Ethereum, &[mint])
            .await
            .unwrap();
        drop(conn);
        assert!(project(&pool, upto + 20).await.rebuilt);
        assert!(
            rows(&pool)
                .await
                .contains(&(XMASH.into(), "3".into(), "10".into(), "0".into()))
        );

        let mut tx = pool.begin().await.unwrap();
        CollectionsV1Index::default()
            .rewind(&mut tx, upto)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        // The walk publishes again past the rewound mint's block, with
        // nothing walked there: the rows are rebuilt all the same.
        assert!(project(&pool, upto + 30).await.rebuilt, "a rewind rebuilds");
        assert_eq!(rows(&pool).await, first);
    }

    /// A walk's first publication rebuilds the rows whatever was walked, so a
    /// change to the fold or to the vendored data reaches them with the next
    /// process or lease; later ones write nothing until an event is walked,
    /// and a failed pass makes the next one rebuild again.
    #[tokio::test]
    async fn a_fresh_walk_rebuilds_the_rows_once() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let upto = 26_499_473;
        let mut conn = pool.acquire().await.unwrap();
        insert_events(&mut conn, ChainNet::Ethereum, &walked())
            .await
            .unwrap();
        drop(conn);
        project(&pool, upto).await;
        let name = || async {
            sqlx::query_scalar::<_, String>(
                "SELECT name FROM shop.items WHERE network = 'ethereum' AND contract_address = $1 AND item_id = 1",
            )
            .bind(XMASH)
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        let fresh = name().await;
        let stale = || async {
            sqlx::query(
                "UPDATE shop.items SET name = 'stale' WHERE network = 'ethereum' AND contract_address = $1 AND item_id = 1",
            )
            .bind(XMASH)
            .execute(&pool)
            .await
            .unwrap();
        };
        let ix = CollectionsV1Index::projecting(true);
        let publish = |at: i64| {
            let (pool, ix) = (pool.clone(), &ix);
            async move {
                let mut tx = pool.begin().await.unwrap();
                ix.publish(&mut tx, at).await.unwrap();
                tx.commit().await.unwrap();
            }
        };
        stale().await;
        publish(upto + 1).await;
        assert_eq!(name().await, fresh, "the first publication rebuilds");
        stale().await;
        publish(upto + 2).await;
        assert_eq!(
            name().await,
            "stale",
            "nothing walked since: nothing written"
        );
        ix.abandon();
        publish(upto + 3).await;
        assert_eq!(name().await, fresh, "after a failed pass it rebuilds again");
    }

    /// Once Ethereum is handed to the index, the catalogue routes serve its
    /// projection of the v1 collections, and refuse them while a rewind
    /// withdraws the publication.
    #[tokio::test]
    async fn an_owned_ethereum_is_served_from_the_projection() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use std::sync::Arc;
        use tower::ServiceExt;

        use super::super::catalogue::owning;
        use super::super::{MarketplaceState, Settings, market_router};

        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let upto = 26_499_473;
        let mut conn = pool.acquire().await.unwrap();
        insert_events(&mut conn, ChainNet::Ethereum, &walked())
            .await
            .unwrap();
        drop(conn);
        project(&pool, upto).await;
        owning::own(&pool, "ethereum", 1, upto, 60).await;
        // Polygon too, empty: a priced read needs a source on every network.
        owning::own(&pool, "matic", 137, 100, 60).await;
        // The item reads also join the open trades: local writes are their
        // source, and the trade-events index has published.
        super::super::trade_events::tests::publish_cursors(&pool).await;
        let state = Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            Settings {
                data_writes_enabled: true,
                ..Settings::for_tests()
            },
        ));
        let get = |uri: &'static str| {
            let app = market_router(state.clone());
            async move {
                let resp = app
                    .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                let status = resp.status();
                let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .unwrap();
                (
                    status,
                    serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null),
                )
            }
        };
        const COLLECTIONS: &str =
            "/v1/collections?contractAddress=0xdd9c7bc159dacb19c9f6b9d7e23948c87aa2397f";
        const ITEMS: &str =
            "/v1/items?contractAddress=0xdd9c7bc159dacb19c9f6b9d7e23948c87aa2397f&first=10";
        let (status, body) = get(COLLECTIONS).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"][0]["name"], "dcl://xmash_up_2020", "{body}");
        let (status, body) = get(ITEMS).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let items = body["data"].as_array().unwrap();
        assert_eq!(items.len(), 5, "{body}");
        let hat = items
            .iter()
            .find(|i| {
                i["urn"]
                    == "urn:decentraland:ethereum:collections-v1:xmash_up_2020:xmash_up_cap_hat"
            })
            .unwrap_or_else(|| panic!("{body}"));
        assert_eq!(
            (&hat["rarity"], &hat["available"]),
            (&json!("mythic"), &json!(1)),
            "{hat}"
        );

        owning::rewind(&pool, "ethereum").await;
        for uri in [COLLECTIONS, ITEMS] {
            let (status, body) = get(uri).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
        }
    }

    /// The first walk's cost over a real endpoint, for METRICS.md: run with
    /// `COLLECTIONS_V1_MEASURE_RPC=<Ethereum mainnet RPC> cargo test -p
    /// catalyst-shop --lib collections_v1::tests::measure_the_first_walk --
    /// --ignored --nocapture`. Passes run back to back (a job pauses
    /// `MARKETPLACE_INDEX_INTERVAL_SECS` between them) until the walk
    /// publishes.
    #[tokio::test]
    #[ignore = "walks Ethereum mainnet over a real endpoint"]
    async fn measure_the_first_walk() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let Ok(url) = std::env::var("COLLECTIONS_V1_MEASURE_RPC") else {
            return;
        };
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let http = crate::http::Outbound::for_tests();
        let sent = AtomicUsize::new(0);
        let source = crate::chain_index::logs::RpcSource::new(
            crate::chain_index::rpc::Rpc::at(&http, &url).counting(&sent),
        );
        let ix = CollectionsV1Index::default();
        let settings = chain_walk::IndexSettings::default();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let started = std::time::Instant::now();
        let (mut passes, mut get_logs, mut blocks) = (0, 0, 0);
        loop {
            let out = chain_walk::run_pass(&pool, &source, &ix, &settings, &owner, None, &|_| {})
                .await
                .unwrap();
            passes += 1;
            get_logs += out.requests;
            blocks += out.blocks();
            if out.published {
                break;
            }
            assert!(passes < 100, "no publication after {passes} passes");
        }
        let kinds: Vec<(String, i64)> = sqlx::query_as(
            "SELECT kind, count(*) FROM shop.collection_event WHERE network = 'ethereum' GROUP BY kind ORDER BY kind",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        println!(
            "first walk: {passes} passes, {blocks} blocks, {get_logs} eth_getLogs, {} HTTP requests, {:.1} s; rows {kinds:?}",
            sent.load(Ordering::Relaxed),
            started.elapsed().as_secs_f64()
        );
    }
}
