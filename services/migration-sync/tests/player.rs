//! Real source SQL -> captured command -> shared native target mutation.
use anyhow::{Result, ensure};
use dcl_migration_sync::player::{self, Outcome, Scope};
use serde_json::Value;
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

const WORLD: &str = "replay.dcl.eth";
const SCENE: &str = "bafkreiplayercommandfixture";
const PLACE: &str = "a1111111-1111-4111-8111-111111111111";
const PLAYER: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

async fn command(
    pool: &PgPool,
    op: &str,
    player: Option<&str>,
    key: Option<&str>,
    value: Option<&str>,
) -> Result<()> {
    sqlx::query("SELECT migration_player_capture.apply_command($1,$2,$3,$4,$5,$6,$7)")
        .bind(op)
        .bind(WORLD)
        .bind(Uuid::parse_str(PLACE)?)
        .bind(SCENE)
        .bind(player)
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}
async fn count(pool: &PgPool) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM worlds.player_storage")
            .fetch_one(pool)
            .await?,
    )
}
async fn step(source: &PgPool, target: &PgPool) -> Result<()> {
    assert!(player::pump_once(source, target, "players").await?);
    assert_eq!(
        player::apply_next(target, "players").await?,
        Outcome::Applied
    );
    Ok(())
}
async fn assertions(source: &PgPool, target: &PgPool, drift: bool) -> Result<()> {
    source.execute(sqlx::raw_sql("CREATE TABLE public.player_storage(world_name varchar(255) NOT NULL,place_id uuid NOT NULL,player_address varchar(42) NOT NULL,key varchar(255) NOT NULL,value jsonb NOT NULL,value_size integer NOT NULL,created_at timestamp NOT NULL,updated_at timestamp NOT NULL,CONSTRAINT player_storage_pkey PRIMARY KEY(world_name,place_id,player_address,key));")).await?;
    let root = Path::new(env!("DCL_ONE_ROOT"));
    let mut files =
        std::fs::read_dir(root.join("schemas"))?.collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|f| f.file_name());
    for f in files {
        if f.path().extension().is_some_and(|e| e == "sql") {
            target
                .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(
                    f.path(),
                )?)))
                .await?;
        }
    }
    sqlx::query("INSERT INTO worlds.worlds(name) VALUES($1)")
        .bind(WORLD)
        .execute(target)
        .await?;
    sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES($1,$2,'[]','{}',$3,ARRAY['0,0'],0)").bind(WORLD).bind(SCENE).bind(PLAYER).execute(target).await?;
    sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES($1,$2,$3,ARRAY['0,0'])").bind(WORLD).bind(Uuid::parse_str(PLACE)?).bind(SCENE).execute(target).await?;
    let scopes = [Scope {
        world_name: WORLD.into(),
        place_id: Uuid::parse_str(PLACE)?,
        scene_id: SCENE.into(),
    }];
    player::install(source, &scopes).await?;
    player::install(source, &scopes).await?;
    assert!(
        sqlx::query("DELETE FROM migration_player_capture.scope")
            .execute(source)
            .await
            .is_err()
    );
    // The target cannot silently overwrite a different preimported baseline.
    sqlx::query("INSERT INTO worlds.player_storage(realm,scene_namespace,player_address,key,value,value_size) VALUES($1,$2,$3,'extra','1',1)").bind(WORLD).bind(PLACE).bind(PLAYER).execute(target).await?;
    assert!(player::bootstrap(source, target, "players").await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM migration_sync.player_streams")
            .fetch_one(target)
            .await?,
        0
    );
    sqlx::query("DELETE FROM worlds.player_storage")
        .execute(target)
        .await?;
    // A normal serving process fences registration even for an empty baseline.
    let mut guard = target.acquire().await?.detach();
    sqlx::query("SELECT pg_advisory_lock_shared(hashtextextended('dcl-one/migration-follower',0))")
        .execute(&mut guard)
        .await?;
    let held = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        player::bootstrap(source, target, "players"),
    )
    .await;
    assert!(held.is_err());
    guard.close().await?;
    sqlx::query("UPDATE worlds.worlds SET mirror_source='https://example.invalid'")
        .execute(target)
        .await?;
    assert!(player::bootstrap(source, target, "players").await.is_err());
    sqlx::query("UPDATE worlds.worlds SET mirror_source=NULL")
        .execute(target)
        .await?;
    player::bootstrap(source, target, "players").await?;
    assert!(
        player::set_paused(target, "players", false, false)
            .await
            .is_err()
    );
    player::set_paused(target, "players", false, true).await?;
    assert_eq!(player::apply_next(target, "players").await?, Outcome::Idle);

    if drift {
        command(source, "set", Some(PLAYER), Some("old-scene"), Some("1")).await?;
        assert!(player::pump_once(source, target, "players").await?);
        sqlx::query("UPDATE explore.world_scene_place SET entity_id='replacement'")
            .execute(target)
            .await?;
        assert_eq!(
            player::apply_next(target, "players").await?,
            Outcome::Blocked
        );
        assert_eq!(count(target).await?, 0);
        return Ok(());
    }
    // Commands roll back with source data, including their allocated cursor.
    let mut rolled = source.begin().await?;
    sqlx::query("SELECT migration_player_capture.apply_command('set',$1,$2,$3,$4,'rolled','1')")
        .bind(WORLD)
        .bind(Uuid::parse_str(PLACE)?)
        .bind(SCENE)
        .bind(PLAYER)
        .execute(&mut *rolled)
        .await?;
    assert!(!player::pump_once(source, target, "players").await?);
    rolled.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT position FROM migration_player_capture.control")
            .fetch_one(source)
            .await?,
        0
    );
    command(
        source,
        "set",
        Some(PLAYER),
        Some("score"),
        Some("{\"z\": 3, \"a\":\"é\"}"),
    )
    .await?;
    assert!(player::pump_once(source, target, "players").await?);
    assert_eq!(count(target).await?, 0);
    assert_eq!(
        player::apply_next(target, "players").await?,
        Outcome::Applied
    );
    assert_eq!(count(target).await?, 1);
    // Lost success response / retry never reexecutes an already receipted head.
    assert_eq!(player::apply_next(target, "players").await?, Outcome::Idle);
    assert!(!player::pump_once(source, target, "players").await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM migration_sync.player_receipts")
            .fetch_one(target)
            .await?,
        1
    );
    let accepted: Value = sqlx::query_scalar(
        "SELECT command FROM migration_player_capture.transactions WHERE position=1",
    )
    .fetch_one(source)
    .await?;
    let size: i32 = sqlx::query_scalar("SELECT value_size FROM worlds.player_storage")
        .fetch_one(target)
        .await?;
    assert_eq!(size as i64, accepted["value_size"].as_i64().unwrap());
    command(source, "set", Some(PLAYER), Some("score"), Some("4")).await?;
    step(source, target).await?;
    command(source, "set", Some(OTHER), Some("score"), Some("5")).await?;
    step(source, target).await?;
    command(source, "clear_player", Some(PLAYER), None, None).await?;
    step(source, target).await?;
    assert_eq!(count(target).await?, 1);
    command(source, "delete", Some(PLAYER), Some("absent"), None).await?;
    step(source, target).await?;
    command(source, "clear_scene", None, None, None).await?;
    step(source, target).await?;
    assert_eq!(count(target).await?, 0);
    command(source, "clear_scene", None, None, None).await?;
    step(source, target).await?;
    // Pausing retains capture, but performs no target mutations.
    player::set_paused(target, "players", true, false).await?;
    command(source, "set", Some(PLAYER), Some("paused"), Some("6")).await?;
    assert!(player::pump_once(source, target, "players").await?);
    assert_eq!(player::apply_next(target, "players").await?, Outcome::Idle);
    player::set_paused(target, "players", false, true).await?;
    assert_eq!(
        player::apply_next(target, "players").await?,
        Outcome::Applied
    );
    // Contention is retryable and leaves no permanent conflict or receipt.
    command(source, "set", Some(PLAYER), Some("paused"), Some("6")).await?;
    assert!(player::pump_once(source, target, "players").await?);
    let mut contender = target.acquire().await?.detach();
    sqlx::query("SELECT pg_advisory_lock(hashtextextended($1,0))")
        .bind(format!("player-storage:{WORLD}:{PLAYER}"))
        .execute(&mut contender)
        .await?;
    assert!(player::apply_next(target, "players").await.is_err());
    assert_eq!(player::status(target, "players").await?["blocked"], 0);
    contender.close().await?;
    assert_eq!(
        player::apply_next(target, "players").await?,
        Outcome::Applied
    );
    // Unmarked SQL cannot become a convenient row-copy fallback.
    sqlx::query("UPDATE public.player_storage SET value='7' WHERE key='paused'")
        .execute(source)
        .await?;
    assert!(player::pump_once(source, target, "players").await?);
    assert_eq!(
        player::apply_next(target, "players").await?,
        Outcome::Blocked
    );
    assert_eq!(
        player::apply_next(target, "players").await?,
        Outcome::Blocked
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value::text FROM worlds.player_storage WHERE key='paused'"
        )
        .fetch_one(target)
        .await?,
        "6"
    );
    let status = player::status(target, "players").await?;
    assert_eq!(status["blocked"], 1);
    // Shared native function maintains public quota and key validation semantics.
    let native=sqlx::query_scalar::<_,bool>("SELECT inserted FROM worlds.apply_player_storage_command_v1('set',$1,$2,false,$3,'large','1234',4,3,100,now()::timestamp,now()::timestamp)").bind(WORLD).bind(PLACE).bind(PLAYER).fetch_one(target).await?;
    assert!(!native);
    assert!(sqlx::query("SELECT * FROM worlds.apply_player_storage_command_v1(NULL,$1,$2,false,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL)").bind(WORLD).bind(PLACE).execute(target).await.is_err());
    assert!(sqlx::query("SELECT * FROM worlds.apply_player_storage_command_v1('delete',$1,$2,false,$3,$4,NULL,NULL,NULL,NULL,NULL,NULL)").bind(WORLD).bind(PLACE).bind(PLAYER).bind("x".repeat(256)).execute(target).await.is_err());
    // The additive definition is equivalent and preserves existing rows.
    target
        .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(
            root.join("schemas/migrations/0040_player_storage_command.sql"),
        )?)))
        .await?;
    assert_eq!(count(target).await?, 1);
    Ok(())
}

#[tokio::test]
async fn player_acceptance_replays_atomically_and_blocks_unknown_writers() -> Result<()> {
    let Ok(url) = std::env::var("MIGRATION_SYNC_TEST_DATABASE_URL") else {
        ensure!(
            std::env::var_os("CI").is_none(),
            "CI requires MIGRATION_SYNC_TEST_DATABASE_URL"
        );
        eprintln!("player PostgreSQL test skipped: MIGRATION_SYNC_TEST_DATABASE_URL not set");
        return Ok(());
    };
    run_fixture(&url, false).await?;
    run_fixture(&url, true).await
}
async fn run_fixture(url: &str, drift: bool) -> Result<()> {
    let options = PgConnectOptions::from_str(url)?;
    let mut admin = PgConnection::connect_with(&options).await?;
    let names = [
        format!("player_source_{}", Uuid::new_v4().simple()),
        format!("player_target_{}", Uuid::new_v4().simple()),
    ];
    for name in &names {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {name}"
            ))))
            .await?;
    }
    let source = PgPool::connect_with(options.clone().database(&names[0])).await?;
    let target = sqlx::postgres::PgPoolOptions::new()
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET lock_timeout='200ms'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options.database(&names[1]))
        .await?;
    let result = assertions(&source, &target, drift).await;
    source.close().await;
    target.close().await;
    for name in &names {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {name} WITH(FORCE)"
            ))))
            .await?;
    }
    result
}
