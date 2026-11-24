//! The packets being routed at once. The participant's event loop hands each admitted packet
//! here and never waits for a slot: with every slot taken the packet is dropped and counted
//! `overloaded`, so the loop keeps draining the room's events (and sees a disconnect in time)
//! whatever the database or the SFU are doing. The slots belong to the process, not to one
//! connection, and a connection's packets are aborted ([`InFlight::abort`]) or drained
//! ([`InFlight::drain`]) before the next one routes, so no more than the limit ever run.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub struct InFlight {
    slots: Arc<Semaphore>,
    tasks: JoinSet<()>,
}

impl InFlight {
    /// At most `limit` (at least one) packets routed at once.
    pub fn new(limit: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(limit.max(1))),
            tasks: JoinSet::new(),
        }
    }

    /// Routes `work` when a slot is free; false, and `work` is dropped unstarted, when none is.
    pub fn try_spawn(&mut self, work: impl Future<Output = ()> + Send + 'static) -> bool {
        let Ok(slot) = self.slots.clone().try_acquire_owned() else {
            return false;
        };
        self.tasks.spawn(async move {
            work.await;
            drop(slot);
        });
        true
    }

    /// Waits for one routed packet to finish; pending forever when none is routing, so it can
    /// sit in a `select!` beside the room's events.
    pub async fn reap(&mut self) {
        match self.tasks.join_next().await {
            Some(Err(e)) if e.is_panic() => {
                tracing::error!(error = %e, "routing a packet panicked")
            }
            Some(_) => {}
            None => std::future::pending().await,
        }
    }

    /// Stops every packet being routed and waits until each has let go of its slot.
    pub async fn abort(&mut self) {
        self.tasks.shutdown().await;
    }

    /// Lets the packets being routed finish for up to `grace`, then aborts the rest.
    pub async fn drain(&mut self, grace: Duration) {
        let _ = tokio::time::timeout(grace, async {
            while self.tasks.join_next().await.is_some() {}
        })
        .await;
        self.abort().await;
    }

    /// Packets being routed.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Work that never ends, counting how many run at once.
    fn stuck(
        running: &Arc<AtomicUsize>,
        peak: &Arc<AtomicUsize>,
    ) -> impl Future<Output = ()> + Send + use<> {
        let (running, peak) = (running.clone(), peak.clone());
        async move {
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            struct Leave(Arc<AtomicUsize>);
            impl Drop for Leave {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _leave = Leave(running);
            std::future::pending::<()>().await;
        }
    }

    #[tokio::test]
    async fn a_packet_beyond_the_limit_is_refused_at_once_instead_of_waiting() {
        let (running, peak) = Default::default();
        let mut in_flight = InFlight::new(4);
        for _ in 0..4 {
            assert!(in_flight.try_spawn(stuck(&running, &peak)));
        }
        // every slot is held by work that never ends: the fifth is refused without waiting
        assert!(!in_flight.try_spawn(async { unreachable!("refused work never starts") }));
        tokio::task::yield_now().await;
        assert_eq!(running.load(Ordering::SeqCst), 4);
        assert_eq!(in_flight.len(), 4);
    }

    #[tokio::test]
    async fn aborting_a_connection_s_packets_frees_their_slots_before_the_next_routes() {
        let (running, peak): (Arc<AtomicUsize>, Arc<AtomicUsize>) = Default::default();
        let mut in_flight = InFlight::new(3);
        for _ in 0..3 {
            assert!(in_flight.try_spawn(stuck(&running, &peak)));
        }
        tokio::task::yield_now().await;
        // a reconnect: the old connection's packets stop before the new connection routes
        in_flight.abort().await;
        assert_eq!(running.load(Ordering::SeqCst), 0);
        assert!(in_flight.is_empty());
        for _ in 0..3 {
            assert!(in_flight.try_spawn(stuck(&running, &peak)));
        }
        assert!(!in_flight.try_spawn(async {}));
        tokio::task::yield_now().await;
        assert_eq!(peak.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn a_drain_lets_quick_packets_finish_and_aborts_stuck_ones_after_the_grace() {
        let (running, peak) = Default::default();
        let finished = Arc::new(AtomicUsize::new(0));
        let mut in_flight = InFlight::new(4);
        let done = finished.clone();
        assert!(in_flight.try_spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            done.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(in_flight.try_spawn(stuck(&running, &peak)));
        let started = tokio::time::Instant::now();
        in_flight.drain(Duration::from_secs(5)).await;
        assert_eq!(started.elapsed(), Duration::from_secs(5));
        assert_eq!(finished.load(Ordering::SeqCst), 1);
        assert_eq!(running.load(Ordering::SeqCst), 0);
        assert!(in_flight.is_empty());
    }

    #[tokio::test]
    async fn reaping_collects_finished_packets_and_frees_their_slots() {
        let mut in_flight = InFlight::new(1);
        assert!(in_flight.try_spawn(async {}));
        in_flight.reap().await;
        assert!(in_flight.is_empty());
        assert!(in_flight.try_spawn(async {}));
    }
}
