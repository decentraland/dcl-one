//! `POST /v1/collections/:id/tos`: a creator's acceptance of the publication
//! terms, as builder-server takes it (its `saveTOSSchema`), kept on the node
//! (`builder.terms_acceptances`) and forwarded to the warehouse
//! (`WAREHOUSE_URL`, `x-token: WAREHOUSE_TOKEN`) as builder-server sends it:
//! `{context, event, body}`. The acceptance is stored first; the forward is
//! tried once right after, through the guarded client that follows no
//! redirect, and then by [`forward_due`] (the leased `builder-terms-forward`
//! job) at most [`MAX_ATTEMPTS`] times, doubling the wait from a minute. A
//! delivery the warehouse answers with a non-2xx status is retried; one it
//! received but whose answer was lost is sent again. No route answers the
//! stored rows, and a signer's acceptance replaces its earlier one of the
//! same collection and event, so they stay one per collection, signer and
//! event, each naming at most as many hashes as the collection has items.

use crate::shared::writes::PassGate;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::State;
use crate::access::{self, CollectionManage};
use crate::chain_state;
use crate::handlers::newsletter::is_valid_email;
use crate::http::ApiError;

/// The most attempts an acceptance's forward gets.
pub const MAX_ATTEMPTS: i32 = 6;
/// The most due acceptances one pass forwards.
pub const PASS_LIMIT: i64 = 50;
/// The most hashes an acceptance names (builder-server's bound).
pub const MAX_HASHES: usize = 40_000;
/// The longest hash an acceptance names: a third-party item's keccak-256
/// in hex; a CID is shorter.
pub const MAX_HASH_LEN: usize = 64;

const COLLECTION_EVENT: &str = "publish_collection_tos";
const THIRD_PARTY_EVENT: &str = "publish_third_party_items_tos";

/// An accepted body: the email, the event and the hashes it names.
#[derive(Debug, PartialEq)]
pub struct Acceptance {
    pub email: String,
    pub event: &'static str,
    pub hashes: Option<Vec<String>>,
}

fn invalid_body() -> ApiError {
    ApiError::bad_request("Invalid request body")
}

/// builder-server's `saveTOSSchema`: `email` required and an email,
/// `collection_address` a string, `event` one of the two, `hashes` 1 to
/// [`MAX_HASHES`] strings, each a content hash (ASCII letters and digits, at
/// most [`MAX_HASH_LEN`]); other keys are dropped, as its validator drops
/// them.
pub fn parse(body: &Value) -> Result<Acceptance, ApiError> {
    let fields: &Map<String, Value> = body.as_object().ok_or_else(invalid_body)?;
    let email = fields
        .get("email")
        .and_then(Value::as_str)
        .filter(|email| is_valid_email(email))
        .ok_or_else(invalid_body)?;
    if fields
        .get("collection_address")
        .is_some_and(|address| !address.is_string())
    {
        return Err(invalid_body());
    }
    let event = match fields.get("event") {
        None => COLLECTION_EVENT,
        Some(event) if event == COLLECTION_EVENT => COLLECTION_EVENT,
        Some(event) if event == THIRD_PARTY_EVENT => THIRD_PARTY_EVENT,
        Some(_) => return Err(invalid_body()),
    };
    let hashes = match fields.get("hashes") {
        None => None,
        Some(hashes) => {
            let hashes = hashes
                .as_array()
                .filter(|h| !h.is_empty() && h.len() <= MAX_HASHES)
                .ok_or_else(invalid_body)?;
            Some(
                hashes
                    .iter()
                    .map(|h| {
                        h.as_str()
                            .filter(|h| {
                                h.len() <= MAX_HASH_LEN
                                    && h.bytes().all(|b| b.is_ascii_alphanumeric())
                            })
                            .map(str::to_string)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(invalid_body)?,
            )
        }
    };
    Ok(Acceptance {
        email: email.to_string(),
        event,
        hashes,
    })
}

/// Stores `acceptance` for the collection `manage` names, in place of the
/// signer's earlier one of the same event, in one transaction that locks
/// the collection and decides again ([`access::confirm_collection_manage`]);
/// more hashes than the collection has items is a 400. Then tries its
/// forward once when a warehouse is configured.
pub async fn accept(
    state: &State,
    manage: &CollectionManage,
    acceptance: Acceptance,
) -> Result<(), ApiError> {
    let mut tx = state.items.pool().begin().await?;
    let (contract, urn): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT contract_address, CASE WHEN third_party_id IS NOT NULL THEN urn_suffix END \
         FROM builder.collections WHERE id = $1 FOR UPDATE",
    )
    .bind(manage.id())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    let view = chain_state::for_decision(&mut tx).await?;
    access::confirm_collection_manage(&mut tx, &view, manage).await?;
    if let Some(hashes) = &acceptance.hashes {
        let items: i64 =
            sqlx::query_scalar("SELECT count(*) FROM builder.items WHERE collection_id = $1")
                .bind(manage.id())
                .fetch_one(&mut *tx)
                .await?;
        if hashes.len() as i64 > items {
            return Err(invalid_body());
        }
    }
    sqlx::query(
        "DELETE FROM builder.terms_acceptances \
         WHERE collection_id = $1 AND signer = $2 AND event = $3 AND forwarded_at IS NOT NULL",
    )
    .bind(manage.id())
    .bind(manage.signer())
    .bind(acceptance.event)
    .execute(&mut *tx)
    .await?;
    let waiting: Option<(Uuid, Value)> = sqlx::query_as(
        "SELECT id, body FROM builder.terms_acceptances \
         WHERE collection_id = $1 AND signer = $2 AND event = $3 AND forwarded_at IS NULL",
    )
    .bind(manage.id())
    .bind(manage.signer())
    .bind(acceptance.event)
    .fetch_optional(&mut *tx)
    .await?;
    let body = match acceptance.event {
        THIRD_PARTY_EVENT => json!({
            "email": acceptance.email,
            "urn": urn.unwrap_or_else(|| "Unknown urn".into()),
            "hashes": merged(waiting.as_ref().map(|(_, body)| &body["hashes"]), acceptance.hashes),
        }),
        _ => json!({
            "email": acceptance.email,
            "eth_address": manage.signer(),
            "collection_address": contract.unwrap_or_else(|| "Unknown address".into()),
        }),
    };
    let id = waiting.map_or_else(Uuid::new_v4, |(id, _)| id);
    let configured = state.settings.warehouse.is_some();
    sqlx::query(
        "INSERT INTO builder.terms_acceptances (id, collection_id, signer, event, body, attempts, next_attempt_at) \
         VALUES ($1, $2, $3, $4, $5, $6, timezone('UTC', now()) + make_interval(mins => $6)) \
         ON CONFLICT (id) DO UPDATE SET body = EXCLUDED.body, attempts = EXCLUDED.attempts, \
           next_attempt_at = EXCLUDED.next_attempt_at",
    )
    .bind(id)
    .bind(manage.id())
    .bind(manage.signer())
    .bind(acceptance.event)
    .bind(&body)
    .bind(i32::from(configured))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if configured && send(state, acceptance.event, &body).await {
        delivered(state, id, &body).await?;
    }
    Ok(())
}

/// The hashes of an acceptance still waiting for the warehouse, then the
/// new ones it doesn't hold, at most [`MAX_HASHES`], the newest kept.
fn merged(waiting: Option<&Value>, new: Option<Vec<String>>) -> Option<Vec<String>> {
    let mut hashes: Vec<String> = waiting
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|h| h.as_str().map(str::to_string))
        .collect();
    if hashes.is_empty() {
        return new;
    }
    let mut held: std::collections::HashSet<String> = hashes.iter().cloned().collect();
    for hash in new.unwrap_or_default() {
        if held.insert(hash.clone()) {
            hashes.push(hash);
        }
    }
    let excess = hashes.len().saturating_sub(MAX_HASHES);
    Some(hashes.split_off(excess))
}

/// Marks acceptance `id` forwarded, unless a newer acceptance merged into
/// it after `body` was sent.
pub(crate) async fn delivered(state: &State, id: Uuid, body: &Value) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE builder.terms_acceptances SET forwarded_at = timezone('UTC', now()) WHERE id = $1 AND body = $2",
    )
    .bind(id)
    .bind(body)
    .execute(state.items.pool())
    .await?;
    Ok(())
}

/// One delivery to the warehouse; whether it answered 2xx.
async fn send(state: &State, event: &str, body: &Value) -> bool {
    let Some(warehouse) = &state.settings.warehouse else {
        return false;
    };
    let context = match &warehouse.context_prefix {
        Some(prefix) => format!("{prefix}-builder"),
        None => "builder".to_string(),
    };
    let sent = state
        .guarded
        .post(&warehouse.url)
        .header("x-token", &warehouse.token)
        .json(&json!({ "context": context, "event": event, "body": body }))
        .send()
        .await;
    match sent {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            tracing::warn!(status = %response.status(), "warehouse refused a terms acceptance; it is retried");
            false
        }
        Err(error) => {
            tracing::warn!(
                connect = error.is_connect(),
                "warehouse forward of a terms acceptance failed; it is retried"
            );
            false
        }
    }
}

/// One pass of the forward: claims up to [`PASS_LIMIT`] due acceptances,
/// counting the attempt and setting the next one before sending, and
/// answers how many the warehouse took. Nothing without a warehouse or
/// while creator writes are off.
pub async fn forward_due(state: &State) -> anyhow::Result<u64> {
    static GATE: PassGate = PassGate::new("builder-terms-forward");
    if state.settings.warehouse.is_none() || !GATE.allows(state.settings.writes_enabled) {
        return Ok(0);
    }
    let due: Vec<(Uuid, String, Value)> = sqlx::query_as(
        "UPDATE builder.terms_acceptances t SET attempts = t.attempts + 1, \
           next_attempt_at = timezone('UTC', now()) + make_interval(mins => power(2, t.attempts)::int) \
         WHERE t.id IN (SELECT id FROM builder.terms_acceptances \
           WHERE forwarded_at IS NULL AND attempts < $1 AND next_attempt_at <= timezone('UTC', now()) \
           ORDER BY next_attempt_at LIMIT $2 FOR UPDATE SKIP LOCKED) \
         RETURNING t.id, t.event, t.body",
    )
    .bind(MAX_ATTEMPTS)
    .bind(PASS_LIMIT)
    .fetch_all(state.items.pool())
    .await?;
    let mut forwarded = 0;
    for (id, event, body) in due {
        if send(state, &event, &body).await {
            delivered(state, id, &body).await?;
            forwarded += 1;
        }
    }
    Ok(forwarded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_follows_builder_servers_schema() {
        let accepted = parse(&json!({ "email": "jane.doe@example.com", "other": 1 })).unwrap();
        assert_eq!(
            accepted,
            Acceptance {
                email: "jane.doe@example.com".into(),
                event: COLLECTION_EVENT,
                hashes: None
            }
        );
        let third_party = parse(&json!({ "email": "jane.doe@example.com", "event": THIRD_PARTY_EVENT, "hashes": ["a"] }))
            .unwrap();
        assert_eq!(third_party.hashes, Some(vec!["a".to_string()]));
        let too_many = vec!["h"; MAX_HASHES + 1];
        for refused in [
            json!([]),
            json!({ "email": "jane.doe@example.com", "hashes": too_many }),
            json!({ "email": "jane.doe@example.com", "event": null }),
        ] {
            assert!(parse(&refused).is_err(), "{refused}");
        }
        assert!(
            parse(&json!({ "email": "jane.doe@example.com", "hashes": vec!["h"; MAX_HASHES] }))
                .is_ok()
        );
    }
}
