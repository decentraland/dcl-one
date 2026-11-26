// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/adapters/studios-db.ts, src/controllers/handlers/admin-studios.ts (the validation and the error codes), src/migrations/1786700000000_studios.ts
//! Studios: scene studios that give Credits to their players out of a budget
//! an operator assigns (`credits.studios`, `credits.studio_operators`,
//! `credits.studio_budget_entries`). This module keeps the studio, the
//! wallets that act for it and its budget ledger, in USD cents (the API
//! takes whole Credits; 1 Credit = 10 cents).
//!
//! A studio has no limits of its own: it can give all of its budget. What
//! bounds the money is the budget, and new budget (any positive entry) is
//! bounded by a service-wide daily ceiling (`CREDITS_DAILY_STUDIO_DEPOSIT_LIMIT_CENTS`,
//! fail-closed: unset refuses new budget), so a leaked admin token or a bug
//! cannot conjure more than a day's ceiling to give away.
//!
//! Every write that changes a studio's operators or budget holds the
//! studio's row lock; new budget first takes one service-wide advisory lock,
//! with no date in it (the database clock is read once the locks are held,
//! and that one reading is both the day the ceiling counts and the time the
//! entry records). The budget transaction pins READ COMMITTED whatever the
//! session default, so each statement reads a snapshot taken after the lock
//! was granted, and a balance or a day's total is never read from before the
//! previous holder committed. A studio's page reads from one REPEATABLE READ,
//! READ ONLY snapshot, so its figures and its history agree.
//!
//! Every refusal carries a stable `code` next to its message, as
//! credits-server answers them. The grants a studio's operators give out of
//! its budget are `studio_grants.rs`.

use std::sync::LazyLock;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};

use super::alerts::{grouped, usd};
use super::errors::ApiError;
use super::store::{AuditEntry, Credits};

/// US cents per Credit (the 0.10 USD peg).
pub const USD_CENTS_PER_CREDIT: i64 = 10;
/// The largest budget change, either way: 10,000,000 Credits ($1,000,000). The
/// daily ceiling refuses far less; this keeps the arithmetic exact.
pub const MAX_CREDITS_PER_BUDGET_CHANGE: i64 = 10_000_000;
/// The longest studio name.
pub const MAX_NAME_CHARS: usize = 80;
/// The longest budget note.
pub const MAX_NOTE_CHARS: usize = 200;
/// A history page's default and largest size.
pub const DEFAULT_PAGE_SIZE: i64 = 50;
pub const MAX_PAGE_SIZE: i64 = 200;

/// The one lock every change that adds budget takes (credits-server's
/// `STUDIO_DEPOSITS_LOCK_ID`: the first 15 hex digits of the SHA-256 of
/// `studio-budget-deposits`).
static DEPOSITS_LOCK_ID: LazyLock<i64> = LazyLock::new(|| {
    let digest = hex::encode(Sha256::digest(b"studio-budget-deposits"));
    i64::from_str_radix(&digest[..15], 16).expect("15 hex digits fit an i64")
});

/// A refusal of a studio route: its status, message and stable code.
#[derive(Debug)]
pub enum StudioError {
    /// The admin gate's refusal, answered as the gate answers it.
    Refused(ApiError),
    Invalid(String),
    NotFound,
    NameTaken(String),
    OperatorNotFound,
    BudgetExceeded {
        balance_cents: i64,
    },
    IdempotencyKeyReused(String),
    DailyDepositLimitExceeded {
        deposited_today_cents: i64,
        requested_cents: i64,
        limit_cents: i64,
    },
    DepositsNotConfigured,
    /// A grant from a paused studio, named.
    Paused(String),
    /// A grant to a wallet that acts for the studio.
    RecipientIsOperator(String),
    /// A grant to a wallet flagged for multi-accounting; the reason is never
    /// answered, so a studio cannot learn which wallets carry a flag.
    RecipientNotEligible,
    GrantLimitExceeded {
        max_cents: i64,
        requested_cents: i64,
    },
    /// No per-grant ceiling is configured (`CREDITS_STUDIO_GRANT_MAX_CENTS`).
    GrantsNotConfigured,
    Internal(String),
}

impl From<sqlx::Error> for StudioError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(format!("database: {e}"))
    }
}

impl From<ApiError> for StudioError {
    fn from(e: ApiError) -> Self {
        Self::Internal(e.to_string())
    }
}

/// `$12.34`, as credits-server describes an amount of cents.
fn describe_usd_cents(cents: i64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// `1,000 credits ($100.00)`, as credits-server describes a studio grant.
fn describe_grant_cents(cents: i64) -> String {
    let credits = match cents % USD_CENTS_PER_CREDIT {
        0 => grouped(cents / USD_CENTS_PER_CREDIT),
        tenths => format!("{}.{}", grouped(cents / USD_CENTS_PER_CREDIT), tenths.abs()),
    };
    format!("{credits} credits ({})", usd(cents))
}

impl StudioError {
    fn status_code_message(&self) -> (StatusCode, &'static str, String) {
        match self {
            // `into_response` answers a refusal as the gate answers it.
            Self::Refused(_) => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Unauthorized".into(),
            ),
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, "INVALID_REQUEST", message.clone()),
            // A wallet that does not act for a studio is told it does not exist.
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "STUDIO_NOT_FOUND",
                "Studio not found".into(),
            ),
            Self::NameTaken(name) => (
                StatusCode::CONFLICT,
                "STUDIO_NAME_TAKEN",
                format!("A studio named \"{name}\" already exists"),
            ),
            Self::OperatorNotFound => (
                StatusCode::NOT_FOUND,
                "OPERATOR_NOT_FOUND",
                "That wallet is not an operator of this studio".into(),
            ),
            Self::BudgetExceeded { balance_cents } => (
                StatusCode::CONFLICT,
                "STUDIO_BUDGET_EXCEEDED",
                format!(
                    "That would take the studio's budget below zero (balance: {balance_cents} cents)"
                ),
            ),
            Self::IdempotencyKeyReused(key) => (
                StatusCode::CONFLICT,
                "IDEMPOTENCY_KEY_REUSED",
                format!("Idempotency key \"{key}\" was already used for a different request"),
            ),
            Self::DailyDepositLimitExceeded {
                deposited_today_cents,
                requested_cents,
                limit_cents,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                "DAILY_DEPOSIT_LIMIT_EXCEEDED",
                format!(
                    "Daily studio deposit limit exceeded: {} already added today, {} requested, limit {}",
                    describe_usd_cents(*deposited_today_cents),
                    describe_usd_cents(*requested_cents),
                    describe_usd_cents(*limit_cents)
                ),
            ),
            Self::DepositsNotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPOSITS_NOT_CONFIGURED",
                "Studio deposits are not available".into(),
            ),
            Self::Paused(name) => (
                StatusCode::CONFLICT,
                "STUDIO_PAUSED",
                format!("Studio \"{name}\" is paused and cannot grant credits"),
            ),
            Self::RecipientIsOperator(address) => (
                StatusCode::FORBIDDEN,
                "STUDIO_OPERATOR",
                format!("{address} is an operator of this studio and cannot receive its credits"),
            ),
            Self::RecipientNotEligible => (
                StatusCode::CONFLICT,
                "RECIPIENT_NOT_ELIGIBLE",
                "This account cannot receive Credits from a studio".into(),
            ),
            Self::GrantLimitExceeded {
                max_cents,
                requested_cents,
            } => (
                StatusCode::CONFLICT,
                "GRANT_LIMIT_EXCEEDED",
                format!(
                    "A single studio grant may give at most {}; {} was requested",
                    describe_grant_cents(*max_cents),
                    describe_grant_cents(*requested_cents)
                ),
            ),
            Self::GrantsNotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "GRANTS_NOT_CONFIGURED",
                "Studio grants are not available".into(),
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Internal server error".into(),
            ),
        }
    }
}

impl IntoResponse for StudioError {
    fn into_response(self) -> Response {
        if let Self::Refused(refusal) = self {
            return refusal.into_response();
        }
        match &self {
            Self::Internal(detail) => tracing::error!(error = %detail, "a studio route failed"),
            Self::DepositsNotConfigured => tracing::error!(
                "refused new studio budget: CREDITS_DAILY_STUDIO_DEPOSIT_LIMIT_CENTS is unset or negative"
            ),
            Self::GrantsNotConfigured => tracing::error!(
                "refused a studio grant: CREDITS_STUDIO_GRANT_MAX_CENTS is unset or negative"
            ),
            _ => {}
        }
        let (status, code, error) = self.status_code_message();
        (status, Json(json!({ "error": error, "code": code }))).into_response()
    }
}

/// Unicode format characters (`Cf`): invisible characters such as zero-width
/// spaces and direction overrides, which could make two names look the same.
fn is_format_char(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// Text an operator types that is shown on one line: no control (`Cc`) or
/// format (`Cf`) character, which could fake another field or two equal names.
pub(super) fn is_one_printable_line(text: &str) -> bool {
    !text.chars().any(|c| c.is_control() || is_format_char(c))
}

/// A studio id from the path, lowercase; checked up front, since PostgreSQL
/// refuses a malformed uuid with an error rather than a miss.
pub fn parse_studio_id(raw: &str) -> Result<String, StudioError> {
    let bytes = raw.as_bytes();
    let shaped = bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        });
    if !shaped {
        return Err(StudioError::Invalid("Invalid studio id".into()));
    }
    Ok(raw.to_ascii_lowercase())
}

/// A studio name: required, trimmed, one printable line of at most
/// [`MAX_NAME_CHARS`].
pub fn parse_name(value: Option<&JsonValue>) -> Result<String, StudioError> {
    let name = value
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| StudioError::Invalid("\"name\" is required".into()))?;
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(StudioError::Invalid(format!(
            "\"name\" must be at most {MAX_NAME_CHARS} characters"
        )));
    }
    if !is_one_printable_line(name) {
        return Err(StudioError::Invalid(
            "\"name\" must be a single line of printable characters".into(),
        ));
    }
    Ok(name.to_string())
}

/// An optional note: trimmed, one printable line of at most
/// [`MAX_NOTE_CHARS`]; blank is none.
pub fn parse_note(value: Option<&JsonValue>) -> Result<Option<String>, StudioError> {
    let note = match value {
        None | Some(JsonValue::Null) => return Ok(None),
        Some(JsonValue::String(note)) => note.trim(),
        Some(_) => return Err(StudioError::Invalid("\"note\" must be a string".into())),
    };
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(StudioError::Invalid(format!(
            "\"note\" must be at most {MAX_NOTE_CHARS} characters"
        )));
    }
    if !is_one_printable_line(note) {
        return Err(StudioError::Invalid(
            "\"note\" must be a single line of printable characters".into(),
        ));
    }
    Ok((!note.is_empty()).then(|| note.to_string()))
}

/// A non-zero whole number of Credits, at most
/// [`MAX_CREDITS_PER_BUDGET_CHANGE`] either way.
pub fn parse_credits_change(value: Option<&JsonValue>) -> Result<i64, StudioError> {
    value
        .and_then(JsonValue::as_i64)
        // A range, not `abs`: `i64::MIN` has no absolute value.
        .filter(|c| {
            *c != 0 && (-MAX_CREDITS_PER_BUDGET_CHANGE..=MAX_CREDITS_PER_BUDGET_CHANGE).contains(c)
        })
        .ok_or_else(|| {
            StudioError::Invalid(format!(
                "\"credits\" must be a non-zero whole number of at most {MAX_CREDITS_PER_BUDGET_CHANGE} either way"
            ))
        })
}

/// The caller's key for a write: 8 to 128 letters, digits or `._:-`.
pub fn parse_idempotency_key(value: Option<&JsonValue>) -> Result<String, StudioError> {
    value
        .and_then(JsonValue::as_str)
        .filter(|k| {
            (8..=128).contains(&k.len())
                && k.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
        })
        .map(str::to_string)
        .ok_or_else(|| {
            StudioError::Invalid(
                "\"idempotencyKey\" is required: 8 to 128 letters, digits or ._:- (a new UUID per operation will do)"
                    .into(),
            )
        })
}

/// A studio with its budget figures, read from the ledger in one statement.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StudioSummary {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_by: String,
    /// Epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
    pub balance_cents: i64,
    pub deposited_cents: i64,
    pub granted_cents: i64,
    pub grant_count: i64,
    pub operator_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StudioOperator {
    pub address: String,
    pub added_by: String,
    pub created_at: i64,
}

/// One budget entry; a grant names the ledger row it credited
/// (`creditId`) and the wallet that received it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StudioBudgetEntry {
    pub id: String,
    pub kind: String,
    pub usd_cents: i64,
    pub note: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    pub credit_id: Option<String>,
    pub recipient: Option<String>,
}

/// A studio's page: its figures, operators and a page of its history with the
/// history's total, from one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioDetail {
    pub studio: StudioSummary,
    pub operators: Vec<StudioOperator>,
    pub entries: Vec<StudioBudgetEntry>,
    pub total: i64,
}

/// What a budget change wrote, or what its key already wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetChange {
    pub entry_id: String,
    pub studio_name: String,
    /// The balance now: after this change, or, on a replay, as it stands.
    pub balance_cents: i64,
    pub replayed: bool,
}

/// A budget change an operator asks for.
pub struct BudgetChangeRequest<'a> {
    pub studio_id: &'a str,
    /// `deposit` or `adjustment`.
    pub kind: &'a str,
    pub usd_cents: i64,
    pub note: Option<&'a str>,
    pub idempotency_key: &'a str,
    /// The ceiling on the day's new budget; `None` refuses new budget.
    pub daily_deposit_limit_cents: Option<i64>,
    pub actor: &'a str,
}

/// The studio with its ledger and operator figures, aggregated so one row
/// comes back per studio.
macro_rules! summary_select {
    () => {
        "\
    SELECT s.id::text AS id, s.name, s.status, s.created_by, \
           (EXTRACT(EPOCH FROM s.created_at) * 1000)::bigint AS created_at, \
           (EXTRACT(EPOCH FROM s.updated_at) * 1000)::bigint AS updated_at, \
           COALESCE(b.balance, 0)::bigint AS balance_cents, \
           COALESCE(b.deposited, 0)::bigint AS deposited_cents, \
           COALESCE(b.granted, 0)::bigint AS granted_cents, \
           COALESCE(b.grants, 0)::bigint AS grant_count, \
           COALESCE(o.operators, 0)::bigint AS operator_count \
    FROM credits.studios s \
    LEFT JOIN ( \
        SELECT studio_id, SUM(usd_cents) AS balance, \
               SUM(usd_cents) FILTER (WHERE kind = 'deposit') AS deposited, \
               -SUM(usd_cents) FILTER (WHERE kind = 'grant') AS granted, \
               COUNT(*) FILTER (WHERE kind = 'grant') AS grants \
        FROM credits.studio_budget_entries GROUP BY studio_id \
    ) b ON b.studio_id = s.id \
    LEFT JOIN ( \
        SELECT studio_id, COUNT(*) AS operators FROM credits.studio_operators GROUP BY studio_id \
    ) o ON o.studio_id = s.id"
    };
}

/// [`summary_select!`] for the other studio modules.
pub(super) const SUMMARY_SELECT: &str = summary_select!();

pub(super) fn map_summary(row: &sqlx::postgres::PgRow) -> StudioSummary {
    StudioSummary {
        id: row.get("id"),
        name: row.get("name"),
        status: row.get("status"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        balance_cents: row.get("balance_cents"),
        deposited_cents: row.get("deposited_cents"),
        granted_cents: row.get("granted_cents"),
        grant_count: row.get("grant_count"),
        operator_count: row.get("operator_count"),
    }
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// A stable fingerprint of what a budget request asked for, kept next to its
/// idempotency key: a JSON array, whose order no serializer setting changes.
fn budget_request_hash(kind: &str, usd_cents: i64, note: Option<&str>) -> String {
    let request = json!(["budget", kind, usd_cents, note]);
    hex::encode(Sha256::digest(request.to_string().as_bytes()))
}

/// Takes the studio's row lock on `tx`; `NotFound` when there is none.
/// Returns the studio's name.
async fn lock_studio(tx: &mut PgConnection, studio_id: &str) -> Result<String, StudioError> {
    sqlx::query_scalar("SELECT name FROM credits.studios WHERE id = $1::uuid FOR UPDATE")
        .bind(studio_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StudioError::NotFound)
}

/// The studio's balance in cents, read on `tx` under its lock.
pub(super) async fn balance_cents(
    tx: &mut PgConnection,
    studio_id: &str,
) -> Result<i64, StudioError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(SUM(usd_cents), 0)::bigint FROM credits.studio_budget_entries WHERE studio_id = $1::uuid",
    )
    .bind(studio_id)
    .fetch_one(&mut *tx)
    .await?)
}

impl Credits {
    /// The studio, its operators and a page of its budget history, newest
    /// first, all read from one snapshot, so the balance, the entries and
    /// their total agree even while a change commits between the reads.
    pub async fn studio_detail(
        &self,
        studio_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<StudioDetail>, StudioError> {
        let mut tx = self.read_snapshot().await?;
        let row = sqlx::query(concat!(summary_select!(), " WHERE s.id = $1::uuid"))
            .bind(studio_id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(studio) = row.as_ref().map(map_summary) else {
            tx.commit().await?;
            return Ok(None);
        };
        let operators = Self::operators_on(&mut tx, studio_id).await?;
        let (entries, total) = Self::entries_on(&mut tx, studio_id, limit, offset).await?;
        tx.commit().await?;
        Ok(Some(StudioDetail {
            studio,
            operators,
            entries,
            total,
        }))
    }

    pub async fn studio(&self, studio_id: &str) -> Result<Option<StudioSummary>, StudioError> {
        let row = sqlx::query(concat!(summary_select!(), " WHERE s.id = $1::uuid"))
            .bind(studio_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(map_summary))
    }

    pub async fn studios(&self) -> Result<Vec<StudioSummary>, StudioError> {
        let rows = sqlx::query(concat!(summary_select!(), " ORDER BY lower(s.name), s.id"))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(map_summary).collect())
    }

    /// Creates an active studio with an empty budget.
    pub async fn create_studio(
        &self,
        name: &str,
        actor: &str,
    ) -> Result<StudioSummary, StudioError> {
        let mut tx = self.pool.begin().await?;
        let id: String = match sqlx::query_scalar(
            "INSERT INTO credits.studios (name, created_by) VALUES ($1, $2) RETURNING id::text",
        )
        .bind(name)
        .bind(actor)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(id) => id,
            Err(e) if is_unique_violation(&e) => return Err(StudioError::NameTaken(name.into())),
            Err(e) => return Err(e.into()),
        };
        Self::audit(
            &mut *tx,
            AuditEntry {
                action: "studio.create",
                actor,
                target: Some(&id),
                detail: &json!({ "name": name }),
            },
        )
        .await?;
        tx.commit().await?;
        self.studio(&id).await?.ok_or(StudioError::NotFound)
    }

    /// Renames, pauses or resumes a studio (a paused studio keeps its budget
    /// but cannot grant).
    pub async fn update_studio(
        &self,
        studio_id: &str,
        name: Option<&str>,
        status: Option<&str>,
        actor: &str,
    ) -> Result<StudioSummary, StudioError> {
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE credits.studios SET updated_at = timezone('UTC', now()), \
                 name = COALESCE($2, name), status = COALESCE($3, status) \
             WHERE id = $1::uuid",
        )
        .bind(studio_id)
        .bind(name)
        .bind(status)
        .execute(&mut *tx)
        .await;
        match updated {
            Ok(done) if done.rows_affected() == 0 => return Err(StudioError::NotFound),
            Ok(_) => {}
            Err(e) if is_unique_violation(&e) => {
                return Err(StudioError::NameTaken(name.unwrap_or_default().into()));
            }
            Err(e) => return Err(e.into()),
        }
        Self::audit(
            &mut *tx,
            AuditEntry {
                action: "studio.update",
                actor,
                target: Some(studio_id),
                detail: &json!({ "name": name, "status": status }),
            },
        )
        .await?;
        tx.commit().await?;
        self.studio(studio_id).await?.ok_or(StudioError::NotFound)
    }

    async fn operators_on(
        conn: &mut PgConnection,
        studio_id: &str,
    ) -> Result<Vec<StudioOperator>, StudioError> {
        let rows = sqlx::query(
            "SELECT address, added_by, (EXTRACT(EPOCH FROM created_at) * 1000)::bigint AS created_at \
             FROM credits.studio_operators WHERE studio_id = $1::uuid ORDER BY created_at, address",
        )
        .bind(studio_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .iter()
            .map(|r| StudioOperator {
                address: r.get("address"),
                added_by: r.get("added_by"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    /// Lets `address` act for the studio; adding one that already does is not
    /// an error. Under the studio's lock, the one a grant holds while it reads
    /// who acts for the studio.
    pub async fn add_studio_operator(
        &self,
        studio_id: &str,
        address: &str,
        actor: &str,
    ) -> Result<bool, StudioError> {
        let mut tx = self.pool.begin().await?;
        lock_studio(&mut tx, studio_id).await?;
        let added = sqlx::query(
            "INSERT INTO credits.studio_operators (studio_id, address, added_by) VALUES ($1::uuid, $2, $3) \
             ON CONFLICT (studio_id, address) DO NOTHING",
        )
        .bind(studio_id)
        .bind(address)
        .bind(actor)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if added {
            Self::audit(
                &mut *tx,
                AuditEntry {
                    action: "studio.operator.add",
                    actor,
                    target: Some(studio_id),
                    detail: &json!({ "address": address }),
                },
            )
            .await?;
        }
        tx.commit().await?;
        Ok(added)
    }

    /// Stops `address` acting for the studio; `false` when it did not.
    pub async fn remove_studio_operator(
        &self,
        studio_id: &str,
        address: &str,
        actor: &str,
    ) -> Result<bool, StudioError> {
        let mut tx = self.pool.begin().await?;
        lock_studio(&mut tx, studio_id).await?;
        let removed = sqlx::query(
            "DELETE FROM credits.studio_operators WHERE studio_id = $1::uuid AND address = $2",
        )
        .bind(studio_id)
        .bind(address)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if removed {
            Self::audit(
                &mut *tx,
                AuditEntry {
                    action: "studio.operator.remove",
                    actor,
                    target: Some(studio_id),
                    detail: &json!({ "address": address }),
                },
            )
            .await?;
        }
        tx.commit().await?;
        Ok(removed)
    }

    /// Records a deposit or an adjustment. A repeat of the key with the same
    /// request answers what it wrote, before any ceiling or balance check
    /// (even with the ceiling reached or unset); with another request it is
    /// refused. New budget is counted against the day's ceiling (the UTC day
    /// of the database clock, read under the deposits lock); an adjustment may
    /// never take the balance below zero.
    pub async fn record_studio_budget_change(
        &self,
        req: BudgetChangeRequest<'_>,
    ) -> Result<BudgetChange, StudioError> {
        let adds_budget = req.usd_cents > 0;
        let request_hash = budget_request_hash(req.kind, req.usd_cents, req.note);
        let mut tx = self.pool.begin().await?;
        // Each statement must see what the previous lock holder committed, so
        // the level is pinned rather than inherited from the session default:
        // under REPEATABLE READ a waiter's snapshot could predate it.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        if adds_budget {
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(*DEPOSITS_LOCK_ID)
                .execute(&mut *tx)
                .await?;
        }
        let studio_name = lock_studio(&mut tx, req.studio_id).await?;
        // One reading of the database clock, under the locks: the day the
        // ceiling counts and the time the entry records are the same.
        let now: NaiveDateTime = sqlx::query_scalar("SELECT timezone('UTC', clock_timestamp())")
            .fetch_one(&mut *tx)
            .await?;

        let existing = sqlx::query(
            "SELECT id::text AS id, request_hash FROM credits.studio_budget_entries \
             WHERE studio_id = $1::uuid AND idempotency_key = $2",
        )
        .bind(req.studio_id)
        .bind(req.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing.get::<String, _>("request_hash") != request_hash {
                return Err(StudioError::IdempotencyKeyReused(
                    req.idempotency_key.into(),
                ));
            }
            let balance_cents = balance_cents(&mut tx, req.studio_id).await?;
            tx.commit().await?;
            return Ok(BudgetChange {
                entry_id: existing.get("id"),
                studio_name,
                balance_cents,
                replayed: true,
            });
        }

        if adds_budget {
            let limit_cents = req
                .daily_deposit_limit_cents
                .ok_or(StudioError::DepositsNotConfigured)?;
            let deposited_today_cents: i64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(usd_cents), 0)::bigint FROM credits.studio_budget_entries \
                 WHERE usd_cents > 0 \
                   AND created_at >= date_trunc('day', $1::timestamp) \
                   AND created_at < date_trunc('day', $1::timestamp) + interval '1 day'",
            )
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;
            if deposited_today_cents + req.usd_cents > limit_cents {
                return Err(StudioError::DailyDepositLimitExceeded {
                    deposited_today_cents,
                    requested_cents: req.usd_cents,
                    limit_cents,
                });
            }
        }

        let balance = balance_cents(&mut tx, req.studio_id).await?;
        if balance + req.usd_cents < 0 {
            return Err(StudioError::BudgetExceeded {
                balance_cents: balance,
            });
        }
        let entry_id: String = sqlx::query_scalar(
            "INSERT INTO credits.studio_budget_entries \
                 (studio_id, usd_cents, kind, note, idempotency_key, request_hash, created_by, created_at) \
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8) \
             RETURNING id::text",
        )
        .bind(req.studio_id)
        .bind(req.usd_cents)
        .bind(req.kind)
        .bind(req.note)
        .bind(req.idempotency_key)
        .bind(&request_hash)
        .bind(req.actor)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        Self::audit(
            &mut *tx,
            AuditEntry {
                action: "studio.budget",
                actor: req.actor,
                target: Some(req.studio_id),
                detail: &json!({
                    "entryId": entry_id, "kind": req.kind, "usdCents": req.usd_cents, "note": req.note,
                }),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(BudgetChange {
            entry_id,
            studio_name,
            balance_cents: balance + req.usd_cents,
            replayed: false,
        })
    }

    /// A page of the studio's budget history, newest first, with its total.
    async fn entries_on(
        conn: &mut PgConnection,
        studio_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<StudioBudgetEntry>, i64), StudioError> {
        let rows = sqlx::query(
            "SELECT e.id::text AS id, e.kind, e.usd_cents, e.note, e.created_by, \
                    (EXTRACT(EPOCH FROM e.created_at) * 1000)::bigint AS created_at, \
                    e.ledger_id::text AS credit_id, l.address AS recipient \
             FROM credits.studio_budget_entries e \
             LEFT JOIN credits.ledger l ON l.id = e.ledger_id \
             WHERE e.studio_id = $1::uuid \
             ORDER BY e.created_at DESC, e.id \
             LIMIT $2 OFFSET $3",
        )
        .bind(studio_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *conn)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credits.studio_budget_entries WHERE studio_id = $1::uuid",
        )
        .bind(studio_id)
        .fetch_one(&mut *conn)
        .await?;
        let entries = rows
            .iter()
            .map(|r| StudioBudgetEntry {
                id: r.get("id"),
                kind: r.get("kind"),
                usd_cents: r.get("usd_cents"),
                note: r.get("note"),
                created_by: r.get("created_by"),
                created_at: r.get("created_at"),
                credit_id: r.get("credit_id"),
                recipient: r.get("recipient"),
            })
            .collect();
        Ok((entries, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_deposits_lock_is_credits_servers() {
        // 0x + the first 15 hex digits of sha256("studio-budget-deposits").
        let digest = hex::encode(Sha256::digest(b"studio-budget-deposits"));
        assert_eq!(
            *DEPOSITS_LOCK_ID,
            i64::from_str_radix(&digest[..15], 16).unwrap()
        );
        assert!(*DEPOSITS_LOCK_ID > 0);
    }

    #[test]
    fn a_budget_request_hash_names_every_field() {
        let hash = budget_request_hash("deposit", 1000, None);
        assert_eq!(hash, budget_request_hash("deposit", 1000, None));
        assert_eq!(hash.len(), 64);
        for other in [
            budget_request_hash("adjustment", 1000, None),
            budget_request_hash("deposit", 1010, None),
            budget_request_hash("deposit", 1000, Some("")),
            budget_request_hash("deposit", 1000, Some("why")),
        ] {
            assert_ne!(hash, other);
        }
    }

    #[test]
    fn a_grant_over_the_ceiling_is_described_as_credits_server_does() {
        let (_, code, message) = StudioError::GrantLimitExceeded {
            max_cents: 10_000,
            requested_cents: 1_234_505,
        }
        .status_code_message();
        assert_eq!(code, "GRANT_LIMIT_EXCEEDED");
        assert_eq!(
            message,
            "A single studio grant may give at most 1,000 credits ($100.00); \
             123,450.5 credits ($12,345.05) was requested"
        );
    }

    #[test]
    fn a_studio_id_is_a_uuid() {
        assert_eq!(
            parse_studio_id("6F9619FF-8B86-D011-B42D-00C04FC964FF").unwrap(),
            "6f9619ff-8b86-d011-b42d-00c04fc964ff"
        );
        for bad in [
            "",
            "6f9619ff8b86d011b42d00c04fc964ff",
            "6f9619ff-8b86-d011-b42d-00c04fc964fz",
            "x",
        ] {
            assert!(parse_studio_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_and_notes_are_one_printable_line() {
        assert_eq!(parse_name(Some(&json!("  Acme  "))).unwrap(), "Acme");
        assert!(parse_name(None).is_err());
        assert!(parse_name(Some(&json!("   "))).is_err());
        assert!(parse_name(Some(&json!("a".repeat(81)))).is_err());
        assert!(parse_name(Some(&json!("a".repeat(80)))).is_ok());
        assert!(parse_name(Some(&json!("two\nlines"))).is_err());
        assert!(
            parse_name(Some(&json!("zero\u{200B}width"))).is_err(),
            "a format character"
        );
        assert!(parse_name(Some(&json!("rtl\u{202E}override"))).is_err());
        assert_eq!(parse_note(None).unwrap(), None);
        assert_eq!(parse_note(Some(&json!(null))).unwrap(), None);
        assert_eq!(parse_note(Some(&json!("  "))).unwrap(), None);
        assert_eq!(
            parse_note(Some(&json!(" why "))).unwrap().as_deref(),
            Some("why")
        );
        assert!(parse_note(Some(&json!(5))).is_err());
        assert!(parse_note(Some(&json!("n".repeat(201)))).is_err());
    }

    #[test]
    fn a_credits_change_is_a_bounded_non_zero_whole_number() {
        assert_eq!(parse_credits_change(Some(&json!(5))).unwrap(), 5);
        assert_eq!(parse_credits_change(Some(&json!(-5))).unwrap(), -5);
        assert_eq!(
            parse_credits_change(Some(&json!(MAX_CREDITS_PER_BUDGET_CHANGE))).unwrap(),
            MAX_CREDITS_PER_BUDGET_CHANGE
        );
        for bad in [
            json!(0),
            json!(1.5),
            json!("5"),
            json!(MAX_CREDITS_PER_BUDGET_CHANGE + 1),
            json!(-MAX_CREDITS_PER_BUDGET_CHANGE - 1),
            json!(i64::MIN),
            json!(i64::MAX),
            json!(u64::MAX),
        ] {
            assert!(parse_credits_change(Some(&bad)).is_err(), "{bad}");
        }
        assert!(parse_credits_change(None).is_err());
    }

    #[test]
    fn an_idempotency_key_is_eight_to_128_safe_characters() {
        assert!(parse_idempotency_key(Some(&json!("abcd-123"))).is_ok());
        assert!(parse_idempotency_key(Some(&json!("a.b_c:d-e"))).is_ok());
        for bad in [
            json!("short"),
            json!("x".repeat(129)),
            json!("has space1"),
            json!(12345678),
        ] {
            assert!(parse_idempotency_key(Some(&bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn refusals_carry_credits_servers_status_and_code() {
        let shape = |e: StudioError| {
            let (status, code, message) = e.status_code_message();
            (status.as_u16(), code, message)
        };
        assert_eq!(
            shape(StudioError::NotFound),
            (404, "STUDIO_NOT_FOUND", "Studio not found".into())
        );
        assert_eq!(shape(StudioError::NameTaken("Acme".into())).0, 409);
        assert_eq!(
            shape(StudioError::BudgetExceeded { balance_cents: 30 }).1,
            "STUDIO_BUDGET_EXCEEDED"
        );
        assert_eq!(
            shape(StudioError::DailyDepositLimitExceeded {
                deposited_today_cents: 1050,
                requested_cents: 500,
                limit_cents: 1200
            }),
            (
                429,
                "DAILY_DEPOSIT_LIMIT_EXCEEDED",
                "Daily studio deposit limit exceeded: $10.50 already added today, $5.00 requested, limit $12.00".into()
            )
        );
        assert_eq!(shape(StudioError::DepositsNotConfigured).0, 503);
        assert_eq!(
            shape(StudioError::Internal("x".into())),
            (500, "INTERNAL_ERROR", "Internal server error".into())
        );
    }
}
