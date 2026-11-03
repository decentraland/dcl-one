//! Request slots handed out by priority instead of first come first served: the scene or avatar
//! a download belongs to decides, through the keys `scene_runner` keeps here, so the nearest
//! in-front scene's files download first. A hash is registered by the entity that owns it (a
//! scene root, each avatar wearing a wearable) and released when that entity goes, so the set
//! of keys follows the entities' lifecycle; the values are written in place on frames with a
//! download waiting. Requests queue per hash, so a key change re-ranks queues, not every waiting
//! request, and a grant pops one queue's head.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, Weak},
};

use common::partition::LoadPriority;
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
            let abandoned = gate.state.lock().unwrap().remove_ticket(self.seq);
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
    scenes: HashMap<String, SceneQueue>,
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
    ) -> (Registration, DownloadAttempts) {
        let mut state = gate.state.lock().unwrap();
        let id = state.next_download;
        state.next_download += 1;
        let ticket = gate.enqueue(&mut state, [owner.clone()]);
        state.readers.insert(
            id,
            ReaderState {
                owners: [(owner.clone(), 1)].into_iter().collect(),
                queued: Some(ticket.seq),
                cancel,
            },
        );
        drop(state);
        gate.grant();
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
                first: Some(ticket),
            },
        )
    }

    pub(super) fn try_register(&self, owner: Option<String>) -> Option<Registration> {
        let mut state = self.gate.state.lock().unwrap();
        let readers = state.readers.get_mut(&self.id)?;
        let count = readers.owners.entry(owner.clone()).or_default();
        *count += 1;
        if *count == 1 {
            if let Some(seq) = readers.queued {
                state.join(seq, owner.clone());
            }
        }
        Some(Registration {
            readers: self.clone(),
            owner,
        })
    }

    fn retry_ticket(&self) -> Result<Arc<Ticket>, GateClosed> {
        let mut state = self.gate.state.lock().unwrap();
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
        let ticket = match self.first.take() {
            Some(ticket) => ticket,
            None => self.readers.retry_ticket()?,
        };
        let permit = self.readers.gate.acquire(ticket).await?;
        let mut state = self.readers.gate.state.lock().unwrap();
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
        let mut state = self.readers.gate.state.lock().unwrap();
        let id = self.readers.id;
        let readers = state.readers.get_mut(&id).expect("registered download");
        let count = readers
            .owners
            .get_mut(&self.owner)
            .expect("registered reader");
        *count -= 1;
        if *count > 0 {
            return;
        }
        readers.owners.remove(&self.owner);
        let queued = readers.queued;
        if readers.owners.is_empty() {
            let cancelled = state.readers.remove(&id).and_then(|readers| readers.cancel);
            let abandoned = queued.and_then(|seq| state.remove_ticket(seq));
            // Both senders can wake tasks which immediately need the gate.
            drop(state);
            drop(cancelled);
            drop(abandoned);
        } else if let Some(seq) = queued {
            if let Some(waiting) = state.waiting.get_mut(&seq) {
                waiting.owners.retain(|owner| owner != &self.owner);
                state.remove_membership(seq, &self.owner);
            }
        }
    }
}

struct KeyEntry {
    key: LoadPriority,
    /// the entities that hold the hash: one scene root, or each avatar wearing it
    holders: usize,
    generation: u64,
}

struct SceneQueue {
    tickets: BTreeSet<u64>,
    /// every holder released the hash while these waited: the scene has left, and ranks last
    departed: bool,
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
                next_seq: 0,
                next_download: 0,
                readers: HashMap::new(),
                keys: HashMap::new(),
                generation: 0,
                no_scene: BTreeSet::new(),
                waiting: HashMap::new(),
                scenes: HashMap::new(),
            }),
        })
    }

    /// Queue a request of `scene` (`None`: a request with no hash, ranked after every registered
    /// one). A free slot with nobody ahead is granted at once; either way the slot arrives
    /// through `acquire`.
    pub fn ticket(self: &Arc<Self>, scene: Option<String>) -> Arc<Ticket> {
        let ticket = self.enqueue(&mut self.state.lock().unwrap(), [scene]);
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
            self.state.lock().unwrap().join(ticket.seq, owner);
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
        let mut state = self.state.lock().unwrap();
        if state.available > 0 && state.waiting.is_empty() {
            state.available -= 1;
            Some(Permit {
                gate: Some(self.clone()),
            })
        } else {
            None
        }
    }

    pub fn add_permits(self: &Arc<Self>, n: usize) {
        self.state.lock().unwrap().available += n;
        self.grant();
    }

    /// Forget up to `n` free slots; slots in use are not taken back. Returns how many went.
    pub fn forget_permits(&self, n: usize) -> usize {
        let mut state = self.state.lock().unwrap();
        let freed = n.min(state.available);
        state.available -= freed;
        freed
    }

    /// An entity that owns `hash` (a scene root, an avatar wearing it) has appeared: the hash
    /// ranks by `key`, or by the best key of its holders when it is held already.
    pub fn register(&self, hash: &str, key: LoadPriority) {
        let mut state = self.state.lock().unwrap();
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
    }

    /// An entity that owned `hash` has gone. With no holder left the key goes, and requests
    /// still queued under the hash rank last: they do not count as unknown.
    pub fn unregister(&self, hash: &str) {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.keys.get_mut(hash) else {
            return;
        };
        entry.holders -= 1;
        if entry.holders > 0 {
            return;
        }
        state.keys.remove(hash);
        if let Some(queue) = state.scenes.get_mut(hash) {
            queue.departed = !queue.tickets.is_empty();
        }
    }

    /// A frame's keys, written in place: the first write of a hash replaces its key, later
    /// writes in the same publish take the best, and a hash nobody registered is ignored.
    pub fn publish<'a>(&self, keys: impl IntoIterator<Item = (&'a str, LoadPriority)>) {
        let mut state = self.state.lock().unwrap();
        state.generation += 1;
        let generation = state.generation;
        for (hash, key) in keys {
            if let Some(entry) = state.keys.get_mut(hash) {
                if entry.generation == generation {
                    entry.key = entry.key.min(key);
                } else {
                    entry.key = key;
                    entry.generation = generation;
                }
            }
        }
    }

    /// Whether any request waits for a slot. While none does, the keys are not read, so the
    /// publisher skips the frame.
    pub fn has_waiters(&self) -> bool {
        let state = self.state.lock().unwrap();
        !state.no_scene.is_empty() || !state.scenes.is_empty()
    }

    pub fn scene_priority(&self, scene: &str) -> Option<LoadPriority> {
        self.state
            .lock()
            .unwrap()
            .keys
            .get(scene)
            .map(|entry| entry.key)
    }

    #[cfg(test)]
    pub fn waiting(&self) -> usize {
        self.state.lock().unwrap().waiting()
    }

    fn release(self: &Arc<Self>) {
        self.state.lock().unwrap().available += 1;
        self.grant();
    }

    /// Reserve under the gate lock and deliver outside it. Dropping an unclaimed permit
    /// re-enters the gate, and waking the receiver may schedule another download.
    pub(super) fn grant(self: &Arc<Self>) {
        loop {
            let grant = {
                let mut state = self.state.lock().unwrap();
                if state.available == 0 {
                    return;
                }
                let Some(seq) = state.best() else {
                    return;
                };
                let waiting = state.remove_ticket(seq).expect("queued ticket");
                state.available -= 1;
                waiting.grant
            };
            if let Err(mut permit) = grant.send(Permit {
                gate: Some(self.clone()),
            }) {
                // This loop returns and reassigns the unclaimed slot. Disarm its Drop so a
                // run of cancelled receivers cannot recurse through release -> grant.
                permit.gate.take();
                self.state.lock().unwrap().available += 1;
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
            Some(hash) => self
                .scenes
                .entry(hash)
                .or_insert_with(|| SceneQueue {
                    tickets: BTreeSet::new(),
                    departed: false,
                })
                .tickets
                .insert(seq),
        };
    }

    fn remove_membership(&mut self, seq: u64, owner: &Option<String>) {
        if let Some(hash) = owner {
            if let Some(queue) = self.scenes.get_mut(hash) {
                queue.tickets.remove(&seq);
                if queue.tickets.is_empty() {
                    self.scenes.remove(hash);
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

    /// A registered hash ranks by its key; one whose holders all left while these requests
    /// waited ranks last; one nobody registered ranks with the requests that carry no hash.
    fn rank(&self, hash: &str, queue: &SceneQueue) -> LoadPriority {
        match self.keys.get(hash) {
            Some(entry) => entry.key,
            None if queue.departed => LoadPriority::BOTTOM,
            None => LoadPriority::OTHER,
        }
    }

    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.no_scene.len() + self.scenes.values().map(|q| q.tickets.len()).sum::<usize>()
    }

    /// The oldest ticket of the best-ranked queue: best key first, then the oldest head.
    fn best(&self) -> Option<u64> {
        self.no_scene
            .first()
            .map(|seq| (LoadPriority::OTHER, *seq))
            .into_iter()
            .chain(self.scenes.iter().filter_map(|(hash, queue)| {
                Some((self.rank(hash, queue), *queue.tickets.first()?))
            }))
            .min()
            .map(|(_, seq)| seq)
    }
}

#[cfg(test)]
mod tests;
