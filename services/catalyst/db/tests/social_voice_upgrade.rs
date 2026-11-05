//! Upgrade genuinely prepared v2 facts through the composed v3 release and
//! production additive executor, retaining the historical migration ledger.
use catalyst_db::{MIGRATIONS_SQL, SCHEMA_SQL, schema_migrate};
use sha2::{Digest, Sha256};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
const STEP: &str = "0020_social_voice_notifications";
/// The steps a prepared version-2 database has not recorded.
const PREPARED_PENDING_STEPS: [&str; 4] = [
    "0018_event_hook_compatibility",
    "0019_event_hook_admin_priority",
    "0023_social_referral_pending_reward",
    "0024_social_referral_reward_receipt",
];
const PREPARED_V2: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/schema/v2-prepared-c1fab3991.sql"
));
const RELEASE_V3: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/schemas/releases/v3-private-voice-pending.sql"
));
const VOICE_FACTS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/schemas/releases/v3/41_social_voice_notifications.sql"
));

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

struct Database {
    pool: PgPool,
    admin: PgPool,
    name: String,
}
impl Database {
    async fn new() -> Option<Self> {
        Self::with_schema(PREPARED_V2).await
    }
    async fn with_schema(schema: &str) -> Option<Self> {
        let url = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
        let options: PgConnectOptions = url.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_test_social_voice_{}",
            uuid::Uuid::new_v4().simple()
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(
                options
                    .database(&name)
                    .options([("TimeZone", "UTC"), ("search_path", "content,meta")]),
            )
            .await
            .unwrap();
        // Captured from c1fab3991's real v2 release and additive executor; the
        // provenance manifest beside the fixture pins these bytes and ledger.
        assert_eq!(
            hex::encode(Sha256::digest(PREPARED_V2.as_bytes())),
            "3242308447fdc6294c7c100d1265eaba911f56dfaf1c03a463dcfc542c6328c6"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(schema.to_owned()))
            .execute(&pool)
            .await
            .unwrap();
        Some(Self { pool, admin, name })
    }
    async fn close(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH(FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
}
#[tokio::test]
async fn populated_voice_notification_upgrade_preserves_facts_and_replays_its_ledger() {
    let Some(db) = Database::new().await else {
        return;
    };
    let pool = &db.pool;
    let id = uuid::Uuid::new_v4();
    let owner = "0x1111111111111111111111111111111111111111";
    sqlx::query(
        "INSERT INTO social.communities(id,name,owner_address) VALUES($1,'Existing community',$2)",
    )
    .bind(id)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO social.community_members(community_id,member_address,role) VALUES($1,$2,'owner')").bind(id).bind(owner).execute(pool).await.unwrap();
    // Null facts remain absent; live and expired generations retain their exact
    // scope, UUID and TTL without refreshing either during the release.
    sqlx::raw_sql("INSERT INTO social.communities(id,name,owner_address,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) VALUES ('a1111111-b111-4111-8111-e11111111111','Live generation','owner','all','11111111-1111-4111-8111-111111111111','2100-01-02 03:04:05.123456'),('a2222222-b222-4222-8222-e22222222222','Expired generation','owner','members','22222222-2222-4222-8222-222222222222','2000-01-02 03:04:05.654321')")
        .execute(pool)
        .await
        .unwrap();
    let facts_sql = "SELECT jsonb_agg(to_jsonb(c)-'voice_notification_scope'-'voice_notification_generation'-'voice_notification_expires_at' ORDER BY id)::text FROM social.communities c";
    let facts: String = sqlx::query_scalar(facts_sql).fetch_one(pool).await.unwrap();
    let notifications: String = sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(id::text,id,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) ORDER BY id)::text FROM social.communities WHERE voice_notification_generation IS NOT NULL").fetch_one(pool).await.unwrap();
    let ledger_sql =
        "SELECT step,sha256,created_fresh,applied_at FROM meta.schema_steps ORDER BY step";
    let before: Vec<(String, String, bool, chrono::NaiveDateTime)> =
        sqlx::query_as(ledger_sql).fetch_all(pool).await.unwrap();
    let source = schema_migrate::parse_steps(MIGRATIONS_SQL)
        .into_iter()
        .find(|step| step.name == STEP)
        .unwrap();
    assert!(
        before
            .iter()
            .any(|(step, sha256, fresh, _)| { step == STEP && sha256 == &source.sha256 && !fresh })
    );
    let invalid =
        sqlx::query("UPDATE social.communities SET voice_notification_scope='all' WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    assert!(matches!(
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL).await,
        Err(schema_migrate::MigrateError::Incompatible { version: 2, .. })
    ));
    // Expand psql's include from the same canonical bytes as fresh bootstrap.
    sqlx::raw_sql(sqlx::AssertSqlSafe(release_v3()))
        .execute(pool)
        .await
        .unwrap();
    let done = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert_eq!(
        done.steps, PREPARED_PENDING_STEPS,
        "only steps the prepared capture lacks run; recorded historical steps never replay"
    );
    let after: String = sqlx::query_scalar(facts_sql).fetch_one(pool).await.unwrap();
    assert_eq!(after, facts);
    let member: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.community_members WHERE community_id=$1 AND member_address=$2",
    )
    .bind(id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(member, 1);
    let empty:bool=sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM social.community_voice_notifications WHERE community_id=$1)").bind(id).fetch_one(pool).await.unwrap();
    assert!(empty);
    let copied: String = sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(raw_community_id,community_id,notification_scope,generation,expires_at) ORDER BY community_id)::text FROM social.community_voice_notifications").fetch_one(pool).await.unwrap();
    assert_eq!(copied, notifications);
    let retired: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_attribute WHERE attrelid='social.communities'::regclass AND NOT attisdropped AND attname IN ('voice_notification_scope','voice_notification_generation','voice_notification_expires_at')").fetch_one(pool).await.unwrap();
    assert_eq!(
        retired, 0,
        "historical 0020 must not re-add retired columns"
    );
    let ledger: Vec<(String, String, bool, chrono::NaiveDateTime)> =
        sqlx::query_as(ledger_sql).fetch_all(pool).await.unwrap();
    assert_eq!(ledger.len(), before.len() + PREPARED_PENDING_STEPS.len());
    for row in &before {
        assert!(
            ledger.contains(row),
            "every historical hash, origin and timestamp survives: {row:?}"
        );
    }
    let added: Vec<_> = ledger.iter().filter(|row| !before.contains(row)).collect();
    assert_eq!(
        added.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
        PREPARED_PENDING_STEPS
    );
    let embedded = schema_migrate::parse_steps(MIGRATIONS_SQL);
    for row in added {
        assert_eq!(
            row.1,
            embedded
                .iter()
                .find(|step| step.name == row.0)
                .unwrap()
                .sha256
        );
        assert!(
            !row.2,
            "the actual executor records each new step as migrated"
        );
    }
    let replay = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert!(replay.steps.is_empty());
    assert!(replay.created.is_empty());
    catalyst_db::bootstrap(pool).await.unwrap();
    let event = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO social.event_payloads(id,payload,expires_at) VALUES($1,decode('01','hex'),now()+interval '24 hours')").bind(event).execute(pool).await.unwrap();
    let invalid = sqlx::query("INSERT INTO social.community_voice_notifications(raw_community_id,community_id,notification_scope,generation,expires_at) VALUES($1,$2,'invalid',gen_random_uuid(),now()+interval '24 hours')")
        .bind(id.to_string()).bind(id).execute(pool).await.unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    db.close().await;
}

const MAIN_V2: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/schema/v2-main-0fb63aa3f.sql"
));

async fn release_result(pool: &PgPool, release: &str) -> Result<(), sqlx::Error> {
    let mut connection = pool.acquire().await?;
    let result = sqlx::raw_sql(sqlx::AssertSqlSafe(release.to_owned()))
        .execute(&mut *connection)
        .await
        .map(|_| ());
    if result.is_err() {
        sqlx::query("ROLLBACK").execute(&mut *connection).await?;
    }
    result
}

async fn assert_release_indexes(pool: &PgPool) {
    let indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE i.indrelid='social.private_voice_chats'::regclass AND c.relname IN ('private_voice_chats_caller_idx','private_voice_chats_callee_idx') AND NOT i.indisunique AND i.indisvalid")
        .fetch_one(pool).await.unwrap();
    assert_eq!(
        indexes, 2,
        "both replacement indexes exist at the release commit, before migrate"
    );
}

#[tokio::test]
async fn real_main_v2_release_rolls_back_at_each_boundary_then_migrates_without_fake_receipts() {
    let release = release_v3();
    for boundary in [
        "-- Cutover boundary: destination created.",
        "-- Cutover boundary: legacy facts verified.",
        "-- Cutover boundary: replacement indexes created.",
        "UPDATE meta.schema SET version = 3",
    ] {
        let Some(db) = Database::with_schema(MAIN_V2).await else {
            return;
        };
        let pool = &db.pool;
        sqlx::raw_sql("INSERT INTO social.communities(id,name,owner_address) VALUES ('a1111111-b111-4111-8111-e11111111111','main community','owner'); INSERT INTO social.community_members(community_id,member_address,role) VALUES ('a1111111-b111-4111-8111-e11111111111','owner','owner'); INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES ('11111111-1111-4111-8111-111111111111','pending-caller','pending-callee'),('22222222-2222-4222-8222-222222222222','media-caller','media-callee'); INSERT INTO social.private_voice_chat_participants(call_id,address,status) VALUES ('22222222-2222-4222-8222-222222222222','media-caller','connection_interrupted'),('22222222-2222-4222-8222-222222222222','media-callee','not_connected')")
            .execute(pool).await.unwrap();
        let snapshot = "SELECT jsonb_build_object('calls',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM social.private_voice_chats c),'participants',(SELECT jsonb_agg(to_jsonb(p) ORDER BY call_id,address) FROM social.private_voice_chat_participants p),'members',(SELECT jsonb_agg(to_jsonb(m) ORDER BY community_id,member_address) FROM social.community_members m))::text";
        let before: String = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
        let ledger_sql =
            "SELECT step,sha256,created_fresh,applied_at FROM meta.schema_steps ORDER BY step";
        let ledger: Vec<(String, String, bool, chrono::NaiveDateTime)> =
            sqlx::query_as(ledger_sql).fetch_all(pool).await.unwrap();
        assert!(!ledger.iter().any(|row| row.0 == STEP));
        let mut interrupted = pool.acquire().await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(
            release[..release.find(boundary).unwrap()].to_owned(),
        ))
        .execute(&mut *interrupted)
        .await
        .unwrap();
        interrupted.close().await.unwrap();
        let (version,target,legacy,uniques): (i32,bool,i64,i64) = sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),to_regclass('social.community_voice_notifications') IS NOT NULL,(SELECT count(*) FROM pg_attribute WHERE attrelid='social.communities'::regclass AND NOT attisdropped AND attname LIKE 'voice_notification_%'),(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
        assert_eq!(
            (version, target, legacy, uniques),
            (2, false, 0, 2),
            "{boundary}: interrupted cutover rolls back"
        );
        let after: Vec<(String, String, bool, chrono::NaiveDateTime)> =
            sqlx::query_as(ledger_sql).fetch_all(pool).await.unwrap();
        assert_eq!(ledger, after);
        release_result(pool, &release).await.unwrap();
        assert_release_indexes(pool).await;
        assert!(release_result(pool, &release).await.is_err());
        let initial = schema_migrate::status(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap();
        assert!(!initial.pending_steps.iter().any(|step| step == STEP));
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap();
        catalyst_db::bootstrap(pool).await.unwrap();
        assert_eq!(
            schema_migrate::status(pool, SCHEMA_SQL, MIGRATIONS_SQL)
                .await
                .unwrap()
                .exit_code(),
            0
        );
        assert!(
            schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
                .await
                .unwrap()
                .is_empty()
        );
        let after: String = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
        assert_eq!(
            before, after,
            "every call, participant and membership value survives"
        );
        let after: Vec<(String, String, bool, chrono::NaiveDateTime)> =
            sqlx::query_as(ledger_sql).fetch_all(pool).await.unwrap();
        assert!(
            !after.iter().any(|row| row.0 == STEP),
            "no invented historical 0020 receipt"
        );
        for row in &ledger {
            assert!(
                after.contains(row),
                "original ledger row survives unchanged"
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.community_voice_notifications"
            )
            .fetch_one(pool)
            .await
            .unwrap(),
            0
        );
        db.close().await;
    }
}

/// Shipped main v2 has `social.referral_progress` without the referral
/// lifecycle columns. Step 0021 adds them: its volatile `id` default rewrites
/// the table under ACCESS EXCLUSIVE (inside the writer-stopped cutover) and
/// mints an id per existing row. Every stored value survives; granted rows are
/// marked, and no reward is left pending.
#[tokio::test]
async fn populated_main_v2_referrals_survive_the_upgrade_with_minted_ids() {
    let Some(db) = Database::with_schema(MAIN_V2).await else {
        return;
    };
    let pool = &db.pool;
    sqlx::raw_sql("INSERT INTO social.referral_progress(invited_user,referrer,status,created_at,updated_at) SELECT 'invited-'||n,'referrer-'||(n%3),(ARRAY['pending','signed_up','tier_granted','rejected_ip_match'])[1+n%4],timestamp '2026-01-01'+n*interval '1 minute',timestamp '2026-02-01'+n*interval '1 minute' FROM generate_series(1,200) AS n")
        .execute(pool).await.unwrap();
    let snapshot = "SELECT jsonb_agg(jsonb_build_object('invited_user',invited_user,'referrer',referrer,'status',status,'created_at',created_at,'updated_at',updated_at) ORDER BY invited_user)::text FROM social.referral_progress";
    let before: String = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
    let file = "SELECT pg_relation_filenode('social.referral_progress')::bigint";
    let file_before: i64 = sqlx::query_scalar(file).fetch_one(pool).await.unwrap();
    release_result(pool, &release_v3()).await.unwrap();
    let done = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert!(
        done.steps
            .iter()
            .any(|step| step == "0021_social_referral_lifecycle")
    );
    catalyst_db::bootstrap(pool).await.unwrap();
    assert_eq!(
        schema_migrate::status(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap()
            .exit_code(),
        0
    );
    let after: String = sqlx::query_scalar(snapshot).fetch_one(pool).await.unwrap();
    assert_eq!(before, after, "every stored referral value survives");
    let (rows, ids, granted, tier_rows, pending): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*),count(DISTINCT id),count(*) FILTER (WHERE tier_granted),count(*) FILTER (WHERE status='tier_granted'),count(reward_tier) FROM social.referral_progress",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!((rows, ids, pending), (200, 200, 0), "one minted id per row");
    assert_eq!(granted, tier_rows, "granted rows are marked");
    let file_after: i64 = sqlx::query_scalar(file).fetch_one(pool).await.unwrap();
    assert_ne!(
        file_before, file_after,
        "the volatile id default rewrites the table"
    );
    db.close().await;
}

#[tokio::test]
async fn v3_rejects_retired_column_constraint_and_index_drift_without_mutation() {
    let Some(db) = Database::with_schema(MAIN_V2).await else {
        return;
    };
    let pool = &db.pool;
    release_result(pool, &release_v3()).await.unwrap();
    schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE social.communities ADD COLUMN voice_notification_scope text, ADD COLUMN voice_notification_generation uuid, ADD COLUMN voice_notification_expires_at timestamp without time zone, ADD CONSTRAINT communities_voice_notification_scope_check CHECK (voice_notification_scope IS NULL); CREATE INDEX communities_voice_notifications_idx ON social.communities (voice_notification_expires_at)").execute(pool).await.unwrap();
    let gaps = schema_migrate::gaps(pool, SCHEMA_SQL).await.unwrap();
    assert_eq!(gaps.retired.len(), 5);
    for column in [
        "voice_notification_scope",
        "voice_notification_generation",
        "voice_notification_expires_at",
    ] {
        assert!(gaps.fatal().iter().any(|gap| gap.ends_with(column)));
    }
    assert_eq!(
        schema_migrate::status(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap()
            .exit_code(),
        4
    );
    assert!(matches!(
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL).await,
        Err(schema_migrate::MigrateError::RetiredDrift(_))
    ));
    assert!(catalyst_db::bootstrap(pool).await.is_err());
    assert_eq!(
        schema_migrate::gaps(pool, SCHEMA_SQL)
            .await
            .unwrap()
            .retired
            .len(),
        5
    );
    db.close().await;
}

#[tokio::test]
async fn release_refuses_partial_or_unreceipted_legacy_layout_and_changed_hash() {
    for corruption in [
        "ALTER TABLE social.communities DROP COLUMN voice_notification_scope CASCADE",
        "DELETE FROM meta.schema_steps WHERE step='0020_social_voice_notifications'",
        "ALTER TABLE social.communities DROP CONSTRAINT communities_voice_notification_scope_check; ALTER TABLE social.communities ADD CONSTRAINT communities_voice_notification_scope_check CHECK (true); INSERT INTO social.communities(id,name,owner_address,voice_notification_scope) VALUES ('a1111111-b111-4111-8111-e11111111111','incomplete fact','owner','all')",
        "UPDATE meta.schema_steps SET sha256=repeat('0',64) WHERE step='0020_social_voice_notifications'",
    ] {
        let Some(db) = Database::new().await else {
            return;
        };
        let pool = &db.pool;
        sqlx::raw_sql(sqlx::AssertSqlSafe(corruption.to_owned()))
            .execute(pool)
            .await
            .unwrap();
        assert!(release_result(pool, &release_v3()).await.is_err());
        let (version,target,uniques): (i32,bool,i64) = sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),to_regclass('social.community_voice_notifications') IS NOT NULL,(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
        assert_eq!((version, target, uniques), (2, false, 2));
        db.close().await;
    }
}

#[tokio::test]
async fn v3_retired_step_still_verifies_genuine_receipt_hash_in_all_entry_points() {
    let Some(db) = Database::new().await else {
        return;
    };
    let pool = &db.pool;
    release_result(pool, &release_v3()).await.unwrap();
    schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    sqlx::query("UPDATE meta.schema_steps SET sha256=repeat('0',64) WHERE step=$1")
        .bind(STEP)
        .execute(pool)
        .await
        .unwrap();
    let status = schema_migrate::status(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert_eq!(status.exit_code(), 4);
    assert_eq!(status.drifted_steps, [STEP]);
    assert!(matches!(
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL).await,
        Err(schema_migrate::MigrateError::StepDrift { .. })
    ));
    assert!(catalyst_db::bootstrap(pool).await.is_err());
    db.close().await;
}
