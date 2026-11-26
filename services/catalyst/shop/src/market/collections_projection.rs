//! The collections index's projection: `shop.collection_event` and
//! `shop.collection_seed`, folded ([`super::collections_fold`]) into the
//! catalogue rows of `shop.collections` and `shop.items` for one network.
//!
//! The first projection takes the network's tables from the collections
//! mirror (`shop.collection_projection.owned_at`, set in its transaction):
//! from then the fold is their only writer, and the mirror refuses to write
//! the network. Both take the network's advisory lock around the ownership
//! check and the write ([`CATALOGUE_LOCK`]), the mirror shared and the fold
//! exclusive, so a mirror page that read the network as unowned commits
//! before the projection that takes it, and none commits after.
//!
//! A projection stands at a block (`projected_through`). The next one folds
//! the events past it onto the rows ([`collections_fold::resume`]); a
//! collection with no row yet (created since, or waiting for its creation
//! read) or an event that names an item without a row (its metadata took no
//! type) is folded again from its creation instead. A rewind clears
//! `projected_through`, and the next fold rebuilds every row of the network
//! from the whole log, in one transaction, so a reader sees the old rows or
//! the new ones.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use alloy_primitives::U256;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Row};

use super::chain_walk::{ChainNet, Owner};
use super::collections_fold::{
    self, CollectionRow, EMOTE_V1, ItemRow, Mints, OUTCOMES, Pending, SMART_WEARABLE_V1,
    WEARABLE_V2,
};
use super::collections_index::{
    CollectionEvent, KIND_ADD_ITEM, KIND_COMPLETE, KIND_CREATORSHIP_TRANSFERRED, KIND_ISSUE,
    KIND_OWNERSHIP_TRANSFERRED, KIND_PROXY_CREATED, KIND_RESCUE_ITEM, KIND_SET_APPROVED,
    KIND_SET_EDITABLE, KIND_SET_GLOBAL_MANAGER, KIND_SET_GLOBAL_MINTER, KIND_SET_ITEM_MANAGER,
    KIND_SET_ITEM_MINTER, KIND_UPDATE_ITEM_DATA, Seed,
};
use super::sales_index::STORE_MATIC;
use crate::shared::now_utc;

/// The first key of the catalogue's advisory lock (`pg_advisory_xact_lock`
/// with the network's chain id as the second): `coll`.
pub const CATALOGUE_LOCK: i32 = 0x636f_6c6c;

/// Rows per insert statement.
const ROWS_PER_STATEMENT: usize = 1_000;

const KINDS: [&str; 14] = [
    KIND_PROXY_CREATED,
    KIND_ADD_ITEM,
    KIND_RESCUE_ITEM,
    KIND_UPDATE_ITEM_DATA,
    KIND_ISSUE,
    KIND_SET_GLOBAL_MINTER,
    KIND_SET_GLOBAL_MANAGER,
    KIND_SET_ITEM_MINTER,
    KIND_SET_ITEM_MANAGER,
    KIND_SET_APPROVED,
    KIND_SET_EDITABLE,
    KIND_COMPLETE,
    KIND_CREATORSHIP_TRANSFERRED,
    KIND_OWNERSHIP_TRANSFERRED,
];

/// The `shop.collection_event` columns [`event_of`] reads.
macro_rules! event_columns {
    () => {
        "block_number, log_index, block_time, kind, contract_address, item_id::text AS item_id, account, \
         previous_account, flag, amount::text AS amount, max_supply::text AS max_supply, rarity, beneficiary, \
         metadata, content_hash"
    };
}

/// What one projection wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Projected {
    /// It folded every walked collection from its creation.
    pub rebuilt: bool,
    /// The collections whose rows it wrote.
    pub collections: usize,
    /// Of those, the ones folded from their creation.
    pub replayed: usize,
    /// Walked collections still waiting for their creation read.
    pub waiting: usize,
}

fn event_of(row: &sqlx::postgres::PgRow) -> CollectionEvent {
    let kind: String = row.get("kind");
    CollectionEvent {
        block_number: row.get("block_number"),
        log_index: row.get("log_index"),
        block_time: row.get("block_time"),
        kind: KINDS.iter().copied().find(|k| *k == kind).unwrap_or(""),
        contract_address: row.get("contract_address"),
        item_id: row.get("item_id"),
        account: row.get("account"),
        previous_account: row.get("previous_account"),
        flag: row.get("flag"),
        amount: row.get("amount"),
        max_supply: row.get("max_supply"),
        rarity: row.get("rarity"),
        beneficiary: row.get("beneficiary"),
        metadata: row.get("metadata"),
        content_hash: row.get("content_hash"),
    }
}

fn u256(s: &str) -> U256 {
    U256::from_str_radix(s, 10).unwrap_or_default()
}

/// Takes the network's catalogue lock for the rest of the transaction:
/// exclusive for the fold, shared for the mirror.
pub async fn lock(conn: &mut PgConnection, net: ChainNet, exclusive: bool) -> sqlx::Result<()> {
    let sql = if exclusive {
        "SELECT pg_advisory_xact_lock($1, $2)"
    } else {
        "SELECT pg_advisory_xact_lock_shared($1, $2)"
    };
    sqlx::query(sql)
        .bind(CATALOGUE_LOCK)
        .bind(net.chain_id() as i32)
        .execute(conn)
        .await?;
    Ok(())
}

/// Whether the collections index's fold owns `net`'s catalogue rows.
pub async fn owned(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.collection_projection WHERE network = $1 AND owned_at IS NOT NULL)",
    )
    .bind(net.db_name())
    .fetch_one(conn)
    .await
}

/// Folds `net`'s log to `upto` into its catalogue rows, taking them from the
/// mirror if it is the first projection, in one transaction fenced by
/// `owner`.
pub async fn project(
    pool: &PgPool,
    owner: &Owner,
    net: ChainNet,
    upto: i64,
) -> anyhow::Result<Projected> {
    let mut tx = pool.begin().await?;
    lock(&mut tx, net, true).await?;
    sqlx::query(
        "INSERT INTO shop.collection_projection (network) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind(net.db_name())
    .execute(&mut *tx)
    .await?;
    let through: Option<i64> = sqlx::query_scalar(
        "SELECT projected_through FROM shop.collection_projection WHERE network = $1 FOR UPDATE",
    )
    .bind(net.db_name())
    .fetch_one(&mut *tx)
    .await?;
    let out = match through {
        Some(through) if through <= upto => advance(&mut tx, net, through, upto).await?,
        _ => rebuild(&mut tx, net, upto).await?,
    };
    sqlx::query(
        "UPDATE shop.collection_projection SET owned_at = COALESCE(owned_at, timezone('UTC', now())), \
         projected_through = $2, pending = $3, generation = generation + 1, \
         updated_at = timezone('UTC', now()) WHERE network = $1",
    )
    .bind(net.db_name())
    .bind(upto)
    .bind(out.waiting as i32)
    .execute(&mut *tx)
    .await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(out)
}

/// Hands `net`'s catalogue back to the collections mirror, in one
/// transaction under the exclusive catalogue lock: the network's rows and
/// the mirror's pass markers go, and the ownership is cleared. Every read of
/// the network then waits for the mirror's next complete passes, which
/// rewrite it whole. Run it after the index process stops owning the network
/// (`MARKETPLACE_COLLECTIONS_INDEX_OWNS` without it), or its next fold takes
/// the network again. It does nothing, and answers false, when the index does
/// not own the network.
pub async fn hand_back(pool: &PgPool, net: ChainNet) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    lock(&mut tx, net, true).await?;
    // Only while the index owns it: a repeated hand-back (one whose commit
    // went unconfirmed) must not erase what the mirror wrote since.
    if !owned(&mut tx, net).await? {
        return Ok(false);
    }
    for sql in [
        "DELETE FROM shop.items WHERE network = $1",
        "DELETE FROM shop.collections WHERE network = $1",
        "DELETE FROM shop.mirror_state WHERE network = $1 AND mirror IN ('collections', 'items')",
        "UPDATE shop.collection_projection SET owned_at = NULL, projected_through = NULL, pending = 0, \
         generation = generation + 1, updated_at = timezone('UTC', now()) WHERE network = $1",
    ] {
        sqlx::query(sql)
            .bind(net.db_name())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Clears the block the projection stands at, in the transaction of a
/// rewind: the next fold rebuilds the network's rows.
pub async fn invalidate(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE shop.collection_projection SET projected_through = NULL, updated_at = timezone('UTC', now()) \
         WHERE network = $1",
    )
    .bind(net.db_name())
    .execute(conn)
    .await?;
    Ok(())
}

/// Every walked collection from its creation; the network's rows replaced.
async fn rebuild(conn: &mut PgConnection, net: ChainNet, upto: i64) -> anyhow::Result<Projected> {
    let folded = replay(conn, net, None, upto).await?;
    sqlx::query("DELETE FROM shop.items WHERE network = $1")
        .bind(net.db_name())
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM shop.collections WHERE network = $1")
        .bind(net.db_name())
        .execute(&mut *conn)
        .await?;
    write(conn, net, &folded.rows).await?;
    Ok(Projected {
        rebuilt: true,
        collections: folded.rows.len(),
        replayed: folded.rows.len(),
        waiting: folded.waiting,
    })
}

/// The collections folded, and how many wait for their creation read.
struct Folded {
    rows: Vec<(CollectionRow, Vec<ItemRow>)>,
    waiting: usize,
}

/// Folds `contracts` (every walked collection when `None`) from their
/// creation to `upto`: their events but the mints, the mints of their item
/// minters holding a finite allowance, and their items' mints aggregated.
async fn replay(
    conn: &mut PgConnection,
    net: ChainNet,
    contracts: Option<&[String]>,
    upto: i64,
) -> anyhow::Result<Folded> {
    let all = contracts.is_none();
    let wanted: Vec<String> = contracts.map(<[String]>::to_vec).unwrap_or_default();
    let rows = sqlx::query(concat!(
            "SELECT ",
            event_columns!(),
            " FROM shop.collection_event \
         WHERE network = $1 AND kind <> 'issue' AND block_number <= $2 AND ($3 OR contract_address = ANY($4)) \
         ORDER BY contract_address, block_number, log_index"
        ))
    .bind(net.db_name())
    .bind(upto)
    .bind(all)
    .bind(&wanted)
    .fetch_all(&mut *conn)
    .await?;
    let mut events: BTreeMap<String, Vec<CollectionEvent>> = BTreeMap::new();
    for row in &rows {
        let e = event_of(row);
        events
            .entry(e.contract_address.clone())
            .or_default()
            .push(e);
    }
    // Every collection's events begin at its creation: the mints are read
    // from the oldest one's block on.
    let since = events
        .values()
        .filter_map(|es| es.iter().find(|e| e.kind == KIND_PROXY_CREATED))
        .map(|e| e.block_number)
        .min()
        .unwrap_or(upto + 1);
    let scope: Vec<String> = events.keys().cloned().collect();
    let tracked: Vec<(String, String, String)> = events
        .iter()
        .flat_map(|(c, es)| {
            collections_fold::tracked_minters(es)
                .into_iter()
                .map(move |(item, minter)| (c.clone(), item, minter))
        })
        .collect();
    if !tracked.is_empty() {
        let (cs, rest): (Vec<String>, Vec<(String, String)>) =
            tracked.into_iter().map(|(c, i, m)| (c, (i, m))).unzip();
        let (items, minters): (Vec<String>, Vec<String>) = rest.into_iter().unzip();
        let issues = sqlx::query(concat!(
            "SELECT ",
            event_columns!(),
            " FROM shop.collection_event e \
             JOIN UNNEST($3::text[], $4::text[], $5::text[]) AS t(c, i, m) \
               ON e.contract_address = t.c AND e.item_id = t.i::numeric AND e.account = t.m \
             WHERE e.network = $1 AND e.kind = 'issue' AND e.block_number BETWEEN $6 AND $2"
        ))
        .bind(net.db_name())
        .bind(upto)
        .bind(&cs)
        .bind(&items)
        .bind(&minters)
        .bind(since)
        .fetch_all(&mut *conn)
        .await?;
        for row in &issues {
            let e = event_of(row);
            events
                .entry(e.contract_address.clone())
                .or_default()
                .push(e);
        }
        for es in events.values_mut() {
            es.sort_by_key(|e| (e.block_number, e.log_index));
        }
    }
    let minted = sqlx::query(
        "SELECT contract_address, item_id::text AS item_id, max(amount)::text AS max_issued, \
                max(block_time) FILTER (WHERE account = $5) AS sold_at \
         FROM shop.collection_event \
         WHERE network = $1 AND kind = 'issue' AND block_number BETWEEN $3 AND $2 \
           AND ($4 OR contract_address = ANY($6)) \
         GROUP BY contract_address, item_id",
    )
    .bind(net.db_name())
    .bind(upto)
    .bind(since)
    .bind(all)
    .bind(STORE_MATIC)
    .bind(&scope)
    .fetch_all(&mut *conn)
    .await?;
    let mut mints: HashMap<String, HashMap<U256, Mints>> = HashMap::new();
    for row in &minted {
        let contract: String = row.get("contract_address");
        let item: String = row.get("item_id");
        let max: Option<String> = row.get("max_issued");
        mints.entry(contract).or_default().insert(
            u256(&item),
            Mints {
                max_issued: max.as_deref().map(u256).unwrap_or_default(),
                primary_sold_at: row.get("sold_at"),
            },
        );
    }
    // Only a creation read proven on the walked branch: one of a branch the
    // chain left must not reach the catalogue before the gate undoes it.
    let seeds: HashMap<String, Seed> = sqlx::query(
        "SELECT contract_address, name, symbol, creator, is_approved, is_editable, read_block \
         FROM shop.collection_seed WHERE network = $1 AND contract_address = ANY($2) AND verified",
    )
    .bind(net.db_name())
    .bind(&scope)
    .fetch_all(&mut *conn)
    .await?
    .iter()
    .map(|r| {
        let seed = Seed {
            contract_address: r.get("contract_address"),
            name: r.get("name"),
            symbol: r.get("symbol"),
            creator: r.get("creator"),
            is_approved: r.get("is_approved"),
            is_editable: r.get("is_editable"),
            read_block: r.get("read_block"),
        };
        (seed.contract_address.clone(), seed)
    })
    .collect();
    let none = HashMap::new();
    let mut out = Folded {
        rows: Vec::new(),
        waiting: 0,
    };
    for (contract, es) in &events {
        let minted = mints.get(contract).unwrap_or(&none);
        match collections_fold::fold(contract, es, seeds.get(contract), minted, upto) {
            Ok(rows) => out.rows.push(rows),
            Err(Pending::Seed) => out.waiting += 1,
            // Events of a collection whose creation is past `upto` or was
            // never walked: nothing to project.
            Err(Pending::NotCreated) => {}
        }
    }
    Ok(out)
}

/// Folds the events past `through`, up to `upto`, onto the rows; replays the
/// collections it cannot resume.
async fn advance(
    conn: &mut PgConnection,
    net: ChainNet,
    through: i64,
    upto: i64,
) -> anyhow::Result<Projected> {
    let rows = sqlx::query(concat!(
            "SELECT ",
            event_columns!(),
            " FROM shop.collection_event \
         WHERE network = $1 AND block_number > $2 AND block_number <= $3 ORDER BY block_number, log_index"
        ))
    .bind(net.db_name())
    .bind(through)
    .bind(upto)
    .fetch_all(&mut *conn)
    .await?;
    let mut fresh: BTreeMap<String, Vec<CollectionEvent>> = BTreeMap::new();
    for row in &rows {
        let e = event_of(row);
        fresh.entry(e.contract_address.clone()).or_default().push(e);
    }
    // Walked collections without a row: created since, or still waiting
    // for their creation read.
    let unprojected: BTreeSet<String> = sqlx::query_scalar(
        "SELECT e.contract_address FROM shop.collection_event e \
         WHERE e.network = $1 AND e.kind = 'proxy_created' AND e.block_number <= $2 \
           AND NOT EXISTS (SELECT 1 FROM shop.collections c \
                           WHERE c.network = $1 AND c.contract_address = e.contract_address)",
    )
    .bind(net.db_name())
    .bind(upto)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    let touched: Vec<String> = fresh
        .keys()
        .filter(|c| !unprojected.contains(*c))
        .cloned()
        .collect();
    let mut stored = load(conn, net, &touched).await?;
    let mut written = Vec::new();
    let mut again: Vec<String> = unprojected.into_iter().collect();
    for contract in touched {
        let Some((collection, items)) = stored.remove(&contract) else {
            // Events of a collection the fold never created: a rewind
            // would have rebuilt it; replay it from what the log holds.
            again.push(contract);
            continue;
        };
        match collections_fold::resume(collection, items, &fresh[&contract]) {
            Ok(rows) => written.push(rows),
            Err(collections_fold::NeedsReplay) => again.push(contract),
        }
    }
    let mut out = Projected {
        collections: written.len(),
        ..Default::default()
    };
    if !again.is_empty() {
        let folded = replay(conn, net, Some(&again), upto).await?;
        out.replayed = folded.rows.len();
        out.collections += folded.rows.len();
        out.waiting = folded.waiting;
        written.extend(folded.rows);
    }
    let contracts: Vec<&str> = written
        .iter()
        .map(|(c, _)| c.contract_address.as_str())
        .collect();
    sqlx::query("DELETE FROM shop.items WHERE network = $1 AND contract_address = ANY($2)")
        .bind(net.db_name())
        .bind(&contracts)
        .execute(&mut *conn)
        .await?;
    write(conn, net, &written).await?;
    Ok(out)
}

/// The stored rows of `contracts`, as the fold reads them back.
async fn load(
    conn: &mut PgConnection,
    net: ChainNet,
    contracts: &[String],
) -> anyhow::Result<HashMap<String, (CollectionRow, Vec<ItemRow>)>> {
    if contracts.is_empty() {
        return Ok(HashMap::new());
    }
    let mut out: HashMap<String, (CollectionRow, Vec<ItemRow>)> = HashMap::new();
    for r in sqlx::query(
        "SELECT contract_address, name, symbol, creator, owner, urn, is_completed, is_approved, is_editable, \
                items_count, minters, managers, search_is_store_minter, search_text, created_at, updated_at, \
                reviewed_at, first_listed_at \
         FROM shop.collections WHERE network = $1 AND contract_address = ANY($2)",
    )
    .bind(net.db_name())
    .bind(contracts)
    .fetch_all(&mut *conn)
    .await?
    {
        let c = CollectionRow {
            contract_address: r.get("contract_address"),
            name: r.get("name"),
            symbol: r.get::<Option<String>, _>("symbol").unwrap_or_default(),
            creator: r.get("creator"),
            owner: r.get("owner"),
            urn: r.get("urn"),
            is_completed: r.get("is_completed"),
            is_approved: r.get("is_approved"),
            is_editable: r.get("is_editable"),
            items_count: r.get("items_count"),
            minters: r.get("minters"),
            managers: r.get("managers"),
            search_is_store_minter: r.get("search_is_store_minter"),
            search_text: r.get::<Option<String>, _>("search_text").unwrap_or_default(),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            reviewed_at: r.get("reviewed_at"),
            first_listed_at: r.get("first_listed_at"),
        };
        out.insert(c.contract_address.clone(), (c, Vec::new()));
    }
    for r in sqlx::query(
        "SELECT contract_address, item_id::text AS item_id, item_type, urn, creator, beneficiary, rarity, \
                total_supply::text AS total_supply, max_supply::text AS max_supply, price::text AS price, name, \
                description, category, body_shapes, emote_loop, emote_has_sound, emote_has_geometry, \
                emote_outcome_type, minters, minter_allowances::text[] AS minter_allowances, \
                search_is_store_minter, search_is_collection_approved, search_text, created_at, updated_at, \
                reviewed_at, sold_at, first_listed_at, content_hash \
         FROM shop.items WHERE network = $1 AND contract_address = ANY($2) ORDER BY contract_address, item_id",
    )
    .bind(net.db_name())
    .bind(contracts)
    .fetch_all(&mut *conn)
    .await?
    {
        let contract: String = r.get("contract_address");
        let Some((_, items)) = out.get_mut(&contract) else {
            continue;
        };
        let item_type: String = r.get("item_type");
        let outcome: Option<String> = r.get("emote_outcome_type");
        let allowances: Vec<String> = r.get("minter_allowances");
        items.push(ItemRow {
            item_id: u256(&r.get::<String, _>("item_id")),
            item_type: [WEARABLE_V2, SMART_WEARABLE_V1, EMOTE_V1]
                .into_iter()
                .find(|t| *t == item_type)
                .unwrap_or(WEARABLE_V2),
            urn: r.get("urn"),
            creator: r.get("creator"),
            beneficiary: r.get::<Option<String>, _>("beneficiary").unwrap_or_default(),
            rarity: r.get("rarity"),
            total_supply: u256(&r.get::<String, _>("total_supply")),
            max_supply: u256(&r.get::<String, _>("max_supply")),
            available: U256::ZERO,
            price: u256(&r.get::<String, _>("price")),
            image: String::new(),
            name: r.get("name"),
            description: r.get::<Option<String>, _>("description").unwrap_or_default(),
            category: r.get("category"),
            body_shapes: r.get("body_shapes"),
            is_smart: item_type == SMART_WEARABLE_V1,
            emote_loop: r.get("emote_loop"),
            emote_has_sound: r.get("emote_has_sound"),
            emote_has_geometry: r.get("emote_has_geometry"),
            emote_outcome_type: outcome
                .and_then(|o| OUTCOMES.iter().find(|(_, name)| *name == o).map(|(_, name)| *name)),
            minters: r.get("minters"),
            minter_allowances: allowances.iter().map(|a| u256(a)).collect(),
            search_is_store_minter: r.get("search_is_store_minter"),
            search_is_collection_approved: r.get("search_is_collection_approved"),
            search_text: r.get::<Option<String>, _>("search_text").unwrap_or_default(),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            reviewed_at: r.get("reviewed_at"),
            sold_at: r.get("sold_at"),
            first_listed_at: r.get("first_listed_at"),
            content_hash: r.get("content_hash"),
        });
    }
    Ok(out)
}

fn collection_record(c: &CollectionRow) -> Value {
    json!({
        "contract_address": c.contract_address, "name": c.name, "symbol": c.symbol, "creator": c.creator,
        "owner": c.owner, "urn": c.urn, "is_completed": c.is_completed, "is_approved": c.is_approved,
        "is_editable": c.is_editable, "items_count": c.items_count, "minters": c.minters, "managers": c.managers,
        "search_is_store_minter": c.search_is_store_minter, "search_text": c.search_text,
        "created_at": c.created_at, "updated_at": c.updated_at, "reviewed_at": c.reviewed_at,
        "first_listed_at": c.first_listed_at,
    })
}

fn item_record(contract: &str, i: &ItemRow) -> Value {
    json!({
        "contract_address": contract, "item_id": i.item_id.to_string(), "item_type": i.item_type, "urn": i.urn,
        "creator": i.creator, "beneficiary": i.beneficiary, "rarity": i.rarity,
        "total_supply": i.total_supply.to_string(), "max_supply": i.max_supply.to_string(),
        "available": i.available.to_string(), "price": i.price.to_string(), "image": i.image, "name": i.name,
        "description": i.description, "category": i.category, "body_shapes": i.body_shapes, "is_smart": i.is_smart,
        "emote_loop": i.emote_loop, "emote_has_sound": i.emote_has_sound, "emote_has_geometry": i.emote_has_geometry,
        "emote_outcome_type": i.emote_outcome_type, "search_is_store_minter": i.search_is_store_minter,
        "search_is_collection_approved": i.search_is_collection_approved, "created_at": i.created_at,
        "updated_at": i.updated_at, "reviewed_at": i.reviewed_at, "sold_at": i.sold_at,
        "first_listed_at": i.first_listed_at, "search_text": i.search_text, "minters": i.minters,
        "minter_allowances": i.minter_allowances.iter().map(U256::to_string).collect::<Vec<_>>(),
        "content_hash": i.content_hash,
    })
}

/// Writes `rows`: each collection upserted, its items inserted (the caller
/// deleted the ones they replace).
async fn write(
    conn: &mut PgConnection,
    net: ChainNet,
    rows: &[(CollectionRow, Vec<ItemRow>)],
) -> anyhow::Result<()> {
    let chain_id = net.chain_id() as i32;
    let mirrored_at = now_utc();
    let collections: Vec<Value> = rows.iter().map(|(c, _)| collection_record(c)).collect();
    for chunk in collections.chunks(ROWS_PER_STATEMENT) {
        sqlx::query(
            "INSERT INTO shop.collections \
                 (network, contract_address, chain_id, name, symbol, creator, owner, urn, is_completed, is_approved, \
                  is_editable, items_count, minters, managers, search_is_store_minter, search_text, created_at, \
                  updated_at, reviewed_at, first_listed_at, mirrored_at) \
             SELECT $1, r.contract_address, $2, r.name, r.symbol, r.creator, r.owner, r.urn, r.is_completed, \
                    r.is_approved, r.is_editable, r.items_count, r.minters, r.managers, r.search_is_store_minter, \
                    r.search_text, r.created_at, r.updated_at, r.reviewed_at, r.first_listed_at, $3 \
             FROM jsonb_to_recordset($4) AS r(contract_address text, name text, symbol text, creator text, \
                  owner text, urn text, is_completed boolean, is_approved boolean, is_editable boolean, \
                  items_count integer, minters text[], managers text[], search_is_store_minter boolean, \
                  search_text text, created_at bigint, updated_at bigint, reviewed_at bigint, first_listed_at bigint) \
             ON CONFLICT (network, contract_address) DO UPDATE SET \
                 chain_id = EXCLUDED.chain_id, name = EXCLUDED.name, symbol = EXCLUDED.symbol, \
                 creator = EXCLUDED.creator, owner = EXCLUDED.owner, urn = EXCLUDED.urn, \
                 is_completed = EXCLUDED.is_completed, is_approved = EXCLUDED.is_approved, \
                 is_editable = EXCLUDED.is_editable, items_count = EXCLUDED.items_count, \
                 minters = EXCLUDED.minters, managers = EXCLUDED.managers, \
                 search_is_store_minter = EXCLUDED.search_is_store_minter, search_text = EXCLUDED.search_text, \
                 created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at, \
                 reviewed_at = EXCLUDED.reviewed_at, first_listed_at = EXCLUDED.first_listed_at, \
                 mirrored_at = EXCLUDED.mirrored_at",
        )
        .bind(net.db_name())
        .bind(chain_id)
        .bind(mirrored_at)
        .bind(Value::Array(chunk.to_vec()))
        .execute(&mut *conn)
        .await?;
    }
    let items: Vec<Value> = rows
        .iter()
        .flat_map(|(c, items)| items.iter().map(|i| item_record(&c.contract_address, i)))
        .collect();
    for chunk in items.chunks(ROWS_PER_STATEMENT) {
        sqlx::query(
            "INSERT INTO shop.items \
                 (network, contract_address, item_id, chain_id, item_type, urn, creator, beneficiary, rarity, \
                  total_supply, max_supply, available, price, image, name, description, category, body_shapes, \
                  is_smart, emote_loop, emote_has_sound, emote_has_geometry, emote_outcome_type, \
                  search_is_store_minter, search_is_collection_approved, created_at, updated_at, reviewed_at, \
                  sold_at, first_listed_at, mirrored_at, search_text, minters, minter_allowances, content_hash) \
             SELECT $1, r.contract_address, r.item_id::numeric, $2, r.item_type, r.urn, r.creator, r.beneficiary, \
                    r.rarity, r.total_supply::numeric, r.max_supply::numeric, r.available::numeric, r.price::numeric, \
                    r.image, r.name, r.description, r.category, r.body_shapes, r.is_smart, r.emote_loop, \
                    r.emote_has_sound, r.emote_has_geometry, r.emote_outcome_type, r.search_is_store_minter, \
                    r.search_is_collection_approved, r.created_at, r.updated_at, r.reviewed_at, r.sold_at, \
                    r.first_listed_at, $3, r.search_text, r.minters, r.minter_allowances::numeric[], r.content_hash \
             FROM jsonb_to_recordset($4) AS r(contract_address text, item_id text, item_type text, urn text, \
                  creator text, beneficiary text, rarity text, total_supply text, max_supply text, available text, \
                  price text, image text, name text, description text, category text, body_shapes text[], \
                  is_smart boolean, emote_loop boolean, emote_has_sound boolean, emote_has_geometry boolean, \
                  emote_outcome_type text, search_is_store_minter boolean, search_is_collection_approved boolean, \
                  created_at bigint, updated_at bigint, reviewed_at bigint, sold_at bigint, first_listed_at bigint, \
                  search_text text, minters text[], minter_allowances text[], content_hash text)",
        )
        .bind(net.db_name())
        .bind(chain_id)
        .bind(mirrored_at)
        .bind(Value::Array(chunk.to_vec()))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain_index::rpc::RpcLog;
    use crate::market::chain_walk::{self, IndexSettings};
    use crate::market::collections_index::{self, CollectionsIndex};
    use crate::market::graph::Network;
    use crate::market::mirrors;

    const CREATED: &str = "0x5f255679f7192bae8440746311757c4a5f752107";
    const MIRRORED_ONLY: &str = "0x00000000000000000000000000000000000000ee";
    const NET: ChainNet = ChainNet::Matic;

    /// The walk's real fixture, as rows of the log.
    fn logged() -> Vec<CollectionEvent> {
        let logs: Vec<RpcLog> = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/shop/collections-polygon-logs.json"
        )))
        .unwrap();
        logs.iter()
            .filter_map(|l| {
                let block = chain_walk::log_position(l).0;
                collections_index::decode_log(l, 1_600_000_000 + block).unwrap()
            })
            .collect()
    }

    async fn seed_token(pool: &PgPool) -> collections_index::ReadsToken {
        collections_index::reads_token(pool, NET).await.unwrap()
    }

    /// The hash of the walk's block the creation reads are taken at.
    const WALKED: &str = "0x00000000000000000000000000000000000000000000000000000000000000aa";

    /// Writes `seeds` as the gate does: taken at the walk's block, proven
    /// (`verified`), the only ones a projection takes.
    async fn proven(pool: &PgPool, seeds: &[Seed]) {
        collections_index::write_seeds(pool, NET, seeds, WALKED, seed_token(pool).await)
            .await
            .unwrap();
    }

    fn seed() -> Seed {
        Seed {
            contract_address: CREATED.into(),
            name: "World Track Memorabilia".into(),
            symbol: "DCL-WRLDTRCKMMRBL".into(),
            creator: "0x23be90335e79d3245615985d91f69814f98a0fab".into(),
            is_approved: true,
            is_editable: true,
            read_block: 97_700_000,
        }
    }

    /// A database holding the log of the fixture up to `upto`, the
    /// collection's creation read when `seeded`, the mirror's copy of a
    /// collection the log does not have, and the walk's projection owner.
    async fn setup(seeded: bool) -> Option<(PgPool, Owner)> {
        let pool = crate::db::scratch::isolated_pool(4).await?;
        let mut conn = pool.acquire().await.unwrap();
        collections_index::insert_events(&mut conn, NET, &logged())
            .await
            .unwrap();
        drop(conn);
        if seeded {
            proven(&pool, &[seed()]).await;
        }
        mirrors::collections::upsert(
            &pool,
            Network::Matic,
            137,
            false,
            &[mirrors::collections::fixture(
                MIRRORED_ONLY,
                "Mirror",
                "0xc",
                true,
            )],
        )
        .await
        .unwrap();
        let ix = CollectionsIndex::projecting(true);
        let owner = chain_walk::test_owner(&pool, &ix, &IndexSettings::default()).await;
        Some((pool, owner))
    }

    async fn catalogue(pool: &PgPool) -> (Vec<String>, Vec<String>) {
        let collections: Vec<String> = sqlx::query_scalar(
            "SELECT row(contract_address, name, symbol, creator, owner, urn, is_completed, is_approved, is_editable, \
                        items_count, minters, managers, search_is_store_minter, search_text, created_at, updated_at, \
                        reviewed_at, first_listed_at)::text \
             FROM shop.collections WHERE network = 'matic' ORDER BY contract_address",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let items: Vec<String> = sqlx::query_scalar(
            "SELECT row(contract_address, item_id, item_type, urn, creator, beneficiary, rarity, total_supply, \
                        max_supply, available, price, image, name, description, category, body_shapes, is_smart, \
                        emote_loop, emote_has_sound, emote_has_geometry, emote_outcome_type, search_is_store_minter, \
                        search_is_collection_approved, created_at, updated_at, reviewed_at, sold_at, \
                        first_listed_at, search_text, minters, minter_allowances, content_hash)::text \
             FROM shop.items WHERE network = 'matic' ORDER BY contract_address, item_id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        (collections, items)
    }

    async fn marker(pool: &PgPool) -> (bool, Option<i64>) {
        sqlx::query_as(
            "SELECT owned_at IS NOT NULL, projected_through FROM shop.collection_projection WHERE network = 'matic'",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// The first projection takes the network: the mirror's rows go, the
    /// log's collection and items are written, and the marker records it.
    #[tokio::test]
    async fn the_first_projection_takes_the_network_from_the_mirror() {
        let Some((pool, owner)) = setup(true).await else {
            return;
        };
        let out = project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert_eq!(
            out,
            Projected {
                rebuilt: true,
                collections: 1,
                replayed: 1,
                waiting: 0
            }
        );
        assert_eq!(marker(&pool).await, (true, Some(97_895_000)));
        let (name, approved, completed, items): (String, bool, bool, i32) = sqlx::query_as(
            "SELECT name, is_approved, is_completed, items_count FROM shop.collections WHERE network = 'matic'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (name.as_str(), approved, completed, items),
            ("World Track Memorabilia", true, true, 2)
        );
        let supply: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT item_id::text, total_supply::text, available::text FROM shop.items WHERE network = 'matic' ORDER BY item_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            supply,
            vec![
                ("0".into(), "3".into(), "997".into()),
                ("1".into(), "2".into(), "98".into())
            ]
        );
    }

    /// Once the index owns the network, a rebuild writes each item's
    /// on-chain content hash, the one its newest `AddItem` or `RescueItem`
    /// logged, so a minted item still reads as published unchanged.
    #[tokio::test]
    async fn a_rebuild_keeps_each_items_content_hash() {
        let Some((pool, owner)) = setup(true).await else {
            return;
        };
        project(&pool, &owner, NET, 97_895_000).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        invalidate(&mut conn, NET).await.unwrap();
        drop(conn);
        assert!(
            project(&pool, &owner, NET, 97_895_000)
                .await
                .unwrap()
                .rebuilt
        );
        let logged: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT DISTINCT ON (item_id) item_id::text, content_hash FROM shop.collection_event \
             WHERE network = 'matic' AND kind IN ('add_item', 'rescue_item') \
             ORDER BY item_id, block_number DESC, log_index DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(logged.iter().all(|(_, hash)| hash.is_some()));
        let written: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT item_id::text, content_hash FROM shop.items WHERE network = 'matic' ORDER BY item_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(written, logged);
    }

    /// Once the index owns the network, a mirror page for it is not
    /// written; another network's still is.
    #[tokio::test]
    async fn the_mirror_does_not_write_a_network_the_index_owns() {
        let Some((pool, owner)) = setup(true).await else {
            return;
        };
        project(&pool, &owner, NET, 97_895_000).await.unwrap();
        let before = catalogue(&pool).await;
        let page = [mirrors::collections::fixture(
            CREATED,
            "Overwritten",
            "0xc",
            false,
        )];
        assert_eq!(
            mirrors::collections::upsert(&pool, Network::Matic, 137, false, &page)
                .await
                .unwrap(),
            0
        );
        let items = [mirrors::items::fixture(
            CREATED,
            "0",
            "wearable_v2",
            "Overwritten",
        )];
        assert_eq!(
            mirrors::items::upsert(&pool, Network::Matic, 137, false, &items)
                .await
                .unwrap(),
            0
        );
        assert_eq!(catalogue(&pool).await, before);
        assert_eq!(
            mirrors::collections::upsert(&pool, Network::Ethereum, 1, false, &page)
                .await
                .unwrap(),
            1
        );
    }

    /// A projection resumed from the rows writes what a rebuild to the
    /// same block writes, and a rewind makes the next one a rebuild.
    #[tokio::test]
    async fn a_resumed_projection_equals_a_rebuild() {
        let Some((pool, owner)) = setup(true).await else {
            return;
        };
        let first = project(&pool, &owner, NET, 97_751_000).await.unwrap();
        assert!(first.rebuilt);
        let second = project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert_eq!(
            second,
            Projected {
                rebuilt: false,
                collections: 1,
                replayed: 0,
                waiting: 0
            },
            "the mints and minters past 97,751,000 folded onto the rows"
        );
        let resumed = catalogue(&pool).await;
        let mut conn = pool.acquire().await.unwrap();
        invalidate(&mut conn, NET).await.unwrap();
        drop(conn);
        assert_eq!(marker(&pool).await, (true, None));
        let rebuilt = project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert!(rebuilt.rebuilt);
        assert_eq!(catalogue(&pool).await, resumed);
    }

    /// A walked collection without its creation read waits, without
    /// holding the projection back; once read, the next projection writes it.
    #[tokio::test]
    async fn a_collection_waits_for_its_creation_read() {
        let Some((pool, owner)) = setup(false).await else {
            return;
        };
        let out = project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert_eq!((out.collections, out.waiting), (0, 1));
        assert_eq!(catalogue(&pool).await, (vec![], vec![]));
        assert_eq!(
            completeness(&pool).await,
            (1, 1),
            "one collection pending: not complete"
        );
        // A row written before reads were proven (no hash): not taken.
        sqlx::query(
            "INSERT INTO shop.collection_seed (network, contract_address, name, symbol, creator, is_approved, is_editable, read_block) \
             VALUES ('matic', $1, 'Legacy', 'L', '0x23be90335e79d3245615985d91f69814f98a0fab', true, true, 97700000)",
        )
        .bind(CREATED)
        .execute(&pool)
        .await
        .unwrap();
        let out = project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert_eq!(
            (out.collections, out.waiting),
            (0, 1),
            "a read not proven on the walked branch is not taken"
        );
        proven(&pool, &[seed()]).await;
        let out = project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert_eq!(
            (out.rebuilt, out.collections, out.replayed, out.waiting),
            (false, 1, 1, 0)
        );
        assert_eq!(catalogue(&pool).await.1.len(), 2);
        assert_eq!(
            completeness(&pool).await,
            (0, 3),
            "the late read, once proven, completes it at the same block"
        );
    }

    /// The marker's pending count and generation, for the next change's reads.
    async fn completeness(pool: &PgPool) -> (i32, i64) {
        sqlx::query_as(
            "SELECT pending, generation FROM shop.collection_projection WHERE network = 'matic'",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn approved(pool: &PgPool) -> bool {
        sqlx::query_scalar("SELECT is_approved FROM shop.collections WHERE network = 'matic'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn seeds(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM shop.collection_seed")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn rewind_to(pool: &PgPool, to: i64) {
        let ix = CollectionsIndex::projecting(true);
        let mut tx = pool.begin().await.unwrap();
        crate::market::chain_walk::ChainIndex::rewind(&ix, &mut tx, to)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    /// A rewind drops the creation reads taken past it: a read of a branch
    /// the chain left (one that saw an approval the new branch lacks) is
    /// read again, and the fold takes the new one.
    #[tokio::test]
    async fn a_rewind_drops_the_creation_reads_past_it() {
        let Some((pool, owner)) = setup(false).await else {
            return;
        };
        let read_after_approval = Seed {
            read_block: 97_740_000,
            ..seed()
        };
        proven(&pool, &[read_after_approval]).await;
        project(&pool, &owner, NET, 97_895_000).await.unwrap();
        assert!(approved(&pool).await);
        // The branch past 97,720,000, the approval at 97,732,808 with it, is gone.
        rewind_to(&pool, 97_720_000).await;
        assert_eq!(seeds(&pool).await, 0);
        let new_branch = Seed {
            is_approved: false,
            read_block: 97_721_000,
            ..seed()
        };
        proven(&pool, &[new_branch]).await;
        let out = project(&pool, &owner, NET, 97_721_000).await.unwrap();
        assert!(out.rebuilt);
        assert!(
            !approved(&pool).await,
            "the new branch's read, not the old one's"
        );
    }

    /// Creation reads taken before a rewind (or a new owner) are not
    /// written after it; one written while it runs is deleted by it.
    #[tokio::test]
    async fn creation_reads_never_outlive_the_rewind_they_raced() {
        let Some((pool, _owner)) = setup(false).await else {
            return;
        };
        let before = seed_token(&pool).await;
        rewind_to(&pool, 97_800_000).await;
        assert_eq!(
            collections_index::write_seeds(&pool, NET, &[seed()], WALKED, before)
                .await
                .unwrap(),
            0,
            "taken before the rewind"
        );

        let mut writing = pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock_shared($1, 137)")
            .bind(collections_index::READS_LOCK)
            .execute(&mut *writing)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO shop.collection_seed (network, contract_address, name, symbol, creator, is_approved, is_editable, read_block) \
             VALUES ('matic', $1, 'n', 's', '0x23be90335e79d3245615985d91f69814f98a0fab', true, true, 97800000)",
        )
        .bind(CREATED)
        .execute(&mut *writing)
        .await
        .unwrap();
        let rewinding = {
            let pool = pool.clone();
            tokio::spawn(async move { rewind_to(&pool, 97_720_000).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            !rewinding.is_finished(),
            "the rewind waits for the write in flight"
        );
        writing.commit().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), rewinding)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(seeds(&pool).await, 0, "deleted by the rewind it raced");
    }

    /// A mirror page that read the network as unowned holds the shared lock
    /// to its commit: the projection that takes the network waits for it,
    /// then replaces what it wrote.
    #[tokio::test]
    async fn the_projection_waits_for_a_mirror_page_in_flight() {
        let Some((pool, owner)) = setup(true).await else {
            return;
        };
        let mut page = pool.begin().await.unwrap();
        lock(&mut page, NET, false).await.unwrap();
        assert!(!owned(&mut page, NET).await.unwrap());
        sqlx::query("UPDATE shop.collections SET name = 'in flight' WHERE network = 'matic'")
            .execute(&mut *page)
            .await
            .unwrap();
        let projecting = {
            let (pool, owner) = (pool.clone(), owner.clone());
            tokio::spawn(async move { project(&pool, &owner, NET, 97_895_000).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            !projecting.is_finished(),
            "the projection waits for the page"
        );
        page.commit().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), projecting)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM shop.collections WHERE network = 'matic'")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(names, vec!["World Track Memorabilia".to_string()]);
    }
}
