//! A database of a test's own, for tests that the shared scratch database
//! cannot isolate: statements that walk EVERY wallet (the credits
//! realignment) would adjust the half-seeded wallets of the tests running
//! beside them, and the mirrors' completeness rows (`shop.mirror_state`) are
//! deleted and reset by other tests. Dropped at the end.

// Money and chain values: a silently truncated or sign-flipped cast is a bug.
#![deny(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

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
        crate::db::scratch::create_database_with_schema(&admin, &name).await;
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

/// Published mainnet LAND ownership for a signed-listing fixture.
pub(crate) async fn publish_land_owner(pool: &PgPool, registry: &str, token: &str, owner: &str) {
    let net = crate::land_authz::LAND_MAINNET;
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry,
             last_block, folded_block, folded_at, full_replay_started)
         VALUES ($1,$2,$3,100,100,timezone('UTC',now()),true) ON CONFLICT DO NOTHING",
    )
    .bind(net.network)
    .bind(net.land_registry)
    .bind(net.estate_registry)
    .execute(pool)
    .await
    .expect("publish test LAND ownership");
    sqlx::query(
        "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log)
         VALUES ($1,$2::numeric,$3,1,0) ON CONFLICT DO NOTHING",
    )
    .bind(registry)
    .bind(token)
    .bind(owner)
    .execute(pool)
    .await
    .expect("seed test LAND owner");
}

/// The folded open verdict for a listing fixture. The route tests decide
/// separately when the trade-events cursor publishes.
pub(crate) async fn open_trade_verdict(pool: &PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block, digest)
         SELECT id,lower(network),'open',0,coalesce(trade_digest, hashed_signature)
         FROM shop.trades WHERE id=$1::uuid
         ON CONFLICT (trade_id) DO UPDATE SET status='open', executions=0,
             digest=EXCLUDED.digest, digest_failed_at=NULL, folded_block=0",
    )
    .bind(id)
    .execute(pool)
    .await
    .expect("seed test open verdict");
}
