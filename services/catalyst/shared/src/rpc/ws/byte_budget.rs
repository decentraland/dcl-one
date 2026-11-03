//! Byte admission for the guarded writer lane. A single large frame can occupy
//! an otherwise empty lane; another frame waits until charged bytes drain.
use std::sync::Arc;
use tokio::sync::Notify;

pub(super) struct ByteBudget {
    limit: usize,
    used: std::sync::Mutex<usize>,
    changed: Notify,
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
        })
    }
    pub(super) async fn reserve(self: &Arc<Self>, bytes: usize) -> Permit {
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
}
