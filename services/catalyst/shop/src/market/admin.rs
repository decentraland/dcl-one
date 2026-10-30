//! The operator routes (`/v1/admin/moderation/*`, `/v1/admin/disputes/*`,
//! `/v1/admin/listings/{kind}/{hash}/force-cancel`, `/v1/admin/audit`) over
//! the local `shop.moderation_flags`, `disputes`, `admin_audit` and
//! `trade_cancellations`. Bearer `MARKET_ADMIN_TOKEN` (or `_FILE`); unset,
//! every admin route answers 403 "admin controls disabled". The targets are
//! v2's own listings, `shop.trades.hashed_signature`, and the market
//! federation's log (`fed_actions`, the old `market_*_local` the old routes
//! targeted), so a flag, dispute or force-cancel names a trade or a logged
//! action this node holds; a force-cancel of a trade writes the operator
//! kind of `trade_cancellations`, which the bids and orders status rule
//! reads, and of a logged bid or order the old operator cancellation in the
//! log (signer `operator:<actor>`), which the changes feed carries. The
//! writes sit behind `DATA_WRITES_ENABLED` (the router's gate); the reads
//! answer on a read-only node.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;

use super::{MarketplaceState, SharedState};

pub const ADMIN_TOKEN_ENV: &str = "MARKET_ADMIN_TOKEN";
const ACTOR: &str = "admin-token";
const LIST_LIMIT: i64 = 500;

fn err(code: StatusCode, message: impl Into<String>) -> Response {
    (
        code,
        Json(json!({ "ok": false, "message": message.into() })),
    )
        .into_response()
}

/// The shared operator gate (`crate::auth::admin_bearer`: one header parse,
/// one constant-time compare, closed while unset) with this surface's two
/// old answers: 403 "admin controls disabled" without a configured token,
/// 403 "admin bearer token required" on a missing or wrong one.
pub fn require_admin(state: &MarketplaceState, headers: &HeaderMap) -> Result<String, Response> {
    use crate::auth::admin_bearer::{Refusal, check};
    match check(state.settings.market.admin_token.as_deref(), headers) {
        Ok(()) => Ok(ACTOR.to_string()),
        Err(Refusal::NotConfigured) => Err(err(
            StatusCode::FORBIDDEN,
            format!("admin controls disabled ({ADMIN_TOKEN_ENV} unset)"),
        )),
        Err(Refusal::NoBearer | Refusal::Mismatch) => {
            Err(err(StatusCode::FORBIDDEN, "admin bearer token required"))
        }
    }
}

fn valid_target_kind(kind: &str) -> bool {
    matches!(kind, "bid" | "order" | "trade")
}

fn seconds(t: chrono::NaiveDateTime) -> i64 {
    t.and_utc().timestamp()
}

/// The target exists when this node holds a trade under that hashed
/// signature of the right type (`bid` = bid, `order` = a public order,
/// `trade` = any), or the federation log an action of that kind.
async fn target_exists(
    state: &MarketplaceState,
    kind: &str,
    hash: &str,
) -> Result<bool, sqlx::Error> {
    Ok(trade_target(state, kind, hash).await? || fed_target(state, kind, hash).await?)
}

async fn trade_target(
    state: &MarketplaceState,
    kind: &str,
    hash: &str,
) -> Result<bool, sqlx::Error> {
    let type_filter = match kind {
        "bid" => " AND type = 'bid'",
        "order" => " AND type IN ('public_nft_order', 'public_item_order')",
        "trade" => "",
        _ => return Ok(false),
    };
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM shop.trades WHERE hashed_signature = $1{type_filter})"
    )))
    .bind(hash.to_lowercase())
    .fetch_one(&state.pool)
    .await
}

async fn fed_target(state: &MarketplaceState, kind: &str, hash: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.fed_actions WHERE signature_hash = $1 AND kind = $2)",
    )
    .bind(hash.to_lowercase())
    .bind(kind)
    .fetch_one(&state.pool)
    .await
}

/// The old operator cancellation of a logged bid or order: one `cancel`
/// action per target (a second call answers the first), signed
/// `operator:<actor>`, carried by the changes feed.
async fn force_cancel_logged(
    state: &MarketplaceState,
    actor: &str,
    kind: &str,
    hash: &str,
    reason: &str,
) -> Response {
    let existing: Result<Option<String>, _> = sqlx::query_scalar("SELECT signature_hash FROM shop.fed_actions WHERE kind = 'cancel' AND target_hash = $1 ORDER BY seq LIMIT 1").bind(hash).fetch_optional(&state.pool).await;
    match existing {
        Ok(Some(cancellation_hash)) => return Json(json!({ "ok": true, "target_hash": hash, "cancellation_hash": cancellation_hash, "already_cancelled": true })).into_response(),
        Ok(None) => {}
        Err(e) => return db_error("force_cancel existing check", e),
    }
    let now = chrono::Utc::now().timestamp();
    let cancellation_hash = operator_cancellation_hash(kind, hash, now);
    let payload = json!({ "operator_force_cancel": true, "actor": actor, "reason": reason, "target_kind": kind });
    let res = sqlx::query(
        "INSERT INTO shop.fed_actions (signature_hash, kind, signer, target_hash, target_kind, signed_at, message_payload) \
         VALUES ($1, 'cancel', $2, $3, $4, $5, $6)",
    )
    .bind(&cancellation_hash)
    .bind(format!("operator:{actor}"))
    .bind(hash)
    .bind(kind)
    .bind(now)
    .bind(&payload)
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        return db_error("force_cancel insert", e);
    }
    write_audit(
        state,
        actor,
        "listing.force_cancel",
        kind,
        hash,
        json!({ "reason": reason, "cancellation_hash": cancellation_hash }),
    )
    .await;
    Json(json!({ "ok": true, "target_hash": hash, "cancellation_hash": cancellation_hash }))
        .into_response()
}

fn operator_cancellation_hash(kind: &str, hash: &str, now: i64) -> String {
    let mut h = Sha256::new();
    h.update(b"operator-force-cancel:");
    h.update(kind.as_bytes());
    h.update(b":");
    h.update(hash.as_bytes());
    h.update(b":");
    h.update(now.to_le_bytes());
    format!("operator:{}", hex::encode(h.finalize()))
}

async fn write_audit(
    state: &MarketplaceState,
    actor: &str,
    action: &str,
    target_kind: &str,
    target_hash: &str,
    detail: Value,
) {
    let res = sqlx::query("INSERT INTO shop.admin_audit (actor, action, target_kind, target_hash, detail) VALUES ($1, $2, $3, $4, $5)")
        .bind(actor)
        .bind(action)
        .bind(target_kind)
        .bind(target_hash)
        .bind(detail)
        .execute(&state.pool)
        .await;
    if let Err(e) = res {
        tracing::error!(error = %e, action, target_hash, "admin audit write failed");
    }
}

fn db_error(context: &str, e: sqlx::Error) -> Response {
    tracing::error!(error = %e, context, "admin query failed");
    err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
}

#[derive(Debug, Default, Deserialize)]
pub struct FlagBody {
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

pub async fn set_flag(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((kind, hash)): Path<(String, String)>,
    body: Option<Json<FlagBody>>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !valid_target_kind(&kind) {
        return err(StatusCode::BAD_REQUEST, "kind must be bid|order|trade");
    }
    let b = body.map(|Json(b)| b).unwrap_or_default();
    let severity = b.severity.unwrap_or_else(|| "review".to_string());
    if !matches!(severity.as_str(), "review" | "hide" | "block") {
        return err(
            StatusCode::BAD_REQUEST,
            "severity must be review|hide|block",
        );
    }
    let reason = b.reason.unwrap_or_default();
    match target_exists(&state, &kind, &hash).await {
        Ok(true) => {}
        Ok(false) => return err(StatusCode::NOT_FOUND, "target not found in local log"),
        Err(e) => return db_error("set_flag existence check", e),
    }
    let res = sqlx::query(
        "INSERT INTO shop.moderation_flags (target_hash, target_kind, severity, reason, flagged_by, flagged_at) \
         VALUES ($1, $2, $3, $4, $5, timezone('UTC', now())) \
         ON CONFLICT (target_hash) DO UPDATE SET target_kind = EXCLUDED.target_kind, severity = EXCLUDED.severity, \
           reason = EXCLUDED.reason, flagged_by = EXCLUDED.flagged_by, flagged_at = EXCLUDED.flagged_at",
    )
    .bind(&hash)
    .bind(&kind)
    .bind(&severity)
    .bind(&reason)
    .bind(&actor)
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        return db_error("set_flag upsert", e);
    }
    write_audit(
        &state,
        &actor,
        "flag.set",
        &kind,
        &hash,
        json!({ "severity": severity, "reason": reason }),
    )
    .await;
    Json(json!({ "ok": true, "target_kind": kind, "target_hash": hash, "severity": severity }))
        .into_response()
}

pub async fn clear_flag(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((kind, hash)): Path<(String, String)>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !valid_target_kind(&kind) {
        return err(StatusCode::BAD_REQUEST, "kind must be bid|order|trade");
    }
    let removed = match sqlx::query("DELETE FROM shop.moderation_flags WHERE target_hash = $1")
        .bind(&hash)
        .execute(&state.pool)
        .await
    {
        Ok(r) => r.rows_affected() > 0,
        Err(e) => return db_error("clear_flag delete", e),
    };
    if removed {
        write_audit(&state, &actor, "flag.clear", &kind, &hash, json!({})).await;
    }
    Json(json!({ "ok": true, "target_hash": hash, "removed": removed })).into_response()
}

#[derive(Debug, Default, Deserialize)]
pub struct ListFlagsQuery {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FlagEntry {
    pub target_hash: String,
    pub target_kind: String,
    pub severity: String,
    pub reason: String,
    pub flagged_by: String,
    pub flagged_at: i64,
}

fn list_envelope<T: Serialize>(data: Vec<T>) -> Response {
    let total = data.len();
    Json(json!({ "data": data, "total": total })).into_response()
}

pub async fn list_flags(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<ListFlagsQuery>,
) -> Response {
    if let Err(r) = require_admin(&state, &headers) {
        return r;
    }
    let rows = sqlx::query(
        "SELECT target_hash, target_kind, severity, reason, flagged_by, flagged_at FROM shop.moderation_flags \
         WHERE ($1::text IS NULL OR target_kind = $1) AND ($2::text IS NULL OR severity = $2) \
         ORDER BY flagged_at DESC LIMIT $3",
    )
    .bind(q.kind.as_deref())
    .bind(q.severity.as_deref())
    .bind(LIST_LIMIT)
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => list_envelope(
            rows.iter()
                .map(|r| FlagEntry {
                    target_hash: r.get("target_hash"),
                    target_kind: r.get("target_kind"),
                    severity: r.get("severity"),
                    reason: r.get("reason"),
                    flagged_by: r.get("flagged_by"),
                    flagged_at: seconds(r.get("flagged_at")),
                })
                .collect(),
        ),
        Err(e) => db_error("list_flags", e),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct OpenDisputeBody {
    #[serde(default)]
    pub reason: Option<String>,
}

pub async fn open_dispute(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(trade_hash): Path<String>,
    body: Option<Json<OpenDisputeBody>>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let reason = body.and_then(|Json(b)| b.reason).unwrap_or_default();
    match target_exists(&state, "trade", &trade_hash).await {
        Ok(true) => {}
        Ok(false) => return err(StatusCode::NOT_FOUND, "trade not found in local log"),
        Err(e) => return db_error("open_dispute existence check", e),
    }
    let res = sqlx::query(
        "INSERT INTO shop.disputes (trade_hash, status, reason, opened_by, opened_at) VALUES ($1, 'open', $2, $3, timezone('UTC', now())) \
         ON CONFLICT (trade_hash) DO UPDATE SET status = 'open', reason = EXCLUDED.reason, opened_by = EXCLUDED.opened_by, \
           opened_at = EXCLUDED.opened_at, resolution = '', resolved_by = NULL, resolved_at = NULL",
    )
    .bind(&trade_hash)
    .bind(&reason)
    .bind(&actor)
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        return db_error("open_dispute upsert", e);
    }
    write_audit(
        &state,
        &actor,
        "dispute.open",
        "trade",
        &trade_hash,
        json!({ "reason": reason }),
    )
    .await;
    Json(json!({ "ok": true, "trade_hash": trade_hash, "status": "open" })).into_response()
}

#[derive(Debug, Default, Deserialize)]
pub struct ResolveDisputeBody {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub resolution: Option<String>,
}

pub async fn resolve_dispute(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(trade_hash): Path<String>,
    body: Option<Json<ResolveDisputeBody>>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let b = body.map(|Json(b)| b).unwrap_or_default();
    let status = b.status.unwrap_or_else(|| "resolved".to_string());
    if !matches!(status.as_str(), "resolved" | "rejected") {
        return err(StatusCode::BAD_REQUEST, "status must be resolved|rejected");
    }
    let resolution = b.resolution.unwrap_or_default();
    let updated = match sqlx::query(
        "UPDATE shop.disputes SET status = $2, resolution = $3, resolved_by = $4, resolved_at = timezone('UTC', now()) \
         WHERE trade_hash = $1 AND status = 'open'",
    )
    .bind(&trade_hash)
    .bind(&status)
    .bind(&resolution)
    .bind(&actor)
    .execute(&state.pool)
    .await
    {
        Ok(r) => r.rows_affected() > 0,
        Err(e) => return db_error("resolve_dispute update", e),
    };
    if !updated {
        return err(StatusCode::NOT_FOUND, "no open dispute for this trade");
    }
    write_audit(
        &state,
        &actor,
        "dispute.resolve",
        "trade",
        &trade_hash,
        json!({ "status": status, "resolution": resolution }),
    )
    .await;
    Json(json!({ "ok": true, "trade_hash": trade_hash, "status": status })).into_response()
}

#[derive(Debug, Default, Deserialize)]
pub struct ListDisputesQuery {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DisputeEntry {
    pub trade_hash: String,
    pub status: String,
    pub reason: String,
    pub resolution: String,
    pub opened_by: String,
    pub opened_at: i64,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<i64>,
}

pub async fn list_disputes(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<ListDisputesQuery>,
) -> Response {
    if let Err(r) = require_admin(&state, &headers) {
        return r;
    }
    let rows = sqlx::query(
        "SELECT trade_hash, status, reason, resolution, opened_by, opened_at, resolved_by, resolved_at FROM shop.disputes \
         WHERE ($1::text IS NULL OR status = $1) ORDER BY opened_at DESC LIMIT $2",
    )
    .bind(q.status.as_deref())
    .bind(LIST_LIMIT)
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => list_envelope(
            rows.iter()
                .map(|r| DisputeEntry {
                    trade_hash: r.get("trade_hash"),
                    status: r.get("status"),
                    reason: r.get("reason"),
                    resolution: r.get("resolution"),
                    opened_by: r.get("opened_by"),
                    opened_at: seconds(r.get("opened_at")),
                    resolved_by: r.get("resolved_by"),
                    resolved_at: r
                        .get::<Option<chrono::NaiveDateTime>, _>("resolved_at")
                        .map(seconds),
                })
                .collect(),
        ),
        Err(e) => db_error("list_disputes", e),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ForceCancelBody {
    #[serde(default)]
    pub reason: Option<String>,
}

/// The operator's cancellation record (`kind = 'operator'`) keyed by the
/// target's hashed signature; a second call answers the existing one.
pub async fn force_cancel(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((kind, hash)): Path<(String, String)>,
    body: Option<Json<ForceCancelBody>>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !matches!(kind.as_str(), "bid" | "order") {
        return err(
            StatusCode::BAD_REQUEST,
            "kind must be bid|order (trades cannot be force-cancelled)",
        );
    }
    let reason = body.and_then(|Json(b)| b.reason).unwrap_or_default();
    let hash = hash.to_lowercase();
    match trade_target(&state, &kind, &hash).await {
        Ok(true) => {}
        Ok(false) => match fed_target(&state, &kind, &hash).await {
            Ok(true) => return force_cancel_logged(&state, &actor, &kind, &hash, &reason).await,
            Ok(false) => return err(StatusCode::NOT_FOUND, "target not found in local log"),
            Err(e) => return db_error("force_cancel existence check", e),
        },
        Err(e) => return db_error("force_cancel existence check", e),
    }
    let existing: Result<Option<(Option<String>, String)>, _> = sqlx::query_as(
        "SELECT signature, kind FROM shop.trade_cancellations WHERE target_hashed_signature = $1",
    )
    .bind(&hash)
    .fetch_optional(&state.pool)
    .await;
    match existing {
        Ok(Some((signature, existing_kind))) => {
            let cancellation_hash = signature.unwrap_or_else(|| format!("{existing_kind}:{hash}"));
            return Json(json!({ "ok": true, "target_hash": hash, "cancellation_hash": cancellation_hash, "already_cancelled": true })).into_response();
        }
        Ok(None) => {}
        Err(e) => return db_error("force_cancel existing check", e),
    }
    let cancellation_hash =
        operator_cancellation_hash(&kind, &hash, chrono::Utc::now().timestamp());
    let res = sqlx::query(
        "INSERT INTO shop.trade_cancellations (target_hashed_signature, kind, signer, signature, reason) \
         VALUES ($1, 'operator', $2, $3, $4)",
    )
    .bind(&hash)
    .bind(format!("operator:{actor}"))
    .bind(&cancellation_hash)
    .bind(&reason)
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        return db_error("force_cancel insert", e);
    }
    write_audit(
        &state,
        &actor,
        "listing.force_cancel",
        &kind,
        &hash,
        json!({ "reason": reason, "cancellation_hash": cancellation_hash }),
    )
    .await;
    state.catalog_cache.invalidate();
    Json(json!({ "ok": true, "target_hash": hash, "cancellation_hash": cancellation_hash }))
        .into_response()
}

#[derive(Debug, Default, Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub target_hash: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub target_kind: String,
    pub target_hash: String,
    pub detail: Value,
    pub created_at: i64,
}

pub async fn list_audit(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Response {
    if let Err(r) = require_admin(&state, &headers) {
        return r;
    }
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    let rows = sqlx::query(
        "SELECT id, actor, action, target_kind, target_hash, detail, created_at FROM shop.admin_audit \
         WHERE ($1::text IS NULL OR target_hash = $1) AND ($2::text IS NULL OR action = $2) ORDER BY id DESC LIMIT $3",
    )
    .bind(q.target_hash.as_deref())
    .bind(q.action.as_deref())
    .bind(limit)
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => list_envelope(
            rows.iter()
                .map(|r| AuditEntry {
                    id: r.get("id"),
                    actor: r.get("actor"),
                    action: r.get("action"),
                    target_kind: r.get("target_kind"),
                    target_hash: r.get("target_hash"),
                    detail: r.get("detail"),
                    created_at: seconds(r.get("created_at")),
                })
                .collect(),
        ),
        Err(e) => db_error("list_audit", e),
    }
}

/// Whether `signer` cannot be the signer of an operator record: the wire
/// `signer` column holds `operator:<actor>` for a force-cancel.
pub fn is_operator_signer(signer: &str) -> bool {
    signer.starts_with("operator:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn target_kind_validation_and_envelopes() {
        assert!(
            valid_target_kind("bid") && valid_target_kind("order") && valid_target_kind("trade")
        );
        assert!(!valid_target_kind("listing") && !valid_target_kind(""));
        let entry = FlagEntry {
            target_hash: "0xabc".into(),
            target_kind: "order".into(),
            severity: "review".into(),
            reason: "spam".into(),
            flagged_by: "admin-token".into(),
            flagged_at: 1_700_000_000,
        };
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            json!({ "target_hash": "0xabc", "target_kind": "order", "severity": "review", "reason": "spam", "flagged_by": "admin-token", "flagged_at": 1_700_000_000_i64 })
        );
        let open = DisputeEntry {
            trade_hash: "0xtrade".into(),
            status: "open".into(),
            reason: "fraud".into(),
            resolution: String::new(),
            opened_by: "admin-token".into(),
            opened_at: 1_700_000_000,
            resolved_by: None,
            resolved_at: None,
        };
        let v = serde_json::to_value(&open).unwrap();
        assert!(
            v.as_object().unwrap().contains_key("resolved_by") && v["resolved_at"].is_null(),
            "null keys stay on the wire"
        );
        assert!(is_operator_signer("operator:admin-token"));
    }

    #[tokio::test]
    async fn the_bearer_gate_answers_like_the_old_one() {
        let state = crate::market::tests::support::state_without_db();
        let none = HeaderMap::new();
        let refused = require_admin(&state, &none).unwrap_err();
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
        let mut settings = super::super::Settings::for_tests();
        settings.market.admin_token = Some("s3cr3t".into());
        let state = crate::market::tests::support::state_without_db_settings(settings);
        assert_eq!(
            require_admin(&state, &none).unwrap_err().status(),
            StatusCode::FORBIDDEN
        );
        let mut wrong = HeaderMap::new();
        wrong.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer nope"),
        );
        assert_eq!(
            require_admin(&state, &wrong).unwrap_err().status(),
            StatusCode::FORBIDDEN
        );
        let mut lower = HeaderMap::new();
        lower.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("bearer s3cr3t"),
        );
        assert!(
            require_admin(&state, &lower).is_err(),
            "the exact 'Bearer ' prefix, as twenty of the old gates required"
        );
        let mut right = HeaderMap::new();
        right.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer s3cr3t"),
        );
        assert_eq!(require_admin(&state, &right).unwrap(), "admin-token");
    }
}
