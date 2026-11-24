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
//! Readiness, the stage-5 rule ([`check_ready`]): the grid's chain must be
//! Ethereum mainnet (the nft index walks only it), the nft index published
//! on Ethereum (it folds the orders and the names; `chain_walk::ready_sql`,
//! the cursor bound to chain 1) and `shop.trades` readable
//! ([`super::trades::readiness_on`]); otherwise [`LandMarketError::Unready`]
//! names the source and the grid is not built. The trades' chain verdicts
//! (`shop.trade_chain_state`) close sold and cancelled listings while the
//! trade-events index is published on Ethereum, as they do for the Shop.
//! Rental listings are the ones signed for the grid's chain and its Rentals
//! contract.

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

const ORDERS_SQL: &str = "\
SELECT contract_address, token_id::text AS token_id, status = 'open' AS open, seller, price::text AS price,
       expires_at, created_at, updated_at
FROM shop.nft_order
WHERE network = 'ethereum' AND contract_address = ANY($1)";

const DATA_SQL: &str = "\
SELECT d.token_address, d.token_id::text AS token_id, d.name, e.block_time
FROM shop.land_data d
LEFT JOIN shop.nft_event e
  ON e.network = 'ethereum' AND e.block_number = d.updated_block AND e.log_index = d.updated_log
WHERE d.token_address = ANY($1)";

const RENTALS_SQL: &str = "\
SELECT r.contract_address, r.token_id::text AS token_id, r.expiration, r.updated_at,
       (SELECT jsonb_agg(jsonb_build_array(p.min_days, p.max_days, p.price_per_day::text) ORDER BY p.min_days, p.max_days)
          FROM signatures.rental_periods p WHERE p.rental_id = r.id) AS periods
FROM signatures.rentals r
WHERE r.status = 'open' AND r.contract_address = ANY($1) AND r.expiration >= $2
  AND r.chain_id = $3 AND lower(r.rental_contract_address) = $4
  AND EXISTS (SELECT 1 FROM signatures.rental_periods p WHERE p.rental_id = r.id)";

/// The open trades of the LAND and Estate registries on Ethereum: `$1`
/// now, `$2` the trades floor (`trades::mirrored_since`), `$3` the two
/// registries.
fn trades_sql() -> String {
    format!(
        "WITH {} SELECT u.id::text AS id, u.contract_address_sent, u.token_id, u.signer, \
           u.amount_received::text AS price, (EXTRACT(EPOCH FROM u.created_at) * 1000)::bigint AS created_at_ms \
         FROM unified_trades u \
         WHERE u.type = 'public_nft_order' AND u.token_id IS NOT NULL AND lower(u.network) = 'ethereum'",
        super::catalog::unified_trades_cte_of("$1", "$2", Some("$3"))
    )
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
pub(crate) async fn check_ready(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    chain_id: i64,
    now: DateTime<Utc>,
) -> Result<(), LandMarketError> {
    let mainnet = chain_walk::mainnet_chain(ChainNet::Ethereum);
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
    match super::trades::readiness_on(state, conn).await {
        Ok(_) => Ok(()),
        Err(super::trades::NotReady::Refused(why)) => {
            Err(LandMarketError::Unready(Source::TradesMirror, why))
        }
        Err(super::trades::NotReady::Database(e)) => Err(e.into()),
    }
}

/// The rows of [`land_market`], on `conn`, once [`check_ready`] passed there.
pub(crate) async fn read(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    chain_id: i64,
    land: &str,
    estate: &str,
    now: DateTime<Utc>,
) -> Result<LandMarket, LandMarketError> {
    let contracts = vec![land.to_ascii_lowercase(), estate.to_ascii_lowercase()];
    let naive_now = now.naive_utc();
    let orders = sqlx::query(ORDERS_SQL)
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
    let data = sqlx::query(DATA_SQL)
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
    let trades = sqlx::query(sqlx::AssertSqlSafe(trades_sql()))
        .bind(naive_now)
        .bind(super::trades::mirrored_since(&state.settings, naive_now))
        .bind(&contracts)
        .fetch_all(&mut *conn)
        .await?
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
        Some(venue) => sqlx::query(RENTALS_SQL)
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
    const SEPOLIA: i64 = 11_155_111;

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
            "INSERT INTO shop.trade_assets (trade_id, direction, asset_type, contract_address, beneficiary) VALUES ($1, 'received', 1, $2, $3) RETURNING id",
        )
        .bind(id)
        .bind(addr('8'))
        .bind(signer)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO shop.trade_assets_erc20 (asset_id, amount) VALUES ($1, $2::numeric * 1000000000000000000)")
            .bind(received)
            .bind(mana as i64)
            .execute(pool)
            .await
            .unwrap();
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
            "the open trades of the two registries only"
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

        // A sold trade closes once the trade-events index has published it.
        let estate_trade: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM shop.trades WHERE signature = 'sig-estate'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO shop.trade_chain_state (trade_id, network, executions, status, folded_block) VALUES ($1, 'ethereum', 1, 'sold', 10)",
        )
        .bind(estate_trade)
        .execute(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            market(&state, 1).await.unwrap().trades.len(),
            2,
            "an unpublished verdict is not read, as in the Shop"
        );
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ('trade_events', 'ethereum', 1, 10, 10, 10, timezone('UTC', now()), true, 1)",
        )
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
