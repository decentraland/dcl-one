//! The validator's claims on requests, their leases and its verdicts.
//!
//! A claim takes the oldest open request with claims left whose lease is not
//! held and whose collection's lock doesn't hold, skipping one another claim
//! holds (`FOR UPDATE SKIP LOCKED`). It answers builder-server's validation
//! manifest built from the items as saved now (a request whose items are all
//! gone settles `superseded` and the next is taken) with a new claim id and
//! a lease of [`LEASE`]. Only the current claim of an open request may
//! extend its lease, up to [`LEASE_LIMIT`] after the claim, or post the
//! verdict that settles it; a verdict from any other claim settles nothing.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{LEASE, LEASE_LIMIT, MAX_CLAIMS, MAX_ITEMS, saved_items};
use crate::State;
use crate::access::LOCK_HOLDS;
use crate::content::{JsJson, js_stringify};
use crate::http::{ApiError, iso};

/// A claimed request: builder-server's validation manifest with the claim's
/// id and the end of its lease.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Claimed {
    pub validation_id: Uuid,
    pub claim_id: Uuid,
    pub collection_id: Uuid,
    pub lease_until: String,
    pub items: Vec<Box<RawValue>>,
}

type ClaimableRow = (Uuid, Uuid, Vec<Uuid>, Option<String>);

/// Claims the oldest claimable request; `None` when there is none.
pub async fn claim(state: &State) -> Result<Option<Claimed>, ApiError> {
    let now = (state.now)();
    let mut tx = state.items.pool().begin().await?;
    loop {
        let claimable: Option<ClaimableRow> = sqlx::query_as(
            "SELECT r.id, r.collection_id, r.item_ids, c.contract_address \
             FROM builder.validation_requests r JOIN builder.collections c ON c.id = r.collection_id \
             WHERE r.settled_at IS NULL AND r.claims < $2 AND (r.lease_until IS NULL OR r.lease_until < $1) \
               AND (c.lock IS NULL OR c.lock <= $3) \
             ORDER BY r.created_at, r.id LIMIT 1 FOR UPDATE OF r SKIP LOCKED",
        )
        .bind(now.naive_utc())
        .bind(MAX_CLAIMS)
        .bind((now - LOCK_HOLDS).naive_utc())
        .fetch_optional(&mut *tx)
        .await?;
        let Some((validation_id, collection_id, item_ids, contract)) = claimable else {
            tx.commit().await?;
            return Ok(None);
        };
        let items = match &contract {
            Some(_) => saved_items(&mut tx, collection_id, Some(&item_ids)).await?,
            None => Vec::new(),
        };
        let Some(contract) = contract.filter(|_| !items.is_empty()) else {
            sqlx::query(
                "UPDATE builder.validation_requests SET settled_at = $2, outcome = 'superseded' WHERE id = $1",
            )
            .bind(validation_id)
            .bind(now.naive_utc())
            .execute(&mut *tx)
            .await?;
            tracing::info!(validation = %validation_id, collection = %collection_id, "builder validation: its items are gone");
            continue;
        };
        let mut hashes = Map::new();
        let mut manifest = Vec::with_capacity(items.len());
        for item in &items {
            let content_hash = item.content_hash(&contract);
            hashes.insert(item.id.to_string(), Value::String(content_hash.clone()));
            let entry = JsJson::Object(vec![
                ("itemId".into(), JsJson::String(item.id.to_string())),
                ("contentHash".into(), JsJson::String(content_hash)),
                ("metadata".into(), item.metadata(&contract)),
                ("contents".into(), item.deployed_files()),
            ]);
            manifest.push(
                RawValue::from_string(js_stringify(&entry))
                    .map_err(|e| ApiError::internal(format!("manifest entry: {e}")))?,
            );
        }
        let claim_id = Uuid::new_v4();
        let lease_until = now + LEASE;
        sqlx::query(
            "UPDATE builder.validation_requests SET claims = claims + 1, claim_id = $2, claimed_at = $3, \
               lease_until = $4, content_hashes = $5 WHERE id = $1",
        )
        .bind(validation_id)
        .bind(claim_id)
        .bind(now.naive_utc())
        .bind(lease_until.naive_utc())
        .bind(Value::Object(hashes))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(validation = %validation_id, collection = %collection_id, claim = %claim_id, items = manifest.len(), "builder validation claimed");
        return Ok(Some(Claimed {
            validation_id,
            claim_id,
            collection_id,
            lease_until: iso(&lease_until),
            items: manifest,
        }));
    }
}

/// Extends the lease of `claim` on `validation` to [`LEASE`] from now, at
/// most [`LEASE_LIMIT`] after the claim; `None` when the claim isn't
/// current.
pub async fn extend(
    state: &State,
    validation: Uuid,
    claim: Uuid,
) -> Result<Option<DateTime<Utc>>, ApiError> {
    let now = (state.now)();
    let until: Option<chrono::NaiveDateTime> = sqlx::query_scalar(
        "UPDATE builder.validation_requests SET lease_until = LEAST($4, claimed_at + $5) \
         WHERE id = $1 AND claim_id = $2 AND settled_at IS NULL AND lease_until >= $3 AND claimed_at + $5 > $3 \
         RETURNING lease_until",
    )
    .bind(validation)
    .bind(claim)
    .bind(now.naive_utc())
    .bind((now + LEASE).naive_utc())
    .bind(LEASE_LIMIT)
    .fetch_optional(state.items.pool())
    .await?;
    if until.is_none() {
        tracing::info!(%validation, %claim, "builder validation: lease refused, the claim is not current");
    }
    Ok(until.map(|until| until.and_utc()))
}

const TOKEN: usize = 64;
const SHORT_TEXT: usize = 256;
const LONG_TEXT: usize = 10_000;
const MAX_FINDINGS: usize = 1_000;
const LOGGED_ERROR: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Passed,
    Rejected,
    Error,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Rejected => "rejected",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum BodyShape {
    Male,
    Female,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Finding {
    rule: String,
    check: String,
    #[allow(dead_code)]
    severity: Severity,
    message: String,
    #[serde(rename = "where")]
    location: Option<String>,
    #[allow(dead_code)]
    body_shape: Option<BodyShape>,
    #[allow(dead_code)]
    measured: Option<f64>,
    #[allow(dead_code)]
    limit: Option<f64>,
    fix: Option<String>,
    docs: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResultItem {
    item_id: String,
    content_hash: String,
    #[serde(deserialize_with = "Option::deserialize")]
    passed: Option<bool>,
    findings: Vec<Finding>,
    visual_summary: Option<String>,
    #[allow(dead_code)]
    unsupported: Option<bool>,
    error: Option<String>,
}

/// The validator's verdict on a claim: builder-server's validation result
/// and the `claimId` it was claimed under.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Verdict {
    validation_id: String,
    pub collection_id: String,
    verdict: Outcome,
    rules_version: String,
    items: Vec<ResultItem>,
    reason: Option<String>,
    retryable: Option<bool>,
    claim_id: Uuid,
    #[serde(skip)]
    result: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>,
}

fn token(value: &str) -> bool {
    (1..=TOKEN).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:_+-".contains(&b))
}

fn within(value: &Option<String>, max: usize) -> bool {
    value.as_ref().is_none_or(|v| v.chars().count() <= max)
}

impl Verdict {
    /// builder-server's `validationResultSchema` with `claimId`, its
    /// `validationId` and each `itemId` a UUID in the canonical form a claim
    /// answers; 400 `Invalid request body` otherwise.
    pub fn parse(body: &[u8]) -> Result<Self, ApiError> {
        let refused = || ApiError::bad_request("Invalid request body");
        let mut verdict: Self = serde_json::from_slice(body).map_err(|_| refused())?;
        verdict.result = serde_json::from_slice(body).map_err(|_| refused())?;
        verdict.result.remove("claimId");
        let canonical = |id: &str| Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id);
        let valid = canonical(&verdict.validation_id)
            && token(&verdict.collection_id)
            && token(&verdict.rules_version)
            && within(&verdict.reason, SHORT_TEXT)
            && verdict.items.len() <= MAX_ITEMS
            && verdict.items.iter().all(|item| {
                canonical(&item.item_id)
                    && item.content_hash.chars().count() <= SHORT_TEXT
                    && within(&item.error, LONG_TEXT)
                    && within(&item.visual_summary, 50_000)
                    && item.findings.len() <= MAX_FINDINGS
                    && item.findings.iter().all(|f| {
                        f.rule.chars().count() <= SHORT_TEXT
                            && f.check.chars().count() <= SHORT_TEXT
                            && f.message.chars().count() <= LONG_TEXT
                            && within(&f.location, 1_024)
                            && within(&f.fix, LONG_TEXT)
                            && within(&f.docs, 2_048)
                    })
            });
        if !valid {
            return Err(refused());
        }
        Ok(verdict)
    }
}

fn one_line(text: &str) -> String {
    let line = crate::curation::slack::one_line(text);
    if line.chars().count() > LOGGED_ERROR {
        line.chars().take(LOGGED_ERROR).chain(['…']).collect()
    } else {
        line
    }
}

fn another_collection(data: Value) -> ApiError {
    ApiError::bad_request("The result belongs to another collection").with_data(data)
}

/// Settles the request `verdict` names with its outcome when it comes from
/// the request's current claim; a retryable error ends the lease instead,
/// so the request can be claimed again. With the review flow on, the
/// verdict also acts on the review in the same transaction
/// ([`crate::review::on_verdict`]). 400 for a verdict about another
/// collection than `path_id` or an item its request didn't name.
pub async fn settle(
    state: &State,
    path_id: &str,
    verdict: Verdict,
    verified: &crate::handlers::validator::Verified,
) -> Result<(), ApiError> {
    if verdict.collection_id != path_id {
        return Err(another_collection(
            json!({ "id": path_id, "collectionId": verdict.collection_id }),
        ));
    }
    let Ok(validation) = Uuid::parse_str(&verdict.validation_id) else {
        return Err(ApiError::bad_request("Invalid request body"));
    };
    let now = (state.now)();
    let mut tx = state.items.pool().begin().await?;
    let named: Option<Uuid> =
        sqlx::query_scalar("SELECT collection_id FROM builder.validation_requests WHERE id = $1")
            .bind(validation)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(collection) = named {
        sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
            .bind(collection)
            .execute(&mut *tx)
            .await?;
    }
    type RequestRow = (
        Uuid,
        Vec<Uuid>,
        Option<Uuid>,
        Option<chrono::NaiveDateTime>,
        bool,
        chrono::NaiveDateTime,
    );
    let request: Option<RequestRow> = sqlx::query_as(
        "SELECT collection_id, item_ids, claim_id, lease_until, settled_at IS NOT NULL, created_at \
         FROM builder.validation_requests WHERE id = $1 FOR UPDATE",
    )
    .bind(validation)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((collection, item_ids, claim, lease_until, settled, requested_at)) = request else {
        tracing::info!(%validation, "builder validation: a verdict for no request");
        return Ok(());
    };
    if collection.to_string() != path_id {
        return Err(another_collection(json!({ "id": path_id })));
    }
    if verdict
        .items
        .iter()
        .any(|item| Uuid::parse_str(&item.item_id).map_or(true, |id| !item_ids.contains(&id)))
    {
        return Err(
            ApiError::bad_request("The result names an item the validation didn't")
                .with_data(json!({ "id": path_id })),
        );
    }
    let current = !settled
        && claim == Some(verdict.claim_id)
        && lease_until.is_some_and(|until| until.and_utc() >= now);
    if !current {
        tracing::info!(%validation, %collection, claim = %verdict.claim_id, "builder validation: a verdict from a claim that isn't current settles nothing");
        return Ok(());
    }
    let retry = verdict.verdict == Outcome::Error && verdict.retryable == Some(true);
    if retry {
        sqlx::query("UPDATE builder.validation_requests SET lease_until = $2 WHERE id = $1")
            .bind(validation)
            .bind((now - TimeDelta::microseconds(1)).naive_utc())
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "UPDATE builder.validation_requests SET settled_at = $2, outcome = $3 WHERE id = $1",
        )
        .bind(validation)
        .bind(now.naive_utc())
        .bind(verdict.verdict.as_str())
        .execute(&mut *tx)
        .await?;
    }
    let current_hashes: std::collections::HashMap<Uuid, String> =
        match sqlx::query_scalar::<_, Option<String>>(
            "SELECT contract_address FROM builder.collections WHERE id = $1",
        )
        .bind(collection)
        .fetch_one(&mut *tx)
        .await?
        {
            Some(contract) => saved_items(&mut tx, collection, Some(&item_ids))
                .await?
                .iter()
                .map(|item| (item.id, item.content_hash(&contract)))
                .collect(),
            None => Default::default(),
        };
    let stale: Vec<&str> = verdict
        .items
        .iter()
        .filter(|item| {
            Uuid::parse_str(&item.item_id)
                .ok()
                .and_then(|id| current_hashes.get(&id))
                .is_some_and(|hash| *hash != item.content_hash)
        })
        .map(|item| item.item_id.as_str())
        .collect();
    let notices = if crate::review::enabled(&mut tx, state).await? {
        let judged: &[&str] = if verdict.verdict == Outcome::Error {
            &[]
        } else {
            &stale
        };
        crate::review::on_verdict(
            &mut tx,
            state,
            &crate::access::validator_verdict(verified, collection),
            validation,
            requested_at,
            &verdict.result,
            judged,
        )
        .await?
    } else {
        Vec::new()
    };
    tx.commit().await?;
    if !notices.is_empty() {
        let name: String = sqlx::query_scalar("SELECT name FROM builder.collections WHERE id = $1")
            .bind(collection)
            .fetch_one(state.items.pool())
            .await?;
        for notice in notices {
            state
                .notices
                .send(&state.guarded, collection, &name, "", notice);
        }
    }
    let items: Vec<String> = verdict
        .items
        .iter()
        .map(|item| match (&item.passed, &item.error) {
            (_, Some(error)) => format!("{}=error({})", item.item_id, one_line(error)),
            (Some(passed), None) => format!("{}={passed}", item.item_id),
            (None, None) => format!("{}=undecided", item.item_id),
        })
        .collect();
    tracing::info!(
        %validation,
        %collection,
        verdict = verdict.verdict.as_str(),
        rules = %verdict.rules_version,
        retry,
        items = %items.join(" "),
        stale = %stale.join(" "),
        "builder validation verdict"
    );
    Ok(())
}
