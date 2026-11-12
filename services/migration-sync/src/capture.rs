//! Explicit source-local outbox capture for the bounded Places pilot.
//!
//! Positions are committed outbox ordinals encoded in the envelope's cursor
//! representation, not WAL LSNs. The source generation and adapter identity
//! prevent silently interpreting another capture history as this stream.
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::model::{Envelope, FavoriteChange, parse_lsn, validate_stream};
use crate::store;

pub const SOURCE_SQL: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/migration-sync/source/places.sql"
));
const MAX_BASELINE_ROWS: usize = 100_000;

#[derive(Debug, Serialize)]
pub struct SourceInfo {
    pub adapter: &'static str,
    pub generation: Uuid,
    pub position: i64,
    pub scope: Vec<Uuid>,
}

/// Convert the outbox adapter's position to the existing ordered cursor format.
/// This representation does not make it a WAL location.
pub fn position_cursor(position: i64) -> Result<String> {
    ensure!(position >= 0, "outbox position cannot be negative");
    let value = position as u64;
    Ok(format!("{:X}/{:X}", value >> 32, value & 0xffff_ffff))
}

pub async fn source_info(source: &PgPool) -> Result<SourceInfo> {
    let row = sqlx::query(
        "SELECT version,generation,position FROM migration_capture.control WHERE singleton",
    )
    .fetch_one(source)
    .await?;
    ensure!(
        row.try_get::<i32, _>("version")? == 1,
        "unsupported capture adapter version"
    );
    let scope = sqlx::query_scalar::<_, Uuid>(
        "SELECT entity_id FROM migration_capture.scope ORDER BY entity_id",
    )
    .fetch_all(source)
    .await?;
    Ok(SourceInfo {
        adapter: "places_outbox_v1",
        generation: row.try_get("generation")?,
        position: row.try_get("position")?,
        scope,
    })
}

/// Operator-only installation, never invoked by polling. Its transaction is
/// local to the source and does not contact the destination.
pub async fn install(source: &PgPool, scope: &[Uuid]) -> Result<SourceInfo> {
    ensure!(
        !scope.is_empty() && scope.len() <= 1000,
        "capture requires 1 to 1000 explicit catalogue IDs"
    );
    ensure!(
        scope.iter().all(|id| !id.is_nil()),
        "capture scope cannot contain a nil ID"
    );
    let mut wanted = scope.to_vec();
    wanted.sort();
    wanted.dedup();
    ensure!(
        wanted.len() == scope.len(),
        "capture scope contains duplicate IDs"
    );
    let mut tx = source.begin().await?;
    // Serialize installers as well as preventing source writes while the
    // triggers and scope become visible together.
    sqlx::query("SELECT pg_advisory_xact_lock(179321,1)")
        .execute(&mut *tx)
        .await?;
    let installed: bool =
        sqlx::query_scalar("SELECT to_regnamespace('migration_capture') IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    if installed {
        let actual: Vec<Uuid> =
            sqlx::query_scalar("SELECT entity_id FROM migration_capture.scope ORDER BY entity_id")
                .fetch_all(&mut *tx)
                .await?;
        ensure!(
            wanted == actual,
            "installed capture scope is immutable; use a new reviewed baseline to change it"
        );
    } else {
        let present: i64 =
            sqlx::query_scalar("SELECT count(*) FROM public.places WHERE lower(id::text)=ANY($1::text[]) AND NOT world")
                .bind(wanted.iter().map(Uuid::to_string).collect::<Vec<_>>())
                .fetch_one(&mut *tx)
                .await?;
        ensure!(
            present == wanted.len() as i64,
            "capture scope must contain existing Genesis places"
        );
        // The only dynamic SQL is this repository-owned, fixed installation.
        sqlx::raw_sql(sqlx::AssertSqlSafe(SOURCE_SQL))
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO migration_capture.scope(entity_id) SELECT unnest($1::uuid[])")
            .bind(&wanted)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    source_info(source).await
}

/// Establish a consistent source snapshot without keeping the source lock while
/// reading its full data or waiting for the target. The source pool needs at
/// least two connections: one lock holder and one repeatable-read snapshot.
pub async fn bootstrap(source: &PgPool, target: &PgPool, stream: &str) -> Result<SourceInfo> {
    validate_stream(stream)?;
    let mut guard = source.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(179321,1)")
        .execute(&mut *guard)
        .await?;
    let mut snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *snapshot)
        .await?;
    let row = sqlx::query(
        "SELECT version,generation,position FROM migration_capture.control WHERE singleton",
    )
    .fetch_one(&mut *snapshot)
    .await?;
    ensure!(
        row.try_get::<i32, _>("version")? == 1,
        "unsupported capture adapter version"
    );
    let generation: Uuid = row.try_get("generation")?;
    let position: i64 = row.try_get("position")?;
    let scope: Vec<Uuid> =
        sqlx::query_scalar("SELECT entity_id FROM migration_capture.scope ORDER BY entity_id")
            .fetch_all(&mut *snapshot)
            .await?;
    ensure!(!scope.is_empty(), "capture scope is empty");
    guard.commit().await?;
    // The snapshot was established under the same lock used by every captured
    // write. Later source commits are in the outbox beyond this boundary.
    let rows = sqlx::query(
        "SELECT f.\"user\"::text AS actor,f.entity_id::text AS entity_id,
                f.user_activity::double precision AS activity,f.created_at AS created_at
         FROM public.user_favorites f JOIN migration_capture.scope s ON s.entity_id::text=lower(f.entity_id::text)
         ORDER BY f.entity_id,f.\"user\" LIMIT 100001",
    )
    .fetch_all(&mut *snapshot)
    .await?;
    ensure!(
        rows.len() <= MAX_BASELINE_ROWS,
        "pilot baseline exceeds bounded export size; choose a smaller scope"
    );
    let mut baseline = Vec::with_capacity(rows.len());
    for row in rows {
        let change = FavoriteChange::Upsert {
            user: row.try_get("actor")?,
            entity_id: row.try_get("entity_id")?,
            user_activity: row.try_get("activity")?,
            created_at: row.try_get::<DateTime<Utc>, _>("created_at")?,
        };
        change.validate()?;
        baseline.push(change);
    }
    snapshot.commit().await?;
    let boundary = position_cursor(position)?;
    let mut tx = target.begin().await?;
    // Serializes independent bootstrap attempts without invoking a normal
    // backend, provider or source-side operation.
    let fenced: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('dcl-one/migration-follower',0))",
    )
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        fenced,
        "normal destination instance is active; stop it before bootstrap"
    );
    ensure!(!generation.is_nil(), "source generation cannot be nil");
    let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.streams)")
        .fetch_one(&mut *tx)
        .await?;
    ensure!(
        !occupied,
        "bootstrap requires an unregistered destination; never overwrite an active generation"
    );
    let existing: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM explore.user_favorites WHERE entity_id=ANY($1))",
    )
    .bind(scope.iter().map(ToString::to_string).collect::<Vec<_>>())
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        !existing,
        "bootstrap scope already contains target favorites; use a clean scoped baseline"
    );
    for entity in &scope {
        let local: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM explore.place WHERE id=$1 AND NOT world AND origin='local')")
            .bind(entity.to_string()).fetch_one(&mut *tx).await?;
        ensure!(local, "bootstrap catalogue identity is missing or mirrored");
    }
    sqlx::query("INSERT INTO migration_sync.streams(name,source,generation,baseline_lsn,applied_lsn) VALUES($1,'places',$2,$3::text::pg_lsn,$3::text::pg_lsn)")
        .bind(stream).bind(generation).bind(&boundary).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO migration_sync.capture_scope(stream,generation,entity_id) SELECT $1,$2,unnest($3::uuid[])")
        .bind(stream).bind(generation).bind(&scope).execute(&mut *tx).await?;
    for change in baseline {
        if let FavoriteChange::Upsert {
            user,
            entity_id,
            user_activity,
            created_at,
        } = change
        {
            sqlx::query("INSERT INTO explore.user_favorites(\"user\",entity_id,user_activity,created_at) VALUES($1,$2,$3,$4)")
                .bind(user).bind(entity_id).bind(user_activity).bind(created_at.naive_utc()).execute(&mut *tx).await?;
        }
    }
    for entity in &scope {
        sqlx::query("INSERT INTO explore.place_interaction_counts(entity_id,favorites,likes,dislikes)
            SELECT $1,(SELECT count(*) FROM explore.user_favorites WHERE entity_id=$1),
                (SELECT count(*) FROM explore.user_likes WHERE entity_id=$1 AND \"like\"),
                (SELECT count(*) FROM explore.user_likes WHERE entity_id=$1 AND NOT \"like\")
            ON CONFLICT(entity_id) DO UPDATE SET favorites=EXCLUDED.favorites,likes=EXCLUDED.likes,dislikes=EXCLUDED.dislikes,updated_at=timezone('UTC',now())")
            .bind(entity.to_string()).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO migration_sync.capture_sources(stream,generation,captured_position,observed_position,last_contact_at) VALUES($1,$2,$3,$3,clock_timestamp())")
        .bind(stream).bind(generation).bind(position).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(SourceInfo {
        adapter: "places_outbox_v1",
        generation,
        position,
        scope,
    })
}

/// Durably capture at most one complete source transaction. Source outbox rows
/// are never acknowledged/deleted by this reader. A crash after enqueue and
/// before offset advancement replays the same immutable transaction safely.
pub async fn pump_once(source: &PgPool, target: &PgPool, stream: &str) -> Result<bool> {
    let info = source_info(source).await?;
    let row = sqlx::query("SELECT generation,adapter,captured_position FROM migration_sync.capture_sources WHERE stream=$1")
        .bind(stream).fetch_optional(target).await?.context("stream has no capture bootstrap")?;
    ensure!(
        row.try_get::<Uuid, _>("generation")? == info.generation,
        "source generation changed; stop capture and rebootstrap explicitly"
    );
    ensure!(
        row.try_get::<String, _>("adapter")? == info.adapter,
        "capture adapter mismatch"
    );
    let approved_scope: Vec<Uuid> = sqlx::query_scalar("SELECT entity_id FROM migration_sync.capture_scope WHERE stream=$1 AND generation=$2 ORDER BY entity_id")
        .bind(stream).bind(info.generation).fetch_all(target).await?;
    ensure!(
        approved_scope == info.scope,
        "source capture scope changed; stop and rebootstrap explicitly"
    );
    let captured: i64 = row.try_get("captured_position")?;
    ensure!(info.position >= captured, "source history moved backwards");
    let next = sqlx::query("SELECT position,command FROM migration_capture.transactions WHERE position>$1 ORDER BY position LIMIT 1")
        .bind(captured).fetch_optional(source).await?;
    if let Some(next) = next {
        let position: i64 = next.try_get("position")?;
        ensure!(position == captured + 1, "source capture history has a gap");
        let size = sqlx::query("SELECT count(*) AS rows, COALESCE(sum(octet_length(payload::text)),0)::bigint AS bytes FROM migration_capture.changes WHERE position=$1")
            .bind(position).fetch_one(source).await?;
        ensure!(
            size.try_get::<i64, _>("rows")? <= 1000 && size.try_get::<i64, _>("bytes")? <= 1048576,
            "source transaction exceeds bounded capture size"
        );
        let changes: Vec<Value> = sqlx::query_scalar(
            "SELECT payload FROM migration_capture.changes WHERE position=$1 ORDER BY ordinal LIMIT 1001",
        )
        .bind(position)
        .fetch_all(source)
        .await?;
        let command: Option<Value> = next.try_get("command")?;
        let envelope: Envelope = serde_json::from_value(json!({
            "version":1,"source":"places","stream":stream,"generation":info.generation,
            "previous_lsn":position_cursor(captured)?,"commit_lsn":position_cursor(position)?,
            "mode":if command.is_some(){"dual"}else{"source_only"},"changes":changes,"command":command
        })).context("source outbox transaction does not satisfy the capture contract")?;
        envelope.validate()?;
        store::enqueue(target, &envelope).await?;
        let result = sqlx::query(
            "UPDATE migration_sync.capture_sources SET captured_position=$3,
            observed_position=GREATEST(observed_position,$4),last_contact_at=clock_timestamp()
            WHERE stream=$1 AND generation=$2 AND captured_position=$5",
        )
        .bind(stream)
        .bind(info.generation)
        .bind(position)
        .bind(info.position.max(position))
        .bind(captured)
        .execute(target)
        .await?;
        // Another capture process may have advanced the exact same immutable
        // position. It cannot cause a transaction to be dropped.
        if result.rows_affected() == 0 {
            let current:i64=sqlx::query_scalar("SELECT captured_position FROM migration_sync.capture_sources WHERE stream=$1 AND generation=$2")
                .bind(stream).bind(info.generation).fetch_one(target).await?;
            ensure!(current >= position, "capture offset changed unexpectedly");
        }
        Ok(true)
    } else {
        ensure!(
            info.position == captured,
            "source capture history is missing transactions"
        );
        sqlx::query("UPDATE migration_sync.capture_sources SET observed_position=GREATEST(observed_position,$3),last_contact_at=clock_timestamp() WHERE stream=$1 AND generation=$2")
            .bind(stream).bind(info.generation).bind(info.position).execute(target).await?;
        Ok(false)
    }
}

/// Check target projection at a consistent source boundary while source writers
/// continue afterward. Intended for scoped rehearsal/reconciliation reports.
pub async fn reconcile(source: &PgPool, target: &PgPool, stream: &str) -> Result<Value> {
    let info = source_info(source).await?;
    let status = store::status(target, stream).await?;
    ensure!(
        status["generation"] == json!(info.generation),
        "source and target generations differ"
    );
    let approved_scope: Vec<Uuid> = sqlx::query_scalar("SELECT entity_id FROM migration_sync.capture_scope WHERE stream=$1 AND generation=$2 ORDER BY entity_id")
        .bind(stream).bind(info.generation).fetch_all(target).await?;
    ensure!(
        approved_scope == info.scope,
        "source and target capture scopes differ"
    );
    let applied = status["applied_lsn"]
        .as_str()
        .context("missing applied position")?;
    ensure!(
        parse_lsn(applied)? == info.position as u64,
        "reconciliation requires a caught-up boundary"
    );
    // Equality here is a point-in-time diagnostic; concurrent source changes
    // invalidate the result below rather than being called a consistent proof.
    let source_rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_array(f.\"user\"::text,f.entity_id::text,f.user_activity,to_char(f.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')) FROM public.user_favorites f JOIN migration_capture.scope s ON s.entity_id::text=lower(f.entity_id::text) ORDER BY f.\"user\",f.entity_id")
        .fetch_all(source).await?;
    let target_rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_array(\"user\",entity_id,user_activity,to_char(created_at,'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')) FROM explore.user_favorites WHERE entity_id=ANY($1) ORDER BY \"user\",entity_id")
        .bind(info.scope.iter().map(ToString::to_string).collect::<Vec<_>>()).fetch_all(target).await?;
    let source_counts: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_array(s.entity_id::text,count(f.entity_id)) FROM migration_capture.scope s LEFT JOIN public.user_favorites f ON lower(f.entity_id::text)=s.entity_id::text GROUP BY s.entity_id ORDER BY s.entity_id")
        .fetch_all(source).await?;
    let target_counts: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_array(s.entity_id,c.favorites) FROM unnest($1::text[]) AS s(entity_id) LEFT JOIN explore.place_interaction_counts c USING(entity_id) ORDER BY s.entity_id")
        .bind(info.scope.iter().map(ToString::to_string).collect::<Vec<_>>()).fetch_all(target).await?;
    let after = source_info(source).await?;
    ensure!(
        after.position == info.position && after.generation == info.generation,
        "source changed during reconciliation; retry at a stable boundary"
    );
    let target_after = store::status(target, stream).await?;
    ensure!(
        target_after["applied_lsn"] == status["applied_lsn"]
            && target_after["generation"] == status["generation"],
        "target changed during reconciliation"
    );
    Ok(
        json!({"stream":stream,"generation":info.generation,"position":info.position,"matches":source_rows==target_rows && source_counts==target_counts,"rows_match":source_rows==target_rows,"counts_match":source_counts==target_counts,"source_rows":source_rows.len(),"target_rows":target_rows.len()}),
    )
}
