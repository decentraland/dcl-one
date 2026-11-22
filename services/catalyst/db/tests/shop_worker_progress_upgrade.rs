//! Upgrade regression for durable shop worker progress.
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
                panic!("worker progress upgrade requires CATALYST_TEST_DATABASE_URL in CI")
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

async fn ledger(pool: &PgPool) -> Vec<(String, String, bool)> {
    sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps ORDER BY step")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn shop_worker_progress_upgrade_preserves_ledger_and_is_idempotent() {
    let Some(db) = OwnedDatabase::new().await else {
        return;
    };
    let pool = &db.pool;
    sqlx::query("DROP TABLE shop.worker_progress")
        .execute(pool)
        .await
        .unwrap();
    let before = ledger(pool).await;
    let done = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert!(done.steps.is_empty());
    assert!(done.created.contains(&schema_migrate::Object::Relation(
        "shop.worker_progress".into()
    )));
    assert_eq!(ledger(pool).await, before);
    sqlx::query("INSERT INTO shop.worker_progress (worker, scope) VALUES ('upgrade-test', 1)")
        .execute(pool)
        .await
        .unwrap();
    let defaults: (i64, i64, Vec<String>, Vec<String>) = sqlx::query_as(
        "SELECT position, ceiling, key_position, key_ceiling FROM shop.worker_progress WHERE worker = 'upgrade-test' AND scope = 1"
    ).fetch_one(pool).await.unwrap();
    assert_eq!(defaults, (0, 0, vec![], vec![]));
    sqlx::query("UPDATE shop.worker_progress SET position = 7, ceiling = 10 WHERE worker = 'upgrade-test' AND scope = 1")
        .execute(pool).await.unwrap();
    assert!(
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(ledger(pool).await, before);
    let preserved: (i64, i64) = sqlx::query_as(
        "SELECT position, ceiling FROM shop.worker_progress WHERE worker = 'upgrade-test' AND scope = 1"
    ).fetch_one(pool).await.unwrap();
    assert_eq!(preserved, (7, 10));
    db.close().await;
}
