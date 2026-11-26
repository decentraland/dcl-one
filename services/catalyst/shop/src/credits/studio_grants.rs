// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/adapters/db/db.ts (findStudioUsdGrantReplay, createStudioUsdGrant), src/logic/credits-granter.ts (grantStudioUsdCredits, readStudioGrantMaxCents), src/adapters/studios-db.ts (listStudiosForOperator, isOperator, getStudioGrants), src/controllers/handlers/studio-operators.ts (the body's checks)
//! Studio grants: a studio's operator gives a player Credits out of the
//! studio's budget. The grant is a non-expiring paid-bucket credit like an
//! admin's (`credits.ledger`, its `ops.admin_audit` row and the "A gift
//! from {studio}" notice), and a `grant` entry in the studio's budget that
//! names the ledger row and takes its cents.
//!
//! A grant is written in one transaction that pins READ COMMITTED whatever
//! the session default and takes the studio's row lock first. The operators,
//! the status and the balance are read after the lock is granted, each in a
//! snapshot that includes what the previous holder committed: two grants
//! racing for the last of a budget run one after the other, and the second
//! sees the first's entry. Operator changes, pauses and budget changes take
//! the same row lock. The grant then takes the recipient's wallet row; no path
//! takes a wallet row before a studio row.
//!
//! A refusal writes nothing. The order of judgment is credits-server's:
//! - the signer must act for the studio (else the studio is not found);
//! - a repeat of the idempotency key with the same request answers what it
//!   wrote, even once the studio is paused, the player flagged or the ceiling
//!   lowered (read with the operator check, from one snapshot);
//! - the per-grant ceiling (`CREDITS_STUDIO_GRANT_MAX_CENTS`, fail-closed);
//! - under the lock: the signer still acts for the studio, before its key is
//!   answered; then the key; then the studio is active, the player is not one
//!   of its operators, and its balance covers the grant;
//! - last, the player's multi-accounting flag, so that a paused or empty
//!   studio cannot use the refusal to learn which wallets carry one.
//!
//! The node keeps Credits in USD only, so credits-server's check that USD
//! credits are enabled has no counterpart.

use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use sqlx::Row;

use super::admin_store::GrantRequest;
use super::store::Credits;
use super::studios::{
    SUMMARY_SELECT, StudioError, StudioSummary, USD_CENTS_PER_CREDIT, balance_cents,
    is_one_printable_line, map_summary,
};

/// The longest grant reason, shown on one line in the studio's history.
pub const MAX_REASON_CHARS: usize = 80;
/// The longest grant annotation.
pub const MAX_ANNOTATION_CHARS: usize = 200;

/// The studio as its operators see it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperatorStudio {
    pub id: String,
    pub name: String,
    pub status: String,
    pub balance_cents: i64,
    pub granted_cents: i64,
    pub grant_count: i64,
}

impl From<StudioSummary> for OperatorStudio {
    fn from(s: StudioSummary) -> Self {
        Self {
            id: s.id,
            name: s.name,
            status: s.status,
            balance_cents: s.balance_cents,
            granted_cents: s.granted_cents,
            grant_count: s.grant_count,
        }
    }
}

/// One grant a studio gave, as its operators see it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StudioGrantRow {
    /// The ledger row the grant credited.
    pub credit_id: String,
    pub recipient: String,
    pub usd_cents: i64,
    pub reason: Option<String>,
    pub granted_by: String,
    /// Epoch milliseconds.
    pub created_at: i64,
}

/// What a grant wrote, or what its key already wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioGrant {
    pub credit_id: String,
    pub studio_name: String,
    /// The studio's balance now: after this grant, or, on a replay, as it stands.
    pub balance_cents: i64,
    pub replayed: bool,
}

/// A grant an operator asks for, its body already checked.
#[derive(Debug, Clone, Copy)]
pub struct StudioGrantRequest<'a> {
    pub studio_id: &'a str,
    /// The player, lowercase.
    pub recipient: &'a str,
    /// The signing operator, lowercase.
    pub granter: &'a str,
    pub usd_cents: i64,
    pub reason: &'a str,
    pub annotation: Option<&'a str>,
    pub idempotency_key: &'a str,
}

/// A stable fingerprint of what a grant asked for, kept next to its key. A
/// budget change hashes `["budget", ...]`, so a key is never answered for the
/// other kind of request.
pub fn grant_request_hash(
    recipient: &str,
    usd_cents: i64,
    reason: &str,
    annotation: Option<&str>,
) -> String {
    let request = json!(["grant", recipient, usd_cents, reason, annotation]);
    hex::encode(Sha256::digest(request.to_string().as_bytes()))
}

/// The largest integer a JSON number carries exactly (`Number.MAX_SAFE_INTEGER`).
pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// Whole Credits to grant: a positive whole number whose cents stay exact,
/// as credits-server requires (`Number.isSafeInteger(credits)` and
/// `Number.isSafeInteger(credits * 10)`), whatever the configured ceiling. A
/// JSON number is a number however it is spelled: `5`, `5.0` and `5e0` are
/// all five Credits, as in credits-server; `5.5` is not whole.
pub fn parse_grant_credits(value: Option<&JsonValue>) -> Result<i64, StudioError> {
    value
        .and_then(whole_number)
        .filter(|c| {
            *c > 0
                && c.checked_mul(USD_CENTS_PER_CREDIT)
                    .is_some_and(|cents| cents <= MAX_SAFE_INTEGER)
        })
        .ok_or_else(|| StudioError::Invalid("\"credits\" must be a positive whole number".into()))
}

/// A JSON number that is a whole number within the safe-integer range, in
/// any spelling; the cents are then computed on the integer.
fn whole_number(value: &JsonValue) -> Option<i64> {
    value.as_i64().or_else(|| {
        let f = value.as_f64()?;
        // Within ±(2^53 − 1) every f64 is exactly an integer or not, so the
        // conversion below is exact.
        (f.fract() == 0.0 && f.abs() <= MAX_SAFE_INTEGER as f64).then_some(f as i64)
    })
}

/// The grant's reason: required, trimmed, one printable line of at most
/// [`MAX_REASON_CHARS`].
pub fn parse_reason(value: Option<&JsonValue>) -> Result<String, StudioError> {
    let reason = value
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .ok_or_else(|| StudioError::Invalid("\"reason\" is required".into()))?;
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(StudioError::Invalid(format!(
            "\"reason\" must be at most {MAX_REASON_CHARS} characters"
        )));
    }
    if !is_one_printable_line(reason) {
        return Err(StudioError::Invalid(
            "\"reason\" must be a single line of printable characters".into(),
        ));
    }
    Ok(reason.to_string())
}

/// An optional annotation: trimmed, one printable line of at most
/// [`MAX_ANNOTATION_CHARS`]; blank is none.
pub fn parse_annotation(value: Option<&JsonValue>) -> Result<Option<String>, StudioError> {
    let annotation = match value {
        None | Some(JsonValue::Null) => return Ok(None),
        Some(JsonValue::String(a)) => a.trim(),
        Some(_) => {
            return Err(StudioError::Invalid(
                "\"annotation\" must be a string".into(),
            ));
        }
    };
    if annotation.chars().count() > MAX_ANNOTATION_CHARS {
        return Err(StudioError::Invalid(format!(
            "\"annotation\" must be at most {MAX_ANNOTATION_CHARS} characters"
        )));
    }
    if !is_one_printable_line(annotation) {
        return Err(StudioError::Invalid(
            "\"annotation\" must be a single line of printable characters".into(),
        ));
    }
    Ok((!annotation.is_empty()).then(|| annotation.to_string()))
}

impl Credits {
    /// The studios `operator` acts for, by name.
    pub async fn operator_studios(
        &self,
        operator: &str,
    ) -> Result<Vec<StudioSummary>, StudioError> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SUMMARY_SELECT} WHERE s.id IN (SELECT studio_id FROM credits.studio_operators WHERE address = $1) \
             ORDER BY lower(s.name), s.id"
        )))
        .bind(operator)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(map_summary).collect())
    }

    /// One of `operator`'s studios with a page of its grants, newest first,
    /// and their total, from one snapshot; `None` when the studio does not
    /// exist or `operator` does not act for it.
    pub async fn operator_studio(
        &self,
        studio_id: &str,
        operator: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<(StudioSummary, Vec<StudioGrantRow>, i64)>, StudioError> {
        let mut tx = self.read_snapshot().await?;
        let acts: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM credits.studio_operators WHERE studio_id = $1::uuid AND address = $2)",
        )
        .bind(studio_id)
        .bind(operator)
        .fetch_one(&mut *tx)
        .await?;
        let row = if acts {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "{SUMMARY_SELECT} WHERE s.id = $1::uuid"
            )))
            .bind(studio_id)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            None
        };
        let Some(studio) = row.as_ref().map(map_summary) else {
            tx.commit().await?;
            return Ok(None);
        };
        let rows = sqlx::query(
            "SELECT e.ledger_id::text AS credit_id, l.address AS recipient, -e.usd_cents AS usd_cents, \
                    e.note AS reason, e.created_by AS granted_by, \
                    (EXTRACT(EPOCH FROM e.created_at) * 1000)::bigint AS created_at \
             FROM credits.studio_budget_entries e JOIN credits.ledger l ON l.id = e.ledger_id \
             WHERE e.studio_id = $1::uuid AND e.kind = 'grant' \
             ORDER BY e.created_at DESC, e.id \
             LIMIT $2 OFFSET $3",
        )
        .bind(studio_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credits.studio_budget_entries WHERE studio_id = $1::uuid AND kind = 'grant'",
        )
        .bind(studio_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        let grants = rows
            .iter()
            .map(|r| StudioGrantRow {
                credit_id: r.get("credit_id"),
                recipient: r.get("recipient"),
                usd_cents: r.get("usd_cents"),
                reason: r.get("reason"),
                granted_by: r.get("granted_by"),
                created_at: r.get("created_at"),
            })
            .collect();
        Ok(Some((studio, grants, total)))
    }

    /// Whether `req`'s granter acts for the studio and, if so, the grant its
    /// key already made, read without the studio's lock so that a repeat is
    /// answered before anything that only decides a new grant. The operator,
    /// the entry and the balance are read from one snapshot, so nothing is
    /// told from after the granter stopped acting for the studio. `NotFound`
    /// when it does not act for it; `IdempotencyKeyReused` when the key was
    /// used for another request (a budget change included). The write checks
    /// both again under the lock: this read only ever answers early.
    pub async fn studio_grant_replay(
        &self,
        req: StudioGrantRequest<'_>,
    ) -> Result<Option<StudioGrant>, StudioError> {
        let hash = grant_request_hash(req.recipient, req.usd_cents, req.reason, req.annotation);
        let mut tx = self.read_snapshot().await?;
        let acts: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM credits.studio_operators WHERE studio_id = $1::uuid AND address = $2)",
        )
        .bind(req.studio_id)
        .bind(req.granter)
        .fetch_one(&mut *tx)
        .await?;
        if !acts {
            return Err(StudioError::NotFound);
        }
        let Some(entry) = sqlx::query(
            "SELECT e.request_hash, e.ledger_id::text AS credit_id, s.name \
             FROM credits.studio_budget_entries e JOIN credits.studios s ON s.id = e.studio_id \
             WHERE e.studio_id = $1::uuid AND e.idempotency_key = $2",
        )
        .bind(req.studio_id)
        .bind(req.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        else {
            tx.commit().await?;
            return Ok(None);
        };
        let credit_id: Option<String> = entry.get("credit_id");
        let (true, Some(credit_id)) = (entry.get::<String, _>("request_hash") == hash, credit_id)
        else {
            return Err(StudioError::IdempotencyKeyReused(
                req.idempotency_key.into(),
            ));
        };
        let balance_cents = balance_cents(&mut tx, req.studio_id).await?;
        tx.commit().await?;
        Ok(Some(StudioGrant {
            credit_id,
            studio_name: entry.get("name"),
            balance_cents,
            replayed: true,
        }))
    }

    /// Writes a studio grant, or answers the one its key already wrote. See
    /// the module doc for the lock, the isolation level and the order of the
    /// checks; every refusal rolls the transaction back.
    pub async fn grant_from_studio(
        &self,
        req: StudioGrantRequest<'_>,
    ) -> Result<StudioGrant, StudioError> {
        let hash = grant_request_hash(req.recipient, req.usd_cents, req.reason, req.annotation);
        let mut tx = self.pool.begin().await?;
        // Each statement after the lock must see what the previous holder
        // committed: under REPEATABLE READ a waiter's snapshot could predate it
        // and overdraw the budget.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        let studio =
            sqlx::query("SELECT name, status FROM credits.studios WHERE id = $1::uuid FOR UPDATE")
                .bind(req.studio_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(StudioError::NotFound)?;
        let studio_name: String = studio.get("name");
        let status: String = studio.get("status");

        // Who acts for the studio, under its lock: an operator removed since
        // the route's read is told the studio is not found, before anything
        // about the studio or its key, a past grant included.
        let acting: Vec<String> = sqlx::query_scalar(
            "SELECT address FROM credits.studio_operators \
             WHERE studio_id = $1::uuid AND address IN ($2, $3)",
        )
        .bind(req.studio_id)
        .bind(req.granter)
        .bind(req.recipient)
        .fetch_all(&mut *tx)
        .await?;
        if !acting.iter().any(|a| a == req.granter) {
            return Err(StudioError::NotFound);
        }

        // A repeat is answered before the studio's current state is judged.
        let existing = sqlx::query(
            "SELECT request_hash, ledger_id::text AS credit_id FROM credits.studio_budget_entries \
             WHERE studio_id = $1::uuid AND idempotency_key = $2",
        )
        .bind(req.studio_id)
        .bind(req.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            let credit_id: Option<String> = existing.get("credit_id");
            let (true, Some(credit_id)) =
                (existing.get::<String, _>("request_hash") == hash, credit_id)
            else {
                return Err(StudioError::IdempotencyKeyReused(
                    req.idempotency_key.into(),
                ));
            };
            let balance_cents = balance_cents(&mut tx, req.studio_id).await?;
            tx.commit().await?;
            return Ok(StudioGrant {
                credit_id,
                studio_name,
                balance_cents,
                replayed: true,
            });
        }

        if status != "active" {
            return Err(StudioError::Paused(studio_name));
        }
        if acting.iter().any(|a| a == req.recipient) {
            return Err(StudioError::RecipientIsOperator(req.recipient.into()));
        }
        let balance = balance_cents(&mut tx, req.studio_id).await?;
        if balance < req.usd_cents {
            return Err(StudioError::BudgetExceeded {
                balance_cents: balance,
            });
        }
        let flagged: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM credits.wallet_flags WHERE address = $1)",
        )
        .bind(req.recipient)
        .fetch_one(&mut *tx)
        .await?;
        if flagged {
            tracing::info!(
                studio_id = %req.studio_id,
                recipient = %req.recipient,
                "refused a studio grant to a wallet flagged for multi-accounting"
            );
            return Err(StudioError::RecipientNotEligible);
        }

        let credits = (req.usd_cents / USD_CENTS_PER_CREDIT).to_string();
        let detail = json!({
            "address": req.recipient,
            "studioId": req.studio_id,
            "studioName": studio_name,
            "annotation": req.annotation,
            "idempotencyKey": req.idempotency_key,
        });
        let (_, ledger_id) = self
            .paid_credit_in_tx(
                &mut tx,
                GrantRequest {
                    address: req.recipient,
                    amount: &credits,
                    kind: "grant",
                    reason: Some(req.reason),
                    actor: Some(req.granter),
                    // The studio's key lives on its entry: `credits.idempotency`
                    // is one key space shared with every admin grant.
                    idempotency_key: None,
                    detail: &detail,
                    studio_name: Some(&studio_name),
                },
            )
            .await?;
        let ledger_id = ledger_id
            .ok_or_else(|| StudioError::Internal("a new grant wrote no ledger row".into()))?;
        sqlx::query(
            "INSERT INTO credits.studio_budget_entries \
                 (studio_id, usd_cents, kind, ledger_id, note, idempotency_key, request_hash, created_by, created_at) \
             VALUES ($1::uuid, $2, 'grant', $3, $4, $5, $6, $7, timezone('UTC', clock_timestamp()))",
        )
        .bind(req.studio_id)
        .bind(-req.usd_cents)
        .bind(ledger_id)
        .bind(req.reason)
        .bind(req.idempotency_key)
        .bind(&hash)
        .bind(req.granter)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(StudioGrant {
            credit_id: ledger_id.to_string(),
            studio_name,
            balance_cents: balance - req.usd_cents,
            replayed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grant_request_hash_names_every_field_and_differs_from_a_budget_change() {
        let a = "0x00000000000000000000000000000000000000aa";
        let hash = grant_request_hash(a, 100, "why", None);
        assert_eq!(hash, grant_request_hash(a, 100, "why", None));
        for other in [
            grant_request_hash(
                "0x00000000000000000000000000000000000000bb",
                100,
                "why",
                None,
            ),
            grant_request_hash(a, 110, "why", None),
            grant_request_hash(a, 100, "why not", None),
            grant_request_hash(a, 100, "why", Some("")),
            grant_request_hash(a, 100, "why", Some("note")),
        ] {
            assert_ne!(hash, other);
        }
    }

    #[test]
    fn a_grant_body_is_checked_as_credits_server_checks_it() {
        assert_eq!(parse_grant_credits(Some(&json!(5))).unwrap(), 5);
        // The last whole Credit whose cents are a safe integer, and the next.
        assert_eq!(
            parse_grant_credits(Some(&json!(900_719_925_474_099_i64))).unwrap(),
            900_719_925_474_099
        );
        assert_eq!(
            900_719_925_474_099 * USD_CENTS_PER_CREDIT,
            MAX_SAFE_INTEGER - 1
        );
        for bad in [
            json!(0),
            json!(-1),
            json!(1.5),
            json!("5"),
            json!(900_719_925_474_100_i64),
            json!(i64::MAX),
        ] {
            assert!(parse_grant_credits(Some(&bad)).is_err(), "{bad}");
        }
        assert!(parse_grant_credits(None).is_err());
        // Raw JSON as a client sends it: a whole number in any spelling is
        // that many Credits, as `Number.isSafeInteger` reads it upstream.
        let raw = |text: &str| parse_grant_credits(Some(&serde_json::from_str(text).unwrap()));
        for (text, credits) in [
            ("5", 5),
            ("5.0", 5),
            ("5e0", 5),
            ("5E0", 5),
            ("0.5e1", 5),
            ("900719925474099", 900_719_925_474_099),
            ("900719925474099.0", 900_719_925_474_099),
            ("9.00719925474099e14", 900_719_925_474_099),
        ] {
            assert_eq!(raw(text).unwrap(), credits, "{text}");
        }
        for text in [
            "5.5",
            "0.0",
            "-5.0",
            "5e-1",
            "900719925474100",
            "900719925474100.0",
            "9.007199254741e14",
            "1e300",
            "\"5\"",
        ] {
            assert!(raw(text).is_err(), "{text}");
        }
        assert_eq!(parse_reason(Some(&json!(" thanks "))).unwrap(), "thanks");
        assert!(parse_reason(None).is_err());
        assert!(parse_reason(Some(&json!("  "))).is_err());
        assert!(parse_reason(Some(&json!("r".repeat(81)))).is_err());
        assert!(parse_reason(Some(&json!("r".repeat(80)))).is_ok());
        assert!(parse_reason(Some(&json!("two\nlines"))).is_err());
        assert_eq!(parse_annotation(None).unwrap(), None);
        assert_eq!(parse_annotation(Some(&json!(null))).unwrap(), None);
        assert_eq!(parse_annotation(Some(&json!(" "))).unwrap(), None);
        assert_eq!(
            parse_annotation(Some(&json!(" a "))).unwrap().as_deref(),
            Some("a")
        );
        assert!(parse_annotation(Some(&json!(3))).is_err());
        assert!(parse_annotation(Some(&json!("a".repeat(201)))).is_err());
        assert!(parse_annotation(Some(&json!("zero\u{200B}width"))).is_err());
    }
}
