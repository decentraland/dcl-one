//! The sync leader's status, shared through the database with every other
//! process of the fleet. The orchestrator already persists the frontier in
//! `content.system_properties` (`sync_frontier`, [`super::repository`]) and
//! beats `sync_heartbeat` between bootstrap phases and from the
//! pointer-changes poller; the leader's publisher adds `sync_state` (the
//! [`SyncState`] wire form, [`SyncState::to_property`]) and `sync_holder`
//! (which process wrote it), and refreshes `sync_heartbeat` on every pass,
//! so the heartbeat means "the process holding the `sync` lease is alive"
//! through the hours a from-scratch bootstrap spends inside one snapshot
//! phase (on 2026-09-27 a fresh node showed `up=false` for the whole phase
//! and the release readiness wait, which needs `up`, would have timed out);
//! the frontier remains the progress signal. A process that does not hold
//! the lease pulls the four into its own [`SyncStatus`] every few seconds,
//! so /about, /status and the readiness sampler read the same answer from
//! any process behind nginx; a stale heartbeat still means the leader is
//! down, wherever it ran.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::jobs::{JobContext, Jobs, PeriodicCfg};
use crate::{SyncState, SyncStatus};

pub const STATE_KEY: &str = "sync_state";
pub const HOLDER_KEY: &str = "sync_holder";
pub const FRONTIER_KEY: &str = "sync_frontier";
pub const HEARTBEAT_KEY: &str = "sync_heartbeat";

/// How often a follower pulls and the leader publishes.
pub const INTERVAL: Duration = Duration::from_secs(5);

/// `<hostname>:<pid>`, what `sync_holder` records. `/etc/hostname` comes
/// first: the hardened unit (`ProcSubset=pid`) hides `/proc/sys`, and
/// systemd does not export `HOSTNAME`.
pub fn holder_id() -> String {
    let host = ["/etc/hostname", "/proc/sys/kernel/hostname"]
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|h| h.trim().to_string())
        .find(|h| !h.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "unknown-host".to_string());
    format!("{host}:{}", std::process::id())
}

/// Writes this process's state as the fleet's (`sync_state`, `sync_holder`).
pub async fn publish(pool: &PgPool, status: &SyncStatus, holder: &str) -> sqlx::Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO content.system_properties (key, value) VALUES ($1, $2), ($3, $4), ($5, $6)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(STATE_KEY)
    .bind(status.state().to_property())
    .bind(HOLDER_KEY)
    .bind(holder)
    .bind(HEARTBEAT_KEY)
    .bind(now_ms.to_string())
    .execute(pool)
    .await?;
    status.set_heartbeat_ms(now_ms);
    Ok(())
}

/// What the database says about the fleet's sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub state: SyncState,
    pub holder: Option<String>,
    pub frontier_ms: Option<i64>,
    pub heartbeat_ms: Option<i64>,
}

/// `None` until a leader has published a state.
pub async fn read(pool: &PgPool) -> sqlx::Result<Option<Published>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM content.system_properties WHERE key = ANY($1)")
            .bind([STATE_KEY, HOLDER_KEY, FRONTIER_KEY, HEARTBEAT_KEY])
            .fetch_all(pool)
            .await?;
    let get = |k: &str| {
        rows.iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    let Some(state) = get(STATE_KEY).and_then(SyncState::from_property) else {
        return Ok(None);
    };
    Ok(Some(Published {
        state,
        holder: get(HOLDER_KEY).map(str::to_string),
        frontier_ms: get(FRONTIER_KEY).and_then(|v| v.parse().ok()),
        heartbeat_ms: get(HEARTBEAT_KEY).and_then(|v| v.parse().ok()),
    }))
}

/// One pull into `status`, unless this process is the authority. Returns
/// whether a published state existed.
pub async fn pull(pool: &PgPool, status: &SyncStatus) -> sqlx::Result<bool> {
    if status.is_authority() {
        return Ok(true);
    }
    let Some(published) = read(pool).await? else {
        return Ok(false);
    };
    if status.is_authority() {
        return Ok(true);
    }
    status.set_state(published.state);
    if let Some(frontier) = published.frontier_ms {
        status.set_sync_frontier_ms(frontier);
    }
    if let Some(heartbeat) = published.heartbeat_ms {
        status.set_heartbeat_ms(heartbeat);
    }
    Ok(true)
}

/// The per-process job `sync-status-mirror`: runs in every process and pulls
/// while the process is not the authority.
pub fn spawn_mirror(jobs: &Jobs, pool: PgPool, status: Arc<SyncStatus>) {
    let cfg = PeriodicCfg::every(INTERVAL).with_error_backoff(Duration::from_secs(60));
    jobs.spawn_periodic("sync-status-mirror", cfg, move |ctx| {
        let pool = pool.clone();
        let status = status.clone();
        async move {
            if status.is_authority() {
                ctx.progress("this process is the sync authority; nothing to mirror");
                return Ok(());
            }
            match pull(&pool, &status).await? {
                true => ctx.progress(format!("mirrored: {}", status.state_name())),
                false => ctx.progress("no sync leader has published a state yet"),
            }
            Ok(())
        }
    });
}

/// The leader's helper `sync-status-publish`, scoped to the leased sync job:
/// publishes right away and then every [`INTERVAL`].
pub fn spawn_publisher(jobs: &Jobs, parent: &JobContext, pool: PgPool, status: Arc<SyncStatus>) {
    let holder = holder_id();
    let cfg = PeriodicCfg::every(INTERVAL).with_error_backoff(Duration::from_secs(60));
    jobs.spawn_periodic_scoped(parent, "sync-status-publish", cfg, move |ctx| {
        let pool = pool.clone();
        let status = status.clone();
        let holder = holder.clone();
        async move {
            publish(&pool, &status, &holder).await?;
            ctx.progress(format!("published {} as {holder}", status.state_name()));
            Ok(())
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_property_form_round_trips_every_state() {
        for state in [
            SyncState::Bootstrapping,
            SyncState::Syncing,
            SyncState::Failed,
            SyncState::PartiallySynced {
                ready_types: vec!["profile".into(), "wearable".into()],
            },
            SyncState::PartiallySynced {
                ready_types: vec![],
            },
        ] {
            assert_eq!(SyncState::from_property(&state.to_property()), Some(state));
        }
        assert_eq!(SyncState::from_property("Nonsense"), None);
    }

    #[tokio::test]
    async fn a_follower_mirrors_what_the_leader_published_and_an_authority_ignores_it() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let leader = SyncStatus::default();
        leader.set_authority(true);
        leader.set_state(SyncState::PartiallySynced {
            ready_types: vec!["profile".into()],
        });
        let before = chrono::Utc::now().timestamp_millis();
        // The suite shares one database and other tests beat `sync_heartbeat`
        // and move `sync_frontier` too: retry until one publish/pull pair
        // sees only its own writes.
        let follower = SyncStatus::default();
        let mut agreed = false;
        for _ in 0..50 {
            sqlx::query(
                "INSERT INTO content.system_properties (key, value) VALUES ($1, '1700000000000')
                 ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(FRONTIER_KEY)
            .execute(&pool)
            .await
            .unwrap();
            publish(&pool, &leader, "test-host:1").await.unwrap();
            let beat = leader
                .heartbeat_ms()
                .expect("the publisher beats the leader's heartbeat");
            assert!(beat >= before, "{beat} < {before}");
            assert!(pull(&pool, &follower).await.unwrap());
            if follower.heartbeat_ms() == Some(beat)
                && follower.sync_frontier_ms() == Some(1_700_000_000_000)
            {
                agreed = true;
                break;
            }
        }
        assert!(
            agreed,
            "the follower never saw the leader's own heartbeat and frontier"
        );
        assert_eq!(follower.state_name(), "Syncing");
        assert!(follower.is_type_ready("profile"));
        assert!(!follower.is_type_ready("scene"));
        let published = read(&pool).await.unwrap().unwrap();
        assert_eq!(published.holder.as_deref(), Some("test-host:1"));

        let authority = SyncStatus::default();
        authority.set_authority(true);
        authority.set_state(SyncState::Failed);
        assert!(pull(&pool, &authority).await.unwrap());
        assert_eq!(authority.state_name(), "Failed");
    }
}
