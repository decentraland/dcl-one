//! Bounded, source-only adapters sharing transport and ordered metadata.
//! SQL identifiers come only from the closed Domain enum, never caller input.
use crate::{
    capture::position_cursor,
    events_schedules,
    model::{Envelope, FavoriteChange, parse_lsn, validate_stream},
    social_settings, store,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Domain {
    Events,
    SocialSettings,
}
impl Domain {
    pub fn source(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::SocialSettings => "social-settings",
        }
    }
    pub fn adapter(self) -> &'static str {
        match self {
            Self::Events => events_schedules::ADAPTER,
            Self::SocialSettings => social_settings::ADAPTER,
        }
    }
    fn schema(self) -> &'static str {
        match self {
            Self::Events => "migration_capture_events",
            Self::SocialSettings => "migration_capture_social_settings",
        }
    }
    fn key_column(self) -> &'static str {
        match self {
            Self::Events => "entity_id",
            Self::SocialSettings => "address",
        }
    }
    fn lock_key(self) -> i32 {
        match self {
            Self::Events => 2,
            Self::SocialSettings => 3,
        }
    }
    fn sql(self) -> &'static str {
        match self {
            Self::Events => events_schedules::SOURCE_SQL,
            Self::SocialSettings => social_settings::SOURCE_SQL,
        }
    }
    fn validate_key(self, key: &str) -> Result<()> {
        match self {
            Self::Events => events_schedules::ScheduleChange::Delete { id: key.into() }.validate(),
            Self::SocialSettings => social_settings::validate_address(key),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct SourceInfo {
    pub source: &'static str,
    pub adapter: &'static str,
    pub generation: Uuid,
    pub position: i64,
    pub scope: Vec<String>,
}

async fn info(connection: &mut PgConnection, domain: Domain) -> Result<SourceInfo> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT version,generation,position FROM {}.control WHERE singleton",
        domain.schema()
    )))
    .fetch_one(&mut *connection)
    .await?;
    ensure!(
        row.try_get::<i32, _>("version")? == 1,
        "unsupported domain capture version"
    );
    let scope: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {}::text FROM {}.scope ORDER BY 1",
        domain.key_column(),
        domain.schema()
    )))
    .fetch_all(&mut *connection)
    .await?;
    ensure!(
        !scope.is_empty() && scope.len() <= 1000,
        "invalid installed domain scope"
    );
    for key in &scope {
        domain.validate_key(key)?;
    }
    let result = SourceInfo {
        source: domain.source(),
        adapter: domain.adapter(),
        generation: row.try_get("generation")?,
        position: row.try_get("position")?,
        scope,
    };
    ensure!(
        !result.generation.is_nil() && result.position >= 0,
        "invalid source capture identity"
    );
    Ok(result)
}
async fn lock(connection: &mut PgConnection, domain: Domain) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(179321,$1)")
        .bind(domain.lock_key())
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn install(source: &PgPool, domain: Domain, keys: &[String]) -> Result<SourceInfo> {
    ensure!(
        !keys.is_empty() && keys.len() <= 1000,
        "domain capture requires 1 to 1000 explicit keys"
    );
    for key in keys {
        domain.validate_key(key)?;
    }
    let mut wanted = keys.to_vec();
    wanted.sort();
    wanted.dedup();
    ensure!(wanted.len() == keys.len(), "duplicate domain scope keys");
    let mut tx = source.begin().await?;
    lock(&mut tx, domain).await?;
    let exists: bool = sqlx::query_scalar("SELECT to_regnamespace($1) IS NOT NULL")
        .bind(domain.schema())
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        ensure!(
            info(&mut tx, domain).await?.scope == wanted,
            "installed domain scope is immutable"
        );
    } else {
        sqlx::raw_sql(domain.sql()).execute(&mut *tx).await?;
        let projection = if domain == Domain::Events {
            "unnest($1::text[])::uuid"
        } else {
            "unnest($1::text[])"
        };
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {}.scope({}) SELECT {}",
            domain.schema(),
            domain.key_column(),
            projection
        )))
        .bind(&wanted)
        .execute(&mut *tx)
        .await?;
    }
    let result = info(&mut tx, domain).await?;
    tx.commit().await?;
    Ok(result)
}

async fn source_changes(
    connection: &mut PgConnection,
    domain: Domain,
) -> Result<Vec<FavoriteChange>> {
    let rows: Vec<Value> = match domain {
        Domain::Events => events_schedules::read_snapshot(connection)
            .await?
            .into_iter()
            .map(|schedule| {
                serde_json::to_value(events_schedules::ScheduleChange::Upsert { schedule })
            })
            .collect::<std::result::Result<_, _>>()?,
        Domain::SocialSettings => social_settings::read_snapshot(connection)
            .await?
            .into_iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<_, _>>()?,
    };
    ensure!(
        rows.len() <= 100_000,
        "domain snapshot exceeds bounded export size"
    );
    let changes: Vec<FavoriteChange> = rows
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<_, _>>()
        .context("invalid source domain snapshot")?;
    for change in &changes {
        change.validate()?;
        ensure!(
            change.source() == domain.source(),
            "snapshot source mismatch"
        );
    }
    Ok(changes)
}
async fn target_rows(
    connection: &mut PgConnection,
    domain: Domain,
    scope: &[String],
) -> Result<Vec<Value>> {
    match domain {
        Domain::Events => Ok(events_schedules::read_target(connection, scope)
            .await?
            .into_iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<_, _>>()?),
        Domain::SocialSettings => social_settings::canonical_target(connection, scope).await,
    }
}
async fn canonical_source(connection: &mut PgConnection, domain: Domain) -> Result<Vec<Value>> {
    match domain {
        Domain::Events => Ok(events_schedules::read_snapshot(connection)
            .await?
            .into_iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<_, _>>()?),
        Domain::SocialSettings => Ok(social_settings::read_snapshot(connection)
            .await?
            .into_iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<_, _>>()?),
    }
}

pub async fn bootstrap(
    source: &PgPool,
    target: &PgPool,
    domain: Domain,
    stream: &str,
) -> Result<SourceInfo> {
    validate_stream(stream)?;
    let mut guard = source.begin().await?;
    lock(&mut guard, domain).await?;
    let mut snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *snapshot)
        .await?;
    let boundary = info(&mut snapshot, domain).await?;
    guard.commit().await?;
    let rows = source_changes(&mut snapshot, domain).await?;
    snapshot.commit().await?;
    let mut tx = target.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let admitted: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('dcl-one/migration-follower',0))",
    )
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        admitted,
        "normal serving process is active; bootstrap requires isolation"
    );
    let occupied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM migration_sync.streams WHERE name=$1 OR source=$2)",
    )
    .bind(stream)
    .bind(domain.source())
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        !occupied,
        "domain source or stream is already registered; never replace its baseline"
    );
    ensure!(
        target_rows(&mut tx, domain, &boundary.scope)
            .await?
            .is_empty(),
        "domain bootstrap requires an empty scoped target"
    );
    let cursor = position_cursor(boundary.position)?;
    sqlx::query("INSERT INTO migration_sync.streams(name,source,generation,baseline_lsn,applied_lsn) VALUES($1,$2,$3,$4::text::pg_lsn,$4::text::pg_lsn)")
        .bind(stream).bind(domain.source()).bind(boundary.generation).bind(cursor).execute(&mut *tx).await?;
    for change in rows {
        if let Some(schedule) = change.event_change() {
            events_schedules::apply(&mut tx, &schedule).await?;
        } else if let Some(settings) = change.social_change() {
            social_settings::apply(&mut tx, &settings).await?;
        } else {
            anyhow::bail!("snapshot has unsupported domain change");
        }
    }
    sqlx::query("INSERT INTO migration_sync.domain_scope(stream,generation,key) SELECT $1,$2,unnest($3::text[])").bind(stream).bind(boundary.generation).bind(&boundary.scope).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO migration_sync.capture_sources(stream,generation,adapter,captured_position,observed_position,last_contact_at) VALUES($1,$2,$3,$4,$4,clock_timestamp())")
        .bind(stream).bind(boundary.generation).bind(domain.adapter()).bind(boundary.position).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(boundary)
}
async fn verify_target(
    target: &PgPool,
    domain: Domain,
    stream: &str,
    source: &SourceInfo,
) -> Result<i64> {
    let row=sqlx::query("SELECT s.source,c.generation,c.adapter,c.captured_position FROM migration_sync.capture_sources c JOIN migration_sync.streams s ON s.name=c.stream AND s.generation=c.generation WHERE c.stream=$1")
        .bind(stream).fetch_optional(target).await?.context("domain stream has no capture bootstrap")?;
    ensure!(
        row.try_get::<String, _>("source")? == domain.source()
            && row.try_get::<String, _>("adapter")? == domain.adapter()
            && row.try_get::<Uuid, _>("generation")? == source.generation,
        "domain source identity differs"
    );
    let scope:Vec<String>=sqlx::query_scalar("SELECT key FROM migration_sync.domain_scope WHERE stream=$1 AND generation=$2 ORDER BY key").bind(stream).bind(source.generation).fetch_all(target).await?;
    ensure!(scope == source.scope, "domain source scope differs");
    let captured: i64 = row.try_get("captured_position")?;
    ensure!(
        source.position >= captured,
        "domain history moved backwards"
    );
    Ok(captured)
}
pub async fn pump_once(
    source: &PgPool,
    target: &PgPool,
    domain: Domain,
    stream: &str,
) -> Result<bool> {
    let boundary = info(&mut *source.acquire().await?, domain).await?;
    let captured = verify_target(target, domain, stream, &boundary).await?;
    let next = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT position,command FROM {}.transactions WHERE position>$1 ORDER BY position LIMIT 1",
        domain.schema()
    )))
    .bind(captured)
    .fetch_optional(source)
    .await?;
    if let Some(row) = next {
        let position: i64 = row.try_get("position")?;
        ensure!(position == captured + 1, "domain capture history has a gap");
        ensure!(
            row.try_get::<Option<Value>, _>("command")?.is_none(),
            "native commands are not supported for this domain"
        );
        let size=sqlx::query(sqlx::AssertSqlSafe(format!("SELECT count(*) AS rows, COALESCE(sum(octet_length(payload::text)),0)::bigint AS bytes FROM {}.changes WHERE position=$1",domain.schema())))
            .bind(position).fetch_one(source).await?;
        ensure!(
            size.try_get::<i64, _>("rows")? <= 1000
                && size.try_get::<i64, _>("bytes")? <= 1024 * 1024,
            "domain transaction exceeds bounded capture size"
        );
        let changes: Vec<Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT payload FROM {}.changes WHERE position=$1 ORDER BY ordinal LIMIT 1001",
            domain.schema()
        )))
        .bind(position)
        .fetch_all(source)
        .await?;
        let envelope:Envelope=serde_json::from_value(json!({"version":1,"source":domain.source(),"stream":stream,"generation":boundary.generation,"previous_lsn":position_cursor(captured)?,"commit_lsn":position_cursor(position)?,"mode":"source_only","changes":changes})).context("invalid domain capture envelope")?;
        store::enqueue(target, &envelope).await?;
        let updated=sqlx::query("UPDATE migration_sync.capture_sources SET captured_position=$3,observed_position=GREATEST(observed_position,$4),last_contact_at=clock_timestamp() WHERE stream=$1 AND generation=$2 AND captured_position=$5")
            .bind(stream).bind(boundary.generation).bind(position).bind(boundary.position.max(position)).bind(captured).execute(target).await?;
        if updated.rows_affected() == 0 {
            let current:i64=sqlx::query_scalar("SELECT captured_position FROM migration_sync.capture_sources WHERE stream=$1 AND generation=$2").bind(stream).bind(boundary.generation).fetch_one(target).await?;
            ensure!(current >= position, "domain progress changed unexpectedly");
        }
        Ok(true)
    } else {
        ensure!(
            boundary.position == captured,
            "domain capture history is missing transactions"
        );
        sqlx::query("UPDATE migration_sync.capture_sources SET observed_position=GREATEST(observed_position,$3),last_contact_at=clock_timestamp() WHERE stream=$1 AND generation=$2").bind(stream).bind(boundary.generation).bind(boundary.position).execute(target).await?;
        Ok(false)
    }
}
pub async fn reconcile(
    source: &PgPool,
    target: &PgPool,
    domain: Domain,
    stream: &str,
) -> Result<Value> {
    let before = info(&mut *source.acquire().await?, domain).await?;
    verify_target(target, domain, stream, &before).await?;
    let status = store::status(target, stream).await?;
    ensure!(
        parse_lsn(
            status["applied_lsn"]
                .as_str()
                .context("missing domain cursor")?
        )? == before.position as u64,
        "domain reconciliation requires caught-up application"
    );
    let mut source_snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *source_snapshot)
        .await?;
    let source_rows = canonical_source(&mut source_snapshot, domain).await?;
    source_snapshot.commit().await?;
    let mut target_snapshot = target.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *target_snapshot)
        .await?;
    let target_rows = target_rows(&mut target_snapshot, domain, &before.scope).await?;
    target_snapshot.commit().await?;
    let after = info(&mut *source.acquire().await?, domain).await?;
    ensure!(
        before.generation == after.generation
            && before.position == after.position
            && before.scope == after.scope,
        "domain source changed during reconciliation"
    );
    verify_target(target, domain, stream, &after).await?;
    let after_target = store::status(target, stream).await?;
    ensure!(
        after_target["generation"] == status["generation"]
            && after_target["applied_lsn"] == status["applied_lsn"],
        "domain target changed during reconciliation"
    );
    Ok(
        json!({"stream":stream,"source":domain.source(),"generation":before.generation,"position":before.position,"matches":source_rows==target_rows,"source_rows":source_rows.len(),"target_rows":target_rows.len()}),
    )
}
