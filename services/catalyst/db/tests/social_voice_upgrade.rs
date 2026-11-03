//! Upgrade genuinely prepared v2 facts through the composed v3 release and
//! production additive executor, retaining the historical migration ledger.
use catalyst_db::{MIGRATIONS_SQL, SCHEMA_SQL, schema_migrate};
use sha2::{Digest, Sha256};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
const STEP: &str = "0020_social_voice_notifications";
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
    "/schemas/41_social_voice_notifications.sql"
));

fn release_v3() -> String {
    let directive = "\\ir ../41_social_voice_notifications.sql";
    assert_eq!(RELEASE_V3.matches(directive).count(), 1);
    RELEASE_V3.replace(directive, VOICE_FACTS)
}

struct Database {
    pool: PgPool,
    admin: PgPool,
    name: String,
}
impl Database {
    async fn new() -> Option<Self> {
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
        sqlx::raw_sql(PREPARED_V2).execute(&pool).await.unwrap();
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
    assert!(
        done.steps.is_empty(),
        "recorded historical steps are never replayed"
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
    assert_eq!(ledger, before, "retain every historical ledger value");
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
