//! The badges half of the gatekeeper's room events: every wallet join or
//! leave of a scene or world room is queued here without blocking the
//! webhook, and a writer task inserts the queue into
//! `social.presence_events` in batches (keyed by LiveKit's event id, so a
//! redelivered webhook is one row). The engine applies the rows later.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::comms::gatekeeper::events::RoomEventSink;
use crate::comms::protocol::bus::room_events::{RoomEvent, RoomEventKind, RoomType};
use crate::identity::Address;

/// Events queued between the webhook and the writer; past it they are dropped.
pub const QUEUE: usize = 10_000;
const BATCH: usize = 500;
/// The pause before a failed batch insert is tried once more.
const RETRY_BACKOFF: Duration = Duration::from_secs(1);

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
        }
    }

    /// Stores `batch`, trying once more after a pause.
    async fn store(&self, pool: &PgPool, batch: Vec<PresenceEvent>) -> Result<u64, sqlx::Error> {
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
