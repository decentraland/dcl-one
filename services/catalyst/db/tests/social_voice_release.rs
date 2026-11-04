//! Exercise the dormant v3 release against genuine v2 layouts and receipts.
//! Current v3 apply/migrate/bootstrap execution is checked in social_voice_upgrade.
use sha2::{Digest, Sha256};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};

const STEP: &str = "0020_social_voice_notifications";
const STEP_HASH: &str = "41c5172fe07ee74acad62a9e1fa3ae74e5eaeb51d2a13262146f20034faaef17";
const PREPARED_V2: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/schema/v2-prepared-c1fab3991.sql"
));
const MAIN_V2: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/schema/v2-main-0fb63aa3f.sql"
));
const UPGRADE_ROWS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/schema/v2-upgrade-rows-c1fab3991.sql"
));
const ORIGIN_ROWS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/schema/v2-origin-rows-c1fab3991.sql"
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
const LEDGER: &str =
    "SELECT step,sha256,created_fresh,applied_at FROM meta.schema_steps ORDER BY step";
type Receipt = (String, String, bool, chrono::NaiveDateTime);

#[derive(Clone, Copy, Debug)]
enum Source {
    Main,
    Prepared,
}
impl Source {
    fn schema(self) -> &'static str {
        match self {
            Self::Main => MAIN_V2,
            Self::Prepared => PREPARED_V2,
        }
    }
}
fn assert_capture_hash(text: &str, sha256: &str) {
    assert_eq!(hex::encode(Sha256::digest(text.as_bytes())), sha256);
}
fn release_v3() -> String {
    let mut release = RELEASE_V3.to_owned();
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
    async fn empty() -> Option<Self> {
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
        Some(Self { pool, admin, name })
    }
    async fn new(source: Source) -> Option<Self> {
        // Hashes are the untouched output digests in the adjacent provenance
        // manifests; import their real ledger, never invent historical receipts.
        assert_capture_hash(
            MAIN_V2,
            "0a7e97ef603c04b9ee0f956d8242fd301aa523f94c6b1fec335264eaa9b60be3",
        );
        assert_capture_hash(
            PREPARED_V2,
            "3242308447fdc6294c7c100d1265eaba911f56dfaf1c03a463dcfc542c6328c6",
        );
        assert_capture_hash(
            UPGRADE_ROWS,
            "c8354451493799ce31e99c9369fc247021aa61045dedd9682709e2bbff0396bf",
        );
        assert_capture_hash(
            ORIGIN_ROWS,
            "869841157fcb18ddb32fd030f6920127c41142656f91046ae426be0dd1ab3925",
        );
        let db = Self::empty().await?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(source.schema().to_owned()))
            .execute(&db.pool)
            .await
            .unwrap();
        if matches!(source, Source::Prepared) {
            for rows in [UPGRADE_ROWS, ORIGIN_ROWS] {
                sqlx::raw_sql(sqlx::AssertSqlSafe(rows.to_owned()))
                    .execute(&db.pool)
                    .await
                    .unwrap();
            }
        }
        let receipts: Vec<Receipt> = sqlx::query_as(LEDGER).fetch_all(&db.pool).await.unwrap();
        match source {
            Source::Main => assert!(!receipts.iter().any(|row| row.0 == STEP)),
            Source::Prepared => assert!(
                receipts
                    .iter()
                    .any(|row| row.0 == STEP && row.1 == STEP_HASH && !row.2)
            ),
        }
        Some(db)
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
    let indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE i.indrelid='social.private_voice_chats'::regclass AND c.relname IN ('private_voice_chats_caller_idx','private_voice_chats_callee_idx') AND NOT i.indisunique AND i.indisvalid").fetch_one(pool).await.unwrap();
    assert_eq!(
        indexes, 2,
        "both replacement indexes exist at release commit"
    );
}
async fn preserved_facts(pool: &PgPool) -> Vec<(String, String)> {
    let mut facts = Vec::new();
    // Every table populated by either genuine application-row capture, plus
    // seeded private intent/media facts. JSONB retains each value exactly.
    for table in [
        "credits.purchases",
        "worlds.worlds",
        "explore.event_profile_settings",
        "explore.event",
        "explore.event_moderation",
        "explore.events_local",
        "explore.place",
        "social.communities",
        "social.community_members",
        "social.private_voice_chats",
        "social.private_voice_chat_participants",
    ] {
        let projection = if table == "social.communities" {
            "to_jsonb(t)-'voice_notification_scope'-'voice_notification_generation'-'voice_notification_expires_at'"
        } else {
            "to_jsonb(t)"
        };
        let sql = format!(
            "SELECT coalesce(jsonb_agg({projection} ORDER BY {projection}), '[]'::jsonb)::text FROM {table} t"
        );
        facts.push((
            table.into(),
            sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(pool)
                .await
                .unwrap(),
        ));
    }
    facts
}
async fn seed_voice_facts(pool: &PgPool, source: Source) {
    // Synthetic edge facts supplement captured rows; they do not alter the
    // historical stamp or ledger and use the source layout's own constraints.
    sqlx::raw_sql("INSERT INTO social.communities(id,name,owner_address) VALUES ('a1111111-b111-4111-8111-e11111111111','Existing community','owner'); INSERT INTO social.community_members(community_id,member_address,role) VALUES ('a1111111-b111-4111-8111-e11111111111','owner','owner'); INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES ('11111111-1111-4111-8111-111111111111','pending-caller','pending-callee'),('22222222-2222-4222-8222-222222222222','media-caller','media-callee'); INSERT INTO social.private_voice_chat_participants(call_id,address,status) VALUES ('22222222-2222-4222-8222-222222222222','media-caller','connection_interrupted'),('22222222-2222-4222-8222-222222222222','media-callee','not_connected')").execute(pool).await.unwrap();
    if matches!(source, Source::Prepared) {
        sqlx::raw_sql("INSERT INTO social.communities(id,name,owner_address,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) VALUES ('a2222222-b222-4222-8222-e22222222222','Live generation','owner','all','11111111-1111-4111-8111-111111111111','2100-01-02 03:04:05.123456'),('a3333333-b333-4333-8333-e33333333333','Expired generation','owner','members','22222222-2222-4222-8222-222222222222','2000-01-02 03:04:05.654321')").execute(pool).await.unwrap();
    }
}
async fn notification_facts(pool: &PgPool, source: Source) -> String {
    if matches!(source, Source::Main) {
        return "[]".into();
    }
    sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_array(id::text,id,voice_notification_scope,voice_notification_generation,voice_notification_expires_at) ORDER BY id),'[]'::jsonb)::text FROM social.communities WHERE voice_notification_generation IS NOT NULL").fetch_one(pool).await.unwrap()
}
#[tokio::test]
async fn both_genuine_v2_routes_roll_back_at_every_boundary_then_preserve_rows_and_real_ledger() {
    let release = release_v3();
    for source in [Source::Main, Source::Prepared] {
        for boundary in [
            "-- Cutover boundary: destination created.",
            "-- Cutover boundary: legacy facts verified.",
            "-- Cutover boundary: replacement indexes created.",
            "UPDATE meta.schema SET version = 3",
            "COMMIT;",
        ] {
            let Some(db) = Database::new(source).await else {
                return;
            };
            let pool = &db.pool;
            seed_voice_facts(pool, source).await;
            let facts = preserved_facts(pool).await;
            let notifications = notification_facts(pool, source).await;
            let ledger: Vec<Receipt> = sqlx::query_as(LEDGER).fetch_all(pool).await.unwrap();
            let mut interrupted = pool.acquire().await.unwrap();
            sqlx::raw_sql(sqlx::AssertSqlSafe(
                release[..release.find(boundary).unwrap()].to_owned(),
            ))
            .execute(&mut *interrupted)
            .await
            .unwrap();
            if boundary == "-- Cutover boundary: replacement indexes created." {
                let (version,uniques,indexes): (i32,i64,i64) = sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key')),(SELECT count(*) FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE i.indrelid='social.private_voice_chats'::regclass AND c.relname IN ('private_voice_chats_caller_idx','private_voice_chats_callee_idx') AND NOT i.indisunique AND i.indisvalid)").fetch_one(&mut *interrupted).await.unwrap();
                assert_eq!(
                    (version, uniques, indexes),
                    (2, 2, 2),
                    "replacement indexes precede UNIQUE removal and final stamp"
                );
            }
            interrupted.close().await.unwrap();
            let (version,target,legacy,uniques): (i32,bool,i64,i64) = sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),to_regclass('social.community_voice_notifications') IS NOT NULL,(SELECT count(*) FROM pg_attribute WHERE attrelid='social.communities'::regclass AND NOT attisdropped AND attname LIKE 'voice_notification_%'),(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
            assert_eq!(
                (version, target, legacy, uniques),
                (
                    2,
                    false,
                    if matches!(source, Source::Prepared) {
                        3
                    } else {
                        0
                    },
                    2
                ),
                "{source:?}: {boundary}"
            );
            assert_eq!(preserved_facts(pool).await, facts);
            assert_eq!(notification_facts(pool, source).await, notifications);
            assert_eq!(
                sqlx::query_as::<_, Receipt>(LEDGER)
                    .fetch_all(pool)
                    .await
                    .unwrap(),
                ledger
            );
            release_result(pool, &release).await.unwrap();
            assert_release_indexes(pool).await;
            assert_eq!(
                sqlx::query_scalar::<_, i32>("SELECT version FROM meta.schema WHERE id=1")
                    .fetch_one(pool)
                    .await
                    .unwrap(),
                3
            );
            let copied: String = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_array(raw_community_id,community_id,notification_scope,generation,expires_at) ORDER BY community_id),'[]'::jsonb)::text FROM social.community_voice_notifications").fetch_one(pool).await.unwrap();
            assert_eq!(
                copied, notifications,
                "scope, generation and exact TTL, including expired facts"
            );
            assert!(
                release_result(pool, &release).await.is_err(),
                "release refuses execution at3"
            );
            assert_eq!(preserved_facts(pool).await, facts);
            assert_eq!(
                sqlx::query_as::<_, Receipt>(LEDGER)
                    .fetch_all(pool)
                    .await
                    .unwrap(),
                ledger,
                "no invented, retired or rewritten historical receipt"
            );
            db.close().await;
        }
    }
}

#[tokio::test]
async fn release_refuses_partial_or_unreceipted_legacy_layout_and_changed_hash() {
    for corruption in [
        "ALTER TABLE social.communities DROP COLUMN voice_notification_scope CASCADE",
        "DELETE FROM meta.schema_steps WHERE step='0020_social_voice_notifications'",
        "ALTER TABLE social.communities DROP CONSTRAINT communities_voice_notification_scope_check; ALTER TABLE social.communities ADD CONSTRAINT communities_voice_notification_scope_check CHECK (true); INSERT INTO social.communities(id,name,owner_address,voice_notification_scope) VALUES ('a4444444-b444-4444-8444-e44444444444','incomplete fact','owner','all')",
        "UPDATE meta.schema_steps SET sha256=repeat('0',64) WHERE step='0020_social_voice_notifications'",
    ] {
        let Some(db) = Database::new(Source::Prepared).await else {
            return;
        };
        let pool = &db.pool;
        sqlx::raw_sql(sqlx::AssertSqlSafe(corruption.to_owned()))
            .execute(pool)
            .await
            .unwrap();
        let facts = preserved_facts(pool).await;
        let ledger: Vec<Receipt> = sqlx::query_as(LEDGER).fetch_all(pool).await.unwrap();
        assert!(release_result(pool, &release_v3()).await.is_err());
        let (version,target,uniques): (i32,bool,i64) = sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),to_regclass('social.community_voice_notifications') IS NOT NULL,(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
        assert_eq!((version, target, uniques), (2, false, 2));
        assert_eq!(preserved_facts(pool).await, facts);
        assert_eq!(
            sqlx::query_as::<_, Receipt>(LEDGER)
                .fetch_all(pool)
                .await
                .unwrap(),
            ledger
        );
        db.close().await;
    }
}

#[tokio::test]
async fn release_refuses_overlapping_or_non_normalized_pending_without_mutation() {
    for source in [Source::Main, Source::Prepared] {
        for unnormalized in [false, true] {
            let Some(db) = Database::new(source).await else {
                return;
            };
            let pool = &db.pool;
            sqlx::query("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,'callee'),($3,'other-caller',$4)").bind(uuid::Uuid::new_v4()).bind(if unnormalized {"MixedCaller"} else {"caller"}).bind(uuid::Uuid::new_v4()).bind(if unnormalized {"other-callee"} else {"caller"}).execute(pool).await.unwrap();
            let before = preserved_facts(pool).await;
            let ledger: Vec<Receipt> = sqlx::query_as(LEDGER).fetch_all(pool).await.unwrap();
            let error = release_result(pool, &release_v3()).await.unwrap_err();
            assert!(
                error.to_string().contains(if unnormalized {
                    "non-normalized pending"
                } else {
                    "overlapping pending"
                }),
                "{error}"
            );
            assert_eq!(preserved_facts(pool).await, before);
            assert_eq!(
                sqlx::query_as::<_, Receipt>(LEDGER)
                    .fetch_all(pool)
                    .await
                    .unwrap(),
                ledger
            );
            let (version,target,uniques): (i32,bool,i64) = sqlx::query_as("SELECT (SELECT version FROM meta.schema WHERE id=1),to_regclass('social.community_voice_notifications') IS NOT NULL,(SELECT count(*) FROM pg_constraint WHERE conrelid='social.private_voice_chats'::regclass AND conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))").fetch_one(pool).await.unwrap();
            assert_eq!((version, target, uniques), (2, false, 2));
            db.close().await;
        }
    }
}
