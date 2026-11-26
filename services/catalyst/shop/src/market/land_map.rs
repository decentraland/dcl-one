//! What the explorer map's tile grid takes from Shop for the LAND and Estate
//! tokens (`explore::api::map::LandMarket`, implemented by `serve` over
//! [`land_market`], since Explore sits below Shop): the legacy order of each
//! token (`shop.nft_order`, whatever its status, so the grid can tell a live
//! order and date the last order event), the open off-chain listings that
//! sell one ([`super::catalog::unified_trades_cte_of`]: Shop's own open-trade
//! rule, so the map prices what the Shop lists), the LAND data names
//! (`shop.land_data`, with the time of the log that wrote each) and this
//! node's open rental listings (`signatures.*`, an empty set being the truth).
//!
//! Every read, readiness included, runs on the caller's connection: the
//! explorer map passes the REPEATABLE READ snapshot its LAND rows came from,
//! so the LAND facts, the index cursors and these rows are one state of the
//! database. A rewind that withdraws a publication and deletes rows after
//! the snapshot began is invisible to it; the next build sees it whole.
//!
//! Readiness, the stage-5 rule (`check_ready`), on the caller's clock (the
//! map build passes its snapshot's transaction clock, the one every
//! comparison of the build uses): the grid's chain must be
//! Ethereum mainnet (the nft index walks only it), the nft index published
//! on Ethereum (it folds the orders and the names; `chain_walk::ready_sql`,
//! the cursor bound to chain 1) and `shop.trades` readable
//! ([`super::trades::readiness_on`]), and the trade-events index current on
//! Ethereum (`trade_events::stale_networks_on`); otherwise
//! [`LandMarketError::Unready`] names the source and the grid is not built.
//! The trades' chain verdicts (`shop.trade_chain_state`) close sold and
//! cancelled listings, as they do for the Shop (`unified_trades`).
//! Trades count only when priced in MANA: an ERC-20 (received asset type 1)
//! whose token is the chain's MANA (`listings::mana_token`); another token,
//! or a price in USD-pegged MANA, is not a MANA amount. Rental listings are the ones
//! signed for the grid's chain and its Rentals contract.

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{PgConnection, Row};

use super::chain_walk::{self, ChainNet, INDEXER_NFT};
use super::{MarketplaceState, Source};
use crate::rentals::signature::rentals_contract;

/// `shop.nft_order`'s row of a LAND or Estate token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandOrder {
    pub contract_address: String,
    pub token_id: String,
    pub open: bool,
    pub seller: String,
    /// Wei, decimal.
    pub price: String,
    pub expires_at_ms: i64,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds.
    pub updated_at: i64,
}

/// An open `public_nft_order` selling a LAND or Estate token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandTrade {
    pub id: String,
    pub contract_address: String,
    pub token_id: String,
    pub signer: String,
    /// Wei, decimal.
    pub price: String,
    pub created_at_ms: i64,
}

/// An open, unexpired rental listing of a LAND or Estate token; periods as
/// `(min_days, max_days, price_per_day)`, by `min_days`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandRental {
    pub contract_address: String,
    pub token_id: String,
    pub expiration_ms: i64,
    pub updated_at_ms: i64,
    pub periods: Vec<(i64, i64, String)>,
}

/// `shop.land_data`'s row of a LAND or Estate token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandData {
    pub contract_address: String,
    pub token_id: String,
    pub name: Option<String>,
    /// Unix seconds of the Update log that wrote it.
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct LandMarket {
    pub orders: Vec<LandOrder>,
    pub trades: Vec<LandTrade>,
    pub rentals: Vec<LandRental>,
    pub data: Vec<LandData>,
}

#[derive(Debug)]
pub enum LandMarketError {
    Unready(Source, String),
    Failed(anyhow::Error),
}

impl From<sqlx::Error> for LandMarketError {
    fn from(e: sqlx::Error) -> Self {
        Self::Failed(e.into())
    }
}

/// The `shop.nft_order` rows the build reads: the registries `contracts_p`'
/// legacy orders on Ethereum. [`marks_sql`] counts and positions the same
/// rows.
fn orders_filter(contracts_p: &str) -> String {
    format!("network = 'ethereum' AND contract_address = ANY({contracts_p})")
}

/// The `shop.land_data` rows (as `alias`) the build reads: the registries
/// `contracts_p`' names. [`marks_sql`] counts and positions the same rows.
fn data_filter(alias: &str, contracts_p: &str) -> String {
    format!("{alias}.token_address = ANY({contracts_p})")
}

fn orders_sql() -> String {
    format!(
        "SELECT contract_address, token_id::text AS token_id, status = 'open' AS open, seller, price::text AS price, \
                expires_at, created_at, updated_at \
         FROM shop.nft_order WHERE {}",
        orders_filter("$1")
    )
}

fn data_sql() -> String {
    format!(
        "SELECT d.token_address, d.token_id::text AS token_id, d.name, e.block_time \
         FROM shop.land_data d \
         LEFT JOIN shop.nft_event e \
           ON e.network = 'ethereum' AND e.block_number = d.updated_block AND e.log_index = d.updated_log \
         WHERE {}",
        data_filter("d", "$1")
    )
}

/// The open, unexpired rental listings of the registries `contracts_p` at
/// `now_p` signed for chain `chain_p` and its Rentals contract `venue_p`, with
/// their periods: what the build reads, and what [`marks_sql`] digests.
fn rentals_sql(contracts_p: &str, now_p: &str, chain_p: &str, venue_p: &str) -> String {
    format!(
        "SELECT r.id, r.contract_address, r.token_id::text AS token_id, r.expiration, r.updated_at, \
           (SELECT jsonb_agg(jsonb_build_array(p.min_days, p.max_days, p.price_per_day::text) ORDER BY p.min_days, p.max_days) \
              FROM signatures.rental_periods p WHERE p.rental_id = r.id) AS periods \
         FROM signatures.rentals r \
         WHERE r.status = 'open' AND r.contract_address = ANY({contracts_p}) AND r.expiration >= {now_p} \
           AND r.chain_id = {chain_p} AND lower(r.rental_contract_address) = {venue_p} \
           AND EXISTS (SELECT 1 FROM signatures.rental_periods p WHERE p.rental_id = r.id)"
    )
}

/// The `unified_trades` rows the map prices LAND and Estate tokens with:
/// sells of a token on Ethereum, received in the MANA token `mana_p`.
fn land_trades_filter(mana_p: &str) -> String {
    format!(
        "u.type = 'public_nft_order' AND u.token_id IS NOT NULL AND lower(u.network) = 'ethereum' \
         AND u.received_asset_type = 1 AND lower(u.received_contract) = {mana_p}"
    )
}

/// The columns the build reads of a `unified_trades` row, and [`marks_sql`]
/// digests.
const TRADE_COLUMNS: &str = "u.id::text AS id, u.contract_address_sent, u.token_id, u.signer, \
    u.amount_received::text AS price, (EXTRACT(EPOCH FROM u.created_at) * 1000)::bigint AS created_at_ms";

/// The open trades of the LAND and Estate registries on Ethereum priced in
/// MANA: `$1` now, `$2` the trades floor (`trades::mirrored_since`), `$3`
/// the two registries, `$4` the chain's MANA token.
fn trades_sql() -> String {
    format!(
        "WITH {} SELECT {TRADE_COLUMNS} FROM unified_trades u WHERE {}",
        super::catalog::unified_trades_cte_of("$1", "$2", Some("$3")),
        land_trades_filter("$4")
    )
}

/// The hex SHA-256 of the rows of `rows_sql` (a subquery), as text, ordered
/// by `order_by` and then by the whole row: a total order, so equal rows
/// give equal digests whatever plan reads them (a trade has a row per
/// asset, so its `id` alone ties).
fn rows_digest(rows_sql: &str, order_by: &str) -> String {
    format!(
        "(SELECT encode(sha256(convert_to(coalesce(string_agg(x::text, ',' ORDER BY {order_by}, x::text), ''), 'UTF8')), 'hex') \
          FROM ({rows_sql}) x)"
    )
}

/// What the map build watches on Shop's side ([`land_market_marks`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandMarketMarks {
    /// Every source [`land_market`] needs is ready.
    pub ready: bool,
    /// Moves whenever a row [`land_market`] reads changes, or its readiness
    /// does; opaque, compared for equality only.
    pub marks: String,
}

/// [`land_market_marks`]' one statement: `$1` now, `$2` now in seconds rounded
/// half up (as the map build rounds it), `$3` the
/// two registries, `$4` the trades floor, `$5` the MANA token, `$6` the
/// chain, `$7` its Rentals contract. The nft index's orders and names are
/// marked by the cursor's rewrite counts (`reorg_rewinds`, `rebuilds`, each
/// raised in the transaction that rewrites rows in place: a rewind, a
/// rebuild, the estate repair's refold) and their newest
/// positions and counts: between two rewrites the walk only appends events
/// and the fold stamps each row with the event it applies, so a change moves
/// a position or a count. The live orders are counted with the build's
/// expiry rule, `round(expires_at / 1000) > round(now / 1000)` in seconds
/// rounded half up (saturated expiries included), so the count moves at the
/// very instant the build drops an expired order. The trades and rentals are small sets, marked by a
/// digest of the very rows [`land_market`] reads at `$1` (the trades after
/// the trade-events verdicts the CTE applies).
fn marks_sql() -> String {
    format!(
        "WITH {cte} \
         SELECT EXISTS (SELECT 1 FROM shop.chain_cursor c WHERE c.indexer = 'nft' AND c.network = 'ethereum' AND {nft_ready}) AS nft_ready, \
           EXISTS (SELECT 1 FROM shop.chain_cursor c WHERE c.indexer = 'trade_events' AND c.network = 'ethereum' AND {verdicts_ready}) AS verdicts_ready, \
           concat_ws('|', \
             coalesce((SELECT c.reorg_rewinds || '.' || c.rebuilds FROM shop.chain_cursor c WHERE c.indexer = 'nft' AND c.network = 'ethereum'), '-'), \
             (SELECT count(*) FILTER (WHERE status = 'open' AND floor((expires_at::numeric + 500) / 1000) > $2) FROM shop.nft_order WHERE {orders}), \
             coalesce((SELECT updated_block || '.' || updated_log FROM shop.nft_order WHERE {orders} \
                       ORDER BY updated_block DESC, updated_log DESC LIMIT 1), '-'), \
             (SELECT count(*) FROM shop.land_data d WHERE {data}), \
             coalesce((SELECT d.updated_block || '.' || d.updated_log FROM shop.land_data d WHERE {data} \
                       ORDER BY d.updated_block DESC, d.updated_log DESC LIMIT 1), '-'), \
             {trades}, \
             {rentals}) AS marks",
        cte = super::catalog::unified_trades_cte_of("$1", "$4", Some("$3")),
        nft_ready = chain_walk::ready_sql("c", "$1"),
        orders = orders_filter("$3"),
        data = data_filter("d", "$3"),
        verdicts_ready = super::trade_events::verdicts_ready_sql("c", "$1"),
        trades = rows_digest(
            &format!(
                "SELECT {TRADE_COLUMNS} FROM unified_trades u WHERE {}",
                land_trades_filter("$5")
            ),
            "x.id",
        ),
        rentals = rows_digest(&rentals_sql("$3", "$1", "$6", "$7"), "x.id"),
    )
}

/// The marks of what [`land_market`] would read for `land` and `estate` on
/// chain `chain_id` at `now`, on `conn`: one read-only statement, cheap
/// enough to run every few seconds, so the map rebuilds when Shop's side of
/// its inputs changes rather than on a timer.
pub async fn land_market_marks(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    chain_id: i64,
    land: &str,
    estate: &str,
    now: DateTime<Utc>,
) -> Result<LandMarketMarks, LandMarketError> {
    let mainnet = ChainNet::Ethereum.chain_id();
    let (Some(mana), true) = (super::listings::mana_token(chain_id), chain_id == mainnet) else {
        return Ok(LandMarketMarks {
            ready: false,
            marks: format!("chain {chain_id}"),
        });
    };
    let trades_ready = match super::trades::readiness_on(state, conn, now).await {
        Ok(_) => true,
        Err(super::trades::NotReady::Refused(_)) => false,
        Err(super::trades::NotReady::Database(e)) => return Err(e.into()),
    };
    let contracts = vec![land.to_ascii_lowercase(), estate.to_ascii_lowercase()];
    let naive_now = now.naive_utc();
    let (nft_ready, verdicts_ready, marks): (bool, bool, String) =
        sqlx::query_as(sqlx::AssertSqlSafe(marks_sql()))
            .bind(naive_now)
            .bind((now.timestamp_millis() + 500).div_euclid(1000))
            .bind(&contracts)
            .bind(super::trades::mirrored_since(&state.settings, naive_now))
            .bind(mana)
            .bind(chain_id as i32)
            .bind(rentals_contract(chain_id as u64))
            .fetch_one(&mut *conn)
            .await?;
    // Every source the build's `check_ready` asks for, the trade-events
    // verdicts current on Ethereum included (`trade_events::stale_networks_on`
    // over the same `verdicts_ready_sql`).
    Ok(LandMarketMarks {
        ready: nft_ready && trades_ready && verdicts_ready,
        marks: format!("{nft_ready}|{trades_ready}|{verdicts_ready}|{marks}"),
    })
}

/// The LAND and Estate facts of `land` and `estate` (lowercase registry
/// addresses of chain `chain_id`) at `now`, read on `conn` (see the module
/// docs), or the source that is not ready.
pub async fn land_market(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    chain_id: i64,
    land: &str,
    estate: &str,
    now: DateTime<Utc>,
) -> Result<LandMarket, LandMarketError> {
    check_ready(state, conn, chain_id, now).await?;
    read(state, conn, chain_id, land, estate, now).await
}

/// The stage-5 gate of [`land_market`], on `conn`.
async fn check_ready(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    chain_id: i64,
    now: DateTime<Utc>,
) -> Result<(), LandMarketError> {
    let mainnet = ChainNet::Ethereum.chain_id();
    if chain_id != mainnet {
        return Err(LandMarketError::Unready(
            Source::NftIndex,
            format!(
                "the nft index walks Ethereum mainnet (chain {mainnet}) only, and this node's LAND index is on chain {chain_id}"
            ),
        ));
    }
    let published: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM shop.chain_cursor c \
         WHERE c.indexer = $2 AND c.network = 'ethereum' AND {})",
        chain_walk::ready_sql("c", "$1")
    )))
    .bind(now.naive_utc())
    .bind(INDEXER_NFT)
    .fetch_one(&mut *conn)
    .await?;
    if !published {
        return Err(LandMarketError::Unready(
            Source::NftIndex,
            "the nft index has not published a recent fold on Ethereum (LAND and Estate orders and names)"
                .into(),
        ));
    }
    match super::trades::readiness_on(state, conn, now).await {
        Ok(_) => {}
        Err(super::trades::NotReady::Refused(why)) => {
            return Err(LandMarketError::Unready(Source::TradesMirror, why));
        }
        Err(super::trades::NotReady::Database(e)) => return Err(e.into()),
    }
    let stale = super::trade_events::stale_networks_on(conn, &[ChainNet::Ethereum], now).await?;
    if !stale.is_empty() {
        return Err(LandMarketError::Unready(
            Source::TradeEventsIndex,
            super::trade_events::stale_message(&stale),
        ));
    }
    Ok(())
}

/// The rows of [`land_market`], on `conn`, once `check_ready` passed there.
async fn read(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    chain_id: i64,
    land: &str,
    estate: &str,
    now: DateTime<Utc>,
) -> Result<LandMarket, LandMarketError> {
    let contracts = vec![land.to_ascii_lowercase(), estate.to_ascii_lowercase()];
    let naive_now = now.naive_utc();
    let orders = sqlx::query(sqlx::AssertSqlSafe(orders_sql()))
        .bind(&contracts)
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(|r| LandOrder {
            contract_address: r.get("contract_address"),
            token_id: r.get("token_id"),
            open: r.get("open"),
            seller: r.get("seller"),
            price: r.get("price"),
            expires_at_ms: r.get("expires_at"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();
    let data = sqlx::query(sqlx::AssertSqlSafe(data_sql()))
        .bind(&contracts)
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(|r| LandData {
            contract_address: r.get("token_address"),
            token_id: r.get("token_id"),
            name: r.get("name"),
            updated_at: r.get::<Option<i64>, _>("block_time").filter(|t| *t > 0),
        })
        .collect();
    let trades = match super::listings::mana_token(chain_id) {
        Some(mana) => {
            sqlx::query(sqlx::AssertSqlSafe(trades_sql()))
                .bind(naive_now)
                .bind(super::trades::mirrored_since(&state.settings, naive_now))
                .bind(&contracts)
                .bind(mana)
                .fetch_all(&mut *conn)
                .await?
        }
        // No MANA on this chain: no trade can be priced in it.
        None => Vec::new(),
    }
    .iter()
    .map(|r| LandTrade {
        id: r.get("id"),
        contract_address: r.get("contract_address_sent"),
        token_id: r.get("token_id"),
        signer: r.get("signer"),
        price: r.get("price"),
        created_at_ms: r.get("created_at_ms"),
    })
    .collect();
    let rentals = match rentals_contract(chain_id as u64) {
        Some(venue) => sqlx::query(sqlx::AssertSqlSafe(rentals_sql("$1", "$2", "$3", "$4")))
            .bind(&contracts)
            .bind(naive_now)
            .bind(chain_id as i32)
            .bind(venue)
            .fetch_all(&mut *conn)
            .await?
            .iter()
            .map(|r| {
                let periods: Option<serde_json::Value> = r.get("periods");
                LandRental {
                    contract_address: r.get("contract_address"),
                    token_id: r.get("token_id"),
                    expiration_ms: r
                        .get::<NaiveDateTime, _>("expiration")
                        .and_utc()
                        .timestamp_millis(),
                    updated_at_ms: r
                        .get::<NaiveDateTime, _>("updated_at")
                        .and_utc()
                        .timestamp_millis(),
                    periods: periods
                        .as_ref()
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|p| {
                            Some((
                                p.get(0)?.as_i64()?,
                                p.get(1)?.as_i64()?,
                                p.get(2)?.as_str()?.to_string(),
                            ))
                        })
                        .collect(),
                }
            })
            .collect(),
        // No Rentals contract on this chain: no listing can be signed for it.
        None => Vec::new(),
    };
    Ok(LandMarket {
        orders,
        trades,
        rentals,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Settings;
    use crate::market::tests::support::OwnDb;

    const LAND: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
    const ESTATE: &str = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
    const SEPOLIA: i64 = crate::types::ChainId::SEPOLIA.0 as i64;

    fn addr(c: char) -> String {
        format!("0x{}", c.to_string().repeat(40))
    }

    /// A REPEATABLE READ, READ ONLY snapshot, as the map build opens one.
    async fn snapshot(pool: &sqlx::PgPool) -> sqlx::Transaction<'static, sqlx::Postgres> {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx
    }

    /// One build's read: its own snapshot and clock (a cursor published
    /// after the clock would look published in the future).
    async fn market(
        state: &MarketplaceState,
        chain_id: i64,
    ) -> Result<LandMarket, LandMarketError> {
        let mut tx = snapshot(&state.pool).await;
        land_market(state, &mut tx, chain_id, LAND, ESTATE, Utc::now()).await
    }

    fn unready(r: Result<LandMarket, LandMarketError>) -> Source {
        match r {
            Err(LandMarketError::Unready(source, _)) => source,
            other => panic!("expected a source not ready, got {other:?}"),
        }
    }

    async fn publish_nft(pool: &sqlx::PgPool) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ('nft', 'ethereum', 1, 100, 100, 100, timezone('UTC', now()), true, 1)",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_orders(pool: &sqlx::PgPool) {
        sqlx::query(
            "INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) VALUES \
             ('ethereum', $1, 7, '0x01', $3, 'open', $4, 1500000000000000000000, 4102444800000, 1700000100, 1700000100, 50, 0), \
             ('ethereum', $2, 5, '0x02', $3, 'cancelled', $5, 9000000000000000000, 4102444800000, 1700000000, 1700000200, 51, 0), \
             ('ethereum', $6, 5, '0x03', $3, 'open', $5, 1, 4102444800000, 1700000000, 1700000200, 52, 0), \
             ('matic', $2, 6, '0x04', $3, 'open', $5, 1, 4102444800000, 1700000000, 1700000200, 53, 0)",
        )
        .bind(ESTATE)
        .bind(LAND)
        .bind(addr('7'))
        .bind(addr('b'))
        .bind(addr('a'))
        .bind(addr('6'))
        .execute(pool)
        .await
        .unwrap();
    }

    /// An open public_nft_order selling `token` of `contract` for `mana`.
    async fn trade(
        pool: &sqlx::PgPool,
        tag: &str,
        contract: &str,
        token: &str,
        signer: &str,
        mana: u64,
        expires_in: &str,
    ) {
        let mana_token = super::super::listings::mana_token(1).unwrap();
        priced_trade(
            pool,
            tag,
            contract,
            token,
            signer,
            mana,
            expires_in,
            (1, mana_token),
        )
        .await;
    }

    /// [`trade`] received as `(asset type, token contract)`: (1, the chain's
    /// MANA) is a MANA price; another ERC-20, or type 2 (USD-pegged MANA),
    /// is not.
    #[allow(clippy::too_many_arguments)]
    async fn priced_trade(
        pool: &sqlx::PgPool,
        tag: &str,
        contract: &str,
        token: &str,
        signer: &str,
        mana: u64,
        expires_in: &str,
        (received, received_token): (i16, &str),
    ) {
        let id: uuid::Uuid = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.trades (network, chain_id, signature, hashed_signature, checks, signer, type, expires_at, effective_since, contract) \
             VALUES ('ETHEREUM', 1, $1, $2, '{{\"uses\": 1}}', $3, 'public_nft_order', \
                     timezone('UTC', now()) + interval '{expires_in}', timezone('UTC', now()) - interval '1 day', $4) RETURNING id"
        )))
        .bind(format!("sig-{tag}"))
        .bind(format!("hash-{tag}"))
        .bind(signer)
        .bind(addr('9'))
        .fetch_one(pool)
        .await
        .unwrap();
        let sent: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO shop.trade_assets (trade_id, direction, asset_type, contract_address) VALUES ($1, 'sent', 3, $2) RETURNING id",
        )
        .bind(id)
        .bind(contract)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO shop.trade_assets_erc721 (asset_id, token_id) VALUES ($1, $2)")
            .bind(sent)
            .bind(token)
            .execute(pool)
            .await
            .unwrap();
        let received: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO shop.trade_assets (trade_id, direction, asset_type, contract_address, beneficiary) VALUES ($1, 'received', $4, $2, $3) RETURNING id",
        )
        .bind(id)
        .bind(received_token)
        .bind(signer)
        .bind(received)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO shop.trade_assets_erc20 (asset_id, amount) VALUES ($1, $2::numeric * 1000000000000000000)")
            .bind(received)
            .bind(mana as i64)
            .execute(pool)
            .await
            .unwrap();
        crate::test_db::open_trade_verdict(pool, &id.to_string()).await;
        if [LAND, ESTATE].contains(&contract) {
            crate::test_db::publish_land_owner(pool, contract, token, signer).await;
        }
    }

    /// An open rental listing of `token` of `contract`, signed for `chain_id`
    /// and the Rentals contract `venue`, expiring in `expires_in`.
    async fn rental(
        pool: &sqlx::PgPool,
        contract: &str,
        token: i64,
        chain_id: i32,
        venue: &str,
        expires_in: &str,
    ) {
        let nft_id = format!("{contract}-{token}");
        sqlx::query(
            "INSERT INTO signatures.rental_assets (contract_address, token_id, nft_id, network, chain_id, category, search_text, nft_created_at, nft_updated_at, fetched_at) \
             VALUES ($1, $2, $3, 'ethereum', $4, 'estate', '', timezone('UTC', now()), timezone('UTC', now()), timezone('UTC', now()))",
        )
        .bind(contract)
        .bind(token)
        .bind(&nft_id)
        .bind(chain_id)
        .execute(pool)
        .await
        .unwrap();
        let id: uuid::Uuid = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "INSERT INTO signatures.rentals (contract_address, token_id, network, chain_id, expiration, nonces, signature, rental_contract_address) \
             VALUES ($1, $2, 'ethereum', $3, timezone('UTC', now()) + interval '{expires_in}', ARRAY['0','0','0'], $4, $5) RETURNING id"
        )))
        .bind(contract)
        .bind(token)
        .bind(chain_id)
        .bind(format!("rental-{nft_id}"))
        .bind(venue)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO signatures.rental_listings (id, lessor) VALUES ($1, $2)")
            .bind(id)
            .bind(addr('b'))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO signatures.rental_periods (rental_id, min_days, max_days, price_per_day) VALUES \
             ($1, 30, 30, 10), ($1, 1, 7, 500000000000000000000)",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }

    fn state_over(pool: &sqlx::PgPool, settings: Settings) -> MarketplaceState {
        MarketplaceState::new(pool.clone(), crate::http::Outbound::for_tests(), settings)
    }

    fn local_trades() -> Settings {
        Settings {
            data_writes_enabled: true,
            ..Settings::for_tests()
        }
    }

    #[tokio::test]
    async fn land_market_waits_for_its_sources_then_reads_orders_trades_names_and_rentals() {
        let Some(db) = OwnDb::create("land_map").await else {
            return;
        };
        let state = state_over(&db.pool, local_trades());

        // The nft index has not published: no answer.
        assert_eq!(unready(market(&state, 1).await), Source::NftIndex);

        // Published on mainnet; a LAND index of another chain is refused
        // rather than read against it.
        publish_nft(&db.pool).await;
        assert_eq!(unready(market(&state, SEPOLIA).await), Source::NftIndex);

        // shop.trades has no source on a node that takes no trades.
        let closed = state_over(&db.pool, Settings::for_tests());
        assert_eq!(unready(market(&closed, 1).await), Source::TradesMirror);

        // Orders (any status, any registry token), names with their log's time.
        insert_orders(&db.pool).await;
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, detail) \
             VALUES ('ethereum', 60, 3, 1700000900, 'land_data', $1, $1, 5, '0,\"Corner\",\"\"')",
        )
        .bind(LAND)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.land_data (token_address, token_id, name, description, updated_block, updated_log) VALUES \
             ($1, 5, 'Corner', NULL, 60, 3), ($2, 7, 'Seven', NULL, 61, 0)",
        )
        .bind(LAND)
        .bind(ESTATE)
        .execute(&db.pool)
        .await
        .unwrap();
        // Two open trades of the registries, one expired, one of another contract.
        trade(&db.pool, "estate", ESTATE, "7", &addr('b'), 1200, "1 day").await;
        trade(&db.pool, "land", LAND, "5", &addr('a'), 42, "1 day").await;
        trade(&db.pool, "expired", LAND, "5", &addr('a'), 41, "-1 hour").await;
        trade(&db.pool, "other", &addr('6'), "5", &addr('a'), 40, "1 day").await;
        // LAND trades priced in another ERC-20 and in USD-pegged MANA: not
        // MANA prices.
        let mana_token = super::super::listings::mana_token(1).unwrap();
        priced_trade(
            &db.pool,
            "other-token",
            LAND,
            "6",
            &addr('a'),
            38,
            "1 day",
            (1, &addr('8')),
        )
        .await;
        priced_trade(
            &db.pool,
            "usd",
            LAND,
            "6",
            &addr('a'),
            39,
            "1 day",
            (2, mana_token),
        )
        .await;
        // Rentals: the estate's open mainnet listing; an expired one; one of
        // the same LAND contract signed for Sepolia; one naming another
        // Rentals contract on mainnet.
        let mainnet_venue = rentals_contract(1).unwrap();
        rental(&db.pool, ESTATE, 7, 1, mainnet_venue, "1 day").await;
        rental(&db.pool, LAND, 6, 1, mainnet_venue, "-1 day").await;
        rental(
            &db.pool,
            LAND,
            5,
            SEPOLIA as i32,
            rentals_contract(SEPOLIA as u64).unwrap(),
            "1 day",
        )
        .await;
        rental(&db.pool, LAND, 8, 1, &addr('4'), "1 day").await;

        // The trades' chain verdicts are not current on Ethereum: no
        // answer, rather than sold or cancelled trades read as open. The
        // map reads Ethereum only, so Polygon's cursor is not asked for.
        assert_eq!(unready(market(&state, 1).await), Source::TradeEventsIndex);
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ('trade_events', 'ethereum', 1, 10, 10, 10, timezone('UTC', now()), true, 1)",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        let m = market(&state, 1).await.unwrap();
        let mut orders = m.orders.clone();
        orders.sort_by(|a, b| a.token_id.cmp(&b.token_id));
        assert_eq!(
            orders.len(),
            2,
            "both registries' Ethereum rows, whatever their status"
        );
        assert_eq!(
            (
                orders[0].contract_address.as_str(),
                orders[0].token_id.as_str(),
                orders[0].open,
                orders[0].updated_at
            ),
            (LAND, "5", false, 1_700_000_200)
        );
        assert_eq!(
            (
                orders[1].token_id.as_str(),
                orders[1].open,
                orders[1].seller.as_str(),
                orders[1].price.as_str()
            ),
            ("7", true, addr('b').as_str(), "1500000000000000000000")
        );
        assert_eq!(orders[1].expires_at_ms, 4_102_444_800_000);
        let mut data = m.data.clone();
        data.sort_by(|a, b| a.token_id.cmp(&b.token_id));
        assert_eq!(
            data,
            vec![
                LandData {
                    contract_address: LAND.into(),
                    token_id: "5".into(),
                    name: Some("Corner".into()),
                    updated_at: Some(1_700_000_900),
                },
                LandData {
                    contract_address: ESTATE.into(),
                    token_id: "7".into(),
                    name: Some("Seven".into()),
                    updated_at: None,
                },
            ]
        );
        let mut trades: Vec<(String, String, String, String)> = m
            .trades
            .iter()
            .map(|t| {
                (
                    t.contract_address.clone(),
                    t.token_id.clone(),
                    t.signer.clone(),
                    t.price.clone(),
                )
            })
            .collect();
        trades.sort();
        assert_eq!(
            trades,
            vec![
                (
                    ESTATE.into(),
                    "7".into(),
                    addr('b'),
                    "1200000000000000000000".into()
                ),
                (
                    LAND.into(),
                    "5".into(),
                    addr('a'),
                    "42000000000000000000".into()
                ),
            ],
            "the open MANA trades of the two registries only"
        );
        assert!(m.trades.iter().all(|t| t.created_at_ms > 0));
        assert_eq!(
            m.rentals
                .iter()
                .map(|r| (r.contract_address.as_str(), r.token_id.as_str()))
                .collect::<Vec<_>>(),
            [(ESTATE, "7")],
            "the expired, Sepolia and other-contract listings are left out"
        );
        let rental = &m.rentals[0];
        assert_eq!(
            rental.periods,
            vec![
                (1, 7, "500000000000000000000".to_string()),
                (30, 30, "10".to_string())
            ],
            "by min_days"
        );
        assert!(rental.expiration_ms > Utc::now().timestamp_millis());

        // A sold trade closes as soon as the trade-events index folds it.
        let estate_trade: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM shop.trades WHERE signature = 'sig-estate'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO shop.trade_chain_state (trade_id, network, executions, status, folded_block) VALUES ($1, 'ethereum', 1, 'sold', 10)
             ON CONFLICT (trade_id) DO UPDATE SET status='sold', executions=1, folded_block=10",
        )
        .bind(estate_trade)
        .execute(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            market(&state, 1)
                .await
                .unwrap()
                .trades
                .iter()
                .map(|t| t.token_id.clone())
                .collect::<Vec<_>>(),
            ["5"],
            "the sold estate trade is closed"
        );
        let before_transfer = marks_at(&state, Utc::now()).await;
        sqlx::query(
            "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) \
             VALUES (101, 0, 1700000900, $1, 'transfer', 5, $2)",
        )
        .bind(LAND)
        .bind(addr('c'))
        .execute(&db.pool)
        .await
        .unwrap();
        assert!(
            market(&state, 1).await.unwrap().trades.is_empty(),
            "a transfer ahead of the fold hides the stale LAND owner's listing"
        );
        assert_ne!(before_transfer, marks_at(&state, Utc::now()).await);
        // A stale publish is not trusted: no answer, never the sold trade
        // read as open again.
        sqlx::query("UPDATE shop.chain_cursor SET folded_at = folded_at - interval '31 minutes' WHERE indexer = 'trade_events'")
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(unready(market(&state, 1).await), Source::TradeEventsIndex);
        db.drop().await;
    }

    async fn marks_at(state: &MarketplaceState, now: DateTime<Utc>) -> LandMarketMarks {
        let mut tx = snapshot(&state.pool).await;
        land_market_marks(state, &mut tx, 1, LAND, ESTATE, now)
            .await
            .unwrap()
    }

    /// The marks move with every row [`land_market`] reads for the two
    /// registries (an expiry at `now` included) and with readiness, and stay
    /// put for another contract.
    #[tokio::test]
    async fn the_marks_move_with_every_input_and_not_for_another_contract() {
        let Some(db) = OwnDb::create("land_map_marks").await else {
            return;
        };
        let state = state_over(&db.pool, local_trades());
        let now = Utc::now();
        let unpublished = marks_at(&state, now).await;
        assert!(!unpublished.ready, "the nft index has not published");
        let mut tx = snapshot(&db.pool).await;
        let sepolia = land_market_marks(&state, &mut tx, SEPOLIA, LAND, ESTATE, now)
            .await
            .unwrap();
        assert!(!sepolia.ready, "only mainnet is read");
        drop(tx);

        publish_nft(&db.pool).await;
        let nft_only = marks_at(&state, Utc::now()).await;
        assert!(
            !nft_only.ready,
            "the trade-events verdicts are not published yet, so the build would not run"
        );
        assert_ne!(nft_only.marks, unpublished.marks, "publication moves them");
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ('trade_events', 'ethereum', 1, 10, 10, 10, timezone('UTC', now()), true, 1)",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        let mut last = marks_at(&state, Utc::now()).await;
        assert!(last.ready, "every source the build needs is published");
        assert_ne!(last.marks, nft_only.marks, "and so does the verdicts'");
        // Each step must move the marks.
        fn step(last: &mut LandMarketMarks, what: &str, next: LandMarketMarks) {
            assert_ne!(next.marks, last.marks, "{what} moves the marks");
            *last = next;
        }

        insert_orders(&db.pool).await;
        step(&mut last, "an order", marks_at(&state, Utc::now()).await);
        // An order of another contract.
        sqlx::query(
            "INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) \
             VALUES ('ethereum', $1, 9, '0x09', $1, 'open', $1, 1, 4102444800000, 1, 1, 999, 0)",
        )
        .bind(addr('3'))
        .execute(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            marks_at(&state, Utc::now()).await.marks,
            last.marks,
            "another contract's order does not"
        );
        sqlx::query(
            "INSERT INTO shop.land_data (token_address, token_id, name, description, updated_block, updated_log) VALUES ($1, 5, 'Corner', NULL, 60, 3)",
        )
        .bind(LAND)
        .execute(&db.pool)
        .await
        .unwrap();
        step(&mut last, "a name", marks_at(&state, Utc::now()).await);
        trade(&db.pool, "land", LAND, "5", &addr('a'), 42, "1 hour").await;
        step(&mut last, "a trade", marks_at(&state, Utc::now()).await);
        trade(&db.pool, "other", &addr('3'), "5", &addr('a'), 42, "1 day").await;
        assert_eq!(
            marks_at(&state, Utc::now()).await.marks,
            last.marks,
            "another contract's trade does not"
        );
        sqlx::query(
            "INSERT INTO shop.trade_cancellations (target_hashed_signature, kind, signer, signature) VALUES ('hash-land', 'signer', $1, 'sig')",
        )
        .bind(addr('a'))
        .execute(&db.pool)
        .await
        .unwrap();
        step(
            &mut last,
            "a cancellation",
            marks_at(&state, Utc::now()).await,
        );
        trade(&db.pool, "estate", ESTATE, "7", &addr('b'), 1200, "1 day").await;
        step(
            &mut last,
            "another trade",
            marks_at(&state, Utc::now()).await,
        );
        let estate_trade: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM shop.trades WHERE signature = 'sig-estate'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO shop.trade_chain_state (trade_id, network, executions, status, folded_block) VALUES ($1, 'ethereum', 1, 'sold', 10)
             ON CONFLICT (trade_id) DO UPDATE SET status='sold', executions=1, folded_block=10",
        )
        .bind(estate_trade)
        .execute(&db.pool)
        .await
        .unwrap();
        step(
            &mut last,
            "a folded sale verdict",
            marks_at(&state, Utc::now()).await,
        );
        rental(
            &db.pool,
            ESTATE,
            7,
            1,
            rentals_contract(1).unwrap(),
            "2 hours",
        )
        .await;
        step(
            &mut last,
            "a rental listing",
            marks_at(&state, Utc::now()).await,
        );

        // What a count, a newest time and a sum of times would miss.
        let same_created_at = |tag: &'static str, like: &'static str| {
            let pool = db.pool.clone();
            async move {
                sqlx::query(
                    "UPDATE shop.trades SET created_at = (SELECT created_at FROM shop.trades WHERE signature = $2) \
                     WHERE signature = $1",
                )
                .bind(format!("sig-{tag}"))
                .bind(format!("sig-{like}"))
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        trade(&db.pool, "swap-a", LAND, "5", &addr('a'), 50, "1 day").await;
        step(&mut last, "a trade", marks_at(&state, Utc::now()).await);
        trade(&db.pool, "swap-b", LAND, "5", &addr('a'), 60, "1 day").await;
        same_created_at("swap-b", "swap-a").await;
        sqlx::query(
            "INSERT INTO shop.trade_cancellations (target_hashed_signature, kind, signer, signature) VALUES ('hash-swap-a', 'signer', $1, 'sig')",
        )
        .bind(addr('a'))
        .execute(&db.pool)
        .await
        .unwrap();
        step(
            &mut last,
            "a trade replaced by another created at the same time",
            marks_at(&state, Utc::now()).await,
        );
        trade(&db.pool, "verdict-a", ESTATE, "7", &addr('b'), 70, "1 day").await;
        trade(&db.pool, "verdict-b", ESTATE, "7", &addr('b'), 80, "1 day").await;
        same_created_at("verdict-b", "verdict-a").await;
        let trade_id = |tag: &'static str| {
            let pool = db.pool.clone();
            async move {
                sqlx::query_scalar::<_, uuid::Uuid>(
                    "SELECT id FROM shop.trades WHERE signature = $1",
                )
                .bind(format!("sig-{tag}"))
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let (verdict_a, verdict_b) = (trade_id("verdict-a").await, trade_id("verdict-b").await);
        let sold = "INSERT INTO shop.trade_chain_state (trade_id, network, executions, status, folded_block) VALUES ($1, 'ethereum', 1, 'sold', 10)
             ON CONFLICT (trade_id) DO UPDATE SET status='sold', executions=1, folded_block=10";
        sqlx::query(sold)
            .bind(verdict_a)
            .execute(&db.pool)
            .await
            .unwrap();
        step(
            &mut last,
            "two trades, one sold on chain",
            marks_at(&state, Utc::now()).await,
        );
        crate::test_db::open_trade_verdict(&db.pool, &verdict_a.to_string()).await;
        sqlx::query(sold)
            .bind(verdict_b)
            .execute(&db.pool)
            .await
            .unwrap();
        step(
            &mut last,
            "the verdict moved to the other trade",
            marks_at(&state, Utc::now()).await,
        );
        sqlx::query("UPDATE signatures.rental_periods SET price_per_day = price_per_day + 1 WHERE min_days = 30")
            .execute(&db.pool)
            .await
            .unwrap();
        step(
            &mut last,
            "a rental period repriced in place",
            marks_at(&state, Utc::now()).await,
        );
        // The nft index rewrites rows in place only in a reorg rewind or a
        // rebuild, each counted in the cursor by its own statement.
        sqlx::query(
            "UPDATE shop.chain_cursor SET reorg_rewinds = reorg_rewinds + 1 WHERE indexer = 'nft'",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        step(
            &mut last,
            "a reorg rewind of the nft index",
            marks_at(&state, Utc::now()).await,
        );
        sqlx::query("UPDATE shop.chain_cursor SET rebuilds = rebuilds + 1 WHERE indexer = 'nft'")
            .execute(&db.pool)
            .await
            .unwrap();
        step(
            &mut last,
            "a rebuild of the nft index",
            marks_at(&state, Utc::now()).await,
        );

        // Expiries, read at a later clock (the cursors published then, so
        // both reads are ready): the order (2100) outlives it, the rental
        // (2 hours) does not.
        let at_now = marks_at(&state, Utc::now()).await;
        let later = Utc::now() + chrono::Duration::hours(3);
        sqlx::query("UPDATE shop.chain_cursor SET folded_at = $1::timestamp WHERE indexer IN ('nft', 'trade_events')")
            .bind(later.naive_utc())
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE shop.trades SET expires_at = expires_at + interval '1 day' WHERE signature = 'sig-land'")
            .execute(&db.pool)
            .await
            .unwrap();
        let at_later = marks_at(&state, later).await;
        assert!(at_now.ready && at_later.ready);
        assert_ne!(
            at_later.marks, at_now.marks,
            "an expiry at the clock moves them"
        );

        // An order expires on the build's rounded second: live until half a
        // second before its expiry second, as `build::compose` reads it.
        let expiry_s = later.timestamp() + 3600;
        sqlx::query(
            "INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) \
             VALUES ('ethereum', $1, 77, '0x77', $1, 'open', $1, 1, $2, 1, 1, 998, 0)",
        )
        .bind(LAND)
        .bind(expiry_s * 1000)
        .execute(&db.pool)
        .await
        .unwrap();
        let at_ms = |ms: i64| marks_at(&state, DateTime::from_timestamp_millis(ms).unwrap());
        let (before, half, at_expiry) = (
            at_ms(expiry_s * 1000 - 501).await,
            at_ms(expiry_s * 1000 - 500).await,
            at_ms(expiry_s * 1000).await,
        );
        assert_ne!(
            before.marks, half.marks,
            "the half second before its expiry drops it"
        );
        assert_eq!(
            half.marks, at_expiry.marks,
            "and nothing moves at the raw expiry"
        );

        // The map's build job reads them in snapshots of its own and in the
        // build's, and compares the two: equal rows at an equal clock read
        // the same on any connection.
        let mut pooled = db.pool.acquire().await.unwrap();
        let outside = land_market_marks(&state, &mut pooled, 1, LAND, ESTATE, later)
            .await
            .unwrap();
        drop(pooled);
        assert_eq!(outside, marks_at(&state, later).await);

        // Readiness withdrawn.
        sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) WHERE indexer IN ('nft', 'trade_events')")
            .execute(&db.pool)
            .await
            .unwrap();
        let published = marks_at(&state, Utc::now()).await;
        assert!(published.ready);
        // Stale trade-events verdicts withdraw it too, as they withdraw the
        // build (`check_ready`).
        sqlx::query("UPDATE shop.chain_cursor SET folded_at = folded_at - interval '31 minutes' WHERE indexer = 'trade_events'")
            .execute(&db.pool)
            .await
            .unwrap();
        let stale = marks_at(&state, Utc::now()).await;
        assert!(!stale.ready, "stale verdicts are not ready");
        assert_ne!(stale.marks, published.marks);
        sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) WHERE indexer = 'trade_events'")
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(marks_at(&state, Utc::now()).await.ready);
        sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'nft'")
            .execute(&db.pool)
            .await
            .unwrap();
        let withdrawn = marks_at(&state, Utc::now()).await;
        assert!(!withdrawn.ready);
        assert_ne!(withdrawn.marks, published.marks);
        db.drop().await;
    }

    /// A rewind committed after the snapshot passed readiness (the cursor's
    /// publication withdrawn, the orders deleted) is invisible to that
    /// snapshot's reads, which return the coherent earlier state; the next
    /// snapshot sees the withdrawal and answers not ready.
    #[tokio::test]
    async fn a_withdrawal_committed_after_readiness_does_not_reach_the_snapshot_reads() {
        let Some(db) = OwnDb::create("land_map_window").await else {
            return;
        };
        let state = state_over(&db.pool, local_trades());
        publish_nft(&db.pool).await;
        crate::market::trade_events::tests::publish_cursors(&db.pool).await;
        insert_orders(&db.pool).await;

        let mut tx = snapshot(&db.pool).await;
        check_ready(&state, &mut tx, 1, Utc::now()).await.unwrap();
        sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'nft'")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.nft_order")
            .execute(&db.pool)
            .await
            .unwrap();
        let held = read(&state, &mut tx, 1, LAND, ESTATE, Utc::now())
            .await
            .unwrap();
        assert_eq!(
            held.orders.len(),
            2,
            "the snapshot keeps the rows readiness saw"
        );
        drop(tx);

        assert_eq!(unready(market(&state, 1).await), Source::NftIndex);
        db.drop().await;
    }
}
