use std::{cell::Cell, future::Future, time::Duration};

use tokio::{
    sync::Semaphore,
    time::{error::Elapsed, timeout},
};

const MAX_CONCURRENT_FETCHES: usize = 5;

pub(super) struct SceneRequestBudget {
    outstanding: Cell<u64>,
    slots: Semaphore,
    server_timeout: Option<Duration>,
}

impl SceneRequestBudget {
    pub(super) fn for_scene(is_server: bool) -> Self {
        Self {
            outstanding: Cell::new(0),
            slots: Semaphore::new(MAX_CONCURRENT_FETCHES),
            server_timeout: is_server.then_some(Duration::from_secs(15)),
        }
    }

    pub(super) fn outstanding(&self) -> u64 {
        self.outstanding.get()
    }

    pub(super) fn track_call(&self) -> OutstandingCall<'_> {
        self.outstanding.set(self.outstanding.get() + 1);
        OutstandingCall(&self.outstanding)
    }

    pub(super) async fn run<F: Future>(&self, request: F) -> Result<F::Output, Elapsed> {
        let _slot = self.slots.acquire().await.expect("scene budget closed");
        match self.server_timeout {
            Some(limit) => timeout(limit, request).await,
            None => Ok(request.await),
        }
    }
}

pub(super) struct OutstandingCall<'a>(&'a Cell<u64>);
impl Drop for OutstandingCall<'_> {
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
        join_all((0..40).map(|_| async {
            let _call = budget.track_call();
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
            assert_eq!((a.outstanding(), b.outstanding()), (40, 40));

            drop(busy_a);
            assert_eq!(a.outstanding(), 0);
            assert!(a.run(ready(())).await.is_ok());
            assert_eq!(b.outstanding(), 40);
        }
    }

    #[tokio::test]
    async fn server_deadline_starts_only_after_admission() {
        let mut scene = SceneRequestBudget::for_scene(true);
        *scene.server_timeout.as_mut().unwrap() = Duration::from_millis(20);
        let _four_slots = scene.slots.acquire_many(4).await.unwrap();
        let last_slot = scene.slots.acquire().await.unwrap();
        let started = Cell::new(false);
        let mut queued = Box::pin(scene.run(async {
            started.set(true);
            pending::<()>().await
        }));
        let mut next = Box::pin(scene.run(ready(())));
        assert!(poll!(&mut queued).is_pending());
        assert!(poll!(&mut next).is_pending());
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(poll!(&mut queued).is_pending());
        assert!(!started.get());
        drop(last_slot);
        assert!(poll!(&mut next).is_pending(), "oldest waiter owns the slot");
        assert!(poll!(&mut queued).is_pending());
        assert!(started.get());
        assert!(queued.await.is_err());
        assert!(next.await.is_ok());
    }
}
