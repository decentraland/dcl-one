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
    locally_served: &[String],
) -> PlaceBuckets {
    let found: HashMap<&str, &CurationRow> =
        rows.iter().map(|row| (row.id.as_str(), row)).collect();
    let local: HashSet<&str> = locally_served.iter().map(String::as_str).collect();
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
    let written = write_overlays(
        &mut tx,
        &[(id.to_string(), json!({ "ranking": ranking_value(ranking) }))],
        actor,
    )
    .await?;
    tx.commit().await?;
    Ok(written)
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
    let mut curated = None;
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
    let outcome = match curated {
        None => ScoreRankingOutcome::NotWritable,
        Some(true) => ScoreRankingOutcome::RefusedCurated,
        Some(false) => {
            write_overlays(
                &mut tx,
                &[(id.to_string(), json!({ "ranking": ranking_value(ranking) }))],
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
    let written = write_overlays(&mut tx, &[(id.to_string(), overlay)], actor).await?;
    tx.commit().await?;
    Ok(written)
}

fn place_curation_sql() -> String {
    format!(
        "SELECT id, highlighted, world, {EXCLUDE_FROM_RANKING_SQL} AS exclude_from_ranking FROM explore.place WHERE id = ANY($1)"
    )
}

const LOCALLY_SERVED_WORLD_IDS: &str =
    "SELECT id FROM explore.place_world_local WHERE world IS TRUE AND id = ANY($1)";

fn world_resolution_sql() -> String {
    format!(
        "SELECT s.submitted AS submitted, p.id AS id, p.highlighted AS highlighted, {EXCLUDE_FROM_RANKING_SQL} AS exclude_from_ranking
         FROM unnest($1::text[]) AS s(submitted)
         JOIN explore.place_indexed p ON p.world IS TRUE AND (p.id = s.submitted OR lower(p.world_name) = lower(s.submitted))
         ORDER BY s.submitted, p.id"
    )
}

/// The automated rows of one leg the replace clears: ranked, uncurated, not kept.
fn clear_candidates_sql(world: bool) -> String {
    let leg = if world { "TRUE" } else { "FALSE" };
    format!(
        "SELECT id FROM explore.place_indexed WHERE world IS {leg} AND highlighted IS FALSE
         AND {EXCLUDE_FROM_RANKING_SQL} IS FALSE AND {RANKING_IS_SET_SQL} AND id <> ALL($1::text[])"
    )
}

/// Replaces the automated ranking set of each leg in `scope` in one
/// transaction: every ranking named is written, every automated ranking of
/// that leg not named is cleared to 0, curated rows are never touched, and a
/// leg out of scope is neither written nor cleared. A clear then a separate
/// write would leave a window where the population reads as unranked.
pub async fn replace_ranking(
    pool: &PgPool,
    places: &[RankingEntry],
    worlds: &[RankingEntry],
    scope: ReplaceScope,
    actor: &str,
) -> Result<ReplaceRankingResult, ApiError> {
    let mut tx = pool.begin().await?;
    let place_ids: Vec<String> = places.iter().map(|e| e.id.clone()).collect();
    let mut curation = Vec::new();
    let mut locally_served = Vec::new();
    if !place_ids.is_empty() {
        curation = sqlx::query(sqlx::AssertSqlSafe(place_curation_sql()))
            .bind(&place_ids)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| CurationRow {
                id: r.get("id"),
                highlighted: r.get("highlighted"),
                exclude_from_ranking: r.get("exclude_from_ranking"),
                world: r.get("world"),
            })
            .collect();
        locally_served = sqlx::query_scalar(LOCALLY_SERVED_WORLD_IDS)
            .bind(&place_ids)
            .fetch_all(&mut *tx)
            .await?;
    }
    let place_buckets = classify_places(places, &curation, &locally_served);

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
    for (world, in_scope, writable) in [
        (false, scope.places, &place_buckets.writable),
        (true, scope.worlds, &world_buckets.writable),
    ] {
        if !in_scope {
            continue;
        }
        let keep: Vec<String> = writable.iter().map(|e| e.id.clone()).collect();
        let applied: Vec<(String, Value)> = writable
            .iter()
            .map(|e| (e.id.clone(), json!({ "ranking": e.ranking })))
            .collect();
        let applied_count = write_overlays(&mut tx, &applied, actor).await? as i64;
        let cleared_ids: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(clear_candidates_sql(world)))
                .bind(&keep)
                .fetch_all(&mut *tx)
                .await?;
        let cleared: Vec<(String, Value)> = cleared_ids
            .into_iter()
            .map(|id| (id, json!({ "ranking": 0 })))
            .collect();
        let cleared_count = write_overlays(&mut tx, &cleared, actor).await? as i64;
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
