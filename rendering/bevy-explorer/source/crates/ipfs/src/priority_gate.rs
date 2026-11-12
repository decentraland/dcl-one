//! Request slots handed out by priority instead of first come first served: the scene or avatar
//! a download belongs to decides, through the keys `scene_runner` keeps here, so the nearest
//! in-front scene's files download first. A hash is registered by the entity that owns it (a
//! scene root, each avatar wearing a wearable) and released when that entity goes, so the set
//! of keys follows the entities' lifecycle; the values are written in place on frames in which a
//! grant can choose between requests. Requests queue per hash, so a key change re-ranks queues,
//! not every waiting request, and a grant pops one queue's head.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
    time::Duration,
};

use bevy::log::error;
use common::partition::{LoadPriority, Tier};
use tokio::sync::oneshot;

/// A request's place in the queue, taken when a read decides to download and awaited by the
/// download for its slot. A later reader of the same download queues the same ticket under its
/// own scene too, so the download rises with the nearest of its readers
/// and is granted once.
pub struct Ticket {
    pub(super) seq: u64,
    gate: Weak<PriorityGate>,
    slot: Mutex<Option<oneshot::Receiver<Permit>>>,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if let Some(gate) = self.gate.upgrade() {
            let abandoned = gate.state().remove_ticket(self.seq);
            // Closing the sender can wake its waiter.
            drop(abandoned);
        }
    }
}

struct Waiting {
    owners: Vec<Option<String>>,
    grant: oneshot::Sender<Permit>,
}

pub struct PriorityGate {
    state: Mutex<GateState>,
}

struct GateState {
    available: usize,
    /// permits delivered and not yet dropped
    in_use: usize,
    next_seq: u64,
    next_download: u64,
    readers: HashMap<u64, ReaderState>,
    /// the registered hashes and their keys
    keys: HashMap<String, KeyEntry>,
    /// counts publishes, so that a frame's first write of a key replaces it and later writes
    /// (a wearable's other avatars) take the best
    generation: u64,
    /// requests that carry no hash: after every registered one
    no_scene: BTreeSet<u64>,
    waiting: HashMap<u64, Waiting>,
    /// requests by hash, oldest first; a hash's set is dropped when it empties
    scenes: HashMap<String, BTreeSet<u64>>,
    /// hashes whose holders all left while a request of theirs was alive: they rank last until
    /// that request ends or the hash is registered again
    departed: HashSet<String>,
    /// while set, requests ranked in this tier or a later one wait even when a slot is free
    held_from: Option<Tier>,
}

/// The live readers of one logical download, including while it is between attempts.
struct ReaderState {
    owners: HashMap<Option<String>, usize>,
    queued: Option<u64>,
    // Dropping this sender cancels even an active or sleeping shared download.
    cancel: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub(super) struct DownloadReaders {
    gate: Arc<PriorityGate>,
    id: u64,
}

/// The task owns its original place in the queue before its worker is polled.
pub(super) struct DownloadAttempts {
    readers: DownloadReaders,
    first: Option<Arc<Ticket>>,
    initial_delay: Option<Duration>,
}

pub(super) struct Registration {
    pub(super) readers: DownloadReaders,
    owner: Option<String>,
}

impl DownloadReaders {
    pub(super) fn first(
        gate: &Arc<PriorityGate>,
        owner: Option<String>,
        cancel: Option<oneshot::Sender<()>>,
        initial_delay: Option<Duration>,
    ) -> (Registration, DownloadAttempts) {
        let mut state = gate.state();
        let id = state.next_download;
        state.next_download += 1;
        // Delayed preview downloads can acquire readers immediately, but only enter the
        // gate once the artificial delay ends. They never hold a slot while sleeping.
        let ticket = initial_delay
            .is_none()
            .then(|| gate.enqueue(&mut state, [owner.clone()]));
        state.readers.insert(
            id,
            ReaderState {
                owners: [(owner.clone(), 1)].into_iter().collect(),
                queued: ticket.as_ref().map(|ticket| ticket.seq),
                cancel,
            },
        );
        drop(state);
        if ticket.is_some() {
            gate.grant();
        }
        let readers = Self {
            gate: gate.clone(),
            id,
        };
        (
            Registration {
                readers: readers.clone(),
                owner,
            },
            DownloadAttempts {
                readers,
                first: ticket,
                initial_delay,
            },
        )
    }

    pub(super) fn try_register(&self, owner: Option<String>) -> Option<Registration> {
        let mut state = self.gate.state();
        let readers = state.readers.get_mut(&self.id)?;
        let count = readers.owners.entry(owner.clone()).or_default();
        *count += 1;
        let joined = *count == 1 && readers.queued.is_some();
        if joined {
            if let Some(seq) = readers.queued {
                state.join(seq, owner.clone());
            }
        }
        drop(state);
        // the new owner may rank ahead of a hold the ticket waited behind
        if joined {
            self.gate.grant();
        }
        Some(Registration {
            readers: self.clone(),
            owner,
        })
    }

    fn next_ticket(&self) -> Result<Arc<Ticket>, GateClosed> {
        let mut state = self.gate.state();
        let seq = state.next_seq;
        let readers = state.readers.get_mut(&self.id).ok_or(GateClosed)?;
        readers.queued = Some(seq);
        let owners: Vec<_> = readers.owners.keys().cloned().collect();
        let ticket = self.gate.enqueue(&mut state, owners);
        drop(state);
        self.gate.grant();
        Ok(ticket)
    }
}

impl DownloadAttempts {
    pub(super) async fn acquire(&mut self) -> Result<Permit, GateClosed> {
        if let Some(delay) = self.initial_delay.take() {
            async_std::task::sleep(delay).await;
        }
        let ticket = match self.first.take() {
            Some(ticket) => ticket,
            None => self.readers.next_ticket()?,
        };
        let permit = self.readers.gate.acquire(ticket).await?;
        let mut state = self.readers.gate.state();
        // The last reader could have left between the grant and this task waking.
        state
            .readers
            .get_mut(&self.readers.id)
            .ok_or(GateClosed)?
            .queued = None;
        Ok(permit)
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut state = self.readers.gate.state();
        let id = self.readers.id;
        // A destructor must not panic: a missing count is a bookkeeping bug to report, not to
        // abort the process for while it unwinds.
        let Some(readers) = state.readers.get_mut(&id) else {
            error!("request slot reader dropped after its download {id} closed");
            return;
        };
        let Some(count) = readers.owners.get_mut(&self.owner) else {
            error!("request slot reader dropped twice for download {id}");
            return;
        };
        *count -= 1;
        if *count > 0 {
            return;
        }
        readers.owners.remove(&self.owner);
        let queued = readers.queued;
        if readers.owners.is_empty() {
            let cancelled = state.readers.remove(&id).and_then(|readers| readers.cancel);
            let abandoned = queued.and_then(|seq| state.remove_ticket(seq));
            state.settle_departed(&self.owner);
            // Both senders can wake tasks which immediately need the gate.
            drop(state);
            drop(cancelled);
            drop(abandoned);
        } else {
            if let Some(seq) = queued {
                if let Some(waiting) = state.waiting.get_mut(&seq) {
                    waiting.owners.retain(|owner| owner != &self.owner);
                    state.remove_membership(seq, &self.owner);
                }
            }
            state.settle_departed(&self.owner);
        }
    }
}

struct KeyEntry {
    key: LoadPriority,
    /// the entities that hold the hash: one scene root, or each avatar wearing it
    holders: usize,
    generation: u64,
}

/// A held request slot; dropping it frees the slot for the best waiting ticket.
pub struct Permit {
    gate: Option<Arc<PriorityGate>>,
}

impl platform::http::HttpAttemptPermit for Permit {}

impl Drop for Permit {
    fn drop(&mut self) {
        if let Some(gate) = self.gate.take() {
            gate.release();
        }
    }
}

#[derive(Debug)]
pub struct GateClosed;

impl std::fmt::Display for GateClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("request slots closed")
    }
}

impl std::error::Error for GateClosed {}

impl PriorityGate {
    pub fn new(permits: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(GateState {
                available: permits,
                in_use: 0,
                next_seq: 0,
                next_download: 0,
                readers: HashMap::new(),
                keys: HashMap::new(),
                generation: 0,
                no_scene: BTreeSet::new(),
                waiting: HashMap::new(),
                scenes: HashMap::new(),
                departed: HashSet::new(),
                held_from: None,
            }),
        })
    }

    /// Recover a poisoned lock: refusing it would turn every later grant, release and
    /// destructor into another panic.
    fn state(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queue a request of `scene` (`None`: a request with no hash, ranked after every registered
    /// one). A free slot with nobody ahead is granted at once; either way the slot arrives
    /// through `acquire`.
    pub fn ticket(self: &Arc<Self>, scene: Option<String>) -> Arc<Ticket> {
        let ticket = self.enqueue(&mut self.state(), [scene]);
        self.grant();
        ticket
    }

    /// Publish one FIFO sequence under all current owners. The caller can store that sequence
    /// with its reader registrations before delivering any grant.
    fn enqueue(
        self: &Arc<Self>,
        state: &mut GateState,
        owners: impl IntoIterator<Item = Option<String>>,
    ) -> Arc<Ticket> {
        let (grant, slot) = oneshot::channel();
        let seq = state.next_seq;
        state.next_seq += 1;
        state.waiting.insert(
            seq,
            Waiting {
                owners: Vec::new(),
                grant,
            },
        );
        for owner in owners {
            state.join(seq, owner);
        }
        Arc::new(Ticket {
            seq,
            gate: Arc::downgrade(self),
            slot: Mutex::new(Some(slot)),
        })
    }

    /// Add a reader's scene to this queued ticket without changing its FIFO sequence.
    pub fn join(self: &Arc<Self>, ticket: &Arc<Ticket>, owner: Option<String>) {
        // Sequence numbers are local to each gate.
        if ticket.gate.as_ptr() == Arc::as_ptr(self) {
            self.state().join(ticket.seq, owner);
            self.grant();
        }
    }

    /// The ticket's slot, once it is its turn. A ticket is awaited once.
    pub async fn acquire(self: &Arc<Self>, ticket: Arc<Ticket>) -> Result<Permit, GateClosed> {
        let slot = ticket.slot.lock().unwrap().take().ok_or(GateClosed)?;
        slot.await.map_err(|_| GateClosed)
    }

    #[cfg(test)]
    pub fn try_acquire(self: &Arc<Self>) -> Option<Permit> {
        let mut state = self.state();
        if state.available > 0 && state.waiting.is_empty() {
            state.available -= 1;
            state.in_use += 1;
            Some(Permit {
                gate: Some(self.clone()),
            })
        } else {
            None
        }
    }

    pub(super) fn available_permits(&self) -> usize {
        self.state().available
    }

    pub fn add_permits(self: &Arc<Self>, n: usize) {
        self.state().available += n;
        self.grant();
    }

    /// Forget up to `n` free slots; slots in use are not taken back. Returns how many went.
    pub fn forget_permits(&self, n: usize) -> usize {
        let mut state = self.state();
        let freed = n.min(state.available);
        state.available -= freed;
        freed
    }

    /// An entity that owns `hash` (a scene root, an avatar wearing it) has appeared: the hash
    /// ranks by `key`, or by the best key of its holders when it is held already. Registering
    /// can lift a waiting request out of a hold (a departed or unregistered hash gains an
    /// owner, or a better one), so while a hold is on, a hash with requests waiting is offered a
    /// free slot at once.
    pub fn register(self: &Arc<Self>, hash: &str, key: LoadPriority) {
        let mut state = self.state();
        state.departed.remove(hash);
        let released = state.held_from.is_some() && state.scenes.contains_key(hash);
        match state.keys.get_mut(hash) {
            Some(entry) => {
                entry.holders += 1;
                entry.key = entry.key.min(key);
            }
            None => {
                state.keys.insert(
                    hash.to_owned(),
                    KeyEntry {
                        key,
                        holders: 1,
                        generation: 0,
                    },
                );
            }
        }
        drop(state);
        if released {
            self.grant();
        }
    }

    /// An entity that owned `hash` has gone. With no holder left the key goes, and while a
    /// request under the hash is alive (waiting, in an attempt or between attempts) the hash
    /// ranks last: its requests, retries and late readers do not count as unknown.
    pub fn unregister(&self, hash: &str) {
        let mut state = self.state();
        let Some(entry) = state.keys.get_mut(hash) else {
            return;
        };
        entry.holders -= 1;
        if entry.holders > 0 {
            return;
        }
        state.keys.remove(hash);
        if state.claimed(hash) {
            state.departed.insert(hash.to_owned());
        }
    }

    /// A frame's keys, written in place: the first write of a hash replaces its key, later
    /// writes in the same publish take the best, and a hash nobody registered is ignored.
    /// A key can leave the held tiers (the player walks into a neighbour, which rises to the
    /// player's scenes); while a hold is on and requests wait under such a hash, a free slot is
    /// offered at once.
    pub fn publish<'a>(self: &Arc<Self>, keys: impl IntoIterator<Item = (&'a str, LoadPriority)>) {
        let mut state = self.state();
        state.generation += 1;
        let generation = state.generation;
        let held_from = state.held_from;
        let held = |key: &LoadPriority| held_from.is_some_and(|tier| key.tier >= tier);
        let mut released = false;
        let GateState {
            keys: entries,
            scenes,
            ..
        } = &mut *state;
        for (hash, key) in keys {
            if let Some(entry) = entries.get_mut(hash) {
                let was_held = held(&entry.key);
                if entry.generation == generation {
                    entry.key = entry.key.min(key);
                } else {
                    entry.key = key;
                    entry.generation = generation;
                }
                released |= was_held && !held(&entry.key) && scenes.contains_key(hash);
            }
        }
        drop(state);
        if released {
            self.grant();
        }
    }

    /// Whether any request waits for a slot.
    pub fn has_waiters(&self) -> bool {
        let state = self.state();
        !state.no_scene.is_empty() || !state.scenes.is_empty()
    }

    /// Whether a grant before the next publish may choose between requests: one waits, or a
    /// slot is in use and its release can go to a request that arrives meanwhile. While neither
    /// holds, no grant reads the keys, so the publisher skips the frame.
    pub fn reads_keys(&self) -> bool {
        let state = self.state();
        state.in_use > 0 || !state.no_scene.is_empty() || !state.scenes.is_empty()
    }

    pub fn scene_priority(&self, scene: &str) -> Option<LoadPriority> {
        self.state().keys.get(scene).map(|entry| entry.key)
    }

    /// Hold every request ranked in `tier` or a later one: they keep their place in the queue
    /// but are not granted a slot, even a free one, until the hold is lifted (`None`). Requests
    /// in earlier tiers are granted as before. Downloads already holding a slot are not stopped.
    pub fn hold_from(self: &Arc<Self>, tier: Option<Tier>) {
        let mut state = self.state();
        if state.held_from == tier {
            return;
        }
        state.held_from = tier;
        drop(state);
        self.grant();
    }

    pub fn held_from(&self) -> Option<Tier> {
        self.state().held_from
    }

    #[cfg(test)]
    pub fn waiting(&self) -> usize {
        self.state().waiting()
    }

    fn release(self: &Arc<Self>) {
        let mut state = self.state();
        state.available += 1;
        state.in_use = state.in_use.saturating_sub(1);
        drop(state);
        self.grant();
    }

    /// Reserve under the gate lock and deliver outside it. Dropping an unclaimed permit
    /// re-enters the gate, and waking the receiver may schedule another download.
    pub(super) fn grant(self: &Arc<Self>) {
        loop {
            let grant = {
                let mut state = self.state();
                if state.available == 0 {
                    return;
                }
                let Some(seq) = state.best() else {
                    return;
                };
                // `grant` runs inside `Permit::drop`: a queue entry without its ticket is dropped
                // and the choice made again instead of panicking there.
                let Some(waiting) = state.remove_ticket(seq) else {
                    error!("request slot queue held ticket {seq}, which no longer waits");
                    state.forget_seq(seq);
                    continue;
                };
                state.available -= 1;
                state.in_use += 1;
                waiting.grant
            };
            if let Err(mut permit) = grant.send(Permit {
                gate: Some(self.clone()),
            }) {
                // This loop returns and reassigns the unclaimed slot. Disarm its Drop so a
                // run of cancelled receivers cannot recurse through release -> grant.
                permit.gate.take();
                let mut state = self.state();
                state.available += 1;
                state.in_use = state.in_use.saturating_sub(1);
            }
        }
    }
}

impl GateState {
    fn join(&mut self, seq: u64, owner: Option<String>) {
        let Some(waiting) = self.waiting.get_mut(&seq) else {
            return; // already granted or cancelled
        };
        if waiting.owners.contains(&owner) {
            return;
        }
        waiting.owners.push(owner.clone());
        match owner {
            None => self.no_scene.insert(seq),
            Some(hash) => self.scenes.entry(hash).or_default().insert(seq),
        };
    }

    fn remove_membership(&mut self, seq: u64, owner: &Option<String>) {
        if let Some(hash) = owner {
            if let Some(queue) = self.scenes.get_mut(hash) {
                queue.remove(&seq);
                if queue.is_empty() {
                    self.scenes.remove(hash);
                    self.settle_departed(owner);
                }
            }
        } else {
            self.no_scene.remove(&seq);
        }
    }

    fn remove_ticket(&mut self, seq: u64) -> Option<Waiting> {
        let waiting = self.waiting.remove(&seq)?;
        for owner in &waiting.owners {
            self.remove_membership(seq, owner);
        }
        Some(waiting)
    }

    /// Drop a sequence from every queue; for a queue entry whose ticket is already gone.
    fn forget_seq(&mut self, seq: u64) {
        self.no_scene.remove(&seq);
        self.scenes.retain(|_, queue| {
            queue.remove(&seq);
            !queue.is_empty()
        });
    }

    /// Whether a request under `hash` is alive: waiting, or a download with a reader of it.
    fn claimed(&self, hash: &str) -> bool {
        self.scenes.contains_key(hash)
            || self.readers.values().any(|readers| {
                readers
                    .owners
                    .keys()
                    .any(|owner| owner.as_deref() == Some(hash))
            })
    }

    /// A departed hash keeps ranking last only while one of its requests is alive.
    fn settle_departed(&mut self, owner: &Option<String>) {
        if let Some(hash) = owner {
            if self.departed.contains(hash) && !self.claimed(hash) {
                self.departed.remove(hash);
            }
        }
    }

    /// A registered hash ranks by its key; one whose holders all left while a request of it
    /// was alive ranks last; one nobody registered ranks with the requests that carry no hash.
    fn rank(&self, hash: &str) -> LoadPriority {
        match self.keys.get(hash) {
            Some(entry) => entry.key,
            None if self.departed.contains(hash) => LoadPriority::BOTTOM,
            None => LoadPriority::OTHER,
        }
    }

    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.no_scene.len() + self.scenes.values().map(BTreeSet::len).sum::<usize>()
    }

    /// The oldest ticket of the best-ranked queue: best key first, then the oldest head. A queue
    /// ranked in a held tier is passed over; a ticket queued under several owners is granted
    /// through any of them that is not held.
    fn best(&self) -> Option<u64> {
        let held = |key: &LoadPriority| self.held_from.is_some_and(|tier| key.tier >= tier);
        self.no_scene
            .first()
            .map(|seq| (LoadPriority::OTHER, *seq))
            .into_iter()
            .chain(
                self.scenes
                    .iter()
                    .filter_map(|(hash, queue)| Some((self.rank(hash), *queue.first()?))),
            )
            .filter(|(key, _)| !held(key))
            .min()
            .map(|(_, seq)| seq)
    }
}

#[cfg(test)]
mod tests;
