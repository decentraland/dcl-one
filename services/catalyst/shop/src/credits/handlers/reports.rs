// from: credits-server@d9c6a261:src/controllers/handlers/get-credits.ts, src/controllers/handlers/get-credit-reasons.ts, src/controllers/routes.ts (GET /credits, GET /credit-reasons)
//! `GET /credits` and `GET /credit-reasons` on the credits host (and
//! `/admin/credits`, `/admin/credit-reasons` under `/shop-api`): the ops
//! dashboard's cross-wallet credits list and the reasons its grant filter
//! offers, behind [`require_read_admin`]. The query parameters, defaults and
//! row shape are the old server's; `reports.rs` says where each row comes
//! from.

use axum::Json;
use axum::extract::{OriginalUri, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::credits::SharedState;
use crate::credits::admin::{paginate, require_read_admin};
use crate::credits::errors::ApiError;
use crate::credits::reports::{CreditFilters, CreditListRow, CreditOrder, ON_DEMAND_GOAL};

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
    limit: Option<i64>,
    offset: Option<i64>,
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
    let descending = !matches!(q.order_direction.as_deref(), Some("asc"));
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

/// `route` is the mount's own local path, so a proxy's `x-original-path`
/// only counts when it ends with this route, not with another one that
/// happens to share its last segment.
async fn list_credits_at(
    state: &SharedState,
    original: &OriginalUri,
    headers: &HeaderMap,
    q: &CreditListQuery,
    route: &str,
) -> Result<Json<Paginated<CreditRecordOut>>, ApiError> {
    require_read_admin(state, headers, route, original).await?;
    let (filters, order, descending) = credit_filters(q);
    let (limit, offset) = paginate(q.limit, q.offset);
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
    limit: Option<i64>,
    offset: Option<i64>,
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
    let (limit, offset) = paginate(q.limit, q.offset);
    let (data, total) = state.credits.report_credit_reasons(limit, offset).await?;
    Ok(Json(Paginated { data, total }))
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
