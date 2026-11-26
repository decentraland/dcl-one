//! The `collections` index on Polygon: the collection factories'
//! `ProxyCreated` logs and the events of every collection they created
//! (items added, rescued and edited, mints, global and item minters and
//! managers, approval, editability, completion, creatorship and ownership),
//! kept in `shop.collection_event` with the cursor advance and deleted past a
//! rewind. It is the native source `shop.collections` and `shop.items` are
//! to be folded from, in place of the collections subgraph; until that
//! projection exists the index only keeps the log, and its publication
//! (`folded_block`) says the log is complete to that block.
//!
//! A window's address filter is fixed before its logs are read. A
//! collection created inside a window emits its `initialize` events right
//! after the factory's `ProxyCreated`, in the same transaction, so taking the
//! filter from the walked `ProxyCreated` logs would miss them. The filter
//! takes the factories' own lists instead (`collections(i)`, read by the gate
//! before each attempt into `shop.collection_hint`), a superset of what any
//! window can create; a window that walks a `ProxyCreated` of an address its
//! filter did not take fails before it commits, and the next attempt reads
//! the lists again. A hint is only a filter: a collection exists for the
//! index once its factory's `ProxyCreated` is walked.
//!
//! `initialize` sets the name, symbol, creator, approval and editability
//! without an event. The gate reads them once per walked collection at the
//! walk's own cursor block, by the hash the walk stored for it
//! (`shop.collection_seed`, with that block), for the projection to take a
//! value from the last walked event that sets it, and from that read only
//! when no event does and the read is at or below the block it folds to.

use std::collections::{HashMap, HashSet};

use alloy_primitives::{Address, B256, Bytes, U256, hex};
use alloy_sol_types::{SolCall, SolEvent, SolValue, sol};
use anyhow::{Context, ensure};
use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{self, ChainIndex, ChainNet, INDEXER_COLLECTIONS, LogGroup, Owner};
use crate::chain_index::rpc::{BlockHeader, Rpc, RpcLog};

/// The collection factories on Polygon (`CollectionFactory` and
/// `CollectionFactoryV3` under `matic` in `evm/addresses.json`) with the
/// block the pinned collections subgraph starts each from
/// (`decentraland/collections-graph` `config/matic.json` at ab6c586).
pub const FACTORIES: [(&str, u64); 2] = [
    ("0xb549b2442b2bd0a53795bc5cdcbfe0caf7aca9f8", 15_202_000),
    ("0x3195e88ae10704b359764cb38e429d24f1c2f781", 28_121_692),
];

/// The first factory's start: a full history begins here.
pub const START_BLOCK: u64 = FACTORIES[0].1;

/// List indexes one refresh reads at most: a size an endpoint misreports
/// costs a bounded read, and a fresh node fills its lists over several
/// attempts.
pub const HINT_READS_PER_REFRESH: usize = 1_000;

/// List indexes read and kept together.
pub const HINT_CHUNK: usize = 200;

/// The largest list size taken as an answer: no factory lists near this
/// many collections, so a larger one is a faulty endpoint's.
pub const MAX_LIST_SIZE: u32 = 1_000_000;

/// What one refresh of the factory lists did: the hints it added or made
/// final, the indexes below the finalized size it left for the next one,
/// and the provisional indexes past it the budget left unread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refreshed {
    pub added: usize,
    pub remaining: usize,
    pub provisional_unread: usize,
}

/// The first key of the advisory lock (with the network's chain id) over the
/// gate's reads of the chain, the factory lists and the creation reads:
/// `read`. A rewind holds it exclusive, a write of those reads shared.
pub const READS_LOCK: i32 = 0x7265_6164;

/// Calls per JSON-RPC batch the gate sends (one HTTP request each).
pub const CALLS_PER_BATCH: usize = 100;

/// Collections whose creation state one gate reads at most; the rest wait
/// for the next attempt, so a fresh node's first gates stay short.
pub const SEEDS_PER_GATE: usize = 500;

/// Heights of final factory list rows one gate asks the endpoint for at
/// most ([`revalidate_reads`]); the rest wait for the next attempts, in turn.
pub const HEIGHTS_PER_GATE: usize = 100;

sol! {
    interface Factory {
        event ProxyCreated(address indexed _address, bytes32 _salt);
        function collectionsSize() external view returns (uint256);
        function collections(uint256 index) external view returns (address);
    }

    interface Collection {
        struct Item {
            string rarity;
            uint256 maxSupply;
            uint256 totalSupply;
            uint256 price;
            address beneficiary;
            string metadata;
            string contentHash;
        }
        event AddItem(uint256 indexed _itemId, Item _item);
        event RescueItem(uint256 indexed _itemId, string _contentHash, string _metadata);
        event UpdateItemData(uint256 indexed _itemId, uint256 _price, address _beneficiary, string _metadata);
        event Issue(address indexed _beneficiary, uint256 indexed _tokenId, uint256 indexed _itemId, uint256 _issuedId, address _caller);
        event SetGlobalMinter(address indexed _minter, bool _value);
        event SetGlobalManager(address indexed _manager, bool _value);
        event SetItemMinter(uint256 indexed _itemId, address indexed _minter, uint256 _value);
        event SetItemManager(uint256 indexed _itemId, address indexed _manager, bool _value);
        event SetApproved(bool _previousValue, bool _newValue);
        event SetEditable(bool _previousValue, bool _newValue);
        event Complete();
        event CreatorshipTransferred(address indexed _previousCreator, address indexed _newCreator);
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
        function name() external view returns (string);
        function symbol() external view returns (string);
        function creator() external view returns (address);
        function isApproved() external view returns (bool);
        function isEditable() external view returns (bool);
    }
}

pub const KIND_PROXY_CREATED: &str = "proxy_created";
pub const KIND_ADD_ITEM: &str = "add_item";
pub const KIND_RESCUE_ITEM: &str = "rescue_item";
pub const KIND_UPDATE_ITEM_DATA: &str = "update_item_data";
pub const KIND_ISSUE: &str = "issue";
pub const KIND_SET_GLOBAL_MINTER: &str = "set_global_minter";
pub const KIND_SET_GLOBAL_MANAGER: &str = "set_global_manager";
pub const KIND_SET_ITEM_MINTER: &str = "set_item_minter";
pub const KIND_SET_ITEM_MANAGER: &str = "set_item_manager";
pub const KIND_SET_APPROVED: &str = "set_approved";
pub const KIND_SET_EDITABLE: &str = "set_editable";
pub const KIND_COMPLETE: &str = "complete";
pub const KIND_CREATORSHIP_TRANSFERRED: &str = "creatorship_transferred";
pub const KIND_OWNERSHIP_TRANSFERRED: &str = "ownership_transferred";

/// The topics walked on every collection, in the event declaration order.
pub const COLLECTION_TOPICS: [B256; 13] = [
    Collection::AddItem::SIGNATURE_HASH,
    Collection::RescueItem::SIGNATURE_HASH,
    Collection::UpdateItemData::SIGNATURE_HASH,
    Collection::Issue::SIGNATURE_HASH,
    Collection::SetGlobalMinter::SIGNATURE_HASH,
    Collection::SetGlobalManager::SIGNATURE_HASH,
    Collection::SetItemMinter::SIGNATURE_HASH,
    Collection::SetItemManager::SIGNATURE_HASH,
    Collection::SetApproved::SIGNATURE_HASH,
    Collection::SetEditable::SIGNATURE_HASH,
    Collection::Complete::SIGNATURE_HASH,
    Collection::CreatorshipTransferred::SIGNATURE_HASH,
    Collection::OwnershipTransferred::SIGNATURE_HASH,
];

/// One row of `shop.collection_event`. `contract_address` is the collection
/// (for `proxy_created`, the one created). The other columns hold what each
/// kind carries and are NULL otherwise:
/// - `account`: the factory of a `proxy_created`; the minter or manager of
///   the four `set_*_minter`/`set_*_manager`; the new creator or owner of a
///   transfer; the caller of an `issue`.
/// - `previous_account`: the previous creator or owner of a transfer.
/// - `flag`: the value of `set_global_minter`, `set_global_manager` and
///   `set_item_manager`; the new value of `set_approved` and `set_editable`.
/// - `amount`: the price of `add_item` and `update_item_data`, the allowance
///   of `set_item_minter`, the issued id of an `issue`.
/// - `max_supply`, `rarity`: `add_item`'s; `beneficiary`: `add_item`'s and
///   `update_item_data`'s; `metadata`: `add_item`'s, `rescue_item`'s and
///   `update_item_data`'s; `content_hash`: `add_item`'s and `rescue_item`'s.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollectionEvent {
    pub block_number: i64,
    pub log_index: i32,
    pub block_time: i64,
    pub kind: &'static str,
    pub contract_address: String,
    pub item_id: Option<String>,
    pub account: Option<String>,
    pub previous_account: Option<String>,
    pub flag: Option<bool>,
    pub amount: Option<String>,
    pub max_supply: Option<String>,
    pub rarity: Option<String>,
    pub beneficiary: Option<String>,
    pub metadata: Option<String>,
    pub content_hash: Option<String>,
}

fn address(a: Address) -> String {
    format!("{a:#x}")
}

/// A string as Postgres `text` stores it: NUL removed (the contracts take
/// any bytes; `decode_raw_log` already read them as lossy UTF-8).
fn text(s: String) -> String {
    if s.contains('\0') {
        s.replace('\0', "")
    } else {
        s
    }
}

/// One log as a row: `Ok(None)` for a topic the index does not walk, an
/// error for a log of a walked topic that does not decode (the factories'
/// collections share one ABI, so it is not data to skip).
pub fn decode_log(log: &RpcLog, block_time: i64) -> anyhow::Result<Option<CollectionEvent>> {
    let (block, index) = chain_walk::log_position(log);
    ensure!(
        block >= 0 && block != i64::MAX && (0..=i64::from(i32::MAX)).contains(&index),
        "collections log without a valid position"
    );
    let topics = log
        .topics
        .iter()
        .map(|t| t.parse::<B256>())
        .collect::<Result<Vec<_>, _>>()
        .context("collections log topic")?;
    let Some(topic0) = topics.first().copied() else {
        return Ok(None);
    };
    let data = hex::decode(log.data.trim_start_matches("0x")).context("collections log data")?;
    let emitter = log.address.to_ascii_lowercase();
    let base = |kind: &'static str| CollectionEvent {
        block_number: block,
        log_index: index as i32,
        block_time,
        kind,
        contract_address: emitter.clone(),
        ..Default::default()
    };
    let decoded = |what: &'static str| {
        move |e: alloy_sol_types::Error| anyhow::anyhow!("{what} at block {block} log {index}: {e}")
    };
    let event = match topic0 {
        Factory::ProxyCreated::SIGNATURE_HASH => {
            ensure!(
                FACTORIES.iter().any(|(f, _)| *f == emitter),
                "ProxyCreated at block {block} log {index} from {emitter}, not a collection factory"
            );
            let e = Factory::ProxyCreated::decode_raw_log(topics, &data)
                .map_err(decoded("ProxyCreated"))?;
            CollectionEvent {
                contract_address: address(e._address),
                account: Some(emitter.clone()),
                ..base(KIND_PROXY_CREATED)
            }
        }
        Collection::AddItem::SIGNATURE_HASH => {
            let e =
                Collection::AddItem::decode_raw_log(topics, &data).map_err(decoded("AddItem"))?;
            CollectionEvent {
                item_id: Some(e._itemId.to_string()),
                amount: Some(e._item.price.to_string()),
                max_supply: Some(e._item.maxSupply.to_string()),
                rarity: Some(text(e._item.rarity)),
                beneficiary: Some(address(e._item.beneficiary)),
                metadata: Some(text(e._item.metadata)),
                content_hash: Some(text(e._item.contentHash)),
                ..base(KIND_ADD_ITEM)
            }
        }
        Collection::RescueItem::SIGNATURE_HASH => {
            let e = Collection::RescueItem::decode_raw_log(topics, &data)
                .map_err(decoded("RescueItem"))?;
            CollectionEvent {
                item_id: Some(e._itemId.to_string()),
                metadata: Some(text(e._metadata)),
                content_hash: Some(text(e._contentHash)),
                ..base(KIND_RESCUE_ITEM)
            }
        }
        Collection::UpdateItemData::SIGNATURE_HASH => {
            let e = Collection::UpdateItemData::decode_raw_log(topics, &data)
                .map_err(decoded("UpdateItemData"))?;
            CollectionEvent {
                item_id: Some(e._itemId.to_string()),
                amount: Some(e._price.to_string()),
                beneficiary: Some(address(e._beneficiary)),
                metadata: Some(text(e._metadata)),
                ..base(KIND_UPDATE_ITEM_DATA)
            }
        }
        Collection::Issue::SIGNATURE_HASH => {
            let e = Collection::Issue::decode_raw_log(topics, &data).map_err(decoded("Issue"))?;
            CollectionEvent {
                item_id: Some(e._itemId.to_string()),
                amount: Some(e._issuedId.to_string()),
                account: Some(address(e._caller)),
                ..base(KIND_ISSUE)
            }
        }
        Collection::SetGlobalMinter::SIGNATURE_HASH => {
            let e = Collection::SetGlobalMinter::decode_raw_log(topics, &data)
                .map_err(decoded("SetGlobalMinter"))?;
            CollectionEvent {
                account: Some(address(e._minter)),
                flag: Some(e._value),
                ..base(KIND_SET_GLOBAL_MINTER)
            }
        }
        Collection::SetGlobalManager::SIGNATURE_HASH => {
            let e = Collection::SetGlobalManager::decode_raw_log(topics, &data)
                .map_err(decoded("SetGlobalManager"))?;
            CollectionEvent {
                account: Some(address(e._manager)),
                flag: Some(e._value),
                ..base(KIND_SET_GLOBAL_MANAGER)
            }
        }
        Collection::SetItemMinter::SIGNATURE_HASH => {
            let e = Collection::SetItemMinter::decode_raw_log(topics, &data)
                .map_err(decoded("SetItemMinter"))?;
            CollectionEvent {
                item_id: Some(e._itemId.to_string()),
                account: Some(address(e._minter)),
                amount: Some(e._value.to_string()),
                ..base(KIND_SET_ITEM_MINTER)
            }
        }
        Collection::SetItemManager::SIGNATURE_HASH => {
            let e = Collection::SetItemManager::decode_raw_log(topics, &data)
                .map_err(decoded("SetItemManager"))?;
            CollectionEvent {
                item_id: Some(e._itemId.to_string()),
                account: Some(address(e._manager)),
                flag: Some(e._value),
                ..base(KIND_SET_ITEM_MANAGER)
            }
        }
        Collection::SetApproved::SIGNATURE_HASH => {
            let e = Collection::SetApproved::decode_raw_log(topics, &data)
                .map_err(decoded("SetApproved"))?;
            CollectionEvent {
                flag: Some(e._newValue),
                ..base(KIND_SET_APPROVED)
            }
        }
        Collection::SetEditable::SIGNATURE_HASH => {
            let e = Collection::SetEditable::decode_raw_log(topics, &data)
                .map_err(decoded("SetEditable"))?;
            CollectionEvent {
                flag: Some(e._newValue),
                ..base(KIND_SET_EDITABLE)
            }
        }
        Collection::Complete::SIGNATURE_HASH => {
            Collection::Complete::decode_raw_log(topics, &data).map_err(decoded("Complete"))?;
            base(KIND_COMPLETE)
        }
        Collection::CreatorshipTransferred::SIGNATURE_HASH => {
            let e = Collection::CreatorshipTransferred::decode_raw_log(topics, &data)
                .map_err(decoded("CreatorshipTransferred"))?;
            CollectionEvent {
                account: Some(address(e._newCreator)),
                previous_account: Some(address(e._previousCreator)),
                ..base(KIND_CREATORSHIP_TRANSFERRED)
            }
        }
        Collection::OwnershipTransferred::SIGNATURE_HASH => {
            let e = Collection::OwnershipTransferred::decode_raw_log(topics, &data)
                .map_err(decoded("OwnershipTransferred"))?;
            CollectionEvent {
                account: Some(address(e.newOwner)),
                previous_account: Some(address(e.previousOwner)),
                ..base(KIND_OWNERSHIP_TRANSFERRED)
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(event))
}

/// Inserts `rows` (duplicates dropped); the rows written.
pub async fn insert_events(
    conn: &mut PgConnection,
    net: ChainNet,
    rows: &[CollectionEvent],
) -> anyhow::Result<usize> {
    let mut written = 0usize;
    for chunk in rows.chunks(1_000) {
        let col = |f: fn(&CollectionEvent) -> Option<String>| -> Vec<Option<String>> {
            chunk.iter().map(f).collect()
        };
        written += sqlx::query(
            "INSERT INTO shop.collection_event \
                 (network, block_number, log_index, block_time, kind, contract_address, item_id, account, \
                  previous_account, flag, amount, max_supply, rarity, beneficiary, metadata, content_hash) \
             SELECT $1, b, l, t, k, c, i::numeric, a, pa, f, am::numeric, ms::numeric, r, bn, m, h \
             FROM UNNEST($2::bigint[], $3::int[], $4::bigint[], $5::text[], $6::text[], $7::text[], $8::text[], \
                         $9::text[], $10::bool[], $11::text[], $12::text[], $13::text[], $14::text[], $15::text[], \
                         $16::text[]) \
                  AS u(b, l, t, k, c, i, a, pa, f, am, ms, r, bn, m, h) \
             ON CONFLICT (network, block_number, log_index) DO NOTHING",
        )
        .bind(net.db_name())
        .bind(chunk.iter().map(|r| r.block_number).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.log_index).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.block_time).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.kind).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.contract_address.as_str()).collect::<Vec<_>>())
        .bind(col(|r| r.item_id.clone()))
        .bind(col(|r| r.account.clone()))
        .bind(col(|r| r.previous_account.clone()))
        .bind(chunk.iter().map(|r| r.flag).collect::<Vec<_>>())
        .bind(col(|r| r.amount.clone()))
        .bind(col(|r| r.max_supply.clone()))
        .bind(col(|r| r.rarity.clone()))
        .bind(col(|r| r.beneficiary.clone()))
        .bind(col(|r| r.metadata.clone()))
        .bind(col(|r| r.content_hash.clone()))
        .execute(&mut *conn)
        .await?
        .rows_affected() as usize;
    }
    Ok(written)
}

/// A collection the filter may take and the factory that listed it (`None`
/// for a walked one with no hint).
pub type Known = (String, Option<String>);

/// The Polygon walk. `addresses` keeps, per pass, the hinted and walked
/// collections; `filter` the collections the current window's groups took,
/// for its `write` to check.
#[derive(Default)]
pub struct CollectionsIndex {
    addresses: Mutex<Option<Vec<Known>>>,
    filter: Mutex<HashSet<String>>,
    /// Whether its fold projects the log into `shop.collections` and
    /// `shop.items` (a process that runs no collections mirror); otherwise
    /// it keeps the log alone.
    project: bool,
}

impl CollectionsIndex {
    /// The walk keeping the log alone.
    pub fn new() -> Self {
        Self::default()
    }

    /// The walk, projecting the log into the catalogue when `project`.
    pub fn projecting(project: bool) -> Self {
        Self {
            project,
            ..Self::default()
        }
    }

    /// The hinted and walked collections, read once per pass.
    async fn load(&self, pool: &PgPool) -> anyhow::Result<Vec<Known>> {
        if let Some(known) = self.addresses.lock().clone() {
            return Ok(known);
        }
        let known: Vec<Known> = sqlx::query_as(
            "SELECT address, factory FROM shop.collection_hint WHERE network = $1 \
             UNION \
             SELECT e.contract_address, NULL FROM shop.collection_event e \
             WHERE e.network = $1 AND e.kind = 'proxy_created' \
               AND NOT EXISTS (SELECT 1 FROM shop.collection_hint h WHERE h.network = $1 AND h.address = e.contract_address)",
        )
        .bind(ChainNet::Matic.db_name())
        .fetch_all(pool)
        .await?;
        *self.addresses.lock() = Some(known.clone());
        Ok(known)
    }

    fn reset(&self) {
        *self.addresses.lock() = None;
        self.filter.lock().clear();
    }
}

/// The groups of window `[_, to]` over `known` (address, factory): each
/// factory deployed by `to` with `ProxyCreated`, and every collection of
/// those factories (and every walked one) with the collection topics.
pub fn groups_for(to: u64, known: &[Known]) -> (Vec<LogGroup>, HashSet<String>) {
    let live: Vec<&str> = FACTORIES
        .iter()
        .filter(|(_, from)| *from <= to)
        .map(|(f, _)| *f)
        .collect();
    let mut collections: Vec<String> = known
        .iter()
        .filter(|(_, factory)| factory.as_deref().is_none_or(|f| live.contains(&f)))
        .map(|(a, _)| a.clone())
        .collect();
    collections.sort();
    collections.dedup();
    let filter: HashSet<String> = collections.iter().cloned().collect();
    let topics: Vec<String> = COLLECTION_TOPICS
        .iter()
        .map(|t| format!("{t:#x}"))
        .collect();
    let mut groups = vec![LogGroup {
        addresses: live.iter().map(|f| f.to_string()).collect(),
        topics: json!([[format!("{:#x}", Factory::ProxyCreated::SIGNATURE_HASH)]]),
    }];
    if !collections.is_empty() {
        groups.push(LogGroup {
            addresses: collections,
            topics: json!([topics]),
        });
    }
    (groups, filter)
}

#[async_trait::async_trait]
impl ChainIndex for CollectionsIndex {
    fn indexer(&self) -> &'static str {
        INDEXER_COLLECTIONS
    }

    fn net(&self) -> ChainNet {
        ChainNet::Matic
    }

    fn start_block(&self) -> u64 {
        START_BLOCK
    }

    fn abandon(&self) {
        self.reset();
    }

    fn begin_pass(&self) {
        self.reset();
    }

    /// Every pass alone starts here: the hints the gate just read are read
    /// again (a shared walk does it in `begin_pass`).
    #[allow(
        clippy::too_many_arguments,
        reason = "the trait's signature: the pass's source, anchor, budget, chunk, owner and cursor"
    )]
    async fn lanes(
        &self,
        _pool: &PgPool,
        _source: &dyn chain_walk::LogSource,
        _anchor: (u64, i64),
        _budget: usize,
        _chunk: &mut usize,
        _owner: &Owner,
        _cursor: Option<(u64, &str)>,
    ) -> anyhow::Result<chain_walk::Spent> {
        self.reset();
        Ok(chain_walk::Spent::default())
    }

    async fn groups(
        &self,
        pool: &PgPool,
        _from: u64,
        to: u64,
        _anchor: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        let known = self.load(pool).await?;
        let (groups, filter) = groups_for(to, &known);
        *self.filter.lock() = filter;
        Ok(groups)
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
            let time = times
                .get(&block)
                .copied()
                .with_context(|| format!("collections: no block time for block {block}"))?;
            if let Some(row) = decode_log(log, time)? {
                rows.push(row);
            }
        }
        {
            let filter = self.filter.lock();
            if let Some(missed) = rows
                .iter()
                .find(|r| r.kind == KIND_PROXY_CREATED && !filter.contains(&r.contract_address))
            {
                anyhow::bail!(
                    "collections matic: block {} created {}, which this window's filter did not take; \
                     the next attempt reads the factory lists again",
                    missed.block_number,
                    missed.contract_address
                );
            }
        }
        insert_events(conn, ChainNet::Matic, &rows).await
    }

    /// Deletes the log past `to` and the creation reads taken past it, makes
    /// the factory list rows read past it provisional again (a branch the
    /// chain may have left), raises the walk's epoch and clears the
    /// projection's block. An operator's rebuild runs it from the start. It
    /// holds the reads lock exclusive first, so a read written while it runs
    /// is undone by it, and one taken before it is refused after it
    /// ([`ReadsToken`]).
    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
            .bind(READS_LOCK)
            .bind(ChainNet::Matic.chain_id() as i32)
            .execute(&mut *conn)
            .await?;
        sqlx::query("DELETE FROM shop.collection_event WHERE network = $1 AND block_number > $2")
            .bind(ChainNet::Matic.db_name())
            .bind(to)
            .execute(&mut *conn)
            .await?;
        sqlx::query("DELETE FROM shop.collection_seed WHERE network = $1 AND read_block > $2")
            .bind(ChainNet::Matic.db_name())
            .bind(to)
            .execute(&mut *conn)
            .await?;
        sqlx::query(
            "UPDATE shop.collection_hint SET finalized = false, read_block = NULL, read_hash = NULL, \
                    verified = false \
             WHERE network = $1 AND finalized AND (read_block IS NULL OR read_block > $2)",
        )
        .bind(ChainNet::Matic.db_name())
        .bind(to)
        .execute(&mut *conn)
        .await?;
        sqlx::query(
            "INSERT INTO shop.collection_walk (network, epoch) VALUES ($1, 1) \
             ON CONFLICT (network) DO UPDATE SET epoch = shop.collection_walk.epoch + 1",
        )
        .bind(ChainNet::Matic.db_name())
        .execute(&mut *conn)
        .await?;
        super::collections_projection::invalidate(conn, ChainNet::Matic).await?;
        self.reset();
        Ok(())
    }

    /// Projects the log into the catalogue when this process owns it
    /// ([`super::collections_projection`]); either way the publication says
    /// the log, and the projection when there is one, are complete to `upto`.
    async fn fold(
        &self,
        pool: &PgPool,
        _since: Option<i64>,
        upto: i64,
        owner: &Owner,
    ) -> anyhow::Result<bool> {
        if self.project {
            let out =
                super::collections_projection::project(pool, owner, ChainNet::Matic, upto).await?;
            tracing::info!(
                upto,
                rebuilt = out.rebuilt,
                collections = out.collections,
                replayed = out.replayed,
                waiting = out.waiting,
                "collections matic: projected"
            );
        }
        Ok(true)
    }
}

/// The block parameter of a read of the state at the block whose hash is
/// `hash` (EIP-1898): what is read and the hash it is stored with are one
/// block's, and the read is refused once that block is not canonical.
pub fn at_block(hash: &str) -> Value {
    json!({ "blockHash": hash, "requireCanonical": true })
}

/// One `eth_call` of `data` to `to` at `block` (`"latest"`, or
/// [`at_block`]), as a batch element.
fn call_request(id: usize, to: &str, data: &[u8], block: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "eth_call",
        "params": [{ "to": to, "data": format!("0x{}", hex::encode(data)) }, block],
    })
}

/// What one `eth_call` answered: the bytes it returned, `None` when it
/// returned nothing or reverted (no contract at the block), or the JSON-RPC
/// error code of any other refusal (`-1` for an answer that never came).
pub type Answer = Result<Option<Vec<u8>>, i64>;

fn answer_of(envelope: &Value) -> Answer {
    if let Some(result) = envelope.get("result") {
        return Ok(result
            .as_str()
            .and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
            .filter(|b| !b.is_empty()));
    }
    let error = envelope.get("error");
    let code = error.and_then(|e| e.get("code")).and_then(Value::as_i64);
    let reverted = code == Some(3)
        || error
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|m| m.to_ascii_lowercase().contains("revert"));
    if reverted {
        Ok(None)
    } else {
        Err(code.unwrap_or(-1))
    }
}

/// The answers of `calls` (to, data) at `block`, in their order, in JSON-RPC
/// batches of at most [`CALLS_PER_BATCH`]. A batch the endpoint refuses (a
/// transport error, a status, or an answer that lacks some of its calls, as
/// geth answers a batch past its limit) is halved and sent again; a single
/// call it refuses as a batch is sent as a plain request, whose failure
/// fails the read. A batch answered with no element at all (an endpoint that
/// does not take batches) is sent call by call. A call the endpoint answers
/// with an error, inside a batch answered whole, keeps its code.
pub async fn eth_calls(
    rpc: &Rpc<'_>,
    calls: &[(String, Vec<u8>)],
    block: &Value,
) -> anyhow::Result<Vec<Answer>> {
    let mut out: Vec<Answer> = vec![Err(-1); calls.len()];
    let mut todo: Vec<(usize, usize)> = (0..calls.len())
        .step_by(CALLS_PER_BATCH)
        .map(|start| (start, CALLS_PER_BATCH.min(calls.len() - start)))
        .rev()
        .collect();
    while let Some((start, len)) = todo.pop() {
        let body: Vec<Value> = (start..start + len)
            .map(|i| call_request(i, &calls[i].0, &calls[i].1, block))
            .collect();
        let refused = match rpc.batch(&body).await {
            Ok(answers) if answers.is_empty() => {
                for (i, request) in (start..).zip(&body) {
                    let envelope = rpc
                        .call("eth_call", request["params"].clone())
                        .await
                        .map_err(|e| anyhow::anyhow!("eth_call: {e}"))?;
                    out[i] = answer_of(&envelope);
                }
                false
            }
            Ok(answers) => {
                let mut got: Vec<Option<Answer>> = vec![None; len];
                for answer in &answers {
                    let Some(id) = answer.get("id").and_then(Value::as_u64) else {
                        continue;
                    };
                    let id = id as usize;
                    if (start..start + len).contains(&id) {
                        got[id - start] = Some(answer_of(answer));
                    }
                }
                // A batch an endpoint answers in part was refused whole: a
                // batch past geth's limit is answered by one -32600 error
                // with the first request's id.
                if got.iter().all(Option::is_some) {
                    for (i, answer) in (start..).zip(got) {
                        out[i] = answer.expect("checked above");
                    }
                    false
                } else {
                    true
                }
            }
            Err(_) => true,
        };
        if refused && len > 1 {
            let half = len / 2;
            todo.push((start + half, len - half));
            todo.push((start, half));
        } else if refused {
            let envelope = rpc
                .call("eth_call", body[0]["params"].clone())
                .await
                .map_err(|e| anyhow::anyhow!("eth_call: {e}"))?;
            out[start] = answer_of(&envelope);
        }
    }
    Ok(out)
}

/// The JSON-RPC error codes among `answers`, for a log line: how many, and
/// the first.
fn refusals(answers: &[Answer]) -> Option<(usize, i64)> {
    let mut codes = answers.iter().filter_map(|a| a.as_ref().err());
    let first = *codes.next()?;
    Some((1 + codes.count(), first))
}

/// Reads the factories' lists into `shop.collection_hint`. Every index the
/// list holds at the finalized block `finalized` is read there once, by its
/// hash ([`at_block`]), a row read at a finalized block being final and
/// keeping that height and hash; the indexes past it are read at
/// the latest block at every refresh until the finalized block reaches
/// them, so the filter already takes a collection the walk can reach before
/// it is final. A provisional address a reorg replaced stays a hint the
/// filter takes in vain; the address read at the finalized block is added
/// beside it. Once each list is final up to the finalized block, a refresh
/// costs the four `collectionsSize()` reads.
///
/// At most [`HINT_READS_PER_REFRESH`] indexes are read in one refresh, in
/// chunks kept as soon as they are read: first the indexes below the
/// finalized sizes of both lists that have no final row (`remaining`
/// counts the ones left), then, with what is left of it, the provisional
/// ones (`provisional_unread` counts the ones left). A size past
/// [`MAX_LIST_SIZE`] fails the refresh before anything is read, and the
/// indexes still to read are found from the stored rows, so a refresh's
/// work is bounded by the table and the budget whatever an endpoint
/// answers. An index without an answer (none, a refusal, or not an address)
/// fails the refresh, so no attempt walks on a list it could not read whole.
pub async fn refresh_hints(
    pool: &PgPool,
    rpc: &Rpc<'_>,
    finalized: &BlockHeader,
) -> anyhow::Result<Refreshed> {
    let net = ChainNet::Matic;
    let token = reads_token(pool, net).await?;
    let final_block = i64::try_from(finalized.number).context("finalized block out of range")?;
    let final_hash = finalized.hash.clone();
    let at_final = at_block(&finalized.hash);
    let latest = json!("latest");
    let size_calls: Vec<(String, Vec<u8>)> = FACTORIES
        .iter()
        .map(|(f, _)| (f.to_string(), Factory::collectionsSizeCall {}.abi_encode()))
        .collect();
    let size = |answer: Answer, factory: &str| -> anyhow::Result<u32> {
        let size = answer
            .ok()
            .flatten()
            .and_then(|b| U256::abi_decode(&b).ok())
            .with_context(|| format!("{factory}.collectionsSize() did not answer"))?;
        u32::try_from(size)
            .ok()
            .filter(|s| *s <= MAX_LIST_SIZE)
            .with_context(|| {
                format!("{factory}.collectionsSize() answered {size}, past {MAX_LIST_SIZE}")
            })
    };
    let final_sizes = eth_calls(rpc, &size_calls, &at_final).await?;
    let latest_sizes = eth_calls(rpc, &size_calls, &latest).await?;
    let mut lists = Vec::new();
    for (((factory, _), final_size), latest_size) in
        FACTORIES.iter().zip(final_sizes).zip(latest_sizes)
    {
        lists.push((
            *factory,
            size(final_size, factory)?,
            size(latest_size, factory)?,
        ));
    }
    let mut out = Refreshed::default();
    let mut unread = Vec::new();
    let mut budget = HINT_READS_PER_REFRESH;
    // The indexes below each finalized size without a final row, every
    // list's before any provisional read.
    let mut finals_to_read = Vec::new();
    for (factory, final_size, _) in &lists {
        let have: HashSet<i32> = sqlx::query_scalar(
            "SELECT idx FROM shop.collection_hint \
             WHERE network = $1 AND factory = $2 AND finalized AND idx < $3",
        )
        .bind(net.db_name())
        .bind(factory)
        .bind(*final_size as i32)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
        let holes = (*final_size as usize).saturating_sub(have.len());
        let mut indexes = Vec::new();
        let mut i = 0u32;
        while i < *final_size && indexes.len() < budget.min(holes) {
            if !have.contains(&(i as i32)) {
                indexes.push(i as i32);
            }
            i += 1;
        }
        budget -= indexes.len();
        out.remaining += holes - indexes.len();
        finals_to_read.push((*factory, indexes));
    }
    let mut provisional_to_read = Vec::new();
    for (factory, final_size, latest_size) in &lists {
        let indexes: Vec<i32> = (*final_size..*latest_size)
            .take(budget)
            .map(|i| i as i32)
            .collect();
        budget -= indexes.len();
        out.provisional_unread +=
            (*latest_size).saturating_sub(*final_size) as usize - indexes.len();
        provisional_to_read.push((*factory, indexes));
    }
    for (reads, block, finalized) in [
        (finals_to_read, &at_final, true),
        (provisional_to_read, &latest, false),
    ] {
        for (factory, indexes) in reads {
            // Each chunk is kept as soon as it is read: an error later in
            // the refresh loses none of it.
            for chunk in indexes.chunks(HINT_CHUNK) {
                let calls: Vec<(String, Vec<u8>)> = chunk
                    .iter()
                    .map(|i| {
                        let index = U256::from(*i as u32);
                        (
                            factory.to_string(),
                            Factory::collectionsCall { index }.abi_encode(),
                        )
                    })
                    .collect();
                let answers = eth_calls(rpc, &calls, block).await?;
                if let Some((count, first)) = refusals(&answers) {
                    tracing::warn!(
                        count,
                        first_code = first,
                        factory,
                        "collections matic: factory list reads refused"
                    );
                }
                let mut idxs = Vec::new();
                let mut addrs = Vec::new();
                for (i, answer) in chunk.iter().zip(answers) {
                    match answer
                        .ok()
                        .flatten()
                        .and_then(|b| Address::abi_decode(&b).ok())
                        .filter(|a| !a.is_zero())
                    {
                        Some(a) => {
                            idxs.push(*i);
                            addrs.push(address(a));
                        }
                        None => unread.push(format!("{factory}[{i}]")),
                    }
                }
                // Under the reads lock, and only while the walk has not
                // rewound since the reads began: a rewind makes the rows it
                // finds read past it provisional, and a chunk read before
                // it must not finalize them again.
                let mut tx = pool.begin().await?;
                sqlx::query("SELECT pg_advisory_xact_lock_shared($1, $2)")
                    .bind(READS_LOCK)
                    .bind(net.chain_id() as i32)
                    .execute(&mut *tx)
                    .await?;
                anyhow::ensure!(
                    reads_token(&mut *tx, net).await? == token,
                    "the walk rewound while the factory lists were read; the next attempt reads them again"
                );
                out.added += sqlx::query(
                    "INSERT INTO shop.collection_hint \
                         (network, factory, idx, address, finalized, read_block, read_hash) \
                     SELECT $1, $2, i, a, $5, $6, $7 FROM UNNEST($3::int[], $4::text[]) AS u(i, a) \
                     ON CONFLICT (network, address) DO UPDATE \
                     SET finalized = true, factory = EXCLUDED.factory, idx = EXCLUDED.idx, \
                         read_block = EXCLUDED.read_block, read_hash = EXCLUDED.read_hash, verified = false \
                     WHERE EXCLUDED.finalized AND NOT shop.collection_hint.finalized",
                )
                .bind(net.db_name())
                .bind(factory)
                .bind(&idxs)
                .bind(&addrs)
                .bind(finalized)
                .bind(finalized.then_some(final_block))
                .bind(finalized.then_some(final_hash.as_str()))
                .execute(&mut *tx)
                .await?
                .rows_affected() as usize;
                tx.commit().await?;
            }
        }
    }
    if let Some(first) = unread.first() {
        anyhow::bail!(
            "{} factory list indexes did not answer (first {first}); {} hints kept",
            unread.len(),
            out.added
        );
    }
    Ok(out)
}

/// What `initialize` set without an event, as one read at `read_block`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    pub contract_address: String,
    pub name: String,
    pub symbol: String,
    pub creator: String,
    pub is_approved: bool,
    pub is_editable: bool,
    pub read_block: i64,
}

/// The five answers of one collection, or `None` when any is missing (a
/// contract not yet deployed at the block, or a refused call).
pub fn seed_of(contract: &str, answers: &[Answer], read_block: u64) -> Option<Seed> {
    let [name, symbol, creator, approved, editable] = answers else {
        return None;
    };
    let bytes = |a: &Answer| a.as_ref().ok()?.clone();
    let string = |a: &Answer| {
        Bytes::abi_decode(&bytes(a)?)
            .ok()
            .map(|s| text(String::from_utf8_lossy(&s).into_owned()))
    };
    Some(Seed {
        contract_address: contract.to_string(),
        name: string(name)?,
        symbol: string(symbol)?,
        creator: address(Address::abi_decode(&bytes(creator)?).ok()?),
        is_approved: bool::abi_decode(&bytes(approved)?).ok()?,
        is_editable: bool::abi_decode(&bytes(editable)?).ok()?,
        read_block: i64::try_from(read_block).ok()?,
    })
}

/// Reads the creation state of up to [`SEEDS_PER_GATE`] walked collections
/// without a proven one, oldest first, at the walk's own cursor block, named
/// by the hash the walk stored for it ([`at_block`]), into
/// `shop.collection_seed`. The cursor and the walk's [`ReadsToken`] are read
/// in one snapshot and the write is fenced by that token, so every read is
/// of the walked branch and is written proven (`verified`); a rewind below
/// it deletes it in the cursor's own transaction. No header proves
/// anything here: the hash is the walk's. An endpoint that no longer holds
/// that block canonical, or does not serve its state, refuses the read, and
/// the collection waits for the next gate. Returns the reads written.
pub async fn read_seeds(pool: &PgPool, rpc: &Rpc<'_>) -> anyhow::Result<usize> {
    let net = ChainNet::Matic;
    let walk: Option<(i64, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT c.owner_generation, COALESCE(w.epoch, 0), c.last_block, c.last_block_hash \
         FROM shop.chain_cursor c LEFT JOIN shop.collection_walk w ON w.network = c.network \
         WHERE c.indexer = $1 AND c.network = $2",
    )
    .bind(INDEXER_COLLECTIONS)
    .bind(net.db_name())
    .fetch_optional(pool)
    .await?;
    let Some((generation, epoch, walked, Some(hash))) = walk else {
        return Ok(0);
    };
    let Ok(at) = u64::try_from(walked) else {
        return Ok(0);
    };
    let pending: Vec<String> = sqlx::query_scalar(
        "SELECT e.contract_address FROM shop.collection_event e \
         WHERE e.network = $1 AND e.kind = 'proxy_created' AND e.block_number <= $2 \
           AND NOT EXISTS (SELECT 1 FROM shop.collection_seed s \
                           WHERE s.network = $1 AND s.contract_address = e.contract_address AND s.verified) \
         ORDER BY e.block_number, e.log_index LIMIT $3",
    )
    .bind(net.db_name())
    .bind(walked)
    .bind(SEEDS_PER_GATE as i64)
    .fetch_all(pool)
    .await?;
    if pending.is_empty() {
        return Ok(0);
    }
    let selectors: [Vec<u8>; 5] = [
        Collection::nameCall {}.abi_encode(),
        Collection::symbolCall {}.abi_encode(),
        Collection::creatorCall {}.abi_encode(),
        Collection::isApprovedCall {}.abi_encode(),
        Collection::isEditableCall {}.abi_encode(),
    ];
    let calls: Vec<(String, Vec<u8>)> = pending
        .iter()
        .flat_map(|c| selectors.iter().map(move |s| (c.clone(), s.clone())))
        .collect();
    let answers = eth_calls(rpc, &calls, &at_block(&hash)).await?;
    if let Some((count, first)) = refusals(&answers) {
        tracing::warn!(
            count,
            first_code = first,
            "collections matic: creation reads refused; they are read again later"
        );
    }
    let seeds: Vec<Seed> = pending
        .iter()
        .zip(answers.chunks(selectors.len()))
        .filter_map(|(c, a)| seed_of(c, a, at))
        .collect();
    write_seeds(pool, net, &seeds, &hash, Some((generation, epoch))).await
}

/// What a write of the gate's reads checks has not moved since the reads
/// began: the walk's owner generation and its epoch (`shop.collection_walk`,
/// raised by every rewind and rebuild); `None` before the walk has a cursor.
pub type ReadsToken = Option<(i64, i64)>;

/// The walk's [`ReadsToken`] now.
pub async fn reads_token<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    net: ChainNet,
) -> sqlx::Result<ReadsToken> {
    sqlx::query_as(
        "SELECT c.owner_generation, COALESCE(w.epoch, 0) FROM shop.chain_cursor c \
         LEFT JOIN shop.collection_walk w ON w.network = c.network \
         WHERE c.indexer = $1 AND c.network = $2",
    )
    .bind(INDEXER_COLLECTIONS)
    .bind(net.db_name())
    .fetch_optional(executor)
    .await
}

/// Writes `seeds`, read at the walk's cursor block `read_hash` since `token`
/// was taken, as proven (`verified`): a collection's first proven read
/// stands, and one replaces a row written before reads were proven. Under
/// the reads lock, shared, it writes nothing when a rewind, a rebuild or a
/// new owner moved the walk since `token`: that block may no longer be the
/// walked branch's.
pub async fn write_seeds(
    pool: &PgPool,
    net: ChainNet,
    seeds: &[Seed],
    read_hash: &str,
    token: ReadsToken,
) -> anyhow::Result<usize> {
    if seeds.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared($1, $2)")
        .bind(READS_LOCK)
        .bind(net.chain_id() as i32)
        .execute(&mut *tx)
        .await?;
    if reads_token(&mut *tx, net).await? != token {
        tracing::info!(
            network = net.db_name(),
            "collections: the walk moved while the creation reads were taken; they are read again"
        );
        return Ok(0);
    }
    let written = sqlx::query(
        "INSERT INTO shop.collection_seed \
             (network, contract_address, name, symbol, creator, is_approved, is_editable, read_block, read_hash, verified) \
         SELECT $1, c, n, s, cr, a, e, b, $9, true \
         FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[], $6::bool[], $7::bool[], $8::bigint[]) \
              AS u(c, n, s, cr, a, e, b) \
         ON CONFLICT (network, contract_address) DO UPDATE \
         SET name = EXCLUDED.name, symbol = EXCLUDED.symbol, creator = EXCLUDED.creator, \
             is_approved = EXCLUDED.is_approved, is_editable = EXCLUDED.is_editable, \
             read_block = EXCLUDED.read_block, read_hash = EXCLUDED.read_hash, verified = true, \
             read_at = EXCLUDED.read_at \
         WHERE NOT shop.collection_seed.verified",
    )
    .bind(net.db_name())
    .bind(seeds.iter().map(|s| s.contract_address.as_str()).collect::<Vec<_>>())
    .bind(seeds.iter().map(|s| s.name.as_str()).collect::<Vec<_>>())
    .bind(seeds.iter().map(|s| s.symbol.as_str()).collect::<Vec<_>>())
    .bind(seeds.iter().map(|s| s.creator.as_str()).collect::<Vec<_>>())
    .bind(seeds.iter().map(|s| s.is_approved).collect::<Vec<_>>())
    .bind(seeds.iter().map(|s| s.is_editable).collect::<Vec<_>>())
    .bind(seeds.iter().map(|s| s.read_block).collect::<Vec<_>>())
    .bind(read_hash)
    .execute(&mut *tx)
    .await?
    .rows_affected() as usize;
    tx.commit().await?;
    Ok(written)
}

/// The hashes of the blocks at `heights`, each checked to be that block's:
/// in JSON-RPC batches, then one request each for what a batch did not
/// answer whole, or for all of them when the endpoint refuses batches. A
/// height the endpoint does not serve fails the read.
async fn block_hashes(rpc: &Rpc<'_>, heights: &[u64]) -> anyhow::Result<HashMap<u64, String>> {
    let hash_like = |h: &str| {
        h.len() == 66 && h.starts_with("0x") && h[2..].bytes().all(|c| c.is_ascii_hexdigit())
    };
    let mut out = HashMap::new();
    if let Ok((headers, _)) = rpc.block_headers(heights).await {
        for (number, header) in headers {
            if header.number == number && hash_like(&header.hash) {
                out.insert(number, header.hash);
            }
        }
    }
    for number in heights {
        if !out.contains_key(number) {
            let header = rpc
                .block_header(*number)
                .await
                .map_err(|e| anyhow::anyhow!("block header {number}: {e}"))?;
            out.insert(*number, header.hash);
        }
    }
    Ok(out)
}

/// Checks the final factory list rows against the chain. Each keeps the
/// height and hash of the finalized block it was read at ([`at_block`]);
/// until it is `verified`:
/// - one with no hash (written before hashes were kept, by an index
///   already published) is proven by the walk's log when the walk has its
///   full history and holds its exact slot (its height becomes that
///   creation's block); otherwise it is undone without a read: it becomes
///   provisional again, to be read again;
/// - at or below the walk's cursor, on a walk with its full history, the
///   walk's own log decides, slot by slot: a factory's list grows only by
///   the push after each `ProxyCreated` it emits, so slot i is its i-th
///   walked `ProxyCreated`. That `ProxyCreated` naming the row's address, at
///   or before that height, proves the row (`verified`; from then a rewind
///   below it undoes it). Anything else (another address in that slot, or
///   none yet) means the walked branch does not hold it there, and the row is
///   undone, to be read again into the slot the walk has it in;
/// - otherwise (ahead of the walk, or on a walk without its full history,
///   whose log lacks the older collections) the endpoint's hash at that
///   height is asked. Another hash undoes the row; the same one leaves it as
///   it is. A header never proves a row: two replies may come from two
///   branches.
///
/// At most `max_heights` heights are asked, in turn from where the last
/// check stopped (`shop.collection_walk.checked_through`), so every height
/// is asked within a few attempts. A header the endpoint does not serve
/// fails the call. The writes hold the reads lock exclusive, raise the
/// walk's epoch when a row is undone (so a read in flight from a left
/// branch is not written either), and are dropped when a rewind or a new
/// owner moved the walk since the check began. The creation reads need no
/// check: [`read_seeds`] takes them at the walk's own block.
///
/// Returns the rows undone.
pub async fn revalidate_reads(
    pool: &PgPool,
    rpc: &Rpc<'_>,
    max_heights: usize,
) -> anyhow::Result<usize> {
    let net = ChainNet::Matic;
    let token = reads_token(pool, net).await?;
    let walk: Option<(i64, bool)> = sqlx::query_as(
        "SELECT last_block, full_history FROM shop.chain_cursor WHERE indexer = $1 AND network = $2",
    )
    .bind(INDEXER_COLLECTIONS)
    .bind(net.db_name())
    .fetch_optional(pool)
    .await?;
    let (walked, full) = walk.map_or((None, false), |(b, f)| (Some(b), f));
    // The heights the walk's log does not decide.
    let heights: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT read_block FROM shop.collection_hint \
         WHERE network = $1 AND finalized AND NOT verified \
           AND read_hash IS NOT NULL AND read_block >= 0 \
           AND NOT ($3 AND read_block <= $2) \
         ORDER BY read_block",
    )
    .bind(net.db_name())
    .bind(walked.unwrap_or(-1))
    .bind(full && walked.is_some())
    .fetch_all(pool)
    .await?;
    let through: Option<i64> =
        sqlx::query_scalar("SELECT checked_through FROM shop.collection_walk WHERE network = $1")
            .bind(net.db_name())
            .fetch_optional(pool)
            .await?
            .flatten();
    let from = through.unwrap_or(-1);
    let turn: Vec<i64> = heights
        .iter()
        .filter(|h| **h > from)
        .chain(heights.iter().filter(|h| **h <= from))
        .take(max_heights)
        .copied()
        .collect();
    let next_through = (heights.len() > turn.len())
        .then(|| turn.last().copied())
        .flatten();
    let mut left = Vec::new();
    if !turn.is_empty() {
        let ask: Vec<u64> = turn.iter().map(|h| *h as u64).collect();
        let hashes = block_hashes(rpc, &ask).await?;
        let stored: Vec<(i64, String)> = sqlx::query_as(
            "SELECT DISTINCT read_block, read_hash FROM shop.collection_hint \
             WHERE network = $1 AND finalized AND NOT verified AND read_block = ANY($2) AND read_hash IS NOT NULL",
        )
        .bind(net.db_name())
        .bind(&turn)
        .fetch_all(pool)
        .await?;
        left = stored
            .into_iter()
            .filter(|(block, hash)| hashes.get(&(*block as u64)) != Some(hash))
            .collect();
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
        .bind(READS_LOCK)
        .bind(net.chain_id() as i32)
        .execute(&mut *tx)
        .await?;
    if reads_token(&mut *tx, net).await? != token {
        tracing::info!(
            network = net.db_name(),
            "collections: the walk moved while the lists were checked; the next attempt checks them again"
        );
        return Ok(0);
    }
    macro_rules! demote {
        () => {
            "UPDATE shop.collection_hint h \
             SET finalized = false, read_block = NULL, read_hash = NULL, verified = false "
        };
    }
    // The walk's log, against the cursor as it stands in the same statement.
    macro_rules! walked_through {
        () => {
            "FROM shop.chain_cursor c \
             WHERE h.network = $1 AND h.finalized AND NOT h.verified AND h.read_hash IS NOT NULL \
               AND c.indexer = $2 AND c.network = h.network AND c.full_history \
               AND h.read_block <= c.last_block "
        };
    }
    // A factory's list grows by one push per `createCollection`, right after
    // the one `ProxyCreated` it emits, and nothing else writes it: slot i is
    // the factory's i-th `ProxyCreated` in the walk's log. With CREATE2 an
    // address may keep its identity on a branch that created it in another
    // order, so the row's exact slot is checked, not the address alone.
    // Every factory's slots, ranked once for the statement.
    macro_rules! slots {
        () => {
            "WITH slots AS MATERIALIZED ( \
                 SELECT e.account AS factory, e.contract_address AS address, e.block_number, \
                        row_number() OVER (PARTITION BY e.account ORDER BY e.block_number, e.log_index) - 1 AS slot \
                 FROM shop.collection_event e WHERE e.network = $1 AND e.kind = 'proxy_created' \
             ) "
        };
    }
    macro_rules! created {
        () => {
            "EXISTS (SELECT 1 FROM slots s \
             WHERE s.factory = h.factory AND s.slot = h.idx AND s.address = h.address \
               AND s.block_number <= h.read_block)"
        };
    }
    // A row written before reads kept their block (an index already
    // published) needs no read where the walk holds its slot: proven there,
    // with the creation's block as its height, so a rewind below the
    // creation undoes it.
    sqlx::query(concat!(
        slots!(),
        "UPDATE shop.collection_hint h SET verified = true, read_block = s.block_number \
         FROM shop.chain_cursor c, slots s \
         WHERE h.network = $1 AND h.finalized AND NOT h.verified \
           AND (h.read_hash IS NULL OR h.read_block IS NULL) \
           AND c.indexer = $2 AND c.network = h.network AND c.full_history \
           AND s.factory = h.factory AND s.slot = h.idx AND s.address = h.address"
    ))
    .bind(net.db_name())
    .bind(INDEXER_COLLECTIONS)
    .execute(&mut *tx)
    .await?;
    let mut undone = sqlx::query(concat!(
        demote!(),
        "WHERE h.network = $1 AND h.finalized AND NOT h.verified \
           AND (h.read_hash IS NULL OR h.read_block IS NULL)"
    ))
    .bind(net.db_name())
    .execute(&mut *tx)
    .await?
    .rows_affected() as usize;
    sqlx::query(concat!(
        slots!(),
        "UPDATE shop.collection_hint h SET verified = true ",
        walked_through!(),
        "AND ",
        created!()
    ))
    .bind(net.db_name())
    .bind(INDEXER_COLLECTIONS)
    .execute(&mut *tx)
    .await?;
    undone += sqlx::query(concat!(
        slots!(),
        demote!(),
        walked_through!(),
        "AND NOT ",
        created!()
    ))
    .bind(net.db_name())
    .bind(INDEXER_COLLECTIONS)
    .execute(&mut *tx)
    .await?
    .rows_affected() as usize;
    for (block, hash) in &left {
        undone += sqlx::query(concat!(
            demote!(),
            "WHERE h.network = $1 AND h.finalized AND NOT h.verified \
               AND h.read_block = $2 AND h.read_hash = $3"
        ))
        .bind(net.db_name())
        .bind(block)
        .bind(hash)
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;
    }
    sqlx::query(
        "INSERT INTO shop.collection_walk (network, epoch, checked_through) VALUES ($1, $2, $3) \
         ON CONFLICT (network) DO UPDATE \
         SET epoch = shop.collection_walk.epoch + $2, checked_through = EXCLUDED.checked_through",
    )
    .bind(net.db_name())
    .bind(i64::from(undone > 0))
    .bind(next_through)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(undone)
}

/// The gate before each attempt: the endpoint must serve Polygon (what it
/// reads is kept until a rewind or rebuild past it, or until the chain leaves
/// the branch it was read on: [`revalidate_reads`], which runs first and
/// fails the attempt if it cannot check); then the factories' lists at
/// Polygon's finalized block, read once (a failure fails the attempt: a
/// filter without the newest collections would fail the windows that walk
/// them); then the creation reads of the walked collections at the walk's
/// own block ([`read_seeds`]; a failure is only logged: the walk does not
/// need them). While the lists are not read whole up to the
/// finalized sizes (a fresh node's take several attempts), the attempt
/// waits for the next interval; provisional indexes past them never hold
/// it back (a window that walks a collection the filter lacks still fails).
pub async fn gate(pool: &PgPool, rpc: &Rpc<'_>) -> anyhow::Result<Option<String>> {
    let net = ChainNet::Matic;
    super::ensure_chain(&chain_walk::RpcSource::new(*rpc), net.chain_id())
        .await
        .context("collections matic: the endpoint's chain")?;
    let finalized = rpc
        .finalized_header()
        .await
        .context("collections matic: the finalized block")?;
    let undone = revalidate_reads(pool, rpc, HEIGHTS_PER_GATE)
        .await
        .context("collections matic: checking the reads ahead of the walk")?;
    if undone > 0 {
        tracing::warn!(
            undone,
            "collections matic: reads of a branch the chain left are read again"
        );
    }
    let hinted = refresh_hints(pool, rpc, &finalized)
        .await
        .context("collections matic: reading the factory lists")?;
    if hinted.added > 0 {
        tracing::info!(
            hinted = hinted.added,
            "collections matic: new collections in the factory lists"
        );
    }
    if hinted.provisional_unread > 0 {
        tracing::warn!(
            unread = hinted.provisional_unread,
            "collections matic: provisional factory list indexes left for the next refresh"
        );
    }
    if hinted.remaining > 0 {
        return Ok(Some(format!(
            "collections matic: {} factory list indexes still to read",
            hinted.remaining
        )));
    }
    match read_seeds(pool, rpc).await {
        Ok(0) => {}
        Ok(seeded) => tracing::info!(seeded, "collections matic: creation state read"),
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "collections matic: reading creation state failed; the next attempt retries")
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::LogData;
    use std::sync::Arc;

    use crate::chain_index::logs::{LogSource, Read, Window};
    use crate::chain_index::rpc::{BlockHeader, RpcError};

    const V3: &str = "0x3195e88ae10704b359764cb38e429d24f1c2f781";
    /// The collection the fixture follows from its creation (block 97,707,994).
    const CREATED: &str = "0x5f255679f7192bae8440746311757c4a5f752107";
    /// An older collection the fixture holds a manager grant and a rescue of.
    const OLDER: &str = "0xa968767179877418fe9313a54a3abe4d1ee24f0f";
    const CREATOR: &str = "0x23be90335e79d3245615985d91f69814f98a0fab";

    /// Real Polygon logs: the factory's ProxyCreated of `CREATED` and every
    /// log both collections emitted from then to block 97,895,000, the
    /// collection topics and others (Transfer, BaseURI, meta-transactions).
    fn fixture() -> Vec<RpcLog> {
        serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/shop/collections-polygon-logs.json"
        )))
        .unwrap()
    }

    fn at(logs: &[RpcLog], block: i64, index: i64) -> &RpcLog {
        logs.iter()
            .find(|l| chain_walk::log_position(l) == (block, index))
            .unwrap()
    }

    fn rpc_log(address: &str, data: LogData, block: u64, index: u64) -> RpcLog {
        RpcLog {
            address: address.into(),
            topics: data.topics().iter().map(|t| format!("{t:#x}")).collect(),
            data: format!("0x{}", hex::encode(&data.data)),
            block_number: format!("0x{block:x}"),
            log_index: format!("0x{index:x}"),
            block_timestamp: None,
            block_hash: None,
            transaction_hash: None,
        }
    }

    #[test]
    fn topics_are_the_contracts_event_signatures() {
        let expect = [
            (
                Factory::ProxyCreated::SIGNATURE_HASH,
                "0xcfaab0d6675a72a93c114f48dd85add1076be0c88545968759ef034da7ad146f",
            ),
            (
                Collection::AddItem::SIGNATURE_HASH,
                "0x1464db75caec2c4f1b56b5246ef01a229ff5edc3b0fcf9b2953fec53b3987d8f",
            ),
            (
                Collection::RescueItem::SIGNATURE_HASH,
                "0x87a972ab2db2d47a0bbefe72cefc4fe5a38b1b9d2bc4b9f366b59fdb6dbd9581",
            ),
            (
                Collection::UpdateItemData::SIGNATURE_HASH,
                "0xc3835f7dd5063bbc8fdc7077944498159008d73d30bf55fe6513e683c8c38209",
            ),
            (
                Collection::Issue::SIGNATURE_HASH,
                super::super::sales_index::TOPIC_ISSUE,
            ),
            (
                Collection::SetGlobalMinter::SIGNATURE_HASH,
                "0x27defe4b442489e757345f4f743096c5db8d2399e1474bb9653118cdf5313869",
            ),
            (
                Collection::SetGlobalManager::SIGNATURE_HASH,
                "0x68bb2af98a57ecc88afb707f2d75e1e253c3e30191071548355731e21322a432",
            ),
            (
                Collection::SetItemMinter::SIGNATURE_HASH,
                "0x45a634291439a8ec187e2b70279696615889c34c17c3a1208a7f85544e13b565",
            ),
            (
                Collection::SetItemManager::SIGNATURE_HASH,
                "0x1271b13cf757efd2a177fe5f26c2111b69ce904f1d0e3372832dc4e4773298cc",
            ),
            (
                Collection::SetApproved::SIGNATURE_HASH,
                "0x9ea3c7bd2b8ff87a3768286a529f5932ca385584c8ddf42eea8eade2ac1fb121",
            ),
            (
                Collection::SetEditable::SIGNATURE_HASH,
                "0xc8ca126810b77bfdcb10ae82ef3bc72619d404f9e3c4bf2057f03f057dcf230a",
            ),
            (
                Collection::Complete::SIGNATURE_HASH,
                "0x01b7dcb42d49142a99e4c98da755263c600213a33b780986779405b9823501d3",
            ),
            (
                Collection::CreatorshipTransferred::SIGNATURE_HASH,
                "0xf25c6226b06a6bf4b2c2391bb66962bb4290c323cf8b5814efb185aba3f42d5f",
            ),
            (
                Collection::OwnershipTransferred::SIGNATURE_HASH,
                "0x8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e0",
            ),
        ];
        for (topic, hex) in expect {
            assert_eq!(format!("{topic:#x}"), hex);
        }
        assert_eq!(
            COLLECTION_TOPICS.len(),
            expect.len() - 1,
            "every collection topic is walked"
        );
    }

    #[test]
    fn factories_match_the_pinned_address_book() {
        let pinned: Value = serde_json::from_str(include_str!("../evm/addresses.json")).unwrap();
        let book = |name: &str| pinned["matic"][name].as_str().unwrap().to_lowercase();
        assert_eq!(
            FACTORIES.map(|(f, _)| f.to_string()),
            [book("CollectionFactory"), book("CollectionFactoryV3")]
        );
    }

    /// The real creation transaction and the collection's later logs decode
    /// to the rows the projection needs; the logs of other topics are not rows.
    #[test]
    fn real_polygon_logs_decode_to_rows() {
        let logs = fixture();
        let row = |block, index| decode_log(at(&logs, block, index), 7).unwrap().unwrap();
        let created = row(97_707_994, 30);
        assert_eq!(
            (
                created.kind,
                created.contract_address.as_str(),
                created.account.as_deref()
            ),
            (KIND_PROXY_CREATED, CREATED, Some(V3))
        );
        assert_eq!(
            (created.block_number, created.log_index, created.block_time),
            (97_707_994, 30, 7)
        );
        let init_owner = row(97_707_994, 31);
        assert_eq!(
            (
                init_owner.kind,
                init_owner.previous_account.as_deref(),
                init_owner.account.as_deref()
            ),
            (
                KIND_OWNERSHIP_TRANSFERRED,
                Some("0x0000000000000000000000000000000000000000"),
                Some(V3)
            )
        );
        let handed = row(97_707_994, 36);
        assert_eq!(
            (
                handed.previous_account.as_deref(),
                handed.account.as_deref()
            ),
            (Some(V3), Some("0xbf6755a83c0dcdbb2933a96ea778e00b717d7004"))
        );
        let helmet = row(97_707_994, 33);
        assert_eq!(helmet.kind, KIND_ADD_ITEM);
        assert_eq!(helmet.contract_address, CREATED);
        assert_eq!(helmet.item_id.as_deref(), Some("0"));
        assert_eq!(helmet.rarity.as_deref(), Some("epic"));
        assert_eq!(helmet.max_supply.as_deref(), Some("1000"));
        assert_eq!(
            helmet.amount.as_deref(),
            Some(U256::MAX.to_string().as_str()),
            "the builder's not-for-sale price"
        );
        assert_eq!(helmet.beneficiary.as_deref(), Some(CREATOR));
        assert_eq!(
            helmet.metadata.as_deref(),
            Some(
                "1:w:Vintage Leather Helmet:Vintage racing leather with stitched detail and warm nickel.:helmet:BaseMale,BaseFemale"
            )
        );
        assert_eq!(helmet.content_hash.as_deref(), Some(""));
        assert_eq!(row(97_707_994, 34).rarity.as_deref(), Some("legendary"));
        assert_eq!(row(97_707_994, 35).kind, KIND_COMPLETE);
        let rescue = row(97_732_792, 9);
        assert_eq!(
            (rescue.kind, rescue.item_id.as_deref()),
            (KIND_RESCUE_ITEM, Some("0"))
        );
        assert_eq!(
            rescue.content_hash.as_deref(),
            Some("bafkreihkpym6g2ykw4ah7nu4435jubdtaxpjrrvcn4jre3dfxqd5k634em")
        );
        assert_eq!(rescue.metadata, helmet.metadata);
        let approved = row(97_732_808, 165);
        assert_eq!(
            (approved.kind, approved.flag),
            (KIND_SET_APPROVED, Some(true))
        );
        let mint = row(97_751_770, 85);
        assert_eq!(
            (
                mint.kind,
                mint.item_id.as_deref(),
                mint.amount.as_deref(),
                mint.account.as_deref()
            ),
            (KIND_ISSUE, Some("0"), Some("1"), Some(CREATOR))
        );
        let minter = row(97_754_539, 3);
        assert_eq!(
            (minter.kind, minter.account.as_deref(), minter.flag),
            (
                KIND_SET_GLOBAL_MINTER,
                Some("0xc36f78a5a475576edaf6c9010ddb5c968fb0a0de"),
                Some(true)
            )
        );
        let manager = row(97_012_160, 65);
        assert_eq!(
            (
                manager.kind,
                manager.contract_address.as_str(),
                manager.account.as_deref(),
                manager.flag
            ),
            (
                KIND_SET_GLOBAL_MANAGER,
                OLDER,
                Some("0xe44ecce4e86c2263a093415da2bbdcecb1a4a1c6"),
                Some(true)
            )
        );
        // Transfer (the nft index's), BaseURI and the meta-transaction log.
        for (block, index) in [(97_751_770, 84), (97_707_994, 32), (97_751_770, 83)] {
            assert_eq!(decode_log(at(&logs, block, index), 7).unwrap(), None);
        }
        let rows = logs
            .iter()
            .filter_map(|l| decode_log(l, 7).unwrap())
            .count();
        assert_eq!(rows, 19);
    }

    /// The kinds the fixture lacks, from logs encoded with the contracts' ABI.
    #[test]
    fn every_other_kind_decodes_from_the_contract_abi() {
        let c = Address::repeat_byte(0xc1);
        let who = Address::repeat_byte(0xab);
        let decode = |data: LogData| {
            decode_log(&rpc_log(&address(c), data, 9, 2), 1)
                .unwrap()
                .unwrap()
        };
        let update = decode(
            Collection::UpdateItemData {
                _itemId: U256::from(4),
                _price: U256::from(5_000u64),
                _beneficiary: who,
                _metadata: "1:e:Wave:hi:greetings:BaseMale:1:so".into(),
            }
            .encode_log_data(),
        );
        assert_eq!(
            (
                update.kind,
                update.item_id.as_deref(),
                update.amount.as_deref(),
                update.beneficiary.as_deref()
            ),
            (
                KIND_UPDATE_ITEM_DATA,
                Some("4"),
                Some("5000"),
                Some(address(who).as_str())
            )
        );
        assert_eq!(
            update.metadata.as_deref(),
            Some("1:e:Wave:hi:greetings:BaseMale:1:so")
        );
        let item_minter = decode(
            Collection::SetItemMinter {
                _itemId: U256::from(3),
                _minter: who,
                _value: U256::MAX,
            }
            .encode_log_data(),
        );
        assert_eq!(
            (
                item_minter.kind,
                item_minter.item_id.as_deref(),
                item_minter.account.as_deref()
            ),
            (KIND_SET_ITEM_MINTER, Some("3"), Some(address(who).as_str()))
        );
        assert_eq!(item_minter.amount, Some(U256::MAX.to_string()));
        let item_manager = decode(
            Collection::SetItemManager {
                _itemId: U256::from(3),
                _manager: who,
                _value: false,
            }
            .encode_log_data(),
        );
        assert_eq!(
            (item_manager.kind, item_manager.flag),
            (KIND_SET_ITEM_MANAGER, Some(false))
        );
        let editable = decode(
            Collection::SetEditable {
                _previousValue: true,
                _newValue: false,
            }
            .encode_log_data(),
        );
        assert_eq!(
            (editable.kind, editable.flag),
            (KIND_SET_EDITABLE, Some(false))
        );
        let creator = decode(
            Collection::CreatorshipTransferred {
                _previousCreator: Address::repeat_byte(0xcc),
                _newCreator: who,
            }
            .encode_log_data(),
        );
        assert_eq!(
            (
                creator.kind,
                creator.previous_account.as_deref(),
                creator.account.as_deref()
            ),
            (
                KIND_CREATORSHIP_TRANSFERRED,
                Some(address(Address::repeat_byte(0xcc)).as_str()),
                Some(address(who).as_str())
            )
        );
        assert_eq!(creator.contract_address, address(c));
    }

    /// Bytes `text` cannot hold are dropped, invalid UTF-8 is replaced; a
    /// ProxyCreated from another contract and a payload that does not decode
    /// fail instead of being skipped.
    #[test]
    fn odd_payloads_are_stored_or_refused_never_skipped() {
        let c = address(Address::repeat_byte(0xc1));
        let item = Collection::Item {
            rarity: "ep\0ic".into(),
            maxSupply: U256::from(10),
            totalSupply: U256::ZERO,
            price: U256::from(1),
            beneficiary: Address::repeat_byte(0xbe),
            metadata: "1:w:A\0B:d:hat:BaseMale".into(),
            contentHash: String::new(),
        };
        let added = decode_log(
            &rpc_log(
                &c,
                Collection::AddItem {
                    _itemId: U256::from(1),
                    _item: item,
                }
                .encode_log_data(),
                9,
                0,
            ),
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(added.rarity.as_deref(), Some("epic"));
        assert_eq!(added.metadata.as_deref(), Some("1:w:AB:d:hat:BaseMale"));
        let mut bad_utf8 = rpc_log(
            &c,
            Collection::RescueItem {
                _itemId: U256::from(1),
                _contentHash: "h".into(),
                _metadata: "xyz".into(),
            }
            .encode_log_data(),
            9,
            1,
        );
        // The metadata's three bytes are the last word's first three.
        let at = bad_utf8.data.len() - 64;
        bad_utf8.data.replace_range(at..at + 2, "ff");
        let rescue = decode_log(&bad_utf8, 1).unwrap().unwrap();
        assert_eq!(rescue.metadata.as_deref(), Some("\u{fffd}yz"));

        let foreign = rpc_log(
            &c,
            Factory::ProxyCreated {
                _address: Address::repeat_byte(0x11),
                _salt: B256::ZERO,
            }
            .encode_log_data(),
            9,
            2,
        );
        assert!(
            decode_log(&foreign, 1)
                .unwrap_err()
                .to_string()
                .contains("not a collection factory")
        );
        let mut short = rpc_log(
            &c,
            Collection::SetApproved {
                _previousValue: false,
                _newValue: true,
            }
            .encode_log_data(),
            9,
            3,
        );
        short.data.truncate(66);
        assert!(decode_log(&short, 1).is_err());
    }

    #[test]
    fn the_filter_takes_each_factory_and_its_collections_from_its_deployment() {
        let known = vec![
            (
                "0x00000000000000000000000000000000000000a2".to_string(),
                Some(FACTORIES[0].0.to_string()),
            ),
            (
                "0x00000000000000000000000000000000000000a3".to_string(),
                Some(FACTORIES[1].0.to_string()),
            ),
            (
                "0x00000000000000000000000000000000000000aa".to_string(),
                None,
            ),
        ];
        let (groups, filter) = groups_for(FACTORIES[1].1 - 1, &known);
        assert_eq!(groups[0].addresses, vec![FACTORIES[0].0.to_string()]);
        assert_eq!(
            groups[1].addresses,
            vec![
                "0x00000000000000000000000000000000000000a2",
                "0x00000000000000000000000000000000000000aa"
            ]
        );
        assert_eq!(filter.len(), 2);
        let (groups, filter) = groups_for(FACTORIES[1].1, &known);
        assert_eq!(groups[0].addresses.len(), 2);
        assert_eq!(groups[1].addresses.len(), 3);
        assert!(filter.contains("0x00000000000000000000000000000000000000a3"));
        assert_eq!(
            groups[1].topics[0].as_array().unwrap().len(),
            COLLECTION_TOPICS.len()
        );
        let (groups, _) = groups_for(FACTORIES[0].1, &[]);
        assert_eq!(groups.len(), 1, "no collection yet, no empty group");
    }

    #[test]
    fn a_seed_needs_all_five_answers() {
        let answers: [Answer; 5] = [
            Ok(Some("World Track Memorabilia".to_string().abi_encode())),
            Ok(Some("DCL-WRLDTRCKMMRBL".to_string().abi_encode())),
            Ok(Some(CREATOR.parse::<Address>().unwrap().abi_encode())),
            Ok(Some(true.abi_encode())),
            Ok(Some(false.abi_encode())),
        ];
        let seed = seed_of(CREATED, &answers, 97_800_000).unwrap();
        assert_eq!(
            seed,
            Seed {
                contract_address: CREATED.into(),
                name: "World Track Memorabilia".into(),
                symbol: "DCL-WRLDTRCKMMRBL".into(),
                creator: CREATOR.into(),
                is_approved: true,
                is_editable: false,
                read_block: 97_800_000,
            }
        );
        let mut missing = answers.clone();
        missing[2] = Ok(None);
        assert_eq!(seed_of(CREATED, &missing, 1), None);
        let mut refused = answers.clone();
        refused[4] = Err(-32005);
        assert_eq!(seed_of(CREATED, &refused, 1), None);
        assert_eq!(seed_of(CREATED, &answers[..4], 1), None);
    }

    /// A chain that answers `logs` by the groups' addresses and first
    /// topics, with one hash per block and a finalized block.
    struct Chain {
        logs: Vec<RpcLog>,
        finalized: u64,
    }

    fn header(number: u64) -> BlockHeader {
        BlockHeader {
            number,
            hash: format!("0x{number:064x}"),
            timestamp: 1_600_000_000 + 2 * number as i64,
        }
    }

    #[async_trait::async_trait]
    impl LogSource for Chain {
        async fn chain_id(&self) -> Result<Read<u64>, RpcError> {
            Ok(Read {
                value: 137,
                requests: 1,
            })
        }
        async fn head(&self) -> Result<Read<u64>, RpcError> {
            Ok(Read {
                value: self.finalized + 64,
                requests: 1,
            })
        }
        async fn finalized(&self) -> Result<Read<BlockHeader>, RpcError> {
            Ok(Read {
                value: header(self.finalized),
                requests: 1,
            })
        }
        async fn header(&self, number: u64) -> Result<Read<BlockHeader>, RpcError> {
            Ok(Read {
                value: header(number),
                requests: 1,
            })
        }
        async fn window(
            &self,
            from: u64,
            to: u64,
            groups: &[LogGroup],
            _chunk: &mut usize,
        ) -> Result<Window, RpcError> {
            let takes = |l: &RpcLog| {
                groups.iter().any(|g| {
                    g.addresses
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(&l.address))
                        && g.topics[0]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|t| t.as_str().unwrap().eq_ignore_ascii_case(&l.topics[0]))
                })
            };
            let mut logs: Vec<RpcLog> = self
                .logs
                .iter()
                .filter(|l| {
                    let b = chain_walk::log_position(l).0 as u64;
                    (from..=to).contains(&b) && takes(l)
                })
                .cloned()
                .collect();
            logs.sort_by_key(chain_walk::log_position);
            Ok(Window {
                logs,
                reached: to,
                requests: groups.len(),
                charge: groups.len(),
                portal_sends: 0,
            })
        }
        async fn block_times(&self, logs: &[RpcLog]) -> Result<Read<HashMap<i64, i64>>, RpcError> {
            let value = logs
                .iter()
                .map(|l| {
                    let b = chain_walk::log_position(l).0;
                    (b, header(b as u64).timestamp)
                })
                .collect();
            Ok(Read { value, requests: 1 })
        }
    }

    fn walk_settings() -> chain_walk::IndexSettings {
        chain_walk::IndexSettings {
            collections_enabled: true,
            start_block_matic: Some(97_012_000),
            requests_per_pass: 1_000,
            ..Default::default()
        }
    }

    async fn hint(pool: &PgPool, idx: i32, address: &str) {
        sqlx::query("INSERT INTO shop.collection_hint (network, factory, idx, address, finalized) VALUES ('matic', $1, $2, $3, true)")
            .bind(V3)
            .bind(idx)
            .bind(address)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn kinds(pool: &PgPool) -> Vec<(String, i64)> {
        sqlx::query_as(
            "SELECT kind, count(*) FROM shop.collection_event WHERE network = 'matic' GROUP BY kind ORDER BY kind",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// The walk over the real logs keeps every walked topic of the hinted
    /// collections, nothing else, and a rewind drops what lies past it.
    #[tokio::test]
    async fn the_walk_keeps_the_hinted_collections_log() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        hint(&pool, 0, OLDER).await;
        hint(&pool, 1, CREATED).await;
        let chain = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let out = chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        assert_eq!(out.indexed_to, 97_895_000);
        assert_eq!(
            kinds(&pool).await,
            [
                ("add_item", 2),
                ("complete", 1),
                ("issue", 5),
                ("ownership_transferred", 2),
                ("proxy_created", 1),
                ("rescue_item", 4),
                ("set_approved", 1),
                ("set_global_manager", 1),
                ("set_global_minter", 2),
            ]
            .map(|(k, n)| (k.to_string(), n))
        );
        let (contract, item, amount, max): (String, String, String, String) = sqlx::query_as(
            "SELECT contract_address, item_id::text, amount::text, max_supply::text FROM shop.collection_event \
             WHERE network = 'matic' AND block_number = 97707994 AND log_index = 34",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (contract.as_str(), item.as_str(), max.as_str()),
            (CREATED, "1", "100")
        );
        assert_eq!(amount, U256::MAX.to_string());
        let time: i64 = sqlx::query_scalar(
            "SELECT block_time FROM shop.collection_event WHERE network = 'matic' AND kind = 'proxy_created'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(time, header(97_707_994).timestamp);

        let mut tx = pool.begin().await.unwrap();
        ix.rewind(&mut tx, 97_750_000).await.unwrap();
        tx.commit().await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.collection_event")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            left, 11,
            "the older collection's two rows and the nine of the creation up to its approval"
        );
    }

    /// A window that walks a ProxyCreated its filter did not take commits
    /// nothing; once the factory list has it, the walk takes the creation
    /// and the collection's own logs from that window on.
    #[tokio::test]
    async fn a_collection_missing_from_the_hints_fails_its_window() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        hint(&pool, 0, OLDER).await;
        let chain = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let err = chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains(CREATED), "{err:#}");
        let cursor = chain_walk::cursor(&pool, INDEXER_COLLECTIONS, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        assert!(
            cursor.last_block < 97_707_994,
            "the window holding the creation was not committed"
        );
        assert_eq!(
            kinds(&pool).await,
            [("rescue_item", 1), ("set_global_manager", 1)].map(|(k, n)| (k.to_string(), n))
        );

        hint(&pool, 1, CREATED).await;
        chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        let created: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM shop.collection_event WHERE contract_address = $1",
        )
        .bind(CREATED)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(created, 17);
    }

    /// A pass alone after a successful one takes the hints the gate added
    /// since: the index reads them again at the start of every pass.
    #[tokio::test]
    async fn a_pass_alone_takes_the_hints_added_since_the_last() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        hint(&pool, 0, OLDER).await;
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let early = Chain {
            logs: fixture(),
            finalized: 97_700_000,
        };
        chain_walk::run_pass(&pool, &early, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        hint(&pool, 1, CREATED).await;
        let later = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        chain_walk::run_pass(&pool, &later, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        let created: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM shop.collection_event WHERE contract_address = $1",
        )
        .bind(CREATED)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(created, 17);
    }

    /// An operator's rebuild undoes the gate's reads in flight: a token
    /// taken after the job claimed the walk again, before the rebuild's
    /// cleanup, writes nothing after it; one taken after it writes.
    #[tokio::test]
    async fn a_rebuild_refuses_creation_reads_taken_before_it() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        hint(&pool, 0, OLDER).await;
        hint(&pool, 1, CREATED).await;
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let chain = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        // The operator's rebuild request, as DATABASE.md gives it.
        sqlx::query(
            "UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()), \
                    folded_block = NULL, folded_at = NULL, owner_generation = owner_generation + 1 \
             WHERE indexer = 'collections' AND network = 'matic'",
        )
        .execute(&pool)
        .await
        .unwrap();
        // A walk that rewound before: its epoch row exists already.
        sqlx::query("INSERT INTO shop.collection_walk (network, epoch) VALUES ('matic', 5)")
            .execute(&pool)
            .await
            .unwrap();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let in_flight = reads_token(&pool, ChainNet::Matic).await.unwrap();
        chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.collection_event")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left, 0, "the rebuild ran");
        let read = Seed {
            contract_address: CREATED.into(),
            name: "n".into(),
            symbol: "s".into(),
            creator: CREATOR.into(),
            is_approved: true,
            is_editable: true,
            read_block: 97_800_000,
        };
        assert_eq!(
            write_seeds(
                &pool,
                ChainNet::Matic,
                std::slice::from_ref(&read),
                &header(97_800_000).hash,
                in_flight
            )
            .await
            .unwrap(),
            0
        );
        let now = reads_token(&pool, ChainNet::Matic).await.unwrap();
        assert_eq!(
            write_seeds(
                &pool,
                ChainNet::Matic,
                &[read],
                &header(97_800_000).hash,
                now
            )
            .await
            .unwrap(),
            1
        );
    }

    /// A rewind makes the factory list rows read past it provisional: a
    /// branch that puts another collection at an index read final before
    /// gets its own read, and the window that creates that collection
    /// commits.
    #[tokio::test]
    async fn a_replacement_branch_gets_its_own_factory_list_reads() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        for (idx, address, read_block) in [(0, OLDER, 97_000_000i64), (1, CREATED, 97_750_000)] {
            sqlx::query(
                "INSERT INTO shop.collection_hint (network, factory, idx, address, finalized, read_block) \
                 VALUES ('matic', $1, $2, $3, true, $4)",
            )
            .bind(V3)
            .bind(idx)
            .bind(address)
            .bind(read_block)
            .execute(&pool)
            .await
            .unwrap();
        }
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let chain = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();

        // A reorg to 97,700,000: the branch past it creates `replacement`
        // where CREATED was, at the same list index.
        let replacement = "0x00000000000000000000000000000000000000b7";
        let to = 97_700_000i64;
        let cur = chain_walk::cursor(&pool, INDEXER_COLLECTIONS, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        let at = header(to as u64);
        let mut tx = pool.begin().await.unwrap();
        ChainIndex::rewind(&ix, &mut tx, to).await.unwrap();
        chain_walk::rewind(
            &mut tx,
            INDEXER_COLLECTIONS,
            ChainNet::Matic,
            (cur.last_block, cur.last_block_hash.as_deref()),
            to,
            Some((at.hash.as_str(), at.timestamp)),
        )
        .await
        .unwrap();
        owner.check(&mut tx).await.unwrap();
        tx.commit().await.unwrap();
        let finals: Vec<(String, bool)> =
            sqlx::query_as("SELECT address, finalized FROM shop.collection_hint ORDER BY idx")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            finals,
            vec![(OLDER.into(), true), (CREATED.into(), false)],
            "only the row read past the rewind is read again"
        );

        let size = Factory::collectionsSizeCall {}.abi_encode();
        let mut calls = HashMap::new();
        for block in ["0x64", "latest"] {
            calls.insert(
                call_key(FACTORIES[0].0, size.clone(), block),
                word(U256::ZERO),
            );
            calls.insert(call_key(V3, size.clone(), block), word(U256::from(2)));
        }
        calls.insert(call_key(V3, nth(1), "0x64"), addr(replacement));
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let refreshed = refresh_hints(&pool, &Rpc::at(&http, &url), &final_at(100))
            .await
            .unwrap();
        assert_eq!((refreshed.added, refreshed.remaining), (1, 0));

        let branch = Chain {
            logs: replaced(replacement),
            finalized: 97_895_000,
        };
        chain_walk::run_pass(&pool, &branch, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        assert_eq!(
            created_rows(&pool, replacement).await,
            vec![(replacement.to_string(), 17)]
        );
    }

    /// The fixture's logs on a branch that creates `replacement` where
    /// CREATED was: same blocks, same list index.
    fn replaced(replacement: &str) -> Vec<RpcLog> {
        let created_topic = format!("0x{:0>64}", &CREATED[2..]);
        let replacement_topic = format!("0x{:0>64}", &replacement[2..]);
        fixture()
            .into_iter()
            .map(|mut l| {
                if l.address.eq_ignore_ascii_case(CREATED) {
                    l.address = replacement.into();
                }
                for t in &mut l.topics {
                    if t.eq_ignore_ascii_case(&created_topic) {
                        *t = replacement_topic.clone();
                    }
                }
                l
            })
            .collect()
    }

    /// The walked rows of CREATED and `replacement`, per contract.
    async fn created_rows(pool: &PgPool, replacement: &str) -> Vec<(String, i64)> {
        sqlx::query_as(
            "SELECT contract_address, count(*) FROM shop.collection_event \
             WHERE contract_address IN ($1, $2) GROUP BY contract_address",
        )
        .bind(CREATED)
        .bind(replacement)
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// A V3 list row read final at `read_block` on the branch whose block
    /// there hashes to `read_hash`.
    async fn hint_read(pool: &PgPool, idx: i32, address: &str, read_block: u64, read_hash: &str) {
        sqlx::query(
            "INSERT INTO shop.collection_hint (network, factory, idx, address, finalized, read_block, read_hash) \
             VALUES ('matic', $1, $2, $3, true, $4, $5)",
        )
        .bind(V3)
        .bind(idx)
        .bind(address)
        .bind(read_block as i64)
        .bind(read_hash)
        .execute(pool)
        .await
        .unwrap();
    }

    /// The hash of a block on a branch the chain left: never the fake
    /// chain's.
    fn left_branch() -> String {
        format!("0x{}", "ab".repeat(32))
    }

    /// The fake endpoint's answers for a gate whose finalized block is
    /// `finalized`: v2's list empty and v3's holding two collections, at
    /// that block and the latest.
    fn gate_calls(finalized: u64) -> HashMap<(String, String, String), String> {
        let at = format!("0x{finalized:x}");
        let mut calls = HashMap::new();
        calls.insert(
            ("block".to_string(), "finalized".to_string(), String::new()),
            format!("{at} {}", header(finalized).hash),
        );
        let size = Factory::collectionsSizeCall {}.abi_encode();
        for block in [at.as_str(), "latest"] {
            calls.insert(
                call_key(FACTORIES[0].0, size.clone(), block),
                word(U256::ZERO),
            );
            calls.insert(call_key(V3, size.clone(), block), word(U256::from(2)));
        }
        calls
    }

    /// A list row read final ahead of the walk is bound to the hash of the
    /// block it was read at. When the chain leaves that branch above the
    /// walk's cursor (no rewind of the walk sees it) and puts another
    /// collection at the index, the window that creates it fails until the
    /// gate finds the hash changed and reads the index again; then it
    /// commits, with no rebuild.
    #[tokio::test]
    async fn a_list_slot_changed_ahead_of_the_walk_is_read_again() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        hint_read(&pool, 0, OLDER, 97_000_000, &header(97_000_000).hash).await;
        hint_read(&pool, 1, CREATED, 97_750_000, &left_branch()).await;
        let replacement = "0x00000000000000000000000000000000000000b7";
        let branch = |finalized| Chain {
            logs: replaced(replacement),
            finalized,
        };
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        chain_walk::run_pass(
            &pool,
            &branch(97_700_000),
            &ix,
            &settings,
            &owner,
            None,
            &|_| {},
        )
        .await
        .unwrap();
        let err = chain_walk::run_pass(
            &pool,
            &branch(97_895_000),
            &ix,
            &settings,
            &owner,
            None,
            &|_| {},
        )
        .await
        .unwrap_err();
        assert!(format!("{err:#}").contains(replacement), "{err:#}");

        let mut calls = gate_calls(97_895_000);
        calls.insert(
            call_key(V3, nth(1), &format!("0x{:x}", 97_895_000)),
            addr(replacement),
        );
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let before = reads_token(&pool, ChainNet::Matic).await.unwrap();
        assert_eq!(gate(&pool, &rpc).await.unwrap(), None);
        assert_ne!(
            reads_token(&pool, ChainNet::Matic).await.unwrap(),
            before,
            "a read in flight from the left branch is not written"
        );
        let hints: Vec<(i32, String, bool, Option<i64>, bool)> = sqlx::query_as(
            "SELECT idx, address, finalized, read_block, verified FROM shop.collection_hint ORDER BY idx, address",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            hints,
            vec![
                (0, OLDER.into(), true, Some(97_000_000), false),
                (1, replacement.into(), true, Some(97_895_000), false),
                (1, CREATED.into(), false, None, false),
            ],
            "the row whose hash holds kept (unproven: this walk lacks its full history); the left branch's read again"
        );
        chain_walk::run_pass(
            &pool,
            &branch(97_895_000),
            &ix,
            &settings,
            &owner,
            None,
            &|_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            created_rows(&pool, replacement).await,
            vec![(replacement.to_string(), 17)]
        );
    }

    /// The catalogue's name for `contract`, when it has a row.
    async fn projected_name(pool: &PgPool, contract: &str) -> Option<String> {
        sqlx::query_scalar(
            "SELECT name FROM shop.collections WHERE network = 'matic' AND contract_address = $1",
        )
        .bind(contract)
        .fetch_optional(pool)
        .await
        .unwrap()
    }

    /// `contract`'s five creation calls answering `name` at `block`.
    fn creation_answers(
        calls: &mut HashMap<(String, String, String), String>,
        contract: &str,
        block: &str,
        name: &str,
    ) {
        for (call, answer) in [
            (Collection::nameCall {}.abi_encode(), word(name.to_string())),
            (
                Collection::symbolCall {}.abi_encode(),
                word("S".to_string()),
            ),
            (Collection::creatorCall {}.abi_encode(), addr(CREATOR)),
            (Collection::isApprovedCall {}.abi_encode(), word(true)),
            (Collection::isEditableCall {}.abi_encode(), word(true)),
        ] {
            calls.insert(call_key(contract, call, block), answer);
        }
    }

    /// The final list reads go by the finalized block's hash: a list slot
    /// stored with that hash is that block's, even when the endpoint's block
    /// at that number is another branch's.
    #[tokio::test]
    async fn final_reads_are_pinned_to_the_finalized_hash() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (on_a, on_b) = (
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b2",
        );
        let a = left_branch();
        let size = Factory::collectionsSizeCall {}.abi_encode();
        let mut calls = HashMap::new();
        calls.insert(
            ("block".to_string(), "finalized".to_string(), String::new()),
            format!("0x64 {a}"),
        );
        for block in [a.as_str(), "0x64", "latest"] {
            calls.insert(
                call_key(FACTORIES[0].0, size.clone(), block),
                word(U256::ZERO),
            );
            calls.insert(call_key(V3, size.clone(), block), word(U256::from(1)));
        }
        calls.insert(call_key(V3, nth(0), &a), addr(on_a));
        calls.insert(call_key(V3, nth(0), "0x64"), addr(on_b));
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        assert_eq!(gate(&pool, &Rpc::at(&http, &url)).await.unwrap(), None);
        let hints: Vec<(String, Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT address, read_block, read_hash FROM shop.collection_hint WHERE finalized",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(hints, vec![(on_a.to_string(), Some(100), Some(a))]);
    }

    /// A creation read is taken at the walk's own cursor block, by the hash
    /// the walk stored for it, whatever the endpoint says of other blocks:
    /// with the endpoint's finalized block on branch A and the walk on
    /// branch B, the read is B's, proven as written, and the next pass
    /// publishes it. Before the walk has passed a creation there is nothing
    /// to read.
    #[tokio::test]
    async fn creation_reads_are_taken_at_the_walks_own_block() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let below = header(97_000_000).hash;
        hint_read(&pool, 0, OLDER, 97_000_000, &below).await;
        hint_read(&pool, 1, CREATED, 97_000_000, &below).await;
        let settings = walk_settings();
        let ix = CollectionsIndex::projecting(true);
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let pass = |finalized| {
            let chain = Chain {
                logs: fixture(),
                finalized,
            };
            let (pool, ix, settings, owner) = (&pool, &ix, &settings, &owner);
            async move {
                chain_walk::run_pass(pool, &chain, ix, settings, owner, None, &|_| {})
                    .await
                    .unwrap()
            }
        };
        pass(97_700_000).await;

        let a = left_branch();
        let b = format!("0x{:x}", 97_895_000);
        let mut calls = gate_calls(97_895_000);
        calls.insert(
            ("block".to_string(), "finalized".to_string(), String::new()),
            format!("{b} {a}"),
        );
        let size = Factory::collectionsSizeCall {}.abi_encode();
        calls.insert(call_key(FACTORIES[0].0, size.clone(), &a), word(U256::ZERO));
        calls.insert(call_key(V3, size, &a), word(U256::from(2)));
        creation_answers(&mut calls, CREATED, &a, "Left");
        creation_answers(&mut calls, CREATED, &b, "Kept");
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let seeds = || async {
            sqlx::query_as::<_, (String, i64, Option<String>, bool)>(
                "SELECT name, read_block, read_hash, verified FROM shop.collection_seed",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        };
        assert_eq!(gate(&pool, &rpc).await.unwrap(), None);
        assert_eq!(seeds().await, vec![], "nothing walked to read yet");
        pass(97_895_000).await;
        assert_eq!(
            projected_name(&pool, CREATED).await,
            None,
            "walked, waiting for its creation read"
        );
        assert_eq!(gate(&pool, &rpc).await.unwrap(), None);
        assert_eq!(
            seeds().await,
            vec![(
                "Kept".to_string(),
                97_895_000,
                Some(header(97_895_000).hash),
                true
            )],
            "branch B's state at the walk's block, proven as written"
        );
        pass(97_895_000).await;
        assert_eq!(
            projected_name(&pool, CREATED).await.as_deref(),
            Some("Kept")
        );
    }

    /// A list row at or below the cursor of a walk with its full history is
    /// decided by the walk's own log, with no header asked: the row of a
    /// collection the walk created by that height is proven, the row of one
    /// it never created there is undone.
    #[tokio::test]
    async fn the_walk_decides_the_list_rows_it_has_passed() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let elsewhere = "0x00000000000000000000000000000000000000e9";
        let read_at = 97_800_000u64;
        // CREATED is the first collection V3 created in this walk's log.
        hint_read(&pool, 0, CREATED, read_at, &left_branch()).await;
        hint_read(&pool, 1, elsewhere, read_at, &left_branch()).await;
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let chain = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        // As a walk from the factories' deployment holds it.
        sqlx::query(
            "UPDATE shop.chain_cursor SET full_history = true WHERE indexer = 'collections' AND network = 'matic'",
        )
        .execute(&pool)
        .await
        .unwrap();
        // A header asked at that height would fail the check.
        let mut calls = HashMap::new();
        calls.insert(
            ("block".to_string(), format!("0x{read_at:x}"), String::new()),
            "null".to_string(),
        );
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let before = reads_token(&pool, ChainNet::Matic).await.unwrap();
        assert_eq!(
            revalidate_reads(&pool, &rpc, HEIGHTS_PER_GATE)
                .await
                .unwrap(),
            1
        );
        assert_ne!(reads_token(&pool, ChainNet::Matic).await.unwrap(), before);
        let rows: Vec<(String, bool, bool)> = sqlx::query_as(
            "SELECT address, finalized, verified FROM shop.collection_hint ORDER BY idx",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                (CREATED.to_string(), true, true),
                (elsewhere.to_string(), false, false)
            ]
        );
    }

    /// A list row written before reads kept their block (by an index
    /// already published) needs no read where a full-history walk holds its
    /// exact slot: it is proven, with the creation's block as its height, so
    /// the published walk does not wait for its lists to be read again. One
    /// the walk does not hold is undone, to be read again.
    #[tokio::test]
    async fn legacy_list_rows_the_walk_holds_need_no_read() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let elsewhere = "0x00000000000000000000000000000000000000e9";
        hint(&pool, 0, CREATED).await;
        hint(&pool, 1, elsewhere).await;
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        let chain = Chain {
            logs: fixture(),
            finalized: 97_895_000,
        };
        chain_walk::run_pass(&pool, &chain, &ix, &settings, &owner, None, &|_| {})
            .await
            .unwrap();
        // As a walk from the factories' deployment holds it: here CREATED is
        // V3's first collection.
        sqlx::query(
            "UPDATE shop.chain_cursor SET full_history = true WHERE indexer = 'collections' AND network = 'matic'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let url = fake_rpc(Arc::default(), Batches::Upto(100), Arc::default()).await;
        let http = crate::http::Outbound::for_tests();
        assert_eq!(
            revalidate_reads(&pool, &Rpc::at(&http, &url), HEIGHTS_PER_GATE)
                .await
                .unwrap(),
            1
        );
        let rows: Vec<(String, bool, bool, Option<i64>)> = sqlx::query_as(
            "SELECT address, finalized, verified, read_block FROM shop.collection_hint ORDER BY idx",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                (CREATED.to_string(), true, true, Some(97_707_994)),
                (elsewhere.to_string(), false, false, None)
            ]
        );
    }

    /// With CREATE2 an address keeps its identity on a branch that created
    /// it in another order, so the walk's own order of a factory's
    /// `ProxyCreated` decides each slot. Branch A listed [Z, X]; the walked
    /// branch created X, then Y. Y's window fails until the gate undoes the
    /// rows read on A and reads the slots again as (0, X) and (1, Y); then it
    /// commits, with no rebuild.
    #[tokio::test]
    async fn a_changed_creation_order_is_read_into_the_walks_slots() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (z, y) = (
            "0x00000000000000000000000000000000000000a7",
            "0x00000000000000000000000000000000000000b8",
        );
        let read_at = 97_800_000u64;
        hint_read(&pool, 0, z, read_at, &left_branch()).await;
        hint_read(&pool, 1, CREATED, read_at, &left_branch()).await;
        // The walked branch: the fixture's creation of CREATED by V3 at
        // 97,707,994, then V3's creation of Y.
        let mut logs = fixture();
        logs.push(rpc_log(
            V3,
            Factory::ProxyCreated {
                _address: y.parse().unwrap(),
                _salt: B256::ZERO,
            }
            .encode_log_data(),
            97_810_000,
            0,
        ));
        let branch = |finalized| Chain {
            logs: logs.clone(),
            finalized,
        };
        let settings = walk_settings();
        let ix = CollectionsIndex::new();
        let owner = chain_walk::test_owner(&pool, &ix, &settings).await;
        chain_walk::run_pass(
            &pool,
            &branch(97_800_000),
            &ix,
            &settings,
            &owner,
            None,
            &|_| {},
        )
        .await
        .unwrap();
        // As a walk from the factories' deployment holds it: here CREATED is
        // V3's first collection.
        sqlx::query(
            "UPDATE shop.chain_cursor SET full_history = true WHERE indexer = 'collections' AND network = 'matic'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let err = chain_walk::run_pass(
            &pool,
            &branch(97_895_000),
            &ix,
            &settings,
            &owner,
            None,
            &|_| {},
        )
        .await
        .unwrap_err();
        assert!(format!("{err:#}").contains(y), "{err:#}");

        let at = format!("0x{:x}", 97_895_000);
        let mut calls = gate_calls(97_895_000);
        calls.insert(call_key(V3, nth(0), &at), addr(CREATED));
        calls.insert(call_key(V3, nth(1), &at), addr(y));
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        assert_eq!(gate(&pool, &Rpc::at(&http, &url)).await.unwrap(), None);
        let finals: Vec<(i32, String)> = sqlx::query_as(
            "SELECT idx, address FROM shop.collection_hint WHERE finalized ORDER BY idx",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(finals, vec![(0, CREATED.to_string()), (1, y.to_string())]);
        chain_walk::run_pass(
            &pool,
            &branch(97_895_000),
            &ix,
            &settings,
            &owner,
            None,
            &|_| {},
        )
        .await
        .unwrap();
        let walked: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM shop.collection_event WHERE kind = 'proxy_created' AND contract_address = $1",
        )
        .bind(y)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(walked, 1, "Y's window committed");
    }

    /// An endpoint that takes no batches still has its headers read, one
    /// request each, attempt after attempt; a list row with no hash to check
    /// is undone without asking for its block, even one the endpoint does
    /// not serve.
    #[tokio::test]
    async fn the_gate_checks_reads_on_an_endpoint_without_batches() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (a, b, c, legacy) = (
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b2",
            "0x00000000000000000000000000000000000000c3",
            "0x00000000000000000000000000000000000000d4",
        );
        hint_read(&pool, 0, a, 100, &header(100).hash).await;
        hint_read(&pool, 1, b, 100, &left_branch()).await;
        sqlx::query(
            "INSERT INTO shop.collection_hint (network, factory, idx, address, finalized, read_block) \
             VALUES ('matic', $1, 2, $2, true, 7)",
        )
        .bind(V3)
        .bind(legacy)
        .execute(&pool)
        .await
        .unwrap();
        let mut calls = gate_calls(100);
        calls.insert(
            ("block".to_string(), "0x7".to_string(), String::new()),
            "null".into(),
        );
        calls.insert(call_key(V3, nth(1), "0x64"), addr(c));
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::BadRequest,
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let state = || async {
            sqlx::query_as::<_, (String, bool, Option<String>)>(
                "SELECT address, finalized, read_hash FROM shop.collection_hint ORDER BY idx, address",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        };
        let expected = vec![
            (a.to_string(), true, Some(header(100).hash)),
            (b.to_string(), false, None),
            (c.to_string(), true, Some(header(100).hash)),
            (legacy.to_string(), false, None),
        ];
        for attempt in 0..2 {
            assert_eq!(gate(&pool, &rpc).await.unwrap(), None, "attempt {attempt}");
            assert_eq!(state().await, expected, "attempt {attempt}");
        }
    }

    /// The heights checked in one attempt are bounded, and the next attempt
    /// goes on where the last stopped, so a changed block past the bound is
    /// still found.
    #[tokio::test]
    async fn the_heights_checked_go_round_in_turn() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let addresses = [
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b2",
            "0x00000000000000000000000000000000000000c3",
        ];
        hint_read(&pool, 0, addresses[0], 10, &header(10).hash).await;
        hint_read(&pool, 1, addresses[1], 20, &header(20).hash).await;
        hint_read(&pool, 2, addresses[2], 30, &left_branch()).await;
        let url = fake_rpc(Arc::default(), Batches::Upto(100), Arc::default()).await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        assert_eq!(revalidate_reads(&pool, &rpc, 2).await.unwrap(), 0);
        assert_eq!(
            revalidate_reads(&pool, &rpc, 2).await.unwrap(),
            1,
            "30, then 10 again"
        );
        let finals: Vec<String> = sqlx::query_scalar(
            "SELECT address FROM shop.collection_hint WHERE finalized ORDER BY idx",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(finals, addresses[..2].to_vec());
    }

    /// How the fake endpoint takes a JSON-RPC batch.
    #[derive(Clone, Copy, Debug)]
    enum Batches {
        /// Up to this many calls; a 413 past it.
        Upto(usize),
        /// None: an error object, with a 200 (an endpoint that takes none).
        Refused,
        /// None: a 400 (plain requests only).
        BadRequest,
        /// Up to this many calls; past it, geth's answer: one -32600 error
        /// with the first request's id.
        Geth(usize),
    }

    /// The fake endpoint's `eth_call` answers by (to, data hex, block): a
    /// hex result, or `error:<code>` for a refusal.
    type Answers = Arc<std::sync::Mutex<HashMap<(String, String, String), String>>>;

    /// A JSON-RPC endpoint for the gate: chain 137, finalized block 100, and
    /// `calls` answering `eth_call`; "0x" otherwise (no contract there). It
    /// takes batches as `batches` says. `seen` counts the `eth_call`s it
    /// answered.
    async fn fake_rpc(
        calls: Answers,
        batches: Batches,
        seen: Arc<std::sync::atomic::AtomicUsize>,
    ) -> String {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use axum::{Json, extract::State, routing::post};
        fn answer(
            calls: &HashMap<(String, String, String), String>,
            req: &Value,
            seen: &std::sync::atomic::AtomicUsize,
        ) -> Value {
            let id = req["id"].clone();
            let result = match req["method"].as_str().unwrap_or_default() {
                "eth_chainId" => json!("0x89"),
                // `("block", tag, "")` overrides a header: "<number hex> <hash>",
                // or "null" for a block the endpoint does not serve.
                "eth_getBlockByNumber" => {
                    let tag = req["params"][0].as_str().unwrap_or_default().to_string();
                    match calls.get(&("block".to_string(), tag.clone(), String::new())) {
                        Some(v) if v == "null" => Value::Null,
                        Some(v) => {
                            let (n, h) = v.split_once(' ').unwrap();
                            json!({ "number": n, "hash": h, "timestamp": "0x5f5e1000" })
                        }
                        None if tag == "finalized" => json!({
                            "number": "0x64", "hash": format!("0x{:064x}", 100), "timestamp": "0x5f5e1000"
                        }),
                        None => {
                            let n = u64::from_str_radix(tag.trim_start_matches("0x"), 16).unwrap();
                            json!({ "number": tag, "hash": format!("0x{n:064x}"), "timestamp": "0x5f5e1000" })
                        }
                    }
                }
                "eth_call" => {
                    seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let p = &req["params"];
                    // A read pinned to the fake chain's own hash of block n
                    // (0x{n:064x}) answers as a read at n; one pinned to any
                    // other hash answers by that hash.
                    let block = match &p[1] {
                        Value::String(tag) => tag.clone(),
                        pinned => {
                            let hash = pinned["blockHash"].as_str().unwrap().to_string();
                            u64::from_str_radix(hash.trim_start_matches("0x"), 16)
                                .ok()
                                .filter(|n| format!("0x{n:064x}") == hash)
                                .map_or(hash, |n| format!("0x{n:x}"))
                        }
                    };
                    let key = (
                        p[0]["to"].as_str().unwrap().to_lowercase(),
                        p[0]["data"].as_str().unwrap().to_string(),
                        block,
                    );
                    let found = calls.get(&key).cloned().unwrap_or_else(|| "0x".into());
                    if let Some(code) = found.strip_prefix("error:") {
                        let code: i64 = code.parse().unwrap();
                        return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": "refused" } });
                    }
                    json!(found)
                }
                _ => {
                    return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "no" } });
                }
            };
            json!({ "jsonrpc": "2.0", "id": id, "result": result })
        }
        type Shared = (Answers, Batches, Arc<std::sync::atomic::AtomicUsize>);
        async fn handle(
            State((calls, batches, seen)): State<Shared>,
            Json(body): Json<Value>,
        ) -> axum::response::Response {
            let calls = calls.lock().unwrap().clone();
            match (body, batches) {
                (Value::Array(_), Batches::Refused) => Json(json!({
                    "jsonrpc": "2.0", "id": null, "error": { "code": -32600, "message": "batches are not supported" }
                }))
                .into_response(),
                (Value::Array(_), Batches::BadRequest) => StatusCode::BAD_REQUEST.into_response(),
                (Value::Array(reqs), Batches::Upto(max)) if reqs.len() > max => {
                    StatusCode::PAYLOAD_TOO_LARGE.into_response()
                }
                (Value::Array(reqs), Batches::Geth(max)) if reqs.len() > max => Json(json!([{
                    "jsonrpc": "2.0", "id": reqs[0]["id"], "error": { "code": -32600, "message": "batch too large" }
                }]))
                .into_response(),
                (Value::Array(reqs), _) => {
                    Json(Value::Array(reqs.iter().map(|r| answer(&calls, r, &seen)).collect())).into_response()
                }
                (one, _) => Json(answer(&calls, &one, &seen)).into_response(),
            }
        }
        let app = axum::Router::new()
            .route("/", post(handle))
            .with_state((calls, batches, seen));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}")
    }

    /// The fake chain's header at `number`, as the gate reads the finalized one.
    fn final_at(number: u64) -> BlockHeader {
        BlockHeader {
            number,
            hash: format!("0x{number:064x}"),
            timestamp: 0,
        }
    }

    fn call_key(to: &str, data: Vec<u8>, block: &str) -> (String, String, String) {
        (
            to.to_string(),
            format!("0x{}", hex::encode(data)),
            block.to_string(),
        )
    }

    fn word(v: impl SolValue) -> String {
        format!("0x{}", hex::encode(v.abi_encode()))
    }

    fn nth(i: u64) -> Vec<u8> {
        Factory::collectionsCall {
            index: U256::from(i),
        }
        .abi_encode()
    }

    fn addr(a: &str) -> String {
        word(a.parse::<Address>().unwrap())
    }

    /// The gate reads each list index once at the finalized block and the
    /// newer ones at the latest block until they are final; an index whose
    /// answer is missing fails the attempt and is read again, never skipped.
    /// No creation state is read for a listed collection the walk has not
    /// created, even one the finalized block answers for.
    #[tokio::test]
    async fn the_gate_reads_the_factory_lists() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (v2, v3) = (FACTORIES[0].0, FACTORIES[1].0);
        let a = "0x00000000000000000000000000000000000000a1";
        let b = "0x00000000000000000000000000000000000000b2";
        let c = "0x00000000000000000000000000000000000000c3";
        let p = "0x00000000000000000000000000000000000000e5";
        let size = Factory::collectionsSizeCall {}.abi_encode();
        let mut calls = HashMap::new();
        for block in ["0x64", "latest"] {
            calls.insert(call_key(v2, size.clone(), block), word(U256::from(2)));
        }
        calls.insert(call_key(v3, size.clone(), "0x64"), word(U256::from(1)));
        calls.insert(call_key(v3, size.clone(), "latest"), word(U256::from(2)));
        calls.insert(call_key(v2, nth(0), "0x64"), addr(a));
        calls.insert(call_key(v3, nth(0), "0x64"), addr(b));
        calls.insert(call_key(v3, nth(1), "latest"), addr(p));
        for (call, answer) in [
            (
                Collection::nameCall {}.abi_encode(),
                word("Alpha".to_string()),
            ),
            (
                Collection::symbolCall {}.abi_encode(),
                word("ALP".to_string()),
            ),
            (Collection::creatorCall {}.abi_encode(), addr(CREATOR)),
            (Collection::isApprovedCall {}.abi_encode(), word(false)),
            (Collection::isEditableCall {}.abi_encode(), word(true)),
        ] {
            calls.insert(call_key(a, call, "0x64"), answer);
        }
        let calls = Arc::new(std::sync::Mutex::new(calls));
        let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let url = fake_rpc(calls.clone(), Batches::Upto(100), seen.clone()).await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let hints = || async {
            sqlx::query_as::<_, (String, i32, String, bool)>(
                "SELECT factory, idx, address, finalized FROM shop.collection_hint ORDER BY address",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        };
        // v2's index 1 does not answer: the attempt fails, the read hints stay.
        let err = gate(&pool, &rpc).await.unwrap_err();
        assert!(format!("{err:#}").contains(&format!("{v2}[1]")), "{err:#}");
        let seeded: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.collection_seed")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(seeded, 0, "nothing past a list it could not read whole");
        calls
            .lock()
            .unwrap()
            .insert(call_key(v2, nth(1), "0x64"), addr(c));
        assert_eq!(gate(&pool, &rpc).await.unwrap(), None);
        assert_eq!(
            hints().await,
            vec![
                (v2.into(), 0, a.into(), true),
                (v3.into(), 0, b.into(), true),
                (v2.into(), 1, c.into(), true),
                (v3.into(), 1, p.into(), false),
            ]
        );
        let seeds: Vec<(String, String, String, String, bool, bool, i64)> = sqlx::query_as(
            "SELECT contract_address, name, symbol, creator, is_approved, is_editable, read_block FROM shop.collection_seed",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            seeds,
            vec![],
            "creation reads wait for the walk to create each collection"
        );

        // The provisional index is read again at every refresh; a reorg put
        // another collection there, and the finalized block reaches it.
        let d = "0x00000000000000000000000000000000000000d4";
        calls
            .lock()
            .unwrap()
            .insert(call_key(v3, nth(1), "latest"), addr(d));
        assert_eq!(
            refresh_hints(&pool, &rpc, &final_at(100))
                .await
                .unwrap()
                .added,
            1,
            "d, provisional"
        );
        calls
            .lock()
            .unwrap()
            .insert(call_key(v3, size.clone(), "0x64"), word(U256::from(2)));
        calls
            .lock()
            .unwrap()
            .insert(call_key(v3, nth(1), "0x64"), addr(d));
        assert_eq!(
            refresh_hints(&pool, &rpc, &final_at(100))
                .await
                .unwrap()
                .added,
            1,
            "d made final"
        );
        assert_eq!(
            hints()
                .await
                .into_iter()
                .filter(|h| h.0 == v3)
                .collect::<Vec<_>>(),
            vec![
                (v3.into(), 0, b.into(), true),
                (v3.into(), 1, d.into(), true),
                (v3.into(), 1, p.into(), false)
            ],
            "the address the reorg replaced stays a provisional hint"
        );
        // Every list final up to the finalized block: only the four sizes.
        let before = seen.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            refresh_hints(&pool, &rpc, &final_at(100))
                .await
                .unwrap()
                .added,
            0
        );
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst) - before, 4);
    }

    /// A fresh node's lists are read over several attempts, at most
    /// [`HINT_READS_PER_REFRESH`] indexes each, the attempt waiting until
    /// they are whole; what a refresh read is kept even when an index in it
    /// is refused.
    #[tokio::test]
    async fn long_lists_are_read_over_several_attempts() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (v2, v3) = (FACTORIES[0].0, FACTORIES[1].0);
        let total = HINT_READS_PER_REFRESH as u64 + 200;
        let size = Factory::collectionsSizeCall {}.abi_encode();
        let mut calls = HashMap::new();
        for block in ["0x64", "latest"] {
            calls.insert(call_key(v2, size.clone(), block), word(U256::from(total)));
            calls.insert(call_key(v3, size.clone(), block), word(U256::ZERO));
        }
        for i in 0..total {
            let a = format!("0x{:040x}", 0x1000 + i);
            calls.insert(call_key(v2, nth(i), "0x64"), addr(&a));
        }
        calls.insert(call_key(v2, nth(450), "0x64"), "error:-32005".into());
        let calls = Arc::new(std::sync::Mutex::new(calls));
        let url = fake_rpc(calls.clone(), Batches::Upto(100), Arc::default()).await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let count = || async {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.collection_hint")
                .fetch_one(&pool)
                .await
                .unwrap()
        };
        let err = gate(&pool, &rpc).await.unwrap_err();
        assert!(
            format!("{err:#}").contains(&format!("{v2}[450]")),
            "{err:#}"
        );
        assert_eq!(
            count().await,
            HINT_READS_PER_REFRESH as i64 - 1,
            "the chunks around it kept"
        );
        calls.lock().unwrap().insert(
            call_key(v2, nth(450), "0x64"),
            addr(&format!("0x{:040x}", 0x1000 + 450)),
        );
        assert_eq!(
            gate(&pool, &rpc).await.unwrap(),
            None,
            "201 left: one attempt"
        );
        assert_eq!(count().await, total as i64);

        let fresh = crate::db::scratch::isolated_pool(2).await.unwrap();
        let waiting = gate(&fresh, &rpc).await.unwrap();
        assert_eq!(
            waiting.as_deref(),
            Some("collections matic: 200 factory list indexes still to read")
        );
        assert_eq!(gate(&fresh, &rpc).await.unwrap(), None);
    }

    /// A size no list reaches fails the refresh at once, before any index
    /// is enumerated or read.
    #[tokio::test]
    async fn an_implausible_list_size_fails_without_reading() {
        let Some(pool) = crate::db::scratch::isolated_pool(1).await else {
            return;
        };
        let size = Factory::collectionsSizeCall {}.abi_encode();
        for huge in [
            U256::from(u64::MAX),
            U256::from(MAX_LIST_SIZE) + U256::from(1),
        ] {
            let mut calls = HashMap::new();
            for (f, _) in FACTORIES {
                for block in ["0x64", "latest"] {
                    calls.insert(call_key(f, size.clone(), block), word(huge));
                }
            }
            let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let url = fake_rpc(
                Arc::new(std::sync::Mutex::new(calls)),
                Batches::Upto(100),
                seen.clone(),
            )
            .await;
            let http = crate::http::Outbound::for_tests();
            let started = std::time::Instant::now();
            let err = refresh_hints(&pool, &Rpc::at(&http, &url), &final_at(100))
                .await
                .unwrap_err();
            assert!(started.elapsed() < std::time::Duration::from_secs(2));
            assert!(format!("{err:#}").contains("past 1000000"), "{err:#}");
            assert_eq!(
                seen.load(std::sync::atomic::Ordering::SeqCst),
                4,
                "the sizes alone"
            );
        }
    }

    /// The final indexes of both lists come before any provisional one, and
    /// a provisional tail longer than one refresh never holds the attempt
    /// back once the lists are final: consecutive gates walk.
    #[tokio::test]
    async fn a_long_provisional_tail_never_holds_the_walk() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (v2, v3) = (FACTORIES[0].0, FACTORIES[1].0);
        let size = Factory::collectionsSizeCall {}.abi_encode();
        let tail = HINT_READS_PER_REFRESH as u64 + 1;
        let mut calls = HashMap::new();
        calls.insert(call_key(v2, size.clone(), "0x64"), word(U256::from(600)));
        calls.insert(
            call_key(v2, size.clone(), "latest"),
            word(U256::from(600 + tail)),
        );
        calls.insert(call_key(v3, size.clone(), "0x64"), word(U256::from(600)));
        calls.insert(call_key(v3, size.clone(), "latest"), word(U256::from(600)));
        for i in 0..600 {
            calls.insert(
                call_key(v2, nth(i), "0x64"),
                addr(&format!("0x{:040x}", 0x1000 + i)),
            );
            calls.insert(
                call_key(v3, nth(i), "0x64"),
                addr(&format!("0x{:040x}", 0x9000 + i)),
            );
        }
        for i in 600..600 + tail {
            calls.insert(
                call_key(v2, nth(i), "latest"),
                addr(&format!("0x{:040x}", 0x1000 + i)),
            );
        }
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, &url);
        let first = refresh_hints(&pool, &rpc, &final_at(100)).await.unwrap();
        assert_eq!(
            (first.remaining, first.provisional_unread),
            (200, tail as usize),
            "v2's 600 and 400 of v3's: every final read before a provisional one"
        );
        let second = refresh_hints(&pool, &rpc, &final_at(100)).await.unwrap();
        assert_eq!(
            (second.remaining, second.provisional_unread),
            (0, tail as usize - 800)
        );
        for _ in 0..2 {
            assert_eq!(
                gate(&pool, &rpc).await.unwrap(),
                None,
                "the final lists are whole"
            );
        }
        let provisional: i64 =
            sqlx::query_scalar("SELECT count(*) FROM shop.collection_hint WHERE NOT finalized")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(provisional, HINT_READS_PER_REFRESH as i64);
        let seeded: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.collection_seed")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(seeded, 0, "no code anywhere: nothing seeded");
    }

    /// Holes are counted below the finalized size the endpoint answers now:
    /// a final row stored past it (read through an endpoint that was ahead)
    /// does not hide a hole below it.
    #[tokio::test]
    async fn a_lagging_endpoint_still_reads_the_holes_below_its_size() {
        let Some(pool) = crate::db::scratch::isolated_pool(1).await else {
            return;
        };
        let (v2, v3) = (FACTORIES[0].0, FACTORIES[1].0);
        sqlx::query(
            "INSERT INTO shop.collection_hint (network, factory, idx, address, finalized) \
             VALUES ('matic', $1, 1, '0x00000000000000000000000000000000000000b1', true)",
        )
        .bind(v2)
        .execute(&pool)
        .await
        .unwrap();
        let size = Factory::collectionsSizeCall {}.abi_encode();
        let a = "0x00000000000000000000000000000000000000a0";
        let mut calls = HashMap::new();
        for block in ["0x64", "latest"] {
            calls.insert(call_key(v2, size.clone(), block), word(U256::from(1)));
            calls.insert(call_key(v3, size.clone(), block), word(U256::ZERO));
        }
        calls.insert(call_key(v2, nth(0), "0x64"), addr(a));
        let url = fake_rpc(
            Arc::new(std::sync::Mutex::new(calls)),
            Batches::Upto(100),
            Arc::default(),
        )
        .await;
        let http = crate::http::Outbound::for_tests();
        let refreshed = refresh_hints(&pool, &Rpc::at(&http, &url), &final_at(100))
            .await
            .unwrap();
        assert_eq!((refreshed.added, refreshed.remaining), (1, 0));
        let read: bool = sqlx::query_scalar(
            "SELECT finalized FROM shop.collection_hint WHERE factory = $1 AND idx = 0 AND address = $2",
        )
        .bind(v2)
        .bind(a)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(read);
    }

    /// An endpoint that does not take batches is asked each call alone; one
    /// that refuses a large batch is sent halves; a call refused with an
    /// error keeps its code, a call that returned nothing is no answer.
    #[tokio::test]
    async fn calls_adapt_to_the_endpoints_batches() {
        let to = "0x00000000000000000000000000000000000000a1";
        let name = Collection::nameCall {}.abi_encode();
        let symbol = Collection::symbolCall {}.abi_encode();
        let mut answers = HashMap::new();
        answers.insert(
            call_key(to, name.clone(), "latest"),
            word("Alpha".to_string()),
        );
        answers.insert(
            call_key(to, symbol.clone(), "latest"),
            "error:-32005".to_string(),
        );
        let answers = Arc::new(std::sync::Mutex::new(answers));
        let http = crate::http::Outbound::for_tests();
        let calls: Vec<(String, Vec<u8>)> = [
            &name,
            &symbol,
            &name,
            &name,
            &Collection::creatorCall {}.abi_encode(),
        ]
        .into_iter()
        .map(|d| (to.to_string(), d.clone()))
        .collect();
        for batches in [
            Batches::Refused,
            Batches::Upto(2),
            Batches::Upto(100),
            Batches::BadRequest,
            Batches::Geth(2),
        ] {
            let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let url = fake_rpc(answers.clone(), batches, seen.clone()).await;
            let got = eth_calls(&Rpc::at(&http, &url), &calls, &json!("latest"))
                .await
                .unwrap();
            let alpha = Ok(Some("Alpha".to_string().abi_encode()));
            assert_eq!(
                got,
                vec![alpha.clone(), Err(-32005), alpha.clone(), alpha, Ok(None)],
                "{batches:?}"
            );
            assert_eq!(
                seen.load(std::sync::atomic::Ordering::SeqCst),
                calls.len(),
                "each call answered once"
            );
            assert_eq!(refusals(&got), Some((1, -32005)));
        }
    }

    /// An endpoint of another chain is refused before anything is read.
    #[tokio::test]
    async fn the_gate_refuses_another_chain() {
        let Some(pool) = crate::db::scratch::isolated_pool(1).await else {
            return;
        };
        use axum::{Json, routing::post};
        let app = axum::Router::new().route(
            "/",
            post(|Json(req): Json<Value>| async move {
                Json(json!({ "jsonrpc": "2.0", "id": req["id"], "result": "0x13882" }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let http = crate::http::Outbound::for_tests();
        let url = format!("http://{addr}");
        let err = gate(&pool, &Rpc::at(&http, &url)).await.unwrap_err();
        assert!(format!("{err:#}").contains("chain"), "{err:#}");
        let hints: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.collection_hint")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hints, 0);
    }
}
