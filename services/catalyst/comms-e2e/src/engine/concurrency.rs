//! Tools for making actors collide on purpose: a shared start line, and seeded gaps so a race can
//! be replayed.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Barrier;

/// Runs `count` steps released from one barrier, so they start in the same instant. Results come
/// back in index order. A step that panics panics the caller.
pub async fn together<T, F, Fut>(count: usize, step: F) -> Vec<T>
where
    F: Fn(usize) -> Fut,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let barrier = Arc::new(Barrier::new(count));
    let tasks: Vec<_> = (0..count)
        .map(|index| {
            let barrier = barrier.clone();
            let work = step(index);
            tokio::spawn(async move {
                barrier.wait().await;
                work.await
            })
        })
        .collect();
    let mut results = Vec::with_capacity(count);
    for task in tasks {
        results.push(task.await.expect("a step of `together` panicked"));
    }
    results
}

/// A small deterministic generator: the same seed gives the same gaps, on every platform.
#[derive(Debug, Clone)]
pub struct Seeded(u64);

impl Seeded {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..bound`; 0 when `bound` is 0.
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 { 0 } else { self.next() % bound }
    }

    /// A gap in `0..=window`, in whole microseconds.
    pub fn gap(&mut self, window: Duration) -> Duration {
        Duration::from_micros(self.below(window.as_micros() as u64 + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
