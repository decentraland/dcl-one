//! The badges half of the gatekeeper's room events: every wallet join or
//! leave of a scene or world room is inserted into `social.presence_events`
//! before the webhook is acknowledged (keyed by LiveKit's event id, so a
//! redelivered webhook is one row); a failed insert fails the webhook and
//! LiveKit delivers it again. The engine applies the rows later.
//! Only events in a place a presence badge could count are stored: the
//! sink checks them against a short-lived snapshot of those places, and
//! looks up a scene entity the snapshot does not know (a redeploy).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::comms::gatekeeper::events::RoomEventStore;
use crate::comms::protocol::bus::room_events::{RoomEvent, RoomEventKind, RoomType};
use crate::identity::Address;
use crate::shared::cache::TtlMap;

use super::Settings;
use super::engine::naive;

/// How long a badge configured, published or archived waits to be seen.
const PLACES_TTL: Duration = Duration::from_secs(5);

/// The places of the badges whose windows an event now could count for (the
/// engine's join lookahead and leave lookback): worlds by name, scenes by
/// parcel and by every entity deployed on one of them when the snapshot was read.
#[derive(Debug, Default)]
struct ActivePlaces {
    worlds: HashSet<String>,
    parcels: Vec<String>,
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
        .bind(naive(now.saturating_sub(settings.max_age_ms)))
        .bind(naive(now.saturating_add(settings.lookahead_ms)))
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
            parcels,
            scenes: scenes.into_iter().collect(),
        })
    }

    /// The scene entities of `batch` the snapshot does not know, when a scene
    /// badge is active.
    fn unknown_scenes(&self, batch: &[PresenceEvent]) -> Vec<String> {
        if self.parcels.is_empty() {
            return Vec::new();
        }
        let unknown: HashSet<&String> = batch
            .iter()
            .filter(|e| !e.world)
            .filter_map(|e| e.scene_id.as_ref())
            .filter(|s| !self.scenes.contains(*s))
            .collect();
        unknown.into_iter().cloned().collect()
    }

    /// Of `ids`, the entities deployed off every badge parcel. One the node
    /// does not hold yet is not among them: the engine decides it.
    async fn off_parcels(&self, pool: &PgPool, ids: &[String]) -> sqlx::Result<HashSet<String>> {
        let off: Vec<String> = sqlx::query_scalar(
            "SELECT entity_id FROM content.deployments \
             WHERE entity_id = ANY($1) AND entity_type = 'scene' \
               AND NOT entity_pointers && $2",
        )
        .bind(ids)
        .bind(&self.parcels)
        .fetch_all(pool)
        .await?;
        Ok(off.into_iter().collect())
    }

    fn counts(&self, event: &PresenceEvent, off: &HashSet<String>) -> bool {
        if event.world {
            self.worlds.contains(&event.realm_name)
        } else {
            event.scene_id.as_ref().is_some_and(|s| {
                self.scenes.contains(s) || (!self.parcels.is_empty() && !off.contains(s))
            })
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
    pool: PgPool,
    settings: Settings,
    places: TtlMap<(), Arc<ActivePlaces>>,
}

impl EventSink {
    /// A sink with the engine's `settings`, so both agree on what a badge
    /// could count.
    pub fn new(pool: PgPool, settings: Settings) -> Self {
        Self {
            pool,
            settings,
            places: TtlMap::new(PLACES_TTL),
        }
    }

    /// The events a presence badge could count. When the places cannot be
    /// read every event is kept: the engine decides again.
    async fn counted(&self, batch: &mut Vec<PresenceEvent>) {
        let pool = &self.pool;
        let places = self
            .places
            .get_or_fetch((), || async {
                let now = chrono::Utc::now().timestamp_millis();
                ActivePlaces::load(pool, now, &self.settings)
                    .await
                    .map(Arc::new)
            })
            .await;
        let places = match places {
            Ok(places) => places,
            Err(error) => {
                tracing::warn!(%error, "presence badge places not read; keeping every event");
                return;
            }
        };
        let unknown = places.unknown_scenes(batch);
        let off = if unknown.is_empty() {
            HashSet::new()
        } else {
            match places.off_parcels(pool, &unknown).await {
                Ok(off) => off,
                Err(error) => {
                    tracing::warn!(%error, "presence scenes not read; keeping their events");
                    HashSet::new()
                }
            }
        };
        batch.retain(|e| places.counts(e, &off));
    }

    /// Stores the event when a badge could count it; the rows inserted.
    pub async fn record(&self, event: PresenceEvent) -> Result<u64, sqlx::Error> {
        let mut batch = vec![event];
        self.counted(&mut batch).await;
        insert(&self.pool, &batch).await
    }
}

#[async_trait::async_trait]
impl RoomEventStore for EventSink {
    async fn store_room_event(&self, event: &RoomEvent) -> anyhow::Result<()> {
        let Some(event) = PresenceEvent::from_room_event(event) else {
            return Ok(());
        };
        self.record(event).await?;
        Ok(())
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
