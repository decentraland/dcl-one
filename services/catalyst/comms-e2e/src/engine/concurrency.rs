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
        let micros = u64::try_from(window.as_micros()).unwrap_or(u64::MAX);
        Duration::from_micros(self.below(micros.saturating_add(1)))
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
        const STEPS: u32 = 32;
        let prepared = Arc::new(AtomicU32::new(0));
        let started = Arc::new(AtomicU32::new(0));
        let releases = together(STEPS as usize, |index| {
            // preparing a step takes a while, so a step let go early would see the count short
            std::thread::sleep(Duration::from_millis(5));
            prepared.fetch_add(1, Ordering::SeqCst);
            let (prepared, started) = (prepared.clone(), started.clone());
            async move {
                // how many steps existed when this one was let go: all of them
                let prepared_when_released = prepared.load(Ordering::SeqCst);
                // and none waits for another to finish: every step is running at once
                started.fetch_add(1, Ordering::SeqCst);
                let deadline = Instant::now() + Duration::from_secs(30);
                while started.load(Ordering::SeqCst) < STEPS && Instant::now() < deadline {
                    tokio::task::yield_now().await;
                }
                (
                    index,
                    prepared_when_released,
                    started.load(Ordering::SeqCst),
                )
            }
        })
        .await;
        assert_eq!(releases.len(), STEPS as usize);
        for (position, (index, prepared_when_released, running)) in releases.iter().enumerate() {
            assert_eq!(position, *index);
            assert_eq!(
                *prepared_when_released, STEPS,
                "step {index} began before every step was ready"
            );
            assert_eq!(*running, STEPS, "step {index} never saw every step running");
        }
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
