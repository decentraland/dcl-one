// from: credits-server@d9c6a261:src/metrics.ts (metricDeclarations: credits_squid_head_known, credits_squid_head_lag_seconds), src/logic/usd-reconciler.ts (settled, recovered), src/controllers/handlers/stripe-webhook.ts (the credited, mismatch and reversal paths)
//! The credits program's Prometheus families. `catalyst serve` records every
//! one: the Stripe webhook, the authorize and admin routes and the credits
//! jobs (`credits-usage-index`, `credits-settlement`,
//! `credits-purchase-receipts`, the outbox tick) all run there, and serve
//! registers [`FAMILIES`] on its registry (`serve_lifecycle::registry`);
//! `catalyst index` records none.
//!
//! - A counter moves only after the transaction that made its fact true
//!   commits ([`Facts`] carries the webhook's to its commit), so a grant
//!   that rolls back counts nothing, and a redelivered Stripe event, whose
//!   claim makes it `already processed`, counts nothing either.
//! - A gauge is set from one read per job pass, never per scrape, and reads
//!   0 until the first pass. The jobs are leased, so in a fleet one serve
//!   replica sets them: aggregate with `max`, never `sum`.
//! - Labels are enums; never an address, an id, a hash or an email. Money is
//!   whole USD cents: credits can be fractional (a partial refund charges
//!   back a proportion), and a counter only counts whole units.
//!
//! # credits-server's series
//!
//! What the node emits for each series credits-server@d9c6a261 declared in
//! `src/metrics.ts`:
//!
//! | Legacy series | Here |
//! | --- | --- |
//! | `credits_squid_head_known` | replaced by `credits_usage_index_ready`: the node reads CreditUsed logs with its own index, and the gauge is 1 while that index is published and fresh (the authorize lane is open) |
//! | `credits_squid_head_lag_seconds` | replaced by `credits_usage_index_lag_seconds`, set while the index is ready |
//! | `credits_granted_total`, `credits_amount_granted`, `credits_goals_completed_total` (`goal_id`, `season_id`), `credits_users_enrolled_total`, `credits_remaining_mana`, `credits_max_mana`, `credits_remaining_mana_claimed`, `credits_threshold_reached` (`season_id`, `season_name`), `captcha_failed_attempts_total`, `captcha_user_flagged_total` | not emitted yet (decentraland/dcl-one#2447): goal grants commit inside several sources' transactions, each to be counted after its own commit |
//! | `credits_sale_proceeds_credited_total`, `credits_sale_proceeds_credited_cents_total`, `credits_sale_proceeds_fills_ignored_total`, `credits_sale_proceeds_pass_errors_total`, `credits_sale_proceeds_squid_schema_unsupported_total` | not applicable: the node has no treasury proceeds consumer |
//! | `http_requests_total`, `http_request_duration_seconds`, `http_request_size_bytes` | replaced by the shared `dcl_http_*` families, `component="shop"`, routes under `/credits` |
//! | `wkc_logger_logs_total` | not applicable: a process-wide count of `tracing` events would mix in every other area's logs |
//!
//! Its worker manager's `worker_run_duration_seconds` and
//! `last_worker_run_timestamp` were never declared or recorded (commented-out
//! code); serve's `dcl_job_*` families cover each credits job by `worker`.

use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily};
use sqlx::Row;

use super::errors::ApiError;
use super::store::Credits;

const PURCHASES: CounterFamily<1> = CounterFamily::new(
    "credits_purchases_total",
    "Settled-payment events the Stripe webhook processed, by outcome.",
    None,
    &["outcome"],
);
const PURCHASE_USD_CENTS: CounterFamily<0> = CounterFamily::new(
    "credits_purchase_usd_cents_total",
    "USD cents charged by granted pack purchases.",
    None,
    &[],
);
const PURCHASE_REVERSALS: CounterFamily<2> = CounterFamily::new(
    "credits_purchase_reversals_total",
    "Reversals that charged credits back from a granted purchase, by kind and whether part could not be recovered.",
    None,
    &["kind", "shortfall"],
);
const RECEIPTS: CounterFamily<1> = CounterFamily::new(
    "credits_purchase_receipts_total",
    "Purchase receipt emails, by terminal status.",
    None,
    &["status"],
);
const PLEDGES: CounterFamily<1> = CounterFamily::new(
    "credits_pledges_total",
    "Credit pledge transitions, by outcome.",
    None,
    &["outcome"],
);
const PLEDGES_LIVE: GaugeFamily<0> = GaugeFamily::new(
    "credits_pledges_live",
    "Pledges holding credits (authorized or retired).",
    None,
    &[],
);
const PLEDGED_USD_CENTS: GaugeFamily<0> = GaugeFamily::new(
    "credits_pledged_usd_cents",
    "USD cents held by live pledges.",
    None,
    &[],
);
const USAGE_INDEX_READY: GaugeFamily<0> = GaugeFamily::new(
    "credits_usage_index_ready",
    "1 while the CreditUsed index is published and fresh (the authorize lane is open), else 0.",
    None,
    &[],
);
const USAGE_INDEX_LAG: GaugeFamily<0> = GaugeFamily::new(
    "credits_usage_index_lag_seconds",
    "Seconds from the last block the CreditUsed index folded and published to the pass that read it; meaningful while credits_usage_index_ready is 1.",
    None,
    &[],
);
const SETTLEMENTS: CounterFamily<2> = CounterFamily::new(
    "credits_settlements_total",
    "CreditUsed logs settled into a ledger debit, by kind and whether the pledge's hold had already ended.",
    None,
    &["kind", "recovered"],
);
const USAGES_UNPAID: GaugeFamily<0> = GaugeFamily::new(
    "credits_usages_unpaid",
    "CreditUsed logs of credits this node signed, at or below the published fold, that no ledger debit settles.",
    None,
    &[],
);
const REVOCATIONS: CounterFamily<1> = CounterFamily::new(
    "credits_revocations_total",
    "revokeCredits outcomes for consumed credits, by status.",
    None,
    &["status"],
);
const REVOCATIONS_PENDING: GaugeFamily<0> = GaugeFamily::new(
    "credits_revocations_pending",
    "Revocations queued and not yet confirmed or failed.",
    None,
    &[],
);

pub const FAMILIES: &[Descriptor] = &[
    PURCHASES.descriptor,
    PURCHASE_USD_CENTS.descriptor,
    PURCHASE_REVERSALS.descriptor,
    RECEIPTS.descriptor,
    PLEDGES.descriptor,
    PLEDGES_LIVE.descriptor,
    PLEDGED_USD_CENTS.descriptor,
    USAGE_INDEX_READY.descriptor,
    USAGE_INDEX_LAG.descriptor,
    SETTLEMENTS.descriptor,
    USAGES_UNPAID.descriptor,
    REVOCATIONS.descriptor,
    REVOCATIONS_PENDING.descriptor,
];

/// What a settled-payment event came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurchaseOutcome {
    Granted,
    AmountMismatch,
    CurrencyMismatch,
    /// The payment of a purchase that had already failed.
    PaymentFailed,
    /// The purchase's other settled event, or a later one: no grant.
    AlreadySettled,
    NoPurchase,
}

impl PurchaseOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::AmountMismatch => "amount_mismatch",
            Self::CurrencyMismatch => "currency_mismatch",
            Self::PaymentFailed => "payment_failed",
            Self::AlreadySettled => "already_settled",
            Self::NoPurchase => "no_purchase",
        }
    }
}

/// A reversal's Stripe kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReversalKind {
    Refund,
    Dispute,
}

/// A pledge transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pledge {
    Authorized,
    Retired,
    Released,
    Expired,
    Consumed,
}

impl Pledge {
    fn label(self) -> &'static str {
        match self {
            Self::Authorized => "authorized",
            Self::Retired => "retired",
            Self::Released => "released",
            Self::Expired => "expired",
            Self::Consumed => "consumed",
        }
    }
}

fn flag(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// What one webhook transaction made true, recorded once it commits.
#[derive(Debug, Default)]
pub struct Facts {
    purchase: Option<PurchaseOutcome>,
    granted_cents: i64,
    reversals: Vec<(ReversalKind, bool)>,
}

impl Facts {
    pub fn purchase(&mut self, outcome: PurchaseOutcome) {
        self.purchase = Some(outcome);
    }

    pub fn granted_cents(&mut self, cents: i64) {
        self.granted_cents = cents.max(0);
    }

    /// A reversal that charged credits back; `shortfall` when part of them
    /// could not be recovered.
    pub fn reversal(&mut self, kind: ReversalKind, shortfall: bool) {
        self.reversals.push((kind, shortfall));
    }

    /// Records the facts. Call it after the commit.
    pub fn record(self) {
        if let Some(outcome) = self.purchase {
            PURCHASES.handle([outcome.label()]).increment(1);
        }
        if self.granted_cents > 0 {
            PURCHASE_USD_CENTS
                .handle([])
                .increment(self.granted_cents as u64);
        }
        for (kind, shortfall) in self.reversals {
            let kind = match kind {
                ReversalKind::Refund => "refund",
                ReversalKind::Dispute => "dispute",
            };
            PURCHASE_REVERSALS
                .handle([kind, flag(shortfall)])
                .increment(1);
        }
    }
}

/// A purchase receipt reached its terminal status (`sent`, `skipped`,
/// `failed`).
pub fn receipt(status: &'static str) {
    RECEIPTS.handle([status]).increment(1);
}

/// `count` pledges made the transition. Call it after the commit.
pub fn pledges(transition: Pledge, count: u64) {
    if count > 0 {
        PLEDGES.handle([transition.label()]).increment(count);
    }
}

/// A CreditUsed log settled into a ledger debit. Call it after the commit.
pub fn settlement(kind: &str, recovered: bool) {
    let kind = if kind == "purchase" {
        "purchase"
    } else {
        "remainder"
    };
    SETTLEMENTS.handle([kind, flag(recovered)]).increment(1);
}

/// One revocation pass's outcomes.
pub fn revocations(sent: usize, confirmed: usize, failed: usize) {
    for (status, n) in [("sent", sent), ("confirmed", confirmed), ("failed", failed)] {
        if n > 0 {
            REVOCATIONS.handle([status]).increment(n as u64);
        }
    }
}

/// The CreditUsed index's readiness and lag, from its pass.
pub fn usage_index(ready: bool, lag_seconds: Option<f64>) {
    USAGE_INDEX_READY
        .handle([])
        .set(if ready { 1.0 } else { 0.0 });
    if let Some(lag) = lag_seconds {
        USAGE_INDEX_LAG.handle([]).set(lag.max(0.0));
    }
}

/// The settlement gauges, from one read per settlement pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub pledges_live: i64,
    pub pledged_usd_cents: i64,
    pub usages_unpaid: i64,
    pub revocations_pending: i64,
}

/// The read behind [`Snapshot`]: live pledges by the partial
/// `authorizations_expiry_idx`, pending revocations by
/// `revocations_due_idx`, and the used credits of this node's signing at or
/// below each chain's published fold with no settlement, as the settlement
/// fold selects them (`settlement::CANDIDATES_SQL`). The live statuses are
/// `authorize::LIVE_STATUSES` (a test holds them equal).
pub(crate) const SNAPSHOT_SQL: &str = "SELECT \
     (SELECT count(*) FROM credits.authorizations WHERE status IN ('authorized', 'retired'))::bigint AS pledges_live, \
     (SELECT COALESCE(sum(usd_cents), 0) FROM credits.authorizations WHERE status IN ('authorized', 'retired'))::bigint AS pledged, \
     (SELECT count(*) FROM credits.usages u \
          JOIN credits.usage_index_cursor c ON c.chain_id = u.chain_id AND c.contract = u.contract \
          JOIN credits.authorizations a ON a.id = u.salt \
          LEFT JOIN credits.usage_settlements s ON s.chain_id = u.chain_id AND s.block_number = u.block_number AND s.log_index = u.log_index \
          WHERE c.folded_block IS NOT NULL AND u.block_number <= c.folded_block AND s.chain_id IS NULL)::bigint AS unpaid, \
     (SELECT count(*) FROM credits.revocations WHERE status IN ('pending', 'sent'))::bigint AS revocations_pending";

impl Snapshot {
    pub async fn read(credits: &Credits) -> Result<Self, ApiError> {
        let row = sqlx::query(SNAPSHOT_SQL).fetch_one(&credits.pool).await?;
        Ok(Self {
            pledges_live: row.get("pledges_live"),
            pledged_usd_cents: row.get("pledged"),
            usages_unpaid: row.get("unpaid"),
            revocations_pending: row.get("revocations_pending"),
        })
    }

    pub fn record(self) {
        PLEDGES_LIVE.handle([]).set(self.pledges_live as f64);
        PLEDGED_USD_CENTS
            .handle([])
            .set(self.pledged_usd_cents as f64);
        USAGES_UNPAID.handle([]).set(self.usages_unpaid as f64);
        REVOCATIONS_PENDING
            .handle([])
            .set(self.revocations_pending as f64);
    }
}
