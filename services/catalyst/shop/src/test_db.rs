//! A database of a test's own, for tests that the shared scratch database
//! cannot isolate: statements that walk EVERY wallet (the credits
//! realignment) would adjust the half-seeded wallets of the tests running
//! beside them, and the mirrors' completeness rows (`shop.mirror_state`) are
//! deleted and reset by other tests. Dropped at the end.

use sqlx::PgPool;

pub(crate) struct OwnDatabase {
    pub(crate) pool: PgPool,
    admin: PgPool,
    name: String,
}

impl OwnDatabase {
    pub(crate) async fn create(label: &str) -> Option<Self> {
        let url = crate::db::scratch::url().await?;
        let opts: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_test_lib_{}_{label}_{}",
            std::process::id(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut own = url::Url::parse(&url).unwrap();
        own.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            own.as_str(),
            &crate::db::PoolSettings {
                max_connections: 4,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        Some(Self { pool, admin, name })
    }

    pub(crate) async fn drop(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}
