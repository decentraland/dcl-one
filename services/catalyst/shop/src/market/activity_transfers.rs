//! The transfer leg of `/v1/activity`: one wallet's newest NFT transfers
//! from the nft index (`shop.nft_event`, kind `transfer`: the collections'
//! wearables and emotes on both networks and the DCL names), as
//! marketplace-server serves them (`ports/transfers`). A transfer out of
//! the wallet is sent (a burn included), one into it received (a mint left
//! out); a transfer to itself is neither. A transfer that settled a sale
//! shows only as that sale: the token's legacy `OrderSuccessful`
//! (`nft_event`), its accepted bid (`sale_event`) or an executed trade
//! naming it (`trade_event`) in the same transaction, or in the same block
//! when either log was indexed without its transaction hash.
//!
//! Each network is read up to the lowest published fold of the three
//! indexes that decide it (`nft`, `trade_events`, `sales`), read on one
//! snapshot with the rows. A network whose three indexes are not all
//! published contributes nothing, and the leg names the missing ones. LAND
//! and Estate transfers live in the land-authz index, which keeps no sender
//! or transaction, and are not read here.

use std::collections::BTreeSet;

use chrono::Utc;
use serde::Serialize;
use sqlx::PgConnection;

use super::chain_walk::{self, ChainNet, INDEXER_NFT, INDEXER_SALES, INDEXER_TRADE_EVENTS};
use super::nft_index::ZERO_ADDRESS;
use super::wire::{ChainId, Network};
use super::{MarketplaceState, Source};

/// One transfer, in marketplace-server's `Transfer` shape (`details.transfer`).
#[derive(Debug, Clone, Serialize)]
pub struct Transfer {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "itemId")]
    pub item_id: Option<String>,
    pub category: String,
    /// `None` on a row the walk indexed before it recorded transfers'
    /// transactions.
    #[serde(rename = "txHash")]
    pub tx_hash: Option<String>,
    /// Milliseconds, as every activity timestamp.
    pub timestamp: i64,
    pub network: Network,
    #[serde(rename = "chainId")]
    pub chain_id: ChainId,
}

/// The two sides of the leg, newest first, each capped on its own.
#[derive(Debug, Default)]
pub struct TransferLegs {
    pub sent: Vec<Transfer>,
    pub received: Vec<Transfer>,
    /// The sources whose unpublished fold left a network out.
    pub unavailable: BTreeSet<&'static str>,
}

#[derive(sqlx::FromRow)]
struct Row {
    sent: bool,
    network: String,
    block_number: i64,
    log_index: i32,
    block_time: i64,
    contract_address: String,
    token_id: String,
    from_address: String,
    to_address: String,
    tx_hash: Option<String>,
    item_id: Option<String>,
    category: String,
}

const INDEXERS: [&str; 3] = [INDEXER_NFT, INDEXER_TRADE_EVENTS, INDEXER_SALES];

fn source(indexer: &str) -> &'static str {
    match indexer {
        INDEXER_TRADE_EVENTS => Source::TradeEventsIndex.name(),
        INDEXER_SALES => Source::SalesIndex.name(),
        _ => Source::NftIndex.name(),
    }
}

/// The networks whose three indexes are published at `now`, each with the
/// lowest published fold, and the sources missing elsewhere.
async fn bounds(
    conn: &mut PgConnection,
    state: &MarketplaceState,
    now: chrono::DateTime<Utc>,
) -> sqlx::Result<(Vec<(ChainNet, i64)>, BTreeSet<&'static str>)> {
    let indexes = &state.settings.market.indexes;
    let published = chain_walk::published_sql_on(
        "c",
        "$2",
        indexes.chain_id(ChainNet::Ethereum),
        indexes.chain_id(ChainNet::Matic),
    );
    let rows: Vec<(String, String, Option<i64>)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT c.network, c.indexer, CASE WHEN {published} THEN c.folded_block END \
         FROM shop.chain_cursor c WHERE c.indexer = ANY($1)"
    )))
    .bind(INDEXERS)
    .bind(now.naive_utc())
    .fetch_all(&mut *conn)
    .await?;
    let mut ready = Vec::new();
    let mut unavailable = BTreeSet::new();
    for net in ChainNet::ALL {
        let mut upto = Some(i64::MAX);
        for indexer in INDEXERS {
            let folded = rows
                .iter()
                .find(|(n, i, _)| n == net.db_name() && i == indexer)
                .and_then(|(_, _, folded)| *folded);
            match folded {
                Some(block) => upto = upto.map(|u| u.min(block)),
                None => {
                    upto = None;
                    unavailable.insert(source(indexer));
                }
            }
        }
        if let Some(upto) = upto {
            ready.push((net, upto));
        }
    }
    Ok((ready, unavailable))
}

/// `$1` the wallet, `$2`/`$3` the read networks and their bounds, `$4` the
/// cap of each side, `$5` the zero address, `$6` the node's DCL registrar.
/// Each side walks its own address index newest first, so a wallet's
/// history is not sorted whole.
fn rows_sql() -> String {
    let side = |sent: bool| {
        let (mine, other) = if sent {
            ("from_address", "to_address")
        } else {
            ("to_address", "from_address")
        };
        let mint = if sent {
            ""
        } else {
            " AND e.from_address <> $5"
        };
        format!(
            "(SELECT {sent} AS sent, e.network, e.block_number, e.log_index, e.block_time, e.contract_address, \
                     e.token_id, e.from_address, e.to_address, e.tx_hash \
              FROM shop.nft_event e JOIN bound b ON b.network = e.network \
              WHERE e.kind = 'transfer' AND e.{mine} = $1 AND e.{other} <> $1{mint} \
                AND e.block_number <= b.upto AND NOT {settled} \
              ORDER BY e.block_time DESC, e.network, e.block_number DESC, e.log_index DESC LIMIT $4)",
            settled = settled_sql("e"),
        )
    };
    format!(
        "WITH bound AS (SELECT * FROM unnest($2::text[], $3::bigint[]) AS b(network, upto)) \
         SELECT t.sent, t.network, t.block_number, t.log_index, t.block_time, t.contract_address, \
                t.token_id::text AS token_id, t.from_address, t.to_address, t.tx_hash, n.item_id::text AS item_id, \
                CASE WHEN t.contract_address = $6 THEN 'ens' WHEN i.item_type = 'emote_v1' THEN 'emote' \
                     ELSE 'wearable' END AS category \
         FROM ({sent} UNION ALL {received}) t \
         LEFT JOIN shop.nft n ON n.network = t.network AND n.contract_address = t.contract_address \
              AND n.token_id = t.token_id \
         LEFT JOIN shop.items i ON i.network = t.network AND i.contract_address = t.contract_address \
              AND i.item_id = n.item_id \
         ORDER BY t.block_time DESC, t.network, t.block_number DESC, t.log_index DESC",
        sent = side(true),
        received = side(false),
    )
}

/// Whether the transfer `e` moved its token in a sale: the same
/// transaction, or the same block when either side has no hash (a row
/// indexed before the walk recorded it; an old trade row keeps an empty
/// one). Each probe reads its table's token or transaction index.
fn settled_sql(e: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM shop.nft_event o \
             WHERE o.network = {e}.network AND o.contract_address = {e}.contract_address \
               AND o.token_id = {e}.token_id AND o.kind = 'order_successful' \
               AND (o.tx_hash = {e}.tx_hash \
                    OR ((o.tx_hash IS NULL OR {e}.tx_hash IS NULL) AND o.block_number = {e}.block_number))) \
          OR EXISTS (SELECT 1 FROM shop.sale_event s \
             WHERE s.network = {e}.network AND s.contract_address = {e}.contract_address \
               AND s.token_id = {e}.token_id AND s.kind = 'bid_accepted' \
               AND (s.tx_hash = {e}.tx_hash OR ({e}.tx_hash IS NULL AND s.block_number = {e}.block_number))) \
          OR EXISTS (SELECT 1 FROM shop.trade_event t \
             CROSS JOIN LATERAL (SELECT jsonb_build_array(jsonb_build_object('assetType', '3', \
                 'contractAddress', {e}.contract_address, 'value', {e}.token_id::text)) AS asset) a \
             WHERE t.network = {e}.network AND t.kind = 'traded' \
               AND (t.tx_hash = {e}.tx_hash \
                    OR (({e}.tx_hash IS NULL OR t.tx_hash = '') AND t.block_number = {e}.block_number)) \
               AND (t.assets->'sent' @> a.asset OR t.assets->'received' @> a.asset)))"
    )
}

fn chain_of(state: &MarketplaceState, net: ChainNet) -> Option<ChainId> {
    u64::try_from(state.settings.market.indexes.chain_id(net))
        .ok()
        .and_then(|id| ChainId::try_from(id).ok())
}

/// The wallet's newest `cap` transfers on each side, from the networks
/// whose indexes are published. A failed read contributes nothing, as the
/// other legs' failures do.
pub async fn transfers_of_address(
    state: &MarketplaceState,
    address: &str,
    cap: i64,
) -> TransferLegs {
    match read(state, address, cap).await {
        Ok(legs) => legs,
        Err(e) => {
            tracing::warn!(error = %e, "activity: the transfers leg failed");
            TransferLegs::default()
        }
    }
}

async fn read(state: &MarketplaceState, address: &str, cap: i64) -> sqlx::Result<TransferLegs> {
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let (ready, unavailable) = bounds(&mut tx, state, Utc::now()).await?;
    let mut legs = TransferLegs {
        unavailable,
        ..Default::default()
    };
    if ready.is_empty() {
        return Ok(legs);
    }
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(rows_sql()))
        .bind(address.to_lowercase())
        .bind(ready.iter().map(|(n, _)| n.db_name()).collect::<Vec<_>>())
        .bind(ready.iter().map(|(_, b)| *b).collect::<Vec<_>>())
        .bind(cap)
        .bind(ZERO_ADDRESS)
        .bind(state.settings.catalogue().registrar())
        .fetch_all(&mut *tx)
        .await?;
    tx.rollback().await?;
    for r in rows {
        let Some(net) = ChainNet::parse(&r.network) else {
            continue;
        };
        let Some(chain_id) = chain_of(state, net) else {
            continue;
        };
        let transfer = Transfer {
            id: format!("{}-{}-{}", r.network, r.block_number, r.log_index),
            from: r.from_address,
            to: r.to_address,
            contract_address: r.contract_address,
            token_id: r.token_id,
            item_id: r.item_id,
            category: r.category,
            tx_hash: r.tx_hash,
            timestamp: r.block_time * 1000,
            network: match net {
                ChainNet::Ethereum => Network::Ethereum,
                ChainNet::Matic => Network::Matic,
            },
            chain_id,
        };
        if r.sent {
            legs.sent.push(transfer);
        } else {
            legs.received.push(transfer);
        }
    }
    Ok(legs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::tests::support::OwnDb;
    use crate::market::transfer_notices::tests::publish;
    use std::sync::Arc;

    const ME: &str = "0x00000000000000000000000000000000000000a1";
    const OTHER: &str = "0x00000000000000000000000000000000000000b0";
    const WEARABLES: &str = "0x00000000000000000000000000000000000c011e";
    const EMOTES: &str = "0x00000000000000000000000000000000000e3073";

    fn tx(n: u32) -> String {
        format!("0x{n:064x}")
    }

    struct Seed<'a>(&'a sqlx::PgPool);

    impl Seed<'_> {
        #[allow(
            clippy::too_many_arguments,
            reason = "fixture: one argument per log field"
        )]
        async fn transfer(
            &self,
            net: &str,
            block: i64,
            log: i32,
            contract: &str,
            token: &str,
            from: &str,
            to: &str,
            hash: Option<String>,
        ) {
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, tx_hash) \
                 VALUES ($1, $2, $3, 1700000000 + $2, 'transfer', $4, $4, $5::numeric, $6, $7, $8)",
            )
            .bind(net)
            .bind(block)
            .bind(log)
            .bind(contract)
            .bind(token)
            .bind(from)
            .bind(to)
            .bind(hash)
            .execute(self.0)
            .await
            .unwrap();
        }

        async fn order_sold(&self, block: i64, log: i32, token: &str, hash: Option<String>) {
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, tx_hash) \
                 VALUES ('matic', $1, $2, 1700000000 + $1, 'order_successful', '0x00000000000000000000000000000000000000ee', $3, $4::numeric, $5, $6, 1, $7)",
            )
            .bind(block)
            .bind(log)
            .bind(WEARABLES)
            .bind(token)
            .bind(ME)
            .bind(OTHER)
            .bind(hash)
            .execute(self.0)
            .await
            .unwrap();
        }

        async fn bid_accepted(&self, block: i64, log: i32, token: &str, hash: String) {
            sqlx::query(
                "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, bid_id, buyer, seller, price, tx_hash) \
                 VALUES ('matic', $1, $2, 1700000000 + $1, 'bid_accepted', '0x00000000000000000000000000000000000000bd', $3, $4::numeric, $5, $6, $7, 1, $8)",
            )
            .bind(block)
            .bind(log)
            .bind(WEARABLES)
            .bind(token)
            .bind(tx(9_000 + log as u32))
            .bind(OTHER)
            .bind(ME)
            .bind(hash)
            .execute(self.0)
            .await
            .unwrap();
        }

        async fn traded(&self, block: i64, log: i32, token: &str, hash: &str) {
            sqlx::query(
                "INSERT INTO shop.trade_event (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, signer, assets, tx_hash) \
                 VALUES ('matic', $1, $2, 1700000000 + $1, '0x00000000000000000000000000000000000000aa', 'traded', $3, $4, $3, \
                         jsonb_build_object('sent', jsonb_build_array(jsonb_build_object('assetType', '3', 'contractAddress', $5::text, 'value', $6::text, 'beneficiary', $7::text, 'extra', '0x')), 'received', '[]'::jsonb), $8)",
            )
            .bind(block)
            .bind(log)
            .bind(OTHER)
            .bind(tx(8_000 + log as u32))
            .bind(WEARABLES)
            .bind(token)
            .bind(OTHER)
            .bind(hash)
            .execute(self.0)
            .await
            .unwrap();
        }
    }

    async fn state(pool: &sqlx::PgPool) -> Arc<MarketplaceState> {
        Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings::for_tests(),
        ))
    }

    fn ids(legs: &[Transfer]) -> Vec<&str> {
        legs.iter().map(|t| t.id.as_str()).collect()
    }

    /// Sent (a burn included) and received (a mint left out), never one to
    /// oneself; a transfer that settled a legacy order, an accepted bid or
    /// a trade, in its transaction or (without a hash) in its block, shows
    /// only as that sale; two moves in one transaction stay two; a row past
    /// the lowest published fold waits; a network whose sales index is not
    /// published contributes nothing and is named.
    #[tokio::test]
    async fn a_wallet_sees_its_transfers_outside_sales_up_to_the_published_folds() {
        let Some(db) = OwnDb::create("activity_transfers").await else {
            return;
        };
        let pool = &db.pool;
        let seed = Seed(pool);
        let zero = ZERO_ADDRESS;
        for (net, block) in [("matic", 200), ("ethereum", 50)] {
            for indexer in [INDEXER_NFT, INDEXER_TRADE_EVENTS] {
                publish(pool, indexer, net, block, 1_700_000_000 + block).await;
            }
        }
        // The Polygon sales fold trails the others: block 190 bounds the read.
        publish(pool, INDEXER_SALES, "matic", 190, 1_700_000_190).await;
        publish(pool, INDEXER_SALES, "ethereum", 50, 1_700_000_050).await;

        seed.transfer("matic", 100, 0, WEARABLES, "1", ME, OTHER, Some(tx(1)))
            .await;
        seed.transfer("matic", 101, 0, WEARABLES, "2", OTHER, ME, Some(tx(2)))
            .await;
        seed.transfer("matic", 102, 0, WEARABLES, "3", zero, ME, Some(tx(3)))
            .await;
        seed.transfer("matic", 103, 0, WEARABLES, "4", ME, zero, Some(tx(4)))
            .await;
        seed.transfer("matic", 104, 0, WEARABLES, "5", ME, ME, Some(tx(5)))
            .await;
        // Sales: a legacy order, an accepted bid and a trade in the same
        // transaction; a row indexed without a hash, sold in its block.
        seed.transfer("matic", 110, 1, WEARABLES, "6", ME, OTHER, Some(tx(6)))
            .await;
        seed.order_sold(110, 2, "6", Some(tx(6))).await;
        seed.transfer("matic", 111, 1, WEARABLES, "7", ME, OTHER, Some(tx(7)))
            .await;
        seed.bid_accepted(111, 2, "7", tx(7)).await;
        seed.transfer("matic", 112, 1, WEARABLES, "8", ME, OTHER, Some(tx(8)))
            .await;
        seed.traded(112, 2, "8", &tx(8)).await;
        seed.transfer("matic", 113, 1, WEARABLES, "9", ME, OTHER, None)
            .await;
        seed.order_sold(113, 2, "9", Some(tx(9))).await;
        // Not a sale: another token's trade in the transaction, and a sale
        // of the token in another block.
        seed.transfer("matic", 114, 1, WEARABLES, "10", ME, OTHER, Some(tx(10)))
            .await;
        seed.traded(114, 2, "11", &tx(10)).await;
        seed.transfer("matic", 115, 1, WEARABLES, "12", ME, OTHER, None)
            .await;
        seed.order_sold(116, 2, "12", None).await;
        // Two moves in one transaction, an emote with its item, a name.
        seed.transfer(
            "matic",
            120,
            1,
            EMOTES,
            &(3u128 << 100).to_string(),
            OTHER,
            ME,
            Some(tx(20)),
        )
        .await;
        seed.transfer("matic", 120, 2, WEARABLES, "21", OTHER, ME, Some(tx(20)))
            .await;
        // Past the Polygon sales fold.
        seed.transfer("matic", 195, 0, WEARABLES, "30", ME, OTHER, Some(tx(30)))
            .await;

        let registrar = crate::market::nft_index::DCL_REGISTRAR;
        seed.transfer("ethereum", 40, 0, registrar, "77", OTHER, ME, Some(tx(40)))
            .await;

        let emote_token = (3u128 << 100).to_string();
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2::numeric, $3, 5, 1, 1, 1, 120, 1, 1)",
        )
        .bind(EMOTES)
        .bind(&emote_token)
        .bind(ME)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 5, 137, 'emote_v1', 'urn:emote:5', $2, 'epic', 1, 10, 9, 0, 'Wave', 'dance', 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(EMOTES)
        .bind(OTHER)
        .execute(pool)
        .await
        .unwrap();

        let state = state(pool).await;
        let legs = transfers_of_address(&state, &ME.to_uppercase().replace("0X", "0x"), 500).await;
        assert_eq!(
            ids(&legs.sent),
            vec!["matic-115-1", "matic-114-1", "matic-103-0", "matic-100-0"]
        );
        assert_eq!(
            ids(&legs.received),
            vec!["matic-120-2", "matic-120-1", "matic-101-0", "ethereum-40-0"]
        );
        assert!(legs.unavailable.is_empty(), "{:?}", legs.unavailable);

        let emote = &legs.received[1];
        assert_eq!(
            (
                emote.category.as_str(),
                emote.item_id.as_deref(),
                emote.tx_hash.as_deref(),
                emote.timestamp,
                emote.chain_id,
            ),
            (
                "emote",
                Some("5"),
                Some(tx(20).as_str()),
                1_700_000_120_000,
                ChainId::MaticMainnet
            )
        );
        let name = &legs.received[3];
        assert_eq!(
            (name.category.as_str(), name.network, name.from.as_str()),
            ("ens", Network::Ethereum, OTHER)
        );
        assert_eq!(legs.received[0].category, "wearable");
        assert_eq!(
            legs.sent[0].tx_hash, None,
            "a row without its hash is served"
        );
        let wire = serde_json::to_value(&legs.sent[3]).unwrap();
        assert_eq!(
            (
                wire["network"].as_str(),
                wire["chainId"].as_u64(),
                wire["txHash"].as_str(),
                wire["itemId"].clone(),
                wire["contractAddress"].as_str(),
            ),
            (
                Some("MATIC"),
                Some(137),
                Some(tx(1).as_str()),
                serde_json::Value::Null,
                Some(WEARABLES)
            )
        );

        // Each side is capped on its own.
        let capped = transfers_of_address(&state, ME, 1).await;
        assert_eq!(
            (ids(&capped.sent), ids(&capped.received)),
            (vec!["matic-115-1"], vec!["matic-120-2"])
        );

        // Ethereum's trade-events fold is withdrawn: only Polygon is read,
        // and the leg names the missing source.
        sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL WHERE indexer = 'trade_events' AND network = 'ethereum'")
            .execute(pool)
            .await
            .unwrap();
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(
            (
                ids(&legs.received),
                legs.unavailable.iter().copied().collect::<Vec<_>>()
            ),
            (
                vec!["matic-120-2", "matic-120-1", "matic-101-0"],
                vec!["trade-events-index"]
            )
        );
        db.drop().await;
    }
}
