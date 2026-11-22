//! Upgrade regression for durable first-wear notification progress.
//! The configured DSN is used only to create and destroy this test's database.
use catalyst_db::{MIGRATIONS_SQL, SCHEMA_SQL, schema_migrate};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};

struct OwnedDatabase {
    pool: PgPool,
    admin: PgPool,
    name: String,
}

impl OwnedDatabase {
    async fn new() -> Option<Self> {
        let url = match std::env::var("CATALYST_TEST_DATABASE_URL") {
            Ok(url) => url,
            Err(_) if std::env::var_os("GITHUB_ACTIONS").is_some() => {
                panic!("first-wear upgrade requires CATALYST_TEST_DATABASE_URL in CI")
            }
            Err(_) => return None,
        };
        let options: PgConnectOptions = url.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!("catalyst_test_progress_{}", uuid::Uuid::new_v4().simple());
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
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
}

async fn legacy_facts(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT 'cursor:' || to_jsonb(c)::text FROM social.first_wear_cursor c
         UNION ALL SELECT 'baseline:' || to_jsonb(b)::text FROM social.first_wear_baseline b
         UNION ALL SELECT 'history:' || to_jsonb(h)::text FROM social.worn_history h
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn first_wear_outbox_upgrade_preserves_legacy_facts_and_is_idempotent() {
    let Some(db) = OwnedDatabase::new().await else {
        return;
    };
    let pool = &db.pool;
    sqlx::raw_sql(
        "INSERT INTO social.first_wear_cursor (id, last_seen) VALUES (1, '2026-01-02 03:04:05');
         INSERT INTO social.first_wear_baseline (address, seeded_at) VALUES ('legacy-wallet', '2026-01-01');
         INSERT INTO social.worn_history (address, urn, first_seen) VALUES ('legacy-wallet', 'urn:existing', '2026-01-01');
         DROP TABLE social.first_wear_outbox;",
    ).execute(pool).await.unwrap();
    let before = legacy_facts(pool).await;
    let ledger_before: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();

    let done = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert!(
        done.steps.is_empty(),
        "new objects require no migration steps"
    );
    assert!(
        done.created.contains(&schema_migrate::Object::Relation(
            "social.first_wear_outbox".into()
        )),
        "{done:?}"
    );
    let indexed: bool = sqlx::query_scalar(
        "SELECT to_regclass('social.first_wear_outbox_attempt_idx') IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(indexed);
    assert_eq!(legacy_facts(pool).await, before);
    let prior_ledger_after: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(prior_ledger_after, ledger_before);
    // Exercise the migrated defaults rather than only checking object names.
    let scheduled: bool = sqlx::query_scalar(
        "INSERT INTO social.first_wear_outbox (address, urn, wearer)
         VALUES ('legacy-wallet', 'urn:existing', 'Wearer') RETURNING attempted_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(scheduled);
    let ledger_after: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();
    assert!(
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(legacy_facts(pool).await, before);
    let rerun_ledger: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps ORDER BY step")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(rerun_ledger, ledger_after);
    let preserved: i64 = sqlx::query_scalar("SELECT count(*) FROM social.first_wear_outbox")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(preserved, 1);
    db.close().await;
}
