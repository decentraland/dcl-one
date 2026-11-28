//! A bounded TTL memo with single-flight fetches: concurrent misses on one key
//! elect a leader and park the rest on a [`Notify`]; the leader always wakes
//! them, including when its task is cancelled mid-fetch (`LeaderGuard`), so a
//! dropped caller can never strand the others. Waiters park with a short
//! re-check bound as well, so a wakeup that lands before a waiter registers
//! costs one extra loop rather than the whole request.
//!
//! [`TtlCache`] is the simpler keyed memo behind a std mutex (no
//! single-flight), recomputed at most once per TTL.
//!
//! [`Coalesce`] is single-flight with no memo: callers that ask while a read
//! is in flight share its output, and nothing outlives the read.

use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use dashmap::mapref::entry::Entry as MapEntry;
use tokio::sync::{Notify, watch};
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
    #[cfg(test)]
    eviction_passes: std::sync::atomic::AtomicUsize,
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
            #[cfg(test)]
            eviction_passes: Default::default(),
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

    /// Drops `key`'s entry, as an eviction would (an expiry keeps the value for
    /// [`TtlMap::get_stale`]), without moving any clock. For tests only: dropping a slot whose
    /// fetch is in flight would drop its waiters' wake-up too.
    #[cfg(any(test, feature = "test-support"))]
    pub fn remove(&self, key: &K) {
        self.map.remove(key);
    }

    pub fn get_fresh(&self, key: &K) -> Option<V> {
        let slot = self.map.get(key)?;
        let entry = slot.cached.as_ref()?;
        (entry.at.elapsed() < self.ttl).then(|| entry.value.clone())
    }

    /// The last value stored under `key`, however old: a failed refresh
    /// keeps it, so a caller can serve it stale.
    pub fn get_stale(&self, key: &K) -> Option<V> {
        let slot = self.map.get(key)?;
        slot.cached.as_ref().map(|entry| entry.value.clone())
    }

    pub fn insert(&self, key: K, value: V) {
        if !self.map.contains_key(&key) {
            self.make_room(1);
        }
        let mut slot = self.map.entry(key).or_insert_with(Slot::empty);
        slot.cached = Some(Entry {
            at: Instant::now(),
            value,
        });
    }

    /// Makes room for `incoming` new keys in a bounded map: once full, expired
    /// entries go, then the oldest down to ~90% of `max_entries` in one pass,
    /// so a full map pays one scan per tenth of its size. In-flight fetches
    /// stay, and concurrent inserts are not serialized: the bound is soft by
    /// at most the in-flight and racing keys.
    fn make_room(&self, incoming: usize) {
        let Some(max) = self.max_entries else { return };
        if self.map.len() + incoming <= max {
            return;
        }
        #[cfg(test)]
        self.eviction_passes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let ttl = self.ttl;
        self.map.retain(|_, slot| {
            slot.notify.is_some() || slot.cached.as_ref().is_some_and(|e| e.at.elapsed() < ttl)
        });
        let low_water = max.saturating_sub(max / 10 + 1);
        let len = self.map.len() + incoming;
        if len <= max {
            return;
        }
        let excess = len.saturating_sub(low_water.max(incoming)).max(1);
        let mut ages: Vec<(Option<Instant>, K)> = self
            .map
            .iter()
            .filter(|slot| slot.notify.is_none())
            .map(|slot| (slot.cached.as_ref().map(|e| e.at), slot.key().clone()))
            .collect();
        if ages.len() > excess {
            ages.select_nth_unstable_by_key(excess - 1, |(at, _)| *at);
            ages.truncate(excess);
        }
        for (_, key) in ages {
            self.map.remove(&key);
        }
    }

    /// The cached value, expired or not, while it is younger than `max_age`:
    /// a stale-on-error read for a caller that may answer an old value when
    /// its fetch fails. A failed fetch leaves the expired entry in place.
    pub fn get_within(&self, key: &K, max_age: Duration) -> Option<V> {
        let slot = self.map.get(key)?;
        let entry = slot.cached.as_ref()?;
        (entry.at.elapsed() < max_age).then(|| entry.value.clone())
    }

    /// Ages `key`'s entry by `by`, as if it had been stored that much earlier.
    #[cfg(any(test, feature = "test-support"))]
    pub fn backdate(&self, key: &K, by: Duration) {
        if let Some(mut slot) = self.map.get_mut(key)
            && let Some(entry) = slot.cached.as_mut()
        {
            entry.at = entry
                .at
                .checked_sub(by)
                .expect("a backdate within the clock");
        }
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

                    // This leader's own slot is in flight, so it survives.
                    self.make_room(0);

                    let fetch = fetch.take().expect("a leader fetches once");
                    let result = fetch().await;
                    self.settle(&key, &result);
                    guard.finished = true;
                    notify.notify_waiters();
                    return result;
                }
            }
        }
    }

    /// [`Self::get_or_fetch`] for a fetch that needs what the caller holds
    /// in `held` (a pooled connection, say), which only the fetching caller
    /// keeps: a hit leaves it with the caller, the leader's fetch borrows it
    /// and the caller keeps it, and a caller that waits on another's fetch
    /// drops it first (`held` becomes `None`), so no waiter holds one.
    /// `Ok(None)`: the fetch it waited on failed, and with `held` gone this
    /// caller must start again (open what it holds anew and call again).
    pub async fn get_or_fetch_holding<G, F, E>(
        &self,
        key: K,
        held: &mut Option<G>,
        fetch: F,
    ) -> Result<Option<V>, E>
    where
        F: AsyncFnOnce(&mut G) -> Result<V, E>,
    {
        loop {
            let action = {
                let mut entry = self.map.entry(key.clone()).or_insert_with(Slot::empty);
                if let Some(cached) = entry.cached.as_ref()
                    && cached.at.elapsed() < self.ttl
                {
                    return Ok(Some(cached.value.clone()));
                }
                if let Some(notify) = entry.notify.as_ref() {
                    Action::Wait(notify.clone())
                } else if held.is_none() {
                    return Ok(None);
                } else {
                    let notify = Arc::new(Notify::new());
                    entry.notify = Some(notify.clone());
                    Action::Lead(notify)
                }
            };
            match action {
                Action::Wait(notify) => {
                    *held = None;
                    let _ = tokio::time::timeout(WAIT_RECHECK, notify.notified()).await;
                }
                Action::Lead(notify) => {
                    let mut guard = LeaderGuard {
                        cache: self,
                        key: key.clone(),
                        notify: notify.clone(),
                        finished: false,
                    };
                    self.make_room(0);
                    let result = fetch(held.as_mut().expect("a leader holds")).await;
                    self.settle(&key, &result);
                    guard.finished = true;
                    notify.notify_waiters();
                    return result.map(Some);
                }
            }
        }
    }

    /// Files a leader's result: a value is cached, and either way the slot
    /// is no longer in flight.
    fn settle<E>(&self, key: &K, result: &Result<V, E>) {
        match result {
            Ok(value) => {
                let entry = Entry {
                    at: Instant::now(),
                    value: value.clone(),
                };
                match self.map.get_mut(key) {
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
                if let Some(mut slot) = self.map.get_mut(key) {
                    slot.notify = None;
                }
            }
        }
    }
}

/// Single-flight with no memo. A caller that asks for a key while a read of
/// it is in flight may take that read's output; once the output is out the
/// key is free again, so a caller arriving after starts a read of its own
/// (an answer is never served past the read that produced it). A joined
/// output was read for another caller, which may have observed a state this
/// one must not be answered from, so a joiner validates it
/// ([`Coalesce::run_accepting`], or the caller itself with
/// [`Coalesce::run_many`]) and reads again when it does not hold. The caller
/// that starts a read runs it inline, so it may borrow what that caller
/// holds. A read whose caller is cancelled ends without output, and the
/// callers that joined it get its keys back to read again, never a value
/// nobody read.
pub struct Coalesce<K, O>
where
    K: Eq + Hash + Clone,
    O: Clone,
{
    flights: DashMap<K, (u64, watch::Receiver<Option<O>>)>,
    next: AtomicU64,
    reads: AtomicU64,
    joins: AtomicU64,
    #[cfg(any(test, feature = "test-support"))]
    changed: Notify,
}

/// The flights a caller joined: each one's output channel and the caller's
/// keys it reads.
type Joined<K, O> = Vec<(u64, watch::Receiver<Option<O>>, Vec<K>)>;

/// What [`Coalesce::run_many`] answers a caller.
pub struct Many<K, O> {
    /// The output of this caller's own read, when some key was not in flight.
    pub own: Option<O>,
    /// Each joined read's output, with this caller's keys it read: the
    /// caller validates it before serving it.
    pub joined: Vec<(O, Vec<K>)>,
    /// This caller's keys whose joined read ended without output.
    pub orphaned: Vec<K>,
}

/// The read a caller leads: its keys leave the map when it ends, published
/// or cancelled (only while they still name this flight), and its joiners
/// then see its output or, without one, the channel closed.
struct Flight<'a, K, O>
where
    K: Eq + Hash + Clone,
    O: Clone,
{
    flights: &'a DashMap<K, (u64, watch::Receiver<Option<O>>)>,
    id: u64,
    keys: Vec<K>,
    output: watch::Sender<Option<O>>,
}

impl<K, O> Flight<'_, K, O>
where
    K: Eq + Hash + Clone,
    O: Clone,
{
    fn publish(&self, output: &O) {
        self.output.send_replace(Some(output.clone()));
    }
}

impl<K, O> Drop for Flight<'_, K, O>
where
    K: Eq + Hash + Clone,
    O: Clone,
{
    fn drop(&mut self) {
        for key in &self.keys {
            self.flights.remove_if(key, |_, (id, _)| *id == self.id);
        }
    }
}

impl<K, O> Default for Coalesce<K, O>
where
    K: Eq + Hash + Clone,
    O: Clone,
{
    fn default() -> Self {
        Self {
            flights: DashMap::new(),
            next: AtomicU64::new(0),
            reads: AtomicU64::new(0),
            joins: AtomicU64::new(0),
            #[cfg(any(test, feature = "test-support"))]
            changed: Notify::new(),
        }
    }
}

impl<K, O> Coalesce<K, O>
where
    K: Eq + Hash + Clone,
    O: Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    fn bump(&self, counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::SeqCst);
        #[cfg(any(test, feature = "test-support"))]
        self.changed.notify_waiters();
    }

    /// Joins the flights `keys` are in and claims the others for one read of
    /// this caller's (`None` when every key is in flight).
    fn claim(&self, keys: &[K]) -> (Option<Flight<'_, K, O>>, Joined<K, O>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (output, receiver) = watch::channel(None);
        let mut led = Vec::new();
        let mut joined: Joined<K, O> = Vec::new();
        let mut seen = HashSet::new();
        for key in keys {
            if !seen.insert(key) {
                continue;
            }
            match self.flights.entry(key.clone()) {
                // A read that published is over, though its caller has not
                // released its keys yet: this caller reads anew.
                MapEntry::Occupied(mut done) if done.get().1.borrow().is_some() => {
                    done.insert((id, receiver.clone()));
                    led.push(key.clone());
                }
                MapEntry::Occupied(flight) => {
                    let (other, channel) = flight.get();
                    match joined.iter_mut().find(|(id, _, _)| id == other) {
                        Some((_, _, keys)) => keys.push(key.clone()),
                        None => {
                            joined.push((*other, channel.clone(), vec![key.clone()]));
                            self.bump(&self.joins);
                        }
                    }
                }
                MapEntry::Vacant(free) => {
                    free.insert((id, receiver.clone()));
                    led.push(key.clone());
                }
            }
        }
        if !led.is_empty() {
            self.bump(&self.reads);
        }
        let flight = (!led.is_empty()).then(|| Flight {
            flights: &self.flights,
            id,
            keys: led,
            output,
        });
        (flight, joined)
    }

    /// The output of `key`'s flight, joined or read here with `read`, for a
    /// caller that may be answered from any read in flight. The one
    /// exception to validating a joined output: a joiner's answer may
    /// predate its arrival by the one read already in flight, and a joined
    /// failure is passed through as it is. Use [`Coalesce::run_accepting`]
    /// when either is not acceptable.
    pub async fn run<F, Fut>(&self, key: K, read: F) -> O
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = O>,
    {
        self.run_accepting(key, read, |_| async { true }).await
    }

    /// The output of `key`'s flight: this caller's own read with `read`, or
    /// a joined read's output that `accept` takes. A joined output `accept`
    /// refuses, and a joined read that ended without output, are followed by
    /// one read of this caller's own, not shared: a caller waits on at most
    /// one joined read, so callers that all refuse (a source failing for
    /// everyone) read side by side instead of one after another. This
    /// caller's own read is always its answer.
    pub async fn run_accepting<F, Fut, A, AFut>(&self, key: K, read: F, mut accept: A) -> O
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = O>,
        A: FnMut(O) -> AFut,
        AFut: Future<Output = bool>,
    {
        let (flight, joined) = self.claim(std::slice::from_ref(&key));
        if let Some(flight) = flight {
            let output = read().await;
            flight.publish(&output);
            return output;
        }
        for (_, mut channel, _) in joined {
            let output = match channel.wait_for(Option::is_some).await {
                Ok(output) => (*output).clone().expect("waited for an output"),
                Err(_) => continue,
            };
            if accept(output.clone()).await {
                return output;
            }
        }
        self.bump(&self.reads);
        read().await
    }

    /// The outputs for many keys: the keys in flight join those flights, and
    /// `read` reads the rest in one flight (called only when some key is not
    /// in flight). The caller serves its own read's output and validates
    /// each joined one per key ([`Many`]).
    pub async fn run_many<F, Fut>(&self, keys: &[K], read: F) -> Many<K, O>
    where
        F: FnOnce(Vec<K>) -> Fut,
        Fut: Future<Output = O>,
    {
        let (flight, joined) = self.claim(keys);
        let mut many = Many {
            own: None,
            joined: Vec::with_capacity(joined.len()),
            orphaned: Vec::new(),
        };
        if let Some(flight) = flight {
            let output = read(flight.keys.clone()).await;
            flight.publish(&output);
            many.own = Some(output);
        }
        for (_, mut channel, keys) in joined {
            let output = match channel.wait_for(Option::is_some).await {
                Ok(output) => (*output).clone().expect("waited for an output"),
                Err(_) => {
                    many.orphaned.extend(keys);
                    continue;
                }
            };
            many.joined.push((output, keys));
        }
        many
    }

    /// The keys in flight now.
    pub fn in_flight(&self) -> usize {
        self.flights.len()
    }

    /// How many reads were started: shared ones (each joined by any number
    /// of callers) and the unshared one a caller makes after refusing a
    /// joined output.
    pub fn reads_started(&self) -> u64 {
        self.reads.load(Ordering::SeqCst)
    }

    /// How many times a caller joined a read in flight.
    pub fn joins(&self) -> u64 {
        self.joins.load(Ordering::SeqCst)
    }

    /// Resolves once `done` holds; tests wait on a join with it rather than
    /// sleeping. A state that never comes fails the test at
    /// [`HANG_GUARD`](crate::test_wait::HANG_GUARD) instead of hanging it.
    #[cfg(any(test, feature = "test-support"))]
    pub async fn until(&self, done: impl Fn(&Self) -> bool) {
        let reached = async {
            loop {
                let changed = self.changed.notified();
                if done(self) {
                    return;
                }
                changed.await;
            }
        };
        tokio::time::timeout(crate::test_wait::HANG_GUARD, reached)
            .await
            .expect("the coalescer did not reach the awaited state in time");
    }
}

/// A keyed TTL memo behind a mutex: the category counts, the parcel aliases and
/// the moderator set are recomputed at most once per TTL.
pub struct Cached<T> {
    pub value: T,
    pub expires_at: std::time::Instant,
}

pub type TtlCache<T> = std::sync::Mutex<std::collections::HashMap<String, Cached<T>>>;

// The memo holds plain values, so a poisoned one is still consistent.
fn lock_cache<T>(
    cache: &TtlCache<T>,
) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, Cached<T>>> {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn cache_get<T: Clone>(cache: &TtlCache<T>, key: &str) -> Option<T> {
    let cache = lock_cache(cache);
    cache
        .get(key)
        .filter(|c| c.expires_at > std::time::Instant::now())
        .map(|c| c.value.clone())
}

pub fn cache_remove<T>(cache: &TtlCache<T>, key: &str) {
    lock_cache(cache).remove(key);
}

pub fn cache_put<T>(cache: &TtlCache<T>, key: String, value: T, ttl: Duration) {
    let mut cache = lock_cache(cache);
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

    /// A gate the reads in a test wait on, and how many reads started.
    #[derive(Default)]
    struct Gate {
        reads: AtomicUsize,
        open: Notify,
        opened: std::sync::atomic::AtomicBool,
    }

    impl Gate {
        async fn pass(&self) {
            self.reads.fetch_add(1, Ordering::SeqCst);
            while !self.opened.load(Ordering::SeqCst) {
                let _ = tokio::time::timeout(Duration::from_millis(5), self.open.notified()).await;
            }
        }

        fn open(&self) {
            self.opened.store(true, Ordering::SeqCst);
            self.open.notify_waiters();
        }

        fn reads(&self) -> usize {
            self.reads.load(Ordering::SeqCst)
        }
    }

    #[tokio::test]
    async fn concurrent_callers_share_one_read_and_nothing_is_kept() {
        let coalesce: Arc<Coalesce<&'static str, u32>> = Arc::new(Coalesce::new());
        let gate = Arc::new(Gate::default());
        let callers: Vec<_> = (0..8)
            .map(|_| {
                let (coalesce, gate) = (coalesce.clone(), gate.clone());
                tokio::spawn(async move {
                    coalesce
                        .run("k", || async {
                            gate.pass().await;
                            7
                        })
                        .await
                })
            })
            .collect();
        coalesce.until(|c| c.joins() >= 7).await;
        assert_eq!(coalesce.joins(), 7);
        assert_eq!(gate.reads(), 1, "one read in flight");
        gate.open();
        for caller in callers {
            assert_eq!(caller.await.unwrap(), 7);
        }
        assert_eq!(gate.reads(), 1, "every caller got the one read");
        assert_eq!(coalesce.reads_started(), 1);
        assert_eq!(coalesce.in_flight(), 0, "nothing is kept");
        assert_eq!(
            coalesce.run("k", || async { 8 }).await,
            8,
            "a later caller reads again"
        );
    }

    #[tokio::test]
    async fn a_cancelled_read_hands_its_key_back() {
        let coalesce: Arc<Coalesce<&'static str, u32>> = Arc::new(Coalesce::new());
        let gate = Arc::new(Gate::default());
        let leader = {
            let coalesce = coalesce.clone();
            tokio::spawn(async move { coalesce.run("k", std::future::pending::<u32>).await })
        };
        coalesce.until(|c| c.reads_started() >= 1).await;
        let joiner = {
            let (coalesce, gate) = (coalesce.clone(), gate.clone());
            tokio::spawn(async move {
                coalesce
                    .run("k", || async {
                        gate.pass().await;
                        9
                    })
                    .await
            })
        };
        coalesce.until(|c| c.joins() >= 1).await;
        assert_eq!(gate.reads(), 0, "the joiner waits on the leader's read");
        leader.abort();
        let _ = leader.await;
        gate.open();
        assert_eq!(joiner.await.unwrap(), 9, "the joiner read it itself");
        assert_eq!(gate.reads(), 1);
        assert_eq!(coalesce.in_flight(), 0);
    }

    /// A joiner that refuses the joined output reads again itself; the
    /// leader is answered its own read.
    #[tokio::test]
    async fn a_refused_joined_output_is_read_again() {
        let coalesce: Arc<Coalesce<&'static str, u32>> = Arc::new(Coalesce::new());
        let gate = Arc::new(Gate::default());
        let leader = {
            let (coalesce, gate) = (coalesce.clone(), gate.clone());
            tokio::spawn(async move {
                coalesce
                    .run("k", || async {
                        gate.pass().await;
                        1
                    })
                    .await
            })
        };
        coalesce.until(|c| c.reads_started() >= 1).await;
        let joiner = {
            let coalesce = coalesce.clone();
            tokio::spawn(async move {
                coalesce
                    .run_accepting("k", || async { 2 }, |seen| async move { seen != 1 })
                    .await
            })
        };
        coalesce.until(|c| c.joins() >= 1).await;
        gate.open();
        assert_eq!(leader.await.unwrap(), 1);
        assert_eq!(joiner.await.unwrap(), 2, "the refused 1 was read again");
        assert_eq!(coalesce.reads_started(), 2);
        assert_eq!(coalesce.in_flight(), 0);
    }

    /// Callers that all refuse a failing read wait on one joined read at
    /// most, then read side by side: four callers of a source that fails
    /// every 100 ms read for 200 ms, not 400 ms.
    #[tokio::test(start_paused = true)]
    async fn refusing_callers_read_side_by_side() {
        let coalesce: Arc<Coalesce<&'static str, Option<u32>>> = Arc::new(Coalesce::new());
        let reads = Arc::new(AtomicUsize::new(0));
        let started = tokio::time::Instant::now();
        let callers: Vec<_> = (0..4)
            .map(|_| {
                let (coalesce, reads) = (coalesce.clone(), reads.clone());
                tokio::spawn(async move {
                    coalesce
                        .run_accepting(
                            "k",
                            || async {
                                reads.fetch_add(1, Ordering::SeqCst);
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                None
                            },
                            |output| async move { output.is_some() },
                        )
                        .await
                })
            })
            .collect();
        for caller in callers {
            assert_eq!(caller.await.unwrap(), None);
        }
        assert_eq!(reads.load(Ordering::SeqCst), 4);
        assert_eq!(coalesce.reads_started(), 4);
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(coalesce.in_flight(), 0);
    }

    #[tokio::test]
    async fn overlapping_sets_read_each_key_once() {
        let coalesce: Arc<Coalesce<u32, Vec<u32>>> = Arc::new(Coalesce::new());
        let gate = Arc::new(Gate::default());
        let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let call = |keys: Vec<u32>| {
            let (coalesce, gate, asked) = (coalesce.clone(), gate.clone(), asked.clone());
            tokio::spawn(async move {
                coalesce
                    .run_many(&keys, |led| async move {
                        asked.lock().unwrap().push(led.clone());
                        gate.pass().await;
                        led
                    })
                    .await
            })
        };
        let first = call(vec![1, 2, 2]);
        coalesce.until(|c| c.reads_started() >= 1).await;
        let second = call(vec![2, 3]);
        coalesce.until(|c| c.joins() >= 1).await;
        gate.open();
        let first = first.await.unwrap();
        assert_eq!(first.own, Some(vec![1, 2]));
        assert!(first.joined.is_empty() && first.orphaned.is_empty());
        let second = second.await.unwrap();
        assert_eq!(second.own, Some(vec![3]), "its own read");
        assert_eq!(
            second.joined,
            vec![(vec![1, 2], vec![2])],
            "the joined read, with the key it read"
        );
        assert!(second.orphaned.is_empty());
        assert_eq!(*asked.lock().unwrap(), vec![vec![1, 2], vec![3]]);
        assert_eq!(coalesce.in_flight(), 0);
    }

    #[tokio::test]
    async fn a_set_joined_to_a_cancelled_read_gets_its_keys_back() {
        let coalesce: Arc<Coalesce<u32, Vec<u32>>> = Arc::new(Coalesce::new());
        let leader = {
            let coalesce = coalesce.clone();
            tokio::spawn(async move {
                coalesce
                    .run_many(&[1, 2], |_| std::future::pending::<Vec<u32>>())
                    .await
            })
        };
        coalesce.until(|c| c.reads_started() >= 1).await;
        let joiner = {
            let coalesce = coalesce.clone();
            tokio::spawn(async move { coalesce.run_many(&[2, 3], |led| async move { led }).await })
        };
        coalesce.until(|c| c.joins() >= 1).await;
        leader.abort();
        let _ = leader.await;
        let joiner = joiner.await.unwrap();
        assert_eq!((joiner.own, joiner.orphaned), (Some(vec![3]), vec![2]));
        assert!(joiner.joined.is_empty());
        assert_eq!(coalesce.in_flight(), 0);
    }

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
    async fn a_failed_refresh_keeps_the_stale_value() {
        let map: TtlMap<String, u32> = TtlMap::new(Duration::ZERO);
        assert_eq!(map.get_stale(&"k".into()), None);
        map.insert("k".into(), 3);
        assert_eq!(
            map.get_fresh(&"k".into()),
            None,
            "a zero TTL is never fresh"
        );
        let failed = map
            .get_or_fetch("k".into(), || async { Err::<u32, &str>("boom") })
            .await;
        assert_eq!(failed, Err("boom"));
        assert_eq!(map.get_stale(&"k".into()), Some(3));
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

    #[tokio::test(start_paused = true)]
    async fn insert_on_a_full_bounded_map_evicts_the_oldest() {
        let map: TtlMap<u32, u32> = TtlMap::bounded(Duration::from_secs(60), 3);
        for k in 0..3 {
            map.insert(k, k);
            tokio::time::advance(Duration::from_millis(1)).await;
        }
        map.insert(1, 10);
        assert_eq!(map.len(), 3, "replacing a key needs no room");
        map.insert(3, 3);
        assert!(map.len() <= 3, "{}", map.len());
        assert_eq!(map.get_fresh(&0), None, "the oldest entry is evicted");
        assert_eq!(map.get_fresh(&1), Some(10));
        assert_eq!(map.get_fresh(&3), Some(3));
    }

    #[tokio::test(start_paused = true)]
    async fn insert_on_a_full_bounded_map_drops_expired_entries_first() {
        let map: TtlMap<u32, u32> = TtlMap::bounded(Duration::from_secs(1), 3);
        map.insert(0, 0);
        map.insert(1, 1);
        tokio::time::advance(Duration::from_secs(2)).await;
        map.insert(2, 2);
        map.insert(3, 3);
        assert_eq!(map.len(), 2, "both expired entries are swept");
        assert_eq!(map.get_fresh(&2), Some(2));
        assert_eq!(map.get_fresh(&3), Some(3));
    }

    #[tokio::test]
    async fn an_unbounded_map_keeps_every_insert() {
        let map: TtlMap<u32, u32> = TtlMap::new(Duration::from_secs(60));
        for k in 0..100 {
            map.insert(k, k);
        }
        assert_eq!(map.len(), 100);
    }

    #[tokio::test(start_paused = true)]
    async fn a_map_of_one_keeps_only_the_newest_entry() {
        let map: TtlMap<u32, u32> = TtlMap::bounded(Duration::from_secs(60), 1);
        for k in 0..5 {
            map.insert(k, k);
            tokio::time::advance(Duration::from_millis(1)).await;
            assert_eq!(map.len(), 1);
            assert_eq!(map.get_fresh(&k), Some(k));
        }
        assert_eq!(
            map.get_or_fetch(9, || async { Ok::<_, ()>(9) }).await,
            Ok(9)
        );
        assert_eq!(map.len(), 1);
        assert_eq!(map.get_fresh(&9), Some(9));
    }

    #[tokio::test]
    async fn a_full_map_evicts_in_batches_not_per_insert() {
        let map: TtlMap<u32, u32> = TtlMap::bounded(Duration::from_secs(60), 100);
        for k in 0..1_000 {
            map.insert(k, k);
        }
        assert!(map.len() <= 100, "{}", map.len());
        let passes = map.eviction_passes.load(Ordering::Relaxed);
        assert!(
            passes <= 1_000 / 10 + 1,
            "one scan per tenth of the map, not per insert: {passes}"
        );
        assert_eq!(map.get_fresh(&999), Some(999), "the newest entry stays");
    }

    #[tokio::test]
    async fn an_in_flight_fetch_survives_eviction_and_lands() {
        let map: TtlMap<u32, u32> = TtlMap::bounded(Duration::from_secs(60), 10);
        let release = Notify::new();
        let fetch = || async {
            release.notified().await;
            Ok::<_, ()>(42)
        };
        let mut pending = std::pin::pin!(map.get_or_fetch(1_000, fetch));
        assert!(futures::poll!(&mut pending).is_pending());
        for k in 0..100 {
            map.insert(k, k);
        }
        assert!(map.len() <= 11, "{}", map.len());
        release.notify_one();
        assert_eq!(pending.await, Ok(42));
        assert_eq!(map.get_fresh(&1_000), Some(42));
    }

    #[tokio::test]
    async fn a_leader_on_a_full_map_evicts_instead_of_clearing() {
        let map: TtlMap<u32, u32> = TtlMap::bounded(Duration::from_secs(60), 10);
        for k in 0..10 {
            map.insert(k, k);
        }
        assert_eq!(
            map.get_or_fetch(99, || async { Ok::<_, ()>(99) }).await,
            Ok(99)
        );
        assert!(map.len() <= 10, "{}", map.len());
        assert_eq!(map.get_fresh(&9), Some(9), "recent entries are kept");
    }

    /// A caller that waits on another's fetch drops what it holds first and
    /// gets the leader's value; the leader keeps what it holds.
    #[tokio::test]
    async fn a_waiter_drops_what_it_holds_before_waiting() {
        let map: Arc<TtlMap<&'static str, u32>> = Arc::new(TtlMap::new(Duration::from_secs(60)));
        let connection = Arc::new(());
        let release = Arc::new(tokio::sync::Notify::new());
        let leader = {
            let (map, release) = (map.clone(), release.clone());
            let mut held = Some(connection.clone());
            tokio::spawn(async move {
                let value = map
                    .get_or_fetch_holding("page", &mut held, async |_: &mut Arc<()>| {
                        release.notified().await;
                        Ok::<u32, ()>(7)
                    })
                    .await;
                (value, held.is_some())
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        let waiter = {
            let map = map.clone();
            let mut held = Some(connection.clone());
            tokio::spawn(async move {
                let value = map
                    .get_or_fetch_holding("page", &mut held, async |_: &mut Arc<()>| {
                        Ok::<u32, ()>(0)
                    })
                    .await;
                (value, held.is_some())
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            Arc::strong_count(&connection),
            2,
            "only the leader still holds a clone"
        );
        release.notify_one();
        assert_eq!(leader.await.unwrap(), (Ok(Some(7)), true));
        assert_eq!(waiter.await.unwrap(), (Ok(Some(7)), false));
        // A hit leaves what the caller holds with it.
        let mut held = Some(connection.clone());
        let hit = map
            .get_or_fetch_holding("page", &mut held, async |_: &mut Arc<()>| Ok::<u32, ()>(0))
            .await;
        assert_eq!((hit, held.is_some()), (Ok(Some(7)), true));
    }

    /// When the fetch a caller waited on fails, the caller, its holding gone,
    /// is told to start again rather than lead without it.
    #[tokio::test]
    async fn a_waiter_whose_leader_failed_starts_again() {
        let map: Arc<TtlMap<&'static str, u32>> = Arc::new(TtlMap::new(Duration::from_secs(60)));
        let release = Arc::new(tokio::sync::Notify::new());
        let leader = {
            let (map, release) = (map.clone(), release.clone());
            tokio::spawn(async move {
                map.get_or_fetch_holding("page", &mut Some(()), async |_: &mut ()| {
                    release.notified().await;
                    Err::<u32, &str>("down")
                })
                .await
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        let waiter = {
            let map = map.clone();
            tokio::spawn(async move {
                let mut held = Some(());
                let value = map
                    .get_or_fetch_holding("page", &mut held, async |_: &mut ()| Ok::<u32, &str>(1))
                    .await;
                (value, held.is_some())
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        release.notify_one();
        assert_eq!(leader.await.unwrap(), Err("down"));
        assert_eq!(waiter.await.unwrap(), (Ok(None), false));
    }
}
