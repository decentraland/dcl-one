//! A value read from an outside source and kept: one good read answers for
//! its lifetime; a read that fails can't tell, and that answer is kept
//! [`FAILURE_TTL`] before the next attempt, never an empty value in place of
//! the failure.
//!
//! At most one read runs at a time, in a task of its own: it finishes and
//! keeps its answer (a failure or a panic included) even when every caller
//! that waited on it went away. [`Kept::get`] waits for it; [`Kept::get_since`]
//! accepts only a read that started at or after a [`Since`], whatever its
//! lifetime: a write's decision takes a read that started after it began, a
//! read-only decision also one that started up to [`READ_WINDOW`] before.

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;

/// How long a failed read answers "can't tell" before the next attempt.
pub const FAILURE_TTL: Duration = Duration::from_secs(30);
/// The whole read, every page, call and retry included.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
/// How long before a read-only decision began a read may have started and
/// still answer it ([`Since::read`]).
pub const READ_WINDOW: Duration = Duration::from_secs(5);

/// The oldest read start a decision accepts.
#[derive(Debug, Clone, Copy)]
pub struct Since(Instant);

impl Since {
    /// A write's decision: a read that started after it began.
    pub fn write() -> Self {
        Self(Instant::now())
    }

    /// A read-only decision: a read that started up to [`READ_WINDOW`]
    /// before it began.
    pub fn read() -> Self {
        let now = Instant::now();
        Self(now.checked_sub(READ_WINDOW).unwrap_or(now))
    }
}

struct Cache<T> {
    good: Option<(Instant, T)>,
    good_started: Option<Instant>,
    failed: Option<Instant>,
    reading: bool,
    reading_started: Option<Instant>,
}

impl<T> Cache<T> {
    fn fresh(&self, ttl: Duration) -> Option<&T> {
        self.good
            .as_ref()
            .filter(|(at, _)| at.elapsed() < ttl)
            .map(|(_, value)| value)
    }

    fn failing(&self) -> bool {
        self.failed.is_some_and(|at| at.elapsed() < FAILURE_TTL)
    }
}

struct Shared<T> {
    cache: Mutex<Cache<T>>,
    settled: Notify,
    ttl: Duration,
}

impl<T> Shared<T> {
    fn cache(&self) -> MutexGuard<'_, Cache<T>> {
        self.cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn settle(&self, read: Option<T>) {
        let mut cache = self.cache();
        match read {
            Some(value) => {
                cache.good = Some((Instant::now(), value));
                cache.good_started = cache.reading_started;
                cache.failed = None;
            }
            None => {
                cache.failed = Some(Instant::now());
                cache.good.take_if(|(at, _)| at.elapsed() >= self.ttl);
            }
        }
        cache.reading = false;
        drop(cache);
        self.settled.notify_waiters();
    }
}

/// Settles its read as failed if the task ends without settling it.
struct Unsettled<T>(Option<Arc<Shared<T>>>);

impl<T> Drop for Unsettled<T> {
    fn drop(&mut self) {
        if let Some(shared) = self.0.take() {
            shared.settle(None);
        }
    }
}

/// One source's read and its cache; a good read answers for `ttl`.
pub struct Kept<T> {
    shared: Arc<Shared<T>>,
}

impl<T: Clone + Send + 'static> Kept<T> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            shared: Arc::new(Shared {
                cache: Mutex::new(Cache {
                    good: None,
                    good_started: None,
                    failed: None,
                    reading: false,
                    reading_started: None,
                }),
                settled: Notify::new(),
                ttl,
            }),
        }
    }

    /// The kept value, waiting for `read` when none is kept; `None` when the
    /// source can't tell.
    pub async fn get<F>(&self, read: impl Fn() -> F) -> Option<T>
    where
        F: Future<Output = Option<T>> + Send + 'static,
    {
        loop {
            let settled = self.shared.settled.notified();
            tokio::pin!(settled);
            settled.as_mut().enable();
            {
                let mut cache = self.shared.cache();
                if let Some(value) = cache.fresh(self.shared.ttl) {
                    return Some(value.clone());
                }
                if cache.failing() {
                    return None;
                }
                if !cache.reading {
                    cache.reading = true;
                    cache.reading_started = Some(Instant::now());
                    self.spawn(read());
                }
            }
            let _ = tokio::time::timeout(FETCH_TIMEOUT + Duration::from_secs(1), settled).await;
        }
    }

    /// The value of a read that started at or after `since`, waiting for
    /// one (and starting it when none runs); `None` when the source can't
    /// tell. Concurrent callers share the read in flight when it started in
    /// time.
    pub async fn get_since<F>(&self, since: Since, read: impl Fn() -> F) -> Option<T>
    where
        F: Future<Output = Option<T>> + Send + 'static,
    {
        loop {
            let settled = self.shared.settled.notified();
            tokio::pin!(settled);
            settled.as_mut().enable();
            {
                let mut cache = self.shared.cache();
                if cache.good_started.is_some_and(|started| started >= since.0)
                    && let Some((_, value)) = &cache.good
                {
                    return Some(value.clone());
                }
                if cache.failing() {
                    return None;
                }
                if !cache.reading {
                    cache.reading = true;
                    cache.reading_started = Some(Instant::now());
                    self.spawn(read());
                }
            }
            let _ = tokio::time::timeout(FETCH_TIMEOUT + Duration::from_secs(1), settled).await;
        }
    }

    fn spawn(&self, read: impl Future<Output = Option<T>> + Send + 'static) {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            let mut unsettled = Unsettled(Some(shared));
            let read = read.await;
            if let Some(shared) = unsettled.0.take() {
                shared.settle(read);
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn backdate(&self, by: Duration) {
        let mut cache = self.shared.cache();
        if let Some((at, _)) = cache.good.as_mut() {
            *at = at.checked_sub(by).unwrap();
        }
        if let Some(at) = cache.good_started.as_mut() {
            *at = at.checked_sub(by).unwrap();
        }
        if let Some(at) = cache.failed.as_mut() {
            *at = at.checked_sub(by).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn counting(
        hits: &Arc<AtomicUsize>,
        delay: Duration,
        ok: bool,
    ) -> impl Fn() -> std::pin::Pin<Box<dyn Future<Output = Option<usize>> + Send>> + Send + use<>
    {
        let hits = hits.clone();
        move || {
            let hits = hits.clone();
            Box::pin(async move {
                let n = hits.fetch_add(1, Ordering::SeqCst) + 1;
                tokio::time::sleep(delay).await;
                ok.then_some(n)
            })
        }
    }

    #[tokio::test]
    async fn a_read_that_started_before_the_question_never_answers_it() {
        let kept = Kept::new(Duration::from_secs(3600));
        let hits = Arc::new(AtomicUsize::new(0));
        let read = counting(&hits, Duration::ZERO, true);
        assert_eq!(kept.get_since(Since::write(), &read).await, Some(1));
        assert_eq!(kept.get_since(Since::write(), &read).await, Some(2));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn questions_asked_before_a_read_started_share_it() {
        let kept = Arc::new(Kept::new(Duration::from_secs(3600)));
        let hits = Arc::new(AtomicUsize::new(0));
        let since = Since::write();
        let mut waiting = Vec::new();
        for _ in 0..20 {
            let kept = kept.clone();
            let read = counting(&hits, Duration::from_millis(50), true);
            waiting.push(tokio::spawn(
                async move { kept.get_since(since, read).await },
            ));
        }
        for answer in waiting {
            assert_eq!(answer.await.unwrap(), Some(1));
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_read_in_flight_when_asked_is_followed_by_one_that_started_after() {
        let kept = Arc::new(Kept::new(Duration::from_secs(3600)));
        let hits = Arc::new(AtomicUsize::new(0));
        let first = {
            let kept = kept.clone();
            let read = counting(&hits, Duration::from_millis(100), true);
            tokio::spawn(async move { kept.get_since(Since::write(), read).await })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        let read = counting(&hits, Duration::from_millis(10), true);
        assert_eq!(kept.get_since(Since::write(), read).await, Some(2));
        assert_eq!(first.await.unwrap(), Some(1));
    }

    #[tokio::test]
    async fn a_failed_read_answers_cant_tell_and_is_kept_before_the_next_attempt() {
        let kept = Kept::new(Duration::from_secs(3600));
        let hits = Arc::new(AtomicUsize::new(0));
        let failing = counting(&hits, Duration::ZERO, false);
        assert_eq!(kept.get_since(Since::write(), &failing).await, None);
        assert_eq!(kept.get_since(Since::write(), &failing).await, None);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        kept.backdate(FAILURE_TTL);
        let read = counting(&hits, Duration::ZERO, true);
        assert_eq!(kept.get_since(Since::write(), &read).await, Some(2));
    }

    #[tokio::test(start_paused = true)]
    async fn read_decisions_at_once_share_one_read_and_one_within_the_window_issues_none() {
        let kept = Arc::new(Kept::new(Duration::ZERO));
        let hits = Arc::new(AtomicUsize::new(0));
        let both: Vec<_> = (0..2)
            .map(|_| {
                let kept = kept.clone();
                let read = counting(&hits, Duration::from_millis(50), true);
                tokio::spawn(async move { kept.get_since(Since::read(), read).await })
            })
            .collect();
        for answer in both {
            assert_eq!(answer.await.unwrap(), Some(1));
        }
        tokio::time::advance(READ_WINDOW - Duration::from_millis(100)).await;
        let read = counting(&hits, Duration::ZERO, true);
        assert_eq!(kept.get_since(Since::read(), &read).await, Some(1));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_millis(100)).await;
        assert_eq!(kept.get_since(Since::read(), &read).await, Some(2));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_write_decision_just_after_a_read_takes_a_read_of_its_own() {
        let kept = Kept::new(Duration::ZERO);
        let hits = Arc::new(AtomicUsize::new(0));
        let read = counting(&hits, Duration::ZERO, true);
        assert_eq!(kept.get_since(Since::read(), &read).await, Some(1));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(kept.get_since(Since::write(), &read).await, Some(2));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}
