//! Byte admission for the guarded writer lane. A single large frame can occupy
//! an otherwise empty lane; another frame waits until charged bytes drain.
use std::sync::Arc;
use tokio::sync::{Notify, Semaphore};

pub(super) struct ByteBudget {
    limit: usize,
    used: std::sync::Mutex<usize>,
    changed: Notify,
    // A guarded waiter keeps its fair admission turn while existing bytes
    // drain. Ordinary retries cannot continually refill ahead of it.
    admission: Semaphore,
}
pub(super) struct Permit {
    budget: Arc<ByteBudget>,
    bytes: usize,
}
impl ByteBudget {
    pub(super) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: std::sync::Mutex::new(0),
            changed: Notify::new(),
            admission: Semaphore::new(1),
        })
    }
    pub(super) async fn reserve(self: &Arc<Self>, bytes: usize) -> Permit {
        let _turn = self.admission.acquire().await.unwrap();
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let mut used = self.used.lock().unwrap();
                if *used == 0 || bytes <= self.limit.saturating_sub(*used) {
                    *used += bytes;
                    return Permit {
                        budget: self.clone(),
                        bytes,
                    };
                }
            }
            changed.await;
        }
    }
    pub(super) fn try_reserve_buffered(self: &Arc<Self>, bytes: usize) -> Option<(Permit, bool)> {
        let _turn = self.admission.try_acquire().ok()?;
        let mut used = self.used.lock().unwrap();
        if *used > self.limit {
            return None;
        }
        *used = used.checked_add(bytes)?;
        Some((
            Permit {
                budget: self.clone(),
                bytes,
            },
            *used > self.limit,
        ))
    }
    pub(super) fn changed(&self) -> tokio::sync::futures::Notified<'_> {
        self.changed.notified()
    }
    pub(super) fn used(&self) -> usize {
        *self.used.lock().unwrap()
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        *self.budget.used.lock().unwrap() -= self.bytes;
        self.budget.changed.notify_waiters();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn one_large_frame_reserves_empty_lane_and_blocks_every_other_frame_until_drained() {
        let budget = ByteBudget::new(1024);
        let large = budget.reserve(32 * 1024).await;
        assert_eq!(budget.used(), 32 * 1024);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), budget.reserve(1))
                .await
                .is_err()
        );
        assert_eq!(budget.used(), 32 * 1024);
        drop(large);
        assert_eq!(budget.used(), 0);
        let first = budget.reserve(700).await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), budget.reserve(400))
                .await
                .is_err()
        );
        let second = budget.reserve(300).await;
        assert_eq!(budget.used(), 1000);
        drop(first);
        drop(second);
        assert_eq!(budget.used(), 0);
    }
    #[tokio::test]
    async fn queued_large_guard_keeps_ordinary_traffic_from_refilling_its_turn() {
        let budget = ByteBudget::new(1024);
        let (existing, _) = budget.try_reserve_buffered(700).unwrap();
        let large = budget.reserve(32 * 1024);
        tokio::pin!(large);
        assert!(futures::poll!(&mut large).is_pending());
        for _ in 0..1000 {
            assert!(budget.try_reserve_buffered(1).is_none());
        }
        assert_eq!(budget.used(), 700);
        drop(existing);
        let large = large.await;
        assert_eq!(budget.used(), 32 * 1024);
        assert!(budget.try_reserve_buffered(1).is_none());
        drop(large);
        let (ordinary, _) = budget.try_reserve_buffered(1).unwrap();
        drop(ordinary);
        assert_eq!(budget.used(), 0);
    }

    #[tokio::test]
    async fn guarded_admission_is_fifo_and_canceled_waiters_release_their_turn() {
        let budget = ByteBudget::new(1024);
        let existing = budget.reserve(700).await;
        let mut first = Box::pin(budget.reserve(2000));
        assert!(futures::poll!(&mut first).is_pending());
        let mut canceled = Box::pin(budget.reserve(3000));
        assert!(futures::poll!(&mut canceled).is_pending());
        let mut next = Box::pin(budget.reserve(4000));
        assert!(futures::poll!(&mut next).is_pending());
        drop(canceled);
        drop(existing);
        assert!(futures::poll!(&mut next).is_pending());
        let first = first.await;
        assert_eq!(budget.used(), 2000);
        assert!(budget.try_reserve_buffered(1).is_none());
        drop(first);
        let next = next.await;
        assert_eq!(budget.used(), 4000);
        drop(next);
        assert!(budget.try_reserve_buffered(1).is_some());
        assert_eq!(budget.used(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn expired_guard_holding_admission_releases_ordinary_and_next_guard() {
        let budget = ByteBudget::new(1024);
        let (existing, _) = budget.try_reserve_buffered(700).unwrap();
        let mut expired = Box::pin(tokio::time::timeout(
            std::time::Duration::from_secs(1),
            budget.reserve(32 * 1024),
        ));
        assert!(futures::poll!(&mut expired).is_pending());
        assert!(budget.try_reserve_buffered(1).is_none());
        assert!(expired.await.is_err());
        assert_eq!(budget.used(), 700);
        let (ordinary, _) = budget.try_reserve_buffered(1).unwrap();
        let next = budget.reserve(300).await;
        assert_eq!(budget.used(), 1001);
        drop(existing);
        drop(ordinary);
        drop(next);
        assert_eq!(budget.used(), 0);
    }
}
