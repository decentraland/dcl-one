//! Request slots handed out by priority instead of first come first served: the scene a
//! download belongs to decides, through the keys `scene_runner` publishes every frame, so the
//! nearest in-front scene's files download first.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use common::partition::LoadPriority;
use tokio::sync::oneshot;

/// The scenes a waiter downloads for: the first reader's, plus every reader that joined the
/// same download. Empty means not a scene's, which is top priority.
pub type WaiterScenes = Arc<Mutex<Vec<String>>>;

// lock order: `state`, then `priorities`, then a waiter's scenes; none held across an await
pub struct PriorityGate {
    state: Mutex<GateState>,
    priorities: RwLock<HashMap<String, LoadPriority>>,
}

struct GateState {
    available: usize,
    next_seq: u64,
    waiters: Vec<Waiter>,
}

struct Waiter {
    seq: u64,
    scenes: WaiterScenes,
    grant: oneshot::Sender<Permit>,
}

impl Waiter {
    /// The best key among the waiter's scenes; a scene without a published key is top.
    fn priority(&self, priorities: &HashMap<String, LoadPriority>) -> LoadPriority {
        self.scenes
            .lock()
            .unwrap()
            .iter()
            .map(|scene| priorities.get(scene).copied().unwrap_or(LoadPriority::TOP))
            .min()
            .unwrap_or(LoadPriority::TOP)
    }
}

/// A held request slot; dropping it frees the slot for the best waiter.
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
                waiters: Vec::new(),
            }),
            priorities: RwLock::new(HashMap::new()),
        })
    }

    /// A slot for a download of `scenes`, when one is free and no one else is waiting, or
    /// once it is this waiter's turn.
    pub async fn acquire(self: &Arc<Self>, scenes: WaiterScenes) -> Result<Permit, GateClosed> {
        let receiver = {
            let mut state = self.state.lock().unwrap();
            if state.available > 0 && state.waiters.is_empty() {
                state.available -= 1;
                return Ok(Permit { gate: self.clone() });
            }
            let (grant, receiver) = oneshot::channel();
            let seq = state.next_seq;
            state.next_seq += 1;
            state.waiters.push(Waiter { seq, scenes, grant });
            receiver
        };
        receiver.await.map_err(|_| GateClosed)
    }

    #[cfg(test)]
    pub fn try_acquire(self: &Arc<Self>) -> Option<Permit> {
        let mut state = self.state.lock().unwrap();
        if state.available > 0 && state.waiters.is_empty() {
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
    /// (despawned, out of range) keeps the bottom key for as long as a waiter names it, so
    /// its queued downloads do not count as unknown and jump to the top.
    pub fn set_priorities(&self, mut priorities: HashMap<String, LoadPriority>) {
        let state = self.state.lock().unwrap();
        let mut current = self.priorities.write().unwrap();
        for waiter in &state.waiters {
            for scene in waiter.scenes.lock().unwrap().iter() {
                if !priorities.contains_key(scene) && current.contains_key(scene) {
                    priorities.insert(scene.clone(), LoadPriority::BOTTOM);
                }
            }
        }
        *current = priorities;
    }

    pub fn scene_priority(&self, scene: &str) -> Option<LoadPriority> {
        self.priorities.read().unwrap().get(scene).copied()
    }

    #[cfg(test)]
    pub fn waiting(&self) -> usize {
        self.state.lock().unwrap().waiters.len()
    }

    fn release(self: &Arc<Self>) {
        self.state.lock().unwrap().available += 1;
        self.grant();
    }

    /// Hand free slots to the waiters with the best current key, oldest first on ties.
    fn grant(self: &Arc<Self>) {
        let mut state = self.state.lock().unwrap();
        // a reader that gave up (its download was cancelled) is no longer waiting
        state.waiters.retain(|waiter| !waiter.grant.is_closed());
        while state.available > 0 && !state.waiters.is_empty() {
            let best = {
                let priorities = self.priorities.read().unwrap();
                (0..state.waiters.len())
                    .min_by_key(|&i| {
                        let waiter = &state.waiters[i];
                        (waiter.priority(&priorities), waiter.seq)
                    })
                    .unwrap()
            };
            let waiter = state.waiters.swap_remove(best);
            state.available -= 1;
            if let Err(permit) = waiter.grant.send(Permit { gate: self.clone() }) {
                // the reader went away since the check above: take the slot back without
                // dropping the permit, whose drop would re-enter the lock held here
                std::mem::forget(permit);
                state.available += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests;
