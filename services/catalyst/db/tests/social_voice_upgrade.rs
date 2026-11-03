//! Upgrade a populated predecessor through the production additive executor.
use catalyst_db::{MIGRATIONS_SQL, SCHEMA_SQL, schema_migrate};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
const STEP: &str = "0020_social_voice_notifications";
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
        catalyst_db::apply_schema(&pool).await.unwrap();
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
    let facts:String=sqlx::query_scalar("SELECT (to_jsonb(c)-'voice_notification_scope'-'voice_notification_generation'-'voice_notification_expires_at')::text FROM social.communities c WHERE id=$1").bind(id).fetch_one(pool).await.unwrap();
    // Reconstruct the populated predecessor, not an application database.
    sqlx::raw_sql("ALTER TABLE social.communities DROP COLUMN voice_notification_scope CASCADE; ALTER TABLE social.communities DROP COLUMN voice_notification_generation CASCADE; ALTER TABLE social.communities DROP COLUMN voice_notification_expires_at CASCADE; DROP TABLE social.event_payloads;").execute(pool).await.unwrap();
    sqlx::query("DELETE FROM meta.schema_steps WHERE step=$1")
        .bind(STEP)
        .execute(pool)
        .await
        .unwrap();
    let before: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT step,sha256,created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();
    let done = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert_eq!(done.steps, vec![STEP]);
    let after:String=sqlx::query_scalar("SELECT (to_jsonb(c)-'voice_notification_scope'-'voice_notification_generation'-'voice_notification_expires_at')::text FROM social.communities c WHERE id=$1").bind(id).fetch_one(pool).await.unwrap();
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
    let empty:bool=sqlx::query_scalar("SELECT voice_notification_scope IS NULL AND voice_notification_generation IS NULL AND voice_notification_expires_at IS NULL FROM social.communities WHERE id=$1").bind(id).fetch_one(pool).await.unwrap();
    assert!(empty);
    let ledger: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT step,sha256,created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(
        ledger
            .iter()
            .filter(|(step, _, _)| step != STEP)
            .cloned()
            .collect::<Vec<_>>(),
        before
    );
    let source = schema_migrate::parse_steps(MIGRATIONS_SQL)
        .into_iter()
        .find(|step| step.name == STEP)
        .unwrap();
    assert!(ledger.contains(&(STEP.into(), source.sha256, false)));
    let replay = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert!(replay.steps.is_empty());
    assert!(replay.created.is_empty());
    let event = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO social.event_payloads(id,payload,expires_at) VALUES($1,decode('01','hex'),now()+interval '24 hours')").bind(event).execute(pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE social.communities SET voice_notification_scope='all' WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await
            .is_err()
    );
    db.close().await;
}
