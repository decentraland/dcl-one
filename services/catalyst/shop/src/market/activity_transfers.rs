//! The transfer leg of `/v1/activity`: one wallet's newest NFT transfers
//! from the nft index (`shop.nft_event`, kind `transfer`: the collections'
//! wearables and emotes on both networks and the DCL names), as
//! marketplace-server serves them (`ports/transfers`). A transfer out of
//! the wallet is sent (a burn included), one into it received (a mint left
//! out); a transfer to itself is neither.
//!
//! A transfer that settled a public sale shows only as that sale, as the
//! activity's sales leg serves it: a priced MANA sale of the token in the
//! sales ledger (`shop.sale_asset`, `shop.sale_execution`, the venues and
//! single-asset trades `sale_reads` publishes) in the same transaction, or
//! in the same block for a transfer indexed without its hash. Each network
//! is read on its own sales publication snapshot (`sale_fold::snapshot`),
//! up to the published block: the common prefix of the `nft`, `sales` and
//! `trade_events` walks whose settlement inputs the sales fold proved, on
//! the branch it published. A network without a valid publication (a
//! testnet node has none) contributes nothing, and the leg names
//! `sales-index`; a failed read names `nft-index`.
//!
//! An item's metadata is bound to the network's chain: a Polygon token's
//! item by its id, an Ethereum v1 token's by its collection URN and wearable
//! id. A collection token the catalogue does not hold has no item and no
//! category.
//!
//! LAND and Estate transfers come from the land-authz index
//! (`land_authz.authz_event`, kind `transfer`, with its sender and
//! transaction), category `parcel` or `estate`, on the same Ethereum
//! snapshot and by the same rules, up to the lower of the sales publication
//! and the land-authz fold. They are read only while the land-authz index
//! of the node's registries is ready and every transfer it holds has its
//! sender and transaction (the indexer repairs the older rows); otherwise
//! Ethereum's collection transfers are still served and the leg names
//! `land-authz-index`.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use chrono::Utc;
use serde::Serialize;

use super::chain_walk::ChainNet;
use super::nft_index::ZERO_ADDRESS;
use super::wire::{ChainId, Network};
use super::{MarketplaceState, Source, listings, sale_fold, sale_reads};

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
    /// `None` for a collection token the catalogue does not hold.
    pub category: Option<String>,
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
    /// The sources that left a network out.
    pub unavailable: BTreeSet<&'static str>,
}

#[derive(sqlx::FromRow)]
struct Row {
    sent: bool,
    block_number: i64,
    log_index: i32,
    block_time: i64,
    contract_address: String,
    token_id: String,
    from_address: String,
    to_address: String,
    tx_hash: Option<String>,
    item_id: Option<String>,
    category: Option<String>,
}

/// `$1` the wallet, `$2` the network, `$3` the cap of each side, `$4` the
/// zero address, `$5` the DCL registrar, `$6` the network's MANA, `$7` its
/// chain. Each side walks its own address index newest first, by time
/// then position, the order the activity merges in: a legacy row indexed
/// with time 0 sorts last on its side as it does in the merge.
fn rows_sql(net: ChainNet) -> String {
    let side = |sent: bool| {
        let (mine, other) = if sent {
            ("from_address", "to_address")
        } else {
            ("to_address", "from_address")
        };
        let mint = if sent {
            ""
        } else {
            " AND e.from_address <> $4"
        };
        format!(
            "(SELECT {sent} AS sent, e.block_number, e.log_index, e.block_time, e.contract_address, \
                     e.token_id, e.from_address, e.to_address, nullif(e.tx_hash, '') AS tx_hash \
              FROM shop.nft_event e CROSS JOIN publication p \
              WHERE e.network = $2 AND e.kind = 'transfer' AND e.{mine} = $1 AND e.{other} <> $1{mint} \
                AND e.block_number <= p.upto AND NOT {settled} \
              ORDER BY e.block_time DESC, e.block_number DESC, e.log_index DESC LIMIT $3)",
            settled = settled_sql("e"),
        )
    };
    let items = match net {
        ChainNet::Matic => {
            "LEFT JOIN shop.items i ON i.network = $2 AND i.contract_address = t.contract_address \
                  AND i.item_id = n.item_id AND i.chain_id = $7"
        }
        ChainNet::Ethereum => {
            "LEFT JOIN shop.collections c ON c.network = $2 AND c.contract_address = t.contract_address \
                  AND c.chain_id = $7 \
             LEFT JOIN shop.items i ON i.urn = c.urn || ':' || n.wearable_id AND i.network = $2 \
                  AND i.chain_id = $7"
        }
    };
    format!(
        "WITH publication AS (SELECT block_number AS upto FROM shop.sale_publication WHERE network = $2) \
         SELECT t.sent, t.block_number, t.log_index, t.block_time, t.contract_address, \
                t.token_id::text AS token_id, t.from_address, t.to_address, t.tx_hash, \
                coalesce(n.item_id, i.item_id)::text AS item_id, \
                CASE WHEN t.contract_address = $5 THEN 'ens' WHEN i.item_type = 'emote_v1' THEN 'emote' \
                     WHEN i.item_type IS NOT NULL THEN 'wearable' END AS category \
         FROM ({sent} UNION ALL {received}) t \
         LEFT JOIN shop.nft n ON n.network = $2 AND n.contract_address = t.contract_address \
              AND n.token_id = t.token_id \
         {items}",
        sent = side(true),
        received = side(false),
    )
}

/// Whether the transfer `e` moved its token in a public sale of the
/// ledger, by the rule `sale_reads/public_sales.sql` publishes (a positive
/// price in the network's MANA; a trade only when it moved one asset): the
/// same transaction, or the same block when the transfer has no hash. A
/// sale's executions and transfers share their block, which bounds the
/// probe to the token's sales in that block.
fn settled_sql(e: &str) -> String {
    settled_sql_on(e, &format!("{e}.network"), &format!("{e}.contract_address"))
}

/// [`settled_sql`] for a log whose network is `network` and whose token's
/// contract is `contract` (the land-authz log names neither as the nft
/// index does).
fn settled_sql_on(e: &str, network: &str, contract: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM shop.sale_asset a JOIN shop.sale_execution x USING (network, block_number, log_index) \
             WHERE a.network = {network} AND a.token_id = {e}.token_id AND a.contract_address = {contract} \
               AND a.price > 0 AND a.currency = $6 AND a.block_number = {e}.block_number \
               AND (x.tx_hash = {e}.tx_hash OR coalesce({e}.tx_hash, '') = '') \
               AND (x.kind <> 'trade' OR NOT EXISTS (SELECT FROM shop.sale_asset other \
                   WHERE (other.network, other.block_number, other.log_index) = (a.network, a.block_number, a.log_index) \
                     AND other.ordinal <> a.ordinal)))"
    )
}

/// The LAND and Estate side of Ethereum's read, from the land-authz log.
/// `$1` the wallet, `$2` the network, `$3` the cap of each side, `$4` the
/// zero address, `$5` the LAND registry, `$6` the network's MANA, `$7` the
/// Estate registry, `$8` the land-authz network. Bounded by the sales
/// publication and the land-authz fold, whichever is lower; each side walks
/// its own transfer index newest first, as [`rows_sql`] does.
fn land_rows_sql() -> String {
    let side = |sent: bool| {
        let (mine, other) = if sent {
            ("from_address", "account")
        } else {
            ("account", "from_address")
        };
        let mint = if sent {
            ""
        } else {
            " AND e.from_address <> $4"
        };
        format!(
            "(SELECT {sent} AS sent, e.block_number, e.log_index, e.block_time, \
                     e.token_address AS contract_address, e.token_id::text AS token_id, \
                     e.from_address, e.account AS to_address, e.tx_hash, NULL::text AS item_id, \
                     CASE WHEN e.token_address = $5 THEN 'parcel' ELSE 'estate' END AS category \
              FROM land_authz.authz_event e CROSS JOIN bound b \
              WHERE e.kind = 'transfer' AND e.{mine} = $1 AND e.{other} <> $1{mint} \
                AND e.token_address IN ($5, $7) AND e.block_number <= b.upto AND NOT {settled} \
              ORDER BY e.block_time DESC, e.block_number DESC, e.log_index DESC LIMIT $3)",
            settled = settled_sql_on("e", "$2", "e.token_address"),
        )
    };
    format!(
        "WITH bound AS (SELECT least(p.block_number, c.folded_block) AS upto \
                        FROM shop.sale_publication p JOIN land_authz.index_cursor c ON c.network = $8 \
                        WHERE p.network = $2) \
         {sent} UNION ALL {received}",
        sent = side(true),
        received = side(false),
    )
}

/// Whether the land-authz log may answer on this snapshot: its index of the
/// node's registries is ready ([`crate::land_authz::indexer::observation_ready`])
/// and every transfer it holds has its sender and transaction.
async fn land_ready(
    conn: &mut sqlx::PgConnection,
    land: crate::land_authz::LandNetwork,
) -> Result<bool, sqlx::Error> {
    let ready = crate::land_authz::indexer::cursor_observed(&mut *conn, land.network)
        .await?
        .is_some_and(|cursor| {
            crate::land_authz::indexer::observation_ready(
                &cursor,
                land.land_registry,
                land.estate_registry,
            )
        });
    if !ready {
        return Ok(false);
    }
    let unrepaired: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM land_authz.authz_event \
                        WHERE kind = 'transfer' AND (from_address IS NULL OR tx_hash IS NULL))",
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(!unrepaired)
}

/// Why a network is left out.
enum Missing {
    Publication,
    Failed(anyhow::Error),
}

type Keyed = (bool, (Reverse<i64>, Reverse<i64>, Reverse<i32>), Transfer);

/// One network's transfers on its sales publication snapshot, each side
/// newest first and capped, keyed for the merge; on Ethereum, LAND and
/// Estate transfers too when the land-authz log may answer, else the
/// source that left them out.
async fn read_network(
    state: &MarketplaceState,
    net: ChainNet,
    address: &str,
    cap: i64,
) -> Result<(Vec<Keyed>, Option<&'static str>), Missing> {
    let now = Utc::now();
    let mut snap = sale_fold::snapshot(&state.pool, &[net], now)
        .await
        .map_err(Missing::Failed)?
        .ok_or(Missing::Publication)?;
    let chain = net.chain_id();
    let wallet = address.to_lowercase();
    let mut rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(rows_sql(net)))
        .bind(&wallet)
        .bind(net.db_name())
        .bind(cap)
        .bind(ZERO_ADDRESS)
        .bind(state.settings.catalogue().registrar())
        .bind(listings::mana_token(chain))
        .bind(chain)
        .fetch_all(&mut *snap.transaction)
        .await
        .map_err(|e| Missing::Failed(e.into()))?;
    let mut left_out = None;
    if net == ChainNet::Ethereum {
        match read_land(&mut snap.transaction, state, &wallet, cap).await {
            Ok(Some(land)) => rows.extend(land),
            Ok(None) => left_out = Some(Source::LandAuthzIndex.name()),
            Err(e) => {
                tracing::warn!(error = %e, "activity: the LAND transfers read failed");
                left_out = Some(Source::LandAuthzIndex.name());
            }
        }
    }
    if Utc::now() > snap.expires_at {
        return Err(Missing::Publication);
    }
    let chain_id = u64::try_from(chain)
        .ok()
        .and_then(|id| ChainId::try_from(id).ok())
        .ok_or_else(|| Missing::Failed(anyhow::anyhow!("no wire chain id for {chain}")))?;
    let rows = rows
        .into_iter()
        .map(|r| {
            let key = (
                Reverse(r.block_time),
                Reverse(r.block_number),
                Reverse(r.log_index),
            );
            let transfer = Transfer {
                id: format!("{}-{}-{}", net.db_name(), r.block_number, r.log_index),
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
            (r.sent, key, transfer)
        })
        .collect();
    Ok((rows, left_out))
}

/// The wallet's LAND and Estate transfers on the Ethereum snapshot `tx`;
/// `None` when the node has no LAND network or the land-authz log may not
/// answer ([`land_ready`]).
async fn read_land(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    state: &MarketplaceState,
    wallet: &str,
    cap: i64,
) -> Result<Option<Vec<Row>>, sqlx::Error> {
    let Some(land) = state.land_network() else {
        return Ok(None);
    };
    if !land_ready(&mut *tx, land).await? {
        return Ok(None);
    }
    let chain = ChainNet::Ethereum.chain_id();
    sqlx::query_as(sqlx::AssertSqlSafe(land_rows_sql()))
        .bind(wallet)
        .bind(ChainNet::Ethereum.db_name())
        .bind(cap)
        .bind(ZERO_ADDRESS)
        .bind(land.land_registry)
        .bind(listings::mana_token(chain))
        .bind(land.estate_registry)
        .bind(land.network)
        .fetch_all(&mut **tx)
        .await
        .map(Some)
}

/// The wallet's newest `cap` transfers on each side, from the networks
/// with a valid sales publication. A network left out contributes nothing
/// and is named, as the other legs' failures leave the answer partial.
pub async fn transfers_of_address(
    state: &MarketplaceState,
    address: &str,
    cap: i64,
) -> TransferLegs {
    let mut legs = TransferLegs::default();
    if !sale_reads::enabled(state) {
        legs.unavailable.insert(Source::SalesIndex.name());
        return legs;
    }
    let reads = futures::future::join_all(
        ChainNet::ALL.map(|net| async move { (net, read_network(state, net, address, cap).await) }),
    )
    .await;
    let (mut sent, mut received) = (Vec::new(), Vec::new());
    for (net, read) in reads {
        match read {
            Ok((rows, left_out)) => {
                legs.unavailable.extend(left_out);
                for (is_sent, key, transfer) in rows {
                    let side = if is_sent { &mut sent } else { &mut received };
                    side.push(((key.0, net.db_name(), key.1, key.2), transfer));
                }
            }
            Err(Missing::Publication) => {
                legs.unavailable.insert(Source::SalesIndex.name());
            }
            Err(Missing::Failed(e)) => {
                tracing::warn!(error = %e, network = net.db_name(), "activity: the transfers leg failed");
                legs.unavailable.insert(Source::NftIndex.name());
            }
        }
    }
    for (side, out) in [(sent, &mut legs.sent), (received, &mut legs.received)] {
        let mut side = side;
        side.sort_by(|a, b| a.0.cmp(&b.0));
        out.extend(side.into_iter().take(cap as usize).map(|(_, t)| t));
    }
    legs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::tests::support::OwnDb;
    use std::sync::Arc;

    const ME: &str = "0x00000000000000000000000000000000000000a1";
    const OTHER: &str = "0x00000000000000000000000000000000000000b0";
    const CARL: &str = "0x00000000000000000000000000000000000000c0";
    const WEARABLES: &str = "0x00000000000000000000000000000000000c011e";
    const EMOTES: &str = "0x00000000000000000000000000000000000e3073";
    const AMOY_ITEMS: &str = "0x0000000000000000000000000000000000a30e70";
    const V1: &str = "0x00000000000000000000000000000000000000e1";
    const VENUE: &str = "0x00000000000000000000000000000000000000ee";

    fn tx(n: i64) -> String {
        format!("0x{n:064x}")
    }

    /// The height both networks' sale inputs are proven and published at.
    fn height() -> i64 {
        crate::market::trade_events::deployments(137)[0].1 as i64 + 1
    }

    /// The sales fold's publication at `height` on both networks, over
    /// fresh, fully covered input cursors (as `market_tests` seeds it).
    async fn publish_ledger(pool: &sqlx::PgPool) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
              full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
             SELECT ix,n,CASE n WHEN 'matic' THEN 137 ELSE 1 END,1,$1,$2,true,$1,timezone('UTC',now()),$3,$1,0
             FROM unnest(ARRAY['nft','sales','trade_events','collections','committee']) ix
             CROSS JOIN unnest(ARRAY['matic','ethereum']) n WHERE n='matic' OR ix IN ('nft','sales','trade_events')",
        )
        .bind(height())
        .bind(tx(91))
        .bind(crate::market::sale_history::VERSION)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.sale_publication(network,block_number,block_hash,input_version,input_epochs,
                input_revisions,late_collections,published_at)
             SELECT network,$1,$2,$3,array_agg(sale_input_epoch ORDER BY indexer),
                    array_agg(sale_history_revision ORDER BY indexer),0,timezone('UTC',now())
             FROM shop.chain_cursor WHERE indexer IN ('nft','sales','trade_events') GROUP BY network",
        )
        .bind(height())
        .bind(tx(91))
        .bind(sale_fold::PUBLICATION_VERSION)
        .execute(pool)
        .await
        .unwrap();
    }

    const LAND: &str = crate::land_authz::LAND_REGISTRY_MAINNET;
    const ESTATE: &str = crate::land_authz::ESTATE_REGISTRY_MAINNET;

    /// The land-authz index of the mainnet registries, folded at `folded`
    /// and published `age_minutes` ago.
    async fn publish_land(pool: &sqlx::PgPool, folded: i64, age_minutes: i32) {
        sqlx::query(
            "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
             VALUES ($1, $2, $3, $4, $4, timezone('UTC', now()) - make_interval(mins => $5), true) \
             ON CONFLICT (network) DO UPDATE SET last_block = EXCLUDED.last_block, folded_block = EXCLUDED.folded_block, \
                 folded_at = EXCLUDED.folded_at",
        )
        .bind(crate::land_authz::NETWORK_MAINNET)
        .bind(LAND)
        .bind(ESTATE)
        .bind(folded)
        .bind(age_minutes)
        .execute(pool)
        .await
        .unwrap();
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

        /// A LAND or Estate transfer in the land-authz log, with its
        /// sender and transaction (`None`: indexed before they were kept).
        #[allow(
            clippy::too_many_arguments,
            reason = "fixture: one argument per log field"
        )]
        async fn land_transfer(
            &self,
            block: i64,
            registry: &str,
            token: &str,
            from: &str,
            to: &str,
            hash: Option<String>,
        ) {
            sqlx::query(
                "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account, from_address, tx_hash) \
                 VALUES ($1, 0, 1700000000 + $1, $2, 'transfer', $3::numeric, $4, $5, $6)",
            )
            .bind(block)
            .bind(registry)
            .bind(token)
            .bind(to)
            .bind(from)
            .bind(hash)
            .execute(self.0)
            .await
            .unwrap();
        }

        /// An Ethereum sale of a LAND or Estate token in the ledger.
        async fn land_sale(&self, block: i64, registry: &str, token: &str, price: i64, hash: &str) {
            sqlx::query(
                "INSERT INTO shop.sale_execution (network, block_number, log_index, tx_hash, venue, kind, block_time) \
                 VALUES ('ethereum', $1, 9, $2, $3, 'order', 1700000000 + $1)",
            )
            .bind(block)
            .bind(hash)
            .bind(VENUE)
            .execute(self.0)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO shop.sale_asset (network, block_number, log_index, ordinal, public_id, beneficiary, kind, \
                     contract_address, token_id, seller, buyer, price, currency) \
                 VALUES ('ethereum', $1, 9, 0, $1 || '-0', $2, 'order', $3, $4::numeric, $5, $2, $6, $7)",
            )
            .bind(block)
            .bind(OTHER)
            .bind(registry)
            .bind(token)
            .bind(ME)
            .bind(price)
            .bind(listings::mana_token(1))
            .execute(self.0)
            .await
            .unwrap();
        }

        /// A Polygon venue execution in the ledger selling `assets`
        /// (token, price in MANA) in transaction `hash`.
        async fn sale(&self, block: i64, kind: &str, assets: &[(&str, i64)], hash: &str) {
            sqlx::query(
                "INSERT INTO shop.sale_execution (network, block_number, log_index, tx_hash, venue, kind, block_time) \
                 VALUES ('matic', $1, 9, $2, $3, $4, 1700000000 + $1)",
            )
            .bind(block)
            .bind(hash)
            .bind(VENUE)
            .bind(kind)
            .execute(self.0)
            .await
            .unwrap();
            for (ordinal, (token, price)) in assets.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO shop.sale_asset (network, block_number, log_index, ordinal, public_id, beneficiary, kind, \
                         contract_address, token_id, seller, buyer, price, currency) \
                     VALUES ('matic', $1, 9, $2, $1 || '-' || $2, $3, $4, $5, $6::numeric, $7, $3, $8, $9)",
                )
                .bind(block)
                .bind(ordinal as i32)
                .bind(OTHER)
                .bind(if kind == "bid" { "bid" } else { "order" })
                .bind(WEARABLES)
                .bind(*token)
                .bind(ME)
                .bind(*price)
                .bind(listings::mana_token(137))
                .execute(self.0)
                .await
                .unwrap();
            }
        }
    }

    async fn node(pool: &sqlx::PgPool, sales_enabled: bool) -> Arc<MarketplaceState> {
        let mut settings = crate::market::Settings::for_tests();
        settings.eth_network = "mainnet".into();
        settings.market.indexes.sales_enabled = sales_enabled;
        Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ))
    }

    fn ids(legs: &[Transfer]) -> Vec<String> {
        legs.iter().map(|t| t.id.clone()).collect()
    }

    fn unavailable(legs: &TransferLegs) -> Vec<&'static str> {
        legs.unavailable.iter().copied().collect()
    }

    /// Each side keeps its newest transfers by time before its cap: a
    /// legacy row indexed with time 0 at the highest position sorts last,
    /// as in the merge, and never displaces a newer transfer.
    #[tokio::test]
    async fn a_legacy_row_without_a_time_never_displaces_a_newer_transfer_from_the_cap() {
        let Some(db) = OwnDb::create("activity_transfers_cap").await else {
            return;
        };
        let pool = &db.pool;
        publish_ledger(pool).await;
        let b = height() - 600;
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, tx_hash) \
             SELECT 'matic', $1 + g, 0, CASE WHEN g = 501 THEN 0 ELSE 1700000000 + g END, 'transfer', $2, $2, g, $3, $4, \
                    '0x' || lpad(to_hex(g), 64, '0') \
             FROM generate_series(1, 501) g",
        )
        .bind(b)
        .bind(WEARABLES)
        .bind(ME)
        .bind(OTHER)
        .execute(pool)
        .await
        .unwrap();
        let legs = transfers_of_address(&*node(pool, true).await, ME, 500).await;
        let sent = ids(&legs.sent);
        assert_eq!(
            (sent.len(), sent.first().cloned(), sent.last().cloned()),
            (
                500,
                Some(format!("matic-{}-0", b + 500)),
                Some(format!("matic-{}-0", b + 1))
            )
        );
        assert!(!sent.contains(&format!("matic-{}-0", b + 501)));
        db.drop().await;
    }

    /// Sent (a burn included) and received (a mint left out), never one to
    /// oneself, up to the published block. A transfer that settled a public
    /// sale of the ledger (an order, a bid, a single-asset trade; in its
    /// transaction, or in its block without a hash) shows only as that
    /// sale; an unpriced sale, a bundle trade, another block's sale of the
    /// token (an empty hash included) and an order log from a contract the
    /// ledger does not count leave it a transfer. Two moves of one
    /// transaction stay two. Items resolve on the network's chain.
    #[tokio::test]
    async fn a_wallet_sees_its_transfers_outside_public_sales_up_to_the_publication() {
        let Some(db) = OwnDb::create("activity_transfers").await else {
            return;
        };
        let pool = &db.pool;
        let seed = Seed(pool);
        publish_ledger(pool).await;
        publish_land(pool, height(), 0).await;
        let zero = ZERO_ADDRESS;
        let b = height() - 100;
        let at = |net: &str, block: i64, log: i32| format!("{net}-{block}-{log}");

        seed.transfer("matic", b + 1, 0, WEARABLES, "1", ME, OTHER, Some(tx(1)))
            .await;
        seed.transfer("matic", b + 2, 0, WEARABLES, "2", OTHER, ME, Some(tx(2)))
            .await;
        seed.transfer("matic", b + 3, 0, WEARABLES, "3", zero, ME, Some(tx(3)))
            .await;
        seed.transfer("matic", b + 4, 0, WEARABLES, "4", ME, zero, Some(tx(4)))
            .await;
        seed.transfer("matic", b + 5, 0, WEARABLES, "5", ME, ME, Some(tx(5)))
            .await;
        // Public sales: an order, a bid, a single-asset trade, and a sale in
        // the block of a transfer indexed without its hash.
        for (block, token, kind) in [(10, "6", "order"), (11, "7", "bid"), (12, "8", "trade")] {
            seed.transfer(
                "matic",
                b + block,
                0,
                WEARABLES,
                token,
                ME,
                OTHER,
                Some(tx(block)),
            )
            .await;
            seed.sale(b + block, kind, &[(token, 5)], &tx(block)).await;
        }
        seed.transfer("matic", b + 13, 0, WEARABLES, "9", ME, OTHER, None)
            .await;
        seed.sale(b + 13, "order", &[("9", 5)], &tx(13)).await;
        // Not public sales of the transfer: another block's sale of a token
        // moved with an empty hash, a raw order log the ledger never
        // counted, a bundle trade, an unpriced sale.
        seed.transfer(
            "matic",
            b + 14,
            0,
            WEARABLES,
            "10",
            ME,
            OTHER,
            Some(String::new()),
        )
        .await;
        seed.sale(b + 6, "order", &[("10", 5)], &tx(60)).await;
        seed.transfer("matic", b + 15, 0, WEARABLES, "11", ME, OTHER, Some(tx(15)))
            .await;
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, tx_hash) \
             VALUES ('matic', $1, 1, 1700000000 + $1, 'order_successful', $2, $2, 11, $3, $4, 5, $5)",
        )
        .bind(b + 15)
        .bind(WEARABLES)
        .bind(ME)
        .bind(OTHER)
        .bind(tx(15))
        .execute(pool)
        .await
        .unwrap();
        seed.transfer("matic", b + 16, 0, WEARABLES, "12", ME, OTHER, Some(tx(16)))
            .await;
        seed.sale(b + 16, "trade", &[("12", 5), ("13", 5)], &tx(16))
            .await;
        seed.transfer("matic", b + 17, 0, WEARABLES, "14", ME, OTHER, Some(tx(17)))
            .await;
        seed.sale(b + 17, "order", &[("14", 0)], &tx(17)).await;
        // Two moves in one transaction; an item another chain's catalogue
        // holds; past the publication.
        seed.transfer("matic", b + 20, 1, EMOTES, "5", OTHER, ME, Some(tx(20)))
            .await;
        seed.transfer("matic", b + 20, 2, WEARABLES, "21", OTHER, ME, Some(tx(20)))
            .await;
        seed.transfer("matic", b + 21, 0, AMOY_ITEMS, "7", OTHER, ME, Some(tx(21)))
            .await;
        seed.transfer(
            "matic",
            height() + 1,
            0,
            WEARABLES,
            "30",
            ME,
            OTHER,
            Some(tx(30)),
        )
        .await;
        let registrar = crate::market::nft_index::DCL_REGISTRAR;
        seed.transfer(
            "ethereum",
            b + 30,
            0,
            registrar,
            "77",
            OTHER,
            ME,
            Some(tx(40)),
        )
        .await;
        seed.transfer("ethereum", b + 31, 0, V1, "41", OTHER, ME, Some(tx(41)))
            .await;
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, wearable_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, 5, $3, 5, NULL, 1, 1, 1, 1, 0, 1), ('matic', $2, 7, $3, 7, NULL, 1, 1, 1, 1, 0, 1), \
                    ('ethereum', $4, 41, $3, NULL, 'hat', 1, 1, 1, 1, 0, 1)",
        )
        .bind(EMOTES)
        .bind(AMOY_ITEMS)
        .bind(ME)
        .bind(V1)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('ethereum', $1, 1, 'V1', $2, $2, 'urn:v1', true, true, false, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(V1)
        .bind(OTHER)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 5, 137, 'emote_v1', 'urn:emote:5', $4, 'epic', 1, 10, 9, 0, 'Wave', 'dance', 1, 1, 1, timezone('UTC', now())), \
                    ('matic', $2, 7, 80002, 'emote_v1', 'urn:amoy:7', $4, 'epic', 1, 10, 9, 0, 'Amoy', 'dance', 1, 1, 1, timezone('UTC', now())), \
                    ('ethereum', $3, 5, 1, 'wearable_v1', 'urn:v1:hat', $4, 'epic', 1, 10, 9, 0, 'Hat', 'hat', 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(EMOTES)
        .bind(AMOY_ITEMS)
        .bind(V1)
        .bind(OTHER)
        .execute(pool)
        .await
        .unwrap();

        let state = node(pool, true).await;
        let legs = transfers_of_address(&state, &ME.to_uppercase().replace("0X", "0x"), 500).await;
        assert_eq!(unavailable(&legs), Vec::<&str>::new());
        assert_eq!(
            ids(&legs.sent),
            [17, 16, 15, 14, 4, 1]
                .map(|o| at("matic", b + o, 0))
                .to_vec()
        );
        assert_eq!(
            ids(&legs.received),
            vec![
                at("ethereum", b + 31, 0),
                at("ethereum", b + 30, 0),
                at("matic", b + 21, 0),
                at("matic", b + 20, 2),
                at("matic", b + 20, 1),
                at("matic", b + 2, 0),
            ]
        );
        let meta = |t: &Transfer| (t.category.clone(), t.item_id.clone());
        let some = |c: &str, i: Option<&str>| (Some(c.to_string()), i.map(str::to_string));
        assert_eq!(
            meta(&legs.received[0]),
            some("wearable", Some("5")),
            "a v1 item by its URN"
        );
        assert_eq!(meta(&legs.received[1]), some("ens", None));
        assert_eq!(
            meta(&legs.received[2]),
            (None, Some("7".into())),
            "another chain's item does not classify the token"
        );
        assert_eq!(
            meta(&legs.received[3]),
            (None, None),
            "an uncatalogued token"
        );
        assert_eq!(meta(&legs.received[4]), some("emote", Some("5")));
        assert_eq!(legs.sent[3].tx_hash, None, "an empty hash is no hash");
        let wire = serde_json::to_value(&legs.sent[5]).unwrap();
        assert_eq!(
            (
                wire["network"].as_str(),
                wire["chainId"].as_u64(),
                wire["txHash"].as_str(),
                wire["timestamp"].as_i64(),
                wire["contractAddress"].as_str(),
            ),
            (
                Some("MATIC"),
                Some(137),
                Some(tx(1).as_str()),
                Some((1_700_000_000 + b + 1) * 1000),
                Some(WEARABLES)
            )
        );

        // Each side is capped on its own, across both networks.
        let capped = transfers_of_address(&state, ME, 1).await;
        assert_eq!(
            (ids(&capped.sent), ids(&capped.received)),
            (
                vec![at("matic", b + 17, 0)],
                vec![at("ethereum", b + 31, 0)]
            )
        );

        // Without Ethereum's publication only Polygon is read; a Polygon
        // input whose history changed since its publication withdraws it.
        sqlx::query("DELETE FROM shop.sale_publication WHERE network = 'ethereum'")
            .execute(pool)
            .await
            .unwrap();
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(
            (legs.received.len(), unavailable(&legs)),
            (4, vec!["sales-index"])
        );
        sqlx::query(
            "UPDATE shop.chain_cursor SET sale_history_revision = sale_history_revision + 1 \
             WHERE network = 'matic' AND indexer = 'nft'",
        )
        .execute(pool)
        .await
        .unwrap();
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(
            (legs.sent.len() + legs.received.len(), unavailable(&legs)),
            (0, vec!["sales-index"])
        );

        // A node without the sales index, and a failed read, are named.
        let legs = transfers_of_address(&*node(pool, false).await, ME, 500).await;
        assert_eq!(unavailable(&legs), vec!["sales-index"]);
        state.pool.close().await;
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(
            (legs.sent.len() + legs.received.len(), unavailable(&legs)),
            (0, vec!["nft-index"])
        );
        db.drop().await;
    }

    /// LAND and Estate transfers come from the land-authz log beside
    /// Ethereum's collection transfers, by the same rules: sent and received
    /// (a mint left out, never one to oneself), a transfer that settled a
    /// LAND sale shows only as that sale, up to the lower of the sales
    /// publication and the land-authz fold. While the land-authz index is
    /// not ready for the node's registries, or still holds a transfer
    /// without its sender or transaction, LAND is left out and named, and
    /// the collections are still served.
    #[tokio::test]
    async fn a_wallet_sees_its_land_and_estate_transfers_from_the_land_index() {
        let Some(db) = OwnDb::create("activity_land_transfers").await else {
            return;
        };
        let pool = &db.pool;
        let seed = Seed(pool);
        publish_ledger(pool).await;
        let b = height() - 100;
        publish_land(pool, b + 50, 0).await;
        let zero = ZERO_ADDRESS;
        let at = |block: i64| format!("ethereum-{block}-0");

        seed.land_transfer(b + 1, LAND, "100", ME, OTHER, Some(tx(101)))
            .await;
        seed.land_transfer(b + 2, ESTATE, "5", OTHER, ME, Some(tx(102)))
            .await;
        seed.land_transfer(b + 3, LAND, "101", zero, ME, Some(tx(103)))
            .await;
        seed.land_transfer(b + 4, LAND, "102", ME, ME, Some(tx(104)))
            .await;
        // Sold: the sale's transaction settles it; another block's sale of
        // the same token does not.
        seed.land_transfer(b + 5, LAND, "200", ME, OTHER, Some(tx(105)))
            .await;
        seed.land_sale(b + 5, LAND, "200", 5, &tx(105)).await;
        seed.land_transfer(b + 6, ESTATE, "6", ME, OTHER, Some(tx(106)))
            .await;
        seed.land_sale(b + 1, ESTATE, "6", 5, &tx(61)).await;
        // A registry the node is not configured with, and past the fold.
        seed.land_transfer(b + 7, V1, "7", OTHER, ME, Some(tx(107)))
            .await;
        seed.land_transfer(b + 60, LAND, "100", OTHER, ME, Some(tx(160)))
            .await;
        // A collection transfer on Ethereum, served whatever LAND's state.
        seed.transfer("ethereum", b + 8, 1, V1, "41", OTHER, ME, Some(tx(108)))
            .await;

        let state = node(pool, true).await;
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(unavailable(&legs), Vec::<&str>::new());
        assert_eq!(ids(&legs.sent), vec![at(b + 6), at(b + 1)]);
        assert_eq!(
            ids(&legs.received),
            vec![format!("ethereum-{}-1", b + 8), at(b + 2)]
        );
        let parcel = serde_json::to_value(&legs.sent[1]).unwrap();
        assert_eq!(
            (
                parcel["from"].as_str(),
                parcel["to"].as_str(),
                parcel["contractAddress"].as_str(),
                parcel["tokenId"].as_str(),
                parcel["category"].as_str(),
                parcel["itemId"].as_str(),
                parcel["txHash"].as_str(),
                parcel["timestamp"].as_i64(),
                parcel["network"].as_str(),
                parcel["chainId"].as_u64(),
            ),
            (
                Some(ME),
                Some(OTHER),
                Some(LAND),
                Some("100"),
                Some("parcel"),
                None,
                Some(tx(101).as_str()),
                Some((1_700_000_000 + b + 1) * 1000),
                Some("ETHEREUM"),
                Some(1),
            )
        );
        assert_eq!(legs.received[1].category.as_deref(), Some("estate"));

        // The fold moves up to the publication: the later transfer shows.
        publish_land(pool, height(), 0).await;
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(
            legs.received.first().map(|t| t.id.clone()),
            Some(at(b + 60))
        );

        // A transfer still without its transaction leaves LAND out.
        seed.land_transfer(b + 9, LAND, "300", OTHER, CARL, None)
            .await;
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(unavailable(&legs), vec!["land-authz-index"]);
        assert_eq!(
            (ids(&legs.sent), ids(&legs.received)),
            (vec![], vec![format!("ethereum-{}-1", b + 8)])
        );
        sqlx::query("UPDATE land_authz.authz_event SET tx_hash = $1 WHERE block_number = $2")
            .bind(tx(109))
            .bind(b + 9)
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(
            unavailable(&transfers_of_address(&state, ME, 500).await),
            Vec::<&str>::new()
        );

        // A fold older than the readiness window, or an index of other
        // registries, leaves LAND out too.
        publish_land(pool, height(), 31).await;
        let legs = transfers_of_address(&state, ME, 500).await;
        assert_eq!(
            (legs.sent.len(), unavailable(&legs)),
            (0, vec!["land-authz-index"])
        );
        publish_land(pool, height(), 0).await;
        sqlx::query("UPDATE land_authz.index_cursor SET estate_registry = $1")
            .bind(V1)
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(
            unavailable(&transfers_of_address(&state, ME, 500).await),
            vec!["land-authz-index"]
        );
        db.drop().await;
    }
}
