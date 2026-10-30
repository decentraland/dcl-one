//! The `ops.admin_audit` read side (`GET /admin/api/audit`). The ONE writer
//! ([`record`], [`record_after`]) lives in `shared::audit` so every domain
//! can record without depending on ops.

use serde_json::Value;
use sqlx::PgPool;

pub use crate::shared::audit::{Entry, record, record_after};

/// `GET /admin/api/audit`: newest first, each filter one of the table's
/// indexes (`admin_audit_at_idx`, `_domain_action_idx`, `_actor_idx`, the
/// partial `_target_idx`); `before` is the id of the oldest row already
/// seen, the keyset for the next page.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Filter {
    pub domain: Option<String>,
    pub action: Option<String>,
    pub actor: Option<String>,
    pub target: Option<String>,
    pub before: Option<i64>,
    pub limit: Option<i64>,
}

pub const MAX_PAGE: i64 = 500;
pub const DEFAULT_PAGE: i64 = 100;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: i64,
    pub at: chrono::NaiveDateTime,
    pub actor: String,
    pub domain: String,
    pub action: String,
    pub target: Option<String>,
    pub detail: Value,
    pub result: String,
}

pub async fn list(pool: &PgPool, filter: &Filter) -> sqlx::Result<Vec<Row>> {
    let limit = filter.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let blank = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    sqlx::query_as::<_, Row>(
        "SELECT id, at, actor, domain, action, target, detail, result
         FROM ops.admin_audit
         WHERE ($1::text IS NULL OR domain = $1)
           AND ($2::text IS NULL OR action = $2)
           AND ($3::text IS NULL OR actor = $3)
           AND ($4::text IS NULL OR target = $4)
           AND ($5::bigint IS NULL OR id < $5)
         ORDER BY at DESC, id DESC
         LIMIT $6",
    )
    .bind(blank(&filter.domain))
    .bind(blank(&filter.action))
    .bind(blank(&filter.actor))
    .bind(blank(&filter.target))
    .bind(filter.before)
    .bind(limit)
    .fetch_all(pool)
    .await
}
