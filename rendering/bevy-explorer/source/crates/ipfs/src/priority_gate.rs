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

    pub fn set_priorities(&self, priorities: HashMap<String, LoadPriority>) {
        *self.priorities.write().unwrap() = priorities;
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
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use bevy::math::FloatOrd;
    use common::partition::LoadPriority;

    use super::*;

    fn key(parcel_distance: u16, is_behind: bool, sqr_distance: f32) -> LoadPriority {
        LoadPriority {
            parcel_distance,
            is_behind,
            sqr_distance: FloatOrd(sqr_distance),
        }
    }

    fn scenes(names: &[&str]) -> WaiterScenes {
        Arc::new(Mutex::new(names.iter().map(|n| (*n).to_owned()).collect()))
    }

    fn priorities(entries: &[(&str, LoadPriority)]) -> HashMap<String, LoadPriority> {
        entries
            .iter()
            .map(|(name, key)| ((*name).to_owned(), *key))
            .collect()
    }

    /// Yield until `count` waiters are queued; a waiter that never queues fails the test.
    async fn settle(gate: &Arc<PriorityGate>, count: usize) {
        for _ in 0..1000 {
            if gate.waiting() == count {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("{} waiters never queued", count);
    }

    type Log = Arc<Mutex<Vec<&'static str>>>;

    /// A reader that takes a slot for `scenes`, records `name`, and frees the slot.
    fn reader(
        gate: &Arc<PriorityGate>,
        name: &'static str,
        scenes: WaiterScenes,
        log: &Log,
    ) -> tokio::task::JoinHandle<()> {
        let gate = gate.clone();
        let log = log.clone();
        tokio::spawn(async move {
            let permit = gate.acquire(scenes).await.unwrap();
            log.lock().unwrap().push(name);
            drop(permit);
        })
    }

    #[tokio::test]
    async fn grants_the_nearest_in_front_scene_first() {
        let gate = PriorityGate::new(1);
        gate.set_priorities(priorities(&[
            ("far", key(5, false, 6400.0)),
            ("near", key(1, false, 100.0)),
            ("near_behind", key(1, true, 100.0)),
        ]));
        let holder = gate.try_acquire().unwrap();
        let log = Log::default();
        let tasks =
            ["far", "near", "near_behind"].map(|name| reader(&gate, name, scenes(&[name]), &log));
        settle(&gate, 3).await;

        drop(holder);
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(*log.lock().unwrap(), vec!["near", "near_behind", "far"]);
    }

    #[tokio::test]
    async fn a_key_change_is_honoured_at_grant_time() {
        let gate = PriorityGate::new(1);
        gate.set_priorities(priorities(&[
            ("a", key(5, false, 6400.0)),
            ("b", key(1, false, 100.0)),
        ]));
        let holder = gate.try_acquire().unwrap();
        let log = Log::default();
        let tasks = ["a", "b"].map(|name| reader(&gate, name, scenes(&[name]), &log));
        settle(&gate, 2).await;

        // the player turned round: a is now the near one
        gate.set_priorities(priorities(&[
            ("a", key(1, false, 100.0)),
            ("b", key(5, false, 6400.0)),
        ]));
        drop(holder);
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(*log.lock().unwrap(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn unknown_scene_is_top_priority() {
        let gate = PriorityGate::new(1);
        gate.set_priorities(priorities(&[("known", key(1, false, 100.0))]));
        let holder = gate.try_acquire().unwrap();
        let log = Log::default();
        let tasks = ["known", "unknown"].map(|name| reader(&gate, name, scenes(&[name]), &log));
        settle(&gate, 2).await;

        drop(holder);
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(*log.lock().unwrap(), vec!["unknown", "known"]);
    }

    #[tokio::test]
    async fn same_key_is_first_come_first_served() {
        let gate = PriorityGate::new(1);
        let holder = gate.try_acquire().unwrap();
        let log = Log::default();
        let tasks = ["first", "second"].map(|name| reader(&gate, name, scenes(&[]), &log));
        settle(&gate, 2).await;

        drop(holder);
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(*log.lock().unwrap(), vec!["first", "second"]);
    }

    #[tokio::test]
    async fn a_cancelled_waiter_does_not_hold_a_slot() {
        let gate = PriorityGate::new(1);
        gate.set_priorities(priorities(&[
            ("far", key(5, false, 6400.0)),
            ("near", key(1, false, 100.0)),
        ]));
        let holder = gate.try_acquire().unwrap();
        let log = Log::default();
        let far = reader(&gate, "far", scenes(&["far"]), &log);
        let near = reader(&gate, "near", scenes(&["near"]), &log);
        settle(&gate, 2).await;

        // the near reader is dropped before it is granted
        near.abort();
        assert!(near.await.unwrap_err().is_cancelled());
        drop(holder);
        far.await.unwrap();

        assert_eq!(*log.lock().unwrap(), vec!["far"]);
        assert_eq!(gate.waiting(), 0);
        assert!(gate.try_acquire().is_some(), "the slot came back");
    }

    #[tokio::test]
    async fn forget_permits_frees_only_what_is_available() {
        let gate = PriorityGate::new(2);
        let holder = gate.try_acquire().unwrap();

        assert_eq!(gate.forget_permits(2), 1);
        assert!(gate.try_acquire().is_none());

        drop(holder);
        let returned = gate.try_acquire();
        assert!(returned.is_some(), "the held slot returns");
        assert!(gate.try_acquire().is_none());

        drop(returned);
        gate.add_permits(1);
        let first = gate.try_acquire();
        let second = gate.try_acquire();
        assert!(first.is_some() && second.is_some());
    }

    #[tokio::test]
    async fn a_joined_reader_raises_the_download() {
        let gate = PriorityGate::new(1);
        gate.set_priorities(priorities(&[
            ("far", key(5, false, 6400.0)),
            ("mid", key(3, false, 2500.0)),
            ("near", key(1, false, 100.0)),
        ]));
        let holder = gate.try_acquire().unwrap();
        let log = Log::default();
        let far_scenes = scenes(&["far"]);
        let tasks = [
            reader(&gate, "far-download", far_scenes.clone(), &log),
            reader(&gate, "mid-download", scenes(&["mid"]), &log),
        ];
        settle(&gate, 2).await;

        // a reader from the near scene joins the far scene's download
        far_scenes.lock().unwrap().push("near".to_owned());
        drop(holder);
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(*log.lock().unwrap(), vec!["far-download", "mid-download"]);
    }
}
