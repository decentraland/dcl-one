//! Ordered native favorite replay for the dedicated migration follower.
//!
//! The request names durable captured evidence; it never supplies a mutation.
//! Native facts and their receipt commit together under the stream's apply lock.
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::shared::errors::ApiError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRequest {
    pub stream: String,
    pub generation: Uuid,
    pub commit_lsn: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FavoriteState {
    pub user_activity: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Command {
    operation_id: Uuid,
    request_hash: String,
    user: String,
    entity_id: String,
    favorite: bool,
    user_activity: f64,
    expected: Option<FavoriteState>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReplayReceipt {
    pub operation_id: Uuid,
    pub commit_lsn: String,
    pub request_hash: String,
    pub state: Option<FavoriteState>,
}

fn canonical_lsn(value: &str) -> bool {
    let Some((high, low)) = value.split_once('/') else {
        return false;
    };
    [high, low].into_iter().all(|part| {
        !part.is_empty()
            && part.len() <= 8
            && (part == "0" || !part.starts_with('0'))
            && part
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    })
}

fn validate(request: &ReplayRequest, command: &Command) -> Result<(), ApiError> {
    let wallet = &command.user;
    let valid = !request.stream.is_empty()
        && request.stream.len() <= 64
        && request
            .stream
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !request.generation.is_nil()
        && canonical_lsn(&request.commit_lsn)
        && !command.operation_id.is_nil()
        && wallet.len() == 42
        && wallet.starts_with("0x")
        && wallet[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && Uuid::parse_str(&command.entity_id).is_ok_and(|id| id.to_string() == command.entity_id)
        && command.user_activity.is_finite()
        && command.favorite == command.expected.is_some()
        && command.request_hash.len() == 64
        && command
            .request_hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && command.expected.as_ref().is_none_or(|state| {
            state.user_activity.is_finite() && state.created_at.timestamp_subsec_nanos() % 1000 == 0
        });
    if !valid {
        return Err(ApiError::bad_request("Invalid captured favorite command"));
    }
    Ok(())
}

pub async fn replay(pool: &PgPool, request: &ReplayRequest) -> Result<ReplayReceipt, ApiError> {
    if !canonical_lsn(&request.commit_lsn) {
        return Err(ApiError::bad_request("Invalid source position"));
    }
    let mut tx = pool.begin().await?;
    let stream = sqlx::query("SELECT generation, paused, applied_lsn::text AS applied FROM migration_sync.streams WHERE name=$1 FOR UPDATE")
        .bind(&request.stream).fetch_optional(&mut *tx).await?
        .ok_or_else(|| ApiError::not_found("Unknown migration stream"))?;
    if stream.try_get::<Uuid, _>("generation")? != request.generation {
        return Err(ApiError::conflict("Migration generation differs"));
    }
    let inbox = sqlx::query("SELECT payload, status, previous_lsn::text AS previous FROM migration_sync.inbox WHERE stream=$1 AND generation=$2 AND commit_lsn=$3::text::pg_lsn")
        .bind(&request.stream).bind(request.generation).bind(&request.commit_lsn)
        .fetch_optional(&mut *tx).await?.ok_or_else(|| ApiError::not_found("Source transaction has not been captured"))?;
    let payload: Value = inbox.try_get("payload")?;
    if payload["version"] != 1
        || payload["source"] != "places"
        || payload["mode"] != "dual"
        || payload["stream"] != request.stream
        || payload["generation"] != request.generation.to_string()
        || payload["commit_lsn"] != request.commit_lsn
        || payload["previous_lsn"] != inbox.try_get::<String, _>("previous")?
    {
        return Err(ApiError::conflict(
            "Captured transaction is not an approved command",
        ));
    }
    let command: Command = serde_json::from_value(payload["command"].clone())
        .map_err(|_| ApiError::bad_request("Missing or malformed source command receipt"))?;
    validate(request, &command)?;
    let allowed: bool = sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM migration_sync.capture_sources WHERE stream=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM migration_sync.capture_scope WHERE stream=$1 AND generation=$2 AND entity_id=$3)")
        .bind(&request.stream).bind(request.generation).bind(Uuid::parse_str(&command.entity_id).map_err(|_|ApiError::bad_request("Invalid entity identity"))?)
        .fetch_one(&mut *tx).await?;
    if !allowed {
        return Err(ApiError::conflict(
            "Command escapes installed capture scope",
        ));
    }

    let receipt = sqlx::query("SELECT commit_lsn::text AS position, command=$4 AS matches, state FROM migration_sync.command_receipts WHERE stream=$1 AND generation=$2 AND operation_id=$3")
        .bind(&request.stream).bind(request.generation).bind(command.operation_id)
        .bind(&payload["command"]).fetch_optional(&mut *tx).await?;
    if let Some(receipt) = receipt {
        if !receipt.try_get::<bool, _>("matches")?
            || receipt.try_get::<String, _>("position")? != request.commit_lsn
        {
            return Err(ApiError::conflict(
                "Operation identity conflicts with an existing receipt",
            ));
        }
        let state = serde_json::from_value(receipt.try_get::<Value, _>("state")?)
            .map_err(|_| ApiError::internal("Invalid stored operation receipt"))?;
        return Ok(ReplayReceipt {
            operation_id: command.operation_id,
            commit_lsn: request.commit_lsn.clone(),
            request_hash: command.request_hash,
            state,
        });
    }
    if stream.try_get::<bool, _>("paused")? {
        return Err(ApiError::service_unavailable("Migration stream is paused"));
    }
    if inbox.try_get::<String, _>("status")? != "pending"
        || inbox.try_get::<String, _>("previous")? != stream.try_get::<String, _>("applied")?
    {
        return Err(ApiError::conflict(
            "Command is not the next unblocked source transaction",
        ));
    }
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM explore.place WHERE id=$1 AND NOT world AND origin='local')",
    )
    .bind(&command.entity_id)
    .fetch_one(&mut *tx)
    .await?;
    if !known && command.favorite {
        return Err(ApiError::conflict(
            "Command catalogue identity is unavailable",
        ));
    }
    let created_at = command
        .expected
        .as_ref()
        .map(|state| state.created_at.naive_utc())
        .unwrap_or_else(|| Utc::now().naive_utc());
    crate::places::interactions::set_favorite_at_in(
        &mut tx,
        &command.entity_id,
        &command.user,
        command.favorite,
        command.user_activity,
        created_at,
    )
    .await?;
    let row = sqlx::query("SELECT user_activity, created_at FROM explore.user_favorites WHERE \"user\"=$1 AND entity_id=$2")
        .bind(&command.user).bind(&command.entity_id).fetch_optional(&mut *tx).await?;
    let state = row
        .map(|row| {
            Ok::<_, sqlx::Error>(FavoriteState {
                user_activity: row.try_get("user_activity")?,
                created_at: row.try_get::<NaiveDateTime, _>("created_at")?.and_utc(),
            })
        })
        .transpose()?;
    if state != command.expected {
        return Err(ApiError::conflict(
            "Native favorite result diverged from the source receipt",
        ));
    }
    sqlx::query("INSERT INTO migration_sync.command_receipts(stream,generation,operation_id,commit_lsn,command,state) VALUES($1,$2,$3,$4::text::pg_lsn,$5,$6)")
        .bind(&request.stream).bind(request.generation).bind(command.operation_id).bind(&request.commit_lsn)
        .bind(&payload["command"]).bind(serde_json::to_value(&state).map_err(|_| ApiError::internal("Cannot encode operation receipt"))?)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(ReplayReceipt {
        operation_id: command.operation_id,
        commit_lsn: request.commit_lsn.clone(),
        request_hash: command.request_hash,
        state,
    })
}
