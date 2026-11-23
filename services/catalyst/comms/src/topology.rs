//! Complete wallet topology for social consumers: Pulse islands (Genesis) united with
//! World room presence. A wallet is online while either source holds it.
//! Pulse receipt age is local liveness, not proof of source age: the existing wire has no
//! source timestamp/incarnation. Deploy one Pulse publisher per node; snapshots replace,
//! never union publisher state. World presence applies every replica's LiveKit room
//! events as they arrive; this replica's complete LiveKit reads correct missed ones.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::protocol::bus::{
    BusError, room_events::MAX_PARTICIPANT_SID_BYTES, topology::decode_islands, wallet_token,
};

pub const TOPOLOGY_RECEIPT_TTL: Duration = Duration::from_secs(5);
/// A World session no event or complete read confirmed for this long stops counting.
pub const WORLD_OCCUPANCY_TTL: Duration = crate::presence::PUBLIC_TTL;
/// How long an event leave is remembered, so a delayed join cannot revive it.
const WORLD_LEFT_TTL: Duration = Duration::from_secs(300);
/// Live sessions held at once; joins past it are dropped and counted.
const MAX_WORLD_LIVE: usize = 200_000;
/// Remembered leaves; the oldest is forgotten first.
const MAX_WORLD_LEFT: usize = 200_000;

/// Match the LiveKit participant SID limit enforced by the webhook parser.
fn valid_world_sid(sid: &str) -> bool {
    (1..=MAX_PARTICIPANT_SID_BYTES).contains(&sid.len())
}

/// One live LiveKit participant session (keyed by its sid) in a World room.
struct LiveSession {
    wallet: String,
    /// The last event or complete read that confirmed it.
    confirmed: Instant,
    /// When the session was first observed; a read started before then cannot end it.
    changed: Instant,
    /// Tells this session's expiry entries from a reused sid's.
    incarnation: u64,
}

/// World sessions. A sid joins at most once, so an event leave is final; a session a read
/// missed or that expired is simply forgotten and comes back when listed or joined again.
#[derive(Default)]
struct WorldSessions {
    live: HashMap<String, LiveSession>,
    /// Live sessions per wallet; only a 0 <-> 1 move changes the wallet's presence.
    per_wallet: HashMap<String, u32>,
    /// Entries are re-armed lazily after confirmation. Entries left by ended
    /// sessions are cleared at their deadlines or compacted on insertion.
    expiry: BinaryHeap<Reverse<(Instant, u64, String)>>,
    left: HashSet<String>,
    left_order: VecDeque<(Instant, String)>,
    incarnations: u64,
}

/// A wallet whose World presence started (`true`) or ended.
type Transition = (String, bool);

impl WorldSessions {
    fn add(&mut self, sid: &str, wallet: String, at: Instant, out: &mut Vec<Transition>) {
        if self.live.len() >= MAX_WORLD_LIVE {
            metrics::counter!("comms_world_presence_dropped_total", "reason" => "live_cap")
                .increment(1);
            return;
        }
        // Churn can leave future deadlines for ended sessions. Compact when
        // at least half the entries are stale, with slack for small sets.
        if self.expiry.len() >= (2 * self.live.len()).max(64) {
            let live = &self.live;
            self.expiry.retain(|Reverse((_, incarnation, sid))| {
                live.get(sid)
                    .is_some_and(|session| session.incarnation == *incarnation)
            });
        }
        self.incarnations += 1;
        let incarnation = self.incarnations;
        self.expiry.push(Reverse((
            at + WORLD_OCCUPANCY_TTL,
            incarnation,
            sid.to_owned(),
        )));
        let count = self.per_wallet.entry(wallet.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            out.push((wallet.clone(), true));
        }
        let session = LiveSession {
            wallet,
            confirmed: at,
            changed: at,
            incarnation,
        };
        self.live.insert(sid.to_owned(), session);
    }

    fn end(&mut self, sid: &str, out: &mut Vec<Transition>) {
        let Some(session) = self.live.remove(sid) else {
            return;
        };
        let count = self.per_wallet.get_mut(&session.wallet).expect("counted");
        *count -= 1;
        if *count == 0 {
            self.per_wallet.remove(&session.wallet);
            out.push((session.wallet, false));
        }
    }

    fn remember_left(&mut self, sid: &str, now: Instant) {
        if !self.left.insert(sid.to_owned()) {
            return;
        }
        self.left_order.push_back((now, sid.to_owned()));
        while self.left.len() > MAX_WORLD_LEFT {
            let Some((_, oldest)) = self.left_order.pop_front() else {
                break;
            };
            self.left.remove(&oldest);
            metrics::counter!("comms_world_presence_dropped_total", "reason" => "left_cap")
                .increment(1);
        }
    }

    /// Ends sessions past their TTL and forgets old leaves.
    fn collect(&mut self, now: Instant, out: &mut Vec<Transition>) {
        while let Some(Reverse((deadline, incarnation, _))) = self.expiry.peek()
            && *deadline <= now
        {
            let (incarnation, sid) = (*incarnation, self.expiry.pop().unwrap().0.2);
            let Some(session) = self.live.get(&sid).filter(|s| s.incarnation == incarnation) else {
                continue;
            };
            let until = session.confirmed + WORLD_OCCUPANCY_TTL;
            if until > now {
                self.expiry.push(Reverse((until, incarnation, sid)));
            } else {
                self.end(&sid, out);
            }
        }
        while let Some((at, _)) = self.left_order.front()
            && now.saturating_duration_since(*at) >= WORLD_LEFT_TTL
        {
            let (_, sid) = self.left_order.pop_front().unwrap();
            self.left.remove(&sid);
        }
    }

    fn apply(
        &mut self,
        sid: &str,
        wallet: String,
        joined: bool,
        now: Instant,
        out: &mut Vec<Transition>,
    ) {
        self.collect(now, out);
        if self.left.contains(sid) {
            return;
        }
        if !joined {
            self.end(sid, out);
            self.remember_left(sid, now);
        } else if !self.live.contains_key(sid) {
            self.add(sid, wallet, now, out);
        }
    }

    /// One read that started at `started` and listed `seen`: confirms and adds what it
    /// listed; when `complete`, also ends live sessions it missed that no event changed
    /// after it started.
    fn observe(
        &mut self,
        started: Instant,
        seen: Vec<(String, String)>,
        complete: bool,
        now: Instant,
        out: &mut Vec<Transition>,
    ) {
        self.collect(now, out);
        let mut listed = HashSet::with_capacity(seen.len());
        for (sid, wallet) in seen {
            if self.left.contains(&sid) {
                continue;
            }
            match self.live.get_mut(&sid) {
                Some(session) => session.confirmed = session.confirmed.max(started),
                None => self.add(&sid, wallet, started, out),
            }
            listed.insert(sid);
        }
        if complete {
            let missed: Vec<String> = self
                .live
                .iter()
                .filter(|(sid, s)| s.changed <= started && !listed.contains(*sid))
                .map(|(sid, _)| sid.clone())
                .collect();
            for sid in missed {
                self.end(&sid, out);
            }
        }
        self.collect(now, out);
    }

    fn next_expiry(&self) -> Option<Instant> {
        self.expiry.peek().map(|Reverse((at, _, _))| *at)
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
    islands_expires_at: Option<Instant>,
    world: Arc<BTreeSet<String>>,
    world_expires_at: Option<Instant>,
}

impl WalletTopology {
    pub fn is_online(&self, wallet: &str) -> bool {
        self.wallets.contains(&wallet.to_ascii_lowercase())
    }

    fn status_is_current(&self, generation: u64, wallet: &str, online: bool) -> bool {
        generation <= self.generation && self.is_online(wallet) == online
    }

    /// The earliest expiry in this snapshot. A deadline that has passed wakes the worker.
    pub fn expires_at(&self) -> Option<Instant> {
        [self.islands_expires_at, self.world_expires_at]
            .into_iter()
            .flatten()
            .min()
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
                    islands_expires_at: None,
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

    fn bump(&self, view: &mut WalletTopology) {
        view.generation += 1;
        self.changed.send_replace(view.generation);
    }

    /// Reschedule expiry when a source advances the deadline without changing membership.
    fn wake_for_earlier_expiry(&self, previous: Option<Instant>, view: &WalletTopology) {
        if view
            .expires_at()
            .is_some_and(|next| previous.is_none_or(|previous| next < previous))
        {
            self.changed.send_replace(view.generation);
        }
    }

    /// Applies World presence transitions to the World set and, where the islands do
    /// not already hold the wallet, to the union.
    fn apply_transitions(&self, state: &mut State, transitions: Vec<Transition>) {
        let view = &mut state.view;
        let previous_expiry = view.expires_at();
        view.world_expires_at = state.world.next_expiry();
        let mut changed = false;
        for (wallet, online) in transitions {
            let world = Arc::make_mut(&mut view.world);
            if online {
                world.insert(wallet.clone());
            } else {
                world.remove(&wallet);
            }
            if state.islands_live && view.islands.contains(&wallet) {
                continue;
            }
            let wallets = Arc::make_mut(&mut view.wallets);
            changed |= if online {
                wallets.insert(wallet)
            } else {
                wallets.remove(&wallet)
            };
        }
        metrics::gauge!("comms_world_presence_sessions").set(state.world.live.len() as f64);
        metrics::gauge!("comms_world_presence_wallets").set(view.world.len() as f64);
        if changed {
            self.bump(view);
        } else {
            self.wake_for_earlier_expiry(previous_expiry, view);
        }
    }

    /// Expires due World sessions and, when the islands changed or crossed their expiry,
    /// rebuilds the union; bumps the generation only when the wallet set changed.
    fn settle(&self, state: &mut State, islands_changed: bool) {
        let now = Instant::now();
        if state.view.world_expires_at.is_some_and(|at| at <= now) {
            let mut transitions = Vec::new();
            state.world.collect(now, &mut transitions);
            self.apply_transitions(state, transitions);
        }
        let view = &mut state.view;
        let previous_expiry = view.expires_at();
        view.islands_expires_at = view
            .received_at
            .map(|at| at + TOPOLOGY_RECEIPT_TTL)
            .filter(|&deadline| deadline > now && !view.islands.is_empty());
        let islands_live = view.islands_expires_at.is_some();
        if !islands_changed && islands_live == state.islands_live {
            return;
        }
        state.islands_live = islands_live;
        let wallets = match (islands_live, view.world.is_empty()) {
            (false, _) => view.world.clone(),
            (true, true) => view.islands.clone(),
            (true, false) => Arc::new(view.islands.union(&view.world).cloned().collect()),
        };
        if wallets != view.wallets {
            view.wallets = wallets;
            self.bump(view);
        } else {
            view.wallets = wallets;
            self.wake_for_earlier_expiry(previous_expiry, view);
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
        if current.view.received_at.is_some_and(|at| at > received_at) {
            return Ok(());
        }
        // Expiry is an effective transition even when the next frame repeats the set.
        self.settle(&mut current, false);
        current.view.islands = islands;
        current.view.received_at = Some(received_at);
        self.settle(&mut current, true);
        Ok(())
    }

    /// Applies one LiveKit join or leave of participant session `sid` in a World room.
    /// Duplicates are no-ops, and a leave stays final if its join arrives later.
    pub fn apply_world_event(&self, sid: &str, identity: &str, joined: bool) {
        if !valid_world_sid(sid) {
            return;
        }
        let Some(wallet) = wallet_token(identity) else {
            return;
        };
        let mut current = self.current.lock().unwrap();
        let mut transitions = Vec::new();
        current
            .world
            .apply(sid, wallet, joined, Instant::now(), &mut transitions);
        self.apply_transitions(&mut current, transitions);
    }

    /// One read of World rooms, started at `started`, listing each participant as
    /// `(sid, identity)`. It confirms and adds what it listed (never what an event left).
    /// A `complete` read of every World room also ends the live sessions it missed,
    /// unless an event changed them after it started.
    pub fn observe_world<'a>(
        &self,
        started: Instant,
        participants: impl IntoIterator<Item = (&'a str, &'a str)>,
        complete: bool,
    ) {
        let seen = participants
            .into_iter()
            .filter(|(sid, _)| valid_world_sid(sid))
            .filter_map(|(sid, identity)| Some((sid.to_owned(), wallet_token(identity)?)))
            .collect();
        let mut current = self.current.lock().unwrap();
        let mut transitions = Vec::new();
        current
            .world
            .observe(started, seen, complete, Instant::now(), &mut transitions);
        self.apply_transitions(&mut current, transitions);
    }

    /// Tests: `wallets` online for the test's length, with no receipt to expire on a
    /// slow run. A later frame older than this hold is ignored, as any stale frame is.
    #[cfg(any(test, feature = "test-support"))]
    pub fn hold_for_tests<'a>(&self, wallets: impl IntoIterator<Item = &'a str>) {
        let mut current = self.current.lock().unwrap();
        current.view.islands = Arc::new(wallets.into_iter().filter_map(wallet_token).collect());
        current.view.received_at = Some(Instant::now() + Duration::from_secs(24 * 60 * 60));
        self.settle(&mut current, true);
        let view = &mut current.view;
        self.bump(view);
    }

    pub fn snapshot(&self) -> WalletTopology {
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false);
        current.view.clone()
    }

    /// Hold the same short lock as replacement through synchronous queue publication.
    /// Publish a still-current status even if another wallet advanced the generation.
    pub fn deliver_if_current(
        &self,
        generation: u64,
        wallet: &str,
        online: bool,
        deliver: impl FnOnce(),
    ) -> bool {
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false);
        if !current.view.status_is_current(generation, wallet, online) {
            return false;
        }
        deliver();
        true
    }

    /// Unrelated wallet changes need not erase an already queued, still-true update.
    /// Compare its generation and current status again at queue consumption.
    pub fn queued_status_is_current(&self, generation: u64, wallet: &str, online: bool) -> bool {
        let mut current = self.current.lock().unwrap();
        self.settle(&mut current, false);
        current.view.status_is_current(generation, wallet, online)
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
    async fn a_held_snapshot_keeps_the_same_pulse_membership_after_expiry() {
        let state = Topology::default();
        state.receive(&frame(&[A])).unwrap();
        state.apply_world_event("PA_world", B, true);
        let held = state.snapshot();
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL).await;
        assert!(held.wallets.contains(A));
        assert!(held.is_online(A));
        let current = state.snapshot();
        assert!(!current.is_online(A));
        assert!(current.is_online(B));
        assert!(!state.queued_status_is_current(held.generation, A, true));
    }

    #[tokio::test(start_paused = true)]
    async fn a_snapshot_deadline_still_wakes_the_worker_after_it_passes() {
        let state = Topology::default();
        state.apply_world_event("PA_world", A, true);
        let deadline = Instant::now() + WORLD_OCCUPANCY_TTL;
        tokio::time::advance(WORLD_OCCUPANCY_TTL - Duration::from_millis(1)).await;
        let held = state.snapshot();
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(held.is_online(A));
        assert_eq!(held.expires_at(), Some(deadline));
        let current = state.snapshot();
        assert!(!current.is_online(A));
        assert_eq!(current.expires_at(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_duplicate_join_does_not_supersede_a_complete_read() {
        let state = Topology::default();
        state.apply_world_event("PA_old", A, true);
        tokio::time::advance(Duration::from_millis(1)).await;
        let started = Instant::now();
        tokio::time::advance(Duration::from_millis(1)).await;
        state.apply_world_event("PA_old", A, true);
        state.apply_world_event("PA_new", B, true);

        state.observe_world(started, [], true);

        let current = state.snapshot();
        assert!(
            !current.is_online(A),
            "the duplicate did not change the session"
        );
        assert!(
            current.is_online(B),
            "the first join is newer than the read"
        );
    }

    fn online(state: &Topology, wallet: &str) -> bool {
        state.snapshot().is_online(wallet)
    }

    #[tokio::test(start_paused = true)]
    async fn publication_uses_the_wallet_status_after_unrelated_changes() {
        let state = Topology::default();
        state.apply_world_event("PA_actor", A, true);
        let generation = state.snapshot().generation;
        state.apply_world_event("PA_other", B, true);
        let current = state.snapshot().generation;
        assert!(current > generation);

        let mut delivered = false;
        assert!(state.deliver_if_current(generation, A, true, || delivered = true));
        assert!(delivered);
        assert!(!state.deliver_if_current(current + 1, A, true, || panic!("future delivery")));

        state.apply_world_event("PA_actor", A, false);
        assert!(!state.deliver_if_current(generation, A, true, || panic!("stale delivery")));
    }

    #[tokio::test(start_paused = true)]
    async fn world_events_and_reads_limit_the_sid_length() {
        let state = Topology::default();
        let event_sid = format!("PA_{}", "a".repeat(125));
        let read_sid = format!("PA_{}", "b".repeat(125));
        let oversized_sid = format!("{event_sid}a");

        state.apply_world_event(&oversized_sid, A, true);
        assert!(!online(&state, A));
        state.observe_world(Instant::now(), [(oversized_sid.as_str(), B)], false);
        assert!(!online(&state, B));
        state.apply_world_event(&oversized_sid, A, false);
        assert!(state.current.lock().unwrap().world.left.is_empty());

        state.apply_world_event(&event_sid, A, true);
        state.observe_world(Instant::now(), [(read_sid.as_str(), B)], false);
        assert!(online(&state, A));
        assert!(online(&state, B));
    }

    #[tokio::test(start_paused = true)]
    async fn world_expiry_entries_stay_bounded_during_join_and_leave_churn() {
        let state = Topology::default();
        state.apply_world_event("PA_stable", A, true);
        for number in 0..256 {
            let sid = format!("PA_{number}");
            state.apply_world_event(&sid, B, true);
            state.apply_world_event(&sid, B, false);
        }
        assert!(state.current.lock().unwrap().world.expiry.len() <= 64);
        assert!(online(&state, A));
        tokio::time::advance(WORLD_OCCUPANCY_TTL).await;
        assert!(!online(&state, A));
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
            true,
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
        state.observe_world(Instant::now(), [], true);
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
        state.observe_world(Instant::now(), [("PA_2", B)], true);
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
        state.observe_world(Instant::now(), [("PA_1", A)], true);
        assert!(!changes.has_changed().unwrap(), "same set, no transition");
    }

    #[tokio::test(start_paused = true)]
    async fn an_earlier_pulse_expiry_wakes_the_worker_without_a_wallet_change() {
        let state = Topology::default();
        state.apply_world_event("PA_world", A, true);
        let initial = state.snapshot();
        let mut changes = state.changes();

        state.receive(&frame(&[A])).unwrap();
        state.apply_world_event("PA_world", A, false);

        let handoff = state.snapshot();
        assert!(handoff.is_online(A));
        assert_eq!(handoff.generation, initial.generation);
        assert!(
            changes.has_changed().unwrap(),
            "the worker needs an earlier timer"
        );
        assert!(handoff.expires_at() < initial.expires_at());
        changes.mark_unchanged();
        tokio::time::sleep_until(handoff.expires_at().unwrap()).await;
        let expired = state.snapshot();
        assert!(!expired.is_online(A));
        assert!(expired.generation > handoff.generation);
    }

    #[tokio::test(start_paused = true)]
    async fn a_session_a_read_missed_comes_back_when_a_later_read_lists_it() {
        let state = Topology::default();
        state.apply_world_event("PA_1", A, true);
        tokio::time::advance(Duration::from_millis(10)).await;
        state.observe_world(Instant::now(), [], true);
        assert!(!online(&state, A), "the read missed it");
        tokio::time::advance(Duration::from_millis(10)).await;
        state.observe_world(Instant::now(), [("PA_1", A)], true);
        assert!(online(&state, A), "only an event leave is final");
        state.apply_world_event("PA_1", A, false);
        tokio::time::advance(Duration::from_millis(10)).await;
        state.observe_world(Instant::now(), [("PA_1", A)], true);
        assert!(!online(&state, A));
    }

    #[tokio::test(start_paused = true)]
    async fn a_join_refreshing_an_expired_session_brings_it_back() {
        let state = Topology::default();
        state.apply_world_event("PA_1", A, true);
        tokio::time::advance(WORLD_OCCUPANCY_TTL).await;
        state.apply_world_event("PA_1", A, true);
        assert!(online(&state, A));
        tokio::time::advance(WORLD_OCCUPANCY_TTL - Duration::from_secs(1)).await;
        assert!(online(&state, A), "the refresh restarted its TTL");
    }

    #[tokio::test(start_paused = true)]
    async fn a_partial_read_confirms_what_it_listed_and_ends_nothing() {
        let state = Topology::default();
        state.apply_world_event("PA_a", A, true);
        state.apply_world_event("PA_b", B, true);
        tokio::time::advance(WORLD_OCCUPANCY_TTL / 2).await;
        state.observe_world(Instant::now(), [("PA_a", A)], false);
        assert!(online(&state, B), "a partial read ends nothing");
        tokio::time::advance(WORLD_OCCUPANCY_TTL / 2).await;
        assert!(online(&state, A), "confirmed by the partial read");
        assert!(!online(&state, B), "nothing confirmed it");
    }

    #[tokio::test(start_paused = true)]
    async fn only_the_first_live_session_and_the_last_leave_move_the_published_set() {
        let state = Topology::default();
        state.apply_world_event("PA_1", A, true);
        let first = state.snapshot();
        state.apply_world_event("PA_2", A, true);
        state.apply_world_event("PA_1", A, false);
        assert!(Arc::ptr_eq(&state.snapshot().wallets, &first.wallets));
        assert_eq!(state.snapshot().generation, first.generation);
    }

    #[tokio::test(start_paused = true)]
    async fn a_world_transition_updates_unshared_sets_in_place() {
        let state = Topology::default();
        state.apply_world_event("PA_1", A, true);
        state.apply_world_event("PA_2", B, true);
        let held = state.snapshot();
        let (wallets, world) = (Arc::as_ptr(&held.wallets), Arc::as_ptr(&held.world));
        drop(held);
        state.apply_world_event("PA_2", B, false);
        let after = state.snapshot();
        assert_eq!(
            Arc::as_ptr(&after.wallets),
            wallets,
            "no copy once no snapshot is held"
        );
        assert_eq!(Arc::as_ptr(&after.world), world);
        assert!(!after.is_online(B));
    }
}
