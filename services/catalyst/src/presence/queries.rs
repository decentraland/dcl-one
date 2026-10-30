//! The read side: the latest snapshot with its scene and world rows, memoized
//! for [`CURRENT_TTL`] and refreshed by the collector on commit (the TTL
//! covers a node whose snapshots are written by another process), and the
//! two per-key histories, newest first, bounded by the clamped limit.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::{CurrentSnapshot, SceneOccupancyRow, WorldHeadcountRow};

pub const CURRENT_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default)]
pub struct CurrentBundle {
    pub current: Option<CurrentSnapshot>,
    pub scenes: Vec<SceneOccupancyRow>,
    pub worlds: Vec<WorldHeadcountRow>,
}

pub struct Queries {
    pool: PgPool,
    current: Mutex<Option<(Instant, Arc<CurrentBundle>)>>,
}

impl Queries {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            current: Mutex::new(None),
        }
    }

    /// The memoized latest snapshot bundle; concurrent misses coalesce onto one load.
    pub async fn current_bundle(&self) -> Result<Arc<CurrentBundle>, sqlx::Error> {
        let mut slot = self.current.lock().await;
        if let Some((at, bundle)) = slot.as_ref()
            && at.elapsed() < CURRENT_TTL
        {
            return Ok(bundle.clone());
        }
        let bundle = Arc::new(self.load_current_bundle().await?);
        *slot = Some((Instant::now(), bundle.clone()));
        Ok(bundle)
    }

    pub async fn refresh_current(&self) -> Result<(), sqlx::Error> {
        let bundle = Arc::new(self.load_current_bundle().await?);
        *self.current.lock().await = Some((Instant::now(), bundle));
        Ok(())
    }

    async fn load_current_bundle(&self) -> Result<CurrentBundle, sqlx::Error> {
        let (current, scenes, worlds) =
            tokio::try_join!(self.current(), self.current_scenes(), self.current_worlds())?;
        Ok(CurrentBundle {
            current,
            scenes,
            worlds,
        })
    }

    /// `snapshots_taken_at_idx`.
    pub async fn current(&self) -> Result<Option<CurrentSnapshot>, sqlx::Error> {
        sqlx::query_as(
            "SELECT id, taken_at, peers_count, islands_count, hot_scenes_count, \
                    scenes_polled, scene_users_total, worlds_polled, active_worlds, \
                    world_users_total, worlds_live_total \
             FROM presence.snapshots ORDER BY taken_at DESC, id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// `scene_occupancy_pointer_idx`.
    pub async fn scene_history(
        &self,
        pointer: &str,
        limit: i64,
    ) -> Result<Vec<SceneOccupancyRow>, sqlx::Error> {
        sqlx::query_as(
            "SELECT taken_at, pointer, scene_name, realm, count \
             FROM presence.scene_occupancy WHERE pointer = $1 \
             ORDER BY taken_at DESC LIMIT $2",
        )
        .bind(pointer)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// The latest snapshot's rows by PK prefix, busiest first.
    pub async fn current_scenes(&self) -> Result<Vec<SceneOccupancyRow>, sqlx::Error> {
        sqlx::query_as(
            "SELECT taken_at, pointer, scene_name, realm, count \
             FROM presence.scene_occupancy \
             WHERE snapshot_id = (SELECT id FROM presence.snapshots ORDER BY taken_at DESC, id DESC LIMIT 1) \
             ORDER BY count DESC, pointer",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// `world_membership_world_idx`.
    pub async fn world_history(
        &self,
        world: &str,
        limit: i64,
    ) -> Result<Vec<WorldHeadcountRow>, sqlx::Error> {
        sqlx::query_as(
            "SELECT taken_at, world_name, count, live_users \
             FROM presence.world_membership WHERE world_name = $1 \
             ORDER BY taken_at DESC LIMIT $2",
        )
        .bind(world)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn current_worlds(&self) -> Result<Vec<WorldHeadcountRow>, sqlx::Error> {
        sqlx::query_as(
            "SELECT taken_at, world_name, count, live_users \
             FROM presence.world_membership \
             WHERE snapshot_id = (SELECT id FROM presence.snapshots ORDER BY taken_at DESC, id DESC LIMIT 1) \
             ORDER BY count DESC, world_name",
        )
        .fetch_all(&self.pool)
        .await
    }
}
