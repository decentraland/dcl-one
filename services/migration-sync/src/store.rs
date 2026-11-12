//! Durable, source-ordered application of the bounded Places pilot.
//!
//! One stream owns the pilot. Its row lock covers application and checkpoint
//! advancement; a failed transaction remains at the head instead of being skipped.
use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sqlx::{Acquire, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::model::{Decision, Envelope, FavoriteChange, parse_lsn, validate_stream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Idle,
    Applied,
    Blocked,
}

/// Record an already validated snapshot boundary. Existing generations cannot
/// be replaced here: rebootstrap requires a separately reviewed state transition.
pub async fn register(pool: &PgPool, name: &str, generation: Uuid, baseline: &str) -> Result<()> {
    validate_stream(name)?;
    ensure!(!generation.is_nil(), "generation must not be nil");
    parse_lsn(baseline)?;
    let mut tx = pool.begin().await?;
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
          (SELECT error_code FROM migration_sync.inbox i WHERE i.stream=s.name AND previous_lsn=s.applied_lsn) AS head_error
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
    if let Decision::Blocked(code) = envelope.classify() {
        return block(tx, name, generation, &position, code).await;
    }
    // Savepoint keeps the head lock when a domain statement fails, while
    // rolling back every domain write before recording the blocked outcome.
    let mut application = tx.begin().await?;
    let result = apply(&mut application, &envelope).await;
    match result {
        Ok(()) => application.commit().await?,
        Err(_) => {
            application.rollback().await?;
            return block(tx, name, generation, &position, "apply_failed").await;
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

async fn apply(tx: &mut Transaction<'_, Postgres>, envelope: &Envelope) -> Result<()> {
    let mut entities = BTreeSet::new();
    for change in &envelope.changes {
        let (user, entity_id, deleted) = match change {
            FavoriteChange::Upsert {
                user,
                entity_id,
                user_activity,
                created_at,
            } => {
                let valid: Option<bool> = sqlx::query_scalar(
                    "SELECT true FROM explore.place WHERE id=$1 AND NOT world AND origin='local' FOR SHARE",
                ).bind(entity_id).fetch_optional(&mut **tx).await?;
                ensure!(
                    valid == Some(true),
                    "catalogue identity is unavailable or not owned by this baseline"
                );
                sqlx::query(
                    "INSERT INTO explore.user_favorites(\"user\",entity_id,user_activity,created_at) VALUES($1,$2,$3,$4)
                     ON CONFLICT(\"user\",entity_id) DO UPDATE SET user_activity=EXCLUDED.user_activity,created_at=EXCLUDED.created_at",
                ).bind(user).bind(entity_id).bind(user_activity).bind(created_at.naive_utc()).execute(&mut **tx).await?;
                (user, entity_id, false)
            }
            FavoriteChange::Delete { user, entity_id } => {
                // A deleted catalogue row is a valid cause of a favorite deletion.
                sqlx::query(
                    "DELETE FROM explore.user_favorites WHERE \"user\"=$1 AND entity_id=$2",
                )
                .bind(user)
                .bind(entity_id)
                .execute(&mut **tx)
                .await?;
                (user, entity_id, true)
            }
        };
        entities.insert(entity_id);
        sqlx::query(
            "INSERT INTO migration_sync.favorite_versions(stream,generation,\"user\",entity_id,commit_lsn,deleted)
             VALUES($1,$2,$3,$4,$5::text::pg_lsn,$6)
             ON CONFLICT(stream,generation,\"user\",entity_id) DO UPDATE SET commit_lsn=EXCLUDED.commit_lsn,deleted=EXCLUDED.deleted",
        ).bind(&envelope.stream).bind(envelope.generation).bind(user).bind(entity_id).bind(&envelope.commit_lsn).bind(deleted)
            .execute(&mut **tx).await?;
    }
    for entity in entities {
        sqlx::query(
            "INSERT INTO explore.place_interaction_counts(entity_id,favorites,likes,dislikes,updated_at)
             SELECT $1,(SELECT count(*) FROM explore.user_favorites WHERE entity_id=$1),
               (SELECT count(*) FROM explore.user_likes WHERE entity_id=$1 AND \"like\"),
               (SELECT count(*) FROM explore.user_likes WHERE entity_id=$1 AND NOT \"like\"),timezone('UTC',now())
             ON CONFLICT(entity_id) DO UPDATE SET favorites=EXCLUDED.favorites,likes=EXCLUDED.likes,
               dislikes=EXCLUDED.dislikes,updated_at=EXCLUDED.updated_at",
        ).bind(entity).execute(&mut **tx).await?;
    }
    Ok(())
}
