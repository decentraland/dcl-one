//! Complete wallet topology for social consumers: Pulse islands (Genesis) united with
//! World room presence. A wallet is online while either source holds it.
//! Pulse receipt age is local liveness, not proof of source age: the existing wire has no
//! source timestamp/incarnation. Deploy one Pulse publisher per node; snapshots replace,
//! never union publisher state. World presence applies every replica's LiveKit room
//! events as they arrive; this replica's complete LiveKit reads correct missed ones.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::protocol::bus::{BusError, topology::decode_islands, wallet_token};

pub const TOPOLOGY_RECEIPT_TTL: Duration = Duration::from_secs(5);
/// A World session no event or complete read confirmed for this long stops counting.
pub const WORLD_OCCUPANCY_TTL: Duration = crate::presence::PUBLIC_TTL;
/// How long a left session is remembered, so a delayed join cannot revive it.
const WORLD_TOMBSTONE_TTL: Duration = Duration::from_secs(300);
/// Sessions held at once; events past it are dropped until reads or expiry free room.
const MAX_WORLD_SESSIONS: usize = 200_000;

fn fresh(at: Option<Instant>, ttl: Duration) -> bool {
    at.is_some_and(|at| at.elapsed() < ttl)
}

/// One LiveKit participant session (keyed by its sid) in a World room. A sid joins at
/// most once, so its leave is terminal whatever order the two arrive in.
struct WorldSession {
    wallet: String,
    present: bool,
    /// The last event or complete read that confirmed this state.
    confirmed: Instant,
    /// When an event last changed it; a read started before then cannot override it.
    changed: Instant,
}

#[derive(Default)]
struct WorldSessions {
    sessions: HashMap<String, WorldSession>,
}

impl WorldSessions {
    fn apply(&mut self, sid: &str, wallet: String, joined: bool, now: Instant) -> bool {
        match self.sessions.get_mut(sid) {
            Some(session) if !session.present => false,
            Some(session) if joined => {
                session.confirmed = now;
                false
            }
            Some(session) => {
                session.present = false;
                session.confirmed = now;
                session.changed = now;
                true
            }
            None => {
                if self.sessions.len() >= MAX_WORLD_SESSIONS {
                    self.collect(now);
                    if self.sessions.len() >= MAX_WORLD_SESSIONS {
                        tracing::warn!("World presence session cap reached; event dropped");
                        return false;
                    }
                }
                let session = WorldSession {
                    wallet,
                    present: joined,
                    confirmed: now,
                    changed: now,
                };
                self.sessions.insert(sid.to_owned(), session);
                joined
            }
        }
    }

    /// One complete read of every World room, started at `started`: confirms what it saw
    /// and ends what it missed, except sessions an event changed after it started.
    fn observe(&mut self, started: Instant, seen: Vec<(String, String)>, now: Instant) {
        let mut listed = HashSet::with_capacity(seen.len());
        for (sid, wallet) in seen {
            match self.sessions.get_mut(&sid) {
                Some(session) if !session.present => {}
                Some(session) => session.confirmed = session.confirmed.max(started),
                None => {
                    if self.sessions.len() < MAX_WORLD_SESSIONS {
                        let session = WorldSession {
                            wallet,
                            present: true,
                            confirmed: started,
                            changed: started,
                        };
                        self.sessions.insert(sid.clone(), session);
                    }
                }
            }
            listed.insert(sid);
        }
        for (sid, session) in &mut self.sessions {
            if session.present && session.changed <= started && !listed.contains(sid) {
                session.present = false;
                session.confirmed = now;
                session.changed = now;
            }
        }
        self.collect(now);
    }

    fn collect(&mut self, now: Instant) {
        self.sessions.retain(|_, s| {
            let ttl = if s.present {
                WORLD_OCCUPANCY_TTL
            } else {
                WORLD_TOMBSTONE_TTL
            };
            now.saturating_duration_since(s.confirmed) < ttl
        });
    }

    /// The wallets of live sessions and the next instant one of them expires.
    fn wallets(&self, now: Instant) -> (BTreeSet<String>, Option<Instant>) {
        let mut wallets = BTreeSet::new();
        let mut expires = None::<Instant>;
        for session in self.sessions.values().filter(|s| s.present) {
            let until = session.confirmed + WORLD_OCCUPANCY_TTL;
            if until > now {
                wallets.insert(session.wallet.clone());
                expires = Some(expires.map_or(until, |at| at.min(until)));
            }
        }
        (wallets, expires)
    }
}

#[derive(Clone, Debug)]
pub struct WalletTopology {
    /// Every wallet either source holds, at snapshot time.
    pub wallets: Arc<BTreeSet<String>>,
    /// The last Pulse islands receipt.
    pub received_at: Option<Instant>,
    pub generation: u64,
    islands: Arc<BTreeSet<String>>,
    world: Arc<BTreeSet<String>>,
    world_expires_at: Option<Instant>,
}

impl WalletTopology {
    pub fn is_online(&self, wallet: &str) -> bool {
        let wallet = wallet.to_ascii_lowercase();
        (fresh(self.received_at, TOPOLOGY_RECEIPT_TTL) && self.islands.contains(&wallet))
            || self.world.contains(&wallet)
    }

    /// The next instant a source's membership may expire; `None` when none will.
    pub fn expires_at(&self) -> Option<Instant> {
        let now = Instant::now();
        [
            self.received_at.map(|at| at + TOPOLOGY_RECEIPT_TTL),
            self.world_expires_at,
        ]
        .into_iter()
        .flatten()
        .filter(|at| *at > now)
        .min()
    }

    fn islands_live(&self) -> bool {
        fresh(self.received_at, TOPOLOGY_RECEIPT_TTL) && !self.islands.is_empty()
    }
}

struct State {
    view: WalletTopology,
    islands_live: bool,
    world: WorldSessions,
}

pub struct Topology {
    current: Mutex<State>,
    changed: watch::Sender<u64>,
}

impl Default for Topology {
    fn default() -> Self {
        Self {
            current: Mutex::new(State {
                view: WalletTopology {
                    wallets: Arc::default(),
                    received_at: None,
                    generation: 0,
                    islands: Arc::default(),
                    world: Arc::default(),
                    world_expires_at: None,
                },
                islands_live: false,
                world: WorldSessions::default(),
            }),
            changed: watch::channel(0).0,
        }
    }
}

impl Topology {
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// Recomputes the union when a source changed or crossed an expiry; bumps the
    /// generation only when the effective wallet set changed.
    fn settle(&self, state: &mut State, islands_changed: bool, world_changed: bool) {
        let now = Instant::now();
        let world_due = state.view.world_expires_at.is_some_and(|at| at <= now);
        if world_changed || world_due {
            let (world, expires) = state.world.wallets(now);
            state.view.world = Arc::new(world);
            state.view.world_expires_at = expires;
        }
        let islands_live = state.view.islands_live();
        if !islands_changed && !world_changed && !world_due && islands_live == state.islands_live {
            return;
        }
        state.islands_live = islands_live;
        let view = &mut state.view;
        let wallets = match (islands_live, view.world.is_empty()) {
            (false, _) => view.world.clone(),
            (true, true) => view.islands.clone(),
            (true, false) => Arc::new(view.islands.union(&view.world).cloned().collect()),
        };
        if wallets != view.wallets {
            view.generation += 1;
            self.changed.send_replace(view.generation);
        }
        view.wallets = wallets;
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
        if current.view.received_at.is_some_and(|at| at > received_at) {
            return Ok(());
        }
        // Expiry is an effective transition even when the next frame repeats the set.
        self.settle(&mut current, false, false);
        current.view.islands = islands;
        current.view.received_at = Some(received_at);
        self.settle(&mut current, true, false);
        Ok(())
    }

    /// Applies one LiveKit join or leave of participant session `sid` in a World room.
    /// Duplicates are no-ops, and a leave stays final if its join arrives later.
    pub fn apply_world_event(&self, sid: &str, identity: &str, joined: bool) {
        let Some(wallet) = wallet_token(identity) else {
            return;
        };
        if sid.is_empty() {
            return;
        }
        let mut current = self.current.lock().unwrap();
        let changed = current.world.apply(sid, wallet, joined, Instant::now());
        self.settle(&mut current, false, changed);
    }

    /// One complete read of every World room, started at `started`, listing each
    /// participant as `(sid, identity)`. Sessions it missed end unless an event changed
    /// them after it started; a leave it predates stays final.
    pub fn observe_world<'a>(
        &self,
        started: Instant,
        participants: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) {
        let seen = participants
            .into_iter()
            .filter(|(sid, _)| !sid.is_empty())
            .filter_map(|(sid, identity)| Some((sid.to_owned(), wallet_token(identity)?)))
            .collect();
        let mut current = self.current.lock().unwrap();
        current.world.observe(started, seen, Instant::now());
        self.settle(&mut current, false, true);
    }

    /// Tests: `wallets` online for the test's length, with no receipt to expire on a
    /// slow run. A later frame older than this hold is ignored, as any stale frame is.
    #[cfg(any(test, feature = "test-support"))]
    pub fn hold_for_tests<'a>(&self, wallets: impl IntoIterator<Item = &'a str>) {
        let mut current = self.current.lock().unwrap();
        current.view.islands = Arc::new(wallets.into_iter().filter_map(wallet_token).collect());
        current.view.received_at = Some(Instant::now() + Duration::from_secs(24 * 60 * 60));
        current.view.generation += 1;
        self.settle(&mut current, true, false);
        self.changed.send_replace(current.view.generation);
    }

    pub fn snapshot(&self) -> WalletTopology {
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false, false);
        current.view.clone()
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
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false, false);
        if current.view.generation != generation || current.view.is_online(wallet) != online {
            return false;
        }
        deliver();
        true
    }

    /// Unrelated wallet changes need not erase an already queued, still-true update.
    /// Compare its generation and current status again at queue consumption.
    pub fn queued_status_is_current(&self, generation: u64, wallet: &str, online: bool) -> bool {
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false, false);
        generation <= current.view.generation && current.view.is_online(wallet) == online
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

    fn online(state: &Topology, wallet: &str) -> bool {
        state.snapshot().is_online(wallet)
    }

    #[tokio::test(start_paused = true)]
    async fn wallet_in_an_island_and_a_world_is_online_once_until_it_is_in_neither() {
        let state = Topology::default();
        state.receive(&frame(&[A])).unwrap();
        state.apply_world_event("PA_a", A, true);
        state.apply_world_event("PA_b", B, true);
        let both = state.snapshot();
        assert_eq!(both.wallets.iter().collect::<Vec<_>>(), [A, B]);
        state.apply_world_event("PA_a", A, false);
        assert!(online(&state, A));
        assert_eq!(
            state.snapshot().generation,
            both.generation,
            "leaving the World while still in an island is no transition"
        );
        state.apply_world_event("PA_a2", A, true);
        state.receive(&frame(&[])).unwrap();
        assert!(online(&state, A), "still inside the World");
        assert_eq!(state.snapshot().generation, both.generation);
        state.apply_world_event("PA_a2", A, false);
        assert!(!online(&state, A));
        assert!(state.snapshot().generation > both.generation);
        assert!(!state.queued_status_is_current(both.generation, A, true));
    }

    #[tokio::test(start_paused = true)]
    async fn duplicates_change_nothing_and_a_late_join_never_revives_a_left_session() {
        let state = Topology::default();
        state.apply_world_event("PA_1", &A.to_uppercase().replace("0X", "0x"), true);
        let joined = state.snapshot().generation;
        state.apply_world_event("PA_1", A, true);
        assert_eq!(state.snapshot().generation, joined, "a duplicate join");
        state.apply_world_event("PA_1", A, false);
        state.apply_world_event("PA_1", A, false);
        state.apply_world_event("PA_1", A, true);
        assert!(!online(&state, A), "a redelivered join after the leave");
        // The leave arrives before its join.
        state.apply_world_event("PA_2", B, false);
        state.apply_world_event("PA_2", B, true);
        assert!(!online(&state, B));
        state.apply_world_event("PA_3", "authoritative-server", true);
        state.apply_world_event("PA_4", "stream:room:1", true);
        state.apply_world_event("", B, true);
        assert!(state.snapshot().wallets.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_wallet_in_several_world_rooms_is_online_until_its_last_leave() {
        let state = Topology::default();
        state.apply_world_event("PA_lobby", A, true);
        state.apply_world_event("PA_scene", A, true);
        state.apply_world_event("PA_lobby", A, false);
        assert!(online(&state, A));
        state.apply_world_event("PA_scene", A, false);
        assert!(!online(&state, A));
    }

    #[tokio::test(start_paused = true)]
    async fn a_complete_read_corrects_drift_but_loses_to_newer_events() {
        let state = Topology::default();
        state.apply_world_event("PA_missed_leave", A, true);
        let started = Instant::now();
        tokio::time::advance(Duration::from_millis(10)).await;
        // Events applied while the read was in flight.
        state.apply_world_event("PA_new", B, true);
        state.apply_world_event("PA_restarted", A, false);
        state.observe_world(
            started,
            [
                ("PA_restarted", A),
                ("PA_unseen", "0xcccccccccccccccccccccccccccccccccccccccc"),
            ],
        );
        let view = state.snapshot();
        assert!(
            !view.is_online(A),
            "the missed leave ends; the newer leave stays"
        );
        assert!(view.is_online(B), "a join newer than the read stays");
        assert!(view.is_online("0xcccccccccccccccccccccccccccccccccccccccc"));
        // A later read without B, started after B's join, ends it.
        tokio::time::advance(Duration::from_millis(10)).await;
        state.observe_world(Instant::now(), []);
        assert!(!online(&state, B));
        assert!(state.snapshot().wallets.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn an_unconfirmed_session_expires_while_reads_keep_failing() {
        let state = Topology::default();
        state.apply_world_event("PA_1", A, true);
        tokio::time::advance(WORLD_OCCUPANCY_TTL / 2).await;
        state.apply_world_event("PA_2", B, true);
        assert_eq!(
            state.snapshot().expires_at(),
            Some(Instant::now() + WORLD_OCCUPANCY_TTL / 2)
        );
        // A quiet Pulse never drops World-only wallets.
        state.receive(&frame(&[])).unwrap();
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL * 2).await;
        assert!(online(&state, A));
        tokio::time::advance(WORLD_OCCUPANCY_TTL / 2).await;
        assert!(!online(&state, A), "no read confirmed it for the whole TTL");
        assert!(online(&state, B), "B's own TTL has not passed");
        // A read that lists it confirms it again.
        state.observe_world(Instant::now(), [("PA_2", B)]);
        tokio::time::advance(WORLD_OCCUPANCY_TTL / 2 + Duration::from_secs(1)).await;
        assert!(online(&state, B));
    }

    #[tokio::test]
    async fn a_world_change_wakes_listeners() {
        let state = Topology::default();
        let mut changes = state.changes();
        state.apply_world_event("PA_1", A, true);
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();
        state.observe_world(Instant::now(), [("PA_1", A)]);
        assert!(!changes.has_changed().unwrap(), "same set, no transition");
    }
}
