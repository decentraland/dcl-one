//! Tools for making actors collide on purpose: a shared start line, and seeded gaps so a race can
//! be replayed.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Barrier;
use tokio::task::JoinSet;

/// Runs `count` steps released from one barrier, so they start in the same instant. Results come
/// back in index order. A step that panics panics the caller. The steps live no longer than the
/// call: when the caller is dropped (a scenario's deadline) or a step panics, the rest are ended.
pub async fn together<T, F, Fut>(count: usize, step: F) -> Vec<T>
where
    F: Fn(usize) -> Fut,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let barrier = Arc::new(Barrier::new(count));
    // a set ends its tasks when it is dropped; a plain spawn would leave them running
    let mut steps = JoinSet::new();
    for index in 0..count {
        let barrier = barrier.clone();
        let work = step(index);
        steps.spawn(async move {
            barrier.wait().await;
            (index, work.await)
        });
    }
    let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
    while let Some(finished) = steps.join_next().await {
        let (index, result) = finished.expect("a step of `together` panicked");
        results[index] = Some(result);
    }
    results
        .into_iter()
        .map(|result| result.expect("every step of `together` finished"))
        .collect()
}

/// A small deterministic generator: the same seed gives the same gaps, on every platform.
#[derive(Debug, Clone)]
pub struct Seeded(u64);

impl Seeded {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn draw(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..bound`; 0 when `bound` is 0.
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 { 0 } else { self.draw() % bound }
    }

    /// A gap in `0..=window`, in whole microseconds.
    pub fn gap(&mut self, window: Duration) -> Duration {
        Duration::from_micros(self.below(window.as_micros() as u64 + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Instant;

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn together_releases_all_at_once() {
        let releases = together(32, |index| async move {
            // each step notes when it was let go
            (index, Instant::now())
        })
        .await;
        assert_eq!(releases.len(), 32);
        assert!(
            releases
                .iter()
                .enumerate()
                .all(|(position, (index, _))| position == *index)
        );
        let first = releases.iter().map(|(_, at)| *at).min().unwrap();
        let last = releases.iter().map(|(_, at)| *at).max().unwrap();
        assert!(
            last - first < Duration::from_millis(50),
            "spread {:?}",
            last - first
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn steps_end_when_their_caller_is_dropped() {
        let finished = Arc::new(AtomicU32::new(0));
        let steps = together(4, |_| {
            let finished = finished.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                finished.fetch_add(1, Ordering::SeqCst);
            }
        });
        // a scenario's deadline drops whatever the scenario was waiting on
        assert!(
            tokio::time::timeout(Duration::from_millis(50), steps)
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(
            finished.load(Ordering::SeqCst),
            0,
            "steps ran on after their caller was gone"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_step_that_panics_ends_the_others() {
        let finished = Arc::new(AtomicU32::new(0));
        let steps = together(4, |index| {
            let finished = finished.clone();
            async move {
                if index == 0 {
                    panic!("the first step breaks");
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
                finished.fetch_add(1, Ordering::SeqCst);
            }
        });
        let outcome = std::panic::AssertUnwindSafe(steps).catch_unwind().await;
        assert!(outcome.is_err(), "the caller must panic with its step");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(
            finished.load(Ordering::SeqCst),
            0,
            "steps ran on after one of them panicked"
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_gaps() {
        let gaps = |seed| {
            let mut seeded = Seeded::new(seed);
            (0..8)
                .map(|_| seeded.gap(Duration::from_millis(100)))
                .collect::<Vec<_>>()
        };
        assert_eq!(gaps(7), gaps(7));
        assert_ne!(gaps(7), gaps(8));
        assert!(gaps(7).iter().all(|gap| *gap <= Duration::from_millis(100)));
        assert_eq!(Seeded::new(1).below(0), 0);
    }
}
