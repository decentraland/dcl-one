//! Single-flight per entity over the renderer: followers of an in-flight
//! render receive the leader's outcome, a bounded semaphore caps concurrent
//! renderer processes, and a bounded negative memo remembers profiles the
//! database did not hold.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use sqlx::PgPool;
use tokio::sync::{Semaphore, broadcast};

use super::cache::{ImageCache, ImageKind, ImageSource};
use super::render::{AvatarRenderer, RenderError};
use super::resolver::{ResolveResult, resolve_avatar};

#[derive(Clone, Debug)]
pub enum RenderOutcome {
    Rendered { body: Bytes, face: Bytes },
    NotFound,
    Failed(String),
}

const NOT_FOUND_TTL: Duration = Duration::from_secs(300);
const NOT_FOUND_MAX: usize = 65536;

/// Bounded negative memo for profiles the resolver reported missing.
struct MissMemo {
    map: Mutex<HashMap<String, Instant>>,
}

impl MissMemo {
    fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    fn hit(&self, entity: &str) -> bool {
        self.map
            .lock()
            .ok()
            .and_then(|m| m.get(entity).copied())
            .is_some_and(|at| at.elapsed() < NOT_FOUND_TTL)
    }

    fn record(&self, entity: &str) {
        if let Ok(mut m) = self.map.lock() {
            if m.len() >= NOT_FOUND_MAX {
                m.retain(|_, at| at.elapsed() < NOT_FOUND_TTL);
            }
            if m.len() < NOT_FOUND_MAX {
                m.insert(entity.to_string(), Instant::now());
            }
        }
    }
}

struct Inner {
    inflight: Mutex<HashMap<String, broadcast::Sender<RenderOutcome>>>,
    not_found: MissMemo,
    limiter: Semaphore,
    cache: ImageCache,
    pool: PgPool,
    renderer: AvatarRenderer,
    /// The content endpoint the renderer downloads wearables from: this node's own.
    content_base: String,
    workdir_root: std::path::PathBuf,
}

#[derive(Clone)]
pub struct RenderQueue {
    inner: Arc<Inner>,
}

impl RenderQueue {
    pub fn new(
        cache: ImageCache,
        pool: PgPool,
        renderer: AvatarRenderer,
        content_base: String,
        max_concurrent: usize,
        workdir_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                inflight: Mutex::new(HashMap::new()),
                not_found: MissMemo::new(),
                limiter: Semaphore::new(max_concurrent.max(1)),
                cache,
                pool,
                renderer,
                content_base,
                workdir_root: workdir_root.into(),
            }),
        }
    }

    pub async fn render_once(&self, entity: &str) -> RenderOutcome {
        if self.inner.not_found.hit(entity) {
            return RenderOutcome::NotFound;
        }
        if let Some(cached) = self.cached(entity).await {
            return cached;
        }

        let (leader, mut rx, tx) = {
            let mut map = self.inner.inflight.lock().unwrap();
            if let Some(existing) = map.get(entity) {
                (false, existing.subscribe(), existing.clone())
            } else {
                let (tx, rx) = broadcast::channel(1);
                map.insert(entity.to_string(), tx.clone());
                (true, rx, tx)
            }
        };

        if !leader {
            return match rx.recv().await {
                Ok(outcome) => outcome,
                Err(_) => Box::pin(self.render_once(entity)).await,
            };
        }

        let mut guard = InflightGuard {
            inner: Arc::clone(&self.inner),
            entity: entity.to_string(),
            tx,
            outcome: None,
        };

        let outcome = self.do_render(entity).await;
        guard.outcome = Some(outcome.clone());
        outcome
    }

    async fn cached(&self, entity: &str) -> Option<RenderOutcome> {
        let (body, face) = tokio::join!(
            self.inner.cache.get(entity, ImageKind::Body),
            self.inner.cache.get(entity, ImageKind::Face)
        );
        Some(RenderOutcome::Rendered {
            body: body?,
            face: face?,
        })
    }

    async fn do_render(&self, entity: &str) -> RenderOutcome {
        let _permit = match self.inner.limiter.acquire().await {
            Ok(p) => p,
            Err(_) => return RenderOutcome::Failed("render semaphore closed".into()),
        };

        let avatar = match resolve_avatar(&self.inner.pool, entity).await {
            ResolveResult::Avatar(v) => v,
            ResolveResult::NotFound => {
                self.inner.not_found.record(entity);
                return RenderOutcome::NotFound;
            }
            ResolveResult::Error(e) => {
                tracing::error!(entity = %entity, error = %e, "profile resolve failed");
                return RenderOutcome::Failed(format!("resolve: {e}"));
            }
        };

        let workdir =
            self.inner
                .workdir_root
                .join(format!("render-{}-{}", entity, std::process::id()));

        let result = self
            .inner
            .renderer
            .render(entity, &avatar, &self.inner.content_base, &workdir)
            .await;

        let outcome = match result {
            Ok(out) => {
                let (body, face) = tokio::join!(
                    tokio::fs::read(&out.body_path),
                    tokio::fs::read(&out.face_path)
                );
                match (body, face) {
                    (Ok(b), Ok(f)) => {
                        keep_render(&self.inner.cache, entity, Bytes::from(b), Bytes::from(f)).await
                    }
                    _ => RenderOutcome::Failed("rendered png unreadable".into()),
                }
            }
            Err(RenderError::OutputMissing { .. }) => {
                tracing::warn!(entity = %entity, "the avatar renderer produced no usable output");
                RenderOutcome::Failed("render produced no output".into())
            }
            Err(e) => {
                tracing::error!(entity = %entity, error = %e, "avatar render failed");
                RenderOutcome::Failed(e.to_string())
            }
        };

        let _ = tokio::fs::remove_dir_all(&workdir).await;
        outcome
    }
}

/// Caches a finished render and answers it. As the origin path, a render the
/// cache could not keep (a store-lock timeout included) is still served.
async fn keep_render(cache: &ImageCache, entity: &str, body: Bytes, face: Bytes) -> RenderOutcome {
    let (w1, w2) = tokio::join!(
        cache.put(entity, ImageKind::Body, ImageSource::Render, &body),
        cache.put(entity, ImageKind::Face, ImageSource::Render, &face)
    );
    if let Err(e) = w1.and(w2) {
        tracing::warn!(entity, error = %e, "a rendered profile image was not cached");
    }
    RenderOutcome::Rendered { body, face }
}

struct InflightGuard {
    inner: Arc<Inner>,
    entity: String,
    tx: broadcast::Sender<RenderOutcome>,
    outcome: Option<RenderOutcome>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        {
            let mut map = self
                .inner
                .inflight
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            map.remove(&self.entity);
        }
        let outcome = self
            .outcome
            .take()
            .unwrap_or_else(|| RenderOutcome::Failed("render task aborted".into()));
        let _ = self.tx.send(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A render the cache cannot keep (the collector holds the store lock) is still served.
    #[tokio::test]
    async fn a_render_the_cache_cannot_keep_is_still_served() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let dir =
            std::env::temp_dir().join(format!("render-keep-{}", uuid::Uuid::new_v4().simple()));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let cache = ImageCache::new(pool.clone(), storage, 60, None);
        let collector = crate::storage::gc::lock_store_exclusive(&pool)
            .await
            .unwrap();
        let outcome = keep_render(
            &cache,
            "entity",
            Bytes::from_static(b"body"),
            Bytes::from_static(b"face"),
        )
        .await;
        collector.rollback().await.unwrap();
        assert!(
            matches!(&outcome, RenderOutcome::Rendered { body, face } if body == "body" && face == "face"),
            "{outcome:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn miss_memo_remembers_recent_misses_only() {
        let memo = MissMemo::new();
        assert!(!memo.hit("a"));
        memo.record("a");
        assert!(memo.hit("a"));
        assert!(!memo.hit("b"));
        memo.map
            .lock()
            .unwrap()
            .insert("old".into(), Instant::now() - NOT_FOUND_TTL * 2);
        assert!(!memo.hit("old"));
    }
}
