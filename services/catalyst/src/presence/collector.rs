//! One collection pass: read the sources, write the snapshot and its five
//! child relations in ONE transaction, refresh the `/current*` memo, roll
//! yesterday up into `daily_stats` once a day, and, when a retention is
//! configured, delete a bounded batch of expired snapshots. The clock is a
//! parameter throughout (`taken_at`, the rollup date, the retention cutoff):
//! nothing here reads `now()` in SQL.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Timelike, Utc};
use futures::{StreamExt, TryStreamExt};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::model::{self, HotScene, Island, Peer};
use super::sources::Sources;
use super::{PARTICIPANT_FANOUT, PresenceState, RETENTION_SWEEP_LIMIT};

#[derive(Debug, Clone)]
pub struct SnapshotSummary {
    pub snapshot_id: i64,
    pub peers: i32,
    pub islands: i32,
    pub hot_scenes: i32,
    pub scenes_polled: i32,
    pub scene_users: i32,
    pub worlds_polled: i32,
    pub active_worlds: i32,
    pub world_users: i32,
}

/// One pass, fully read; `store_snapshot` writes it.
#[derive(Debug, Clone)]
pub struct SnapshotData {
    pub taken_at: NaiveDateTime,
    pub realm: String,
    pub peers: Vec<Peer>,
    pub islands: Vec<Island>,
    pub hot_scenes: Vec<HotScene>,
    /// `(pointer, scene name, wallets)` per hot scene comms answered for.
    pub occupancy: Vec<(String, Option<String>, Vec<String>)>,
    /// `(world, wallets, live-data users)` per active world.
    pub world_rows: Vec<(String, Vec<String>, i32)>,
    pub worlds_live_total: Option<i32>,
}

/// The pass's clock as the tables hold it: `timestamp` keeps microseconds,
/// so the value the memo, the log and the wire carry is truncated once here
/// rather than disagreeing with the stored row in its last three digits.
pub fn pass_clock(now: DateTime<Utc>) -> NaiveDateTime {
    let t = now.naive_utc();
    t.with_nanosecond(t.nanosecond() / 1_000 * 1_000)
        .unwrap_or(t)
}

/// Reads every source: the four list reads together, then the per-scene and
/// per-world rosters at most [`PARTICIPANT_FANOUT`] at a time, in order,
/// each roster lowercased and deduplicated ([`model::dedup_lower`]) whatever
/// the source handed over; the first failure fails the pass (a partial
/// snapshot would read as a quiet realm).
pub async fn fetch(
    sources: &dyn Sources,
    realm: &str,
    taken_at: NaiveDateTime,
) -> Result<SnapshotData> {
    let (peers, islands, hot_scenes, live) = tokio::try_join!(
        sources.peers(),
        sources.islands(),
        sources.hot_scenes(),
        sources.worlds_live_data()
    )?;
    let (active_worlds, worlds_live_total) = match live {
        Some(live) => (
            model::active_worlds(live.per_world),
            Some(model::clamp_i32(live.total_users)),
        ),
        None => (Vec::new(), None),
    };

    let pointers: Vec<(String, Option<String>)> = hot_scenes
        .iter()
        .filter_map(|s| model::hot_scene_pointer(s).map(|p| (p, s.name.clone())))
        .collect();
    let targets: Vec<(bool, String)> = pointers
        .iter()
        .map(|(p, _)| (false, p.clone()))
        .chain(active_worlds.iter().map(|w| (true, w.world_name.clone())))
        .collect();
    let responses: Vec<Option<Vec<String>>> = futures::stream::iter(targets)
        .map(|(is_world, key)| async move {
            let roster = if is_world {
                sources.world_participants(&key).await
            } else {
                sources.scene_participants(realm, &key).await
            };
            roster.map(|r| {
                r.map(|addresses| model::dedup_lower(addresses.iter().map(String::as_str)))
            })
        })
        .buffered(PARTICIPANT_FANOUT)
        .try_collect()
        .await
        .context("participant rosters")?;
    let (scene_resps, world_resps) = responses.split_at(pointers.len());

    let occupancy = pointers
        .into_iter()
        .zip(scene_resps)
        .filter_map(|((pointer, name), resp)| {
            resp.clone().map(|addresses| (pointer, name, addresses))
        })
        .collect();
    let world_rows = active_worlds
        .iter()
        .zip(world_resps)
        .map(|(w, resp)| {
            (
                w.world_name.clone(),
                resp.clone().unwrap_or_default(),
                w.users,
            )
        })
        .collect();

    Ok(SnapshotData {
        taken_at,
        realm: realm.to_string(),
        peers,
        islands,
        hot_scenes,
        occupancy,
        world_rows,
        worlds_live_total,
    })
}

pub fn summarize(snapshot_id: i64, data: &SnapshotData) -> SnapshotSummary {
    SnapshotSummary {
        snapshot_id,
        peers: data.peers.len() as i32,
        islands: data.islands.len() as i32,
        hot_scenes: data.hot_scenes.len() as i32,
        scenes_polled: data.occupancy.len() as i32,
        scene_users: data.occupancy.iter().map(|(_, _, a)| a.len() as i32).sum(),
        worlds_polled: data.world_rows.len() as i32,
        active_worlds: data
            .world_rows
            .iter()
            .filter(|(_, a, _)| !a.is_empty())
            .count() as i32,
        world_users: data.world_rows.iter().map(|(_, a, _)| a.len() as i32).sum(),
    }
}

/// The snapshot row and its five child sets, one `unnest` INSERT each, in
/// one transaction; a duplicate key within a pass keeps the first row
/// (`ON CONFLICT DO NOTHING`), as the old writer did.
pub async fn store_snapshot(pool: &PgPool, data: &SnapshotData) -> Result<i64> {
    let summary = summarize(0, data);
    let mut tx = pool.begin().await.context("begin tx")?;

    let snapshot_id: i64 = sqlx::query_scalar(
        "INSERT INTO presence.snapshots \
            (taken_at, peers_count, islands_count, hot_scenes_count, \
             scenes_polled, scene_users_total, \
             worlds_polled, active_worlds, world_users_total, worlds_live_total) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id",
    )
    .bind(data.taken_at)
    .bind(summary.peers)
    .bind(summary.islands)
    .bind(summary.hot_scenes)
    .bind(summary.scenes_polled)
    .bind(summary.scene_users)
    .bind(summary.worlds_polled)
    .bind(summary.active_worlds)
    .bind(summary.world_users)
    .bind(data.worlds_live_total)
    .fetch_one(&mut *tx)
    .await
    .context("insert snapshot")?;

    if !data.peers.is_empty() {
        let p = &data.peers;
        sqlx::query(
            "INSERT INTO presence.peer_snapshots \
                (snapshot_id, address, parcel_x, parcel_y, position_x, position_y, position_z, last_ping) \
             SELECT $1, * FROM unnest($2::text[], $3::int4[], $4::int4[], \
                                      $5::float8[], $6::float8[], $7::float8[], $8::int8[]) \
             ON CONFLICT DO NOTHING",
        )
        .bind(snapshot_id)
        .bind(p.iter().map(|x| x.address.clone()).collect::<Vec<String>>())
        .bind(p.iter().map(|x| x.parcel_x).collect::<Vec<Option<i32>>>())
        .bind(p.iter().map(|x| x.parcel_y).collect::<Vec<Option<i32>>>())
        .bind(p.iter().map(|x| x.position_x).collect::<Vec<Option<f64>>>())
        .bind(p.iter().map(|x| x.position_y).collect::<Vec<Option<f64>>>())
        .bind(p.iter().map(|x| x.position_z).collect::<Vec<Option<f64>>>())
        .bind(p.iter().map(|x| x.last_ping).collect::<Vec<Option<i64>>>())
        .execute(&mut *tx)
        .await
        .context("insert peers")?;
    }

    if !data.islands.is_empty() {
        let i = &data.islands;
        sqlx::query(
            "INSERT INTO presence.island_snapshots \
                (snapshot_id, island_id, peer_count, max_peers, center_x, center_y, center_z, radius) \
             SELECT $1, * FROM unnest($2::text[], $3::int4[], $4::int4[], \
                                      $5::float8[], $6::float8[], $7::float8[], $8::float8[]) \
             ON CONFLICT DO NOTHING",
        )
        .bind(snapshot_id)
        .bind(i.iter().map(|x| x.island_id.clone()).collect::<Vec<String>>())
        .bind(i.iter().map(|x| x.peer_count).collect::<Vec<i32>>())
        .bind(i.iter().map(|x| x.max_peers).collect::<Vec<Option<i32>>>())
        .bind(i.iter().map(|x| x.center_x).collect::<Vec<Option<f64>>>())
        .bind(i.iter().map(|x| x.center_y).collect::<Vec<Option<f64>>>())
        .bind(i.iter().map(|x| x.center_z).collect::<Vec<Option<f64>>>())
        .bind(i.iter().map(|x| x.radius).collect::<Vec<Option<f64>>>())
        .execute(&mut *tx)
        .await
        .context("insert islands")?;
    }

    if !data.hot_scenes.is_empty() {
        let s = &data.hot_scenes;
        sqlx::query(
            "INSERT INTO presence.hot_scene_snapshots \
                (snapshot_id, scene_id, name, base_x, base_y, users_count, parcel_count, creator, description) \
             SELECT $1, * FROM unnest($2::text[], $3::text[], $4::int4[], $5::int4[], \
                                      $6::int4[], $7::int4[], $8::text[], $9::text[]) \
             ON CONFLICT DO NOTHING",
        )
        .bind(snapshot_id)
        .bind(s.iter().map(|x| x.scene_id.clone()).collect::<Vec<String>>())
        .bind(s.iter().map(|x| x.name.clone()).collect::<Vec<Option<String>>>())
        .bind(s.iter().map(|x| x.base_x).collect::<Vec<Option<i32>>>())
        .bind(s.iter().map(|x| x.base_y).collect::<Vec<Option<i32>>>())
        .bind(s.iter().map(|x| x.users_count).collect::<Vec<Option<i32>>>())
        .bind(s.iter().map(|x| x.parcel_count).collect::<Vec<i32>>())
        .bind(s.iter().map(|x| x.creator.clone()).collect::<Vec<Option<String>>>())
        .bind(s.iter().map(|x| x.description.clone()).collect::<Vec<Option<String>>>())
        .execute(&mut *tx)
        .await
        .context("insert hot scenes")?;
    }

    if !data.occupancy.is_empty() {
        let o = &data.occupancy;
        sqlx::query(
            "INSERT INTO presence.scene_occupancy \
                (snapshot_id, taken_at, pointer, scene_name, realm, addresses, count) \
             SELECT $1, $2, t.pointer, t.scene_name, $3, t.addresses, t.count \
             FROM unnest($4::text[], $5::text[], $6::jsonb[], $7::int4[]) \
                  AS t(pointer, scene_name, addresses, count) \
             ON CONFLICT DO NOTHING",
        )
        .bind(snapshot_id)
        .bind(data.taken_at)
        .bind(&data.realm)
        .bind(o.iter().map(|(p, _, _)| p.clone()).collect::<Vec<String>>())
        .bind(
            o.iter()
                .map(|(_, n, _)| n.clone())
                .collect::<Vec<Option<String>>>(),
        )
        .bind(o.iter().map(|(_, _, a)| json!(a)).collect::<Vec<Value>>())
        .bind(
            o.iter()
                .map(|(_, _, a)| a.len() as i32)
                .collect::<Vec<i32>>(),
        )
        .execute(&mut *tx)
        .await
        .context("insert scene occupancy")?;
    }

    if !data.world_rows.is_empty() {
        let w = &data.world_rows;
        sqlx::query(
            "INSERT INTO presence.world_membership \
                (snapshot_id, taken_at, world_name, addresses, count, live_users) \
             SELECT $1, $2, t.world_name, t.addresses, t.count, t.live_users \
             FROM unnest($3::text[], $4::jsonb[], $5::int4[], $6::int4[]) \
                  AS t(world_name, addresses, count, live_users) \
             ON CONFLICT DO NOTHING",
        )
        .bind(snapshot_id)
        .bind(data.taken_at)
        .bind(w.iter().map(|(n, _, _)| n.clone()).collect::<Vec<String>>())
        .bind(w.iter().map(|(_, a, _)| json!(a)).collect::<Vec<Value>>())
        .bind(
            w.iter()
                .map(|(_, a, _)| a.len() as i32)
                .collect::<Vec<i32>>(),
        )
        .bind(w.iter().map(|(_, _, l)| *l).collect::<Vec<i32>>())
        .execute(&mut *tx)
        .await
        .context("insert world membership")?;
    }

    tx.commit().await.context("commit snapshot")?;
    Ok(snapshot_id)
}

/// The per-UTC-day rollup, upserted from the day's snapshots (derived:
/// re-running it rebuilds the row).
pub async fn aggregate_day(pool: &PgPool, date: NaiveDate) -> Result<()> {
    sqlx::query(
        "INSERT INTO presence.daily_stats \
            (date, snapshots_taken, peak_peers, avg_peers, \
             peak_hot_scene_users, peak_scene_users, peak_world_users) \
         SELECT $1::date, \
                COUNT(*)::int, \
                COALESCE(MAX(peers_count), 0)::int, \
                COALESCE(AVG(peers_count), 0)::double precision, \
                COALESCE((SELECT MAX(hs.users_count) FROM presence.hot_scene_snapshots hs \
                          JOIN presence.snapshots s2 ON s2.id = hs.snapshot_id \
                          WHERE s2.taken_at >= $1::date AND s2.taken_at < ($1::date + 1)), 0)::int, \
                COALESCE((SELECT MAX(so.count) FROM presence.scene_occupancy so \
                          JOIN presence.snapshots s3 ON s3.id = so.snapshot_id \
                          WHERE s3.taken_at >= $1::date AND s3.taken_at < ($1::date + 1)), 0)::int, \
                COALESCE((SELECT MAX(wm.count) FROM presence.world_membership wm \
                          JOIN presence.snapshots s4 ON s4.id = wm.snapshot_id \
                          WHERE s4.taken_at >= $1::date AND s4.taken_at < ($1::date + 1)), 0)::int \
         FROM presence.snapshots s \
         WHERE s.taken_at >= $1::date AND s.taken_at < ($1::date + 1) \
         ON CONFLICT (date) DO UPDATE SET \
            snapshots_taken = EXCLUDED.snapshots_taken, \
            peak_peers = EXCLUDED.peak_peers, \
            avg_peers = EXCLUDED.avg_peers, \
            peak_hot_scene_users = EXCLUDED.peak_hot_scene_users, \
            peak_scene_users = EXCLUDED.peak_scene_users, \
            peak_world_users = EXCLUDED.peak_world_users",
    )
    .bind(date)
    .execute(pool)
    .await
    .context("aggregate daily_stats")?;
    Ok(())
}

/// Deletes at most `limit` of the oldest snapshots taken before `cutoff`
/// (the children go with them by `ON DELETE CASCADE`); returns how many.
/// Bounded so a pass that first enables a retention pays for one day's
/// worth of snapshots, never a year's.
pub async fn sweep_retention(pool: &PgPool, cutoff: NaiveDateTime, limit: i64) -> Result<u64> {
    let done = sqlx::query(
        "WITH due AS (SELECT id FROM presence.snapshots WHERE taken_at < $1 ORDER BY taken_at, id LIMIT $2) \
         DELETE FROM presence.snapshots s USING due WHERE s.id = due.id",
    )
    .bind(cutoff)
    .bind(limit)
    .execute(pool)
    .await
    .context("retention sweep")?;
    Ok(done.rows_affected())
}

/// The collector job's body at `now`. Refuses on a node whose data-plane
/// writes are disabled: the archive is local authority, and a pass recorded
/// here would be lost with the node.
pub async fn run_pass(state: &PresenceState, now: DateTime<Utc>) -> Result<SnapshotSummary> {
    // The switch alone, not the restore stamps: a pass records this node's own regenerable observations.
    if !state.settings.writes_enabled {
        anyhow::bail!(
            "the presence collector records nothing while {}=false",
            state.settings.switch
        );
    }
    let data = fetch(
        state.sources.as_ref(),
        &state.settings.genesis_realm,
        pass_clock(now),
    )
    .await?;
    let snapshot_id = store_snapshot(&state.pool, &data).await?;
    let summary = summarize(snapshot_id, &data);
    tracing::info!(
        snapshot_id,
        peers = summary.peers,
        hot_scenes = summary.hot_scenes,
        scene_users = summary.scene_users,
        world_users = summary.world_users,
        "presence snapshot recorded"
    );
    if let Err(e) = state.queries.refresh_current().await {
        tracing::warn!(error = %e, "presence: current memo refresh failed; serving on demand");
    }

    let yesterday = (now - chrono::Duration::days(1)).date_naive();
    if *state.last_aggregated.lock() != Some(yesterday) {
        match aggregate_day(&state.pool, yesterday).await {
            Ok(()) => *state.last_aggregated.lock() = Some(yesterday),
            Err(e) => {
                tracing::error!(error = %e, date = %yesterday, "presence: daily aggregation failed")
            }
        }
    }

    if let Some(days) = state.settings.retention_days {
        let cutoff = (now - chrono::Duration::days(i64::from(days))).naive_utc();
        match sweep_retention(&state.pool, cutoff, RETENTION_SWEEP_LIMIT).await {
            Ok(0) => {}
            Ok(n) => tracing::info!(
                deleted = n,
                retention_days = days,
                "presence: expired snapshots deleted"
            ),
            Err(e) => tracing::error!(error = %e, "presence: retention sweep failed"),
        }
    }
    Ok(summary)
}
