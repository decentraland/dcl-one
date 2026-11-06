//! The live half of the destinations feed: the headcounts that order
//! `most_active` and fill `user_count`, and the wallets `with_connected_users`
//! lists. The old service asked two services of its fleet over HTTP; here
//! both are in the process. Headcounts are the presence collector's latest
//! snapshot (`presence.scene_occupancy` by pointer, its realms summed,
//! `presence.world_membership`
//! by world name, `live_users` when known), read once per 30 s. Connected
//! wallets come from the comms module's LiveKit rooms through
//! [`Participants`], attached once comms is up (`main`), cached 30 s (15 s
//! for an unknown answer), at most 16 lookups in flight; without comms the
//! answer is `null` ("unknown"), never an empty room.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures::future::BoxFuture;
use sqlx::PgPool;

use super::rows::{PlaceListFilters, PlaceRow};
use crate::shared::cache::TtlMap;

const COUNTS_TTL: Duration = Duration::from_secs(30);
const PARTICIPANTS_TTL: Duration = Duration::from_secs(30);
const UNKNOWN_TTL: Duration = Duration::from_secs(15);
const COMMS_CONCURRENCY: usize = 16;

/// Who stands in a scene's or a world's room; `None` when that cannot be
/// told (no comms, no scene at the pointer), an empty list when nobody does.
pub trait Participants: Send + Sync {
    fn participants<'a>(&'a self, world: bool, key: &'a str) -> BoxFuture<'a, Option<Vec<String>>>;
}

#[derive(Debug, Clone, Default)]
pub struct LiveUserCounts {
    pub places: Vec<(String, i32)>,
    pub worlds: Vec<(String, i32)>,
    /// Whether a presence snapshot exists: without one (the collector off)
    /// the rows keep the mirrored `user_count` rather than reading 0.
    pub snapshot: bool,
}

pub struct Live {
    pool: PgPool,
    counts: TtlMap<(), LiveUserCounts>,
    participants: OnceLock<Arc<dyn Participants>>,
    known: TtlMap<(bool, String), Option<Vec<String>>>,
    unknown: TtlMap<(bool, String), Option<Vec<String>>>,
}

impl Live {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            counts: TtlMap::bounded(COUNTS_TTL, 1),
            participants: OnceLock::new(),
            known: TtlMap::bounded(PARTICIPANTS_TTL, 10_000),
            unknown: TtlMap::bounded(UNKNOWN_TTL, 10_000),
        }
    }

    /// Attaches the comms module (once; a second call is ignored).
    pub fn set_participants(&self, participants: Arc<dyn Participants>) {
        let _ = self.participants.set(participants);
    }

    /// The latest presence snapshot's headcounts; empty when no snapshot
    /// exists or the read fails (the old client's empty answer).
    pub async fn live_user_counts(&self) -> LiveUserCounts {
        if let Some(v) = self.counts.get_fresh(&()) {
            return v;
        }
        let counts = read_counts(&self.pool).await.unwrap_or_else(|e| {
            tracing::debug!(error = %e, "presence headcounts read failed");
            LiveUserCounts::default()
        });
        self.counts.insert((), counts.clone());
        counts
    }

    async fn participants_of(&self, world: bool, key: String) -> Option<Vec<String>> {
        let cache_key = (world, key);
        if let Some(v) = self
            .known
            .get_fresh(&cache_key)
            .or_else(|| self.unknown.get_fresh(&cache_key))
        {
            return v;
        }
        let answer = match self.participants.get() {
            Some(p) => p.participants(world, &cache_key.1).await,
            None => None,
        };
        match &answer {
            Some(_) => self.known.insert(cache_key, answer.clone()),
            None => self.unknown.insert(cache_key, answer.clone()),
        }
        answer
    }

    /// One lookup per distinct location, at most 16 in flight.
    pub async fn connected_by_key(
        self: &Arc<Self>,
        keys: Vec<(bool, String)>,
    ) -> HashMap<(bool, String), Option<Vec<String>>> {
        let sem = Arc::new(tokio::sync::Semaphore::new(COMMS_CONCURRENCY));
        let mut set = tokio::task::JoinSet::new();
        for (world, key) in keys {
            let (me, sem) = (self.clone(), sem.clone());
            set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok();
                let addresses = me.participants_of(world, key.clone()).await;
                ((world, key), addresses)
            });
        }
        let mut out = HashMap::new();
        while let Some(res) = set.join_next().await {
            if let Ok((key, addresses)) = res {
                out.insert(key, addresses);
            }
        }
        out
    }
}

async fn read_counts(pool: &PgPool) -> Result<LiveUserCounts, sqlx::Error> {
    let snapshot: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM presence.snapshots)")
        .fetch_one(pool)
        .await?;
    if !snapshot {
        return Ok(LiveUserCounts::default());
    }
    let places: Vec<(String, i32)> = sqlx::query_as(
        "SELECT pointer, sum(count)::int4 FROM presence.scene_occupancy \
         WHERE snapshot_id = (SELECT id FROM presence.snapshots ORDER BY taken_at DESC, id DESC LIMIT 1) AND pointer <> '' \
         GROUP BY pointer",
    )
    .fetch_all(pool)
    .await?;
    let worlds: Vec<(String, i32)> = sqlx::query_as(
        "SELECT world_name, COALESCE(live_users, count) FROM presence.world_membership \
         WHERE snapshot_id = (SELECT id FROM presence.snapshots ORDER BY taken_at DESC, id DESC LIMIT 1) AND world_name <> ''",
    )
    .fetch_all(pool)
    .await?;
    Ok(LiveUserCounts {
        places,
        worlds,
        snapshot: true,
    })
}

/// `most_active` orders by the live headcounts (the old
/// `inject_live_user_counts`); other orders read none.
pub async fn inject_live_user_counts(
    live: &Live,
    filters: &mut PlaceListFilters,
) -> Option<LiveUserCounts> {
    if !matches!(filters.order_by, super::rows::PlaceOrderBy::MostActive) {
        return None;
    }
    let counts = live.live_user_counts().await;
    apply_count_order(filters, &counts);
    Some(counts)
}

/// Apply the same live ordering to legacy lists, maps and destinations.
pub fn apply_count_order(filters: &mut PlaceListFilters, counts: &LiveUserCounts) {
    if !matches!(filters.order_by, super::rows::PlaceOrderBy::MostActive) {
        return;
    }
    filters.place_user_counts = counts.places.clone();
    filters.world_user_counts = counts.worlds.clone();
}

/// Every row's `user_count` from the headcounts (a world by its lowercase
/// name, a place by its base position), 0 when absent; with no presence
/// snapshot at all, the mirrored figure (0 when none).
pub fn fill_user_counts(rows: &mut [PlaceRow], counts: &LiveUserCounts) {
    if !counts.snapshot {
        for row in rows.iter_mut() {
            row.user_count = Some(row.user_count.unwrap_or(0));
        }
        return;
    }
    let places: HashMap<&str, i32> = counts
        .places
        .iter()
        .map(|(pointer, n)| (pointer.as_str(), *n))
        .collect();
    let worlds: HashMap<String, i32> = counts
        .worlds
        .iter()
        .map(|(name, n)| (name.to_lowercase(), *n))
        .collect();
    for row in rows.iter_mut() {
        let count = if row.world {
            let name = row.world_name.as_deref().unwrap_or(&row.id);
            worlds.get(&name.to_lowercase())
        } else {
            places.get(row.base_position.as_str())
        };
        row.user_count = Some(count.copied().unwrap_or(0));
    }
}

/// The room a row names: a world by its name, a place by its base position.
pub fn comms_key(d: &PlaceRow) -> Option<(bool, String)> {
    if d.world {
        d.world_name.clone().map(|name| (true, name))
    } else {
        Some((false, d.base_position.clone()))
    }
}

/// `connected_addresses` for each row, and a `user_count` no lower than the
/// wallets seen in its room (the old rule).
pub async fn fill_connected(live: &Arc<Live>, rows: &mut [PlaceRow]) {
    let mut keys: Vec<(bool, String)> = Vec::new();
    for key in rows.iter().filter_map(comms_key) {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let connected = live.connected_by_key(keys).await;
    for d in rows.iter_mut() {
        let addresses = comms_key(d)
            .and_then(|k| connected.get(&k).cloned())
            .flatten();
        let seen = addresses.as_ref().map(|a| a.len() as i32).unwrap_or(0);
        if seen > d.user_count.unwrap_or(0) {
            d.user_count = Some(seen);
        }
        d.connected_addresses = Some(addresses);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Option<Vec<String>>);

    impl Participants for Fixed {
        fn participants<'a>(
            &'a self,
            _world: bool,
            _key: &'a str,
        ) -> BoxFuture<'a, Option<Vec<String>>> {
            let v = self.0.clone();
            Box::pin(async move { v })
        }
    }

    fn row(world: bool, id: &str, pos: &str, name: Option<&str>, count: Option<i32>) -> PlaceRow {
        PlaceRow {
            id: id.into(),
            world,
            base_position: pos.into(),
            world_name: name.map(str::to_string),
            user_count: count,
            ..PlaceRow::default()
        }
    }

    #[test]
    fn counts_fill_by_position_or_lowercase_world_name() {
        let counts = LiveUserCounts {
            places: vec![("1,2".into(), 7)],
            worlds: vec![("MyWorld.dcl.eth".into(), 3)],
            snapshot: true,
        };
        let mut rows = vec![
            row(false, "p", "1,2", None, None),
            row(true, "w", "0,0", Some("myworld.DCL.eth"), None),
            row(false, "q", "9,9", None, Some(40)),
        ];
        fill_user_counts(&mut rows, &counts);
        assert_eq!(
            rows.iter().map(|r| r.user_count).collect::<Vec<_>>(),
            [Some(7), Some(3), Some(0)]
        );
        let mut mirrored = vec![
            row(false, "q", "9,9", None, Some(40)),
            row(false, "r", "8,8", None, None),
        ];
        fill_user_counts(&mut mirrored, &LiveUserCounts::default());
        assert_eq!(
            mirrored.iter().map(|r| r.user_count).collect::<Vec<_>>(),
            [Some(40), Some(0)],
            "no snapshot: the mirrored figure"
        );
    }

    /// The old `destinations_connected_tests`: omitted unless asked (the
    /// flag gates the call site), an empty room and an unknown one read
    /// differently, and an unreachable source leaves each row unknown.
    #[tokio::test]
    async fn an_empty_room_and_an_unknown_one_read_differently() {
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let live = Arc::new(Live::new(pool.clone()));
        let mut rows = vec![
            row(false, "p", "1,2", None, Some(0)),
            row(true, "w", "0,0", Some("w.dcl.eth"), Some(0)),
        ];
        fill_connected(&live, &mut rows).await;
        assert!(
            rows.iter().all(|r| r.connected_addresses == Some(None)),
            "no comms attached: unknown"
        );

        let empty = Arc::new(Live::new(pool.clone()));
        empty.set_participants(Arc::new(Fixed(Some(vec![]))));
        let mut rows = vec![row(false, "p", "1,2", None, Some(0))];
        fill_connected(&empty, &mut rows).await;
        assert_eq!(rows[0].connected_addresses, Some(Some(vec![])));

        let two = Arc::new(Live::new(pool));
        two.set_participants(Arc::new(Fixed(Some(vec!["0xa".into(), "0xb".into()]))));
        let mut rows = vec![row(false, "p", "1,2", None, Some(1))];
        fill_connected(&two, &mut rows).await;
        assert_eq!(
            (rows[0].connected_addresses.clone(), rows[0].user_count),
            (
                Some(Some(vec!["0xa".to_string(), "0xb".to_string()])),
                Some(2)
            ),
            "the room's wallets raise the headcount"
        );
    }
}
