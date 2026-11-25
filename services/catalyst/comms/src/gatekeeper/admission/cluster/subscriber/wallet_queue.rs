use futures::FutureExt as _;
use parking_lot::{Mutex, MutexGuard};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

pub const MAX_TOTAL_WORK: usize = 1_024;
pub const MAX_WALLET_WORK: usize = 16;
pub const MAX_RUNNING_WORK: usize = 32;
const MAX_MAINTENANCE_WORK: usize = 128;
const MAX_RUNNING_MAINTENANCE: usize = 16;
const MAX_COLD_MAINTENANCE_WORK: usize = 112;
const MAX_RUNNING_COLD_MAINTENANCE: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MaintenanceClass {
    Urgent,
    Cold,
}
struct Maintenance {
    _item: OwnedSemaphorePermit,
    _cold_item: Option<OwnedSemaphorePermit>,
    class: MaintenanceClass,
}
/// Contains the maximum 60 s token lifetime, LiveKit's 60 s verifier leeway, a two-second
/// boundary margin, two removal phases and the source checks surrounding replacement minting.
pub const WORK_DEADLINE: Duration = Duration::from_secs(150);

pub struct WorkCredit {
    _permit: Option<OwnedSemaphorePermit>,
    _maintenance: Option<OwnedSemaphorePermit>,
    _cold: Option<OwnedSemaphorePermit>,
    released: Option<Arc<Notify>>,
}
impl WorkCredit {
    pub(crate) fn reserve(budget: &Arc<Semaphore>) -> Option<Arc<Self>> {
        Self::reserve_with(budget, None, None, None)
    }
    fn reserve_with(
        budget: &Arc<Semaphore>,
        maintenance: Option<OwnedSemaphorePermit>,
        cold: Option<OwnedSemaphorePermit>,
        released: Option<Arc<Notify>>,
    ) -> Option<Arc<Self>> {
        budget.clone().try_acquire_owned().ok().map(|permit| {
            Arc::new(Self {
                _permit: Some(permit),
                _maintenance: maintenance,
                _cold: cold,
                released,
            })
        })
    }
}
impl Drop for WorkCredit {
    fn drop(&mut self) {
        drop(self._cold.take());
        drop(self._maintenance.take());
        drop(self._permit.take());
        if let Some(released) = &self.released {
            released.notify_one();
        }
    }
}
tokio::task_local! { static CURRENT_CREDIT: Arc<WorkCredit>; }
pub(crate) fn current_work_credit() -> Option<Arc<WorkCredit>> {
    CURRENT_CREDIT.try_with(Arc::clone).ok()
}

pub(super) type QueuedWork = futures::future::BoxFuture<'static, ()>;
pub(super) type Completion = Box<dyn FnOnce() + Send>;

#[derive(Default)]
pub(super) struct WorkOwner {
    cancelled: CancellationToken,
    outstanding: AtomicUsize,
    idle: Notify,
}

impl WorkOwner {
    pub(super) async fn settle(&self) {
        loop {
            let idle = self.idle.notified();
            tokio::pin!(idle);
            idle.as_mut().enable();
            if self.outstanding.load(Ordering::SeqCst) == 0 {
                return;
            }
            idle.await;
        }
    }
}

struct Ownership(Arc<WorkOwner>);

impl Drop for Ownership {
    fn drop(&mut self) {
        self.0.outstanding.fetch_sub(1, Ordering::SeqCst);
        self.0.idle.notify_waiters();
    }
}

struct Completed(Option<Completion>);

impl Drop for Completed {
    fn drop(&mut self) {
        if let Some(done) = self.0.take()
            && std::panic::catch_unwind(std::panic::AssertUnwindSafe(done)).is_err()
        {
            tracing::error!("cluster completion panicked");
        }
    }
}

struct Work {
    future: Option<QueuedWork>,
    _completed: Completed,
    owner: Ownership,
    expires: Instant,
    credit: Arc<WorkCredit>,
    maintenance: Option<Maintenance>,
}

#[derive(Default)]
struct Lane {
    pending: VecDeque<Work>,
    running: bool,
}

#[derive(Default)]
struct State {
    lanes: HashMap<String, Lane>,
    ready: VecDeque<String>,
    running: usize,
    running_maintenance: usize,
    running_cold: usize,
    total: usize,
}

/// Bounded FIFO per wallet, round-robin between wallets. At most one item per
/// wallet executes at once, and only a bounded number of tasks exist globally.
/// Admission is synchronous, before scheduling, so accepted arrival order survives.
/// No replica affinity or distributed publication ordering is implied.
pub struct WalletQueue {
    state: Mutex<State>,
    unscoped_owner: Arc<WorkOwner>,
    credits: Arc<Semaphore>,
    maintenance_items: Arc<Semaphore>,
    maintenance_credits: Arc<Semaphore>,
    cold_items: Arc<Semaphore>,
    cold_credits: Arc<Semaphore>,
    capacity: Arc<Notify>,
}

impl Default for WalletQueue {
    fn default() -> Self {
        Self {
            state: Default::default(),
            unscoped_owner: Default::default(),
            credits: Arc::new(Semaphore::new(MAX_TOTAL_WORK)),
            maintenance_items: Arc::new(Semaphore::new(MAX_MAINTENANCE_WORK)),
            maintenance_credits: Arc::new(Semaphore::new(MAX_MAINTENANCE_WORK)),
            cold_items: Arc::new(Semaphore::new(MAX_COLD_MAINTENANCE_WORK)),
            cold_credits: Arc::new(Semaphore::new(MAX_COLD_MAINTENANCE_WORK)),
            capacity: Arc::new(Notify::new()),
        }
    }
}

impl WalletQueue {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }

    /// Rejects new work at either cap. Rejection drops its captured state and runs
    /// its completion outside the queue lock; accepted work gets the same cleanup
    /// on success, panic or its queue-inclusive deadline. Completion follows lane
    /// retirement when this was the wallet's last item.
    pub fn enqueue(self: &Arc<Self>, wallet: &str, work: QueuedWork, done: Completion) -> bool {
        self.enqueue_scoped(self.unscoped_owner.clone(), wallet, work, done)
    }

    pub(super) fn enqueue_scoped(
        self: &Arc<Self>,
        owner: Arc<WorkOwner>,
        wallet: &str,
        work: QueuedWork,
        done: Completion,
    ) -> bool {
        self.enqueue_scoped_with_credit(owner, wallet, work, done, None)
    }

    pub(super) fn enqueue_scoped_with_credit(
        self: &Arc<Self>,
        owner: Arc<WorkOwner>,
        wallet: &str,
        work: QueuedWork,
        done: Completion,
        retained: Option<Arc<WorkCredit>>,
    ) -> bool {
        self.enqueue_class(owner, wallet, work, done, retained, None)
    }

    #[cfg(test)]
    pub(super) fn enqueue_maintenance(
        self: &Arc<Self>,
        owner: Arc<WorkOwner>,
        wallet: &str,
        work: QueuedWork,
        done: Completion,
        retained: Option<Arc<WorkCredit>>,
    ) -> bool {
        self.enqueue_maintenance_class(owner, wallet, work, done, retained, MaintenanceClass::Cold)
    }

    pub(super) fn enqueue_maintenance_class(
        self: &Arc<Self>,
        owner: Arc<WorkOwner>,
        wallet: &str,
        work: QueuedWork,
        done: Completion,
        retained: Option<Arc<WorkCredit>>,
        class: MaintenanceClass,
    ) -> bool {
        let items = || {
            let cold = if class == MaintenanceClass::Cold {
                Some(self.cold_items.clone().try_acquire_owned().ok()?)
            } else {
                None
            };
            Some(Maintenance {
                _item: self.maintenance_items.clone().try_acquire_owned().ok()?,
                _cold_item: cold,
                class,
            })
        };
        let Some(item) = items() else {
            drop(work);
            drop(Completed(Some(done)));
            return false;
        };
        self.enqueue_class(owner, wallet, work, done, retained, Some(item))
    }

    pub(super) fn maintenance_capacity(&self) -> usize {
        self.maintenance_items.available_permits()
    }
    pub(super) fn cold_maintenance_capacity(&self) -> usize {
        self.maintenance_capacity()
            .min(self.cold_items.available_permits())
    }

    pub(crate) async fn capacity_changed(&self) {
        self.capacity.notified().await;
    }

    fn enqueue_class(
        self: &Arc<Self>,
        owner: Arc<WorkOwner>,
        wallet: &str,
        work: QueuedWork,
        done: Completion,
        retained: Option<Arc<WorkCredit>>,
        maintenance: Option<Maintenance>,
    ) -> bool {
        let credit = retained.or_else(|| {
            let maintenance_credit = if maintenance.is_some() {
                Some(self.maintenance_credits.clone().try_acquire_owned().ok()?)
            } else {
                None
            };
            let cold_credit = if maintenance
                .as_ref()
                .is_some_and(|item| item.class == MaintenanceClass::Cold)
            {
                Some(self.cold_credits.clone().try_acquire_owned().ok()?)
            } else {
                None
            };
            WorkCredit::reserve_with(
                &self.credits,
                maintenance_credit,
                cold_credit,
                Some(self.capacity.clone()),
            )
        });
        let Some(credit) = credit else {
            crate::gatekeeper::admission::cluster::metrics::cluster_work_rejected("global_full");
            drop(work);
            drop(Completed(Some(done)));
            return false;
        };
        owner.outstanding.fetch_add(1, Ordering::SeqCst);
        let mut work = Some(Work {
            future: Some(work),
            _completed: Completed(Some(done)),
            owner: Ownership(owner.clone()),
            expires: Instant::now() + WORK_DEADLINE,
            credit,
            maintenance,
        });
        let scheduled = {
            let mut state = self.state();
            if owner.cancelled.is_cancelled() {
                Err("retired")
            } else if state.total >= MAX_TOTAL_WORK {
                Err("global_full")
            } else if state.lanes.get(wallet).is_some_and(|lane| {
                lane.pending.len() + usize::from(lane.running) >= MAX_WALLET_WORK
            }) {
                Err("wallet_full")
            } else {
                if !state.lanes.contains_key(wallet) {
                    state.ready.push_back(wallet.to_owned());
                }
                state
                    .lanes
                    .entry(wallet.to_owned())
                    .or_default()
                    .pending
                    .push_back(work.take().unwrap());
                state.total += 1;
                Ok(Self::take_ready(&mut state))
            }
        };
        match scheduled {
            Ok(ready) => {
                self.start_ready(ready);
                true
            }
            Err(reason) => {
                crate::gatekeeper::admission::cluster::metrics::cluster_work_rejected(reason);
                drop(work);
                false
            }
        }
    }

    fn take_ready(state: &mut State) -> Vec<(String, Work)> {
        let mut ready = Vec::new();
        while state.running < MAX_RUNNING_WORK {
            // Events and durable takeovers choose a ready lane first. Never overtake
            // an earlier item in the same wallet's FIFO.
            let event = state.ready.iter().position(|wallet| {
                state.lanes[wallet]
                    .pending
                    .front()
                    .is_some_and(|work| work.maintenance.is_none())
            });
            let next = event.or_else(|| {
                if state.running_maintenance >= MAX_RUNNING_MAINTENANCE {
                    return None;
                }
                let urgent = state.ready.iter().position(|wallet| {
                    state.lanes[wallet]
                        .pending
                        .front()
                        .and_then(|work| work.maintenance.as_ref())
                        .is_some_and(|item| item.class == MaintenanceClass::Urgent)
                });
                urgent.or_else(|| {
                    if state.running_cold >= MAX_RUNNING_COLD_MAINTENANCE {
                        return None;
                    }
                    state.ready.iter().position(|wallet| {
                        state.lanes[wallet]
                            .pending
                            .front()
                            .and_then(|work| work.maintenance.as_ref())
                            .is_some_and(|item| item.class == MaintenanceClass::Cold)
                    })
                })
            });
            let Some(index) = next else { break };
            let wallet = state.ready.remove(index).expect("ready lane exists");
            let lane = state
                .lanes
                .get_mut(&wallet)
                .expect("ready wallet has a lane");
            debug_assert!(!lane.running);
            let work = lane
                .pending
                .pop_front()
                .expect("ready wallet has pending work");
            lane.running = true;
            state.running += 1;
            state.running_maintenance += usize::from(work.maintenance.is_some());
            state.running_cold += usize::from(
                work.maintenance
                    .as_ref()
                    .is_some_and(|item| item.class == MaintenanceClass::Cold),
            );
            ready.push((wallet, work));
        }
        crate::gatekeeper::admission::cluster::metrics::cluster_work_depths(
            state.running,
            state.total - state.running,
        );
        ready
    }

    fn start_ready(self: &Arc<Self>, ready: Vec<(String, Work)>) {
        for (wallet, work) in ready {
            let queue = self.clone();
            crate::shared::task::spawn(async move { queue.run(wallet, work).await });
        }
    }

    async fn run(self: Arc<Self>, wallet: String, mut work: Work) {
        if work.owner.0.cancelled.is_cancelled() {
            crate::gatekeeper::admission::cluster::metrics::cluster_work_cancelled();
        } else if Instant::now() >= work.expires {
            crate::gatekeeper::admission::cluster::metrics::cluster_work_expired();
        } else {
            let future = work.future.take().unwrap();
            CURRENT_CREDIT.scope(work.credit.clone(), async { tokio::select! {
                biased;
                _ = work.owner.0.cancelled.cancelled() => crate::gatekeeper::admission::cluster::metrics::cluster_work_cancelled(),
                _ = tokio::time::sleep_until(work.expires) => crate::gatekeeper::admission::cluster::metrics::cluster_work_expired(),
                result = std::panic::AssertUnwindSafe(future).catch_unwind() => {
                    if result.is_err() {
                        tracing::error!(%wallet, "cluster work panicked; the lane continues");
                    }
                }
            }}).await;
        }
        let ready = {
            let mut state = self.state();
            let lane = state
                .lanes
                .get_mut(&wallet)
                .expect("running wallet has a lane");
            lane.running = false;
            if lane.pending.is_empty() {
                state.lanes.remove(&wallet);
            } else {
                state.ready.push_back(wallet);
            }
            state.running -= 1;
            state.running_maintenance -= usize::from(work.maintenance.is_some());
            state.running_cold -= usize::from(
                work.maintenance
                    .as_ref()
                    .is_some_and(|item| item.class == MaintenanceClass::Cold),
            );
            state.total -= 1;
            Self::take_ready(&mut state)
        };
        drop(work);
        self.start_ready(ready);
        self.capacity.notify_one();
    }

    pub(super) fn cancel(self: &Arc<Self>, owner: &Arc<WorkOwner>) {
        let removed = {
            let mut state = self.state();
            owner.cancelled.cancel();
            let mut removed = Vec::new();
            state.lanes.retain(|_, lane| {
                let mut kept = VecDeque::new();
                for work in lane.pending.drain(..) {
                    if Arc::ptr_eq(&work.owner.0, owner) {
                        removed.push(work);
                    } else {
                        kept.push_back(work);
                    }
                }
                lane.pending = kept;
                lane.running || !lane.pending.is_empty()
            });
            let State { ready, lanes, .. } = &mut *state;
            ready.retain(|wallet| lanes.contains_key(wallet));
            state.total -= removed.len();
            crate::gatekeeper::admission::cluster::metrics::cluster_work_depths(
                state.running,
                state.total - state.running,
            );
            removed
        };
        for work in removed {
            crate::gatekeeper::admission::cluster::metrics::cluster_work_cancelled();
            drop(work);
        }
    }

    pub fn tracked_wallets(&self) -> usize {
        self.state().lanes.len()
    }

    pub fn outstanding_work(&self) -> usize {
        self.state().total
    }

    pub fn running_work(&self) -> usize {
        self.state().running
    }

    #[cfg(test)]
    pub(super) fn panic_while_locked_for_test(&self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = self.state.lock();
            panic!("panic while holding the registry");
        }));
    }
}

#[cfg(test)]
mod tests;
