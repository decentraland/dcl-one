//! The MANA spot rows (`economy.price_snapshots`) and the operator overrides
//! (`economy.price_overrides` + `price_override_audit`), schema
//! `schemas/31_economy.sql`. Every clock is a bound parameter; the timestamps
//! are `timestamp without time zone` holding UTC and come back as `DateTime<Utc>`.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};

pub const SOURCE_COINGECKO: &str = "coingecko";

/// How old the newest snapshot may be and still be a price: six default
/// poll intervals (`PRICE_POLL_INTERVAL_SECS` 300). Past it the price route
/// and the credit-priced catalog feeds answer the unavailable envelope
/// instead of serving a dead poller's last quote (the old market crate's
/// `max_staleness_secs` was 86,400; a checkout amount deserves less), and
/// `Settings::resolve` refuses a poll interval the budget cannot cover.
pub const SNAPSHOT_MAX_AGE: Duration = Duration::from_secs(6 * 300);

#[derive(Debug, Clone, PartialEq)]
pub struct PriceSnapshot {
    pub mana_usd: Option<f64>,
    pub mana_eth: Option<f64>,
    pub mana_btc: Option<f64>,
    pub market_cap_usd: Option<f64>,
    pub volume_24h_usd: Option<f64>,
    pub price_change_24h_pct: Option<f64>,
    pub source_updated_at: Option<DateTime<Utc>>,
    pub taken_at: DateTime<Utc>,
}

impl PriceSnapshot {
    /// Since the poller took it (`taken_at`: this node's own liveness, not
    /// the provider's quote time).
    pub fn age_at(&self, now: DateTime<Utc>) -> Duration {
        (now - self.taken_at).to_std().unwrap_or_default()
    }

    pub fn is_fresh_at(&self, now: DateTime<Utc>) -> bool {
        self.age_at(now) <= SNAPSHOT_MAX_AGE
    }
}

/// The column list shared by the poller's `RETURNING` and the read; a macro
/// so both stay `&'static str` literals for sqlx.
macro_rules! snapshot_columns {
    () => {
        "mana_usd::double precision AS mana_usd, \
         mana_eth::double precision AS mana_eth, \
         mana_btc::double precision AS mana_btc, \
         mana_market_cap_usd::double precision AS market_cap_usd, \
         mana_volume_24h_usd::double precision AS volume_24h_usd, \
         mana_price_change_24h_pct::double precision AS price_change_24h_pct, \
         source_updated_at, \
         taken_at"
    };
}
pub(crate) use snapshot_columns;

fn utc(ndt: NaiveDateTime) -> DateTime<Utc> {
    DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc)
}

pub fn snapshot_from_row(r: &PgRow) -> PriceSnapshot {
    PriceSnapshot {
        mana_usd: r.get("mana_usd"),
        mana_eth: r.get("mana_eth"),
        mana_btc: r.get("mana_btc"),
        market_cap_usd: r.get("market_cap_usd"),
        volume_24h_usd: r.get("volume_24h_usd"),
        price_change_24h_pct: r.get("price_change_24h_pct"),
        source_updated_at: r
            .get::<Option<NaiveDateTime>, _>("source_updated_at")
            .map(utc),
        taken_at: utc(r.get::<NaiveDateTime, _>("taken_at")),
    }
}

/// The newest row (CoinGecko or a fallback exchange), served from memory once the in-process poller
/// published one: `serve_live` is the poller switch, so a node without the
/// poller (an external ingester writing the table) reads the row every time.
#[derive(Clone)]
pub struct Prices {
    pool: PgPool,
    live: Arc<RwLock<Option<Arc<PriceSnapshot>>>>,
    serve_live: bool,
}

impl Prices {
    pub fn new(pool: PgPool, serve_live: bool) -> Self {
        Self {
            pool,
            live: Arc::new(RwLock::new(None)),
            serve_live,
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn publish(&self, snapshot: PriceSnapshot) {
        *self.live.write().unwrap() = Some(Arc::new(snapshot));
    }

    pub fn live(&self) -> Option<Arc<PriceSnapshot>> {
        self.live.read().unwrap().clone()
    }

    pub async fn latest(&self) -> Result<Option<Arc<PriceSnapshot>>, sqlx::Error> {
        if self.serve_live
            && let Some(snapshot) = self.live()
        {
            return Ok(Some(snapshot));
        }
        let row = self.latest_snapshot().await?.map(Arc::new);
        if self.serve_live
            && let Some(snapshot) = &row
        {
            let mut live = self.live.write().unwrap();
            if live.is_none() {
                *live = Some(snapshot.clone());
            }
        }
        Ok(row)
    }

    /// `price_snapshots_newest_idx`: the newest row of any source.
    pub async fn latest_snapshot(&self) -> Result<Option<PriceSnapshot>, sqlx::Error> {
        let row = sqlx::query(concat!(
            "SELECT ",
            snapshot_columns!(),
            " FROM economy.price_snapshots ORDER BY taken_at DESC LIMIT 1"
        ))
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(snapshot_from_row))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceOverride {
    pub token_id: String,
    pub vs_currency: String,
    /// The exact numeric, as text.
    pub value: String,
    pub note: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Overrides {
    pool: PgPool,
}

impl Overrides {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<PriceOverride>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT token_id, vs_currency, value::text AS value, note, updated_by, updated_at \
             FROM economy.price_overrides ORDER BY token_id, vs_currency",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_override).collect())
    }

    /// The overrides among `(token_id, vs_currency)` pairs the route asked for.
    pub async fn for_pairs(
        &self,
        token_ids: &[String],
        vs_currencies: &[String],
    ) -> Result<Vec<PriceOverride>, sqlx::Error> {
        if token_ids.is_empty() || vs_currencies.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT token_id, vs_currency, value::text AS value, note, updated_by, updated_at \
             FROM economy.price_overrides WHERE token_id = ANY($1) AND vs_currency = ANY($2)",
        )
        .bind(token_ids)
        .bind(vs_currencies)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_override).collect())
    }

    /// The upsert and its audit row in one statement.
    pub async fn set(
        &self,
        token_id: &str,
        vs_currency: &str,
        value: &str,
        note: Option<&str>,
        admin: &str,
        now: DateTime<Utc>,
    ) -> Result<PriceOverride, sqlx::Error> {
        let row = sqlx::query(
            "WITH o AS ( \
               INSERT INTO economy.price_overrides (token_id, vs_currency, value, note, updated_by, updated_at) \
               VALUES ($1, $2, $3::numeric, $4, $5, $7) \
               ON CONFLICT (token_id, vs_currency) DO UPDATE \
                 SET value = EXCLUDED.value, note = EXCLUDED.note, updated_by = EXCLUDED.updated_by, updated_at = EXCLUDED.updated_at \
               RETURNING token_id, vs_currency, value::text AS value, note, updated_by, updated_at \
             ), a AS ( \
               INSERT INTO economy.price_override_audit (action, token_id, vs_currency, value, note, admin, detail, created_at) \
               VALUES ('override.set', $1, $2, $3::numeric, $4, $5, $6, $7) \
             ) \
             SELECT token_id, vs_currency, value, note, updated_by, updated_at FROM o",
        )
        .bind(token_id)
        .bind(vs_currency)
        .bind(value)
        .bind(note)
        .bind(admin)
        .bind(serde_json::json!({ "token_id": token_id, "vs_currency": vs_currency, "value": value, "note": note }))
        .bind(now.naive_utc())
        .fetch_one(&self.pool)
        .await?;
        Ok(row_to_override(row))
    }

    /// The delete and, only when a row went, its audit row.
    pub async fn clear(
        &self,
        token_id: &str,
        vs_currency: &str,
        admin: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "WITH d AS ( \
               DELETE FROM economy.price_overrides WHERE token_id = $1 AND vs_currency = $2 RETURNING token_id \
             ), a AS ( \
               INSERT INTO economy.price_override_audit (action, token_id, vs_currency, value, note, admin, detail, created_at) \
               SELECT 'override.clear', $1, $2, NULL, NULL, $3, $4, $5 FROM d \
             ) \
             SELECT EXISTS (SELECT 1 FROM d)",
        )
        .bind(token_id)
        .bind(vs_currency)
        .bind(admin)
        .bind(serde_json::json!({ "token_id": token_id, "vs_currency": vs_currency }))
        .bind(now.naive_utc())
        .fetch_one(&self.pool)
        .await
    }
}

fn row_to_override(r: PgRow) -> PriceOverride {
    PriceOverride {
        token_id: r.get("token_id"),
        vs_currency: r.get("vs_currency"),
        value: r.get("value"),
        note: r.get("note"),
        updated_by: r.get("updated_by"),
        updated_at: utc(r.get::<NaiveDateTime, _>("updated_at")),
    }
}
