//! Complete wallet topology for social consumers: Pulse islands (Genesis) united with
//! World room occupancy. A wallet is online while any fresh source holds it.
//! Pulse receipt age is local liveness, not proof of source age: the existing wire has no
//! source timestamp/incarnation. Deploy one Pulse publisher per node; snapshots replace,
//! never union publisher state. World occupancy is this replica's own complete LiveKit
//! observation ([`crate::presence`]), aged from its observation start.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::protocol::bus::{BusError, topology::decode_islands, wallet_token};

pub const TOPOLOGY_RECEIPT_TTL: Duration = Duration::from_secs(5);
/// World occupancy counts as long as `/live-data` and `/connected-world` answer from it.
pub const WORLD_OCCUPANCY_TTL: Duration = crate::presence::PUBLIC_TTL;

fn fresh(at: Option<Instant>, ttl: Duration) -> bool {
    at.is_some_and(|at| at.elapsed() < ttl)
}

#[derive(Clone, Debug)]
pub struct WalletTopology {
    /// Every wallet a fresh source holds, at snapshot time.
    pub wallets: Arc<BTreeSet<String>>,
    /// The last Pulse islands receipt.
    pub received_at: Option<Instant>,
    pub generation: u64,
    islands: Arc<BTreeSet<String>>,
    world: Arc<BTreeSet<String>>,
    world_observed_at: Option<Instant>,
}

impl WalletTopology {
    pub fn is_online(&self, wallet: &str) -> bool {
        let wallet = wallet.to_ascii_lowercase();
        (fresh(self.received_at, TOPOLOGY_RECEIPT_TTL) && self.islands.contains(&wallet))
            || (fresh(self.world_observed_at, WORLD_OCCUPANCY_TTL) && self.world.contains(&wallet))
    }

    /// The next instant a fresh source expires; `None` when none will.
    pub fn expires_at(&self) -> Option<Instant> {
        let now = Instant::now();
        [
            self.received_at.map(|at| at + TOPOLOGY_RECEIPT_TTL),
            self.world_observed_at.map(|at| at + WORLD_OCCUPANCY_TTL),
        ]
        .into_iter()
        .flatten()
        .filter(|at| *at > now)
        .min()
    }

    fn live(&self) -> (bool, bool) {
        (
            fresh(self.received_at, TOPOLOGY_RECEIPT_TTL) && !self.islands.is_empty(),
            fresh(self.world_observed_at, WORLD_OCCUPANCY_TTL) && !self.world.is_empty(),
        )
    }

    fn union(&self, (islands, world): (bool, bool)) -> Arc<BTreeSet<String>> {
        match (islands, world) {
            (false, false) => Arc::default(),
            (true, false) => self.islands.clone(),
            (false, true) => self.world.clone(),
            (true, true) => Arc::new(self.islands.union(&self.world).cloned().collect()),
        }
    }
}

pub struct Topology {
    current: Mutex<(WalletTopology, (bool, bool))>,
    changed: watch::Sender<u64>,
}

impl Default for Topology {
    fn default() -> Self {
        Self {
            current: Mutex::new((
                WalletTopology {
                    wallets: Arc::default(),
                    received_at: None,
                    generation: 0,
                    islands: Arc::default(),
                    world: Arc::default(),
                    world_observed_at: None,
                },
                (false, false),
            )),
            changed: watch::channel(0).0,
        }
    }
}

impl Topology {
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// Recomputes the union when a source changed or crossed its expiry; bumps the
    /// generation only when the effective wallet set changed.
    fn settle(&self, current: &mut (WalletTopology, (bool, bool)), source_changed: bool) {
        let (view, cached) = current;
        let live = view.live();
        if !source_changed && live == *cached {
            return;
        }
        *cached = live;
        let wallets = view.union(live);
        if wallets != view.wallets {
            view.wallets = wallets;
            view.generation += 1;
            self.changed.send_replace(view.generation);
        } else {
            view.wallets = wallets;
        }
    }

    pub fn receive(&self, payload: &[u8]) -> Result<(), BusError> {
        let received_at = Instant::now();
        let message = decode_islands(payload)?;
        let islands = Arc::new(
            message
                .data
                .into_iter()
                .flat_map(|island| island.peers)
                .filter_map(|peer| wallet_token(&peer))
                .collect::<BTreeSet<_>>(),
        );
        if received_at.elapsed() >= TOPOLOGY_RECEIPT_TTL {
            return Ok(());
        }
        let mut current = self.current.lock().unwrap();
        if current.0.received_at.is_some_and(|at| at > received_at) {
            return Ok(());
        }
        // Expiry is an effective transition even when the next frame repeats the set.
        self.settle(&mut current, false);
        current.0.islands = islands;
        current.0.received_at = Some(received_at);
        self.settle(&mut current, true);
        Ok(())
    }

    /// Replaces the World occupancy with one complete observation started at `observed_at`.
    /// An observation older than the current one is ignored.
    pub fn observe_world<'a>(
        &self,
        wallets: impl IntoIterator<Item = &'a str>,
        observed_at: Instant,
    ) {
        let world = Arc::new(wallets.into_iter().filter_map(wallet_token).collect());
        let mut current = self.current.lock().unwrap();
        if current
            .0
            .world_observed_at
            .is_some_and(|at| at > observed_at)
        {
            return;
        }
        self.settle(&mut current, false);
        current.0.world = world;
        current.0.world_observed_at = Some(observed_at);
        self.settle(&mut current, true);
    }

    /// Tests: `wallets` online for the test's length, with no receipt to expire on a
    /// slow run. A later frame older than this hold is ignored, as any stale frame is.
    #[cfg(any(test, feature = "test-support"))]
    pub fn hold_for_tests<'a>(&self, wallets: impl IntoIterator<Item = &'a str>) {
        let mut current = self.current.lock().unwrap();
        current.0.islands = Arc::new(wallets.into_iter().filter_map(wallet_token).collect());
        current.0.received_at = Some(Instant::now() + Duration::from_secs(24 * 60 * 60));
        current.0.generation += 1;
        self.settle(&mut current, true);
        self.changed.send_replace(current.0.generation);
    }

    pub fn snapshot(&self) -> WalletTopology {
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false);
        current.0.clone()
    }

    /// Hold the same short lock as replacement through synchronous queue publication.
    /// A queued online update cannot pass a newer offline replacement.
    pub fn deliver_if_current(
        &self,
        generation: u64,
        wallet: &str,
        online: bool,
        deliver: impl FnOnce(),
    ) -> bool {
        let current = self.current.lock().unwrap();
        if current.0.generation != generation || current.0.is_online(wallet) != online {
            return false;
        }
        deliver();
        true
    }

    /// Unrelated wallet changes need not erase an already queued, still-true update.
    /// Compare its generation and current status again at queue consumption.
    pub fn queued_status_is_current(&self, generation: u64, wallet: &str, online: bool) -> bool {
        let current = self.current.lock().unwrap();
        generation <= current.0.generation && current.0.is_online(wallet) == online
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::pulse::decentraland::kernel::comms::v3::{
        IslandData, IslandStatusMessage,
    };
    use prost::Message;
    fn frame(peers: &[&str]) -> Vec<u8> {
        IslandStatusMessage {
            data: vec![IslandData {
                peers: peers.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }],
        }
        .encode_to_vec()
    }
    const A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[tokio::test]
    async fn initial_friend_remains_eligible_when_unrelated_topology_changes_during_profile_wait() {
        let state = Arc::new(Topology::default());
        state.receive(&frame(&[A])).unwrap();
        let initial = state.snapshot();
        let (resume, pending_profile) = tokio::sync::oneshot::channel();
        let reader = state.clone();
        let pending = tokio::spawn(async move {
            pending_profile.await.unwrap();
            reader.queued_status_is_current(initial.generation, A, true)
        });
        state
            .receive(&frame(&[A, "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]))
            .unwrap();
        resume.send(()).unwrap();
        assert!(
            pending.await.unwrap(),
            "unrelated wallet changes cannot discard stable initial ONLINE"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn complete_replacement_invalid_retention_and_quiet_expiry() {
        let state = Topology::default();
        state
            .receive(&frame(&[A, &A.to_uppercase(), "not-a-wallet"]))
            .unwrap();
        let first = state.snapshot();
        assert_eq!(first.wallets.len(), 1);
        assert!(first.is_online(A));
        state.receive(&frame(&[A])).unwrap();
        assert_eq!(state.snapshot().generation, first.generation);
        let receipt = state.snapshot().received_at;
        assert!(state.receive(&[0xff]).is_err());
        assert!(
            state
                .receive(&vec![
                    0;
                    crate::protocol::bus::topology::MAX_ISLANDS_BYTES + 1
                ])
                .is_err()
        );
        assert_eq!(state.snapshot().received_at, receipt);
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL).await;
        assert!(!state.snapshot().is_online(A));
        assert!(state.receive(&[0xff]).is_err());
        assert!(
            !state.snapshot().is_online(A),
            "invalid data cannot revive an expired receipt"
        );
        assert!(!state.deliver_if_current(first.generation, A, true, || panic!("stale delivery")));
        state.receive(&frame(&[A])).unwrap();
        assert!(state.snapshot().is_online(A));
        let online_generation = state.snapshot().generation;
        state
            .receive(&frame(&["0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]))
            .unwrap();
        assert!(
            !state.snapshot().is_online(A),
            "valid nonempty omission clears immediately"
        );
        assert!(!state.queued_status_is_current(online_generation, A, true));
        state.receive(&[]).unwrap();
        assert!(state.snapshot().wallets.is_empty());
        assert!(state.snapshot().received_at.is_some());
    }

    const B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[tokio::test(start_paused = true)]
    async fn wallet_in_an_island_and_a_world_is_online_once_until_it_is_in_neither() {
        let state = Topology::default();
        state.receive(&frame(&[A])).unwrap();
        state.observe_world([A, B], Instant::now());
        let both = state.snapshot();
        assert_eq!(both.wallets.iter().collect::<Vec<_>>(), [A, B]);
        state.observe_world([B], Instant::now());
        let island_only = state.snapshot();
        assert!(island_only.is_online(A));
        assert_eq!(
            island_only.generation, both.generation,
            "leaving the World while still in an island is no transition"
        );
        state.observe_world([A, B], Instant::now());
        state.receive(&frame(&[])).unwrap();
        assert!(state.snapshot().is_online(A), "still inside the World");
        assert_eq!(state.snapshot().generation, both.generation);
        state.observe_world([B], Instant::now());
        let gone = state.snapshot();
        assert!(!gone.is_online(A));
        assert!(gone.generation > both.generation);
        assert!(!state.queued_status_is_current(both.generation, A, true));
    }

    #[tokio::test(start_paused = true)]
    async fn each_source_expires_on_its_own_clock() {
        let state = Topology::default();
        state.observe_world([&A.to_uppercase(), "authoritative-server"], Instant::now());
        let world = state.snapshot();
        assert_eq!(world.wallets.iter().collect::<Vec<_>>(), [A]);
        // A quiet Pulse never drops World-only wallets.
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL * 2).await;
        assert!(state.snapshot().is_online(A));
        assert_eq!(
            state.snapshot().expires_at(),
            world.world_observed_at.map(|at| at + WORLD_OCCUPANCY_TTL)
        );
        // A stale occupancy never keeps them online, however lively Pulse is.
        state.receive(&frame(&[B])).unwrap();
        tokio::time::advance(WORLD_OCCUPANCY_TTL).await;
        state.receive(&frame(&[B])).unwrap();
        let stale = state.snapshot();
        assert!(!stale.is_online(A));
        assert!(stale.is_online(B));
        assert_eq!(stale.wallets.iter().collect::<Vec<_>>(), [B]);
        assert_eq!(
            stale.expires_at(),
            stale.received_at.map(|at| at + TOPOLOGY_RECEIPT_TTL)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_older_world_observation_never_replaces_a_newer_one() {
        let state = Topology::default();
        let older = Instant::now();
        tokio::time::advance(Duration::from_secs(1)).await;
        state.observe_world([B], Instant::now());
        state.observe_world([A], older);
        assert!(!state.snapshot().is_online(A));
        assert!(state.snapshot().is_online(B));
    }

    #[tokio::test]
    async fn a_world_observation_wakes_listeners() {
        let state = Topology::default();
        let mut changes = state.changes();
        state.observe_world([A], Instant::now());
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();
        state.observe_world([A], Instant::now());
        assert!(!changes.has_changed().unwrap(), "same set, no transition");
    }
}
