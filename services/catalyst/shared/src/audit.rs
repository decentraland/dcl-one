//! The ONE `ops.admin_audit` writer (`schemas/60_ops.sql`, "ops"): one
//! append-only row per operator mutation anywhere in the tree. A module
//! whose mutation is SQL records inside the same transaction
//! ([`record`] takes any executor, a `&mut PgConnection` of the open
//! transaction included) so the audit row and the write land or fail
//! together; a mutation that is not SQL (a sync pause, a cache flush, an
//! rpc allowlist edit) records after the effect through [`record_after`],
//! which logs and moves on when the insert fails, because the mutation has
//! already been applied and refusing the response would only hide that.
//! `detail` is the request as received with every secret stripped by the
//! caller: bearers, keys and cookies never reach this table.

use serde_json::Value;
use sqlx::{PgExecutor, PgPool};

/// The `domain` values `ops.admin_audit` accepts (its CHECK), one per
/// module that records operator mutations.
pub const AUDIT_DOMAINS: &[&str] = &[
    "content",
    "telemetry",
    "credits",
    "rpc",
    "registry",
    "explore",
    "worlds",
    "social",
    "create",
    "market",
    "economy",
    "ops",
];

/// One operator action. `actor` is the authenticated identity (a console
/// wallet, `token:<name>` for a bearer, `loopback`); `domain` one of
/// [`AUDIT_DOMAINS`] (the table's CHECK); `action` the verb under it
/// (`sync.pause`, `failed-deployments.retry`); `target` the row or key acted
/// on; `result` the outcome (`ok`, `error`, ...).
#[derive(Debug, Clone)]
pub struct Entry<'a> {
    pub actor: &'a str,
    pub domain: &'a str,
    pub action: &'a str,
    pub target: Option<&'a str>,
    pub detail: Value,
    pub result: &'a str,
}

impl<'a> Entry<'a> {
    pub fn new(actor: &'a str, domain: &'a str, action: &'a str) -> Self {
        Self {
            actor,
            domain,
            action,
            target: None,
            detail: Value::Object(Default::default()),
            result: "ok",
        }
    }

    pub fn target(mut self, target: Option<&'a str>) -> Self {
        self.target = target;
        self
    }

    pub fn detail(mut self, detail: Value) -> Self {
        self.detail = detail;
        self
    }

    pub fn result(mut self, result: &'a str) -> Self {
        self.result = result;
        self
    }
}

/// Inserts the row through `executor` and answers its id (a ledger row that
/// moved money names it). A domain outside [`AUDIT_DOMAINS`] is refused
/// here before the CHECK does, with the domain in the message.
pub async fn record<'e, E: PgExecutor<'e>>(executor: E, entry: &Entry<'_>) -> sqlx::Result<i64> {
    if !AUDIT_DOMAINS.contains(&entry.domain) {
        return Err(sqlx::Error::Protocol(format!(
            "ops.admin_audit: unknown domain {:?}",
            entry.domain
        )));
    }
    sqlx::query_scalar(
        "INSERT INTO ops.admin_audit (actor, domain, action, target, detail, result)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(entry.actor)
    .bind(entry.domain)
    .bind(entry.action)
    .bind(entry.target)
    .bind(&entry.detail)
    .bind(entry.result)
    .fetch_one(executor)
    .await
}

/// For a mutation that already took effect outside SQL: the row is best
/// effort, a failure is logged with the action (never the detail).
pub async fn record_after(pool: &PgPool, entry: &Entry<'_>) -> Option<i64> {
    match record(pool, entry).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::error!(
                error = %e,
                actor = %entry.actor,
                action = %format!("{}.{}", entry.domain, entry.action),
                "failed to write the ops.admin_audit row (mutation already applied)"
            );
            None
        }
    }
}
