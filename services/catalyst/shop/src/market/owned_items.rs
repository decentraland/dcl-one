//! Wearable and emote ownership from the `nft` index (`shop.nft`): an
//! owner's copies for the inventory routes (`/lambdas/users/{a}/wearables`
//! and `/emotes`, the explorer backpack, `/v1/users`, the
//! `*-by-owner` lists), and the tokens each owner holds of the URNs a
//! profile or an outfit names.
//!
//! Every read runs in one `REPEATABLE READ, READ ONLY` transaction that
//! first checks the index's cursor ([`snapshot`]), so a rewind that commits
//! between the check and the rows cannot strip an owner's items. Two rules:
//!
//! - [`Rule::Complete`], for profiles: the network's walk covers the whole
//!   history and its last fold was published, with no rewind or unpublish
//!   since (`full_history` and `folded_block` set). The fold's rows are what
//!   that publish covered or a later fold of the same walk, so an RPC
//!   outage keeps the profiles served from them.
//! - [`Rule::Fresh`], for the lists, the backpack, `/v1/users`, outfits,
//!   orders and `/v1/nfts`: complete, and published within
//!   [`READY_WINDOW`].
//!
//! Neither rule compares `folded_block` with `last_block`: every pass
//! advances `last_block` before it folds and publishes, so that equality is
//! false for part of every pass while the table still holds a published
//! fold.
//!
//! A v2 token's URN is `urn:decentraland:<network>:collections-v2:<contract>:<item id>`,
//! the item id decoded from its token id; a v1 token's is its collection's
//! URN (`shop.collections`) and the wearable id of its Issue log. The
//! inventory lists also join the item (`shop.items`) for its name, rarity,
//! category and price, and need the items mirror's first complete pass.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use axum::response::Response;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};

use crate::shared::cache::TtlMap;

use super::chain_walk::{self, ChainNet, Cursor, INDEXER_NFT, READY_WINDOW};
use super::nft_index::DCL_REGISTRAR;
use super::{MarketplaceState, Source, is_valid_eth_address, unavailable};

/// How long an owner's copies are memoized: a burst of backpack, list and
/// `/v1/users` reads of one owner costs one query.
pub const OWNED_TTL: Duration = Duration::from_secs(30);
const OWNED_CACHE_MAX_ENTRIES: usize = 20_000;

/// One copy of a collection item an owner holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedCopy {
    pub urn: String,
    pub token_id: String,
    /// The latest transfer's block time (unix seconds).
    pub transferred_at: i64,
    pub price: String,
    pub rarity: String,
    pub name: String,
    /// The item's category (`hat`, `dance`).
    pub category: String,
    /// `wearable_v1`, `wearable_v2`, `smart_wearable_v1` or `emote_v1`.
    pub item_type: String,
}

/// `owner` -> URN -> the token ids held, newest transfer first.
pub type OwnedTokens = HashMap<String, HashMap<String, Vec<String>>>;

pub struct Caches {
    /// `(owner, category)` -> the owner's copies, newest transfer first.
    owned: TtlMap<(String, String), Arc<Vec<OwnedCopy>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            owned: TtlMap::bounded(OWNED_TTL, OWNED_CACHE_MAX_ENTRIES),
        }
    }
}

/// Which cursor state a read accepts (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Complete,
    Fresh,
}

impl Rule {
    pub fn holds(self, c: &Cursor, now: DateTime<Utc>) -> bool {
        match self {
            Rule::Complete => complete(c),
            Rule::Fresh => fresh(c, now),
        }
    }
}

/// The whole history walked and the last fold published, with no rewind or
/// unpublish since, read from the chain the network names and out of
/// quarantine (as [`chain_walk::is_ready_on`] requires of every index).
pub fn complete(c: &Cursor) -> bool {
    c.indexer == INDEXER_NFT
        && c.full_history
        && c.folded_block.is_some()
        && c.folded_at.is_some()
        && c.quarantined_at.is_none()
        && ChainNet::parse(&c.network)
            .is_some_and(|net| c.chain_id == Some(chain_walk::mainnet_chain(net)))
}

/// [`complete`], published within [`READY_WINDOW`] at `now`.
pub fn fresh(c: &Cursor, now: DateTime<Utc>) -> bool {
    let Some(folded_at) = c.folded_at else {
        return false;
    };
    let age = now.naive_utc().signed_duration_since(folded_at);
    complete(c) && age >= ChronoDuration::zero() && age <= READY_WINDOW
}

/// Whether this node walks the nft index: its switch is on and the node
/// serves mainnet, the only chains (1, 137) the walk reads. Without it a
/// profile is served as deployed and the inventory routes answer the
/// unavailable envelope.
pub fn walks_here(state: &MarketplaceState) -> bool {
    state.settings.market.indexes.nft_enabled && state.settings.eth_network.trim() == "mainnet"
}

/// Why a read did not run.
#[derive(Debug)]
pub enum ReadError {
    /// The index (or the items mirror, the source named) does not meet the
    /// read's rule.
    NotReady(Source, String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ReadError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady(_, m) => f.write_str(m),
            Self::Db(e) => write!(f, "nft index read: {e}"),
        }
    }
}

impl ReadError {
    /// `NotReady` is the unavailable envelope naming its source; a database
    /// error is the caller's `db` answer.
    pub fn into_response(self, db: impl FnOnce(&str) -> Response) -> Response {
        match self {
            Self::NotReady(source, m) => unavailable(source, &m),
            Self::Db(e) => db(&e.to_string()),
        }
    }
}

pub fn not_ready(message: &str) -> Response {
    unavailable(Source::NftIndex, message)
}

/// The networks an inventory of `category` reads: emotes exist on Polygon only.
pub fn category_networks(category: &str) -> &'static [ChainNet] {
    if category == "emote" {
        &[ChainNet::Matic]
    } else {
        &[ChainNet::Ethereum, ChainNet::Matic]
    }
}

/// `Err(NotReady)` unless `nets` meet `rule` at `now` (and, with `items`,
/// the items mirror finished a pass on each), on `conn`.
pub async fn check(
    conn: &mut PgConnection,
    nets: &[ChainNet],
    rule: Rule,
    items: bool,
    now: DateTime<Utc>,
) -> Result<(), ReadError> {
    for &net in nets {
        let cursor = chain_walk::cursor(&mut *conn, INDEXER_NFT, net).await?;
        if !cursor.is_some_and(|c| rule.holds(&c, now)) {
            return Err(ReadError::NotReady(
                Source::NftIndex,
                format!(
                    "the {} nft index is not published (catching up, rewound after a reorg, stopped, or MARKETPLACE_NFT_INDEX_ENABLED off)",
                    net.db_name()
                ),
            ));
        }
        if items {
            let mirrored: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM shop.mirror_state WHERE mirror = 'items' AND network = $1 AND last_complete_pass_at IS NOT NULL)",
            )
            .bind(net.db_name())
            .fetch_one(&mut *conn)
            .await?;
            if !mirrored {
                return Err(ReadError::NotReady(
                    Source::ItemsMirror,
                    format!(
                        "the {} items mirror has not completed a pass: the owned items have no name, rarity or category yet",
                        net.db_name()
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// A read-only snapshot on which `nets` meet `rule`: every read of the
/// index for one answer runs on it.
pub async fn snapshot(
    pool: &PgPool,
    nets: &[ChainNet],
    rule: Rule,
    items: bool,
) -> Result<Transaction<'static, Postgres>, ReadError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    check(&mut tx, nets, rule, items, Utc::now()).await?;
    Ok(tx)
}

/// A passed inventory gate: the snapshot the gate was checked on, which
/// [`owned_copies`] reads the rows from. One request opens one.
pub struct Gate(Transaction<'static, Postgres>);

/// The route gate: `Err(envelope)` unless an inventory of `category` may
/// be read now (the node walks the index, [`Rule::Fresh`] and the items
/// mirror's pass hold). A database error is `internal`'s 500, in the
/// route family's own shape (the lambdas answer lamb2's bare one).
pub async fn require_inventory(
    state: &MarketplaceState,
    category: &str,
    internal: fn() -> Response,
) -> Result<Gate, Response> {
    if !walks_here(state) {
        return Err(not_ready(
            "this node does not walk the nft index (MARKETPLACE_NFT_INDEX_ENABLED off, or a network other than mainnet)",
        ));
    }
    snapshot(&state.pool, category_networks(category), Rule::Fresh, true)
        .await
        .map(Gate)
        .map_err(|e| {
            e.into_response(|m| {
                tracing::error!(error = m, "nft index gate failed");
                internal()
            })
        })
}

const COPIES_SQL: &str = "\
SELECT i.urn, n.token_id::text AS token_id, n.transferred_at, COALESCE(i.price::text, '0') AS price, \
       i.rarity, i.name, i.category, i.item_type \
FROM shop.nft n \
JOIN shop.items i ON i.network = n.network AND i.contract_address = n.contract_address AND i.item_id = n.item_id \
WHERE n.owner = $1 AND n.network = 'matic' AND 'matic' = ANY($3) AND i.item_type = ANY($2) \
UNION ALL \
SELECT i.urn, n.token_id::text, n.transferred_at, COALESCE(i.price::text, '0'), i.rarity, i.name, i.category, i.item_type \
FROM shop.nft n \
JOIN shop.collections c ON c.network = n.network AND c.contract_address = n.contract_address \
JOIN shop.items i ON i.urn = c.urn || ':' || n.wearable_id \
WHERE n.owner = $1 AND n.network = 'ethereum' AND 'ethereum' = ANY($3) AND n.contract_address <> $4 \
  AND n.wearable_id IS NOT NULL AND i.item_type = ANY($2)";

fn item_types(category: &str) -> Vec<String> {
    let types: &[&str] = if category == "emote" {
        &["emote_v1"]
    } else {
        &["wearable_v1", "wearable_v2", "smart_wearable_v1"]
    };
    types.iter().map(|t| t.to_string()).collect()
}

/// Newest transfer first, then token id and price (the order the inventory
/// routes always answered in).
pub fn sort_copies(copies: &mut [OwnedCopy]) {
    copies.sort_by(|a, b| {
        b.transferred_at
            .cmp(&a.transferred_at)
            .then_with(|| a.token_id.cmp(&b.token_id))
            .then_with(|| a.price.cmp(&b.price))
    });
}

async fn read_copies(
    conn: &mut PgConnection,
    owner: &str,
    category: &str,
) -> Result<Vec<OwnedCopy>, sqlx::Error> {
    let nets: Vec<&str> = category_networks(category)
        .iter()
        .map(|n| n.db_name())
        .collect();
    let rows = sqlx::query(COPIES_SQL)
        .bind(owner)
        .bind(item_types(category))
        .bind(&nets)
        .bind(DCL_REGISTRAR)
        .fetch_all(conn)
        .await?;
    let mut copies: Vec<OwnedCopy> = rows
        .iter()
        .map(|r| OwnedCopy {
            urn: r.get("urn"),
            token_id: r.get("token_id"),
            transferred_at: r.get("transferred_at"),
            price: r.get("price"),
            rarity: r.get("rarity"),
            name: r.get("name"),
            category: r.get("category"),
            item_type: r.get("item_type"),
        })
        .collect();
    sort_copies(&mut copies);
    Ok(copies)
}

/// Every copy of `category` (`wearable` | `emote`) `owner` holds, newest
/// transfer first, read on the snapshot `gate` passed
/// ([`require_inventory`] for the same category). The gate is checked on
/// every request; only the rows are memoized ([`OWNED_TTL`]), and a
/// memoized answer lets the snapshot go without another query.
pub async fn owned_copies(
    state: &MarketplaceState,
    gate: Gate,
    owner: &str,
    category: &str,
) -> Result<Arc<Vec<OwnedCopy>>, ReadError> {
    let owner = owner.to_lowercase();
    let key = (owner.clone(), category.to_string());
    if let Some(copies) = state.owned_items.owned.get_fresh(&key) {
        return Ok(copies);
    }
    let mut tx = gate.0;
    let copies = state
        .owned_items
        .owned
        .get_or_fetch(key, || async {
            read_copies(&mut tx, &owner, category)
                .await
                .map(Arc::new)
                .map_err(ReadError::Db)
        })
        .await?;
    tx.rollback().await.ok();
    Ok(copies)
}

/// The item an on-chain URN names, in the spelling the index builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemRef {
    /// `urn:decentraland:<network>:collections-v2:<contract>:<item id>`.
    V2 {
        net: ChainNet,
        contract: String,
        item_id: String,
    },
    /// `urn:decentraland:ethereum:collections-v1:<collection>:<wearable id>`.
    V1 {
        collection_urn: String,
        wearable_id: String,
    },
}

impl ItemRef {
    pub fn net(&self) -> ChainNet {
        match self {
            Self::V2 { net, .. } => *net,
            Self::V1 { .. } => ChainNet::Ethereum,
        }
    }
}

/// The item a URN (without a token id) names, when it is a collection item
/// in the index's spelling: network `matic` or `ethereum`, a lowercase
/// contract, a canonical decimal item id. Any other spelling, a third-party
/// or off-chain URN, is `None` and never owned (the URN a holder is matched
/// on must equal the inventory's exactly).
pub fn parse_item_urn(urn: &str) -> Option<ItemRef> {
    let parts: Vec<&str> = urn.split(':').collect();
    let ["urn", "decentraland", network, kind, collection, item] = parts.as_slice() else {
        return None;
    };
    match (*network, *kind) {
        ("matic" | "ethereum", "collections-v2") => {
            let net = if *network == "matic" {
                ChainNet::Matic
            } else {
                ChainNet::Ethereum
            };
            let contract_ok = collection.len() == 42
                && collection.starts_with("0x")
                && collection[2..]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            let item_ok = !item.is_empty()
                && item.len() <= 78
                && item.bytes().all(|b| b.is_ascii_digit())
                && (*item == "0" || !item.starts_with('0'));
            (contract_ok && item_ok).then(|| ItemRef::V2 {
                net,
                contract: collection.to_string(),
                item_id: item.to_string(),
            })
        }
        ("ethereum", "collections-v1") if !collection.is_empty() && !item.is_empty() => {
            Some(ItemRef::V1 {
                collection_urn: format!("urn:decentraland:ethereum:collections-v1:{collection}"),
                wearable_id: item.to_string(),
            })
        }
        _ => None,
    }
}

const TOKENS_V2_SQL: &str = "\
SELECT r.owner, r.urn, n.token_id::text AS token_id \
FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[]) AS r(owner, urn, network, contract, item) \
JOIN shop.nft n ON n.network = r.network AND n.contract_address = r.contract AND n.item_id = r.item::numeric AND n.owner = r.owner \
ORDER BY r.owner, r.urn, n.transferred_block DESC, n.transferred_log DESC, n.token_id";

const TOKENS_V1_SQL: &str = "\
SELECT r.owner, r.urn, n.token_id::text AS token_id \
FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[]) AS r(owner, urn, collection, wearable) \
JOIN shop.collections c ON c.network = 'ethereum' AND c.urn = r.collection \
JOIN shop.nft n ON n.network = 'ethereum' AND n.contract_address = c.contract_address \
  AND n.wearable_id = r.wearable AND n.owner = r.owner \
ORDER BY r.owner, r.urn, n.transferred_block DESC, n.transferred_log DESC, n.token_id";

/// The parseable `(owner, URN)` pairs of `requested`, by kind, and the
/// networks they read.
struct Requests {
    v2: Vec<(String, String, ItemRef)>,
    v1: Vec<(String, String, ItemRef)>,
    nets: Vec<ChainNet>,
}

fn requests(requested: &HashSet<(String, String)>) -> Requests {
    let mut out = Requests {
        v2: Vec::new(),
        v1: Vec::new(),
        nets: Vec::new(),
    };
    for (owner, urn) in requested {
        let owner = owner.to_lowercase();
        if !is_valid_eth_address(&owner) {
            continue;
        }
        let Some(item) = parse_item_urn(urn) else {
            continue;
        };
        if !out.nets.contains(&item.net()) {
            out.nets.push(item.net());
        }
        match item {
            ItemRef::V2 { .. } => out.v2.push((owner, urn.clone(), item)),
            ItemRef::V1 { .. } => out.v1.push((owner, urn.clone(), item)),
        }
    }
    out.nets.sort();
    out
}

/// The tokens each owner holds of the URNs asked about, newest transfer
/// first, read on `conn` (already checked). A pair whose URN is not a
/// collection item in the index's spelling is not owned.
async fn read_tokens(conn: &mut PgConnection, r: &Requests) -> Result<OwnedTokens, sqlx::Error> {
    let mut owned: OwnedTokens = HashMap::new();
    if !r.v2.is_empty() {
        let mut cols: [Vec<String>; 5] = Default::default();
        for (owner, urn, item) in &r.v2 {
            let ItemRef::V2 {
                net,
                contract,
                item_id,
            } = item
            else {
                continue;
            };
            cols[0].push(owner.clone());
            cols[1].push(urn.clone());
            cols[2].push(net.db_name().to_string());
            cols[3].push(contract.clone());
            cols[4].push(item_id.clone());
        }
        // The item ids are canonical decimals of at most 78 digits
        // (`parse_item_urn`), which the statement casts to numeric.
        let rows = sqlx::query(TOKENS_V2_SQL)
            .bind(&cols[0])
            .bind(&cols[1])
            .bind(&cols[2])
            .bind(&cols[3])
            .bind(&cols[4])
            .fetch_all(&mut *conn)
            .await?;
        collect_tokens(&mut owned, &rows);
    }
    if !r.v1.is_empty() {
        let mut cols: [Vec<String>; 4] = Default::default();
        for (owner, urn, item) in &r.v1 {
            let ItemRef::V1 {
                collection_urn,
                wearable_id,
            } = item
            else {
                continue;
            };
            cols[0].push(owner.clone());
            cols[1].push(urn.clone());
            cols[2].push(collection_urn.clone());
            cols[3].push(wearable_id.clone());
        }
        let rows = sqlx::query(TOKENS_V1_SQL)
            .bind(&cols[0])
            .bind(&cols[1])
            .bind(&cols[2])
            .bind(&cols[3])
            .fetch_all(&mut *conn)
            .await?;
        collect_tokens(&mut owned, &rows);
    }
    Ok(owned)
}

fn collect_tokens(owned: &mut OwnedTokens, rows: &[sqlx::postgres::PgRow]) {
    for row in rows {
        owned
            .entry(row.get("owner"))
            .or_default()
            .entry(row.get("urn"))
            .or_default()
            .push(row.get("token_id"));
    }
}

/// Which of the `(owner, URN)` pairs are held, with their tokens newest
/// transfer first, under `rule` on the networks the URNs name. Nothing
/// parseable asked: an empty answer without a read.
pub async fn owned_tokens(
    pool: &PgPool,
    requested: &HashSet<(String, String)>,
    rule: Rule,
) -> Result<OwnedTokens, ReadError> {
    let r = requests(requested);
    if r.v2.is_empty() && r.v1.is_empty() {
        return Ok(HashMap::new());
    }
    let mut tx = snapshot(pool, &r.nets, rule, false).await?;
    let owned = read_tokens(&mut tx, &r).await?;
    tx.rollback().await.ok();
    Ok(owned)
}

/// Seeds what the gates read: a published nft cursor per network and,
/// with `items`, the items mirror's complete pass.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// `settings` with the nft walk switched on (as a mainnet mirror runs it).
    pub fn walking(settings: super::super::Settings) -> super::super::Settings {
        let market = super::super::MarketSettings {
            indexes: chain_walk::IndexSettings {
                nft_enabled: true,
                ..settings.market.indexes.clone()
            },
            ..settings.market.clone()
        };
        settings.with_market(market)
    }

    /// Publishes the nft index of `nets` `age` ago (a complete walk at
    /// block 100) and, with `items`, records the items mirror's pass.
    pub async fn publish(pool: &PgPool, nets: &[ChainNet], items: bool, age: std::time::Duration) {
        for net in nets {
            sqlx::query(
                "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, fold_through, folded_block, folded_at, full_history, chain_id) \
                 VALUES ('nft', $1, 1, 100, '0x' || repeat('0', 64), 100, 100, timezone('UTC', now()) - make_interval(secs => $2), true, $3) \
                 ON CONFLICT (indexer, network) DO UPDATE SET last_block = 100, fold_through = 100, folded_block = 100, \
                     folded_at = EXCLUDED.folded_at, full_history = true, chain_id = EXCLUDED.chain_id",
            )
            .bind(net.db_name())
            .bind(age.as_secs_f64())
            .bind(chain_walk::mainnet_chain(*net))
            .execute(pool)
            .await
            .unwrap();
            if items {
                sqlx::query(
                    "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
                     VALUES ('items', $1, 'https://items.example', timezone('UTC', now())) \
                     ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at",
                )
                .bind(net.db_name())
                .execute(pool)
                .await
                .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor_at(
        now: DateTime<Utc>,
        full: bool,
        folded: Option<i64>,
        last: i64,
        age_min: Option<i64>,
    ) -> Cursor {
        let now = now.naive_utc();
        Cursor {
            indexer: INDEXER_NFT.to_string(),
            network: "matic".to_string(),
            start_block: 1,
            last_block: last,
            last_block_hash: None,
            last_block_time: None,
            fold_through: folded,
            folded_block: folded,
            folded_at: age_min.map(|m| now - ChronoDuration::minutes(m)),
            full_history: full,
            reorg_rewinds: 0,
            updated_at: now,
            chain_id: Some(137),
            owner_generation: 1,
            quarantined_at: None,
            quarantine_reason: None,
            rebuild_requested_at: None,
            diverged_at: None,
        }
    }

    #[test]
    fn the_rules_read_the_publish_marker_not_the_walk_position() {
        let now = Utc::now();
        let cursor = |full, folded, last, age| cursor_at(now, full, folded, last, age);
        // Published, and the next window already advanced past it.
        let mid_pass = cursor(true, Some(100), 140, Some(0));
        assert!(Rule::Complete.holds(&mid_pass, now));
        assert!(Rule::Fresh.holds(&mid_pass, now));
        // Published an hour ago: complete, not fresh.
        let stale = cursor(true, Some(100), 100, Some(60));
        assert!(Rule::Complete.holds(&stale, now));
        assert!(!Rule::Fresh.holds(&stale, now));
        // Rewound (or never published), or a bounded window.
        assert!(!Rule::Complete.holds(&cursor(true, None, 100, None), now));
        assert!(!Rule::Complete.holds(&cursor(false, Some(100), 100, Some(0)), now));
        // Another indexer's cursor never answers for ownership.
        let mut other = cursor(true, Some(100), 100, Some(0));
        other.indexer = chain_walk::INDEXER_SALES.to_string();
        assert!(!Rule::Complete.holds(&other, now));
        // A publish stamped ahead of the clock is not trusted as fresh.
        let ahead = cursor(true, Some(100), 100, Some(-5));
        assert!(!Rule::Fresh.holds(&ahead, now));
    }

    #[test]
    fn only_collection_urns_in_the_index_spelling_resolve() {
        let c = "0x00000000000000000000000000000000000000c1";
        assert_eq!(
            parse_item_urn(&format!("urn:decentraland:matic:collections-v2:{c}:12")),
            Some(ItemRef::V2 {
                net: ChainNet::Matic,
                contract: c.to_string(),
                item_id: "12".to_string()
            })
        );
        assert_eq!(
            parse_item_urn("urn:decentraland:ethereum:collections-v1:halloween_2019:jester_feet"),
            Some(ItemRef::V1 {
                collection_urn: "urn:decentraland:ethereum:collections-v1:halloween_2019".into(),
                wearable_id: "jester_feet".into()
            })
        );
        for other in [
            // The L1 spelling the index never builds, and Polygon's alias.
            "urn:decentraland:mainnet:collections-v1:halloween_2019:jester_feet".to_string(),
            format!("urn:decentraland:polygon:collections-v2:{c}:12"),
            // A checksummed contract, a padded or non-decimal item id.
            "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000C1:12"
                .to_string(),
            format!("urn:decentraland:matic:collections-v2:{c}:012"),
            format!("urn:decentraland:matic:collections-v2:{c}:0x1"),
            // With its token id, third party, off-chain, v1 off Ethereum.
            format!("urn:decentraland:matic:collections-v2:{c}:12:5"),
            "urn:decentraland:matic:collections-thirdparty:p:c:1".to_string(),
            "urn:decentraland:off-chain:base-avatars:hat".to_string(),
            "urn:decentraland:matic:collections-v1:x:y".to_string(),
        ] {
            assert_eq!(parse_item_urn(&other), None, "{other}");
        }
        assert_eq!(
            parse_item_urn(&format!("urn:decentraland:matic:collections-v2:{c}:0"))
                .map(|i| i.net()),
            Some(ChainNet::Matic)
        );
    }

    #[test]
    fn the_requests_skip_invalid_owners_and_name_their_networks() {
        let c = "0x00000000000000000000000000000000000000c1";
        let owner = "0xAAAA000000000000000000000000000000000001";
        let requested: HashSet<(String, String)> = [
            (
                owner.to_string(),
                format!("urn:decentraland:matic:collections-v2:{c}:1"),
            ),
            (
                "not-an-address".to_string(),
                "urn:decentraland:ethereum:collections-v1:x:y".to_string(),
            ),
            (
                owner.to_string(),
                "urn:decentraland:off-chain:base-avatars:hat".to_string(),
            ),
        ]
        .into_iter()
        .collect();
        let r = requests(&requested);
        assert_eq!((r.v2.len(), r.v1.len()), (1, 0));
        assert_eq!(r.v2[0].0, owner.to_lowercase(), "owners are lowercased");
        assert_eq!(r.nets, vec![ChainNet::Matic]);
    }

    #[test]
    fn copies_sort_newest_transfer_first_then_token_then_price() {
        let copy = |token: &str, at: i64, price: &str| OwnedCopy {
            urn: "u".into(),
            token_id: token.into(),
            transferred_at: at,
            price: price.into(),
            rarity: String::new(),
            name: String::new(),
            category: String::new(),
            item_type: String::new(),
        };
        let mut copies = vec![
            copy("2", 100, "1"),
            copy("1", 100, "1"),
            copy("9", 300, "1"),
            copy("1", 100, "0"),
        ];
        sort_copies(&mut copies);
        let order: Vec<(&str, i64, &str)> = copies
            .iter()
            .map(|c| (c.token_id.as_str(), c.transferred_at, c.price.as_str()))
            .collect();
        assert_eq!(
            order,
            vec![
                ("9", 300, "1"),
                ("1", 100, "0"),
                ("1", 100, "1"),
                ("2", 100, "1")
            ]
        );
    }
}
