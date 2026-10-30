//! A bounded TTL memo with single-flight fetches: concurrent misses on one key
//! elect a leader and park the rest on a [`Notify`]; the leader always wakes
//! them, including when its task is cancelled mid-fetch (`LeaderGuard`), so a
//! dropped caller can never strand the others. Waiters park with a short
//! re-check bound as well, so a wakeup that lands before a waiter registers
//! costs one extra loop rather than the whole request.
//!
//! [`TtlCache`] is the simpler keyed memo behind a std mutex (no
//! single-flight), recomputed at most once per TTL.

use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::Notify;
use tokio::time::Instant;

struct Entry<V> {
    at: Instant,
    value: V,
}

struct Slot<V> {
    cached: Option<Entry<V>>,
    notify: Option<Arc<Notify>>,
}

impl<V> Slot<V> {
    fn empty() -> Self {
        Self {
            cached: None,
            notify: None,
        }
    }
}

enum Action {
    Wait(Arc<Notify>),
    Lead(Arc<Notify>),
}

const WAIT_RECHECK: Duration = Duration::from_millis(250);

pub struct TtlMap<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    map: DashMap<K, Slot<V>>,
    ttl: Duration,
    max_entries: Option<usize>,
}

struct LeaderGuard<'a, K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache: &'a TtlMap<K, V>,
    key: K,
    notify: Arc<Notify>,
    finished: bool,
}

impl<K, V> Drop for LeaderGuard<'_, K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Some(mut slot) = self.cache.map.get_mut(&self.key) {
            slot.notify = None;
        }
        self.notify.notify_waiters();
    }
}

impl<K, V> TtlMap<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(ttl: Duration) -> Self {
        Self {
            map: DashMap::new(),
            ttl,
            max_entries: None,
        }
    }

    pub fn bounded(ttl: Duration, max_entries: usize) -> Self {
        Self {
            max_entries: Some(max_entries),
            ..Self::new(ttl)
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&self) {
        self.map.clear();
    }

    pub fn get_fresh(&self, key: &K) -> Option<V> {
        let slot = self.map.get(key)?;
        let entry = slot.cached.as_ref()?;
        (entry.at.elapsed() < self.ttl).then(|| entry.value.clone())
    }

    pub fn insert(&self, key: K, value: V) {
        let mut slot = self.map.entry(key).or_insert_with(Slot::empty);
        slot.cached = Some(Entry {
            at: Instant::now(),
            value,
        });
    }

    pub async fn get_or_fetch<F, Fut, E>(&self, key: K, fetch: F) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        let mut fetch = Some(fetch);
        loop {
            let action = {
                let mut entry = self.map.entry(key.clone()).or_insert_with(Slot::empty);
                if let Some(cached) = entry.cached.as_ref()
                    && cached.at.elapsed() < self.ttl
                {
                    return Ok(cached.value.clone());
                }
                if let Some(notify) = entry.notify.as_ref() {
                    Action::Wait(notify.clone())
                } else {
                    let notify = Arc::new(Notify::new());
                    entry.notify = Some(notify.clone());
                    Action::Lead(notify)
                }
            };

            match action {
                Action::Wait(notify) => {
                    let _ = tokio::time::timeout(WAIT_RECHECK, notify.notified()).await;
                    continue;
                }
                Action::Lead(notify) => {
                    let mut guard = LeaderGuard {
                        cache: self,
                        key: key.clone(),
                        notify: notify.clone(),
                        finished: false,
                    };

                    if let Some(max) = self.max_entries
                        && self.map.len() > max
                    {
                        self.map.clear();
                        self.map.insert(
                            key.clone(),
                            Slot {
                                cached: None,
                                notify: Some(notify.clone()),
                            },
                        );
                    }

                    let fetch = fetch.take().expect("a leader fetches once");
                    let result = fetch().await;
                    match &result {
                        Ok(value) => {
                            let entry = Entry {
                                at: Instant::now(),
                                value: value.clone(),
                            };
                            match self.map.get_mut(&key) {
                                Some(mut slot) => {
                                    slot.cached = Some(entry);
                                    slot.notify = None;
                                }
                                None => {
                                    self.map.insert(
                                        key.clone(),
                                        Slot {
                                            cached: Some(entry),
                                            notify: None,
                                        },
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            if let Some(mut slot) = self.map.get_mut(&key) {
                                slot.notify = None;
                            }
                        }
                    }
                    guard.finished = true;
                    notify.notify_waiters();
                    return result;
                }
            }
        }
    }
}

/// A keyed TTL memo behind a mutex: the category counts, the parcel aliases and
/// the moderator set are recomputed at most once per TTL.
pub struct Cached<T> {
    pub value: T,
    pub expires_at: std::time::Instant,
}

pub type TtlCache<T> = std::sync::Mutex<std::collections::HashMap<String, Cached<T>>>;

pub fn cache_get<T: Clone>(cache: &TtlCache<T>, key: &str) -> Option<T> {
    let cache = cache.lock().unwrap();
    cache
        .get(key)
        .filter(|c| c.expires_at > std::time::Instant::now())
        .map(|c| c.value.clone())
}

pub fn cache_put<T>(cache: &TtlCache<T>, key: String, value: T, ttl: Duration) {
    let mut cache = cache.lock().unwrap();
    let now = std::time::Instant::now();
    cache.retain(|_, c| c.expires_at > now);
    cache.insert(
        key,
        Cached {
            value,
            expires_at: now + ttl,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn fresh_hits_skip_the_fetch_and_clear_forces_one() {
        let map: TtlMap<String, u32> = TtlMap::bounded(Duration::from_secs(5), 10);
        let calls = AtomicUsize::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(7)
        };
        assert_eq!(map.get_or_fetch("k".into(), fetch).await, Ok(7));
        assert_eq!(map.get_or_fetch("k".into(), fetch).await, Ok(7));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        map.clear();
        assert_eq!(map.get_or_fetch("k".into(), fetch).await, Ok(7));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_failed_fetch_is_not_cached() {
        let map: TtlMap<String, u32> = TtlMap::new(Duration::from_secs(5));
        let failed = map
            .get_or_fetch("k".into(), || async { Err::<u32, &str>("boom") })
            .await;
        assert_eq!(failed, Err("boom"));
        assert_eq!(
            map.get_or_fetch("k".into(), || async { Ok::<_, &str>(1) })
                .await,
            Ok(1)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn entries_expire_after_the_ttl() {
        let map: TtlMap<&'static str, u32> = TtlMap::new(Duration::from_millis(20));
        map.insert("k", 1);
        tokio::time::advance(Duration::from_millis(19)).await;
        assert_eq!(map.get_fresh(&"k"), Some(1));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(map.get_fresh(&"k"), None);
    }

    #[tokio::test]
    async fn concurrent_misses_share_one_fetch() {
        let map: TtlMap<&'static str, u32> = TtlMap::new(Duration::from_secs(5));
        let calls = AtomicUsize::new(0);
        let release = Notify::new();
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            release.notified().await;
            Ok::<_, ()>(3)
        };
        let mut misses = std::pin::pin!(futures::future::join_all(
            (0..8).map(|_| map.get_or_fetch("k", fetch))
        ));
        assert!(
            futures::poll!(&mut misses).is_pending(),
            "the leader's fetch holds every miss"
        );
        release.notify_one();
        assert_eq!(misses.await, vec![Ok(3); 8]);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
