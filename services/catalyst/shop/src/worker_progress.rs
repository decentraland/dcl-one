//! Durable, bounded sweeps of obligations which may stay pending forever.
//! A sweep freezes an identity ceiling or creation cutoff as well as its
//! upper key, so new arrivals cannot indefinitely postpone retries. Selection and progress commit together, before external work. A
//! crash can defer selected work until the next sweep, never remove it.

use sqlx::{PgPool, Postgres, Row, Transaction};

pub(crate) struct Sweep<'a> {
    pub tx: Transaction<'a, Postgres>,
    worker: &'static str,
    scope: i64,
    pub started_at: chrono::NaiveDateTime,
    pub position: i64,
    pub ceiling: i64,
    pub key_position: Vec<String>,
    pub key_ceiling: Vec<String>,
}

impl<'a> Sweep<'a> {
    pub async fn begin(pool: &'a PgPool, worker: &'static str, scope: i64) -> sqlx::Result<Self> {
        let mut tx = pool.begin().await?;
        sqlx::query("INSERT INTO shop.worker_progress (worker, scope) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(worker).bind(scope).execute(&mut *tx).await?;
        let r = sqlx::query("SELECT started_at, position, ceiling, key_position, key_ceiling FROM shop.worker_progress WHERE worker = $1 AND scope = $2 FOR UPDATE")
            .bind(worker).bind(scope).fetch_one(&mut *tx).await?;
        Ok(Self {
            tx,
            worker,
            scope,
            started_at: r.get("started_at"),
            position: r.get("position"),
            ceiling: r.get("ceiling"),
            key_position: r.get("key_position"),
            key_ceiling: r.get("key_ceiling"),
        })
    }

    pub async fn commit(mut self) -> sqlx::Result<()> {
        sqlx::query("UPDATE shop.worker_progress SET position = $3, ceiling = $4, key_position = $5, key_ceiling = $6, started_at = $7 WHERE worker = $1 AND scope = $2")
            .bind(self.worker).bind(self.scope).bind(self.position).bind(self.ceiling)
            .bind(self.key_position).bind(self.key_ceiling).bind(self.started_at).execute(&mut *self.tx).await?;
        self.tx.commit().await
    }
}
