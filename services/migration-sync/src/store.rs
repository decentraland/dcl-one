//! Durable, source-ordered application of the bounded Places pilot.
//!
//! One stream owns the pilot. Its row lock covers application and checkpoint
//! advancement; a failed transaction remains at the head instead of being skipped.
use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::model::{Decision, Envelope, parse_lsn, validate_stream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Idle,
    Applied,
    Blocked,
    AwaitingCommand,
}

/// Record an already validated snapshot boundary. Existing generations cannot
/// be replaced here: rebootstrap requires a separately reviewed state transition.
pub async fn register(pool: &PgPool, name: &str, generation: Uuid, baseline: &str) -> Result<()> {
    validate_stream(name)?;
    ensure!(!generation.is_nil(), "generation must not be nil");
    parse_lsn(baseline)?;
    let mut tx = pool.begin().await?;
    let exclusive: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('dcl-one/migration-follower',0))",
    )
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        exclusive,
        "normal serving process is active; registration requires follower isolation"
    );

    sqlx::query(
        "INSERT INTO migration_sync.streams(name, source, generation, baseline_lsn, applied_lsn)
         VALUES ($1, 'places', $2, $3::text::pg_lsn, $3::text::pg_lsn) ON CONFLICT DO NOTHING",
    )
    .bind(name)
    .bind(generation)
    .bind(baseline)
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query(
        "SELECT generation, baseline_lsn::text AS baseline
         FROM migration_sync.streams WHERE name = $1 FOR UPDATE",
    )
    .bind(name)
    .fetch_optional(&mut *tx)
    .await?
    .context("the Places source already belongs to a different stream")?;
    ensure!(
        row.try_get::<Uuid, _>("generation")? == generation
            && parse_lsn(&row.try_get::<String, _>("baseline")?)? == parse_lsn(baseline)?,
        "stream baseline or generation differs; registration cannot reset it"
    );
    tx.commit().await?;
    Ok(())
}

/// Return false for an identical retained transaction; changed payloads or forks
/// fail rather than replacing previously captured evidence.
pub async fn enqueue(pool: &PgPool, envelope: &Envelope) -> Result<bool> {
    envelope.validate()?;
    let payload = serde_json::to_value(envelope)?;
    let mut tx = pool.begin().await?;
    let stream = sqlx::query(
        "SELECT generation, applied_lsn::text AS applied
         FROM migration_sync.streams WHERE name = $1 FOR UPDATE",
    )
    .bind(&envelope.stream)
    .fetch_optional(&mut *tx)
    .await?
    .context("stream is not registered")?;
    ensure!(
        stream.try_get::<Uuid, _>("generation")? == envelope.generation,
        "stream generation mismatch"
    );
    let scoped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.capture_sources WHERE stream=$1 AND generation=$2)")
        .bind(&envelope.stream).bind(envelope.generation).fetch_one(&mut *tx).await?;
    if scoped {
        let mut entities: BTreeSet<&str> = envelope
            .changes
            .iter()
            .map(|change| change.key().1)
            .collect();
        if let Some(command) = &envelope.command {
            entities.insert(&command.entity_id);
        }
        for entity in entities {
            let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.capture_scope WHERE stream=$1 AND generation=$2 AND entity_id=$3)")
                .bind(&envelope.stream).bind(envelope.generation).bind(Uuid::parse_str(entity)?)
                .fetch_one(&mut *tx).await?;
            ensure!(allowed, "transaction escapes installed capture scope");
        }
    }
    let existing: Option<Value> = sqlx::query_scalar(
        "SELECT payload FROM migration_sync.inbox
         WHERE stream=$1 AND generation=$2 AND commit_lsn=$3::text::pg_lsn",
    )
    .bind(&envelope.stream)
    .bind(envelope.generation)
    .bind(&envelope.commit_lsn)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(existing) = existing {
        ensure!(
            existing == payload,
            "transaction payload conflicts with retained evidence"
        );
        tx.commit().await?;
        return Ok(false);
    }
    let applied = parse_lsn(&stream.try_get::<String, _>("applied")?)?;
    ensure!(
        parse_lsn(&envelope.commit_lsn)? > applied,
        "transaction is already represented by the baseline or applied position"
    );
    ensure!(
        parse_lsn(&envelope.previous_lsn)? >= applied,
        "transaction predecessor is behind the applied position"
    );
    let fork: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM migration_sync.inbox
         WHERE stream=$1 AND generation=$2 AND $3::text::pg_lsn < commit_lsn
           AND previous_lsn < $4::text::pg_lsn)",
    )
    .bind(&envelope.stream)
    .bind(envelope.generation)
    .bind(&envelope.previous_lsn)
    .bind(&envelope.commit_lsn)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        !fork,
        "transaction would fork or overlap the captured stream"
    );
    sqlx::query(
        "INSERT INTO migration_sync.inbox(stream,generation,commit_lsn,previous_lsn,payload)
         VALUES($1,$2,$3::text::pg_lsn,$4::text::pg_lsn,$5)",
    )
    .bind(&envelope.stream)
    .bind(envelope.generation)
    .bind(&envelope.commit_lsn)
    .bind(&envelope.previous_lsn)
    .bind(payload)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn pause(pool: &PgPool, name: &str, paused: bool) -> Result<()> {
    // UPDATE takes the same stream-row lock as process_next, so a successful
    // pause waits for an already-running apply and prevents the next one.
    let result =
        sqlx::query("UPDATE migration_sync.streams SET paused=$2, updated_at=now() WHERE name=$1")
            .bind(name)
            .bind(paused)
            .execute(pool)
            .await?;
    ensure!(result.rows_affected() == 1, "stream is not registered");
    Ok(())
}

pub async fn status(pool: &PgPool, name: &str) -> Result<Value> {
    let row = sqlx::query(
        "SELECT source, generation, baseline_lsn::text AS baseline, applied_lsn::text AS applied, paused,
          (SELECT count(*) FROM migration_sync.inbox i WHERE i.stream=s.name AND status='pending') AS pending,
          (SELECT count(*) FROM migration_sync.inbox i WHERE i.stream=s.name AND status='blocked') AS blocked,
          (SELECT commit_lsn::text FROM migration_sync.inbox i WHERE i.stream=s.name AND status='pending' ORDER BY commit_lsn LIMIT 1) AS next_pending,
          (SELECT error_code FROM migration_sync.inbox i WHERE i.stream=s.name AND previous_lsn=s.applied_lsn) AS head_error,
          (SELECT commit_lsn::text FROM migration_sync.inbox i WHERE i.stream=s.name AND previous_lsn=s.applied_lsn) AS head_lsn
         FROM migration_sync.streams s WHERE name=$1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?
    .context("stream is not registered")?;
    Ok(json!({
        "stream": name,
        "source": row.try_get::<String,_>("source")?,
        "generation": row.try_get::<Uuid,_>("generation")?,
        "baseline_lsn": row.try_get::<String,_>("baseline")?,
        "applied_lsn": row.try_get::<String,_>("applied")?,
        "paused": row.try_get::<bool,_>("paused")?,
        "pending": row.try_get::<i64,_>("pending")?,
        "blocked": row.try_get::<i64,_>("blocked")?,
        "next_pending_lsn": row.try_get::<Option<String>,_>("next_pending")?,
        "head_error": row.try_get::<Option<String>,_>("head_error")?,
        "head_lsn": row.try_get::<Option<String>,_>("head_lsn")?,
    }))
}

pub async fn process_next(pool: &PgPool, name: &str) -> Result<Outcome> {
    let mut tx = pool.begin().await?;
    let stream = sqlx::query(
        "SELECT generation, applied_lsn::text AS applied, paused
         FROM migration_sync.streams WHERE name=$1 FOR UPDATE",
    )
    .bind(name)
    .fetch_optional(&mut *tx)
    .await?
    .context("stream is not registered")?;
    if stream.try_get::<bool, _>("paused")? {
        return Ok(Outcome::Idle);
    }
    let generation: Uuid = stream.try_get("generation")?;
    let applied: String = stream.try_get("applied")?;
    let next = sqlx::query(
        "SELECT commit_lsn::text AS position, payload, status
         FROM migration_sync.inbox WHERE stream=$1 AND generation=$2 AND previous_lsn=$3::text::pg_lsn",
    )
    .bind(name)
    .bind(generation)
    .bind(&applied)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(next) = next else {
        // A later transaction can be retained while its predecessor is missing.
        return Ok(Outcome::Idle);
    };
    if next.try_get::<String, _>("status")? == "blocked" {
        return Ok(Outcome::Blocked);
    }
    let position: String = next.try_get("position")?;
    let payload: Value = next.try_get("payload")?;
    let envelope = serde_json::from_value::<Envelope>(payload);
    let envelope = match envelope {
        Ok(value)
            if value.validate().is_ok()
                && value.stream == name
                && value.generation == generation
                && value.previous_lsn == applied
                && value.commit_lsn == position =>
        {
            value
        }
        _ => return block(tx, name, generation, &position, "invalid_envelope").await,
    };
    match envelope.classify() {
        Decision::Blocked(code) => return block(tx, name, generation, &position, code).await,
        Decision::ExecuteCommand => {
            let command = envelope
                .command
                .as_ref()
                .context("validated command missing")?;
            let receipt = sqlx::query(
                "SELECT commit_lsn::text AS position, command=$4 AS matches_command, state=$5 AS matches_state
                 FROM migration_sync.command_receipts WHERE stream=$1 AND generation=$2 AND operation_id=$3",
            ).bind(name).bind(generation).bind(command.operation_id)
             .bind(serde_json::to_value(command)?).bind(serde_json::to_value(&command.expected)?)
             .fetch_optional(&mut *tx).await?;
            let Some(receipt) = receipt else {
                return Ok(Outcome::AwaitingCommand);
            };
            if receipt.try_get::<String, _>("position")? != position
                || !receipt.try_get::<bool, _>("matches_command")?
                || !receipt.try_get::<bool, _>("matches_state")?
            {
                return block(tx, name, generation, &position, "command_receipt_conflict").await;
            }
            let state = sqlx::query("SELECT user_activity, created_at FROM explore.user_favorites WHERE \"user\"=$1 AND entity_id=$2")
                .bind(&command.user).bind(&command.entity_id).fetch_optional(&mut *tx).await?;
            let equivalent = match (state, &command.expected) {
                (None, None) => true,
                (Some(row), Some(expected)) => {
                    row.try_get::<f64, _>("user_activity")? == expected.user_activity
                        && row.try_get::<chrono::NaiveDateTime, _>("created_at")?
                            == expected.created_at.naive_utc()
                }
                _ => false,
            };
            if !equivalent {
                return block(tx, name, generation, &position, "native_state_diverged").await;
            }
            sqlx::query(
                "INSERT INTO migration_sync.favorite_versions(stream,generation,\"user\",entity_id,commit_lsn,deleted)
                 VALUES($1,$2,$3,$4,$5::text::pg_lsn,$6)
                 ON CONFLICT(stream,generation,\"user\",entity_id) DO UPDATE SET commit_lsn=EXCLUDED.commit_lsn,deleted=EXCLUDED.deleted",
            ).bind(name).bind(generation).bind(&command.user).bind(&command.entity_id).bind(&position).bind(!command.favorite)
             .execute(&mut *tx).await?;
            // Native mutation and receipt already committed together. Do not
            // reapply captured favorite writes or increment counters again.
        }
    }
    sqlx::query(
        "UPDATE migration_sync.inbox SET status='applied', applied_at=now()
         WHERE stream=$1 AND generation=$2 AND commit_lsn=$3::text::pg_lsn",
    )
    .bind(name)
    .bind(generation)
    .bind(&position)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE migration_sync.streams SET applied_lsn=$2::text::pg_lsn, updated_at=now() WHERE name=$1",
    )
    .bind(name)
    .bind(&position)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Outcome::Applied)
}

async fn block(
    mut tx: Transaction<'_, Postgres>,
    stream: &str,
    generation: Uuid,
    position: &str,
    code: &str,
) -> Result<Outcome> {
    sqlx::query("UPDATE migration_sync.inbox SET status='blocked', error_code=$4 WHERE stream=$1 AND generation=$2 AND commit_lsn=$3::text::pg_lsn")
        .bind(stream).bind(generation).bind(position).bind(code)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Outcome::Blocked)
}

/// The pointer is read without holding a transaction across network I/O. The
/// follower rechecks the predecessor under the same stream lock as apply.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandPointer {
    pub stream: String,
    pub generation: Uuid,
    pub commit_lsn: String,
}

pub async fn pending_command(pool: &PgPool, name: &str) -> Result<Option<CommandPointer>> {
    let row = sqlx::query("SELECT i.payload FROM migration_sync.streams s JOIN migration_sync.inbox i ON i.stream=s.name AND i.generation=s.generation AND i.previous_lsn=s.applied_lsn WHERE s.name=$1 AND NOT s.paused AND i.status='pending'")
        .bind(name).fetch_optional(pool).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let envelope: Envelope = serde_json::from_value(row.try_get("payload")?)?;
    envelope.validate()?;
    Ok(
        matches!(envelope.classify(), Decision::ExecuteCommand).then_some(CommandPointer {
            stream: envelope.stream,
            generation: envelope.generation,
            commit_lsn: envelope.commit_lsn,
        }),
    )
}

/// A definitive rejection may block only the same current head, and must not
/// overwrite an atomic receipt committed by a concurrent/retried dispatcher.
pub async fn reject_command(pool: &PgPool, pointer: &CommandPointer) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT name FROM migration_sync.streams WHERE name=$1 FOR UPDATE")
        .bind(&pointer.stream)
        .fetch_optional(&mut *tx)
        .await?;
    sqlx::query("UPDATE migration_sync.inbox i SET status='blocked',error_code='native_command_rejected' FROM migration_sync.streams s WHERE s.name=i.stream AND s.generation=i.generation AND i.previous_lsn=s.applied_lsn AND i.stream=$1 AND i.generation=$2 AND i.commit_lsn=$3::text::pg_lsn AND i.status='pending' AND NOT EXISTS(SELECT 1 FROM migration_sync.command_receipts r WHERE r.stream=i.stream AND r.generation=i.generation AND r.commit_lsn=i.commit_lsn)")
        .bind(&pointer.stream).bind(pointer.generation).bind(&pointer.commit_lsn).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
