// from: credits-server@465e628bc:src/controllers/handlers/{authorize-usd-credit,authorize-usd-credit-batch,cancel-usd-intents,record-intent-submission}.ts (the reserve, retire and stamp rules), catalyrst/crates/catalyrst-credits/src/ports/authorize.rs (the wallet-locked budget)
//! The credit store: ONE ephemeral credit per settlement group
//! (`credits.authorizations` + `credits.authorization_lines`), reserved under
//! the wallet row lock with the outstanding pledged budget subtracted, so N
//! concurrent calls cannot each pledge the full balance; replayed by the
//! server-derived idempotency key (the same row, hence the same signature:
//! the digest is deterministic); retired by the client (`cancel`: the
//! pledge keeps counting, a signed credit stays spendable until it expires);
//! released or expired by the sweep once the CreditUsed index has read the
//! chain past the expiry; consumed by the settlement pass whatever its
//! status. The budget a wallet may pledge is `available` at the peg minus
//! `SUM(usd_cents)` of its `authorized` and `retired` rows.

use chrono::NaiveDateTime;
use sqlx::Row;

use super::errors::ApiError;
use super::store::{Credits, now};
use crate::types::{ManaUsdRate, ManaWei, UsdCents};

/// Upstream's live-intent ceiling per wallet: complete or cancel a purchase
/// before starting another once this many credits are outstanding.
pub const MAX_LIVE_CREDITS_PER_WALLET: i64 = 10;

/// The MANA a pledge of `cents` is signed for at `rate` with no buffer and
/// no headroom (the reference value of the credits; the batch path sizes a
/// credit through `CreditSizing` instead).
pub fn pledge_wei(cents: UsdCents, rate: ManaUsdRate) -> Option<ManaWei> {
    ManaWei::from_usd_cents(cents, rate.rate(), rate.decimals(), 0)
}

#[derive(Debug, Clone)]
pub struct AuthorizationRow {
    pub id: String,
    pub address: String,
    pub usd_cents: i64,
    pub amount_wei: String,
    pub status: String,
    pub expires_at: NaiveDateTime,
    pub chain_id: Option<i64>,
    pub contract: Option<String>,
    pub credit_hash: Option<String>,
    pub quoted_cents: Option<i64>,
    pub oracle_rate: Option<String>,
    pub oracle_decimals: Option<i16>,
    pub submitted_tx_hash: Option<String>,
}

/// One line of a group as the store writes it: the identity the payment
/// rule derived (`sale_kind`, `asset_kind`, the asset), the quote and the
/// charge, and the client's own naming of the line for its history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupLine {
    pub quoted_cents: i64,
    pub charged_cents: i64,
    pub sale_kind: &'static str,
    pub asset_kind: &'static str,
    pub asset_chain_id: i64,
    pub asset_contract: String,
    pub asset_item_id: Option<String>,
    pub asset_token_id: Option<String>,
    pub trade_id: Option<String>,
    pub contract_address: Option<String>,
    pub item_id: Option<String>,
    pub registered_name: Option<String>,
}

/// A group to reserve: everything the signed credit binds plus its lines.
pub struct NewGroup<'a> {
    /// The salt, `0x` + 64 lowercase hex.
    pub id: &'a str,
    pub address: &'a str,
    pub chain_id: i64,
    pub contract: &'a str,
    pub credit_hash: &'a str,
    pub quoted_cents: i64,
    pub charged_cents: i64,
    pub amount_wei: &'a str,
    pub oracle_rate: &'a str,
    pub oracle_decimals: i16,
    pub source: Option<&'a str>,
    pub expires_at: NaiveDateTime,
    pub lines: &'a [GroupLine],
}

/// The single-line reserve of the closure pass, kept for the tests and the
/// mock lanes: no chain, no manager, no lines.
pub struct NewAuthorization<'a> {
    pub id: &'a str,
    pub address: &'a str,
    pub usd_cents: i64,
    pub amount_wei: &'a str,
    pub trade_id: Option<&'a str>,
    pub contract_address: Option<&'a str>,
    pub item_id: Option<&'a str>,
    pub source: Option<&'a str>,
    pub expires_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ReserveOutcome {
    pub id: String,
    pub amount_wei: String,
    pub usd_cents: i64,
    pub expires_at: NaiveDateTime,
    pub replayed: bool,
}

/// Why a reserve did not insert (the wire distinguishes the two).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReserveRefusal {
    Insufficient {
        balance_cents: i64,
        required_cents: i64,
    },
    TooManyLive {
        balance_cents: i64,
    },
}

impl From<ReserveRefusal> for ApiError {
    fn from(r: ReserveRefusal) -> Self {
        match r {
            ReserveRefusal::Insufficient { .. } => {
                ApiError::payment_required("insufficient credit balance")
            }
            ReserveRefusal::TooManyLive { .. } => ApiError::conflict(
                "Too many purchases in progress. Complete or cancel them before starting another one.",
            ),
        }
    }
}

/// What a wallet can still pledge: the balance at the peg minus its live pledges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spendable {
    pub balance_cents: i64,
    pub held_cents: i64,
    pub live: i64,
}

impl Spendable {
    pub fn spendable_cents(self) -> i64 {
        (self.balance_cents - self.held_cents).max(0)
    }
}

/// The statuses that hold a pledge against the balance.
pub const LIVE_STATUSES: &str = "('authorized', 'retired')";

impl Credits {
    pub async fn reserve_authorization(
        &self,
        a: &NewAuthorization<'_>,
        idempotency_key: &str,
    ) -> Result<ReserveOutcome, ApiError> {
        self.reserve_single(a, a.amount_wei, idempotency_key).await
    }

    /// [`Self::reserve_authorization`] with the MANA amount [`pledge_wei`]
    /// derives from `a.usd_cents` at `mana_usd`; also returns the rate in wei-per-MANA.
    pub async fn reserve_authorization_priced(
        &self,
        a: &NewAuthorization<'_>,
        mana_usd: &str,
        idempotency_key: &str,
    ) -> Result<(ReserveOutcome, String), ApiError> {
        let rate = ManaUsdRate::parse_usd_per_mana(mana_usd).ok_or_else(|| {
            ApiError::Internal(format!(
                "oracle rate {mana_usd:?} is not a positive decimal"
            ))
        })?;
        let wei = pledge_wei(UsdCents::new(a.usd_cents), rate).ok_or_else(|| {
            ApiError::Internal(format!("no MANA value for {} cents", a.usd_cents))
        })?;
        let outcome = self
            .reserve_single(a, &wei.get().to_string(), idempotency_key)
            .await?;
        Ok((outcome, rate.rate().to_string()))
    }

    async fn reserve_single(
        &self,
        a: &NewAuthorization<'_>,
        amount_wei: &str,
        idempotency_key: &str,
    ) -> Result<ReserveOutcome, ApiError> {
        let group = NewGroup {
            id: a.id,
            address: a.address,
            chain_id: 0,
            contract: "",
            credit_hash: "",
            quoted_cents: a.usd_cents,
            charged_cents: a.usd_cents,
            amount_wei,
            oracle_rate: "",
            oracle_decimals: 0,
            source: a.source,
            expires_at: a.expires_at,
            lines: &[],
        };
        self.reserve_group(
            &group,
            idempotency_key,
            LegacyColumns {
                trade_id: a.trade_id,
                contract_address: a.contract_address,
                item_id: a.item_id,
            },
        )
        .await?
        .map_err(ApiError::from)
    }

    /// Reserves ONE credit for the group: BEGIN; lock the wallet row (the
    /// budget is `available` in cents at the peg, flooring); then, in that
    /// snapshot, the idempotency probe, the outstanding pledged sum, the
    /// live-credit count and the conditional INSERT as one statement; the
    /// lines follow in the same transaction. `Ok(Err(refusal))` is a refusal
    /// with the figures the wire answers.
    pub async fn reserve_credit_group(
        &self,
        g: &NewGroup<'_>,
        idempotency_key: &str,
    ) -> Result<Result<ReserveOutcome, ReserveRefusal>, ApiError> {
        let first = g.lines.first();
        self.reserve_group(
            g,
            idempotency_key,
            LegacyColumns {
                trade_id: first.and_then(|l| l.trade_id.as_deref()),
                contract_address: first.and_then(|l| l.contract_address.as_deref()),
                item_id: first.and_then(|l| l.item_id.as_deref()),
            },
        )
        .await
    }

    async fn reserve_group(
        &self,
        g: &NewGroup<'_>,
        idempotency_key: &str,
        legacy: LegacyColumns<'_>,
    ) -> Result<Result<ReserveOutcome, ReserveRefusal>, ApiError> {
        let mut tx = self.pool.begin().await?;

        let available_cents: Option<i64> =
            sqlx::query_scalar("SELECT floor(available * $2::numeric)::bigint FROM credits.wallets WHERE address = $1 FOR UPDATE")
                .bind(g.address)
                .bind(UsdCents::CENTS_PER_CREDIT)
                .fetch_optional(&mut *tx)
                .await?;
        let balance_cents = available_cents.unwrap_or(0);

        let row = sqlx::query(
            "WITH prior AS ( \
                 SELECT id, amount_wei::text AS amount_wei, usd_cents, expires_at \
                 FROM credits.authorizations WHERE idempotency_key = $10::text \
             ), live AS ( \
                 SELECT COALESCE(SUM(usd_cents), 0)::bigint AS held, count(*)::bigint AS n \
                 FROM credits.authorizations \
                 WHERE address = $2::text AND status IN ('authorized', 'retired') \
             ), ins AS ( \
                 INSERT INTO credits.authorizations \
                     (id, address, usd_cents, amount_wei, trade_id, contract_address, item_id, source, status, \
                      expires_at, idempotency_key, chain_id, contract, credit_hash, quoted_cents, oracle_rate, oracle_decimals) \
                 SELECT $1::text, $2::text, $3::bigint, $4::numeric, $5::text, $6::text, $7::text, $8::text, 'authorized', \
                        $9::timestamp, $10::text, NULLIF($12::bigint, 0), NULLIF($13::text, ''), NULLIF($14::text, ''), \
                        $15::bigint, NULLIF($16::text, '')::numeric, NULLIF($17::smallint, 0) \
                 FROM live l \
                 WHERE NOT EXISTS (SELECT 1 FROM prior) AND $3::bigint <= $11::bigint - l.held AND l.n < $18::bigint \
                 RETURNING id \
             ) \
             SELECT p.id AS prior_id, p.amount_wei AS prior_wei, p.usd_cents AS prior_cents, p.expires_at AS prior_expires, \
                    l.held, l.n, EXISTS (SELECT 1 FROM ins) AS inserted \
             FROM live l LEFT JOIN prior p ON TRUE",
        )
        .bind(g.id)
        .bind(g.address)
        .bind(g.charged_cents)
        .bind(g.amount_wei)
        .bind(legacy.trade_id)
        .bind(legacy.contract_address)
        .bind(legacy.item_id)
        .bind(g.source)
        .bind(g.expires_at)
        .bind(idempotency_key)
        .bind(balance_cents)
        .bind(g.chain_id)
        .bind(g.contract)
        .bind(g.credit_hash)
        .bind(g.quoted_cents)
        .bind(g.oracle_rate)
        .bind(g.oracle_decimals)
        .bind(MAX_LIVE_CREDITS_PER_WALLET)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(prior_id) = row.get::<Option<String>, _>("prior_id") {
            tx.commit().await?;
            return Ok(Ok(ReserveOutcome {
                id: prior_id,
                amount_wei: row.get("prior_wei"),
                usd_cents: row.get("prior_cents"),
                expires_at: row.get("prior_expires"),
                replayed: true,
            }));
        }
        if !row.get::<bool, _>("inserted") {
            tx.rollback().await?;
            let held: i64 = row.get("held");
            let live: i64 = row.get("n");
            let spendable = (balance_cents - held).max(0);
            return Ok(Err(if live >= MAX_LIVE_CREDITS_PER_WALLET {
                ReserveRefusal::TooManyLive {
                    balance_cents: spendable,
                }
            } else {
                ReserveRefusal::Insufficient {
                    balance_cents: spendable,
                    required_cents: g.charged_cents,
                }
            }));
        }
        for (position, line) in g.lines.iter().enumerate() {
            sqlx::query(
                "INSERT INTO credits.authorization_lines \
                     (authorization_id, position, quoted_cents, charged_cents, sale_kind, asset_kind, asset_chain_id, \
                      asset_contract, asset_item_id, asset_token_id, trade_id, contract_address, item_id, registered_name) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::numeric, $10::numeric, $11, $12, $13, $14)",
            )
            .bind(g.id)
            .bind(position as i32)
            .bind(line.quoted_cents)
            .bind(line.charged_cents)
            .bind(line.sale_kind)
            .bind(line.asset_kind)
            .bind(line.asset_chain_id)
            .bind(&line.asset_contract)
            .bind(line.asset_item_id.as_deref())
            .bind(line.asset_token_id.as_deref())
            .bind(line.trade_id.as_deref())
            .bind(line.contract_address.as_deref())
            .bind(line.item_id.as_deref())
            .bind(line.registered_name.as_deref())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(Ok(ReserveOutcome {
            id: g.id.to_string(),
            amount_wei: g.amount_wei.to_string(),
            usd_cents: g.charged_cents,
            expires_at: g.expires_at,
            replayed: false,
        }))
    }

    /// The balance at the peg, the live pledges and their count, in one read.
    pub async fn spendable(&self, address: &str) -> Result<Spendable, ApiError> {
        let row = sqlx::query(
            "SELECT COALESCE((SELECT floor(available * $2::numeric)::bigint FROM credits.wallets WHERE address = $1), 0) AS balance, \
                    COALESCE((SELECT SUM(usd_cents) FROM credits.authorizations WHERE address = $1 AND status IN ('authorized', 'retired')), 0)::bigint AS held, \
                    (SELECT count(*) FROM credits.authorizations WHERE address = $1 AND status IN ('authorized', 'retired'))::bigint AS live",
        )
        .bind(address)
        .bind(UsdCents::CENTS_PER_CREDIT)
        .fetch_one(&self.pool)
        .await?;
        Ok(Spendable {
            balance_cents: row.get("balance"),
            held_cents: row.get("held"),
            live: row.get("live"),
        })
    }

    /// Flip past-expiry pledges: `authorized` to `expired`, `retired` to
    /// `released`, bounded by the CreditUsed index when one is configured
    /// (`usage::expiry_bound_sql`: never before the chain has been read past
    /// the expiry). The outbox worker tick inlines this same UPDATE.
    pub async fn expire_stale_authorizations(&self) -> Result<u64, ApiError> {
        let sql = format!(
            "UPDATE credits.authorizations SET status = CASE status WHEN 'retired' THEN 'released' ELSE 'expired' END \
             WHERE status IN ('authorized', 'retired') AND expires_at <= {}",
            super::usage::expiry_bound_sql("$1", "$2")
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(now())
            .bind(self.usage_chain)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    pub async fn insert_authorization(&self, a: &NewAuthorization<'_>) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO credits.authorizations \
                 (id, address, usd_cents, amount_wei, trade_id, contract_address, item_id, source, status, expires_at, quoted_cents) \
             VALUES ($1, $2, $3, $4::numeric, $5, $6, $7, $8, 'authorized', $9, $3)",
        )
        .bind(a.id)
        .bind(a.address)
        .bind(a.usd_cents)
        .bind(a.amount_wei)
        .bind(a.trade_id)
        .bind(a.contract_address)
        .bind(a.item_id)
        .bind(a.source)
        .bind(a.expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The client cancelled: the signer's `authorized` rows among `salts`
    /// become `retired`. The pledge keeps counting until the sweep releases
    /// it; the answer names when that is at the earliest.
    pub async fn retire_intents(
        &self,
        salts: &[String],
        address: &str,
    ) -> Result<Retired, ApiError> {
        let rows = sqlx::query(
            "UPDATE credits.authorizations SET status = 'retired', retired_at = $3 \
             WHERE id = ANY($1) AND address = $2 AND status = 'authorized' \
             RETURNING id, expires_at",
        )
        .bind(salts)
        .bind(address)
        .bind(now())
        .fetch_all(&self.pool)
        .await?;
        let latest = rows
            .iter()
            .map(|r| r.get::<NaiveDateTime, _>("expires_at"))
            .max();
        Ok(Retired {
            retired: rows.len() as i64,
            ids: rows.iter().map(|r| r.get("id")).collect(),
            latest_expiry: latest,
        })
    }

    /// The closure pass's name for [`Self::retire_intents`]: the count.
    pub async fn release_intents(&self, salts: &[String], address: &str) -> Result<u64, ApiError> {
        Ok(self.retire_intents(salts, address).await?.retired as u64)
    }

    /// Stamps the transaction the caller reported submitting `salts` in, on
    /// the caller's own rows only, keeping the first hash reported.
    pub async fn record_submission(
        &self,
        salts: &[String],
        address: &str,
        tx_hash: &str,
    ) -> Result<i64, ApiError> {
        let res = sqlx::query(
            "UPDATE credits.authorizations SET submitted_tx_hash = COALESCE(submitted_tx_hash, $3), submitted_at = COALESCE(submitted_at, $4) \
             WHERE id = ANY($1) AND address = $2",
        )
        .bind(salts)
        .bind(address)
        .bind(tx_hash)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() as i64)
    }

    /// The authorization `address` already reserved under `idempotency_key`,
    /// if any. The key is derived from the signer, and the address is
    /// checked too, so no other wallet finds the row.
    pub async fn authorization_for_key(
        &self,
        idempotency_key: &str,
        address: &str,
    ) -> Result<Option<String>, ApiError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM credits.authorizations WHERE idempotency_key = $1 AND address = $2",
        )
        .bind(idempotency_key)
        .bind(address)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn get_authorization(&self, id: &str) -> Result<Option<AuthorizationRow>, ApiError> {
        let row = sqlx::query(
            "SELECT id, address, usd_cents, amount_wei::text AS amount_wei, status, expires_at, chain_id, contract, credit_hash, \
                    quoted_cents, oracle_rate::text AS oracle_rate, oracle_decimals, submitted_tx_hash \
             FROM credits.authorizations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| AuthorizationRow {
            id: r.get("id"),
            address: r.get("address"),
            usd_cents: r.get("usd_cents"),
            amount_wei: r.get("amount_wei"),
            status: r.get("status"),
            expires_at: r.get("expires_at"),
            chain_id: r.get("chain_id"),
            contract: r.get("contract"),
            credit_hash: r.get("credit_hash"),
            quoted_cents: r.get("quoted_cents"),
            oracle_rate: r.get("oracle_rate"),
            oracle_decimals: r.get("oracle_decimals"),
            submitted_tx_hash: r.get("submitted_tx_hash"),
        }))
    }
}

struct LegacyColumns<'a> {
    trade_id: Option<&'a str>,
    contract_address: Option<&'a str>,
    item_id: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retired {
    pub retired: i64,
    pub ids: Vec<String>,
    /// The latest expiry among the retired rows: the pledge rejoins the
    /// balance once the chain has been read past it.
    pub latest_expiry: Option<NaiveDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wei(text: &str) -> ManaWei {
        ManaWei::new(alloy_primitives::U256::from_str_radix(text, 10).unwrap())
    }

    #[test]
    fn a_five_dollar_pack_pledges_the_mana_of_five_hundred_cents() {
        let rate = ManaUsdRate::parse_usd_per_mana("0.25").unwrap();
        let budget = UsdCents::from_credits(50).unwrap();
        assert_eq!(budget, UsdCents::new(500), "50 credits at ten cents");
        assert_eq!(
            pledge_wei(budget, rate),
            Some(wei("20000000000000000000")),
            "500 cents at 0.25 USD/MANA is 20 MANA"
        );
        assert_eq!(
            pledge_wei(UsdCents::new(5_000), rate),
            Some(wei("200000000000000000000")),
            "the old 100-cent peg signed 200 MANA for the same pack"
        );
        assert_ne!(
            pledge_wei(budget, rate),
            pledge_wei(UsdCents::new(5_000), rate)
        );
    }

    #[test]
    fn pledge_wei_is_the_integer_oracle_path() {
        let rate = ManaUsdRate::parse_usd_per_mana("0.30").unwrap();
        assert_eq!(
            rate.rate(),
            alloy_primitives::U256::from(300_000_000_000_000_000u64)
        );
        assert_eq!(rate.decimals(), 18);
        let base = pledge_wei(UsdCents::new(200), rate).unwrap();
        assert_eq!(
            base,
            wei("6666666666666666666"),
            "each step floors; numeric division would round the tail up"
        );
        assert_eq!(
            ManaWei::from_usd_cents(UsdCents::new(200), rate.rate(), 18, 200),
            Some(wei("6799999999999999999")),
            "the rounding-vectors.json case at buffer 200"
        );
        assert_eq!(pledge_wei(UsdCents::new(0), rate), None);
        assert_eq!(ManaUsdRate::parse_usd_per_mana("0"), None);
        assert_eq!(ManaUsdRate::parse_usd_per_mana("0.5x"), None);
        assert_eq!(
            ManaUsdRate::parse_usd_per_mana("1").unwrap().rate(),
            alloy_primitives::U256::from(1_000_000_000_000_000_000u64)
        );
        assert_eq!(
            ManaUsdRate::parse_usd_per_mana(".5").unwrap().rate(),
            alloy_primitives::U256::from(500_000_000_000_000_000u64)
        );
        assert_eq!(
            ManaUsdRate::parse_usd_per_mana("0.1234567890123456789")
                .unwrap()
                .rate(),
            alloy_primitives::U256::from(123_456_789_012_345_678u64)
        );
    }

    #[test]
    fn spendable_never_goes_negative_and_refusals_map_to_their_statuses() {
        assert_eq!(
            Spendable {
                balance_cents: 500,
                held_cents: 700,
                live: 2
            }
            .spendable_cents(),
            0
        );
        assert_eq!(
            Spendable {
                balance_cents: 500,
                held_cents: 200,
                live: 1
            }
            .spendable_cents(),
            300
        );
        assert!(matches!(
            ApiError::from(ReserveRefusal::Insufficient {
                balance_cents: 1,
                required_cents: 2
            }),
            ApiError::PaymentRequired(_)
        ));
        assert!(matches!(
            ApiError::from(ReserveRefusal::TooManyLive { balance_cents: 1 }),
            ApiError::Conflict(_)
        ));
    }
}
