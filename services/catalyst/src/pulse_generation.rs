//! Root-owned process fencing. The row stores no native or selected assignment state.
use crate::comms::interfaces::{PulseGeneration, PulseGenerationBinding};
use anyhow::Context;
use sqlx::{Connection, PgPool};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct HeldPulseGeneration {
    binding: PulseGenerationBinding,
    leases: Arc<crate::jobs::Leases>,
    guard: tokio::sync::Mutex<Option<crate::jobs::LeaseGuard>>,
    lost: CancellationToken,
}
impl PulseGeneration for HeldPulseGeneration {
    fn binding(&self) -> &PulseGenerationBinding {
        &self.binding
    }
    fn lost(&self) -> &CancellationToken {
        &self.lost
    }
}
impl HeldPulseGeneration {
    pub async fn acquire(pool: PgPool, audience: String) -> anyhow::Result<Arc<Self>> {
        anyhow::ensure!(
            !audience.is_empty() && audience.len() <= 256,
            "invalid Pulse deployment audience"
        );
        let leases = Arc::new(crate::jobs::Leases::new(
            pool,
            crate::jobs::LeaseSettings::default(),
        ));
        let guard = leases
            .try_acquire(&format!("pulse-generation:{audience}"))
            .await?
            .context("another Pulse generation holds this deployment lease")?;
        let source = uuid::Uuid::new_v4().to_string();
        let write_audience = audience.clone();
        let write_source = source.clone();
        let epoch = guard.with_connection(move |connection| Box::pin(async move {
            let mut tx = connection.begin().await?;
            sqlx::query("SET LOCAL statement_timeout='2000ms'").execute(&mut *tx).await?;
            sqlx::query("SET LOCAL lock_timeout='1500ms'").execute(&mut *tx).await?;
            // The election lock is held by this exact session throughout allocation/commit.
            let epoch: i64 = sqlx::query_scalar("INSERT INTO social.pulse_authority_generation(deployment_audience,epoch,source_incarnation) VALUES($1,nextval('social.archipelago_v4_connection_epoch'),$2::uuid) ON CONFLICT(deployment_audience) DO UPDATE SET epoch=EXCLUDED.epoch,source_incarnation=EXCLUDED.source_incarnation,updated_at=timezone('UTC',clock_timestamp()) WHERE social.pulse_authority_generation.epoch<EXCLUDED.epoch RETURNING epoch")
                .bind(write_audience).bind(write_source).fetch_one(&mut *tx).await?;
            if epoch <= 0 { return Err(sqlx::Error::RowNotFound); }
            tx.commit().await?;
            Ok(epoch as u64)
        })).await?;
        let lost = guard.lost().clone();
        Ok(Arc::new(Self {
            binding: PulseGenerationBinding {
                audience,
                epoch,
                source_incarnation: source,
            },
            leases,
            guard: tokio::sync::Mutex::new(Some(guard)),
            lost,
        }))
    }
    async fn keepalive(&self) {
        let mut interval = tokio::time::interval(self.leases.settings().keepalive);
        loop {
            interval.tick().await;
            self.leases.keepalive_once().await;
            if self.lost.is_cancelled() {
                return;
            }
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    pub async fn end_lease_backend_for_test(&self, admin: &PgPool) {
        let pid = self.leases.backend_pid().await.unwrap();
        sqlx::query("SELECT pg_terminate_backend($1)")
            .bind(pid)
            .execute(admin)
            .await
            .unwrap();
    }
    #[cfg(any(test, feature = "test-support"))]
    pub async fn observe_lease_loss_for_test(&self) {
        self.leases.keepalive_once().await;
    }
    pub async fn close(&self) {
        self.lost.cancel();
        if let Some(guard) = self.guard.lock().await.take() {
            guard.release().await;
        }
    }
}

pub async fn run_from_env() -> anyhow::Result<()> {
    let audience = std::env::var("ARCHIPELAGO_CONTROL_V4_AUDIENCE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("PULSE_V4_AUDIENCE")
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    let audience =
        audience.context("catalyst pulse requires a deployment audience for generation fencing")?;
    let database = std::env::var("DATABASE_URL")
        .context("configured Pulse requires DATABASE_URL for its generation lease")?;
    let pool = crate::db::connect_pool(
        &database,
        &crate::db::PoolSettings {
            max_connections: 2,
            ..Default::default()
        },
    )
    .await?;
    let authority = HeldPulseGeneration::acquire(pool.clone(), audience.trim().into()).await?;
    let pulse = crate::comms::pulse::env::run_from_env_with_generation(Some(authority.clone()));
    tokio::pin!(pulse);
    let result = tokio::select! {
        result = &mut pulse => result,
        _ = authority.keepalive() => pulse.await,
    };
    authority.close().await;
    drop(authority);
    pool.close().await;
    result
}
