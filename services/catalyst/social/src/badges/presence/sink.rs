//! The badges half of the gatekeeper's room events: every wallet join or
//! leave of a scene or world room is queued here without blocking the
//! webhook, and a writer task inserts the queue into
//! `social.presence_events` in batches (keyed by LiveKit's event id, so a
//! redelivered webhook is one row). The engine applies the rows later.
//! Only events in a place a presence badge could count are stored: the
//! writer checks them against a short-lived snapshot of those places.

use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::comms::gatekeeper::events::RoomEventSink;
use crate::comms::protocol::bus::room_events::{RoomEvent, RoomEventKind, RoomType};
use crate::identity::Address;
use crate::shared::cache::TtlMap;

use super::Settings;
use super::engine::naive;

/// Events queued between the webhook and the writer; past it they are dropped.
pub const QUEUE: usize = 10_000;
const BATCH: usize = 500;
/// How long a badge configured, published or archived waits to be seen.
const PLACES_TTL: Duration = Duration::from_secs(30);
/// The pause before a failed batch insert is tried once more.
const RETRY_BACKOFF: Duration = Duration::from_secs(1);

/// The places of the badges whose windows an event now could count for (the
/// engine's join lookahead and leave lookback): worlds by name, scenes by
/// every entity ever deployed on one of their parcels.
#[derive(Debug, Default)]
struct ActivePlaces {
    worlds: HashSet<String>,
    scenes: HashSet<String>,
}

impl ActivePlaces {
    async fn load(pool: &PgPool, now: i64, settings: &Settings) -> sqlx::Result<Self> {
        let (worlds, parcels): (Vec<String>, Vec<String>) = sqlx::query_as(
            "SELECT COALESCE(array_agg(DISTINCT lower(p.world_name)) \
                       FILTER (WHERE p.location_kind = 'world' AND p.world_name IS NOT NULL), '{}'), \
                    COALESCE(array_agg(DISTINCT lower(trim(p.parcel))) \
                       FILTER (WHERE p.location_kind = 'scene' AND p.parcel IS NOT NULL), '{}') \
             FROM social.presence_badges p \
             JOIN social.badge_definitions d ON d.id = p.badge_id \
              AND d.published_at IS NOT NULL AND d.archived_at IS NULL \
             WHERE p.window_end > $1 AND p.window_start <= $2",
        )
        .bind(naive(now - settings.max_age_ms))
        .bind(naive(now + settings.lookahead_ms))
        .fetch_one(pool)
        .await?;
        let scenes = if parcels.is_empty() {
            Vec::new()
        } else {
            sqlx::query_scalar(
                "SELECT entity_id FROM content.deployments \
                 WHERE entity_type = 'scene' AND entity_pointers && $1",
            )
            .bind(&parcels)
            .fetch_all(pool)
            .await?
        };
        Ok(Self {
            worlds: worlds.into_iter().collect(),
            scenes: scenes.into_iter().collect(),
        })
    }

    fn counts(&self, event: &PresenceEvent) -> bool {
        if event.world {
            self.worlds.contains(&event.realm_name)
        } else {
            event
                .scene_id
                .as_ref()
                .is_some_and(|s| self.scenes.contains(s))
        }
    }
}

/// One row of `social.presence_events`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceEvent {
    pub event_id: String,
    pub join: bool,
    pub world: bool,
    pub realm_name: String,
    pub scene_id: Option<String>,
    pub wallet: String,
    pub session_id: String,
    pub event_time_ms: i64,
}

impl PresenceEvent {
    /// The event as a badge input: a wallet's join or leave of a scene or
    /// world room with its session. `authoritative-server`, streamers and any
    /// other identity that is not exactly a wallet are refused.
    pub fn from_room_event(event: &RoomEvent) -> Option<Self> {
        let join = match event.kind() {
            RoomEventKind::Joined => true,
            RoomEventKind::Left => false,
            RoomEventKind::Unspecified => return None,
        };
        let world = match event.room_type() {
            RoomType::World => true,
            RoomType::Scene => false,
            RoomType::Unknown => return None,
        };
        let wallet = Address::parse_untrimmed(&event.identity).ok()?;
        let time = i64::try_from(event.event_time_ms).ok().filter(|t| *t > 0)?;
        if event.event_id.is_empty()
            || event.event_id.len() > 128
            || event.participant_sid.is_empty()
            || event.participant_sid.len() > 128
            || event.realm_name.is_empty()
            || event.realm_name.len() > 512
            || event.scene_id.len() > 512
        {
            return None;
        }
        Some(Self {
            event_id: event.event_id.clone(),
            join,
            world,
            realm_name: event.realm_name.to_lowercase(),
            scene_id: (!event.scene_id.is_empty()).then(|| event.scene_id.clone()),
            wallet: wallet.into_string(),
            session_id: event.participant_sid.clone(),
            event_time_ms: time,
        })
    }
}

pub struct EventSink {
    tx: mpsc::Sender<PresenceEvent>,
    rx: Mutex<Option<mpsc::Receiver<PresenceEvent>>>,
    settings: Settings,
    places: TtlMap<(), Arc<ActivePlaces>>,
}

impl Default for EventSink {
    fn default() -> Self {
        Self::with_capacity(QUEUE)
    }
}

impl EventSink {
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self {
            tx,
            rx: Mutex::new(Some(rx)),
            settings: Settings::default(),
            places: TtlMap::new(PLACES_TTL),
        }
    }

    /// The events a presence badge could count. When the places cannot be
    /// read every event is kept: the engine decides again.
    async fn counted(&self, pool: &PgPool, batch: &mut Vec<PresenceEvent>) {
        let places = self
            .places
            .get_or_fetch((), || async {
                let now = chrono::Utc::now().timestamp_millis();
                ActivePlaces::load(pool, now, &self.settings)
                    .await
                    .map(Arc::new)
            })
            .await;
        match places {
            Ok(places) => batch.retain(|e| places.counts(e)),
            Err(error) => {
                tracing::warn!(%error, "presence badge places not read; keeping every event")
            }
        }
    }

    /// Stores the counted events of `batch`, trying once more after a pause.
    async fn store(
        &self,
        pool: &PgPool,
        mut batch: Vec<PresenceEvent>,
    ) -> Result<u64, sqlx::Error> {
        self.counted(pool, &mut batch).await;
        retry_once(RETRY_BACKOFF, || insert(pool, &batch)).await
    }

    /// Stores whatever is queued now, without the writer (tests). A no-op
    /// once the writer owns the receiver.
    pub async fn flush(&self, pool: &PgPool) -> Result<u64, sqlx::Error> {
        let mut batch = Vec::new();
        if let Some(rx) = self.rx.lock().as_mut() {
            while let Ok(event) = rx.try_recv() {
                batch.push(event);
            }
        }
        self.store(pool, batch).await
    }

    /// The `badges-presence-events` writer, on every process that receives
    /// webhooks: drains the queue in batches until shutdown, then what is left.
    pub fn spawn_writer(self: &Arc<Self>, pool: PgPool, jobs: &crate::jobs::Jobs) {
        let Some(mut rx) = self.rx.lock().take() else {
            return;
        };
        let sink = self.clone();
        jobs.spawn("badges-presence-events", move |ctx| async move {
            let mut batch = Vec::with_capacity(BATCH);
            loop {
                let received = tokio::select! {
                    n = rx.recv_many(&mut batch, BATCH) => n,
                    _ = ctx.cancel.cancelled() => {
                        while let Ok(event) = rx.try_recv() {
                            batch.push(event);
                        }
                        0
                    }
                };
                let rows = batch.len();
                if rows > 0
                    && let Err(error) = sink.store(&pool, std::mem::take(&mut batch)).await
                {
                    metrics::counter!("badges_presence_events_dropped_total", "reason" => "insert_failed")
                        .increment(rows as u64);
                    tracing::warn!(%error, rows, "presence events not stored");
                }
                batch.clear();
                if received == 0 {
                    return Ok(());
                }
            }
        });
    }
}

impl RoomEventSink for EventSink {
    fn observe_room_event(&self, event: &RoomEvent) {
        let Some(event) = PresenceEvent::from_room_event(event) else {
            return;
        };
        if let Err(error) = self.tx.try_send(event) {
            let reason = match error {
                mpsc::error::TrySendError::Full(_) => "queue_full",
                mpsc::error::TrySendError::Closed(_) => "writer_gone",
            };
            metrics::counter!("badges_presence_events_dropped_total", "reason" => reason)
                .increment(1);
            tracing::warn!(reason, "presence event dropped");
        }
    }
}

/// Runs `op` once more after `backoff` when it fails, so a brief database
/// blip loses nothing.
pub(super) async fn retry_once<T, F, Fut>(backoff: Duration, mut op: F) -> Result<T, sqlx::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, sqlx::Error>>,
{
    match op().await {
        Ok(value) => Ok(value),
        Err(error) => {
            tracing::warn!(%error, "presence events not stored; retrying");
            tokio::time::sleep(backoff).await;
            op().await
        }
    }
}

pub async fn insert(pool: &PgPool, events: &[PresenceEvent]) -> Result<u64, sqlx::Error> {
    if events.is_empty() {
        return Ok(0);
    }
    let col = |f: fn(&PresenceEvent) -> String| events.iter().map(f).collect::<Vec<_>>();
    let inserted = sqlx::query(
        "INSERT INTO social.presence_events \
           (event_id, kind, room_type, realm_name, scene_id, wallet, session_id, event_time_ms) \
         SELECT * FROM unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], \
                              $6::text[], $7::text[], $8::bigint[]) \
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(col(|e| e.event_id.clone()))
    .bind(col(|e| if e.join { "join" } else { "leave" }.into()))
    .bind(col(|e| if e.world { "world" } else { "scene" }.into()))
    .bind(col(|e| e.realm_name.clone()))
    .bind(
        events
            .iter()
            .map(|e| e.scene_id.clone())
            .collect::<Vec<_>>(),
    )
    .bind(col(|e| e.wallet.clone()))
    .bind(col(|e| e.session_id.clone()))
    .bind(events.iter().map(|e| e.event_time_ms).collect::<Vec<_>>())
    .execute(pool)
    .await?
    .rows_affected();
    Ok(inserted)
}
