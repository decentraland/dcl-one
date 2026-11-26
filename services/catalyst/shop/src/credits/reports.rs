// from: credits-server@d9c6a261:src/controllers/handlers/get-credits.ts, src/controllers/handlers/get-credit-reasons.ts, src/adapters/db/db.ts (getCredits, getCreditsWhereClause, getCreditReasons)
//! The ops dashboard's cross-wallet credits list and its grant reasons, read
//! from this node's tables in the old server's row shape:
//!
//! - an operator grant (`credits.ledger` kind `grant` with its
//!   `ops.admin_audit` row): a USD row, source `grant`, goal
//!   `on-demand-granted-credits`, its reason and annotation from the audit
//!   detail, and the granter when the audit actor is a wallet (a bearer grant
//!   records `token:<VARIABLE>`, so it has no granter);
//! - a studio grant, source `studio_grant`, on proof that a studio paid for
//!   it: a studio budget entry names the row (`studio_budget_entries.ledger_id`:
//!   an operator's grant, or a migrated one the budget import linked), or its
//!   audit is credits-server's own record of a studio grant (an imported
//!   audit, `upstreamCreditId`, that carries `studioName`, null when the
//!   studio's name was unavailable). An operator's has the granter, reason and
//!   annotation of its audit row. A `studioName` alone proves nothing: the
//!   admin route once wrote one on grants that spent no budget, and those stay
//!   `grant`;
//! - any other grant migrated from the old server (kind `grant`, no audit
//!   row): a USD row, source `migrated`, no goal, reason or granter. The migration folded
//!   every positive non-purchase credit (grants, studio grants, IAP, sale
//!   proceeds and royalties, won disputes) into `grant` without its source,
//!   so these rows are not claimed as the operator's grants;
//! - a pack purchase (`credits.ledger` kind `purchase`): a USD row, source
//!   `purchase`;
//! - a claimed goal credit (`credits.goal_grants` with `claimed_at`): a MANA row with its season,
//!   contract, signature and expiry, source `goal`, and the MANA its salt
//!   has consumed on chain (`credits.usages`).
//!
//! A USD row's `usdCents` is its credits × 10 (1 credit = 10 cents), null
//! when that is not a whole number of cents within a bigint; its `amount`,
//! `contract` and `signature` are null and its `expiresAt` is `"0"`, since a
//! USD credit has no on-chain credit and does not expire. As in the old
//! server, an amount bound (MANA wei) only ever matches MANA rows. The other
//! sources the old server wrote (`on_demand`, `iap`, `refund`,
//! `dispute_won`, `sale_proceeds`, `sale_royalty`) are accepted as filters
//! and match nothing: this node keeps no rows with them.

use sqlx::Row;

use super::errors::ApiError;
use super::store::Credits;

/// The goal the old dashboard always asks for: the operator's grants.
pub const ON_DEMAND_GOAL: &str = "on-demand-granted-credits";

/// The columns the list can be ordered by, and their SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditOrder {
    Timestamp,
    Amount,
    SeasonId,
    ExpiresAt,
}

impl CreditOrder {
    pub fn parse(raw: Option<&str>) -> Option<Self> {
        match raw {
            None | Some("") | Some("timestamp") => Some(Self::Timestamp),
            Some("amount") => Some(Self::Amount),
            Some("season_id") => Some(Self::SeasonId),
            Some("expires_at") => Some(Self::ExpiresAt),
            Some(_) => None,
        }
    }

    fn column(self) -> &'static str {
        match self {
            Self::Timestamp => "created_at",
            Self::Amount => "amount",
            Self::SeasonId => "season_id",
            Self::ExpiresAt => "expires_at",
        }
    }
}

/// Every filter the old `GET /credits` took; `None` is "no filter".
#[derive(Debug, Default, Clone)]
pub struct CreditFilters {
    pub address: Option<String>,
    pub season_id: Option<i32>,
    pub goal_id: Option<String>,
    pub granter_address: Option<String>,
    pub reason: Option<String>,
    pub source: Option<String>,
    /// MANA wei, as decimal strings checked by the handler.
    pub min_amount: Option<String>,
    pub max_amount: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreditListRow {
    pub id: String,
    pub user_address: String,
    pub amount: Option<String>,
    pub contract: Option<String>,
    pub timestamp_ms: i64,
    pub signature: Option<String>,
    pub denomination: String,
    pub usd_cents: Option<i64>,
    pub season_id: Option<i32>,
    pub season_name: Option<String>,
    pub expires_at_secs: i64,
    pub granter_address: Option<String>,
    pub reason: Option<String>,
    pub annotation: Option<String>,
    pub total_consumed: String,
}

/// The three row sources in one shape. `amount` stays numeric so the amount
/// bounds and the amount order compare numbers, not strings.
fn credit_rows() -> String {
    format!(
        "\
    SELECT 'usd:' || l.id::text AS id, l.address AS user_address, \
           NULL::numeric AS amount, NULL::text AS contract, l.created_at, \
           NULL::text AS signature, 'USD'::text AS denomination, \
           {USD_CENTS} AS usd_cents, NULL::integer AS season_id, \
           NULL::text AS season_name, NULL::timestamp AS expires_at, \
           CASE WHEN a.actor ~ '^0x[0-9a-f]{{40}}$' THEN a.actor END AS granter_address, \
           a.detail->>'reason' AS reason, a.detail->>'annotation' AS annotation, \
           0::numeric AS total_consumed, \
           CASE WHEN EXISTS (SELECT 1 FROM credits.studio_budget_entries e WHERE e.ledger_id = l.id) \
                     OR (a.detail ? 'upstreamCreditId' AND a.detail ? 'studioName') THEN 'studio_grant' \
                WHEN a.id IS NULL THEN 'migrated' ELSE 'grant' END AS source, \
           CASE WHEN a.id IS NOT NULL THEN 'on-demand-granted-credits' END AS goal_id \
    FROM credits.ledger l LEFT JOIN ops.admin_audit a ON a.id = l.admin_audit_id \
    WHERE l.kind = 'grant' \
    UNION ALL \
    SELECT 'purchase:' || l.id::text, l.address, NULL::numeric, NULL::text, l.created_at, \
           NULL::text, 'USD'::text, {USD_CENTS}, NULL::integer, NULL::text, NULL::timestamp, \
           NULL::text, NULL::text, NULL::text, 0::numeric, 'purchase'::text, NULL::text \
    FROM credits.ledger l WHERE l.kind = 'purchase' \
    UNION ALL \
    SELECT 'goal:' || g.salt, g.address, g.amount_wei, g.contract, g.created_at, \
           g.signature, 'MANA'::text, NULL::numeric, g.season_id, s.name, g.expires_at, \
           NULL::text, NULL::text, NULL::text, \
           COALESCE((SELECT SUM(u.value_wei) FROM credits.usages u WHERE u.salt = g.salt), 0), \
           'goal'::text, g.goal_id \
    FROM credits.goal_grants g LEFT JOIN credits.seasons s ON s.id = g.season_id \
    WHERE g.claimed_at IS NOT NULL"
    )
}

/// A ledger amount in cents, or NULL when that is not a whole number of
/// cents that fits a bigint (one absurd row must not fail every page).
const USD_CENTS: &str = "CASE WHEN l.amount * 10 = trunc(l.amount * 10) AND l.amount * 10 < 9e18 THEN (l.amount * 10) END";

const CREDIT_WHERE: &str = "\
    WHERE ($1::text IS NULL OR user_address = $1) \
      AND ($2::integer IS NULL OR season_id = $2) \
      AND ($3::text IS NULL OR goal_id = $3) \
      AND ($4::text IS NULL OR granter_address = $4) \
      AND ($5::text IS NULL OR reason = $5) \
      AND ($6::text IS NULL OR source = $6) \
      AND ($7::numeric IS NULL OR amount >= $7::numeric) \
      AND ($8::numeric IS NULL OR amount <= $8::numeric)";

impl Credits {
    /// One page of the cross-wallet list and the count the filters match.
    pub async fn report_list_credits(
        &self,
        filters: &CreditFilters,
        order: CreditOrder,
        descending: bool,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<CreditListRow>, i64), ApiError> {
        // Every interpolated piece is a constant or comes from a closed enum, never from the request.
        let direction = if descending { "DESC" } else { "ASC" };
        // Nullable columns sort their nulls last either way, and ties break on time,
        // then on the id, so a page boundary is stable across requests.
        let nulls_last = if order == CreditOrder::Timestamp {
            ""
        } else {
            " NULLS LAST"
        };
        let page_sql = format!(
            "SELECT id, user_address, amount::text AS amount, contract, \
                    (extract(epoch FROM created_at) * 1000)::bigint AS timestamp_ms, \
                    signature, denomination, usd_cents::bigint AS usd_cents, season_id, season_name, \
                    COALESCE(extract(epoch FROM expires_at)::bigint, 0) AS expires_at_secs, \
                    granter_address, reason, annotation, total_consumed::text AS total_consumed \
             FROM ({rows}) c {CREDIT_WHERE} \
             ORDER BY {col} {direction}{nulls_last}, created_at {direction}, id {direction} \
             LIMIT $9 OFFSET $10",
            col = order.column(),
            rows = credit_rows(),
        );
        let count_sql = format!(
            "SELECT count(*) FROM ({rows}) c {CREDIT_WHERE}",
            rows = credit_rows()
        );

        // The page and its total read one snapshot, so the count describes the page's view.
        let mut tx = self.read_snapshot().await?;
        let page = sqlx::query(sqlx::AssertSqlSafe(page_sql))
            .bind(filters.address.as_deref())
            .bind(filters.season_id)
            .bind(filters.goal_id.as_deref())
            .bind(filters.granter_address.as_deref())
            .bind(filters.reason.as_deref())
            .bind(filters.source.as_deref())
            .bind(filters.min_amount.as_deref())
            .bind(filters.max_amount.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;
        let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql))
            .bind(filters.address.as_deref())
            .bind(filters.season_id)
            .bind(filters.goal_id.as_deref())
            .bind(filters.granter_address.as_deref())
            .bind(filters.reason.as_deref())
            .bind(filters.source.as_deref())
            .bind(filters.min_amount.as_deref())
            .bind(filters.max_amount.as_deref())
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;

        let rows = page
            .into_iter()
            .map(|r| CreditListRow {
                id: r.get("id"),
                user_address: r.get("user_address"),
                amount: r.get("amount"),
                contract: r.get("contract"),
                timestamp_ms: r.get("timestamp_ms"),
                signature: r.get("signature"),
                denomination: r.get("denomination"),
                usd_cents: r.get("usd_cents"),
                season_id: r.get("season_id"),
                season_name: r.get("season_name"),
                expires_at_secs: r.get("expires_at_secs"),
                granter_address: r.get("granter_address"),
                reason: r.get("reason"),
                annotation: r.get("annotation"),
                total_consumed: r.get("total_consumed"),
            })
            .collect();
        Ok((rows, total))
    }

    /// The distinct reasons the operator's grants carry, alphabetical, one
    /// page and the full count (the old server counted only the page).
    pub async fn report_credit_reasons(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<String>, i64), ApiError> {
        const REASONS: &str = "\
            SELECT DISTINCT a.detail->>'reason' AS reason \
            FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id \
            WHERE l.kind = 'grant' AND a.detail->>'reason' IS NOT NULL";
        let mut tx = self.read_snapshot().await?;
        let page: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT reason FROM ({REASONS}) r ORDER BY reason LIMIT $1 OFFSET $2"
        )))
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await?;
        let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM ({REASONS}) r"
        )))
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((page, total))
    }

    /// A REPEATABLE READ, READ ONLY transaction: every statement in it reads
    /// the same snapshot, and nothing in it can write.
    pub(super) async fn read_snapshot(
        &self,
    ) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        // Each report statement is bounded, as the market's reads are.
        sqlx::query("SET LOCAL statement_timeout = '10s'")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
