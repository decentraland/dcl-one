//! The `sales` chain index on Polygon: the primary mints the CollectionStore
//! issues (a mirrored collection's `Issue` whose `_caller` is the store,
//! priced by the store's `Bought` in the same transaction) and
//! the ERC721BidV2 bid lifecycle (`BidCreated`, `BidAccepted`,
//! `BidCancelled`), kept in `shop.sale_event` and folded into `shop.nft_bid`
//! (one row per bidder and token, as the contract keeps them: a bidder's new
//! bid on the same token replaces the old one without a cancellation; the
//! seller is the token's owner when the bid was placed, read from the nft
//! walk's transfers, or the acceptance's seller). `shop.sale_venue` pins the
//! venues and the royalties cut each charges; `check_venues` reads
//! `royaltiesCutPerMillion()` once at the job's start and warns on a
//! difference.

//! Sepolia and Amoy publish raw bid and mint evidence. The economic
//! publication for priced sales and settlements has its own coverage rule.

use std::collections::HashMap;

use alloy_primitives::U256;
use alloy_sol_types::{SolType, sol_data};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{self, ChainIndex, ChainNet, INDEXER_SALES, LogGroup};
use super::chain_walk::{topic_address, topic_u256};
use super::nft_index::{self, DecodeError};
use crate::chain_index::rpc::{Rpc, RpcLog};

/// `Issue(address indexed _beneficiary, uint256 indexed _tokenId, uint256 indexed _itemId, uint256 _issuedId, address _caller)` (v2 collections).
pub const TOPIC_ISSUE: &str = "0x57e2fe3f4d57a597e655a893f43c395d7dc18cb9c9c1898eda75cc0ad7608525";
/// `BidCreated(bytes32 _id, address indexed _tokenAddress, uint256 indexed _tokenId, address indexed _bidder, uint256 _price, uint256 _expiresAt, bytes _fingerprint)`.
pub const TOPIC_BID_CREATED: &str =
    "0xe45b7709f1eed66d6e28f43b32f2003da0f0c40b92f75a8994370516e048620f";
/// `BidAccepted(bytes32 _id, address indexed _tokenAddress, uint256 indexed _tokenId, address _bidder, address indexed _seller, uint256 _price, uint256 _fee)`.
pub const TOPIC_BID_ACCEPTED: &str =
    "0x4e5ca6c6c06fa8d62f2930830e0d370de70f108bd89213de0b51141775e695bd";
/// `BidCancelled(bytes32 _id, address indexed _tokenAddress, uint256 indexed _tokenId, address indexed _bidder)`.
pub const TOPIC_BID_CANCELLED: &str =
    "0xc43098075c34b8b92567bd49f08f55e397ebf82dd82072e3eb1be525c4506f5f";

/// `Bought((address collection, uint256[] ids, uint256[] prices, address[] beneficiaries)[] _itemsToBuy)` (CollectionStore).
pub const TOPIC_BOUGHT: &str = "0x77cc75f8061aa168906862622e88c5b05a026a9c06c02d91ec98543e01e7ad33";

pub const KIND_MINT: &str = "mint";
pub const KIND_BID_CREATED: &str = "bid_created";
pub const KIND_BID_ACCEPTED: &str = "bid_accepted";
pub const KIND_BID_CANCELLED: &str = "bid_cancelled";

/// The CollectionStore on Polygon: an Issue it called is a primary sale.
pub const STORE_MATIC: &str = "0x214ffc0f0103735728dc66b61a22e4f163e275ae";
/// ERC721BidV2 on Polygon and the block the pinned subgraph starts it from.
pub const BID_V2_MATIC: &str = "0xb96697fa4a3361ba35b774a42c58daccaad1b8e1";
pub const BID_V2_MATIC_START_BLOCK: u64 = 22_514_900;
/// Testnet deployments and start blocks from decentraland/collections-graph
/// ab6c58670d94bfbe774a0f56dadef1b541c53e8b, config/amoy.json, and
/// decentraland/marketplace-squid-core 85c6832e96e5459b37f0f315b8e384693684e12d,
/// src/polygon/addresses/amoy.ts (CollectionStore).
#[derive(Clone, Copy)]
pub struct SalesIndex {
    chain: i64,
    polygon: Option<CollectionVenues>,
    bid: &'static str,
    bid_start: u64,
}
#[derive(Clone, Copy)]
struct CollectionVenues {
    store: &'static str,
    market_v2: &'static str,
}

pub const POLYGON: SalesIndex = SalesIndex {
    chain: 137,
    polygon: Some(CollectionVenues {
        store: STORE_MATIC,
        market_v2: "0x480a0f4e360e8964e68858dd231c2922f1df45ef",
    }),
    bid: BID_V2_MATIC,
    bid_start: BID_V2_MATIC_START_BLOCK,
};

pub const AMOY: SalesIndex = SalesIndex {
    chain: 80_002,
    polygon: Some(CollectionVenues {
        store: "0xe36abc9ec616c83caaa386541380829106149d68",
        market_v2: "0x0c8ad1f6aadf89d2eb19f01a100a6143108fe2b0",
    }),
    bid: "0x4b66eab79cc03a96fb6275cfcdf23c0db431606d",
    bid_start: 5_706_662,
};

/// marketplace-squid-core@85c6832e96e5459b37f0f315b8e384693684e12d,
/// src/eth/addresses/sepolia.ts and abi/ERC721Bid.json. Its three bid
/// events have the same signatures and indexed fields as the Polygon ABI.
/// Creation receipt 0x44be8623150396ab165f8d1d5bd80e073c32deb0a3b485bcc75f7e8bca360a1f.
pub const SEPOLIA: SalesIndex = SalesIndex {
    chain: 11_155_111,
    polygon: None,
    bid: "0x2c2835b95852fd975e087b3b25297322728792e2",
    bid_start: 3_831_237,
};

impl SalesIndex {
    /// The ERC721BidV2 contract this deployment walks: a legacy bid's
    /// `bidAddress`.
    pub fn bid_contract(self) -> &'static str {
        self.bid
    }

    pub fn for_chain(chain: i64) -> Option<Self> {
        match chain {
            137 => Some(POLYGON),
            80_002 => Some(AMOY),
            11_155_111 => Some(SEPOLIA),
            _ => None,
        }
    }
}

/// The selector of `royaltiesCutPerMillion()`.
pub const ROYALTIES_CUT_SELECTOR: &str = "0xce2243b4";

/// One row of `shop.sale_event`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaleEvent {
    pub block_number: i64,
    pub log_index: i32,
    pub block_time: i64,
    pub kind: &'static str,
    pub emitter: String,
    pub contract_address: String,
    pub token_id: String,
    pub item_id: Option<String>,
    pub bid_id: Option<String>,
    /// The mint's beneficiary or the bid's bidder.
    pub buyer: String,
    pub seller: Option<String>,
    pub price: Option<String>,
    /// The fee emitted by BidAccepted, in payment-token base units.
    pub bid_fee: Option<String>,
    /// The bid's expiry, unix seconds.
    pub expires_at: Option<i64>,
    pub tx_hash: String,
    /// A creation's `_fingerprint`, 0x-prefixed hex (`0x` when empty).
    pub fingerprint: Option<String>,
}

fn seconds(v: U256) -> i64 {
    let v: u128 = v.try_into().unwrap_or(u128::MAX);
    v.min(i64::MAX as u128) as i64
}

/// One log as a row; `Ok(None)` for a topic this index does not model and
/// for an Issue the store did not call (a creator's own mint).
pub fn decode_log(log: &RpcLog, block_time: i64) -> Result<Option<SaleEvent>, DecodeError> {
    decode_sale(POLYGON, log, block_time)
}

fn decode_sale(
    index: SalesIndex,
    log: &RpcLog,
    block_time: i64,
) -> Result<Option<SaleEvent>, DecodeError> {
    let store = index.polygon.map(|venues| venues.store);
    let (block, index) = chain_walk::log_position(log);
    let index = index as i32;
    let err = |what: &'static str, why: String| DecodeError {
        block,
        index,
        what,
        why,
    };
    let Some(topic0) = log.topics.first().map(|t| t.to_ascii_lowercase()) else {
        return Ok(None);
    };
    let kind = match topic0.as_str() {
        TOPIC_ISSUE => KIND_MINT,
        TOPIC_BID_CREATED => KIND_BID_CREATED,
        TOPIC_BID_ACCEPTED => KIND_BID_ACCEPTED,
        TOPIC_BID_CANCELLED => KIND_BID_CANCELLED,
        _ => return Ok(None),
    };
    let data =
        hex::decode(log.data.trim_start_matches("0x")).map_err(|e| err("data", e.to_string()))?;
    let topic = |i: usize| {
        log.topics
            .get(i)
            .ok_or_else(|| err(kind, format!("needs topic {i}")))
    };
    let addr = |i: usize| {
        topic_address(topic(i)?).ok_or_else(|| err(kind, format!("bad address in topic {i}")))
    };
    let word =
        |i: usize| topic_u256(topic(i)?).ok_or_else(|| err(kind, format!("bad uint in topic {i}")));
    let tx_hash = log
        .transaction_hash
        .as_deref()
        .map(str::to_ascii_lowercase)
        .filter(|h| {
            h.len() == 66 && h.starts_with("0x") && h[2..].bytes().all(|c| c.is_ascii_hexdigit())
        })
        .ok_or_else(|| err(kind, "missing transaction hash".into()))?;
    let base = |contract_address: String, token_id: String, buyer: String| SaleEvent {
        block_number: block,
        log_index: index,
        block_time,
        kind,
        emitter: log.address.to_ascii_lowercase(),
        contract_address,
        token_id,
        item_id: None,
        bid_id: None,
        buyer,
        seller: None,
        price: None,
        bid_fee: None,
        expires_at: None,
        tx_hash: tx_hash.clone(),
        fingerprint: None,
    };
    let event = match kind {
        KIND_MINT => {
            let (_issued, caller) =
                <(sol_data::Uint<256>, sol_data::Address)>::abi_decode_params(&data)
                    .map_err(|e| err(kind, e.to_string()))?;
            if Some(format!("{caller:#x}").as_str()) != store {
                return Ok(None);
            }
            SaleEvent {
                item_id: Some(word(3)?),
                ..base(log.address.to_ascii_lowercase(), word(2)?, addr(1)?)
            }
        }
        KIND_BID_CREATED => {
            let (id, price, expires, fingerprint) = <(
                sol_data::FixedBytes<32>,
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Bytes,
            )>::abi_decode_params(&data)
            .map_err(|e| err(kind, e.to_string()))?;
            SaleEvent {
                bid_id: Some(format!("{id:#x}")),
                price: Some(price.to_string()),
                expires_at: Some(seconds(expires)),
                fingerprint: Some(format!("{fingerprint}")),
                ..base(addr(1)?, word(2)?, addr(3)?)
            }
        }
        KIND_BID_ACCEPTED => {
            let (id, bidder, price, fee) = <(
                sol_data::FixedBytes<32>,
                sol_data::Address,
                sol_data::Uint<256>,
                sol_data::Uint<256>,
            )>::abi_decode_params(&data)
            .map_err(|e| err(kind, e.to_string()))?;
            SaleEvent {
                bid_id: Some(format!("{id:#x}")),
                seller: Some(addr(3)?),
                bid_fee: Some(fee.to_string()),
                price: Some(price.to_string()),
                ..base(addr(1)?, word(2)?, format!("{bidder:#x}"))
            }
        }
        _ => {
            let id = <sol_data::FixedBytes<32>>::abi_decode(&data)
                .map_err(|e| err(kind, e.to_string()))?;
            SaleEvent {
                bid_id: Some(format!("{id:#x}")),
                ..base(addr(1)?, word(2)?, addr(3)?)
            }
        }
    };
    Ok(Some(event))
}

/// One item a store `Bought` paid for: (collection, item id, beneficiary, price).
pub type BoughtItem = (String, String, String, String);

/// A store `Bought` log's transaction and items; `Ok(None)` for another log.
pub fn decode_bought(log: &RpcLog) -> Result<Option<(String, Vec<BoughtItem>)>, DecodeError> {
    bought_at(STORE_MATIC, log)
}

pub(super) fn bought_at(
    store: &str,
    log: &RpcLog,
) -> Result<Option<(String, Vec<BoughtItem>)>, DecodeError> {
    let (block, index) = chain_walk::log_position(log);
    let err = |why: String| DecodeError {
        block,
        index: index as i32,
        what: "Bought",
        why,
    };
    if !log.address.eq_ignore_ascii_case(store)
        || log
            .topics
            .first()
            .map(|t| t.to_ascii_lowercase())
            .as_deref()
            != Some(TOPIC_BOUGHT)
    {
        return Ok(None);
    }
    let tx = log
        .transaction_hash
        .as_deref()
        .map(str::to_ascii_lowercase)
        .filter(|h| {
            h.strip_prefix("0x")
                .is_some_and(|word| word.len() == 64 && word.bytes().all(|c| c.is_ascii_hexdigit()))
        })
        .ok_or_else(|| err("missing or invalid transaction hash".into()))?;
    let data = hex::decode(log.data.trim_start_matches("0x")).map_err(|e| err(e.to_string()))?;
    type Items = sol_data::Array<(
        sol_data::Address,
        sol_data::Array<sol_data::Uint<256>>,
        sol_data::Array<sol_data::Uint<256>>,
        sol_data::Array<sol_data::Address>,
    )>;
    let (items,) = <(Items,)>::abi_decode_params(&data).map_err(|e| err(e.to_string()))?;
    let mut out = Vec::new();
    for (collection, ids, prices, beneficiaries) in items {
        if ids.len() != prices.len() || ids.len() != beneficiaries.len() {
            return Err(err("ids, prices and beneficiaries differ in length".into()));
        }
        for ((id, price), beneficiary) in ids.iter().zip(&prices).zip(&beneficiaries) {
            out.push((
                format!("{collection:#x}"),
                id.to_string(),
                format!("{beneficiary:#x}"),
                price.to_string(),
            ));
        }
    }
    Ok(Some((tx, out)))
}

pub async fn insert_events(
    conn: &mut PgConnection,
    net: ChainNet,
    rows: &[SaleEvent],
) -> Result<usize, sqlx::Error> {
    let mut written = 0;
    for chunk in rows.chunks(1_000) {
        let r = sqlx::query(
            "INSERT INTO shop.sale_event \
                 (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, item_id, bid_id, buyer, seller, price, expires_at, tx_hash, bid_fee, fingerprint) \
             SELECT $1, b, i, t, k, em, c, tok::numeric, it::numeric, bid, by, se, p::numeric, x, h, fee::numeric, fp \
             FROM UNNEST($2::bigint[], $3::int[], $4::bigint[], $5::text[], $6::text[], $7::text[], $8::text[], $9::text[], $10::text[], $11::text[], $12::text[], $13::text[], $14::bigint[], $15::text[], $16::text[], $17::text[]) \
                  AS u(b, i, t, k, em, c, tok, it, bid, by, se, p, x, h, fee, fp) \
             ON CONFLICT (network, block_number, log_index) DO UPDATE \
             SET bid_fee = COALESCE(sale_event.bid_fee, EXCLUDED.bid_fee), price = COALESCE(sale_event.price, EXCLUDED.price), \
                 fingerprint = COALESCE(sale_event.fingerprint, EXCLUDED.fingerprint) \
             WHERE sale_event.tx_hash = EXCLUDED.tx_hash AND sale_event.kind = EXCLUDED.kind \
               AND ((sale_event.bid_fee IS NULL AND EXCLUDED.bid_fee IS NOT NULL) \
                 OR (sale_event.price IS NULL AND EXCLUDED.price IS NOT NULL) \
                 OR (sale_event.fingerprint IS NULL AND EXCLUDED.fingerprint IS NOT NULL))",
        )
        .bind(net.db_name())
        .bind(chunk.iter().map(|r| r.block_number).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.log_index).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.block_time).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.kind.to_string()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.emitter.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.contract_address.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.token_id.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.item_id.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.bid_id.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.buyer.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.seller.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.price.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.expires_at).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.tx_hash.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.bid_fee.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.fingerprint.clone()).collect::<Vec<_>>())
        .execute(&mut *conn)
        .await?;
        written += r.rows_affected() as usize;
    }
    Ok(written)
}

/// The bids fold of the touched keys: every (token, bidder) a bid event
/// past `$2` touched, the open bids on a token with an acceptance after
/// them (whatever fold saw the acceptance, so a database folded before the
/// rule existed converges without a cursor reset), plus the rows still
/// missing their seller that the nft walk (`nft`: its last indexed block,
/// read in the same statement as its transfers, and only while its history
/// is bound to Polygon, out of quarantine and not diverged) has passed
/// since, plus every bid created past `$5`, the lowest point the nft walk
/// rewound to since the last fold (`shop.bid_seller_recheck`): its inferred
/// seller may name an owner the rewind took away. The
/// latest `bid_created` of a key defines the bid; the latest event at or
/// after it, its status, `invalidated` when another bid on the token was
/// accepted after it (the contract drops the token's other bids without an
/// event); the seller is the acceptance's or the owner before the bid as the
/// nft walk's transfers tell it, which a bid past the walk's last block or on
/// a contract still owed a backfill does not read (the walk has not seen every
/// transfer before the bid: the row waits with no seller and is revisited once
/// the walk passes it and the backfill is done). An
/// unchanged row is left alone. The latest event is one backward probe of
/// `sale_event_bidder_idx` per bid (`LATERAL … LIMIT 1`), not a join of two
/// key sets: the planner estimates `created` at one row, and as a join it
/// rescanned every key per key, which a full fold of Polygon's history
/// (about 66k bids) did not finish within the hour.
const FOLD_BIDS_SQL: &str = "
WITH nft AS (
    SELECT COALESCE(max(last_block), -1) AS indexed FROM shop.chain_cursor
    WHERE indexer = $4 AND network = $1 AND chain_id = $3 AND quarantined_at IS NULL AND diverged_at IS NULL
), touched AS (
    SELECT contract_address, token_id, buyer AS bidder FROM shop.sale_event
    WHERE network = $1 AND block_number > $2 AND kind <> 'mint'
    UNION
    SELECT b.contract_address, b.token_id, b.bidder FROM shop.nft_bid b
    WHERE b.network = $1 AND b.status = 'open' AND EXISTS (
        SELECT 1 FROM shop.sale_event a
        WHERE a.network = $1 AND a.contract_address = b.contract_address AND a.token_id = b.token_id
          AND a.kind = 'bid_accepted' AND (a.block_number, a.log_index) > (b.created_block, b.created_log_index))
    UNION
    SELECT b.contract_address, b.token_id, b.bidder FROM shop.nft_bid b
    WHERE b.network = $1 AND $5::bigint IS NOT NULL AND b.created_block > $5
    UNION
    SELECT b.contract_address, b.token_id, b.bidder FROM shop.nft_bid b
    WHERE b.network = $1 AND b.seller IS NULL AND b.created_block <= (SELECT indexed FROM nft) AND EXISTS (
        SELECT 1 FROM shop.nft_event n
        WHERE n.network = $1 AND n.contract_address = b.contract_address AND n.token_id = b.token_id
          AND n.kind = 'transfer' AND (n.block_number, n.log_index) < (b.created_block, b.created_log_index))
), created AS (
    SELECT DISTINCT ON (e.contract_address, e.token_id, e.buyer)
        e.contract_address, e.token_id, e.buyer AS bidder, e.bid_id, e.price, e.expires_at,
        e.block_number, e.log_index, e.block_time, e.fingerprint
    FROM touched t
    JOIN shop.sale_event e ON e.network = $1 AND e.contract_address = t.contract_address
        AND e.token_id = t.token_id AND e.buyer = t.bidder AND e.kind = 'bid_created'
    ORDER BY e.contract_address, e.token_id, e.buyer, e.block_number DESC, e.log_index DESC
)
INSERT INTO shop.nft_bid (network, contract_address, token_id, bidder, bid_id, seller, price, expires_at, status,
    created_block, created_log_index, created_at, updated_block, updated_log_index, updated_at, fingerprint)
SELECT $1, c.contract_address, c.token_id, c.bidder, c.bid_id,
    COALESCE(l.seller, (
        SELECT n.to_address FROM shop.nft_event n
        WHERE n.network = $1 AND n.contract_address = c.contract_address AND n.token_id = c.token_id
          AND n.kind = 'transfer' AND (n.block_number, n.log_index) < (c.block_number, c.log_index)
          AND c.block_number <= (SELECT indexed FROM nft)
          AND NOT EXISTS (
            SELECT 1 FROM shop.nft_contract k
            WHERE k.network = $1 AND k.contract_address = c.contract_address AND k.backfill_from_block IS NOT NULL
              AND COALESCE(k.backfilled_to_block, k.backfill_from_block - 1) < k.walked_from_block - 1)
        ORDER BY n.block_number DESC, n.log_index DESC LIMIT 1)),
    c.price, c.expires_at,
    CASE WHEN l.kind = 'bid_accepted' THEN 'accepted' WHEN l.kind = 'bid_cancelled' THEN 'cancelled'
         WHEN EXISTS (
            SELECT 1 FROM shop.sale_event a
            WHERE a.network = $1 AND a.contract_address = c.contract_address AND a.token_id = c.token_id
              AND a.kind = 'bid_accepted' AND (a.block_number, a.log_index) > (c.block_number, c.log_index)) THEN 'invalidated'
         ELSE 'open' END,
    c.block_number, c.log_index, c.block_time, l.block_number, l.log_index, l.block_time, c.fingerprint
FROM created c
CROSS JOIN LATERAL (
    SELECT e.kind, e.seller, e.block_number, e.log_index, e.block_time
    FROM shop.sale_event e
    WHERE e.network = $1 AND e.contract_address = c.contract_address
      AND e.token_id = c.token_id AND e.buyer = c.bidder AND e.kind <> 'mint'
      AND (e.block_number, e.log_index) >= (c.block_number, c.log_index)
    ORDER BY e.block_number DESC, e.log_index DESC
    LIMIT 1
) l
ON CONFLICT (network, contract_address, token_id, bidder) DO UPDATE SET
    bid_id = EXCLUDED.bid_id, seller = EXCLUDED.seller, price = EXCLUDED.price, expires_at = EXCLUDED.expires_at,
    status = EXCLUDED.status, created_block = EXCLUDED.created_block, created_log_index = EXCLUDED.created_log_index,
    created_at = EXCLUDED.created_at, updated_block = EXCLUDED.updated_block,
    updated_log_index = EXCLUDED.updated_log_index, updated_at = EXCLUDED.updated_at, fingerprint = EXCLUDED.fingerprint
WHERE (nft_bid.bid_id, nft_bid.seller, nft_bid.status, nft_bid.updated_block, nft_bid.updated_log_index, nft_bid.fingerprint)
    IS DISTINCT FROM (EXCLUDED.bid_id, EXCLUDED.seller, EXCLUDED.status, EXCLUDED.updated_block, EXCLUDED.updated_log_index, EXCLUDED.fingerprint)";

/// The fold in one transaction: the keys a bid event past `since` touched
/// (`None`: every key), each over its whole history; the rows written.
pub async fn fold(
    pool: &PgPool,
    since: Option<i64>,
    owner: &chain_walk::Owner,
) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    let bids = fold_in(&mut tx, since, owner).await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(bids)
}

/// [`fold`]'s statements in the caller's transaction, which ends with
/// `owner.check`: the Ethereum bid history repair refolds every key in the
/// transaction that completes it.
pub async fn fold_in(
    tx: &mut PgConnection,
    since: Option<i64>,
    owner: &chain_walk::Owner,
) -> anyhow::Result<u64> {
    let since = since.unwrap_or(-1);
    // The nft walk's rewinds since the last fold, read before the fold's own
    // statement (whose newer snapshot sees each one's rows); only these are
    // deleted after it, so one recorded meanwhile waits for the next fold.
    let (rechecks, recheck_from): (Option<Vec<i64>>, Option<i64>) = sqlx::query_as(
        "SELECT array_agg(id), min(from_block) FROM shop.bid_seller_recheck WHERE network = $1",
    )
    .bind(owner.net.db_name())
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("SET LOCAL statement_timeout = '3600s'")
        .execute(&mut *tx)
        .await?;
    chain_walk::analyze_if_stale(&mut *tx, "shop", "sale_event", since < 0).await?;
    let bids = sqlx::query(FOLD_BIDS_SQL)
        .bind(owner.net.db_name())
        .bind(since)
        .bind(owner.chain_id)
        .bind(chain_walk::INDEXER_NFT)
        .bind(recheck_from)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if let Some(ids) = rechecks {
        sqlx::query("DELETE FROM shop.bid_seller_recheck WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
    }
    Ok(bids)
}

/// Polygon's side of the walk: the mirrored collections' Issue logs (the
/// address set is time-bounded like the nft walk's; a collection the mirror
/// reports late has no store sale before its approval, so no backfill lane)
/// and the bid contract's three events from its deployment.
#[async_trait::async_trait]
impl ChainIndex for SalesIndex {
    fn indexer(&self) -> &'static str {
        INDEXER_SALES
    }

    fn chain_id(&self) -> i64 {
        self.chain
    }

    fn net(&self) -> ChainNet {
        if self.chain == SEPOLIA.chain {
            ChainNet::Ethereum
        } else {
            ChainNet::Matic
        }
    }

    fn start_block(&self) -> u64 {
        match self.chain {
            11_155_111 => self.bid_start,
            80_002 => 5_706_656,
            _ => nft_index::deployments::Deployments::mainnet(ChainNet::Matic).start,
        }
    }

    async fn groups(
        &self,
        pool: &PgPool,
        _from: u64,
        to: u64,
        anchor: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        let mut groups = Vec::new();
        if let Some(venues) = self.polygon {
            let bound = chain_walk::time_at_most(ChainNet::Matic, anchor, to);
            let collections: Vec<String> = sqlx::query_scalar(
            "SELECT contract_address FROM shop.collections WHERE network = $1 AND created_at <= $2 AND chain_id = $3 AND ($4 OR testnet_verified) ORDER BY contract_address",
        )
        .bind(ChainNet::Matic.db_name())
        .bind(bound)
        .bind(self.chain as i32)
        .bind(self.chain == ChainNet::Matic.chain_id())
        .fetch_all(pool)
        .await?;
            if !collections.is_empty() {
                groups.push(LogGroup {
                    addresses: collections,
                    topics: json!([[TOPIC_ISSUE]]),
                });
            }
            // The store's Bought prices the mints of its transaction.
            groups.push(LogGroup {
                addresses: vec![venues.store.to_string()],
                topics: json!([[TOPIC_BOUGHT]]),
            });
        }
        if to >= self.bid_start {
            groups.push(LogGroup {
                addresses: vec![self.bid.to_string()],
                topics: json!([[TOPIC_BID_CREATED, TOPIC_BID_ACCEPTED, TOPIC_BID_CANCELLED]]),
            });
        }
        if self.chain == POLYGON.chain {
            groups.push(super::sale_terms::group(self.net()));
        }
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
            match decode_sale(*self, log, times.get(&block).copied().unwrap_or(0)) {
                Ok(Some(row)) => rows.push(row),
                Ok(None) => {}
                // Keep valid rows in this window. The walker withholds sales
                // coverage when an accounting event cannot be decoded.
                Err(e) => {
                    tracing::warn!(error = %e, "sales index: skipping a log it cannot decode")
                }
            }
        }
        if let Some(venues) = self.polygon {
            let lines = super::store_sales::pair(venues.store, &mut rows, logs)?;
            if self.chain == POLYGON.chain {
                super::sale_terms::write(conn, self.net(), logs).await?;
                super::store_sales::write(conn, &lines, times).await?;
            }
        }
        Ok(insert_events(conn, self.net(), &rows).await?)
    }

    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        if self.chain == POLYGON.chain {
            super::sale_terms::rewind(conn, self.net(), to).await?;
            sqlx::query("DELETE FROM shop.store_sale_line WHERE block_number > $1")
                .bind(to)
                .execute(&mut *conn)
                .await?;
        }
        // The deleted rows' bids are dropped (a mint keys no bid, so none
        // comes back); the cleared fold marker makes the next fold a rebuild,
        // which restores the ones with a remaining creation from the log that
        // is left.
        let gone: Vec<(String, String, String)> = sqlx::query_as(
            "WITH gone AS (DELETE FROM shop.sale_event WHERE network = $1 AND block_number > $2 RETURNING contract_address, token_id, buyer, kind) \
             SELECT DISTINCT contract_address, token_id::text, buyer FROM gone WHERE kind <> 'mint'",
        )
        .bind(self.net().db_name())
        .bind(to)
        .fetch_all(&mut *conn)
        .await?;
        let (contracts, tokens, bidders): (Vec<String>, Vec<String>, Vec<String>) = gone
            .into_iter()
            .fold((vec![], vec![], vec![]), |mut acc, (c, t, b)| {
                acc.0.push(c);
                acc.1.push(t);
                acc.2.push(b);
                acc
            });
        sqlx::query(
            "DELETE FROM shop.nft_bid b USING UNNEST($2::text[], $3::text[], $4::text[]) AS g(c, t, bd) \
             WHERE b.network = $1 AND b.contract_address = g.c AND b.token_id = g.t::numeric AND b.bidder = g.bd",
        )
        .bind(self.net().db_name())
        .bind(&contracts)
        .bind(&tokens)
        .bind(&bidders)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn fold(
        &self,
        pool: &PgPool,
        since: Option<i64>,
        _upto: i64,
        owner: &chain_walk::Owner,
    ) -> anyhow::Result<bool> {
        let bids = fold(pool, since, owner).await?;
        tracing::info!(bids, "sales index: folded");
        Ok(true)
    }
}

/// One venue's pinned royalties cut beside the chain's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenueCheck {
    pub address: String,
    pub kind: String,
    pub pinned: i64,
    pub live: Option<U256>,
}

impl VenueCheck {
    pub fn matches(&self) -> bool {
        self.live == Some(U256::from(self.pinned))
    }
}

/// `royaltiesCutPerMillion()` of every Polygon venue that charges one
/// (`marketplace_v2`, `bid_v2`), compared with `shop.sale_venue`: a
/// difference or an unreadable answer is a warning, never a rewrite.
pub async fn check_venues(
    rpc: &Rpc<'_>,
    pool: &PgPool,
    index: SalesIndex,
) -> anyhow::Result<Vec<VenueCheck>> {
    let Some(venues) = index.polygon else {
        return Ok(Vec::new());
    };
    if index.chain == AMOY.chain {
        // Existing databases already have sale_venue; the schema object pass
        // only seeds a newly created table. Preserve any existing pinned cut.
        sqlx::query(
            "INSERT INTO shop.sale_venue (network, address, kind, royalties_cut_per_million) \
             VALUES ('matic', $1, 'marketplace_v2', 25000), ('matic', $2, 'bid_v2', 25000), \
                    ('matic', $3, 'store', 0) ON CONFLICT (network, address) DO NOTHING",
        )
        .bind(venues.market_v2)
        .bind(index.bid)
        .bind(venues.store)
        .execute(pool)
        .await?;
    }
    let venues: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT address, kind, royalties_cut_per_million FROM shop.sale_venue \
         WHERE network = $1 AND address IN ($2, $3) AND kind IN ('marketplace_v2', 'bid_v2') ORDER BY address",
    )
    .bind(ChainNet::Matic.db_name())
    .bind(venues.market_v2)
    .bind(index.bid)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(venues.len());
    for (address, kind, pinned) in venues {
        let reply = rpc
            .call(
                "eth_call",
                json!([{ "to": address, "data": ROYALTIES_CUT_SELECTOR }, "latest"]),
            )
            .await?;
        let live = reply
            .get("result")
            .and_then(Value::as_str)
            .and_then(|s| U256::from_str_radix(s.trim_start_matches("0x"), 16).ok());
        let check = VenueCheck {
            address,
            kind,
            pinned: i64::from(pinned),
            live,
        };
        match check.live {
            Some(live) if check.matches() => tracing::info!(
                venue = %check.address, kind = %check.kind, cut = %live,
                "sales index: the venue's royalties cut matches the pinned value"
            ),
            Some(live) => tracing::warn!(
                venue = %check.address, kind = %check.kind, pinned = check.pinned, live = %live,
                "sales index: the venue's royalties cut differs from shop.sale_venue"
            ),
            None => tracing::warn!(
                venue = %check.address, kind = %check.kind, reply = %reply,
                "sales index: royaltiesCutPerMillion() is unreadable"
            ),
        }
        out.push(check);
    }
    Ok(out)
}

/// Whether the bids may be trusted at `now` (the cursor is published).
pub async fn ready(
    pool: &PgPool,
    indexes: &chain_walk::IndexSettings,
    net: ChainNet,
    now: DateTime<Utc>,
) -> bool {
    chain_walk::cursor(pool, INDEXER_SALES, net)
        .await
        .ok()
        .flatten()
        .is_some_and(|c| chain_walk::fresh_fold_on(&c, indexes.chain_id(net), now).is_some())
}

/// The index's state for the status route and the tests.
pub async fn status(pool: &PgPool, indexes: &chain_walk::IndexSettings) -> Value {
    let mut out = serde_json::Map::new();
    let now = Utc::now();
    for net in ChainNet::ALL {
        let cursor = chain_walk::cursor(pool, INDEXER_SALES, net)
            .await
            .ok()
            .flatten();
        out.insert(net.db_name().into(),json!(cursor.map(|c| json!({
            "lastBlock": c.last_block, "foldedBlock": c.folded_block, "fullHistory": c.full_history,
            "ready": chain_walk::fresh_fold_on(&c, indexes.chain_id(net), now).is_some(), "reorgRewinds": c.reorg_rewinds, "divergedAt": c.diverged_at,
        }))));
    }
    Value::Object(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::evm::contracts_addrs::DclContracts;
    use alloy_primitives::{Bytes, FixedBytes, keccak256};
    use alloy_sol_types::SolValue;
    use std::str::FromStr;

    const COLLECTION: &str = "0x00000000000000000000000000000000000000c1";
    const ALICE: &str = "0xaaaa000000000000000000000000000000000001";
    const BOB: &str = "0xbbbb000000000000000000000000000000000002";

    #[test]
    fn the_topics_are_the_keccak_of_their_signatures_and_the_venues_the_pinned_book() {
        for (sig, topic) in [
            (
                "Issue(address,uint256,uint256,uint256,address)",
                TOPIC_ISSUE,
            ),
            (
                "BidCreated(bytes32,address,uint256,address,uint256,uint256,bytes)",
                TOPIC_BID_CREATED,
            ),
            (
                "BidAccepted(bytes32,address,uint256,address,address,uint256,uint256)",
                TOPIC_BID_ACCEPTED,
            ),
            (
                "BidCancelled(bytes32,address,uint256,address)",
                TOPIC_BID_CANCELLED,
            ),
            (
                "Bought((address,uint256[],uint256[],address[])[])",
                TOPIC_BOUGHT,
            ),
        ] {
            assert_eq!(format!("{:#x}", keccak256(sig)), topic, "{sig}");
        }
        assert_eq!(
            &format!("{:#x}", keccak256("royaltiesCutPerMillion()"))[..10],
            ROYALTIES_CUT_SELECTOR
        );
        let book = DclContracts::for_chain(137).unwrap();
        assert_eq!(format!("{:#x}", book.collection_store), STORE_MATIC);
        assert_eq!(format!("{:#x}", book.bid_v2), BID_V2_MATIC);
    }

    pub(crate) fn log(
        address: &str,
        topics: Vec<String>,
        data: Vec<u8>,
        block: u64,
        index: u64,
    ) -> RpcLog {
        RpcLog {
            address: address.to_string(),
            topics,
            data: format!("0x{}", hex::encode(data)),
            block_number: format!("0x{block:x}"),
            log_index: format!("0x{index:x}"),
            block_timestamp: Some(format!("0x{:x}", 1_600_000_000 + block)),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", block * 100 + index)),
        }
    }

    pub(crate) fn issue_log(
        collection: &str,
        beneficiary: &str,
        token: U256,
        item: u64,
        caller: &str,
        block: u64,
        index: u64,
    ) -> RpcLog {
        log(
            collection,
            vec![
                TOPIC_ISSUE.into(),
                nft_index::tests::topic_addr(beneficiary),
                nft_index::tests::topic_word(token),
                nft_index::tests::topic_word(U256::from(item)),
            ],
            (
                U256::from(1u8),
                alloy_primitives::Address::from_str(caller).unwrap(),
            )
                .abi_encode_params(),
            block,
            index,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one argument per event log field"
    )]
    pub(crate) fn bid_created_log(
        collection: &str,
        token: U256,
        bidder: &str,
        id: u8,
        price: u64,
        expires: u64,
        block: u64,
        index: u64,
    ) -> RpcLog {
        log(
            BID_V2_MATIC,
            vec![
                TOPIC_BID_CREATED.into(),
                nft_index::tests::topic_addr(collection),
                nft_index::tests::topic_word(token),
                nft_index::tests::topic_addr(bidder),
            ],
            (
                FixedBytes::<32>::repeat_byte(id),
                U256::from(price),
                U256::from(expires),
                Bytes::new(),
            )
                .abi_encode_params(),
            block,
            index,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one argument per event log field"
    )]
    pub(crate) fn bid_accepted_log(
        collection: &str,
        token: U256,
        bidder: &str,
        seller: &str,
        id: u8,
        price: u64,
        block: u64,
        index: u64,
    ) -> RpcLog {
        log(
            BID_V2_MATIC,
            vec![
                TOPIC_BID_ACCEPTED.into(),
                nft_index::tests::topic_addr(collection),
                nft_index::tests::topic_word(token),
                nft_index::tests::topic_addr(seller),
            ],
            (
                FixedBytes::<32>::repeat_byte(id),
                alloy_primitives::Address::from_str(bidder).unwrap(),
                U256::from(price),
                U256::from(price / 40),
            )
                .abi_encode_params(),
            block,
            index,
        )
    }

    pub(crate) fn bid_cancelled_log(
        collection: &str,
        token: U256,
        bidder: &str,
        id: u8,
        block: u64,
        index: u64,
    ) -> RpcLog {
        log(
            BID_V2_MATIC,
            vec![
                TOPIC_BID_CANCELLED.into(),
                nft_index::tests::topic_addr(collection),
                nft_index::tests::topic_word(token),
                nft_index::tests::topic_addr(bidder),
            ],
            FixedBytes::<32>::repeat_byte(id).abi_encode(),
            block,
            index,
        )
    }

    /// A store `Bought` of `items` (collection, item, price, beneficiary) in transaction `tx`.
    pub(crate) fn bought_log(
        items: &[(&str, u64, u128, &str)],
        tx: &str,
        block: u64,
        index: u64,
    ) -> RpcLog {
        let addr = |a: &str| alloy_primitives::Address::from_str(a).unwrap();
        type Item = (
            alloy_primitives::Address,
            Vec<U256>,
            Vec<U256>,
            Vec<alloy_primitives::Address>,
        );
        let encoded: Vec<Item> = items
            .iter()
            .map(|(c, id, price, b)| {
                (
                    addr(c),
                    vec![U256::from(*id)],
                    vec![U256::from(*price)],
                    vec![addr(b)],
                )
            })
            .collect();
        let mut l = log(
            STORE_MATIC,
            vec![TOPIC_BOUGHT.into()],
            (encoded,).abi_encode_params(),
            block,
            index,
        );
        l.transaction_hash = Some(tx.to_string());
        l
    }

    /// A Bought of two items, one of them twice, prices the transaction's
    /// store mints by collection, item and beneficiary; another transaction's
    /// mint and a bare data word stay unpriced.
    #[test]
    fn the_stores_bought_prices_its_transactions_mints() {
        let other = "0x00000000000000000000000000000000000000c2";
        let tx = format!("0x{:064x}", 10_000);
        let mint = |item: u64, beneficiary: &str, index: u64| {
            let token = (U256::from(item) << 216) | U256::from(index);
            decode_log(
                &issue_log(
                    COLLECTION,
                    beneficiary,
                    token,
                    item,
                    STORE_MATIC,
                    100,
                    index,
                ),
                5,
            )
            .unwrap()
            .unwrap()
        };
        let mut rows = vec![
            SaleEvent {
                tx_hash: tx.clone(),
                ..mint(3, ALICE, 0)
            },
            SaleEvent {
                tx_hash: tx.clone(),
                ..mint(3, ALICE, 1)
            },
            SaleEvent {
                tx_hash: tx.clone(),
                ..mint(4, BOB, 2)
            },
            mint(3, ALICE, 3),
        ];
        let bought = bought_log(
            &[
                (COLLECTION, 3, 2_000_000_000_000_000_000, ALICE),
                (COLLECTION, 3, 2_000_000_000_000_000_000, ALICE),
                (other, 4, 9, BOB),
                (COLLECTION, 4, 0, BOB),
            ],
            &tx,
            100,
            4,
        );
        let (decoded_tx, items) = decode_bought(&bought).unwrap().unwrap();
        assert_eq!(decoded_tx, tx);
        assert_eq!(items.len(), 4);
        assert_eq!(
            items[2],
            (
                other.to_string(),
                "4".to_string(),
                BOB.to_string(),
                "9".to_string()
            )
        );
        crate::market::store_sales::pair(STORE_MATIC, &mut rows, &[bought]).unwrap();
        let prices: Vec<_> = rows.iter().map(|r| r.price.as_deref()).collect();
        assert_eq!(
            prices,
            [
                Some("2000000000000000000"),
                Some("2000000000000000000"),
                Some("0"),
                None
            ]
        );
        let mut bare = bought_log(&[], &tx, 100, 5);
        bare.data = format!("0x{:064x}", 1);
        assert!(decode_bought(&bare).is_err());
        assert_eq!(
            decode_bought(&issue_log(
                COLLECTION,
                ALICE,
                U256::from(1u8),
                0,
                STORE_MATIC,
                1,
                0
            ))
            .unwrap(),
            None
        );
    }

    #[test]
    fn a_store_issue_is_a_mint_and_a_creators_own_issue_is_not() {
        let token = (U256::from(3u8) << 216) | U256::from(7u8);
        let kept = decode_log(
            &issue_log(COLLECTION, ALICE, token, 3, STORE_MATIC, 100, 1),
            5,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            kept,
            SaleEvent {
                block_number: 100,
                log_index: 1,
                block_time: 5,
                kind: KIND_MINT,
                emitter: COLLECTION.into(),
                contract_address: COLLECTION.into(),
                token_id: token.to_string(),
                item_id: Some("3".into()),
                bid_id: None,
                buyer: ALICE.into(),
                seller: None,
                price: None,
                bid_fee: None,
                expires_at: None,
                tx_hash: format!("0x{:064x}", 10_001),
                fingerprint: None,
            }
        );
        assert_eq!(
            decode_log(&issue_log(COLLECTION, ALICE, token, 3, BOB, 100, 2), 5).unwrap(),
            None,
            "an Issue the store did not call is a creator's mint, not a sale"
        );
        assert_eq!(
            decode_log(
                &nft_index::tests::transfer_log(COLLECTION, ALICE, BOB, token, 100, 3),
                5
            )
            .unwrap(),
            None
        );
        let mut no_hash = issue_log(COLLECTION, ALICE, token, 3, STORE_MATIC, 100, 1);
        no_hash.transaction_hash = None;
        assert!(decode_log(&no_hash, 5).is_err());
    }

    /// A creation keeps its fingerprint as the marketplace subgraph spelled
    /// `Bytes`: 0x-prefixed lowercase hex, `0x` when empty. No other event
    /// carries one.
    #[test]
    fn a_bid_creation_keeps_its_fingerprint() {
        let token = U256::from(9u8);
        let with = |fingerprint: Vec<u8>| {
            log(
                BID_V2_MATIC,
                vec![
                    TOPIC_BID_CREATED.into(),
                    nft_index::tests::topic_addr(COLLECTION),
                    nft_index::tests::topic_word(token),
                    nft_index::tests::topic_addr(BOB),
                ],
                (
                    FixedBytes::<32>::repeat_byte(0x11),
                    U256::from(700u64),
                    U256::from(1_700_000_000u64),
                    Bytes::from(fingerprint),
                )
                    .abi_encode_params(),
                200,
                0,
            )
        };
        for (bytes, expected) in [(vec![0xAB, 0xCD], "0xabcd"), (vec![], "0x")] {
            let created = decode_log(&with(bytes), 6).unwrap().unwrap();
            assert_eq!(created.fingerprint.as_deref(), Some(expected));
        }
        let cancelled = decode_log(&bid_cancelled_log(COLLECTION, token, BOB, 0x11, 201, 0), 6)
            .unwrap()
            .unwrap();
        assert_eq!(cancelled.fingerprint, None);
    }

    #[test]
    fn the_bid_lifecycle_decodes() {
        let token = U256::from(9u8);
        let created = decode_log(
            &bid_created_log(COLLECTION, token, BOB, 0x11, 700, 1_700_000_000, 200, 0),
            6,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                created.kind,
                created.emitter.as_str(),
                created.contract_address.as_str(),
                created.buyer.as_str(),
                created.bid_id.as_deref(),
                created.price.as_deref(),
                created.expires_at,
                created.seller.as_deref(),
            ),
            (
                KIND_BID_CREATED,
                BID_V2_MATIC,
                COLLECTION,
                BOB,
                Some(format!("0x{}", "11".repeat(32)).as_str()),
                Some("700"),
                Some(1_700_000_000),
                None,
            )
        );
        let accepted = decode_log(
            &bid_accepted_log(COLLECTION, token, BOB, ALICE, 0x11, 700, 201, 0),
            7,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                accepted.kind,
                accepted.buyer.as_str(),
                accepted.seller.as_deref(),
                accepted.price.as_deref(),
                accepted.bid_id.as_deref(),
                accepted.bid_fee.as_deref(),
            ),
            (
                KIND_BID_ACCEPTED,
                BOB,
                Some(ALICE),
                Some("700"),
                Some(format!("0x{}", "11".repeat(32)).as_str()),
                Some("17"),
            )
        );
        let cancelled = decode_log(&bid_cancelled_log(COLLECTION, token, BOB, 0x11, 202, 0), 8)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                cancelled.kind,
                cancelled.buyer.as_str(),
                cancelled.token_id.as_str(),
                cancelled.bid_id.as_deref(),
                cancelled.price,
            ),
            (
                KIND_BID_CANCELLED,
                BOB,
                "9",
                Some(format!("0x{}", "11".repeat(32)).as_str()),
                None,
            )
        );
    }
}
