// from: credits-server@8f62147:src/logic/shop-daily-summary.ts (the closed UTC day, the hour, the exactly-once claim), src/adapters/db/db.ts (getShopDailySummary), src/logic/shop-notifications/component.ts (notifyDailySummary), src/logic/shop-notifications/float-lines.ts (describeFloat)
//! The credits daily summary: one Slack message per closed UTC day, posted
//! the morning after it closed, through the credits alerts outbox
//! (`alerts.rs`).
//!
//! The leased `credits-daily-summary` job runs every [`INTERVAL`] where
//! `OPS_WRITES_ENABLED` is on. Before `CREDITS_DAILY_SUMMARY_HOUR_UTC` (9) it
//! does nothing. From that hour it reports yesterday:
//! - an indexed `EXISTS` on the day's `credits.alerts` row ends the pass when
//!   the day is already queued, so most ticks read nothing else;
//! - otherwise it computes the figures on one REPEATABLE READ, READ ONLY
//!   snapshot whose statements are bounded (`Credits::read_snapshot`);
//! - then it reads the CreditsManager's MANA balance and the MANA/USD rate,
//!   each bounded by [`CHAIN_READ_TIMEOUT`], and queues the row
//!   ([`DAILY_SUMMARY`], keyed by the day, `ON CONFLICT DO NOTHING`).
//!
//! The row is the claim: a restart, a second replica or a pass that raced
//! past the lease adds nothing. A missed window posts on the next tick,
//! still for the right day, and only for yesterday, as credits-server does.
//! The `credits-alerts` job delivers the row like every other credits alert,
//! to `CREDITS_ALERTS_SLACK_WEBHOOK`, rendering it from its payload
//! ([`message`]). A chain read that fails or times out makes its line say
//! "unavailable"; the summary still goes out and the day is never retried
//! for it.
//!
//! What each figure reads (the day is `[00:00, 24:00)` UTC):
//! - **credits sold**: card purchases (`credits.purchases`, `method =
//!   'card'`) created that day and now `paid`, with the card fee estimated at
//!   Stripe's published rate, as the pack feed estimates it;
//! - **items bought, buyers, MANA moved**: the CreditUsed settlements of that
//!   day (`credits.usage_settlements` by `settled_at`): items are the lines of
//!   the purchases settled, one for a purchase whose single item sits on its
//!   authorization (as the purchase audit counts them), never a remainder;
//!   checkouts the distinct (wallet, transaction) pairs, credits spent the
//!   credits debited, MANA the value used. A buyer is new when their first
//!   purchase settlement ever is that day;
//! - **card checkout**: the card purchases created that day by status, in
//!   credits-server's order vocabulary (`history::order_status`);
//! - **unspent**: the paid slice of every wallet (`available -
//!   earned_available`), now;
//! - **float coverage**: what the CreditsManager still owes, now: every
//!   wallet's spendable balance floored at zero, and the claimed, unexpired
//!   goal grants (`credits.goal_grants`) less what they have used
//!   (`credits.usages` of the grant's salt, chain, contract, credit and
//!   wallet, as the wallet's balance reads them); the pace is the value used
//!   over the seven days ending with the reported day;
//! - **warnings** (only when non-zero): authorizations created that day that
//!   expired, and the Stripe reversals (`credits.payment_reversals`) last
//!   updated that day: the reversals with a refunded amount and its sum, and
//!   the disputed ones. A row records both when a refund was followed by a
//!   dispute, and counts in both.
//!
//! "Unspent" is the `paid` bucket, which holds the credits sold and the
//! credits an operator granted alike; the line says so.
//!
//! The reads are once a day. The day filters' columns carry no index, so
//! each day read scans its table: `purchases` (a row per card or MANA
//! purchase), `usage_settlements` and `usages` (a row per CreditUsed log),
//! `authorizations` (a row per checkout attempt), `payment_reversals` (a
//! row per reversed intent) and `wallets`. New buyers aggregate the
//! purchase settlements of all history, in the same one scan. A goal
//! grant's usage is read by salt (`usages_salt_idx`). When the reads fail
//! (past the statement timeout), the pass is retried with a backoff of up
//! to an hour. A failure within [`LAST_CHANCE`] (65 minutes) of the end of
//! the reporting day, judged at the time it failed, may get no retry for
//! that day: it queues the day without its figures, so the channel sees the
//! gap rather than nothing.
//!
//! The rate is used only from a round no older than
//! `MANA_PRICE_MAX_STALENESS_SECS`, the freshness the credits price
//! feed is held to. An older round leaves the float in MANA with its USD
//! value unavailable.
//!
//! What credits-server reports that this does not:
//! - **collections published**: the node takes no credits for a publication;
//! - **the Across and Axelar executors' floats** and their warning: the node
//!   does not run those routes;
//! - **the `STRIPE_FEE_PERCENT`, `STRIPE_FEE_FIXED_CENTS`,
//!   `MANA_FLOAT_WARN_DAYS` and `MANA_FLOAT_WARN_EXECUTOR` settings**: the
//!   fee is Stripe's published rate and the runway warning
//!   [`RUNWAY_WARN_DAYS`], credits-server's defaults;
//! - **reversals by the time they happened**: the node keeps no reversal
//!   time, only the reversal row's last update.

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, U256};
use alloy_sol_types::{SolCall, sol};
use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime, Timelike};
use serde_json::{Value, json};
use sqlx::Row;

use super::alerts::{self, grouped, usd};
use super::errors::ApiError;
use super::store::{Credits, now};
use crate::evm::chain::ChainComponent;
use crate::jobs::{JobContext, Jobs, PeriodicCfg};
use crate::shared::writes::{PassGate, WRITES_DISABLED_MESSAGE};

pub const JOB_NAME: &str = "credits-daily-summary";
/// The `credits.alerts` kind of a day's summary, keyed by the day.
pub const DAILY_SUMMARY: &str = "daily_summary";
/// `CREDITS_DAILY_SUMMARY_HOUR_UTC`'s default.
pub const DEFAULT_HOUR_UTC: u32 = 9;
const INTERVAL: Duration = Duration::from_secs(300);
/// The longest wait between failed passes: a day whose reads keep failing
/// is tried about once an hour, not every [`INTERVAL`].
const MAX_ERROR_BACKOFF: Duration = Duration::from_secs(3600);
/// How close to the end of its reporting day a failure is the day's last
/// chance: the next pass comes up to [`MAX_ERROR_BACKOFF`] after the
/// failure, plus a pass's slack, and may fall on the next day, which reports
/// its own yesterday. Such a failure queues the day without its figures.
pub const LAST_CHANCE: Duration =
    Duration::from_secs(MAX_ERROR_BACKOFF.as_secs() + INTERVAL.as_secs());
/// Each chain read (the balance, the rate) is given up after this long.
pub const CHAIN_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Fewer days of float at the seven-day pace than this raise a warning.
pub const RUNWAY_WARN_DAYS: u64 = 7;
const PACE_DAYS: i64 = 7;
const WEI: u128 = 1_000_000_000_000_000_000;
const USD_CENTS_PER_CREDIT: i64 = 10;

sol! {
    function balanceOf(address owner) external view returns (uint256);
    function decimals() external view returns (uint8);
    function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
}

/// One chain figure: never configured, read and failed, or read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    NotConfigured,
    Unavailable,
    Value(U256),
}

impl Reading {
    fn json(self) -> Value {
        match self {
            Self::NotConfigured => Value::Null,
            Self::Unavailable => json!("unavailable"),
            Self::Value(v) => json!(v.to_string()),
        }
    }

    fn of(value: &Value) -> Self {
        match value {
            Value::Null => Self::NotConfigured,
            Value::String(s) => s
                .parse::<U256>()
                .map(Self::Value)
                .unwrap_or(Self::Unavailable),
            _ => Self::Unavailable,
        }
    }
}

/// The right-now chain figures the summary prices its float with.
#[async_trait]
pub trait ChainFigures: Send + Sync {
    /// The CreditsManager's MANA balance, in wei.
    async fn float_wei(&self) -> Reading;
    /// USD per MANA with 18 decimals.
    async fn mana_usd_18(&self) -> Reading;
}

/// No chain configured: neither figure is shown.
pub struct NoChain;

#[async_trait]
impl ChainFigures for NoChain {
    async fn float_wei(&self) -> Reading {
        Reading::NotConfigured
    }
    async fn mana_usd_18(&self) -> Reading {
        Reading::NotConfigured
    }
}

/// The figures over the collections chain's RPC (`RPC_ENDPOINT_POLYGON`):
/// `balanceOf(creditsManager)` on that chain's MANA, and the MANA/USD
/// aggregator's `decimals()` and `latestRoundData()` decoded as
/// `evm::oracle` decodes them. Each read is bounded by
/// [`CHAIN_READ_TIMEOUT`]; a failure is logged without the endpoint.
pub struct ChainReader {
    pub chain: Arc<ChainComponent>,
    pub credits_manager: Address,
    pub mana: Address,
    pub aggregator: Option<Address>,
    /// The oldest round the rate is read from, in seconds
    /// (`MANA_PRICE_MAX_STALENESS_SECS`, the credits price feed's
    /// freshness); an older one makes the rate unavailable.
    pub max_age_secs: i64,
}

#[async_trait]
impl ChainFigures for ChainReader {
    async fn float_wei(&self) -> Reading {
        let call = balanceOfCall {
            owner: self.credits_manager,
        }
        .abi_encode();
        let read = tokio::time::timeout(CHAIN_READ_TIMEOUT, self.chain.eth_call(self.mana, &call));
        match read.await {
            Ok(Ok(ret)) => match balanceOfCall::abi_decode_returns(&ret) {
                Ok(balance) => Reading::Value(balance),
                Err(_) => unavailable("the CreditsManager balance answered an undecodable result"),
            },
            Ok(Err(_)) => unavailable("the CreditsManager balance could not be read"),
            Err(_) => unavailable("the CreditsManager balance read timed out"),
        }
    }

    async fn mana_usd_18(&self) -> Reading {
        let Some(aggregator) = self.aggregator else {
            return Reading::NotConfigured;
        };
        let decimals_call = decimalsCall {}.abi_encode();
        let round_call = latestRoundDataCall {}.abi_encode();
        let read = tokio::time::timeout(CHAIN_READ_TIMEOUT, async {
            tokio::try_join!(
                self.chain.eth_call(aggregator, &decimals_call),
                self.chain.eth_call(aggregator, &round_call),
            )
        });
        match read.await {
            Ok(Ok((decimals, round))) => {
                match crate::evm::oracle::decode_rate_reading(&decimals, &round) {
                    Ok(rate)
                        if chrono::Utc::now().timestamp() - rate.updated_at_s
                            > self.max_age_secs =>
                    {
                        unavailable("the MANA/USD rate's latest round is stale")
                    }
                    Ok(rate) => Reading::Value(rate.rate_18),
                    Err(_) => unavailable("the MANA/USD rate answered an unusable round"),
                }
            }
            Ok(Err(_)) => unavailable("the MANA/USD rate could not be read"),
            Err(_) => unavailable("the MANA/USD rate read timed out"),
        }
    }
}

impl ChainReader {
    /// The reader over the CreditUsed index's chain and contract
    /// (`CREDITS_MANAGER_CONTRACT`), with that chain's MANA and, on Polygon,
    /// the MANA/USD aggregator the economy module reads, fresh within
    /// `max_age_secs`. [`NoChain`] where the index is not configured or the
    /// chain is not one the node knows.
    pub fn for_usage(
        chain: Option<&Arc<ChainComponent>>,
        usage: &super::usage::Settings,
        max_age_secs: i64,
    ) -> Arc<dyn ChainFigures> {
        let manager = usage
            .contract
            .as_deref()
            .and_then(|c| c.parse::<Address>().ok());
        let chain_id = u64::try_from(usage.chain_id).ok();
        let mana = chain_id
            .and_then(crate::evm::contracts_addrs::DclContracts::for_chain)
            .map(|c| c.mana_token);
        match (chain, manager, mana) {
            (Some(chain), Some(credits_manager), Some(mana)) => Arc::new(Self {
                chain: chain.clone(),
                credits_manager,
                mana,
                aggregator: (chain_id == Some(137))
                    .then_some(crate::evm::oracle::MANA_USD_AGGREGATOR_POLYGON),
                max_age_secs,
            }),
            _ => Arc::new(NoChain),
        }
    }
}

fn unavailable(what: &str) -> Reading {
    tracing::warn!(
        job = JOB_NAME,
        "{what}; the daily summary says it is unavailable"
    );
    Reading::Unavailable
}

/// What a pass did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassOutcome {
    BeforeTheHour,
    AlreadyQueued(NaiveDate),
    Queued(NaiveDate),
}

/// The summary job's settings and chain.
#[derive(Clone)]
pub struct DailySummary {
    pub hour_utc: u32,
    pub chain: Arc<dyn ChainFigures>,
}

impl DailySummary {
    /// Queues yesterday's summary at `now` when it is due and not queued.
    pub async fn run_pass(
        &self,
        credits: &Credits,
        now: NaiveDateTime,
    ) -> Result<PassOutcome, ApiError> {
        self.run_pass_on(credits, || now).await
    }

    /// [`run_pass`](Self::run_pass) at `clock`'s time, read again when the
    /// figures fail: a failure within [`LAST_CHANCE`] of the end of the day
    /// after the reported one queues the day without them.
    pub async fn run_pass_on(
        &self,
        credits: &Credits,
        clock: impl Fn() -> NaiveDateTime,
    ) -> Result<PassOutcome, ApiError> {
        let now = clock();
        if now.hour() < self.hour_utc {
            return Ok(PassOutcome::BeforeTheHour);
        }
        let day = now.date() - chrono::Days::new(1);
        let key = day.format("%Y-%m-%d").to_string();
        let queued: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM credits.alerts WHERE kind = $1 AND dedupe_key = $2)",
        )
        .bind(DAILY_SUMMARY)
        .bind(&key)
        .fetch_one(&credits.pool)
        .await?;
        if queued {
            return Ok(PassOutcome::AlreadyQueued(day));
        }
        let last_pass = (day + chrono::Days::new(2))
            .and_hms_opt(0, 0, 0)
            .unwrap_or_default()
            - chrono::Duration::from_std(LAST_CHANCE).unwrap_or_default();
        let mut payload = match figures(credits, day, now).await {
            Ok(payload) => payload,
            Err(e) if clock() >= last_pass => {
                tracing::error!(job = JOB_NAME, %day, error = %e, "the daily summary's figures could not be read today; queueing the day without them");
                json!({ "day": key, "figuresUnavailable": true })
            }
            Err(e) => return Err(e),
        };
        // Read after the figures, so the RPC is hit once a day rather than
        // on every tick; a balance is a right-now figure, labelled as such.
        let float = self.chain.float_wei().await;
        let rate = match float {
            Reading::Value(_) => self.chain.mana_usd_18().await,
            _ => Reading::NotConfigured,
        };
        payload["floatWei"] = float.json();
        payload["manaUsd18"] = rate.json();
        let mut conn = credits.pool.acquire().await?;
        // A pass that raced another past the check above adds nothing.
        if alerts::queue(&mut conn, DAILY_SUMMARY, &key, payload).await? {
            Ok(PassOutcome::Queued(day))
        } else {
            Ok(PassOutcome::AlreadyQueued(day))
        }
    }
}

/// The day's figures as the alert's payload, read on one snapshot.
pub(crate) async fn figures(
    credits: &Credits,
    day: NaiveDate,
    now: NaiveDateTime,
) -> Result<Value, ApiError> {
    let from = day.and_hms_opt(0, 0, 0).unwrap_or_default();
    let to = from + chrono::Duration::days(1);
    let mut tx = credits.read_snapshot().await?;

    let packs = sqlx::query(
        "SELECT count(*) AS n, COALESCE(sum(credits), 0)::text AS credits, COALESCE(sum(amount_cents), 0)::bigint AS cents \
         FROM credits.purchases WHERE method = 'card' AND status = 'paid' AND created_at >= $1 AND created_at < $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    // A cart of N items settles N lines in one transaction: the
    // transaction, with the wallet, is the checkout a buyer would count.
    let bought = sqlx::query(
        "WITH day AS ( \
             SELECT s.authorization_id, s.address, s.kind, s.value_wei, s.debited_credits, \
                    COALESCE(u.tx_hash, s.authorization_id) AS checkout \
             FROM credits.usage_settlements s \
             LEFT JOIN credits.usages u ON u.chain_id = s.chain_id AND u.block_number = s.block_number \
                                       AND u.log_index = s.log_index \
             WHERE s.settled_at >= $1 AND s.settled_at < $2) \
         SELECT (SELECT COALESCE(sum(GREATEST((SELECT count(*) FROM credits.authorization_lines l \
                                                    WHERE l.authorization_id = p.authorization_id), 1)), 0)::bigint \
                 FROM (SELECT DISTINCT authorization_id FROM day WHERE kind = 'purchase') p) AS items, \
                count(DISTINCT (address, checkout)) FILTER (WHERE kind = 'purchase') AS checkouts, \
                COALESCE(sum(debited_credits), 0)::text AS credits, \
                count(DISTINCT address) FILTER (WHERE kind = 'purchase') AS buyers, \
                COALESCE(sum(value_wei), 0)::text AS mana \
         FROM day",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    let new_buyers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT address FROM credits.usage_settlements WHERE kind = 'purchase' \
         GROUP BY address HAVING min(settled_at) >= $1 AND min(settled_at) < $2) first_purchase",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    let mut checkout = json!({ "credited": 0, "abandoned": 0, "failed": 0, "pending": 0 });
    let statuses = sqlx::query(
        "SELECT status, amount_cents, refunded_cents FROM credits.purchases \
         WHERE method = 'card' AND created_at >= $1 AND created_at < $2",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    for row in &statuses {
        let status = super::history::order_status(
            row.get("status"),
            row.get("amount_cents"),
            row.get("refunded_cents"),
        );
        let key = match status {
            "credited" => "credited",
            "abandoned" => "abandoned",
            "failed" => "failed",
            "processing" => "pending",
            _ => continue,
        };
        checkout[key] = json!(checkout[key].as_i64().unwrap_or(0) + 1);
    }

    let balances = sqlx::query(
        "SELECT COALESCE(sum(available - earned_available), 0)::text AS unspent, \
                COALESCE(sum(GREATEST(available, 0)), 0)::text AS owed \
         FROM credits.wallets",
    )
    .fetch_one(&mut *tx)
    .await?;

    let owed_mana: String = sqlx::query_scalar(
        "SELECT TRUNC(COALESCE(sum(GREATEST(g.amount_wei - COALESCE( \
                    (SELECT sum(u.value_wei) FROM credits.usages u WHERE u.salt = g.salt AND u.chain_id = g.chain_id \
                     AND u.contract = g.contract AND u.credit_id = g.credit_hash AND u.sender = g.address), 0), 0)), 0))::text \
         FROM credits.goal_grants g \
         WHERE g.claimed_at IS NOT NULL AND g.expires_at > $1",
    )
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    let outflow: String = sqlx::query_scalar(
        "SELECT TRUNC(COALESCE(sum(value_wei), 0))::text FROM credits.usages \
         WHERE COALESCE(block_time, indexed_at) >= $1 AND COALESCE(block_time, indexed_at) < $2",
    )
    .bind(to - chrono::Duration::days(PACE_DAYS))
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    let expired: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.authorizations \
         WHERE status = 'expired' AND created_at >= $1 AND created_at < $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    let reversals = sqlx::query(
        "SELECT count(*) FILTER (WHERE refunded_cents > 0) AS refunds, \
                COALESCE(sum(refunded_cents), 0)::bigint AS cents, \
                count(*) FILTER (WHERE disputed) AS chargebacks \
         FROM credits.payment_reversals WHERE updated_at >= $1 AND updated_at < $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(json!({
        "day": day.format("%Y-%m-%d").to_string(),
        "packs": {
            "count": packs.get::<i64, _>("n"),
            "credits": packs.get::<String, _>("credits"),
            "chargedCents": packs.get::<i64, _>("cents"),
        },
        "purchases": {
            "items": bought.get::<i64, _>("items"),
            "checkouts": bought.get::<i64, _>("checkouts"),
            "credits": bought.get::<String, _>("credits"),
            "buyers": bought.get::<i64, _>("buyers"),
            "newBuyers": new_buyers,
            "manaWei": bought.get::<String, _>("mana"),
        },
        "checkout": checkout,
        "unspentCredits": balances.get::<String, _>("unspent"),
        "owed": {
            "credits": balances.get::<String, _>("owed"),
            "manaWei": owed_mana,
        },
        "outflow7dWei": outflow,
        "integrity": {
            "expired": expired,
            "refunds": reversals.get::<i64, _>("refunds"),
            "refundedCents": reversals.get::<i64, _>("cents"),
            "chargebacks": reversals.get::<i64, _>("chargebacks"),
        },
    }))
}

/// A numeric credits amount, rounded up to whole credits (a charge is whole
/// credits already; the ceiling guards a fractional legacy row).
fn credits_ceil(value: &Value) -> i64 {
    let text = value.as_str().unwrap_or("0");
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let whole: i64 = whole.parse().unwrap_or(0);
    if fraction.chars().any(|c| c != '0') && !text.starts_with('-') {
        whole + 1
    } else {
        whole
    }
}

fn wei(value: &Value) -> U256 {
    value
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(U256::ZERO)
}

/// Whole MANA, thousands separated: a float is read for scale.
fn whole_mana(wei: U256) -> String {
    let whole = wei / U256::from(WEI);
    i64::try_from(whole).map_or_else(|_| whole.to_string(), grouped)
}

/// Wei of MANA at `rate_18` USD per MANA, in cents.
fn wei_to_cents(wei: U256, rate_18: U256) -> i64 {
    let cents = wei
        .saturating_mul(rate_18)
        .saturating_mul(U256::from(100u64))
        / U256::from(WEI)
        / U256::from(WEI);
    i64::try_from(cents).unwrap_or(i64::MAX)
}

/// `float / owed`, floored: two decimals below 1×, where the distance to
/// fully covered matters, one above.
fn times(float_cents: i64, owed_cents: i64) -> String {
    let hundredths =
        i64::try_from(i128::from(float_cents) * 100 / i128::from(owed_cents)).unwrap_or(i64::MAX);
    if hundredths < 100 {
        format!("0.{hundredths:02}")
    } else {
        let tenths = hundredths / 10;
        format!("{}.{}", tenths / 10, tenths % 10)
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{} {}", grouped(n), if n == 1 { one } else { many })
}

/// The summary's Slack message, from its payload.
pub(crate) fn message(payload: &Value) -> String {
    let n = |v: &Value| v.as_i64().unwrap_or(0);
    let day = payload["day"].as_str().unwrap_or("an unknown day");
    if payload["figuresUnavailable"].as_bool() == Some(true) {
        return format!(
            ":bar_chart: *Shop — {day} (UTC)*\n:warning: The day's figures could not be read: every pass that day failed. The node's `{JOB_NAME}` errors say why."
        );
    }
    let packs = &payload["packs"];
    let (pack_count, charged) = (n(&packs["count"]), n(&packs["chargedCents"]));
    let pack_credits = credits_ceil(&packs["credits"]);
    let mut lines = vec![
        format!(":bar_chart: *Shop — {day} (UTC)*"),
        format!(
            ":credit_card: *Credits sold*  {} · {} credits",
            plural(pack_count, "pack", "packs"),
            grouped(pack_credits)
        ),
    ];
    // Where the money went: charged, less the card fee and the credits now
    // owed, is what stays. Omitted on a day with nothing sold.
    if pack_count > 0 {
        let fee = alerts::stripe_fee_cents(charged, pack_count);
        let issued = pack_credits * USD_CENTS_PER_CREDIT;
        let net = charged - fee - issued;
        let pct = if charged > 0 {
            alerts::tenths(alerts::round_div(net * 1000, charged))
        } else {
            "0.0".to_string()
        };
        lines.push(format!(
            "      {} charged  −{} Stripe (est.)  −{} credits issued  =  *{}* ours ({pct}%)",
            usd(charged),
            usd(fee),
            usd(issued),
            usd(net)
        ));
    }

    let bought = &payload["purchases"];
    let (items, checkouts) = (n(&bought["items"]), n(&bought["checkouts"]));
    let spent = credits_ceil(&bought["credits"]);
    let spent_cents = spent * USD_CENTS_PER_CREDIT;
    lines.push(if items > 0 {
        // Order value is per checkout: a cart is one purchase.
        let aov = if checkouts > 0 {
            usd(alerts::round_div(spent_cents, checkouts))
        } else {
            usd(0)
        };
        format!(
            ":shopping_bags: *Items bought*  {} in {} · {} credits ({}) · AOV {aov}",
            plural(items, "item", "items"),
            plural(checkouts, "checkout", "checkouts"),
            grouped(spent),
            usd(spent_cents)
        )
    } else {
        ":shopping_bags: *Items bought*  0 items".to_string()
    });
    let moved = wei(&bought["manaWei"]);
    lines.push(format!(
        ":busts_in_silhouette: *Buyers*  {} unique, {} new · {} MANA moved{}",
        grouped(n(&bought["buyers"])),
        grouped(n(&bought["newBuyers"])),
        alerts::mana(&moved.to_string()),
        if spent_cents > 0 {
            format!(" ({})", usd(spent_cents))
        } else {
            String::new()
        }
    ));

    let checkout = &payload["checkout"];
    let (credited, abandoned, failed, pending) = (
        n(&checkout["credited"]),
        n(&checkout["abandoned"]),
        n(&checkout["failed"]),
        n(&checkout["pending"]),
    );
    let settled = credited + abandoned + failed;
    let completion = if settled > 0 {
        format!(
            " → {}% completed",
            alerts::round_div(100 * credited, settled)
        )
    } else {
        String::new()
    };
    let open = if pending > 0 {
        format!(" · {pending} still open")
    } else {
        String::new()
    };
    lines.push(format!(
        ":receipt: *Card checkout*  {credited} credited · {abandoned} abandoned · {failed} declined{completion}{open}"
    ));
    lines.push(format!(
        ":bank: *Unspent*  {} in paid credits (sold or granted) not yet spent",
        usd(credits_ceil(&payload["unspentCredits"]) * USD_CENTS_PER_CREDIT)
    ));

    let mut warnings = Vec::new();
    float_lines(payload, &mut lines, &mut warnings);

    let integrity = &payload["integrity"];
    let expired = n(&integrity["expired"]);
    if expired > 0 {
        warnings.push(format!(
            "{} expired unused",
            plural(expired, "reservation", "reservations")
        ));
    }
    let refunds = n(&integrity["refunds"]);
    if refunds > 0 {
        warnings.push(format!(
            ":warning: {} ({})",
            plural(refunds, "refund", "refunds"),
            usd(n(&integrity["refundedCents"]))
        ));
    }
    let chargebacks = n(&integrity["chargebacks"]);
    if chargebacks > 0 {
        warnings.push(format!(
            ":warning: {}",
            plural(chargebacks, "chargeback", "chargebacks")
        ));
    }
    let mut text = lines.join("\n");
    if !warnings.is_empty() {
        text.push_str("\n\n");
        text.push_str(&warnings.join(" · "));
    }
    text
}

/// Whether the CreditsManager can pay every credit still owed, and for how
/// long at the seven-day pace. A live reading, never the reported day's.
fn float_lines(payload: &Value, lines: &mut Vec<String>, warnings: &mut Vec<String>) {
    let float = match Reading::of(&payload["floatWei"]) {
        Reading::NotConfigured => return,
        Reading::Unavailable => {
            lines.push(
                ":fuelpump: *Float now*  unavailable (could not read the balance on-chain)"
                    .to_string(),
            );
            warnings.push(":warning: Float unavailable".to_string());
            return;
        }
        Reading::Value(float) => float,
    };
    let rate = match Reading::of(&payload["manaUsd18"]) {
        Reading::Value(rate) => Some(rate),
        _ => None,
    };
    let float_cents = rate.map(|rate| wei_to_cents(float, rate));
    let priced = float_cents.map_or(String::new(), |cents| {
        format!(
            " (${} · {} credits)",
            grouped(cents / 100),
            grouped(cents / USD_CENTS_PER_CREDIT)
        )
    });
    lines.push(format!(
        ":fuelpump: *Float now*  CreditsManager {} MANA{priced}",
        whole_mana(float)
    ));

    let mut coverage = Vec::new();
    match (float_cents, rate) {
        (Some(float_cents), Some(rate)) => {
            let owed = &payload["owed"];
            let owed_cents = credits_ceil(&owed["credits"]) * USD_CENTS_PER_CREDIT
                + wei_to_cents(wei(&owed["manaWei"]), rate);
            if owed_cents > 0 {
                let x = times(float_cents, owed_cents);
                coverage.push(format!(
                    "{x}× the {} owed in unspent credits",
                    usd(owed_cents)
                ));
                if float_cents < owed_cents {
                    warnings.push(format!(
                        ":warning: Float covers only {x}× the unspent credits — top up the CreditsManager"
                    ));
                }
            } else {
                coverage.push("nothing owed in unspent credits".to_string());
            }
        }
        _ => {
            coverage.push("unpriced (MANA/USD rate unavailable)".to_string());
            warnings
                .push(":warning: Coverage unpriced (could not read the MANA/USD rate)".to_string());
        }
    }
    let per_day = wei(&payload["outflow7dWei"]) / U256::from(PACE_DAYS as u64);
    if per_day > U256::ZERO {
        let days = float / per_day;
        let days_text = i64::try_from(days).map_or_else(|_| days.to_string(), grouped);
        coverage.push(format!(
            "~{days_text} days at the 7-day pace ({} MANA/day)",
            whole_mana(per_day)
        ));
        if days < U256::from(RUNWAY_WARN_DAYS) {
            warnings.push(format!(
                ":warning: Float runs out in ~{days_text} days at the 7-day pace — top up the CreditsManager"
            ));
        }
    } else {
        coverage.push("no payouts in the last 7 days".to_string());
    }
    lines.push(format!(":shield: *Coverage*  {}", coverage.join(" · ")));
}

/// The `credits-daily-summary` job: a pass every [`INTERVAL`]. It queues a
/// row the ledger node delivers, so it stands down while
/// `OPS_WRITES_ENABLED` is off.
pub fn spawn(jobs: &Jobs, credits: Credits, summary: DailySummary, writes_enabled: bool) {
    if !writes_enabled {
        tracing::info!("OPS_WRITES_ENABLED=false: the credits daily summary job is not running");
        return;
    }
    let gate = Arc::new(PassGate::new(JOB_NAME));
    jobs.spawn_periodic_leased(
        JOB_NAME,
        PeriodicCfg::every(INTERVAL)
            .with_jitter(Duration::from_secs(30))
            .with_error_backoff(MAX_ERROR_BACKOFF),
        move |ctx: JobContext| {
            let credits = credits.clone();
            let summary = summary.clone();
            let gate = gate.clone();
            async move {
                if !gate.allows(writes_enabled) {
                    ctx.progress(WRITES_DISABLED_MESSAGE);
                    return Ok(());
                }
                let outcome = summary
                    .run_pass_on(&credits, now)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                if let PassOutcome::Queued(day) = outcome {
                    ctx.progress(format!("queued the summary of {day}"));
                }
                Ok(())
            }
        },
    );
}
