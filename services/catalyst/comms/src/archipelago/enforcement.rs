use crate::archipelago::ban::BanChecker;
use crate::archipelago::config::ClusterConfig;
use crate::archipelago::registry::PeersRegistry;
use crate::gatekeeper::SharedState;
use crate::jobs::{Jobs, PeriodicCfg};

use dashmap::DashMap;
use futures::stream::StreamExt;

use std::sync::Arc;
use std::time::Duration;

const BAN_SWEEP_CONCURRENCY: usize = 20;

/// Wallet room assignments used only for moderation and participant removal.
/// A pre-heartbeat assignment survives until the last socket leaves.
pub struct Enforcement {
    assignments: DashMap<String, String>,
    cfg: ClusterConfig,
    /// The gatekeeper, whose LiveKit a kick removes the wallet's island participant through.
    gatekeeper: SharedState,
    ban_checker: Arc<BanChecker>,
    registry: Arc<PeersRegistry>,
}

impl Enforcement {
    pub fn new(
        cfg: ClusterConfig,
        gatekeeper: SharedState,
        ban_checker: Arc<BanChecker>,
        registry: Arc<PeersRegistry>,
    ) -> Arc<Self> {
        Arc::new(Self {
            assignments: DashMap::new(),
            cfg,
            gatekeeper,
            ban_checker,
            registry,
        })
    }

    /// Records what the feed told this wallet to join. Always recorded, even for a wallet whose
    /// first heartbeat has not arrived: the ban sweep needs the issued room for participant
    /// removal, and an unchanged assignment may never be repeated.
    pub fn set_island(&self, address: &str, island_id: &str) {
        self.assignments
            .insert(address.to_string(), island_id.to_string());
    }

    /// The live socket registry bounds assignments; disconnect and kick retire them.
    pub fn remove_peer(&self, address: &str) {
        self.assignments.remove(address);
    }

    /// Ends every session of a wallet on this replica and clears its retained room. The
    /// LiveKit participant is removed too, so a kick reaches the media plane and not only the
    /// signalling socket.
    pub fn kick_peer(&self, address: &str, reason: &str) {
        let room = self.island_of(address);
        tracing::info!(addr = %address, reason, "kicking peer");
        self.registry.kick(address);
        self.remove_peer(address);
        if let (Some(room), true) = (room, self.gatekeeper.connector_livekit_armed()) {
            let gatekeeper = Arc::clone(&self.gatekeeper);
            let addr = address.to_string();
            tokio::spawn(async move {
                if !gatekeeper.kick_island_participant(&room, &addr).await {
                    tracing::error!("island ban enforcement request was not persisted");
                }
            });
        }
    }

    pub async fn ban_sweep_once(&self) {
        let addrs = self.registry.wallets();
        if addrs.is_empty() {
            return;
        }
        let checker = Arc::clone(&self.ban_checker);
        let banned = futures::stream::iter(addrs)
            .map(|addr| {
                let checker = Arc::clone(&checker);
                async move { checker.is_banned_fresh(&addr).await.then_some(addr) }
            })
            .buffer_unordered(BAN_SWEEP_CONCURRENCY)
            .filter_map(|hit| async move { hit });
        futures::pin_mut!(banned);
        while let Some(addr) = banned.next().await {
            tracing::info!(addr = %addr, "ban sweep: evicting banned peer from comms");
            self.kick_peer(&addr, "banned");
        }
    }

    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let this = Arc::clone(self);
        let period = Duration::from_secs(self.cfg.ban_sweep_interval_secs.max(1));
        jobs.spawn_periodic(
            "archipelago-ban-sweep",
            PeriodicCfg::every(period).after_first_interval(),
            move |_ctx| {
                let this = this.clone();
                async move {
                    this.ban_sweep_once().await;
                    Ok(())
                }
            },
        );
    }

    pub fn island_of(&self, address: &str) -> Option<String> {
        self.assignments.get(address).map(|v| v.value().clone())
    }
}
