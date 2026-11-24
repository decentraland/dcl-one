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
//! Readiness, the stage-5 rule: the nft index must be published on Ethereum
//! (it folds the orders and the names) and `shop.trades` readable
//! ([`super::trades::readiness`]); otherwise [`LandMarketError::Unready`]
//! names the source and the grid is not built. The trades' chain verdicts
//! (`shop.trade_chain_state`) close sold and cancelled listings while the
//! trade-events index is published on Ethereum, as they do for the Shop.

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::Row;

use super::chain_walk::{self, ChainNet, INDEXER_NFT};
use super::{MarketplaceState, Source};

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
/// addresses) at `now`, or the source that is not ready.
pub async fn land_market(
    state: &MarketplaceState,
    land: &str,
    estate: &str,
    now: DateTime<Utc>,
) -> Result<LandMarket, LandMarketError> {
    if chain_walk::ready_cursor(&state.pool, INDEXER_NFT, ChainNet::Ethereum, now)
        .await
        .is_none()
    {
        return Err(LandMarketError::Unready(
            Source::NftIndex,
            "the nft index has not published a recent fold on Ethereum (LAND and Estate orders and names)"
                .into(),
        ));
    }
    match super::trades::readiness(state).await {
        Ok(_) => {}
        Err(super::trades::NotReady::Refused(why)) => {
            return Err(LandMarketError::Unready(Source::TradesMirror, why));
        }
        Err(super::trades::NotReady::Database(e)) => return Err(e.into()),
    }
    let contracts = vec![land.to_ascii_lowercase(), estate.to_ascii_lowercase()];
    let naive_now = now.naive_utc();

    let orders = sqlx::query(ORDERS_SQL)
        .bind(&contracts)
        .fetch_all(&state.pool)
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
        .fetch_all(&state.pool)
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
        .fetch_all(&state.pool)
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
    let rentals = sqlx::query(RENTALS_SQL)
        .bind(&contracts)
        .bind(naive_now)
        .fetch_all(&state.pool)
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
        .collect();
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

    fn addr(c: char) -> String {
        format!("0x{}", c.to_string().repeat(40))
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

    #[tokio::test]
    async fn land_market_waits_for_its_sources_then_reads_orders_trades_names_and_rentals() {
        let Some(db) = OwnDb::create("land_map").await else {
            return;
        };
        let state = MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            Settings {
                data_writes_enabled: true,
                ..Settings::for_tests()
            },
        );
        // Each read takes its own clock: a cursor published after it would look published in the future.
        let now = Utc::now();
        let unready = |r: Result<LandMarket, LandMarketError>| match r {
            Err(LandMarketError::Unready(source, _)) => source,
            other => panic!("expected a source not ready, got {other:?}"),
        };

        // The nft index has not published: no answer.
        assert_eq!(
            unready(land_market(&state, LAND, ESTATE, Utc::now()).await),
            Source::NftIndex
        );

        // Published, but shop.trades has no source on a node that takes no trades.
        publish_nft(&db.pool).await;
        let closed = MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            Settings::for_tests(),
        );
        assert_eq!(
            unready(land_market(&closed, LAND, ESTATE, Utc::now()).await),
            Source::TradesMirror
        );

        // Orders (any status, any registry token), names with their log's time.
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
        .execute(&db.pool)
        .await
        .unwrap();
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
        // A rental listing of the estate, open, and an expired one of a parcel.
        for (contract, token, nft_id, expiration) in [
            (ESTATE, 7, format!("estate-{ESTATE}-7"), "1 day"),
            (LAND, 5, format!("parcel-{LAND}-5"), "-1 day"),
        ] {
            sqlx::query(
                "INSERT INTO signatures.rental_assets (contract_address, token_id, nft_id, network, chain_id, category, search_text, nft_created_at, nft_updated_at, fetched_at) \
                 VALUES ($1, $2, $3, 'ethereum', 1, 'estate', '', timezone('UTC', now()), timezone('UTC', now()), timezone('UTC', now()))",
            )
            .bind(contract)
            .bind(token)
            .bind(&nft_id)
            .execute(&db.pool)
            .await
            .unwrap();
            let id: uuid::Uuid = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "INSERT INTO signatures.rentals (contract_address, token_id, network, chain_id, expiration, nonces, signature, rental_contract_address) \
                 VALUES ($1, $2, 'ethereum', 1, timezone('UTC', now()) + interval '{expiration}', ARRAY['0','0','0'], $3, $4) RETURNING id"
            )))
            .bind(contract)
            .bind(token)
            .bind(format!("rental-{nft_id}"))
            .bind(addr('5'))
            .fetch_one(&db.pool)
            .await
            .unwrap();
            sqlx::query("INSERT INTO signatures.rental_listings (id, lessor) VALUES ($1, $2)")
                .bind(id)
                .bind(addr('b'))
                .execute(&db.pool)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO signatures.rental_periods (rental_id, min_days, max_days, price_per_day) VALUES \
                 ($1, 30, 30, 10), ($1, 1, 7, 500000000000000000000)",
            )
            .bind(id)
            .execute(&db.pool)
            .await
            .unwrap();
        }

        let m = land_market(&state, LAND, ESTATE, Utc::now()).await.unwrap();
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
        assert_eq!(m.rentals.len(), 1, "the expired listing is left out");
        let rental = &m.rentals[0];
        assert_eq!(
            (rental.contract_address.as_str(), rental.token_id.as_str()),
            (ESTATE, "7")
        );
        assert_eq!(
            rental.periods,
            vec![
                (1, 7, "500000000000000000000".to_string()),
                (30, 30, "10".to_string())
            ],
            "by min_days"
        );
        assert!(rental.expiration_ms > now.timestamp_millis());

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
        let before = land_market(&state, LAND, ESTATE, Utc::now()).await.unwrap();
        assert_eq!(
            before.trades.len(),
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
        let after = land_market(&state, LAND, ESTATE, Utc::now()).await.unwrap();
        assert_eq!(
            after
                .trades
                .iter()
                .map(|t| t.token_id.as_str())
                .collect::<Vec<_>>(),
            ["5"],
            "the sold estate trade is closed"
        );
        db.drop().await;
    }
}
