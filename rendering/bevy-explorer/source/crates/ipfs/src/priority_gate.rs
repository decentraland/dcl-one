//! Request slots handed out by priority instead of first come first served: the scene a
//! download belongs to decides, through the keys `scene_runner` publishes every frame, so the
//! nearest in-front scene's files download first. Requests queue per scene, so a key change
//! re-ranks tens of queues, not every waiting request, and a grant pops one queue's head.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use common::partition::LoadPriority;
use tokio::sync::oneshot;

/// A request's place in the queue, taken when a read decides to download and awaited by the
/// download for its slot. A later reader of the same download queues the same ticket under its
/// own scene too (`PriorityGate::join`), so the download rises with the nearest of its readers
/// and is granted once.
pub struct Ticket {
    seq: u64,
    granted: AtomicBool,
    /// how many queues hold it: a ticket nobody else holds will never be awaited
    queued: AtomicUsize,
    grant: Mutex<Option<oneshot::Sender<Permit>>>,
    slot: Mutex<Option<oneshot::Receiver<Permit>>>,
}

pub struct PriorityGate {
    state: Mutex<GateState>,
}

struct GateState {
    available: usize,
    next_seq: u64,
    priorities: HashMap<String, LoadPriority>,
    /// requests that are not a scene's
    no_scene: VecDeque<Arc<Ticket>>,
    scenes: HashMap<String, SceneQueue>,
}

struct SceneQueue {
    tickets: VecDeque<Arc<Ticket>>,
    /// the scene has had a published key; without one again it has left, and ranks last
    published: bool,
}

/// A held request slot; dropping it frees the slot for the best waiting ticket.
pub struct Permit {
    gate: Arc<PriorityGate>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.gate.release();
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
                priorities: HashMap::new(),
                no_scene: VecDeque::new(),
                scenes: HashMap::new(),
            }),
        })
    }

    /// Queue a request of `scene` (`None`: not a scene's). A free slot with nobody ahead is
    /// granted at once; either way the slot arrives through `acquire`.
    pub fn ticket(self: &Arc<Self>, scene: Option<String>) -> Arc<Ticket> {
        let (grant, slot) = oneshot::channel();
        let ticket = {
            let mut state = self.state.lock().unwrap();
            let ticket = Arc::new(Ticket {
                seq: state.next_seq,
                granted: AtomicBool::new(false),
                queued: AtomicUsize::new(0),
                grant: Mutex::new(Some(grant)),
                slot: Mutex::new(Some(slot)),
            });
            state.next_seq += 1;
            state.enqueue(ticket.clone(), scene);
            ticket
        };
        self.grant();
        ticket
    }

    /// Queue the same ticket under another scene too: a reader of that scene joined its
    /// download.
    pub fn join(self: &Arc<Self>, ticket: &Arc<Ticket>, scene: Option<String>) {
        if ticket.granted.load(Ordering::Acquire) {
            return;
        }
        self.state.lock().unwrap().enqueue(ticket.clone(), scene);
        self.grant();
    }

    /// The ticket's slot, once it is its turn. A ticket is awaited once.
    pub async fn acquire(self: &Arc<Self>, ticket: Arc<Ticket>) -> Result<Permit, GateClosed> {
        let slot = ticket.slot.lock().unwrap().take().ok_or(GateClosed)?;
        slot.await.map_err(|_| GateClosed)
    }

    #[cfg(test)]
    pub fn try_acquire(self: &Arc<Self>) -> Option<Permit> {
        let mut state = self.state.lock().unwrap();
        if state.available > 0 && state.waiting() == 0 {
            state.available -= 1;
            Some(Permit { gate: self.clone() })
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

    /// Replace the published keys. A scene that was published before and is not any more
    /// (despawned) ranks last for as long as requests of its wait, so they do not count as
    /// unknown and jump to the top.
    pub fn set_priorities(&self, priorities: HashMap<String, LoadPriority>) {
        let mut state = self.state.lock().unwrap();
        for (hash, queue) in state.scenes.iter_mut() {
            if priorities.contains_key(hash) {
                queue.published = true;
            }
        }
        state.priorities = priorities;
    }

    pub fn scene_priority(&self, scene: &str) -> Option<LoadPriority> {
        self.state.lock().unwrap().priorities.get(scene).copied()
    }

    #[cfg(test)]
    pub fn waiting(&self) -> usize {
        self.state.lock().unwrap().waiting()
    }

    fn release(self: &Arc<Self>) {
        self.state.lock().unwrap().available += 1;
        self.grant();
    }

    /// Hand free slots to the heads of the best-ranked queues.
    fn grant(self: &Arc<Self>) {
        // dropped after the lock: a ticket may carry a permit in its channel, and a permit's
        // drop takes the lock again
        let mut done: Vec<Arc<Ticket>> = Vec::new();
        let mut unclaimed: Vec<Permit> = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            while state.available > 0 {
                let Some(ticket) = state.pop_best() else {
                    break;
                };
                let left = ticket.queued.fetch_sub(1, Ordering::AcqRel) - 1;
                // granted through another of its queues already, or held by nobody who will
                // await it: the read went away before it was awaited
                let abandoned = Arc::strong_count(&ticket) == left + 1;
                if ticket.granted.swap(true, Ordering::AcqRel) || abandoned {
                    done.push(ticket);
                    continue;
                }
                let Some(grant) = ticket.grant.lock().unwrap().take() else {
                    done.push(ticket);
                    continue;
                };
                state.available -= 1;
                if let Err(permit) = grant.send(Permit { gate: self.clone() }) {
                    // the reader stopped waiting: the slot comes back when `unclaimed` drops
                    unclaimed.push(permit);
                }
                done.push(ticket);
            }
        }
        drop(done);
        drop(unclaimed);
    }
}

impl GateState {
    fn enqueue(&mut self, ticket: Arc<Ticket>, scene: Option<String>) {
        ticket.queued.fetch_add(1, Ordering::AcqRel);
        match scene {
            None => self.no_scene.push_back(ticket),
            Some(hash) => {
                let published = self.priorities.contains_key(&hash);
                let queue = self.scenes.entry(hash).or_insert_with(|| SceneQueue {
                    tickets: VecDeque::new(),
                    published: false,
                });
                queue.published |= published;
                queue.tickets.push_back(ticket);
            }
        }
    }

    fn rank(&self, hash: &str, queue: &SceneQueue) -> LoadPriority {
        match self.priorities.get(hash) {
            Some(key) => *key,
            None if queue.published => LoadPriority::BOTTOM,
            None => LoadPriority::TOP,
        }
    }

    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.no_scene.len() + self.scenes.values().map(|q| q.tickets.len()).sum::<usize>()
    }

    /// The oldest ticket of the best-ranked queue: best key first, then the oldest head.
    fn pop_best(&mut self) -> Option<Arc<Ticket>> {
        self.scenes.retain(|_, queue| !queue.tickets.is_empty());
        let mut best: Option<(LoadPriority, u64, Option<&str>)> = self
            .no_scene
            .front()
            .map(|head| (LoadPriority::TOP, head.seq, None));
        for (hash, queue) in &self.scenes {
            let head = queue.tickets.front().expect("empty queues were dropped");
            let candidate = (self.rank(hash, queue), head.seq, Some(hash.as_str()));
            if best.is_none_or(|(rank, seq, _)| (candidate.0, candidate.1) < (rank, seq)) {
                best = Some(candidate);
            }
        }
        match best? {
            (_, _, None) => self.no_scene.pop_front(),
            (_, _, Some(hash)) => {
                let hash = hash.to_owned();
                self.scenes.get_mut(&hash)?.tickets.pop_front()
            }
        }
    }
}

#[cfg(test)]
mod tests;
