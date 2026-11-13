//! Bounded owner/quota projection. Source job/provider calls never run here.
//! Baseline facts must already match the isolated target; capture then applies
//! ordered before/after facts and stops on target drift or an unknown identity.
use anyhow::{Context, Result, ensure};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

const SOURCE_SQL: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/migration-sync/source/worlds.sql"
));
const MAX_CHANGES: usize = 1000;
const MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Quota {
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Change {
    Owner {
        world: String,
        before: Option<String>,
        after: Option<String>,
    },
    Quota {
        wallet: String,
        before: Option<Quota>,
        after: Option<Quota>,
    },
    Unsupported {
        reason: String,
    },
}
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Idle,
    Applied,
    Blocked,
}

fn wallet_valid(s: &str) -> bool {
    s.len() == 42
        && s.starts_with("0x")
        && s[2..]
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn world_valid(s: &str) -> bool {
    s.len() <= 255
        && s.ends_with(".eth")
        && s.len() > 4
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
}
fn canonical_scope(values: &[String], wallet: bool) -> Result<Vec<String>> {
    ensure!(
        !values.is_empty() && values.len() <= 1000,
        "scope requires 1 to 1000 identities"
    );
    ensure!(
        values.iter().all(|s| if wallet {
            wallet_valid(s)
        } else {
            world_valid(s)
        }),
        "scope identity is not canonical"
    );
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    ensure!(sorted.len() == values.len(), "scope contains duplicates");
    Ok(sorted)
}
async fn isolate(conn: &mut PgConnection) -> Result<()> {
    let isolated: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('dcl-one/migration-follower',0))",
    )
    .fetch_one(conn)
    .await?;
    ensure!(
        isolated,
        "normal runtime is active; isolated follower required"
    );
    Ok(())
}

/// Operator-only source installation. Scope and generation are immutable.
pub async fn install(source: &PgPool, worlds: &[String], wallets: &[String]) -> Result<Value> {
    let worlds = canonical_scope(worlds, false)?;
    let wallets = canonical_scope(wallets, true)?;
    let mut tx = source.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(179321,2)")
        .execute(&mut *tx)
        .await?;
    let installed: bool =
        sqlx::query_scalar("SELECT to_regnamespace('migration_worlds_capture') IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    if installed {
        let actual_worlds: Vec<String> = sqlx::query_scalar(
            "SELECT world_name FROM migration_worlds_capture.world_scope ORDER BY world_name",
        )
        .fetch_all(&mut *tx)
        .await?;
        let actual_wallets: Vec<String> = sqlx::query_scalar(
            "SELECT wallet FROM migration_worlds_capture.wallet_scope ORDER BY wallet",
        )
        .fetch_all(&mut *tx)
        .await?;
        ensure!(
            actual_worlds == worlds && actual_wallets == wallets,
            "capture scope is immutable"
        );
    } else {
        sqlx::query("LOCK TABLE public.worlds, public.blocked IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM public.worlds WHERE name=ANY($1) AND owner=ANY($2)",
        )
        .bind(&worlds)
        .bind(&wallets)
        .fetch_one(&mut *tx)
        .await?;
        ensure!(
            count == worlds.len() as i64,
            "every scoped world needs a canonical owner in wallet scope"
        );
        let aliases: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.worlds WHERE lower(name)=ANY($1) AND name<>lower(name)) OR EXISTS(SELECT 1 FROM public.blocked WHERE lower(wallet)=ANY($2) AND wallet<>lower(wallet))").bind(&worlds).bind(&wallets).fetch_one(&mut *tx).await?;
        ensure!(
            !aliases,
            "noncanonical source aliases require reconciliation"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(SOURCE_SQL))
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO migration_worlds_capture.world_scope SELECT unnest($1::text[])")
            .bind(&worlds)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO migration_worlds_capture.wallet_scope SELECT unnest($1::text[])")
            .bind(&wallets)
            .execute(&mut *tx)
            .await?;
    }
    let row = sqlx::query(
        "SELECT version,generation,position FROM migration_worlds_capture.control WHERE singleton",
    )
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        row.try_get::<i32, _>("version")? == 1,
        "unsupported Worlds capture version"
    );
    let result = json!({"generation":row.try_get::<Uuid,_>("generation")?,"position":row.try_get::<i64,_>("position")?,"world_count":worlds.len(),"wallet_count":wallets.len()});
    tx.commit().await?;
    Ok(result)
}

async fn quota(conn: &mut PgConnection, wallet: &str) -> Result<Option<Quota>> {
    let row = sqlx::query(
        "SELECT created_at,updated_at FROM worlds.quota_blocks WHERE wallet=$1 FOR UPDATE",
    )
    .bind(wallet)
    .fetch_optional(conn)
    .await?;
    row.map(|r| {
        Ok(Quota {
            created_at: r.try_get("created_at")?,
            updated_at: r.try_get("updated_at")?,
        })
    })
    .transpose()
}

/// Establish the cursor against an already imported, matching baseline. No
/// existing business fact is overwritten here. Quiesce source writers for the
/// initial migration if necessary; later changes are retained beyond the cursor.
pub async fn bootstrap(source: &PgPool, target: &PgPool, stream: &str) -> Result<Value> {
    crate::model::validate_stream(stream)?;
    let mut guard = source.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(179321,2)")
        .execute(&mut *guard)
        .await?;
    let mut snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *snapshot)
        .await?;
    let control = sqlx::query(
        "SELECT version,generation,position FROM migration_worlds_capture.control WHERE singleton",
    )
    .fetch_one(&mut *snapshot)
    .await?;
    ensure!(
        control.try_get::<i32, _>("version")? == 1,
        "unsupported Worlds capture version"
    );
    let generation: Uuid = control.try_get("generation")?;
    let position: i64 = control.try_get("position")?;
    guard.commit().await?;
    let worlds: Vec<String> = sqlx::query_scalar("SELECT world_name FROM migration_worlds_capture.world_scope ORDER BY world_name LIMIT 1001").fetch_all(&mut *snapshot).await?;
    let wallets: Vec<String> = sqlx::query_scalar(
        "SELECT wallet FROM migration_worlds_capture.wallet_scope ORDER BY wallet LIMIT 1001",
    )
    .fetch_all(&mut *snapshot)
    .await?;
    canonical_scope(&worlds, false)?;
    canonical_scope(&wallets, true)?;
    let owners =
        sqlx::query("SELECT name,owner FROM public.worlds WHERE name=ANY($1) ORDER BY name")
            .bind(&worlds)
            .fetch_all(&mut *snapshot)
            .await?;
    ensure!(
        owners.len() == worlds.len(),
        "source baseline world missing"
    );
    let blocks = sqlx::query("SELECT wallet,created_at,updated_at FROM public.blocked WHERE wallet=ANY($1) ORDER BY wallet").bind(&wallets).fetch_all(&mut *snapshot).await?;
    let mut tx = target.begin().await?;
    isolate(&mut tx).await?;
    let existing: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.worlds_streams)")
            .fetch_one(&mut *tx)
            .await?;
    ensure!(
        !existing,
        "Worlds baseline already registered; cannot reset generation or progress"
    );
    let outside: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM worlds.worlds WHERE owner=ANY($1) AND NOT(name=ANY($2)))",
    )
    .bind(&wallets)
    .bind(&worlds)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        !outside,
        "quota scope would affect unregistered target worlds"
    );
    for row in owners {
        let name: String = row.try_get("name")?;
        let owner: Option<String> = row.try_get("owner")?;
        ensure!(
            owner.as_ref().is_some_and(|v| wallets.contains(v)),
            "source owner escapes wallet scope"
        );
        let actual =
            sqlx::query("SELECT owner,mirror_source FROM worlds.worlds WHERE name=$1 FOR UPDATE")
                .bind(&name)
                .fetch_optional(&mut *tx)
                .await?
                .context("baseline target world missing")?;
        ensure!(
            actual
                .try_get::<Option<String>, _>("mirror_source")?
                .is_none()
                && actual.try_get::<Option<String>, _>("owner")? == owner,
            "target world must be local and match source baseline owner"
        );
    }
    for wallet in &wallets {
        let expected = blocks
            .iter()
            .find(|r| r.get::<String, _>("wallet") == *wallet)
            .map(|r| Quota {
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            });
        ensure!(
            quota(&mut tx, wallet).await? == expected,
            "quota baseline mismatch; import source before bootstrap"
        );
    }
    sqlx::query("INSERT INTO migration_sync.worlds_streams(name,generation,baseline_position,captured_position,applied_position) VALUES($1,$2,$3,$3,$3)").bind(stream).bind(generation).bind(position).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO migration_sync.worlds_scope SELECT $1,unnest($2::text[])")
        .bind(stream)
        .bind(&worlds)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO migration_sync.worlds_wallet_scope SELECT $1,unnest($2::text[])")
        .bind(stream)
        .bind(&wallets)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    snapshot.commit().await?;
    Ok(json!({"generation":generation,"position":position,"paused":true}))
}

/// Capture one complete source transaction. The target cursor and immutable
/// payload commit together; source connection errors never advance progress.
pub async fn pump_once(source: &PgPool, target: &PgPool, stream: &str) -> Result<bool> {
    let mut tx = target.begin().await?;
    let state = sqlx::query("SELECT generation,captured_position FROM migration_sync.worlds_streams WHERE name=$1 FOR UPDATE").bind(stream).fetch_one(&mut *tx).await?;
    let generation: Uuid = state.try_get("generation")?;
    let next = state
        .try_get::<i64, _>("captured_position")?
        .checked_add(1)
        .context("capture position overflow")?;
    let mut read = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *read)
        .await?;
    let control = sqlx::query(
        "SELECT version,generation,position FROM migration_worlds_capture.control WHERE singleton",
    )
    .fetch_one(&mut *read)
    .await?;
    ensure!(
        control.try_get::<i32, _>("version")? == 1
            && control.try_get::<Uuid, _>("generation")? == generation,
        "source capture identity changed"
    );
    let source_worlds: Vec<String> = sqlx::query_scalar("SELECT world_name FROM migration_worlds_capture.world_scope ORDER BY world_name LIMIT 1001").fetch_all(&mut *read).await?;
    let source_wallets: Vec<String> = sqlx::query_scalar(
        "SELECT wallet FROM migration_worlds_capture.wallet_scope ORDER BY wallet LIMIT 1001",
    )
    .fetch_all(&mut *read)
    .await?;
    let target_worlds: Vec<String> = sqlx::query_scalar(
        "SELECT world_name FROM migration_sync.worlds_scope WHERE stream=$1 ORDER BY world_name",
    )
    .bind(stream)
    .fetch_all(&mut *tx)
    .await?;
    let target_wallets: Vec<String> = sqlx::query_scalar(
        "SELECT wallet FROM migration_sync.worlds_wallet_scope WHERE stream=$1 ORDER BY wallet",
    )
    .bind(stream)
    .fetch_all(&mut *tx)
    .await?;
    ensure!(
        source_worlds == target_worlds && source_wallets == target_wallets,
        "source capture scope changed"
    );
    ensure!(
        control.try_get::<i64, _>("position")? >= next - 1,
        "source capture history regressed"
    );
    if control.try_get::<i64, _>("position")? < next {
        sqlx::query("UPDATE migration_sync.worlds_streams SET last_contact_at=now() WHERE name=$1")
            .bind(stream)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        read.commit().await?;
        return Ok(false);
    }
    let rows: Vec<Value> = sqlx::query_scalar("SELECT payload FROM migration_worlds_capture.changes WHERE position=$1 ORDER BY ordinal LIMIT 1001").bind(next).fetch_all(&mut *read).await?;
    ensure!(!rows.is_empty(), "source capture history has a gap");
    ensure!(
        rows.len() <= MAX_CHANGES && serde_json::to_vec(&rows)?.len() <= MAX_BYTES,
        "source transaction exceeds bounded Worlds capture"
    );
    sqlx::query("INSERT INTO migration_sync.worlds_inbox(stream,generation,position,changes) VALUES($1,$2,$3,$4)").bind(stream).bind(generation).bind(next).bind(json!(rows)).execute(&mut *tx).await?;
    sqlx::query("UPDATE migration_sync.worlds_streams SET captured_position=$2,last_contact_at=now() WHERE name=$1").bind(stream).bind(next).execute(&mut *tx).await?;
    read.commit().await?;
    tx.commit().await?;
    Ok(true)
}

async fn apply_change(conn: &mut PgConnection, stream: &str, change: Change) -> Result<()> {
    match change {
        Change::Unsupported { reason } => {
            let _ = reason;
            anyhow::bail!("unsupported source change");
        }
        Change::Owner {
            world,
            before,
            after,
        } => {
            ensure!(world_valid(&world), "noncanonical world");
            let scoped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.worlds_scope WHERE stream=$1 AND world_name=$2)").bind(stream).bind(&world).fetch_one(&mut *conn).await?;
            ensure!(scoped, "world escapes scope");
            for owner in [&before, &after] {
                let owner = owner.as_ref().context("owner removal unsupported")?;
                ensure!(wallet_valid(owner), "noncanonical owner");
                let scoped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.worlds_wallet_scope WHERE stream=$1 AND wallet=$2)").bind(stream).bind(owner).fetch_one(&mut *conn).await?;
                ensure!(scoped, "owner escapes wallet scope");
            }
            let result = sqlx::query("UPDATE worlds.worlds SET owner=$3 WHERE name=$1 AND owner IS NOT DISTINCT FROM $2 AND mirror_source IS NULL").bind(world).bind(before).bind(after).execute(conn).await?;
            ensure!(
                result.rows_affected() == 1,
                "owner projection conflicts with target"
            );
        }
        Change::Quota {
            wallet,
            before,
            after,
        } => {
            ensure!(wallet_valid(&wallet), "noncanonical quota wallet");
            let scoped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM migration_sync.worlds_wallet_scope WHERE stream=$1 AND wallet=$2)").bind(stream).bind(&wallet).fetch_one(&mut *conn).await?;
            ensure!(scoped, "quota wallet escapes scope");
            ensure!(before.is_some() || after.is_some(), "empty quota outcome");
            for value in [&before, &after].into_iter().flatten() {
                ensure!(
                    value.created_at <= value.updated_at,
                    "invalid quota timestamp order"
                );
            }
            ensure!(
                quota(conn, &wallet).await? == before,
                "quota projection conflicts with target"
            );
            if let Some(value) = after {
                sqlx::query("INSERT INTO worlds.quota_blocks(wallet,created_at,updated_at) VALUES($1,$2,$3) ON CONFLICT(wallet) DO UPDATE SET created_at=excluded.created_at,updated_at=excluded.updated_at").bind(wallet).bind(value.created_at).bind(value.updated_at).execute(conn).await?;
            } else {
                sqlx::query("DELETE FROM worlds.quota_blocks WHERE wallet=$1")
                    .bind(wallet)
                    .execute(conn)
                    .await?;
            }
        }
    }
    Ok(())
}

/// Apply the captured head atomically with its progress. Any incompatible fact
/// rolls the whole source transaction back and records a payload-free blocker.
pub async fn apply_next(target: &PgPool, stream: &str) -> Result<Outcome> {
    let mut tx = target.begin().await?;
    isolate(&mut tx).await?;
    let state = sqlx::query("SELECT generation,applied_position,captured_position,paused,error_code FROM migration_sync.worlds_streams WHERE name=$1 FOR UPDATE").bind(stream).fetch_one(&mut *tx).await?;
    if state.try_get::<Option<String>, _>("error_code")?.is_some() {
        return Ok(Outcome::Blocked);
    }
    if state.try_get::<bool, _>("paused")? {
        return Ok(Outcome::Idle);
    }
    let applied: i64 = state.try_get("applied_position")?;
    if applied == state.try_get::<i64, _>("captured_position")? {
        return Ok(Outcome::Idle);
    }
    let generation: Uuid = state.try_get("generation")?;
    let next = applied.checked_add(1).context("apply position overflow")?;
    let payload: Value = sqlx::query_scalar("SELECT changes FROM migration_sync.worlds_inbox WHERE stream=$1 AND generation=$2 AND position=$3 AND applied_at IS NULL").bind(stream).bind(generation).bind(next).fetch_one(&mut *tx).await?;
    sqlx::query("SAVEPOINT worlds_apply")
        .execute(&mut *tx)
        .await?;
    let result: Result<()> = async {
        let changes: Vec<Change> = serde_json::from_value(payload).map_err(|_| anyhow::anyhow!("invalid Worlds outcome"))?;
        ensure!(!changes.is_empty() && changes.len() <= MAX_CHANGES, "invalid transaction size");
        // A quota wallet is a cross-world fact. Never affect worlds omitted from
        // this target's approved scope, even if another writer added one later.
        let outside: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM worlds.worlds w JOIN migration_sync.worlds_wallet_scope s ON s.wallet=w.owner AND s.stream=$1 WHERE NOT EXISTS(SELECT 1 FROM migration_sync.worlds_scope a WHERE a.stream=$1 AND a.world_name=w.name))").bind(stream).fetch_one(&mut *tx).await?;
        ensure!(!outside, "unregistered target world shares quota wallet");
        for change in changes { apply_change(&mut tx,stream,change).await?; }
        Ok(())
    }.await;
    if result.is_err() {
        sqlx::query("ROLLBACK TO SAVEPOINT worlds_apply")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE migration_sync.worlds_streams SET error_code='worlds_projection_conflict',paused=true WHERE name=$1").bind(stream).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(Outcome::Blocked);
    }
    sqlx::query("UPDATE migration_sync.worlds_inbox SET applied_at=now() WHERE stream=$1 AND generation=$2 AND position=$3").bind(stream).bind(generation).bind(next).execute(&mut *tx).await?;
    sqlx::query("UPDATE migration_sync.worlds_streams SET applied_position=$2 WHERE name=$1")
        .bind(stream)
        .bind(next)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Outcome::Applied)
}

pub async fn set_paused(
    target: &PgPool,
    stream: &str,
    paused: bool,
    rehearsal: bool,
) -> Result<()> {
    ensure!(paused || rehearsal, "Worlds apply remains rehearsal-only");
    let result = sqlx::query("UPDATE migration_sync.worlds_streams SET paused=$2 WHERE name=$1 AND ($2 OR error_code IS NULL)").bind(stream).bind(paused).execute(target).await?;
    ensure!(
        result.rows_affected() == 1,
        "missing or blocked Worlds stream"
    );
    Ok(())
}

pub async fn status(target: &PgPool, stream: &str) -> Result<Value> {
    let row = sqlx::query("SELECT generation,baseline_position,captured_position,applied_position,paused,error_code,extract(epoch FROM clock_timestamp()-last_contact_at)::double precision AS contact_age FROM migration_sync.worlds_streams WHERE name=$1").bind(stream).fetch_one(target).await?;
    Ok(
        json!({"generation":row.try_get::<Uuid,_>("generation")?,"baseline_position":row.try_get::<i64,_>("baseline_position")?,"captured_position":row.try_get::<i64,_>("captured_position")?,"applied_position":row.try_get::<i64,_>("applied_position")?,"paused":row.try_get::<bool,_>("paused")?,"error_code":row.try_get::<Option<String>,_>("error_code")?,"contact_age_seconds":row.try_get::<f64,_>("contact_age")?}),
    )
}
