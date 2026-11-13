//! Accepted source player-storage commands. No scene execution or row fallback.
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

const SOURCE_LOCK: &str = "SELECT pg_advisory_xact_lock(179323,1)";
const TARGET_LOCK: &str =
    "SELECT pg_advisory_xact_lock(hashtextextended('dcl-one/migration-follower',0))";
pub const MAX_COMMAND_BYTES: usize = 1_100_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub version: u32,
    pub op: String,
    pub world_name: String,
    pub place_id: Uuid,
    pub scene_id: String,
    pub player_address: Option<String>,
    pub key: Option<String>,
    pub value_text: Option<String>,
    pub value_size: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Install the fixed source adapter and seal an explicit immutable scene scope.
/// Source DDL is operator-only; capture itself needs read access only.
pub async fn install(source: &PgPool, scopes: &[Scope]) -> Result<Value> {
    ensure!(
        !scopes.is_empty() && scopes.len() <= 1000,
        "player install requires 1 to 1000 scopes"
    );
    let mut wanted = scopes.to_vec();
    wanted.sort_by(|a, b| (&a.world_name, a.place_id).cmp(&(&b.world_name, b.place_id)));
    for s in &wanted {
        validate_scope(&s.world_name, s.place_id, &s.scene_id)?;
    }
    ensure!(
        wanted
            .windows(2)
            .all(|w| w[0].world_name != w[1].world_name || w[0].place_id != w[1].place_id),
        "duplicate player scope"
    );
    let mut tx = source.begin().await?;
    sqlx::query(SOURCE_LOCK).execute(&mut *tx).await?;
    let installed: bool =
        sqlx::query_scalar("SELECT to_regnamespace('migration_player_capture') IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    if installed {
        ensure!(
            source_scopes(&mut tx).await? == wanted,
            "installed player scope differs"
        );
        let info = sqlx::query(
            "SELECT version,sealed FROM migration_player_capture.control WHERE singleton",
        )
        .fetch_one(&mut *tx)
        .await?;
        ensure!(
            info.try_get::<i32, _>("version")? == 1 && info.try_get::<bool, _>("sealed")?,
            "incomplete player capture installation"
        );
    } else {
        sqlx::raw_sql(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/migration-sync/source/player.sql"
        )))
        .execute(&mut *tx)
        .await?;
        for s in &wanted {
            sqlx::query("INSERT INTO migration_player_capture.scope(world_name,place_id,scene_id) VALUES($1,$2,$3)").bind(&s.world_name).bind(s.place_id).bind(&s.scene_id).execute(&mut *tx).await?;
        }
        sqlx::query("SELECT migration_player_capture.seal_scope()")
            .execute(&mut *tx)
            .await?;
    }
    let generation: Uuid = sqlx::query_scalar(
        "SELECT generation FROM migration_player_capture.control WHERE singleton",
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({"generation":generation,"scopes":wanted.len(),"sealed":true}))
}

impl Command {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "unsupported player command version");
        validate_scope(&self.world_name, self.place_id, &self.scene_id)?;
        ensure!(
            matches!(
                self.op.as_str(),
                "set" | "delete" | "clear_player" | "clear_scene"
            ),
            "unsupported player command"
        );
        if self.op == "clear_scene" {
            ensure!(
                self.player_address.is_none(),
                "scene clear cannot name a player"
            );
        } else {
            let wallet = self.player_address.as_deref().unwrap_or("");
            ensure!(
                wallet.len() == 42
                    && wallet.starts_with("0x")
                    && wallet[2..]
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "invalid player address"
            );
        }
        if matches!(self.op.as_str(), "set" | "delete") {
            ensure!(
                self.key.as_ref().is_some_and(|v| !v.is_empty()
                    && v.chars().count() <= 255
                    && !v.contains('\0')),
                "invalid player key"
            );
        } else {
            ensure!(self.key.is_none(), "clear command cannot name key");
        }
        if self.op == "set" {
            let value = self
                .value_text
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing player value"))?;
            ensure!(
                value.len() <= 1_048_576 && self.value_size == Some(value.len() as i64),
                "invalid player value size"
            );
            serde_json::from_str::<Value>(value)
                .map_err(|_| anyhow::anyhow!("invalid player JSON value"))?;
            ensure!(
                self.created_at.is_some() && self.updated_at.is_some(),
                "missing player timestamps"
            );
            ensure!(
                self.created_at <= self.updated_at,
                "invalid player timestamp order"
            );
            ensure!(
                self.created_at
                    .is_some_and(|t| t.timestamp_subsec_nanos() % 1000 == 0)
                    && self
                        .updated_at
                        .is_some_and(|t| t.timestamp_subsec_nanos() % 1000 == 0),
                "unsupported timestamp precision"
            );
        } else {
            ensure!(
                self.value_text.is_none()
                    && self.value_size.is_none()
                    && self.created_at.is_none()
                    && self.updated_at.is_none(),
                "delete command contains value"
            );
        }
        Ok(())
    }
}
fn validate_scope(world: &str, place: Uuid, scene: &str) -> Result<()> {
    ensure!(
        world.ends_with(".eth")
            && world.len() <= 255
            && world == world.to_lowercase()
            && world
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'),
        "unsupported player world"
    );
    ensure!(
        !place.is_nil() && !scene.is_empty() && scene.len() <= 256,
        "invalid player scene identity"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub world_name: String,
    pub place_id: Uuid,
    pub scene_id: String,
}
async fn source_scopes(c: &mut PgConnection) -> Result<Vec<Scope>> {
    let rows=sqlx::query("SELECT world_name,place_id,scene_id FROM migration_player_capture.scope ORDER BY world_name,place_id").fetch_all(c).await?;
    ensure!(
        !rows.is_empty() && rows.len() <= 1000,
        "player capture requires 1 to 1000 scoped scenes"
    );
    let scopes: Vec<Scope> = rows
        .into_iter()
        .map(|r| {
            Ok(Scope {
                world_name: r.try_get("world_name")?,
                place_id: r.try_get("place_id")?,
                scene_id: r.try_get("scene_id")?,
            })
        })
        .collect::<Result<_>>()?;
    for s in &scopes {
        validate_scope(&s.world_name, s.place_id, &s.scene_id)?;
    }
    Ok(scopes)
}
async fn target_scopes(c: &mut PgConnection, stream: &str) -> Result<Vec<Scope>> {
    sqlx::query("SELECT world_name,place_id,scene_id FROM migration_sync.player_scopes WHERE stream=$1 ORDER BY world_name,place_id").bind(stream).fetch_all(c).await?.into_iter().map(|r|Ok(Scope {world_name:r.try_get("world_name")?,place_id:r.try_get("place_id")?,scene_id:r.try_get("scene_id")?})).collect()
}
async fn verify_dependencies(c: &mut PgConnection, scopes: &[Scope]) -> Result<()> {
    for scope in scopes {
        let valid: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM explore.world_scene_place p JOIN worlds.world_scenes s ON s.world_name=p.world_name AND s.entity_id=p.entity_id JOIN worlds.worlds w ON w.name=p.world_name WHERE p.world_name=$1 AND p.identity=$2 AND p.entity_id=$3 AND p.active AND s.mirror_source IS NULL AND w.mirror_source IS NULL)")
            .bind(&scope.world_name).bind(scope.place_id).bind(&scope.scene_id).fetch_one(&mut *c).await?;
        ensure!(valid, "player scene binding changed or is unavailable");
    }
    Ok(())
}

/// Register only after the existing baseline importer has produced exactly the
/// selected source player values. No target values are overwritten by bootstrap.
pub async fn bootstrap(source: &PgPool, target: &PgPool, stream: &str) -> Result<Value> {
    crate::model::validate_stream(stream)?;
    let mut lock = source.begin().await?;
    sqlx::query(SOURCE_LOCK).execute(&mut *lock).await?;
    let mut snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *snapshot)
        .await?;
    let info=sqlx::query("SELECT version,generation,position,sealed FROM migration_player_capture.control WHERE singleton").fetch_one(&mut *snapshot).await?;
    ensure!(
        info.try_get::<i32, _>("version")? == 1,
        "unsupported player source adapter"
    );
    ensure!(
        info.try_get::<bool, _>("sealed")?,
        "player capture scope is not sealed"
    );
    let generation: Uuid = info.try_get("generation")?;
    let position: i64 = info.try_get("position")?;
    let scopes = source_scopes(&mut snapshot).await?;
    lock.commit().await?;
    let mut tx = target.begin().await?;
    sqlx::query(TARGET_LOCK).execute(&mut *tx).await?;
    ensure!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM migration_sync.player_streams)"
        )
        .fetch_one(&mut *tx)
        .await?,
        "player migration already registered"
    );
    verify_dependencies(&mut tx, &scopes).await?;
    for scope in &scopes {
        let source_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM public.player_storage WHERE world_name=$1 AND place_id=$2",
        )
        .bind(&scope.world_name)
        .bind(scope.place_id)
        .fetch_one(&mut *snapshot)
        .await?;
        ensure!(
            source_count <= 100_000,
            "player baseline exceeds rehearsal row bound"
        );
        let source_bytes:i64=sqlx::query_scalar("SELECT COALESCE(sum(pg_column_size(value)::bigint),0)::bigint FROM public.player_storage WHERE world_name=$1 AND place_id=$2").bind(&scope.world_name).bind(scope.place_id).fetch_one(&mut *snapshot).await?;
        ensure!(
            source_bytes <= 64 * 1024 * 1024,
            "player baseline exceeds rehearsal byte bound"
        );
        let rows=sqlx::query("SELECT player_address,key,value,value_size,created_at,updated_at FROM public.player_storage WHERE world_name=$1 AND place_id=$2 ORDER BY player_address,key")
            .bind(&scope.world_name).bind(scope.place_id).fetch_all(&mut *snapshot).await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM worlds.player_storage WHERE realm=$1 AND scene_namespace=$2",
        )
        .bind(&scope.world_name)
        .bind(scope.place_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        ensure!(
            count == rows.len() as i64,
            "player baseline row count differs"
        );
        for row in rows {
            let same:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM worlds.player_storage WHERE realm=$1 AND scene_namespace=$2 AND player_address=$3 AND key=$4 AND value=$5 AND value_size=$6 AND created_at=$7 AND updated_at=$8)")
                .bind(&scope.world_name).bind(scope.place_id.to_string()).bind(row.try_get::<String,_>("player_address")?)
                .bind(row.try_get::<String,_>("key")?).bind(row.try_get::<Value,_>("value")?).bind(row.try_get::<i32,_>("value_size")?)
                .bind(row.try_get::<NaiveDateTime,_>("created_at")?).bind(row.try_get::<NaiveDateTime,_>("updated_at")?).fetch_one(&mut *tx).await?;
            ensure!(same, "player baseline differs from source snapshot");
        }
    }
    sqlx::query("INSERT INTO migration_sync.player_streams(name,generation,baseline,captured,applied) VALUES($1,$2,$3,$3,$3)").bind(stream).bind(generation).bind(position).execute(&mut *tx).await?;
    for scope in &scopes {
        sqlx::query("INSERT INTO migration_sync.player_scopes(stream,world_name,place_id,scene_id) VALUES($1,$2,$3,$4)").bind(stream).bind(&scope.world_name).bind(scope.place_id).bind(&scope.scene_id).execute(&mut *tx).await?;
    }
    snapshot.commit().await?;
    tx.commit().await?;
    Ok(json!({"generation":generation,"baseline":position,"scopes":scopes.len(),"paused":true}))
}

/// Read the next committed source transaction. Source ordinals are transactional
/// and gap-free; a missing next row is a retention/schema error, never skipped.
pub async fn pump_once(source: &PgPool, target: &PgPool, stream: &str) -> Result<bool> {
    let mut tx = target.begin().await?;
    let state = sqlx::query(
        "SELECT generation,captured FROM migration_sync.player_streams WHERE name=$1 FOR UPDATE",
    )
    .bind(stream)
    .fetch_one(&mut *tx)
    .await?;
    let generation: Uuid = state.try_get("generation")?;
    let cursor: i64 = state.try_get("captured")?;
    let mut snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *snapshot)
        .await?;
    let info=sqlx::query("SELECT version,generation,position,sealed FROM migration_player_capture.control WHERE singleton").fetch_one(&mut *snapshot).await?;
    ensure!(
        info.try_get::<i32, _>("version")? == 1
            && info.try_get::<Uuid, _>("generation")? == generation,
        "player source generation changed"
    );
    ensure!(
        info.try_get::<bool, _>("sealed")?,
        "player capture scope is not sealed"
    );
    ensure!(
        source_scopes(&mut snapshot).await? == target_scopes(&mut tx, stream).await?,
        "player source scope changed"
    );
    let head: i64 = info.try_get("position")?;
    ensure!(head >= cursor, "player source cursor moved backwards");
    if head == cursor {
        sqlx::query("UPDATE migration_sync.player_streams SET observed_at=now() WHERE name=$1")
            .bind(stream)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        snapshot.commit().await?;
        return Ok(false);
    }
    let row = sqlx::query(
        "SELECT command,unclassified FROM migration_player_capture.transactions WHERE position=$1",
    )
    .bind(cursor + 1)
    .fetch_one(&mut *snapshot)
    .await?;
    let command: Option<Value> = row.try_get("command")?;
    ensure!(
        command
            .as_ref()
            .is_none_or(|v| v.to_string().len() <= MAX_COMMAND_BYTES),
        "player command exceeds capture size"
    );
    sqlx::query("INSERT INTO migration_sync.player_inbox(stream,generation,position,command,unclassified) VALUES($1,$2,$3,$4,$5)")
        .bind(stream).bind(generation).bind(cursor+1).bind(command).bind(row.try_get::<bool,_>("unclassified")?).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE migration_sync.player_streams SET captured=$2,observed_at=now() WHERE name=$1",
    )
    .bind(stream)
    .bind(cursor + 1)
    .execute(&mut *tx)
    .await?;
    snapshot.commit().await?;
    tx.commit().await?;
    Ok(true)
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum Outcome {
    Idle,
    Applied,
    Blocked,
}
async fn block(
    tx: &mut PgConnection,
    stream: &str,
    generation: Uuid,
    position: i64,
    code: &str,
) -> Result<()> {
    sqlx::query("UPDATE migration_sync.player_inbox SET status='blocked',error_code=$4 WHERE stream=$1 AND generation=$2 AND position=$3").bind(stream).bind(generation).bind(position).bind(code).execute(tx).await?;
    Ok(())
}
pub async fn apply_next(target: &PgPool, stream: &str) -> Result<Outcome> {
    let mut tx = target.begin().await?;
    sqlx::query(TARGET_LOCK).execute(&mut *tx).await?;
    let state=sqlx::query("SELECT generation,applied,paused FROM migration_sync.player_streams WHERE name=$1 FOR UPDATE").bind(stream).fetch_one(&mut *tx).await?;
    if state.try_get::<bool, _>("paused")? {
        return Ok(Outcome::Idle);
    }
    let generation: Uuid = state.try_get("generation")?;
    let position = state.try_get::<i64, _>("applied")? + 1;
    let Some(row)=sqlx::query("SELECT command,unclassified,status FROM migration_sync.player_inbox WHERE stream=$1 AND generation=$2 AND position=$3").bind(stream).bind(generation).bind(position).fetch_optional(&mut *tx).await? else {return Ok(Outcome::Idle);};
    if row.try_get::<String, _>("status")? == "blocked" {
        return Ok(Outcome::Blocked);
    }
    let value: Option<Value> = row.try_get("command")?;
    let command = value
        .clone()
        .and_then(|v| serde_json::from_value::<Command>(v).ok());
    let code = if row.try_get::<bool, _>("unclassified")? {
        Some("unclassified_player_write")
    } else if command.as_ref().is_none_or(|c| c.validate().is_err()) {
        Some("invalid_player_command")
    } else {
        None
    };
    if let Some(code) = code {
        block(&mut tx, stream, generation, position, code).await?;
        tx.commit().await?;
        return Ok(Outcome::Blocked);
    }
    let command = command.ok_or_else(|| anyhow::anyhow!("missing validated player command"))?;
    let scopes = target_scopes(&mut tx, stream).await?;
    let dependencies_match = match verify_dependencies(&mut tx, &scopes).await {
        Ok(()) => true,
        Err(error) if error.chain().any(|cause| cause.is::<sqlx::Error>()) => return Err(error),
        Err(_) => false,
    };
    if !scopes.iter().any(|s| {
        s.world_name == command.world_name
            && s.place_id == command.place_id
            && s.scene_id == command.scene_id
    }) || !dependencies_match
    {
        block(
            &mut tx,
            stream,
            generation,
            position,
            "player_scope_changed",
        )
        .await?;
        tx.commit().await?;
        return Ok(Outcome::Blocked);
    }
    sqlx::query("SAVEPOINT native_player_command")
        .execute(&mut *tx)
        .await?;
    let applied = execute_native(&mut tx, &command).await;
    if let Err(error) = applied {
        // Database availability/locking errors are retryable execution failures,
        // never evidence that the immutable accepted command is incompatible.
        if error.chain().any(|cause| cause.is::<sqlx::Error>()) {
            return Err(error);
        }
        sqlx::query("ROLLBACK TO SAVEPOINT native_player_command")
            .execute(&mut *tx)
            .await?;
        block(
            &mut tx,
            stream,
            generation,
            position,
            "player_native_conflict",
        )
        .await?;
        tx.commit().await?;
        return Ok(Outcome::Blocked);
    }
    sqlx::query("INSERT INTO migration_sync.player_receipts(stream,generation,position,command) VALUES($1,$2,$3,$4)").bind(stream).bind(generation).bind(position).bind(value).execute(&mut *tx).await?;
    sqlx::query("UPDATE migration_sync.player_inbox SET status='applied' WHERE stream=$1 AND generation=$2 AND position=$3").bind(stream).bind(generation).bind(position).execute(&mut *tx).await?;
    sqlx::query("UPDATE migration_sync.player_streams SET applied=$2 WHERE name=$1")
        .bind(stream)
        .bind(position)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Outcome::Applied)
}
async fn execute_native(c: &mut PgConnection, command: &Command) -> Result<()> {
    // Production already accepted quota/auth; don't re-evaluate later external
    // authorization or config. The shared native persistence function enforces
    // storage shape. No live readers/caches run on this rehearsal destination.
    let inserted:bool=sqlx::query_scalar("SELECT inserted FROM worlds.apply_player_storage_command_v1($1,$2,$3,false,$4,$5,$6,$7,2147483647,9223372036854775807,$8,$9)")
        .bind(&command.op).bind(&command.world_name).bind(command.place_id.to_string()).bind(&command.player_address).bind(&command.key).bind(&command.value_text).bind(command.value_size)
        .bind(command.created_at.map(|v|v.naive_utc())).bind(command.updated_at.map(|v|v.naive_utc())).fetch_one(&mut *c).await?;
    ensure!(inserted, "native player operation rejected");
    if command.op == "set" {
        let same:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM worlds.player_storage WHERE realm=$1 AND scene_namespace=$2 AND player_address=$3 AND key=$4 AND value=$5::text::jsonb AND value_size=$6 AND created_at=$7 AND updated_at=$8)")
            .bind(&command.world_name).bind(command.place_id.to_string()).bind(&command.player_address).bind(&command.key).bind(&command.value_text).bind(command.value_size)
            .bind(command.created_at.map(|v|v.naive_utc())).bind(command.updated_at.map(|v|v.naive_utc())).fetch_one(&mut *c).await?;
        ensure!(same, "native player post-state conflict");
    }
    Ok(())
}
pub async fn set_paused(pool: &PgPool, stream: &str, paused: bool, rehearsal: bool) -> Result<()> {
    ensure!(
        paused || rehearsal,
        "player replay requires explicit rehearsal acknowledgement"
    );
    ensure!(
        sqlx::query("UPDATE migration_sync.player_streams SET paused=$2 WHERE name=$1")
            .bind(stream)
            .bind(paused)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
        "unknown player stream"
    );
    Ok(())
}
pub async fn status(pool: &PgPool, stream: &str) -> Result<Value> {
    let row=sqlx::query("SELECT generation,baseline,captured,applied,paused,observed_at,(SELECT count(*) FROM migration_sync.player_inbox i WHERE i.stream=s.name AND i.status='blocked') AS blocked,(SELECT position FROM migration_sync.player_inbox i WHERE i.stream=s.name AND i.status='blocked' ORDER BY position LIMIT 1) AS blocked_position,(SELECT error_code FROM migration_sync.player_inbox i WHERE i.stream=s.name AND i.status='blocked' ORDER BY position LIMIT 1) AS error_code FROM migration_sync.player_streams s WHERE name=$1").bind(stream).fetch_one(pool).await?;
    Ok(
        json!({"generation":row.try_get::<Uuid,_>("generation")?,"baseline":row.try_get::<i64,_>("baseline")?,"captured":row.try_get::<i64,_>("captured")?,"applied":row.try_get::<i64,_>("applied")?,"paused":row.try_get::<bool,_>("paused")?,"observed_at":row.try_get::<DateTime<Utc>,_>("observed_at")?,"blocked":row.try_get::<i64,_>("blocked")?,"blocked_position":row.try_get::<Option<i64>,_>("blocked_position")?,"error_code":row.try_get::<Option<String>,_>("error_code")?}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_command_shape_rejects_ambiguous_scope_and_value_fields() {
        let mut command = Command {
            version: 1,
            op: "set".into(),
            world_name: "example.dcl.eth".into(),
            place_id: Uuid::from_u128(1),
            scene_id: "bafkreiscene".into(),
            player_address: Some(format!("0x{}", "a".repeat(40))),
            key: Some("🎮".repeat(255)),
            value_text: Some("1".into()),
            value_size: Some(1),
            created_at: Some(
                DateTime::parse_from_rfc3339("2026-11-13T00:00:00Z")
                    .unwrap()
                    .to_utc(),
            ),
            updated_at: Some(
                DateTime::parse_from_rfc3339("2026-11-13T00:00:00Z")
                    .unwrap()
                    .to_utc(),
            ),
        };
        command.validate().unwrap();
        command.key = Some("x".repeat(256));
        assert!(command.validate().is_err());
        command.key = Some("key".into());
        command.value_size = Some(2);
        assert!(command.validate().is_err());
        command.value_size = Some(1);
        command.op = "delete".into();
        assert!(command.validate().is_err());
        command.value_text = None;
        command.value_size = None;
        command.created_at = None;
        command.updated_at = None;
        command.validate().unwrap();
        command.op = "clear_scene".into();
        assert!(command.validate().is_err());
        command.player_address = None;
        command.key = None;
        command.validate().unwrap();
        command.world_name = "main".into();
        assert!(command.validate().is_err());
    }
}
