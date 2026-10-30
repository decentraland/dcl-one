//! The data-safe upgrade an existing host takes on activation: a database
//! stamped with the closure base's schema (`c96370f21`, the rendering the
//! first Droplet would have been born with, kept verbatim under
//! `tests/fixtures/schema/`) holding a row, brought up to this binary's
//! layout by `catalyst schema migrate` (`schema_migrate::migrate`, what
//! `catalyst-schema.service` runs) with the row intact, every step of
//! `schemas/migrations/` recorded, `db::bootstrap` accepting it and a second
//! run applying nothing. The base is stamped version 1, so the reviewed
//! release script of version 2 (`schemas/releases/v2-shop-schema.sql`, the
//! Shop schema) runs first, by hand on a host; `migrate` refuses the base
//! before it. Before the steps existed the same call refused the upgrade at
//! `credits.purchases.quantity, credits.purchases.source`.
//! Needs CATALYST_TEST_DATABASE_URL (a database of its own, dropped after).

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use catalyst::schema_migrate::{self, MigrateError};

const CLOSURE_BASE: &str = include_str!("fixtures/schema/closure-base-c96370f21.sql");
const RELEASE_V2: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/schemas/releases/v2-shop-schema.sql"
));

struct Old {
    pool: PgPool,
    admin: PgPool,
    name: String,
}

async fn stamped_with_the_closure_base() -> Option<Old> {
    let url = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
    let base: PgConnectOptions = url
        .parse()
        .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("catalyst_test_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(
            base.database(&name)
                .options([("TimeZone", "UTC"), ("search_path", "content,meta")]),
        )
        .await
        .unwrap();
    sqlx::raw_sql(CLOSURE_BASE)
        .execute(&pool)
        .await
        .expect("the closure base's schema.sql applies");
    Some(Old { pool, admin, name })
}

impl Old {
    async fn drop(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

async fn has_column(pool: &PgPool, table: &str, column: &str) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_attribute WHERE attrelid = to_regclass($1) AND attname = $2 AND NOT attisdropped)")
        .bind(table)
        .bind(column)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_database_stamped_with_the_closure_base_upgrades_to_head_with_its_rows() {
    let Some(old) = stamped_with_the_closure_base().await else {
        return;
    };
    let pool = &old.pool;
    let (identity, version): (String, i32) =
        sqlx::query_as("SELECT identity, version FROM meta.schema WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        (identity.as_str(), version),
        (catalyst::db::SCHEMA_IDENTITY, 1),
        "the closure base is stamped version 1"
    );
    assert!(
        !has_column(pool, "credits.purchases", "quantity").await,
        "the fixture predates the column"
    );
    let ledger: bool = sqlx::query_scalar("SELECT to_regclass('meta.schema_steps') IS NOT NULL")
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(!ledger, "the fixture predates the steps ledger");

    // rows the upgrade must carry: a purchase (the table a step alters) and a world
    sqlx::query("INSERT INTO credits.purchases (address, sku, credits, amount_cents, currency, status) VALUES ('0x' || repeat('a', 40), 'credits_usd_5', 50, 500, 'usd', 'paid')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO worlds.worlds (name, owner) VALUES ('upgrade.dcl.eth', '0x' || repeat('b', 40))").execute(pool).await.unwrap();

    // version 1 is another version: migrate refuses it, and the reviewed
    // release script (schemas/releases/v2-shop-schema.sql: the Shop schema
    // rename, the four ledgers, the stamp) is the hand-run step that makes
    // this migrate's case; it refuses to run twice
    match schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
    {
        Err(MigrateError::Incompatible { version, .. }) => assert_eq!(version, 1),
        other => panic!("{other:?}"),
    }
    sqlx::raw_sql(RELEASE_V2)
        .execute(pool)
        .await
        .expect("the v2 release script applies to a version-1 database");
    let mut again = pool.acquire().await.unwrap();
    assert!(
        sqlx::raw_sql(RELEASE_V2)
            .execute(&mut *again)
            .await
            .is_err(),
        "the version guard refuses a second run"
    );
    sqlx::raw_sql("ROLLBACK")
        .execute(&mut *again)
        .await
        .unwrap();
    drop(again);
    let (version,): (i32,) = sqlx::query_as("SELECT version FROM meta.schema WHERE id = 1")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(version, catalyst::db::SCHEMA_VERSION);
    let (shop, marketplace, ledgers): (bool, bool, i64) = sqlx::query_as(
        "SELECT to_regclass('shop.trades') IS NOT NULL, to_regnamespace('marketplace') IS NOT NULL, \
                (SELECT count(*) FROM pg_tables WHERE schemaname = 'shop' AND tablename IN ('broker_purchases', 'escrow_actions', 'name_transfers', 'usage_grants'))",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        (shop, marketplace, ledgers),
        (true, false, 4),
        "the rename and the four moves"
    );

    let before = schema_migrate::gaps(pool, catalyst::db::SCHEMA_SQL)
        .await
        .unwrap();
    assert!(
        before
            .columns
            .contains(&"credits.purchases.quantity".to_string())
            && before
                .columns
                .contains(&"credits.purchases.source".to_string()),
        "{:?}",
        before.columns
    );
    assert!(
        before.fatal().len() > 20,
        "the base lacks the tables and functions added since: {}",
        before.fatal().len()
    );
    let every_step: Vec<String> = schema_migrate::parse_steps(catalyst::db::MIGRATIONS_SQL)
        .into_iter()
        .map(|step| step.name)
        .collect();
    assert!(every_step.len() >= 4, "{every_step:?}");
    let status =
        schema_migrate::status(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert_eq!(
        status.exit_code(),
        3,
        "migrate has work and nothing stands in its way: {:?}",
        status.lines()
    );
    assert_eq!(status.pending_steps, every_step);
    assert!(
        matches!(
            catalyst::db::bootstrap(pool).await,
            Err(catalyst::db::BootstrapError::Layout(_))
        ),
        "serve refuses the old layout"
    );

    let done =
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(done.steps, every_step);
    assert!(done.created.len() > 20, "{}", done.created.len());

    let (quantity, source, credits): (i32, Option<String>, String) =
        sqlx::query_as("SELECT quantity, source, credits::text FROM credits.purchases")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        (quantity, source, credits.as_str()),
        (1, None, "50"),
        "the row survived and took the backfilled default"
    );
    let worlds: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.worlds WHERE name = 'upgrade.dcl.eth'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(worlds, 1);
    let constraints: Vec<String> =
        sqlx::query_scalar("SELECT conname FROM pg_constraint WHERE conrelid = to_regclass('credits.purchases') AND conname IN ('purchases_quantity_check', 'purchases_source_check') ORDER BY 1")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(
        constraints,
        vec!["purchases_quantity_check", "purchases_source_check"]
    );
    let recorded: Vec<(String, bool)> =
        sqlx::query_as("SELECT step, created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(
        recorded,
        every_step
            .iter()
            .map(|step| (step.clone(), false))
            .collect::<Vec<_>>()
    );

    assert!(
        schema_migrate::gaps(pool, catalyst::db::SCHEMA_SQL)
            .await
            .unwrap()
            .fatal()
            .is_empty()
    );
    catalyst::db::bootstrap(pool)
        .await
        .expect("serve accepts the upgraded database");
    assert!(
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap()
            .is_empty(),
        "a second run applies nothing"
    );
    let status =
        schema_migrate::status(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert_eq!(status.exit_code(), 0, "{:?}", status.lines());

    // a change no step covers (a column dropped by hand) is refused, with nothing applied
    sqlx::query("ALTER TABLE worlds.worlds DROP COLUMN title")
        .execute(pool)
        .await
        .unwrap();
    match schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
    {
        Err(MigrateError::Drift(cols)) => assert_eq!(cols, "worlds.worlds.title"),
        other => panic!("{other:?}"),
    }
    let status =
        schema_migrate::status(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert_eq!(status.exit_code(), 4, "{:?}", status.lines());

    // another version is never migrate's to handle
    sqlx::query("UPDATE meta.schema SET version = version + 1 WHERE id = 1")
        .execute(pool)
        .await
        .unwrap();
    match schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
    {
        Err(MigrateError::Incompatible { version, .. }) => {
            assert_eq!(version, catalyst::db::SCHEMA_VERSION + 1)
        }
        other => panic!("{other:?}"),
    }
    old.drop().await;
}

/// `0007_mirror_origin`: the one origin column, backfilled from the markers
/// it replaces. A place derived from this node's scenes (raw.local) and an
/// event created here (its overlay written in the creating transaction,
/// whether the payload still says so or a moderation replaced it) become
/// 'local'; an overlay that edited a mirrored event leaves it 'upstream', so
/// the next pass writes the upstream's fields back (released, not frozen),
/// while a local event keeps its own. An operator's moderation or delete of a
/// mirrored event is carried into `event_moderation` before that pass, so it
/// survives the release. A community with no member row is a mirrored one.
#[tokio::test]
async fn the_origin_backfill_keeps_local_rows_and_releases_edited_mirrored_events() {
    let Some(old) = stamped_with_the_closure_base().await else {
        return;
    };
    let pool = &old.pool;
    let owner = format!("0x{}", "c".repeat(40));
    sqlx::raw_sql(
        "INSERT INTO explore.place (id, base_position, raw) VALUES \
             ('derived', '1,1', '{\"local\": true}'), ('mirrored', '2,2', '{}');
         INSERT INTO explore.event (id, name, raw) VALUES ('created', 'created', '{}');
         INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ('created', '0xc', '{\"create\": {}}');",
    )
    .execute(pool)
    .await
    .unwrap();
    // created, then moderated: the moderation replaced the payload, never signed_at
    sqlx::raw_sql(
        "INSERT INTO explore.event (id, name, raw) VALUES ('moderated', 'moderated', '{}');
         INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ('moderated', '0xc', '{\"create\": {}}');",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE explore.events_local SET signed_payload = '{\"moderation\": {}}', updated_at = updated_at + interval '1 hour' WHERE id = 'moderated'")
        .execute(pool)
        .await
        .unwrap();
    // a mirrored event an owner edited here: its overlay came after the pass
    sqlx::raw_sql(
        "INSERT INTO explore.event (id, name, raw, fetched_at) VALUES \
             ('edited', 'edited here', '{}', timezone('UTC', now()) - interval '1 hour'), \
             ('plain', 'plain', '{}', timezone('UTC', now()) - interval '1 hour');
         INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ('edited', '0xc', '{\"edit\": {}}');",
    )
    .execute(pool)
    .await
    .unwrap();
    // mirrored events the operator rejected and featured, and deleted, here
    sqlx::raw_sql(
        "INSERT INTO explore.event (id, name, approved, highlighted, raw, fetched_at) VALUES \
             ('rejected-here', 'rejected-here', false, true, '{\"approved\": false, \"rejected\": true, \"rejected_by\": \"0xd\", \"rejection_reason\": \"spam\", \"highlighted\": true}', \
              timezone('UTC', now()) - interval '1 hour'), \
             ('deleted-here', 'deleted-here', true, false, '{\"approved\": true, \"deleted_by_admin\": true, \"deleted_by\": \"0xd\", \"deleted_reason\": \"duplicate\"}', \
              timezone('UTC', now()) - interval '1 hour');
         INSERT INTO explore.events_local (id, signer, signed_payload) VALUES \
             ('rejected-here', '0xd', '{\"moderation\": {\"action\": \"reject\", \"highlighted\": true}}'), \
             ('deleted-here', '0xd', '{\"delete\": {}}');",
    )
    .execute(pool)
    .await
    .unwrap();
    // a mirrored event the operator featured, its payload as the base binary
    // serialized it (every untouched field null); the upstream rejects it later
    sqlx::query(
        "INSERT INTO explore.event (id, name, approved, highlighted, raw, fetched_at) VALUES \
             ('featured-here', 'featured-here', true, true, '{\"approved\": true, \"highlighted\": true}', \
              timezone('UTC', now()) - interval '1 hour')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ('featured-here', '0xd', $1)")
        .bind(serde_json::json!({ "moderation": {
            "action": "feature", "approved": null, "rejected": null, "highlighted": null,
            "trending": null, "name": null, "description": null, "reason": null } }))
        .execute(pool)
        .await
        .unwrap();
    let local_community = uuid::Uuid::new_v4();
    let mirrored_community = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO social.communities (id, name, owner_address) VALUES ($1, 'here', $3), ($2, 'there', $3)",
    )
    .bind(local_community)
    .bind(mirrored_community)
    .bind(&owner)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'owner')",
    )
    .bind(local_community)
    .bind(&owner)
    .execute(pool)
    .await
    .unwrap();

    sqlx::raw_sql(RELEASE_V2).execute(pool).await.unwrap();
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap_or_else(|e| panic!("{e}"));

    let places: Vec<(String, String)> =
        sqlx::query_as("SELECT id, origin FROM explore.place ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(
        places,
        [
            ("derived".into(), "local".into()),
            ("mirrored".into(), "upstream".into())
        ]
    );
    let events: Vec<(String, String)> =
        sqlx::query_as("SELECT id, origin FROM explore.event ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(
        events,
        [
            ("created".into(), "local".into()),
            ("deleted-here".into(), "upstream".into()),
            ("edited".into(), "upstream".into()),
            ("featured-here".into(), "upstream".into()),
            ("moderated".into(), "local".into()),
            ("plain".into(), "upstream".into()),
            ("rejected-here".into(), "upstream".into()),
        ]
    );
    let communities: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, origin FROM social.communities WHERE id = ANY($1)")
            .bind(vec![local_community, mirrored_community])
            .fetch_all(pool)
            .await
            .unwrap();
    let origin_of = |id| {
        communities
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, o)| o.as_str())
    };
    assert_eq!(
        (origin_of(local_community), origin_of(mirrored_community)),
        (Some("local"), Some("upstream"))
    );

    use catalyst::explore::events::mirror::upsert_event;
    for id in ["edited", "created"] {
        let upstream = serde_json::json!({ "id": id, "name": "from upstream" });
        assert!(upsert_event(pool, &upstream).await.unwrap());
    }
    let names: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM explore.event WHERE id IN ('edited', 'created') ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        names,
        [
            ("created".into(), "created".into()),
            ("edited".into(), "from upstream".into()),
        ],
        "the released event takes the upstream's fields, the local one keeps its own"
    );
    for id in ["rejected-here", "deleted-here"] {
        let upstream = serde_json::json!({ "id": id, "name": "from upstream", "approved": true });
        assert!(upsert_event(pool, &upstream).await.unwrap());
    }
    let upstream = serde_json::json!({ "id": "featured-here", "name": "from upstream",
                                       "approved": false, "rejected": true, "highlighted": false });
    assert!(upsert_event(pool, &upstream).await.unwrap());
    type Moderated = (String, String, bool, bool, Option<bool>, Option<bool>);
    let moderated: Vec<Moderated> = sqlx::query_as(
        "SELECT id, name, approved, highlighted, (raw->>'rejected')::boolean, \
                (raw->>'deleted_by_admin')::boolean \
         FROM explore.event WHERE id IN ('rejected-here', 'deleted-here', 'featured-here') ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        moderated,
        [
            (
                "deleted-here".into(),
                "from upstream".into(),
                true,
                false,
                None,
                Some(true)
            ),
            (
                "featured-here".into(),
                "from upstream".into(),
                false,
                true,
                Some(true),
                None
            ),
            (
                "rejected-here".into(),
                "from upstream".into(),
                false,
                true,
                Some(true),
                None
            ),
        ],
        "the upstream's fields land and the operator's decisions stay; a flag the \
         operator never touched (featured-here's approval) is the upstream's"
    );
    let reasons: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, raw->>'rejection_reason', raw->>'deleted_reason' FROM explore.event \
         WHERE id IN ('rejected-here', 'deleted-here') ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        reasons,
        [
            ("deleted-here".into(), None, Some("duplicate".into())),
            ("rejected-here".into(), Some("spam".into()), None),
        ],
        "the operator's reasons survive the pass"
    );
    type Overlay = (
        String,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        String,
    );
    let overlays: Vec<Overlay> =
        sqlx::query_as(
            "SELECT event_id, approved, rejected, highlighted, hidden, set_by FROM explore.event_moderation ORDER BY event_id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(
        overlays,
        [
            (
                "deleted-here".into(),
                None,
                None,
                None,
                Some(true),
                "0xd".into()
            ),
            (
                "featured-here".into(),
                None,
                None,
                Some(true),
                None,
                "0xd".into()
            ),
            (
                "rejected-here".into(),
                Some(false),
                Some(true),
                Some(true),
                None,
                "0xd".into()
            ),
        ]
    );
    old.drop().await;
}

#[tokio::test]
async fn stamped_database_additive_pass_installs_livekit_intents_and_preserves_bans() {
    let Some(old) = stamped_with_the_closure_base().await else {
        return;
    };
    let pool = &old.pool;
    sqlx::raw_sql(RELEASE_V2).execute(pool).await.unwrap();
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap();
    // Model the exact preceding deployed layout: current stamp/step ledger and
    // moderation rows exist, but the newly added relation and its indexes do not.
    sqlx::query("DROP TABLE social.livekit_removal_intents")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO social.scene_bans(place_id,banned_address,banned_by) VALUES ('upgrade-room','upgrade-wallet','upgrade-owner')").execute(pool).await.unwrap();
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap();
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM social.scene_bans WHERE place_id='upgrade-room'"
        )
        .fetch_one(pool)
        .await
        .unwrap(),
        1
    );
    let indexes:i64=sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE schemaname='social' AND tablename='livekit_removal_intents'").fetch_one(pool).await.unwrap();
    assert_eq!(
        indexes, 3,
        "primary, due, and captured-room indexes survive idempotent additive migration"
    );
    sqlx::query("INSERT INTO social.livekit_removal_intents(root_key,generation,cause_kind,wallet,actor) VALUES ('upgrade-root',gen_random_uuid(),'block','upgrade-wallet','upgrade-wallet')").execute(pool).await.unwrap();
    let claimed:i64=sqlx::query_scalar("WITH due AS (SELECT root_key,target_key FROM social.livekit_removal_intents WHERE next_attempt_at<=timezone('UTC',now()) AND claim_until IS NULL ORDER BY next_attempt_at,created_at,root_key,target_key FOR UPDATE SKIP LOCKED LIMIT 16), claimed AS (UPDATE social.livekit_removal_intents t SET claim_id=gen_random_uuid(),claim_until=timezone('UTC',now())+interval '60 seconds' FROM due WHERE t.root_key=due.root_key AND t.target_key=due.target_key RETURNING t.root_key) SELECT count(*) FROM claimed").fetch_one(pool).await.unwrap();
    assert_eq!(claimed, 1);
    old.drop().await;
}

/// `0009_world_scene_ban_keys` on upgrade: bans under a deployed world
/// scene's entity id move to `<world>:<base>`, Genesis rows stay; re-running
/// the step when the key row already exists neither fails nor duplicates.
#[tokio::test]
async fn world_scene_bans_are_rekeyed_on_upgrade_and_the_step_reruns_cleanly() {
    let Some(old) = stamped_with_the_closure_base().await else {
        return;
    };
    let pool = &old.pool;
    let (first, second) = (
        format!("0x{}", "1".repeat(40)),
        format!("0x{}", "2".repeat(40)),
    );
    sqlx::raw_sql(
        "INSERT INTO worlds.worlds (name) VALUES ('bans.dcl.eth');
         INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES
           ('bans.dcl.eth', 'bafkreideclared', '[]'::json, '{\"metadata\": {\"scene\": {\"base\": \"4,5\"}}}', '0x0', ARRAY['3,3', '4,5'], 1),
           ('bans.dcl.eth', 'bafkreilowest', '[]'::json, '{}', '0x0', ARRAY['7,1', '-2,9'], 1);",
    )
    .execute(pool)
    .await
    .unwrap();
    for (place, address) in [
        ("bafkreideclared", &first),
        ("bafkreideclared", &second),
        ("bafkreilowest", &first),
        ("bafkreigenesis", &first),
    ] {
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by) VALUES ($1, $2, $2)")
            .bind(place)
            .bind(address)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::raw_sql(RELEASE_V2).execute(pool).await.unwrap();
    let done =
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        done.steps
            .iter()
            .any(|step| step == "0009_world_scene_ban_keys"),
        "{:?}",
        done.steps
    );
    let bans = || async {
        sqlx::query_as::<_, (String, String)>(
            "SELECT place_id, banned_address FROM social.scene_bans ORDER BY place_id COLLATE \"C\", banned_address",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    };
    let rekeyed = vec![
        ("bafkreigenesis".to_owned(), first.clone()),
        ("bans.dcl.eth:-2,9".to_owned(), first.clone()),
        ("bans.dcl.eth:4,5".to_owned(), first.clone()),
        ("bans.dcl.eth:4,5".to_owned(), second.clone()),
    ];
    assert_eq!(
        bans().await,
        rekeyed,
        "declared base, else the lowest parcel"
    );

    // A ban written under the entity id after the step, its key row present.
    sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by) VALUES ('bafkreideclared', $1, $1)")
        .bind(&first)
        .execute(pool)
        .await
        .unwrap();
    let step = schema_migrate::parse_steps(catalyst::db::MIGRATIONS_SQL)
        .into_iter()
        .find(|s| s.name == "0009_world_scene_ban_keys")
        .expect("the step is embedded");
    for _ in 0..2 {
        sqlx::raw_sql(sqlx::AssertSqlSafe(step.sql.clone()))
            .execute(pool)
            .await
            .expect("the step re-runs over an existing key row");
    }
    let mut expected = rekeyed;
    expected.insert(0, ("bafkreideclared".to_owned(), first.clone()));
    assert_eq!(
        bans().await,
        expected,
        "no duplicate, the key row untouched"
    );
    old.drop().await;
}
