use std::{cell::Cell, future::Future, time::Duration};

use tokio::{
    sync::Semaphore,
    time::{error::Elapsed, timeout},
};

const MAX_CONCURRENT_FETCHES: usize = 5;
const MAX_OUTSTANDING_FETCHES: u32 = 32;

pub(super) struct SceneRequestBudget {
    outstanding: Cell<u32>,
    slots: Semaphore,
    timeout: Duration,
}

impl SceneRequestBudget {
    pub(super) fn for_scene(is_server: bool) -> Self {
        Self {
            outstanding: Cell::new(0),
            slots: Semaphore::new(MAX_CONCURRENT_FETCHES),
            timeout: Duration::from_secs(if is_server { 15 } else { 30 }),
        }
    }

    pub(super) fn in_flight(&self) -> u32 {
        self.outstanding.get()
    }

    pub(super) fn try_start(&self) -> Option<InflightGuard<'_>> {
        if self.in_flight() >= MAX_OUTSTANDING_FETCHES {
            return None;
        }
        self.outstanding.set(self.in_flight() + 1);
        Some(InflightGuard(&self.outstanding))
    }

    pub(super) async fn run<F: Future>(&self, request: F) -> Result<F::Output, Elapsed> {
        timeout(self.timeout, async {
            let _slot = self.slots.acquire().await.expect("scene budget closed");
            request.await
        })
        .await
    }
}

pub(super) struct InflightGuard<'a>(&'a Cell<u32>);
impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deno_core::futures::{future::join_all, poll};
    use std::future::{pending, ready};

    async fn saturate(budget: &SceneRequestBudget, sends: &Cell<usize>) {
        join_all((0..32).map(|_| async {
            let _inflight = budget.try_start().unwrap();
            budget
                .run(async {
                    sends.set(sends.get() + 1);
                    pending::<()>().await;
                })
                .await
                .unwrap();
        }))
        .await;
    }

    #[tokio::test]
    async fn busy_scenes_keep_their_own_capacity() {
        for is_server in [false, true] {
            let a = SceneRequestBudget::for_scene(is_server);
            let b = SceneRequestBudget::for_scene(is_server);
            let sends_a = Cell::new(0);
            let sends_b = Cell::new(0);
            let mut busy_a = Box::pin(saturate(&a, &sends_a));
            let mut busy_b = Box::pin(saturate(&b, &sends_b));
            assert!(poll!(&mut busy_a).is_pending());
            assert!(poll!(&mut busy_b).is_pending());
            assert_eq!((sends_a.get(), sends_b.get()), (5, 5));
            assert!(a.try_start().is_none() && b.try_start().is_none());

            drop(busy_a);
            assert_eq!(a.in_flight(), 0);
            assert!(a.run(ready(())).await.is_ok());
            assert!(b.try_start().is_none());
        }
    }

    #[tokio::test]
    async fn deadline_covers_admission_and_response_body() {
        let mut scene = SceneRequestBudget::for_scene(false);
        scene.timeout = Duration::from_millis(20);
        let sends = Cell::new(0);
        let mut busy = Box::pin(saturate(&scene, &sends));
        assert!(poll!(&mut busy).is_pending());
        let queued = scene.run(ready(())).await;
        assert!(queued.is_err());
        drop(busy);
        assert!(scene.run(pending::<()>()).await.is_err());
        assert!(scene.run(ready(())).await.is_ok());
    }
}
