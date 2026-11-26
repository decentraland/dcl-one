//! An operator's ranking of a catalogue row: the admin token's ranking (any
//! value, `null` clears it), the data team's automated ranking (never over a
//! highlighted or excluded row), the ranking exclusion (which also pins the
//! ranking at 0) and the replace of the whole automated set of each type a
//! run declares (`PUT /api/destinations/ranking`).
//!
//! The old service wrote `raw.ranking` / `raw.exclude_from_ranking` of its
//! `place` row and the next mirror pass wrote upstream's back. Here the
//! decision is `explore.place_ranking` (keyed by the catalogue id, `overlay`
//! the raw keys decided), merged into `raw` of the row in the same
//! transaction and re-applied by the places mirror's upsert and the derive
//! pass (`raw || overlay`), so a pass never undoes it.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Row};

use super::query::EXCLUDE_FROM_RANKING_SQL;
use crate::shared::errors::ApiError;

pub(crate) const RANKING_IS_SET_SQL: &str =
    "jsonb_typeof(raw->'ranking') = 'number' AND raw->'ranking' <> '0'::jsonb";

#[derive(Debug, Clone, PartialEq)]
pub struct RankingEntry {
    pub id: String,
    pub ranking: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ReplaceRankingPlaces {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub applied: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub cleared: i64,
    /// Whether the run declared this type in scope; `false`: the leg was left untouched.
    pub replaced: bool,
    pub skipped_curated: Vec<String>,
    pub skipped_world_backed: Vec<String>,
    pub skipped_missing: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ReplaceRankingWorlds {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub applied: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub cleared: i64,
    pub replaced: bool,
    pub skipped_curated: Vec<String>,
    pub skipped_missing: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ReplaceRankingResult {
    pub places: ReplaceRankingPlaces,
    pub worlds: ReplaceRankingWorlds,
}

/// The legs a replace may write and clear; a leg out of scope is left untouched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplaceScope {
    pub places: bool,
    pub worlds: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreRankingOutcome {
    Stored,
    RefusedCurated,
    NotWritable,
}

pub(crate) struct CurationRow {
    pub id: String,
    pub highlighted: bool,
    pub exclude_from_ranking: bool,
    pub world: bool,
}

pub(crate) struct WorldMatch {
    pub submitted: String,
    pub row_id: String,
    pub highlighted: bool,
    pub exclude_from_ranking: bool,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct PlaceBuckets {
    pub writable: Vec<RankingEntry>,
    pub curated: Vec<String>,
    pub world_backed: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct WorldBuckets {
    pub writable: Vec<RankingEntry>,
    pub curated: Vec<String>,
    pub missing: Vec<String>,
}

pub(crate) fn classify_places(
    entries: &[RankingEntry],
    rows: &[CurationRow],
    world_backed_ids: &[String],
) -> PlaceBuckets {
    let found: HashMap<&str, &CurationRow> =
        rows.iter().map(|row| (row.id.as_str(), row)).collect();
    let local: HashSet<&str> = world_backed_ids.iter().map(String::as_str).collect();
    let mut buckets = PlaceBuckets::default();
    for entry in entries {
        match found.get(entry.id.as_str()) {
            Some(row) if row.highlighted || row.exclude_from_ranking => {
                buckets.curated.push(entry.id.clone())
            }
            Some(row) if row.world => buckets.world_backed.push(entry.id.clone()),
            Some(_) => buckets.writable.push(entry.clone()),
            None if local.contains(entry.id.as_str()) => {
                buckets.world_backed.push(entry.id.clone())
            }
            None => buckets.missing.push(entry.id.clone()),
        }
    }
    buckets
}

pub(crate) fn classify_worlds(
    entries: &[RankingEntry],
    matches: &[WorldMatch],
) -> (WorldBuckets, Vec<String>) {
    let mut found: HashMap<&str, &WorldMatch> = HashMap::new();
    for m in matches {
        found.entry(m.submitted.as_str()).or_insert(m);
    }
    let mut buckets = WorldBuckets::default();
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut collisions: Vec<String> = Vec::new();
    for entry in entries {
        match found.get(entry.id.as_str()) {
            Some(m) if m.highlighted || m.exclude_from_ranking => {
                buckets.curated.push(entry.id.clone())
            }
            Some(m) => {
                if !claimed.insert(m.row_id.as_str()) {
                    collisions.push(entry.id.clone());
                    continue;
                }
                buckets.writable.push(RankingEntry {
                    id: m.row_id.clone(),
                    ranking: entry.ranking,
                })
            }
            None => buckets.missing.push(entry.id.clone()),
        }
    }
    (buckets, collisions)
}

/// A type whose every named destination is unknown is a run built against
/// the wrong catalogue: its write half would be empty and its clear would
/// empty that type's whole ranking. Some misses are ordinary; all of them is
/// refused. A type sent nothing for is the declared "nothing qualified".
pub(crate) fn nothing_resolved(entity_type: &str, sent: usize, missing: usize) -> Option<String> {
    (sent > 0 && missing == sent)
        .then(|| format!("None of the {sent} {entity_type} destinations in this request exist, so it would clear every {entity_type} ranking rather than replace it"))
}

fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// Locks the world-scene identities among `ids`, in identity order: their
/// curation and ranking writes then run one at a time.
pub(super) async fn lock_world_scenes(
    conn: &mut PgConnection,
    ids: &[String],
) -> Result<(), ApiError> {
    let identities: Vec<uuid::Uuid> = ids
        .iter()
        .filter_map(|id| uuid::Uuid::parse_str(id).ok())
        .collect();
    if !identities.is_empty() {
        sqlx::query("SELECT 1 FROM explore.world_scene_place WHERE identity = ANY($1) ORDER BY identity FOR UPDATE")
            .bind(&identities)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Records `overlays` (catalogue id, raw keys decided) and merges them into
/// both legs' `raw`. Returns the number of catalogue rows written.
async fn write_overlays(
    conn: &mut PgConnection,
    overlays: &[(String, Value)],
    actor: &str,
) -> Result<u64, ApiError> {
    if overlays.is_empty() {
        return Ok(0);
    }
    let ids: Vec<String> = overlays.iter().map(|(id, _)| id.clone()).collect();
    lock_world_scenes(conn, &ids).await?;
    let docs: Vec<Value> = overlays.iter().map(|(_, v)| v.clone()).collect();
    sqlx::query(
        "INSERT INTO explore.place_ranking (place_id, overlay, set_by, updated_at)
         SELECT u.id, u.overlay, $3, $4 FROM unnest($1::text[], $2::jsonb[]) AS u(id, overlay)
         ON CONFLICT (place_id) DO UPDATE SET
             overlay = explore.place_ranking.overlay || EXCLUDED.overlay,
             set_by = EXCLUDED.set_by,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(&ids)
    .bind(&docs)
    .bind(actor)
    .bind(now())
    .execute(&mut *conn)
    .await?;
    let mut written = 0;
    for table in ["explore.place", "explore.place_world_local"] {
        let sql = format!(
            "UPDATE {table} AS p SET raw = COALESCE(p.raw, '{{}}'::jsonb) || u.overlay
             FROM unnest($1::text[], $2::jsonb[]) AS u(id, overlay) WHERE p.id = u.id"
        );
        written += sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&ids)
            .bind(&docs)
            .execute(&mut *conn)
            .await?
            .rows_affected();
    }
    Ok(written)
}

fn ranking_value(ranking: Option<f64>) -> Value {
    ranking.map(Value::from).unwrap_or(Value::Null)
}

/// The admin token's ranking of one row: any value, `None` clears it. The
/// number of rows written (0: no such row).
pub async fn set_ranking(
    pool: &PgPool,
    id: &str,
    ranking: Option<f64>,
    actor: &str,
) -> Result<u64, ApiError> {
    let mut tx = pool.begin().await?;
    super::interactions::lock_public_id_writes(&mut tx).await?;
    let key = super::interactions::interaction_key(&mut tx, id).await?;
    let written = write_overlays(
        &mut tx,
        &[(key, json!({ "ranking": ranking_value(ranking) }))],
        actor,
    )
    .await?;
    tx.commit().await?;
    Ok(written)
}

/// A world-scene place `p` with its curation as its projection reads it:
/// its own or its world's highlight, or its ranking exclusion.
const SCENE_CURATION_FROM: &str = "FROM explore.world_scene_place p \
     LEFT JOIN explore.place_moderation m ON m.place_id = p.identity::text \
     LEFT JOIN explore.place_moderation wm ON wm.place_id = p.world_name \
     LEFT JOIN explore.place_ranking r ON r.place_id = p.identity::text";
const SCENE_CURATED: &str = "(COALESCE(m.highlighted, wm.highlighted, false) \
     OR COALESCE(r.overlay->'exclude_from_ranking' = 'true'::jsonb, false))";

/// A world-scene place's curation and canonical id, its identity locked.
fn scene_curation_sql() -> String {
    format!(
        "SELECT {SCENE_CURATED}, p.identity::text {SCENE_CURATION_FROM} WHERE p.identity = $1 FOR UPDATE OF p"
    )
}

fn curation_sql(table: &str) -> String {
    format!(
        "SELECT highlighted, {EXCLUDE_FROM_RANKING_SQL} AS excluded FROM {table} WHERE id = $1 FOR UPDATE"
    )
}

/// The data team's ranking: never over a highlighted or excluded row.
pub async fn set_ranking_from_score(
    pool: &PgPool,
    id: &str,
    ranking: Option<f64>,
    actor: &str,
) -> Result<ScoreRankingOutcome, ApiError> {
    let mut tx = pool.begin().await?;
    super::interactions::lock_public_id_writes(&mut tx).await?;
    let key = super::interactions::interaction_key(&mut tx, id).await?;
    let id = key.as_str();
    // Scene before catalogue row, the order every other curation write takes.
    lock_world_scenes(&mut tx, &[id.to_string()]).await?;
    // A scene wins over a catalogue row with its id, as on read, and is
    // written under its canonical id, the one its projection reads.
    let scene: Option<(bool, String)> = match uuid::Uuid::parse_str(id) {
        Ok(identity) => {
            sqlx::query_as(sqlx::AssertSqlSafe(scene_curation_sql()))
                .bind(identity)
                .fetch_optional(&mut *tx)
                .await?
        }
        Err(_) => None,
    };
    let key = scene
        .as_ref()
        .map_or(id, |(_, canonical)| canonical.as_str())
        .to_string();
    let mut curated = scene.map(|(curated, _)| curated);
    if curated.is_none() {
        for table in ["explore.place", "explore.place_world_local"] {
            if let Some(row) = sqlx::query(sqlx::AssertSqlSafe(curation_sql(table)))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
            {
                curated = Some(row.get::<bool, _>("highlighted") || row.get::<bool, _>("excluded"));
                break;
            }
        }
    }
    let outcome = match curated {
        None => ScoreRankingOutcome::NotWritable,
        Some(true) => ScoreRankingOutcome::RefusedCurated,
        Some(false) => {
            write_overlays(
                &mut tx,
                &[(key, json!({ "ranking": ranking_value(ranking) }))],
                actor,
            )
            .await?;
            ScoreRankingOutcome::Stored
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// Excludes a row from the automated ranking (pinning it at 0) or lets it
/// back in (its ranking stays as it is).
pub async fn set_exclusion(
    pool: &PgPool,
    id: &str,
    exclude: bool,
    actor: &str,
) -> Result<u64, ApiError> {
    let overlay = if exclude {
        json!({ "exclude_from_ranking": true, "ranking": 0 })
    } else {
        json!({ "exclude_from_ranking": false })
    };
    let mut tx = pool.begin().await?;
    super::interactions::lock_public_id_writes(&mut tx).await?;
    let key = super::interactions::interaction_key(&mut tx, id).await?;
    let written = write_overlays(&mut tx, &[(key, overlay)], actor).await?;
    tx.commit().await?;
    Ok(written)
}

fn place_curation_sql() -> String {
    let scene = super::scene_identity::is_scene_id_sql("place_indexed.id");
    let key = super::query::genesis_storage_key_sql("requested.id");
    let alias = super::rows::GENESIS_PLACE_JOIN;
    format!(
        "SELECT requested.id, place_indexed.id AS storage_id,
                COALESCE(genesis_highlighted, highlighted) AS highlighted, world,
                COALESCE(genesis_ranking_overlay->'exclude_from_ranking' = 'true'::jsonb,
                         {EXCLUDE_FROM_RANKING_SQL}) AS exclude_from_ranking
         FROM unnest($1::text[]) AS requested(id)
         JOIN explore.place AS place_indexed ON place_indexed.id = {key}
         {alias}
         WHERE NOT {scene}"
    )
}

const LOCALLY_SERVED_WORLD_IDS: &str =
    "SELECT id FROM explore.place_world_local WHERE world IS TRUE AND id = ANY($1)";

/// The ranked, uncurated world-scene places the places leg clears (a scene
/// wins over a catalogue row with its id). `$1` is spared; `$2`, when set,
/// narrows them to `$2`.
fn scene_clear_candidates_sql() -> String {
    format!(
        "SELECT p.identity::text {SCENE_CURATION_FROM} \
         WHERE jsonb_typeof(r.overlay->'ranking') = 'number' AND r.overlay->'ranking' <> '0'::jsonb \
         AND NOT {SCENE_CURATED} AND p.identity::text <> ALL($1::text[]) \
         AND ($2::text[] IS NULL OR p.identity::text = ANY($2::text[])) ORDER BY p.identity"
    )
}

/// World rows by id or name; a row a scene shadows is left to the scene path.
fn world_resolution_sql() -> String {
    let scene = super::scene_identity::is_scene_id_sql("p.id");
    format!(
        "SELECT s.submitted AS submitted, p.id AS id, p.highlighted AS highlighted, {EXCLUDE_FROM_RANKING_SQL} AS exclude_from_ranking
         FROM unnest($1::text[]) AS s(submitted)
         JOIN explore.place_indexed p ON p.world IS TRUE AND (p.id = s.submitted OR lower(p.world_name) = lower(s.submitted))
         WHERE NOT {scene}
         ORDER BY s.submitted, p.id"
    )
}

/// The automated rows of one leg the replace clears: ranked, uncurated, not
/// kept, and not shadowed by a scene (the scene clear owns those).
fn clear_candidates_sql(world: bool) -> String {
    let leg = if world { "TRUE" } else { "FALSE" };
    let scene = super::scene_identity::is_scene_id_sql("explore.place_indexed.id");
    let unshadowed = super::query::GENESIS_UNSHADOWED;
    format!(
        "SELECT id FROM explore.place_indexed WHERE world IS {leg} AND highlighted IS FALSE
         AND {EXCLUDE_FROM_RANKING_SQL} IS FALSE AND {RANKING_IS_SET_SQL} AND id <> ALL($1::text[])
         AND NOT {scene} AND {unshadowed}"
    )
}

/// Replaces the automated ranking set of each leg in `scope` in one
/// transaction: every ranking named is written, every automated ranking of
/// that leg not named is cleared to 0 (world-scene places with the places
/// leg; one the run names is reported world-backed and kept), curated rows
/// are never touched, and a leg out of scope is neither written nor cleared. A clear then a separate
/// write would leave a window where the population reads as unranked.
pub async fn replace_ranking(
    pool: &PgPool,
    places: &[RankingEntry],
    worlds: &[RankingEntry],
    scope: ReplaceScope,
    actor: &str,
) -> Result<ReplaceRankingResult, ApiError> {
    let mut tx = pool.begin().await?;
    super::interactions::lock_public_id_writes(&mut tx).await?;
    let place_ids: Vec<String> = places.iter().map(|e| e.id.clone()).collect();
    let mut curation = Vec::new();
    let mut world_backed_ids = Vec::new();
    let mut scene_storage_ids = Vec::new();
    let mut storage_by_id = HashMap::new();
    if !place_ids.is_empty() {
        curation = sqlx::query(sqlx::AssertSqlSafe(place_curation_sql()))
            .bind(&place_ids)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| {
                let id: String = r.get("id");
                storage_by_id.insert(id.clone(), r.get::<String, _>("storage_id"));
                CurationRow {
                    id,
                    highlighted: r.get("highlighted"),
                    exclude_from_ranking: r.get("exclude_from_ranking"),
                    world: r.get("world"),
                }
            })
            .collect();
        world_backed_ids = sqlx::query_scalar(LOCALLY_SERVED_WORLD_IDS)
            .bind(&place_ids)
            .fetch_all(&mut *tx)
            .await?;
        // A world-scene place is world-backed, as upstream's per-scene world place rows are.
        let parsed: Vec<uuid::Uuid> = place_ids
            .iter()
            .filter_map(|id| uuid::Uuid::parse_str(id).ok())
            .collect();
        let binding = super::scene_identity::BOUND_WORLD_SCENE_FROM;
        let scenes: HashMap<uuid::Uuid, uuid::Uuid> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT identity, identity FROM explore.world_scene_place WHERE identity = ANY($1)
             UNION SELECT canonical.public_id, p.identity {binding}
             WHERE p.identity = ANY(ARRAY(
                 SELECT identity FROM explore.world_scene_public_id WHERE public_id = ANY($1)
             )) AND canonical.public_id = ANY($1)"
        )))
        .bind(&parsed)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        for id in &place_ids {
            if let Ok(uuid) = uuid::Uuid::parse_str(id)
                && let Some(storage_id) = scenes.get(&uuid)
            {
                world_backed_ids.push(id.clone());
                scene_storage_ids.push(storage_id.to_string());
            }
        }
    }
    let place_buckets = classify_places(places, &curation, &world_backed_ids);
    let mut writable_keys = HashSet::new();
    for entry in &place_buckets.writable {
        let storage = storage_by_id.get(&entry.id).unwrap_or(&entry.id);
        if !writable_keys.insert(storage) {
            return Err(ApiError::bad_request(
                "The same place appears more than once",
            ));
        }
    }

    let world_ids: Vec<String> = worlds.iter().map(|e| e.id.clone()).collect();
    let mut matches = Vec::new();
    if !world_ids.is_empty() {
        matches = sqlx::query(sqlx::AssertSqlSafe(world_resolution_sql()))
            .bind(&world_ids)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| WorldMatch {
                submitted: r.get("submitted"),
                row_id: r.get("id"),
                highlighted: r.get("highlighted"),
                exclude_from_ranking: r.get("exclude_from_ranking"),
            })
            .collect();
    }
    let (world_buckets, collisions) = classify_worlds(worlds, &matches);
    if !collisions.is_empty() {
        tx.rollback().await?;
        let list: Vec<String> = collisions.iter().map(|id| format!("world:{id}")).collect();
        return Err(ApiError::bad_request(format!(
            "The same destination appears more than once: {}",
            list.join(", ")
        )));
    }
    if let Some(message) = nothing_resolved("place", places.len(), place_buckets.missing.len())
        .or_else(|| nothing_resolved("world", worlds.len(), world_buckets.missing.len()))
    {
        tx.rollback().await?;
        return Err(ApiError::bad_request(message));
    }

    let mut result = ReplaceRankingResult {
        places: ReplaceRankingPlaces {
            replaced: scope.places,
            ..Default::default()
        },
        worlds: ReplaceRankingWorlds {
            replaced: scope.worlds,
            ..Default::default()
        },
    };
    // Every clear is chosen before any write, so all scene locks come first and in one order.
    let mut legs = Vec::new();
    for (world, in_scope, writable) in [
        (false, scope.places, &place_buckets.writable),
        (true, scope.worlds, &world_buckets.writable),
    ] {
        if !in_scope {
            continue;
        }
        let applied: Vec<(String, Value)> = writable
            .iter()
            .map(|e| {
                let id = if world {
                    &e.id
                } else {
                    storage_by_id.get(&e.id).unwrap_or(&e.id)
                };
                (id.clone(), json!({ "ranking": e.ranking }))
            })
            .collect();
        let keep: Vec<String> = applied.iter().map(|(id, _)| id.clone()).collect();
        let cleared_ids: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(clear_candidates_sql(world)))
                .bind(&keep)
                .fetch_all(&mut *tx)
                .await?;
        // A scene named in this run is reported world-backed and keeps its ranking;
        // its id is compared in the identity's canonical spelling.
        let spared: Vec<String> = keep
            .iter()
            .cloned()
            .chain(scene_storage_ids.iter().cloned())
            .collect();
        let scene_ids: Vec<String> = if world {
            Vec::new()
        } else {
            sqlx::query_scalar(sqlx::AssertSqlSafe(scene_clear_candidates_sql()))
                .bind(&spared)
                .bind(None::<Vec<String>>)
                .fetch_all(&mut *tx)
                .await?
        };
        legs.push((world, spared, applied, cleared_ids, scene_ids));
    }
    let touched: Vec<String> = legs
        .iter()
        .flat_map(|(_, _, applied, cleared, scenes)| {
            applied
                .iter()
                .map(|(id, _)| id)
                .chain(cleared)
                .chain(scenes)
        })
        .cloned()
        .collect();
    lock_world_scenes(&mut tx, &touched).await?;
    for (world, spared, applied, cleared_ids, scene_ids) in legs {
        let applied_count = write_overlays(&mut tx, &applied, actor).await? as i64;
        let cleared: Vec<(String, Value)> = cleared_ids
            .into_iter()
            .map(|id| (id, json!({ "ranking": 0 })))
            .collect();
        let mut cleared_count = write_overlays(&mut tx, &cleared, actor).await? as i64;
        if !scene_ids.is_empty() {
            // Re-read under the lock: a scene curated meanwhile keeps its ranking.
            let scenes: Vec<(String, Value)> =
                sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(scene_clear_candidates_sql()))
                    .bind(&spared)
                    .bind(&scene_ids)
                    .fetch_all(&mut *tx)
                    .await?
                    .into_iter()
                    .map(|id| (id, json!({ "ranking": 0 })))
                    .collect();
            write_overlays(&mut tx, &scenes, actor).await?;
            // Disjoint from the catalogue clear, which leaves scene ids out.
            cleared_count += scenes.len() as i64;
        }
        if world {
            result.worlds.applied = applied_count;
            result.worlds.cleared = cleared_count;
        } else {
            result.places.applied = applied_count;
            result.places.cleared = cleared_count;
        }
    }
    tx.commit().await?;
    result.places.skipped_curated = place_buckets.curated;
    result.places.skipped_world_backed = place_buckets.world_backed;
    result.places.skipped_missing = place_buckets.missing;
    result.worlds.skipped_curated = world_buckets.curated;
    result.worlds.skipped_missing = world_buckets.missing;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, ranking: f64) -> RankingEntry {
        RankingEntry {
            id: id.to_string(),
            ranking,
        }
    }

    fn row(id: &str, highlighted: bool, exclude: bool, world: bool) -> CurationRow {
        CurationRow {
            id: id.into(),
            highlighted,
            exclude_from_ranking: exclude,
            world,
        }
    }

    #[test]
    fn places_are_bucketed_by_curation_leg_and_presence() {
        let entries = [
            entry("a", 1.0),
            entry("b", 2.0),
            entry("c", 3.0),
            entry("d", 4.0),
            entry("e", 5.0),
        ];
        let rows = [
            row("a", false, false, false),
            row("b", true, false, false),
            row("c", false, true, false),
            row("d", false, false, true),
        ];
        let buckets = classify_places(&entries, &rows, &["e".to_string()]);
        assert_eq!(buckets.writable, vec![entry("a", 1.0)]);
        assert_eq!(buckets.curated, vec!["b".to_string(), "c".to_string()]);
        assert_eq!(buckets.world_backed, vec!["d".to_string(), "e".to_string()]);
        assert!(buckets.missing.is_empty());
        assert_eq!(
            classify_places(&[entry("z", 1.0)], &rows, &[]).missing,
            vec!["z".to_string()]
        );
    }

    #[test]
    fn two_names_of_one_world_collide() {
        let m = |submitted: &str, row_id: &str| WorldMatch {
            submitted: submitted.into(),
            row_id: row_id.into(),
            highlighted: false,
            exclude_from_ranking: false,
        };
        let (buckets, collisions) = classify_worlds(
            &[
                entry("w.dcl.eth", 1.0),
                entry("W-ID", 2.0),
                entry("gone", 3.0),
            ],
            &[m("w.dcl.eth", "W-ID"), m("W-ID", "W-ID")],
        );
        assert_eq!(buckets.writable, vec![entry("W-ID", 1.0)]);
        assert_eq!(collisions, vec!["W-ID".to_string()]);
        assert_eq!(buckets.missing, vec!["gone".to_string()]);
    }

    #[test]
    fn a_type_with_nothing_resolved_is_refused() {
        assert_eq!(
            nothing_resolved("place", 2, 2).as_deref(),
            Some(
                "None of the 2 place destinations in this request exist, so it would clear every place ranking rather than replace it"
            )
        );
        assert_eq!(
            nothing_resolved("world", 3, 2),
            None,
            "some misses are ordinary"
        );
        assert_eq!(
            nothing_resolved("world", 0, 0),
            None,
            "a type sent nothing for is a declared empty set"
        );
    }
}
