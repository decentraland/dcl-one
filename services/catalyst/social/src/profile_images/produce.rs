//! One answer per (entity, kind), produced once however many readers ask:
//! the profile's own snapshot file, an image already stored, then a
//! production that every concurrent reader of the key shares. The
//! production runs detached under the route's budget ([`BUDGET`], longer
//! than the read-through's default guard) and a bound on productions at once
//! ([`MAX_CONCURRENT`]): it asks the origin (retrying transient failures),
//! fills the profile entity through the read-through when the origin has no
//! image, then tries the renderer. It answers 404 only when the entity is not a profile or every
//! configured source says there is no image, and remembers that for a
//! minute; a source that failed or ran out of time is a 503 instead, and the
//! production that outlived its reader still stores what it got.
//!
//! [`ProfileImagesState::warm`] queues faces a response is about to point at
//! (the lambdas' profiles, the Shop's creator directory) for a few workers,
//! so the browser's request finds them stored.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use chrono::NaiveDateTime;
use parking_lot::Mutex;
use tokio::sync::{Semaphore, mpsc, watch};
use tokio::time::Instant;

use super::ProfileImagesState;
use super::cache::{ImageKind, ImageSource};
use super::origin::OriginResult;
use super::queue::RenderOutcome;
use super::resolver::snapshot_from_content;
use crate::auth::is_canonical_cid;

/// How long a production may take, the read-through fill, origin retries
/// and a render included.
pub const BUDGET: Duration = Duration::from_secs(25);
/// Productions at once, the warm workers' included.
pub const MAX_CONCURRENT: usize = 16;
/// Warm workers: they hold at most this many of [`MAX_CONCURRENT`].
const WARM_WORKERS: usize = 4;
/// Faces waiting to be warmed; beyond it a warm request is dropped.
const WARM_QUEUE: usize = 1024;
/// A face queued for warming is not queued again for this long.
const WARM_MEMO_TTL: Duration = Duration::from_secs(600);
/// How long a definitive "no image" stands.
const NOT_FOUND_TTL: Duration = Duration::from_secs(60);
const MEMO_MAX: usize = 65_536;
/// A profile deployed this recently may not be drawn upstream yet: its
/// origin 404 is retried within the budget.
const FRESH_PROFILE_SECS: i64 = 600;

#[derive(Clone, Debug)]
pub enum Answer {
    Image(Bytes),
    NotFound,
    Unavailable,
}

/// What answered, for logs and tests.
#[derive(Default)]
pub struct Counters {
    pub snapshot: AtomicU64,
    pub stored: AtomicU64,
    pub origin: AtomicU64,
    pub render: AtomicU64,
    pub not_found: AtomicU64,
    pub unavailable: AtomicU64,
    /// Readers that shared a production another reader started.
    pub joined: AtomicU64,
    /// Productions started.
    pub produced: AtomicU64,
    /// Faces the warm workers finished.
    pub warmed: AtomicU64,
}

fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// A bounded map of recent keys.
struct Memo<K> {
    map: Mutex<HashMap<K, Instant>>,
    ttl: Duration,
}

impl<K: Eq + Hash + Clone> Memo<K> {
    fn new(ttl: Duration) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    fn hit(&self, key: &K) -> bool {
        self.map
            .lock()
            .get(key)
            .is_some_and(|at| at.elapsed() < self.ttl)
    }

    /// Records `key`; `false` when it was already there and current.
    fn record(&self, key: K) -> bool {
        let mut map = self.map.lock();
        if map.get(&key).is_some_and(|at| at.elapsed() < self.ttl) {
            return false;
        }
        if map.len() >= MEMO_MAX {
            let ttl = self.ttl;
            map.retain(|_, at| at.elapsed() < ttl);
            if map.len() >= MEMO_MAX {
                map.clear();
            }
        }
        map.insert(key, Instant::now());
        true
    }

    fn forget(&self, key: &K) {
        self.map.lock().remove(key);
    }
}

type Key = (String, ImageKind);

pub struct Producer {
    inflight: Mutex<HashMap<Key, watch::Receiver<Option<Answer>>>>,
    permits: Semaphore,
    budget: Duration,
    not_found: Memo<Key>,
    warmed: Memo<String>,
    warm_tx: mpsc::Sender<String>,
    warm_rx: Mutex<Option<mpsc::Receiver<String>>>,
    pub counters: Counters,
}

impl Producer {
    pub fn new(budget: Duration) -> Self {
        let (warm_tx, warm_rx) = mpsc::channel(WARM_QUEUE);
        Self {
            inflight: Mutex::new(HashMap::new()),
            permits: Semaphore::new(MAX_CONCURRENT),
            budget,
            not_found: Memo::new(NOT_FOUND_TTL),
            warmed: Memo::new(WARM_MEMO_TTL),
            warm_tx,
            warm_rx: Mutex::new(Some(warm_rx)),
            counters: Counters::default(),
        }
    }
}

/// Takes the key out of the in-flight map when the production ends, a
/// panicking one included, so the next reader starts afresh.
struct InflightGuard {
    state: Arc<ProfileImagesState>,
    key: Key,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.state.producer.inflight.lock().remove(&self.key);
    }
}

struct Held {
    entity_type: String,
    timestamp: NaiveDateTime,
}

impl ProfileImagesState {
    /// The image for (`entity`, `kind`), `entity` a canonical CID the store
    /// does not withhold.
    pub async fn image(self: &Arc<Self>, entity: &str, kind: ImageKind) -> Answer {
        let counters = &self.producer.counters;
        let mut store_unavailable = false;
        match snapshot_from_content(&self.pool, &self.storage, entity, kind).await {
            Ok(Some(bytes)) => {
                bump(&counters.snapshot);
                return Answer::Image(bytes);
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(entity, kind = ?kind, error = %e, "profile snapshot lookup failed");
                store_unavailable = true;
            }
        }
        if let Some(bytes) = self.cache.get(entity, kind).await {
            bump(&counters.stored);
            return Answer::Image(bytes);
        }
        let key = (entity.to_string(), kind);
        let answer = if self.producer.not_found.hit(&key) {
            Answer::NotFound
        } else {
            self.produce_shared(key).await
        };
        match answer {
            Answer::NotFound if store_unavailable => Answer::Unavailable,
            answer => answer,
        }
    }

    async fn produce_shared(self: &Arc<Self>, key: Key) -> Answer {
        let mut rx = {
            let mut inflight = self.producer.inflight.lock();
            match inflight.get(&key) {
                Some(rx) => {
                    bump(&self.producer.counters.joined);
                    rx.clone()
                }
                None => {
                    let (tx, rx) = watch::channel(None);
                    inflight.insert(key.clone(), rx.clone());
                    bump(&self.producer.counters.produced);
                    let guard = InflightGuard {
                        state: self.clone(),
                        key,
                    };
                    tokio::spawn(async move {
                        let state = guard.state.clone();
                        let (entity, kind) = (guard.key.0.clone(), guard.key.1);
                        let deadline = Instant::now() + state.producer.budget;
                        let answer = tokio::time::timeout_at(
                            deadline,
                            state.produce(&entity, kind, deadline),
                        )
                        .await
                        .unwrap_or_else(|_| {
                            tracing::warn!(entity, kind = ?kind, "profile image budget ran out");
                            Answer::Unavailable
                        });
                        drop(guard);
                        let _ = tx.send(Some(answer));
                    });
                    rx
                }
            }
        };
        let answer = match rx.wait_for(Option::is_some).await {
            Ok(answer) => answer.clone().unwrap_or(Answer::Unavailable),
            Err(_) => Answer::Unavailable,
        };
        let counters = &self.producer.counters;
        match &answer {
            Answer::NotFound => bump(&counters.not_found),
            Answer::Unavailable => bump(&counters.unavailable),
            Answer::Image(_) => {}
        }
        answer
    }

    async fn produce(self: &Arc<Self>, entity: &str, kind: ImageKind, deadline: Instant) -> Answer {
        let Ok(_permit) = self.producer.permits.acquire().await else {
            return Answer::Unavailable;
        };
        if let Some(bytes) = self.cache.get(entity, kind).await {
            return Answer::Image(bytes);
        }
        let counters = &self.producer.counters;
        let fetch = |retry_not_found| async move {
            match &self.origin {
                Some(origin) => Some(
                    origin
                        .fetch_until(entity, kind, deadline, retry_not_found)
                        .await,
                ),
                None => None,
            }
        };
        let mut failed = false;
        let is_fresh = |h: &Held| {
            (chrono::Utc::now().naive_utc() - h.timestamp).num_seconds() < FRESH_PROFILE_SECS
        };
        // What this node holds answers at once; the origin answers for any
        // profile it drew, held or not, so the read-through fills the entity
        // only when the origin has no image (the renderer needs its avatar).
        let mut held = self.held_here(entity).await.unwrap_or_else(|e| {
            tracing::error!(entity, error = %e, "profile entity lookup failed");
            failed = true;
            None
        });
        if held.as_ref().is_some_and(|h| h.entity_type != "profile") {
            return self.no_image(entity, kind);
        }
        let mut fetched = fetch(held.as_ref().is_some_and(is_fresh)).await;
        // Without an origin or a renderer the fill could not lead to an image.
        let derives = self.origin.is_some() || self.render_queue.is_some();
        if held.is_none() && !failed && derives && !matches!(fetched, Some(OriginResult::Hit(_))) {
            held = self.fill(entity, deadline).await.unwrap_or_else(|e| {
                tracing::error!(entity, error = %e, "profile entity fill failed");
                failed = true;
                None
            });
            if held.as_ref().is_some_and(is_fresh)
                && matches!(fetched, Some(OriginResult::NotFound))
            {
                fetched = fetch(true).await;
            }
        }
        let profile = match &held {
            Some(h) if h.entity_type != "profile" => return self.no_image(entity, kind),
            Some(h) => Some(h),
            None => None,
        };
        match fetched {
            Some(OriginResult::Hit(bytes)) => {
                if let Err(e) = self
                    .cache
                    .put(entity, kind, ImageSource::Origin, &bytes)
                    .await
                {
                    tracing::warn!(entity, kind = ?kind, error = %e, "storing a profile image failed");
                }
                bump(&counters.origin);
                tracing::debug!(entity, kind = ?kind, "profile image from the origin");
                return Answer::Image(bytes);
            }
            Some(OriginResult::Error(e)) => {
                tracing::warn!(entity, kind = ?kind, error = %e, "profile image origin failed");
                failed = true;
            }
            Some(OriginResult::NotFound) | None => {}
        }
        if let (Some(queue), Some(_)) = (&self.render_queue, profile) {
            match tokio::time::timeout_at(deadline, queue.render_once(entity)).await {
                Ok(RenderOutcome::Rendered { body, face }) => {
                    bump(&counters.render);
                    return Answer::Image(match kind {
                        ImageKind::Body => body,
                        ImageKind::Face => face,
                    });
                }
                Ok(RenderOutcome::NotFound) => {}
                Ok(RenderOutcome::Failed(e)) => {
                    tracing::warn!(entity, error = %e, "avatar render failed");
                    failed = true;
                }
                Err(_) => failed = true,
            }
        }
        if failed {
            Answer::Unavailable
        } else {
            self.no_image(entity, kind)
        }
    }

    fn no_image(&self, entity: &str, kind: ImageKind) -> Answer {
        self.producer.not_found.record((entity.to_string(), kind));
        Answer::NotFound
    }

    /// The type and timestamp of the entity's deployment here, if any.
    async fn held_here(&self, entity: &str) -> Result<Option<Held>, sqlx::Error> {
        let row: Option<(String, NaiveDateTime)> = sqlx::query_as(
            "SELECT entity_type, entity_timestamp FROM content.deployments \
             WHERE entity_id = $1 LIMIT 1",
        )
        .bind(entity)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(entity_type, timestamp)| Held {
            entity_type,
            timestamp,
        }))
    }

    /// [`held_here`](Self::held_here) after filling the entity through the
    /// read-through, waiting for it up to the deadline (past the
    /// read-through's own guard). `None`: neither this node nor its sources
    /// know it.
    async fn fill(&self, entity: &str, deadline: Instant) -> Result<Option<Held>, sqlx::Error> {
        let Some(read_through) = self.read_through.as_ref() else {
            return Ok(None);
        };
        let wait = deadline.saturating_duration_since(Instant::now());
        read_through
            .active_docs_within(true, &[entity.to_string()], wait)
            .await?;
        self.held_here(entity).await
    }

    /// Queues the faces of `entities` for the warm workers, so the request a
    /// response leads to finds them stored. Bounded: a face queued in the
    /// last ten minutes is skipped and a full queue drops the rest.
    pub fn warm(self: &Arc<Self>, entities: impl IntoIterator<Item = String>) {
        if self.origin.is_none() && self.render_queue.is_none() {
            return;
        }
        self.start_warm_workers();
        for entity in entities {
            if !is_canonical_cid(&entity)
                || self.storage.withholds(&entity)
                || !self.producer.warmed.record(entity.clone())
            {
                continue;
            }
            if let Err(e) = self.producer.warm_tx.try_send(entity) {
                if let mpsc::error::TrySendError::Full(entity) = e {
                    self.producer.warmed.forget(&entity);
                }
                break;
            }
        }
    }

    fn start_warm_workers(self: &Arc<Self>) {
        let Some(rx) = self.producer.warm_rx.lock().take() else {
            return;
        };
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        for _ in 0..WARM_WORKERS {
            let (state, rx) = (self.clone(), rx.clone());
            tokio::spawn(async move {
                loop {
                    let next = rx.lock().await.recv().await;
                    let Some(entity) = next else { break };
                    // A face already stored is warm: no need to read its blob.
                    if !state.cache.holds(&entity, ImageKind::Face).await {
                        let _ = state.image(&entity, ImageKind::Face).await;
                    }
                    bump(&state.producer.counters.warmed);
                }
            });
        }
    }
}
