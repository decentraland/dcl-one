// from: credits-server@d9c6a261:src/controllers/handlers/get-credits.ts, src/controllers/handlers/get-credit-reasons.ts, src/controllers/handlers/get-ledger-stats.ts, src/controllers/handlers/get-purchase-audit.ts, src/controllers/routes.ts
//! `GET /credits` and `GET /credit-reasons` on the credits host (and
//! `/admin/credits`, `/admin/credit-reasons` under `/shop-api`): the ops
//! dashboard's cross-wallet credits list and the reasons its grant filter
//! offers, behind [`require_read_admin`]. The query parameters, defaults and
//! row shape are the old server's; `reports.rs` says where each row comes
//! from. `GET /admin/ledger-stats` and `GET /admin/purchase-audit` answer at
//! the same local path on both mounts, behind the same check
//! (`ledger_stats.rs`, `purchase_audit.rs`).

use axum::Json;
use axum::extract::{OriginalUri, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::credits::SharedState;
use crate::credits::admin::require_read_admin;
use crate::credits::errors::ApiError;
use crate::credits::ledger_stats::LedgerStats;
use crate::credits::purchase_audit::{AUDIT_SORT_KEYS, AuditSort, AuditTransaction};
use crate::credits::reports::{CreditFilters, CreditListRow, CreditOrder, ON_DEMAND_GOAL};
use crate::shared::identity::Address;

/// Every `credit_source` the old server wrote; any other `source` is ignored, as it was.
const KNOWN_SOURCES: &[&str] = &[
    "goal",
    "on_demand",
    "iap",
    "purchase",
    "refund",
    "dispute_won",
    "sale_proceeds",
    "sale_royalty",
    "grant",
    "studio_grant",
    // This node's own: a grant the migration loaded without its old source.
    "migrated",
];

/// Every goal the old server knew; any other `goal_id` is ignored, as it was.
const KNOWN_GOALS: &[&str] = &[
    "log-in",
    "visit-profile",
    "visit-scene",
    "visit-event",
    "visit-event-and-stay",
    "take-photo",
    "visit-location",
    ON_DEMAND_GOAL,
];

#[derive(Debug, Deserialize, Default)]
pub struct CreditListQuery {
    limit: Option<String>,
    offset: Option<String>,
    order_by: Option<String>,
    order_direction: Option<String>,
    source: Option<String>,
    goal_id: Option<String>,
    address: Option<String>,
    granter_address: Option<String>,
    reason: Option<String>,
    season_id: Option<String>,
    min_amount: Option<String>,
    max_amount: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditRecordOut {
    id: String,
    user_address: String,
    /// MANA wei; null on a USD row.
    amount: Option<String>,
    contract: Option<String>,
    /// Milliseconds since the epoch, as a string.
    timestamp: String,
    signature: Option<String>,
    denomination: String,
    usd_cents: Option<i64>,
    season_id: Option<i32>,
    season_name: Option<String>,
    /// Seconds since the epoch, as a string; "0" on a USD row.
    expires_at: String,
    granter_address: Option<String>,
    reason: Option<String>,
    annotation: Option<String>,
    /// MANA wei consumed on chain; "0" on a USD row.
    total_consumed: String,
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    data: Vec<T>,
    total: i64,
}

fn trimmed(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// A decimal amount bound, or nothing: the old server's `Number()` parse
/// dropped anything that was not a number instead of refusing the request.
fn amount_bound(raw: Option<String>) -> Option<String> {
    let s = trimmed(raw)?;
    let (whole, frac) = s.split_once('.').unwrap_or((&s, ""));
    let digits = |p: &str| p.chars().all(|c| c.is_ascii_digit());
    (!whole.is_empty() && whole.len() <= 78 && digits(whole) && digits(frac)).then_some(s)
}

/// The query as the old handler read it, defaults included.
pub(crate) fn credit_filters(q: &CreditListQuery) -> (CreditFilters, CreditOrder, bool) {
    let source = trimmed(q.source.clone()).filter(|s| KNOWN_SOURCES.contains(&s.as_str()));
    let goal = trimmed(q.goal_id.clone()).filter(|g| KNOWN_GOALS.contains(&g.as_str()));
    // With neither a source nor a goal, the list is the operator's grants, as the old
    // dashboard expects; a source drops that default so the source filter stands alone.
    let goal_id = match (&goal, &source) {
        (Some(_), _) => goal,
        (None, Some(_)) => None,
        (None, None) => Some(ON_DEMAND_GOAL.to_string()),
    };
    let filters = CreditFilters {
        address: trimmed(q.address.clone()).map(|a| a.to_lowercase()),
        season_id: trimmed(q.season_id.clone()).and_then(|s| s.parse().ok()),
        goal_id,
        granter_address: trimmed(q.granter_address.clone()).map(|a| a.to_lowercase()),
        reason: trimmed(q.reason.clone()),
        source,
        min_amount: amount_bound(q.min_amount.clone()),
        max_amount: amount_bound(q.max_amount.clone()),
    };
    let order = CreditOrder::parse(q.order_by.as_deref()).unwrap_or(CreditOrder::Timestamp);
    let descending = !q
        .order_direction
        .as_deref()
        .is_some_and(|d| d.trim().eq_ignore_ascii_case("asc"));
    (filters, order, descending)
}

fn record(r: CreditListRow) -> CreditRecordOut {
    CreditRecordOut {
        id: r.id,
        user_address: r.user_address,
        amount: r.amount,
        contract: r.contract,
        timestamp: r.timestamp_ms.to_string(),
        signature: r.signature,
        denomination: r.denomination,
        usd_cents: r.usd_cents,
        season_id: r.season_id,
        season_name: r.season_name,
        expires_at: r.expires_at_secs.to_string(),
        granter_address: r.granter_address,
        reason: r.reason,
        annotation: r.annotation,
        total_consumed: r.total_consumed,
    }
}

/// The credits host's `GET /credits`.
pub async fn list_credits(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    Query(q): Query<CreditListQuery>,
) -> Result<Json<Paginated<CreditRecordOut>>, ApiError> {
    list_credits_at(&state, &original, &headers, &q, "/credits").await
}

/// `/shop-api/admin/credits`: the signature binds its own route.
pub async fn list_credits_canonical(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    Query(q): Query<CreditListQuery>,
) -> Result<Json<Paginated<CreditRecordOut>>, ApiError> {
    list_credits_at(&state, &original, &headers, &q, "/admin/credits").await
}

/// `route` is the mount's own local path: a proxy's `x-original-path` counts
/// only when it is exactly this route ([`require_read_admin`]).
async fn list_credits_at(
    state: &SharedState,
    original: &OriginalUri,
    headers: &HeaderMap,
    q: &CreditListQuery,
    route: &str,
) -> Result<Json<Paginated<CreditRecordOut>>, ApiError> {
    require_read_admin(state, headers, route, original).await?;
    let (filters, order, descending) = credit_filters(q);
    let (limit, offset) = legacy_page(q.limit.as_deref(), q.offset.as_deref());
    let (rows, total) = state
        .credits
        .report_list_credits(&filters, order, descending, limit, offset)
        .await?;
    Ok(Json(Paginated {
        data: rows.into_iter().map(record).collect(),
        total,
    }))
}

#[derive(Debug, Deserialize, Default)]
pub struct ReasonsQuery {
    limit: Option<String>,
    offset: Option<String>,
}

/// The old server's pagination (`@dcl/http-commons`): `limit` defaults to and
/// is capped at 100, `offset` defaults to 0, and a value that is not a
/// number falls back to the default instead of failing the request.
pub(crate) fn legacy_page(limit: Option<&str>, offset: Option<&str>) -> (i64, i64) {
    const MAX: i64 = 100;
    let limit = limit
        .and_then(|l| l.trim().parse::<i64>().ok())
        .filter(|l| *l > 0)
        .map_or(MAX, |l| l.min(MAX));
    let offset = offset
        .and_then(|o| o.trim().parse::<i64>().ok())
        .filter(|o| *o >= 0)
        .unwrap_or(0);
    (limit, offset)
}

/// The credits host's `GET /credit-reasons`.
pub async fn credit_reasons(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    Query(q): Query<ReasonsQuery>,
) -> Result<Json<Paginated<String>>, ApiError> {
    credit_reasons_at(&state, &original, &headers, &q, "/credit-reasons").await
}

/// `/shop-api/admin/credit-reasons`.
pub async fn credit_reasons_canonical(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    Query(q): Query<ReasonsQuery>,
) -> Result<Json<Paginated<String>>, ApiError> {
    credit_reasons_at(&state, &original, &headers, &q, "/admin/credit-reasons").await
}

async fn credit_reasons_at(
    state: &SharedState,
    original: &OriginalUri,
    headers: &HeaderMap,
    q: &ReasonsQuery,
    route: &str,
) -> Result<Json<Paginated<String>>, ApiError> {
    require_read_admin(state, headers, route, original).await?;
    let (limit, offset) = legacy_page(q.limit.as_deref(), q.offset.as_deref());
    let (data, total) = state.credits.report_credit_reasons(limit, offset).await?;
    Ok(Json(Paginated { data, total }))
}

/// `GET /admin/ledger-stats`, at the same local path on the credits host and
/// under `/shop-api`.
pub async fn ledger_stats(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<LedgerStats>, ApiError> {
    require_read_admin(&state, &headers, "/admin/ledger-stats", &original).await?;
    Ok(Json(state.credits.report_ledger_stats().await?))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    limit: Option<String>,
    offset: Option<String>,
    address: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    limit: i64,
    offset: i64,
    /// Rows in this page.
    count: usize,
    /// Transactions the filter matches.
    total: i64,
    sort_by: &'static str,
    sort_direction: &'static str,
    /// Over the returned page only.
    reserved_usd_cents: i64,
    consumed_usd_cents: i64,
    retained_usd_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct AuditOut {
    transactions: Vec<AuditTransaction>,
    page: AuditPage,
}

/// What the audit was asked for, validated as the old server did.
#[derive(Debug, PartialEq)]
pub(crate) struct AuditRequest {
    buyer: Option<String>,
    sort: AuditSort,
    descending: bool,
    limit: i64,
    offset: i64,
}

/// `limit` defaults to 50 and is capped at 200, `offset` at 10,000; a value
/// that is not a positive number falls back. A malformed address, sort key or
/// direction is a 400, never quietly widened or re-sorted.
pub(crate) fn audit_request(q: &AuditQuery) -> Result<AuditRequest, ApiError> {
    let positive = |raw: Option<&str>| {
        raw.and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 1.0)
            .map(|v| v.floor() as i64)
    };
    let limit = positive(q.limit.as_deref()).map_or(50, |l| l.min(200));
    let offset = positive(q.offset.as_deref()).map_or(0, |o| o.min(10_000));
    let buyer = match q.address.as_deref().filter(|a| !a.is_empty()) {
        None => None,
        Some(raw) => Some(
            Address::parse(raw)
                .map_err(|_| ApiError::bad_request("Invalid address"))?
                .as_str()
                .to_string(),
        ),
    };
    let sort = match q.sort_by.as_deref() {
        None => AuditSort::SettledAt,
        Some(raw) => AuditSort::parse(raw).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Invalid sortBy. Valid: {}",
                AUDIT_SORT_KEYS.join(", ")
            ))
        })?,
    };
    let descending = match q.sort_direction.as_deref() {
        None | Some("desc") => true,
        Some("asc") => false,
        Some(_) => {
            return Err(ApiError::bad_request(
                "Invalid sortDirection. Valid: asc, desc",
            ));
        }
    };
    Ok(AuditRequest {
        buyer,
        sort,
        descending,
        limit,
        offset,
    })
}

/// `GET /admin/purchase-audit`, at the same local path on both mounts.
pub async fn purchase_audit(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Result<Json<AuditOut>, ApiError> {
    require_read_admin(&state, &headers, "/admin/purchase-audit", &original).await?;
    let r = audit_request(&q)?;
    let (transactions, total) = state
        .credits
        .report_purchase_audit(r.buyer.as_deref(), r.sort, r.descending, r.limit, r.offset)
        .await?;
    let sum = |f: fn(&AuditTransaction) -> i64| {
        transactions.iter().map(f).fold(0i64, i64::saturating_add)
    };
    let page = AuditPage {
        limit: r.limit,
        offset: r.offset,
        count: transactions.len(),
        total,
        sort_by: r.sort.key(),
        sort_direction: if r.descending { "desc" } else { "asc" },
        reserved_usd_cents: sum(|t| t.reserved_usd_cents),
        consumed_usd_cents: sum(|t| t.consumed_usd_cents),
        retained_usd_cents: sum(|t| t.retained_usd_cents),
    };
    Ok(Json(AuditOut { transactions, page }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pairs: &[(&str, &str)]) -> CreditListQuery {
        let mut q = CreditListQuery::default();
        for (k, v) in pairs {
            let v = Some(v.to_string());
            match *k {
                "order_by" => q.order_by = v,
                "order_direction" => q.order_direction = v,
                "source" => q.source = v,
                "goal_id" => q.goal_id = v,
                "address" => q.address = v,
                "granter_address" => q.granter_address = v,
                "reason" => q.reason = v,
                "season_id" => q.season_id = v,
                "min_amount" => q.min_amount = v,
                "max_amount" => q.max_amount = v,
                other => panic!("unknown key {other}"),
            }
        }
        q
    }

    fn audit(pairs: &[(&str, &str)]) -> Result<AuditRequest, String> {
        let mut q = AuditQuery::default();
        for (k, v) in pairs {
            let v = Some(v.to_string());
            match *k {
                "limit" => q.limit = v,
                "offset" => q.offset = v,
                "address" => q.address = v,
                "sortBy" => q.sort_by = v,
                "sortDirection" => q.sort_direction = v,
                _ => panic!("{k}"),
            }
        }
        audit_request(&q).map_err(|e| e.public_message())
    }

    #[test]
    fn the_audit_request_validates_like_the_old_server() {
        let d = audit(&[]).unwrap();
        assert_eq!(
            (d.limit, d.offset, d.sort, d.descending, d.buyer),
            (50, 0, AuditSort::SettledAt, true, None)
        );
        let r = audit(&[
            ("limit", "12.9"),
            ("offset", "40000"),
            ("sortBy", "items"),
            ("sortDirection", "asc"),
        ])
        .unwrap();
        assert_eq!(
            (r.limit, r.offset, r.sort, r.descending),
            (12, 10_000, AuditSort::Items, false)
        );
        assert_eq!(
            audit(&[("limit", "0"), ("offset", "-1")]).unwrap().limit,
            50
        );
        assert_eq!(audit(&[("limit", "1e9")]).unwrap().limit, 200);
        let a = audit(&[("address", "0xAbCdEf0000000000000000000000000000000001")]).unwrap();
        assert_eq!(
            a.buyer.as_deref(),
            Some("0xabcdef0000000000000000000000000000000001")
        );
        assert_eq!(
            audit(&[("address", "")]).unwrap().buyer,
            None,
            "an empty address is no filter"
        );
        assert_eq!(
            audit(&[("address", "nope")]).unwrap_err(),
            "Invalid address"
        );
        assert!(
            audit(&[("sortBy", "")])
                .unwrap_err()
                .starts_with("Invalid sortBy. Valid: settledAt,")
        );
        assert_eq!(
            audit(&[("sortDirection", "DESC")]).unwrap_err(),
            "Invalid sortDirection. Valid: asc, desc"
        );
    }

    #[test]
    fn pagination_follows_the_old_server() {
        assert_eq!(legacy_page(None, None), (100, 0));
        assert_eq!(legacy_page(Some("25"), Some("50")), (25, 50));
        assert_eq!(legacy_page(Some("500"), None), (100, 0), "capped at 100");
        assert_eq!(
            legacy_page(Some("abc"), Some("-3")),
            (100, 0),
            "garbage falls back"
        );
        assert_eq!(legacy_page(Some("0"), None), (100, 0));
    }

    #[test]
    fn direction_is_case_insensitive() {
        let (_, _, desc) = credit_filters(&query(&[("order_direction", "ASC")]));
        assert!(!desc);
    }

    #[test]
    fn no_source_or_goal_lists_the_operator_grants() {
        let (f, order, desc) = credit_filters(&query(&[]));
        assert_eq!(f.goal_id.as_deref(), Some(ON_DEMAND_GOAL));
        assert_eq!(f.source, None);
        assert_eq!(order, CreditOrder::Timestamp);
        assert!(desc);
    }

    #[test]
    fn a_source_drops_the_goal_default() {
        let (f, _, _) = credit_filters(&query(&[("source", "purchase")]));
        assert_eq!(f.goal_id, None);
        assert_eq!(f.source.as_deref(), Some("purchase"));
    }

    #[test]
    fn unknown_values_are_ignored_as_before() {
        let (f, order, desc) = credit_filters(&query(&[
            ("source", "bogus"),
            ("goal_id", "nope"),
            ("order_by", "x"),
            ("order_direction", "sideways"),
            ("season_id", "abc"),
            ("min_amount", "12e3"),
            ("max_amount", "-5"),
        ]));
        assert_eq!(f.source, None);
        assert_eq!(f.goal_id.as_deref(), Some(ON_DEMAND_GOAL));
        assert_eq!(order, CreditOrder::Timestamp);
        assert!(desc);
        assert_eq!(f.season_id, None);
        assert_eq!(f.min_amount, None);
        assert_eq!(f.max_amount, None);
    }

    #[test]
    fn filters_are_normalised() {
        let (f, order, desc) = credit_filters(&query(&[
            ("address", "  0xABCDEF0000000000000000000000000000000001 "),
            (
                "granter_address",
                "0xAAAA000000000000000000000000000000000002",
            ),
            ("season_id", "7"),
            ("min_amount", "1000"),
            ("max_amount", "2000.5"),
            ("order_by", "amount"),
            ("order_direction", "asc"),
        ]));
        assert_eq!(
            f.address.as_deref(),
            Some("0xabcdef0000000000000000000000000000000001")
        );
        assert_eq!(
            f.granter_address.as_deref(),
            Some("0xaaaa000000000000000000000000000000000002")
        );
        assert_eq!(f.season_id, Some(7));
        assert_eq!(f.min_amount.as_deref(), Some("1000"));
        assert_eq!(f.max_amount.as_deref(), Some("2000.5"));
        assert_eq!(order, CreditOrder::Amount);
        assert!(!desc);
    }
}
