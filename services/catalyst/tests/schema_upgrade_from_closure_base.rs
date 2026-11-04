//! The reviewed upgrade from closure version1 through genuinely prepared
//! version2 to version3. The pinned v2 binary's real release/migrator produced
//! the captured populated layouts and ledger; current tests never fake that
//! preparation or change a stamp to run another version's migrator. They exercise
//! the exact composed v3 release and current real executor, data preservation,
//! refusal without changes, connection-loss rollback, retry and no-op replay.
//! Needs CATALYST_TEST_DATABASE_URL; every database here is owned and dropped.

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use catalyst::schema_migrate::{self, MigrateError};

/// The steps a prepared version-2 database has not recorded.
const PREPARED_PENDING_STEPS: [&str; 4] = [
    "0018_event_hook_compatibility",
    "0019_event_hook_admin_priority",
    "0023_social_referral_pending_reward",
    "0024_shop_items_search_text",
];
type LedgerRow = (String, String, bool, chrono::NaiveDateTime);

async fn step_ledger(pool: &PgPool) -> Vec<LedgerRow> {
    sqlx::query_as(
        "SELECT step,sha256,created_fresh,applied_at FROM meta.schema_steps ORDER BY step",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

fn assert_only_pending_steps_added(before: &[LedgerRow], after: &[LedgerRow]) {
    assert_eq!(after.len(), before.len() + PREPARED_PENDING_STEPS.len());
    for row in before {
        assert!(
            after.contains(row),
            "every historical hash, origin and timestamp survives: {row:?}"
        );
    }
    let added: Vec<_> = after.iter().filter(|row| !before.contains(row)).collect();
    assert_eq!(
        added.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
        PREPARED_PENDING_STEPS
    );
    let steps = schema_migrate::parse_steps(catalyst::db::MIGRATIONS_SQL);
    for row in added {
        let embedded = steps.iter().find(|step| step.name == row.0).unwrap();
        assert_eq!(row.1, embedded.sha256);
        assert!(
            !row.2,
            "the actual executor records each new step as migrated"
        );
    }
}

const CLOSURE_BASE: &str = include_str!("fixtures/schema/closure-base-c96370f21.sql");
const RELEASE_V3: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/schemas/releases/v3-private-voice-pending.sql"
));
const VOICE_FACTS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/schemas/releases/v3/41_social_voice_notifications.sql"
));
const MAIN_V2: &str = include_str!("fixtures/schema/v2-main-0fb63aa3f.sql");
const PREPARED_V2: &str = include_str!("fixtures/schema/v2-prepared-c1fab3991.sql");
const UPGRADE_ROWS: &str = include_str!("fixtures/schema/v2-upgrade-rows-c1fab3991.sql");
const ORIGIN_ROWS: &str = include_str!("fixtures/schema/v2-origin-rows-c1fab3991.sql");
const PRIVATE_INDEXES: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/schemas/releases/v3/42_social_private_voice_indexes.sql"
));
fn release_v3() -> String {
    // SQLx executes the SQL transaction; psql owns this client-side setting.
    let stop_on_error = "\\set ON_ERROR_STOP on\n";
    assert_eq!(RELEASE_V3.matches(stop_on_error).count(), 1);
    let mut release = RELEASE_V3.replace(stop_on_error, "");
    for (directive, canonical) in [
        ("\\ir v3/41_social_voice_notifications.sql", VOICE_FACTS),
        (
            "\\ir v3/42_social_private_voice_indexes.sql",
            PRIVATE_INDEXES,
        ),
    ] {
        assert_eq!(release.matches(directive).count(), 1);
        release = release.replace(directive, canonical);
    }
    release
}
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
    with_schema(CLOSURE_BASE).await
}
async fn prepared_v2(rows: Option<&str>) -> Option<Old> {
    let old = with_schema(PREPARED_V2).await?;
    if let Some(rows) = rows {
        sqlx::raw_sql(sqlx::AssertSqlSafe(rows.to_owned()))
            .execute(&old.pool)
            .await
            .unwrap();
    }
    Some(old)
}
async fn with_schema(schema: &str) -> Option<Old> {
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
    sqlx::raw_sql(sqlx::AssertSqlSafe(schema.to_owned()))
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
    assert!(!has_column(pool, "explore.place_reports", "upload_token_hash").await);
    assert!(!has_column(pool, "social.notifications", "in_app").await);
    assert!(!has_column(pool, "social.event_notification_intents", "id").await);
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

    // A settings table from the later self-writable era, before permission provenance.
    sqlx::raw_sql(
        r#"CREATE TABLE explore.event_profile_settings (
        "user" text PRIMARY KEY, email text,
        email_verified boolean NOT NULL DEFAULT false,
        use_local_time boolean NOT NULL DEFAULT true,
        notify_by_email boolean NOT NULL DEFAULT false,
        notify_by_browser boolean NOT NULL DEFAULT false,
        permissions jsonb NOT NULL DEFAULT '[]'::jsonb,
        created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
        updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
        CONSTRAINT event_profile_settings_user_lowercase CHECK ("user" = lower("user"))
    )"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO explore.event_profile_settings (\"user\",permissions) VALUES ('0xlegacy','[\"edit_any_event\"]')")
        .execute(pool).await.unwrap();

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
    assert!(every_step.len() >= 4);
    let status =
        schema_migrate::status(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert_eq!(status.pending_steps, every_step);
    assert!(matches!(
        catalyst::db::bootstrap(pool).await,
        Err(catalyst::db::BootstrapError::Mismatch { version: 2, .. })
    ));
    let refused = release_result(pool, &release_v3()).await.unwrap_err();
    assert!(refused.to_string().contains("prepared v2"), "{refused}");
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT version FROM meta.schema WHERE id=1")
            .fetch_one(pool)
            .await
            .unwrap(),
        2
    );
    assert!(
        !has_column(pool, "credits.purchases", "quantity").await,
        "refused release leaves unprepared data/layout intact"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT credits::text FROM credits.purchases")
            .fetch_one(pool)
            .await
            .unwrap(),
        "50"
    );
    old.drop().await;

    // The real pinned v2 executor prepared these captured rows and ledger.
    // A current v3 binary cannot execute that v2 preparation under a false stamp.
    let Some(old) = prepared_v2(Some(UPGRADE_ROWS)).await else {
        return;
    };
    let pool = &old.pool;
    let historical_ledger = step_ledger(pool).await;
    release_result(pool, &release_v3()).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT version FROM meta.schema WHERE id=1")
            .fetch_one(pool)
            .await
            .unwrap(),
        catalyst::db::SCHEMA_VERSION
    );
    let (shop, marketplace, ledgers): (bool,bool,i64)=sqlx::query_as("SELECT to_regclass('shop.trades') IS NOT NULL,to_regnamespace('marketplace') IS NOT NULL,(SELECT count(*) FROM pg_tables WHERE schemaname='shop' AND tablename IN ('broker_purchases','escrow_actions','name_transfers','usage_grants'))").fetch_one(pool).await.unwrap();
    assert_eq!((shop, marketplace, ledgers), (true, false, 4));
    let private_indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE i.indrelid='social.private_voice_chats'::regclass AND c.relname IN ('private_voice_chats_caller_idx','private_voice_chats_callee_idx') AND NOT i.indisunique AND i.indisvalid").fetch_one(pool).await.unwrap();
    assert_eq!(
        private_indexes, 2,
        "both address access paths exist before migrate"
    );
    let done =
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert_eq!(
        done.steps, PREPARED_PENDING_STEPS,
        "only steps the prepared capture lacks run; historical steps never replay"
    );
    assert_only_pending_steps_added(&historical_ledger, &step_ledger(pool).await);
    assert!(
        !done.created.iter().any(|object| matches!(object,
        schema_migrate::Object::Index(name) if name == "social.private_voice_chats_caller_idx"
            || name == "social.private_voice_chats_callee_idx")),
        "migrate preserves the address access paths already created by the release"
    );
    assert!(has_column(pool, "explore.place_reports", "upload_token_hash").await);
    assert!(has_column(pool, "social.notifications", "in_app").await);
    assert!(has_column(pool, "social.event_notification_intents", "id").await);
    let owner_indexes: Vec<String> = sqlx::query_scalar(
        "SELECT indexname FROM pg_indexes WHERE schemaname = 'explore' AND indexname IN ('place_world_owner_idx', 'place_world_local_owner_idx') ORDER BY indexname",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        owner_indexes,
        vec!["place_world_local_owner_idx", "place_world_owner_idx"],
        "the prepared owner access paths survive the v3 release"
    );

    let (permissions, granted): (serde_json::Value, bool) = sqlx::query_as(
        "SELECT permissions, permissions_granted FROM explore.event_profile_settings WHERE \"user\"='0xlegacy'"
    ).fetch_one(pool).await.unwrap();
    assert_eq!(
        permissions,
        serde_json::json!(["edit_any_event"]),
        "upgrade preserves settings data"
    );
    assert!(
        !granted,
        "self-writable legacy permissions cannot become authority"
    );

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
    let Some(old) = prepared_v2(Some(ORIGIN_ROWS)).await else {
        return;
    };
    let pool = &old.pool;
    let local_community: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM social.communities WHERE name='here'")
            .fetch_one(pool)
            .await
            .unwrap();
    let mirrored_community: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM social.communities WHERE name='there'")
            .fetch_one(pool)
            .await
            .unwrap();
    release_result(pool, &release_v3()).await.unwrap();
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap();
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
    let Some(old) = prepared_v2(None).await else {
        return;
    };
    let pool = &old.pool;
    sqlx::raw_sql(sqlx::AssertSqlSafe(release_v3()))
        .execute(pool)
        .await
        .unwrap();
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

async fn release_result(pool: &PgPool, sql: &str) -> Result<(), sqlx::Error> {
    let mut connection = pool.acquire().await?;
    let result = sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(&mut *connection)
        .await
        .map(|_| ());
    if result.is_err() {
        sqlx::query("ROLLBACK").execute(&mut *connection).await?;
    }
    result
}

#[tokio::test]
async fn private_voice_release_retains_rows_and_indexes_before_stamp_then_migrate_replays_nothing()
{
    let Some(old) = prepared_v2(None).await else {
        return;
    };
    let pool = &old.pool;
    let pending = uuid::Uuid::new_v4();
    let media = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES($1,'pending-caller','pending-callee'),($2,'media-caller','media-callee')").bind(pending).bind(media).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO social.private_voice_chat_participants(call_id,address,status) VALUES($1,'media-caller','connection_interrupted'),($1,'media-callee','not_connected')").bind(media).execute(pool).await.unwrap();
    let snapshot = "SELECT jsonb_build_object('calls',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM social.private_voice_chats c),'participants',(SELECT jsonb_agg(to_jsonb(p) ORDER BY call_id,address) FROM social.private_voice_chat_participants p))";
    let before: serde_json::Value = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
    assert!(matches!(
        catalyst::db::bootstrap(pool).await,
        Err(catalyst::db::BootstrapError::Mismatch { version: 2, .. })
    ));
    assert!(matches!(
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL).await,
        Err(MigrateError::Incompatible { version: 2, .. })
    ));
    release_result(pool, &release_v3()).await.unwrap();
    let after: serde_json::Value = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
    assert_eq!(
        before, after,
        "the release changes no call or participant value"
    );
    assert!(
        release_result(pool, &release_v3()).await.is_err(),
        "version guard refuses replay"
    );
    let constraints: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key')").fetch_one(pool).await.unwrap();
    assert_eq!(constraints, 0);
    let indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE i.indrelid='social.private_voice_chats'::regclass AND c.relname IN ('private_voice_chats_caller_idx','private_voice_chats_callee_idx') AND NOT i.indisunique AND i.indisvalid").fetch_one(pool).await.unwrap();
    assert_eq!(
        indexes, 2,
        "atomic release already created both address access paths"
    );
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap();
    catalyst::db::bootstrap(pool).await.unwrap();
    assert!(
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap()
            .is_empty()
    );
    let after: serde_json::Value = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
    assert_eq!(
        before, after,
        "migrate and replay retain every stored value"
    );
    let ledger: i64 = sqlx::query_scalar("SELECT count(*) FROM meta.schema_steps")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        ledger as usize,
        schema_migrate::parse_steps(catalyst::db::MIGRATIONS_SQL).len()
    );
    old.drop().await;
}

#[tokio::test]
async fn private_voice_release_refuses_overlapping_or_non_normalized_pending_without_changes() {
    for unnormalized in [false, true] {
        let Some(old) = prepared_v2(None).await else {
            return;
        };
        let pool = &old.pool;
        sqlx::query("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,'callee'),($3,'other-caller',$4)").bind(uuid::Uuid::new_v4()).bind(if unnormalized {"MixedCaller"} else {"caller"}).bind(uuid::Uuid::new_v4()).bind(if unnormalized {"other-callee"} else {"caller"}).execute(pool).await.unwrap();
        let result = release_result(pool, &release_v3()).await.unwrap_err();
        assert!(
            result.to_string().contains(if unnormalized {
                "non-normalized pending"
            } else {
                "overlapping pending"
            }),
            "{result}"
        );
        let (version,rows,constraints):(i32,i64,i64)=sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),(SELECT count(*) FROM social.private_voice_chats),(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
        assert_eq!(
            (version, rows, constraints),
            (2, 2, 2),
            "a refused release leaves the stamp, rows and constraints intact"
        );
        old.drop().await;
    }
}

async fn seed_notification_facts(pool: &PgPool) -> serde_json::Value {
    sqlx::raw_sql("INSERT INTO social.communities(id,name,owner_address,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) VALUES ('a1111111-b111-4111-8111-e11111111111','fresh','owner','all','11111111-1111-4111-8111-111111111111','2100-01-02 03:04:05.123456'),('a2222222-b222-4222-8222-e22222222222','expired','owner','members','22222222-2222-4222-8222-222222222222','2000-01-02 03:04:05.654321'),('a3333333-b333-4333-8333-e33333333333','no facts','owner',NULL,NULL,NULL)")
        .execute(pool).await.unwrap();
    sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(id::text,id,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) ORDER BY id) FROM social.communities WHERE voice_notification_generation IS NOT NULL").fetch_one(pool).await.unwrap()
}
async fn assert_v2_facts(pool: &PgPool, original: &serde_json::Value) {
    let actual:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(id::text,id,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) ORDER BY id) FROM social.communities WHERE voice_notification_generation IS NOT NULL").fetch_one(pool).await.unwrap();
    assert_eq!(&actual, original);
    let (version, target, constraints):(i32,bool,i64)=sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),to_regclass('social.community_voice_notifications') IS NOT NULL,(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
    assert_eq!((version, target, constraints), (2, false, 2));
}

#[tokio::test]
async fn composed_voice_release_rolls_back_connection_loss_at_each_boundary_then_retries_exactly() {
    // The fixture is the actual v2 executor's output. These are prefixes of
    // the production release, with its include expanded from the same bytes.
    let release = release_v3();
    for boundary in [
        "-- Cutover boundary: destination created.",
        "-- Cutover boundary: legacy facts verified.",
        "-- Cutover boundary: replacement indexes created.",
        "UPDATE meta.schema SET version = 3",
    ] {
        let Some(old) = prepared_v2(None).await else {
            return;
        };
        let pool = &old.pool;
        let original = seed_notification_facts(pool).await;
        let ledger_before = step_ledger(pool).await;
        let end = release.find(boundary).unwrap();
        let mut interrupted = pool.acquire().await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(release[..end].to_owned()))
            .execute(&mut *interrupted)
            .await
            .unwrap();
        interrupted.close().await.unwrap();
        assert_v2_facts(pool, &original).await;
        release_result(pool, &release).await.unwrap();
        let copied:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(raw_community_id,community_id,notification_scope,generation,expires_at) ORDER BY community_id) FROM social.community_voice_notifications").fetch_one(pool).await.unwrap();
        assert_eq!(
            copied, original,
            "{boundary}: exact legacy canonical key, scope, generation and TTL including expired"
        );
        for column in [
            "voice_notification_scope",
            "voice_notification_generation",
            "voice_notification_expires_at",
        ] {
            assert!(!has_column(pool, "social.communities", column).await);
        }
        let retired: bool = sqlx::query_scalar(
            "SELECT to_regclass('social.communities_voice_notifications_idx') IS NULL",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(retired);
        assert!(
            release_result(pool, &release).await.is_err(),
            "committed stamp prevents replay/mixed-version use"
        );
        let applied =
            schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
                .await
                .unwrap();
        assert_eq!(
            applied.steps, PREPARED_PENDING_STEPS,
            "only new main event-hook steps apply; historical 0020 never re-adds retired columns"
        );
        catalyst::db::bootstrap(pool).await.unwrap();
        assert!(
            schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
                .await
                .unwrap()
                .is_empty()
        );
        assert_only_pending_steps_added(&ledger_before, &step_ledger(pool).await);
        // New aliases are additional facts, never an overwrite of the canonical backfill.
        sqlx::query("INSERT INTO social.community_voice_notifications(raw_community_id,community_id,notification_scope,generation,expires_at) SELECT upper(raw_community_id),community_id,'members',gen_random_uuid(),expires_at+interval '1 hour' FROM social.community_voice_notifications").execute(pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.community_voice_notifications"
            )
            .fetch_one(pool)
            .await
            .unwrap(),
            4
        );
        old.drop().await;
    }
}

#[tokio::test]
async fn voice_release_refuses_existing_destination_without_overwriting_or_retiring_legacy_facts() {
    let Some(old) = prepared_v2(None).await else {
        return;
    };
    let pool = &old.pool;
    let original = seed_notification_facts(pool).await;
    sqlx::raw_sql(VOICE_FACTS).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO social.community_voice_notifications(raw_community_id,community_id,notification_scope,generation,expires_at) VALUES('A1111111-B111-4111-8111-E11111111111','a1111111-b111-4111-8111-e11111111111','members',gen_random_uuid(),'2200-01-01')").execute(pool).await.unwrap();
    let prior: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM social.community_voice_notifications t")
            .fetch_one(pool)
            .await
            .unwrap();
    let refused = release_result(pool, &release_v3()).await.unwrap_err();
    assert!(
        refused.to_string().contains("destination already exists"),
        "{refused}"
    );
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM social.community_voice_notifications t")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(prior, after);
    sqlx::query("DROP TABLE social.community_voice_notifications")
        .execute(pool)
        .await
        .unwrap();
    assert_v2_facts(pool, &original).await;
    old.drop().await;
}

#[test]
fn prepared_v2_capture_bytes_match_the_pinned_provenance_manifest() {
    use sha2::{Digest, Sha256};
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema/v2-prepared-c1fab3991.json")).unwrap();
    assert_eq!(
        manifest["revision"],
        "c1fab3991444f7f95522683b488ae35371c7277b"
    );
    for (file, bytes) in [
        ("v2-prepared-c1fab3991.sql", PREPARED_V2),
        ("v2-upgrade-rows-c1fab3991.sql", UPGRADE_ROWS),
        ("v2-origin-rows-c1fab3991.sql", ORIGIN_ROWS),
    ] {
        let entry = manifest["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["file"] == file)
            .unwrap();
        assert_eq!(
            entry["sha256"],
            hex::encode(Sha256::digest(bytes.as_bytes())),
            "{file}: capture drift"
        );
    }
}

#[test]
fn shipped_main_v2_capture_matches_its_pinned_executor_provenance() {
    use sha2::{Digest, Sha256};
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema/v2-main-0fb63aa3f.json")).unwrap();
    assert_eq!(
        manifest["revision"],
        "0fb63aa3ff0bf7e7cf515dab06e00263807a7fd6"
    );
    assert_eq!(
        manifest["outputs"][0]["sha256"],
        hex::encode(Sha256::digest(MAIN_V2.as_bytes()))
    );
    assert!(!MAIN_V2.contains("0020_social_voice_notifications"));
}

#[tokio::test]
async fn earlier_event_hook_layouts_upgrade_without_losing_intents() {
    let Some(old) = prepared_v2(None).await else {
        return;
    };
    let pool = &old.pool;
    release_result(pool, &release_v3()).await.unwrap();
    schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
        .await
        .unwrap();
    // Model the actual earlier development layout: buckets already existed,
    // but creator throttling and prioritized claim ordering did not.
    sqlx::raw_sql("ALTER TABLE explore.event_moderation_hooks DROP COLUMN creator_address CASCADE;
        DROP INDEX explore.event_moderation_hooks_due_idx;
        CREATE INDEX event_moderation_hooks_due_idx ON explore.event_moderation_hooks(next_attempt_at,id) WHERE completed_at IS NULL AND attempts<6;
        DELETE FROM meta.schema_steps WHERE step IN ('0018_event_hook_compatibility','0019_event_hook_admin_priority');
        INSERT INTO explore.event_moderation_hooks(id,event_id,kind,event) VALUES(gen_random_uuid(),'retained-submission','submitted','{\"name\":\"Retained intent\"}')")
        .execute(pool).await.unwrap();
    let result =
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert!(
        result
            .steps
            .iter()
            .any(|step| step == "0018_event_hook_compatibility")
    );
    assert!(has_column(pool, "explore.event_moderation_hooks", "creator_address").await);
    assert!(has_column(pool, "explore.event_moderation_hooks", "edit_bucket").await);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='retained-submission' AND event->>'name'='Retained intent'")
        .fetch_one(pool).await.unwrap(),1);
    let due:String=sqlx::query_scalar("SELECT indexdef FROM pg_indexes WHERE schemaname='explore' AND indexname='event_moderation_hooks_due_idx'")
        .fetch_one(pool).await.unwrap();
    assert!(
        due.contains("CASE") && due.contains("THEN 1") && due.contains("ELSE 2"),
        "the object pass replaces the old due ordering: {due}"
    );
    let creator:String=sqlx::query_scalar("SELECT indexdef FROM pg_indexes WHERE schemaname='explore' AND indexname='event_moderation_hooks_creator_idx'")
        .fetch_one(pool).await.unwrap();
    assert!(
        creator.contains("kind = 'edited'"),
        "the creator index covers only throttled edits: {creator}"
    );
    sqlx::query("INSERT INTO explore.event_moderation_hooks(id,event_id,kind,event,creator_address,edit_bucket) VALUES(gen_random_uuid(),'new-edit','edited','{}','creator',timezone('UTC',now()))")
        .execute(pool).await.unwrap();
    assert!(
        schema_migrate::migrate(pool, catalyst::db::SCHEMA_SQL, catalyst::db::MIGRATIONS_SQL)
            .await
            .unwrap()
            .is_empty()
    );
    old.drop().await;
}
