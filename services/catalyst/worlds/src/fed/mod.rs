//! Worlds federation: the peer registry, and the types that keep a peer's word
//! separable from ours.
//!
//! A peer is a **source of content claims and nothing else**: which world names it
//! holds and what public metadata it prints for them. It may never tell us who owns a
//! world, who may deploy to one, or what a local ACL says. Every ownership and
//! permission question resolves through the local path -- `worlds::policy` over
//! `worlds.worlds`, `worlds.world_permissions` and the name authority -- which nothing
//! in this module is reachable from.
//!
//! [`peers`], [`registry`] and [`config`] are the admission gate: `federation-peers.toml`
//! is read at boot, every entry adjudicated (placeholders, reserved hosts, the pinned
//! root, https, a public host, not this node), and a bad entry aborts startup. [`names`]
//! holds the newtypes that make provenance a compile-time property.
//!
//! Admission is also **revocation**. A peer removed from the file stops being published
//! at the next boot, by two independent mechanisms -- one on the write path
//! ([`store::RemoteWorldsComponent::revoke_peers_no_longer_admitted`], in
//! [`FedState::boot`] before the router exists) and one on the read path
//! ([`store::RemoteWorldsComponent::list_mirror`] filtering against the admitted set).
//!
//! [`wire`], [`store`], [`poll`] and [`handlers`] are the read mirror: peer world names
//! and public metadata in `worlds.federation_worlds` keyed by `(peer_id, world_name)`,
//! served only on peer-qualified routes. No blobs, no `/about`, no comms, and zero
//! writes to `worlds.worlds`, `worlds.world_scenes` or `worlds.world_permissions`: a
//! mirrored world is distinguishable from a local one by living in a different table,
//! cannot overwrite a local signed publish (nothing here writes the local tables) and
//! grants nobody host authority (the room resolver and the permission core never read
//! this table).

pub mod config;
pub mod handlers;
pub mod names;
pub mod peers;
pub mod poll;
pub mod registry;
pub mod store;
pub mod wire;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post, put};
use sqlx::PgPool;

use crate::jobs::{Jobs, PeriodicCfg};

pub use config::WorldsFedConfig;
pub use peers::WorldsFederationPeers;
pub use poll::WorldsMirror;

/// The federation state for the lifetime of this process: the adjudicated peer set
/// (by value, behind the `Arc`: no `Mutex`, `RwLock` or reload path, so a peer cannot
/// be de-admitted while a poll is in flight -- de-admission is editing the file and
/// restarting), the mirror store/poller, and the admin bearer.
pub struct FedState {
    pub peers: WorldsFederationPeers,
    pub mirror: WorldsMirror,
    /// `CATALYRST_WORLDS_ADMIN_TOKEN`: unset refuses the two admin routes.
    admin_token: Option<String>,
}

impl FedState {
    /// Call this from `serve` **before** the router is built: it loads and adjudicates
    /// the peer file (a refusal aborts startup rather than degrading a serving
    /// process) and reconciles the mirror against the admitted set, so no request is
    /// ever answered from rows belonging to a peer that has left the file.
    pub async fn boot(
        pool: PgPool,
        cfg: WorldsFedConfig,
        admin_token: Option<String>,
    ) -> Result<Arc<Self>> {
        let peers = WorldsFederationPeers::load(&cfg)?;
        let mirror = WorldsMirror::new(pool, cfg, &peers);
        mirror
            .store()
            .revoke_peers_no_longer_admitted(&peers, chrono::Utc::now().naive_utc())
            .await
            .context(
                "failed to reconcile the worlds mirror against the admitted peer set; refusing to \
                 start rather than serve worlds for a peer that may have been de-admitted",
            )?;
        Ok(Arc::new(Self {
            peers,
            mirror,
            admin_token: admin_token.filter(|t| !t.is_empty()),
        }))
    }

    /// Tests: an already adjudicated peer set, no boot sweep.
    pub fn for_tests(
        pool: PgPool,
        cfg: WorldsFedConfig,
        peers: WorldsFederationPeers,
        admin_token: Option<String>,
    ) -> Arc<Self> {
        let mirror = WorldsMirror::new(pool, cfg, &peers);
        Arc::new(Self {
            peers,
            mirror,
            admin_token,
        })
    }

    /// The background loop, job `worlds-federation-poller`: one pass polls every
    /// admitted peer (`WORLDS_FED_POLL_INTERVAL_SECS` between passes, sleep after work).
    /// Nothing is spawned when federation is off or the file admits no worlds peer. A
    /// pass never fails as a whole -- one peer's failure is that peer's stale status
    /// row -- so the job's trace carries the per-peer tally.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        if !self.peers.is_configured() || self.peers.peers().is_empty() {
            return;
        }
        let state = self.clone();
        let cfg = PeriodicCfg::every(Duration::from_secs(self.mirror.config().poll_interval_secs));
        jobs.spawn_periodic_leased("worlds-federation-poller", cfg, move |ctx| {
            let state = state.clone();
            async move {
                let results = state.mirror.poll_all(&state.peers).await;
                let (mut ok, mut failed) = (0usize, 0usize);
                for (peer, outcome) in &results {
                    match outcome {
                        poll::PollOutcome::Polled(r) => {
                            ok += 1;
                            tracing::info!(
                                peer = %peer,
                                worlds = r.worlds_observed,
                                skipped = r.entries_skipped,
                                truncated = r.truncated,
                                collisions = %r.collisions,
                                "worlds mirror refreshed"
                            );
                        }
                        poll::PollOutcome::Failed(e) => {
                            failed += 1;
                            ctx.error(format!("{peer}: {e}"));
                            tracing::warn!(peer = %peer, error = %e, "worlds mirror poll failed");
                        }
                    }
                }
                ctx.progress(format!("{ok} peers refreshed, {failed} failed"));
                Ok(())
            }
        });
    }
}

/// The four federation routes at their old root paths (the old explore gateway merged
/// the worlds router at the root beside `/federation/places/*` and
/// `/federation/communities/*`).
pub fn router(state: Arc<FedState>) -> Router {
    Router::new()
        .route(
            "/federation/worlds/peers",
            get(handlers::get_federation_peers),
        )
        .route(
            "/federation/worlds/mirror",
            get(handlers::get_federation_mirror),
        )
        .route(
            "/admin/federation/worlds/refresh",
            post(handlers::refresh_federation_mirror),
        )
        .route(
            "/admin/federation/worlds/{peer_id}/{world_name}/hidden",
            put(handlers::set_mirror_world_hidden),
        )
        .with_state(state)
}
