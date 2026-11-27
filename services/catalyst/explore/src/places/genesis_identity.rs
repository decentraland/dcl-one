//! Exact source evidence for exposing a local Genesis scene by its Places UUID.

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

use super::interactions::{lock_public_id_merge, merge_public_id};

pub(crate) const TRUSTED_SOURCE: &str = "https://places.decentraland.org";

const HAS_PUBLIC_FACTS: &str =
    "(EXISTS (SELECT 1 FROM explore.place_interaction_counts i WHERE i.entity_id = c.public_id)
    OR EXISTS (SELECT 1 FROM explore.user_favorites i WHERE i.entity_id = c.public_id)
    OR EXISTS (SELECT 1 FROM explore.user_likes i WHERE i.entity_id = c.public_id)
    OR EXISTS (SELECT 1 FROM explore.place_moderation i WHERE i.place_id = c.public_id)
    OR EXISTS (SELECT 1 FROM explore.place_ranking i WHERE i.place_id = c.public_id))";

/// Store inputs before catalogue deduplication. Replaying a page replaces the
/// evidence for that offset, including rows that appeared more than once.
pub(crate) async fn record_page(
    pool: &PgPool,
    pass_started_at: NaiveDateTime,
    offset: i64,
    raw_places: &[Value],
    reported_total: Option<i64>,
) -> Result<(), sqlx::Error> {
    let sightings: Vec<_> = raw_places
        .iter()
        .filter_map(|place| {
            let id = place["id"].as_str()?;
            let canonical = uuid::Uuid::parse_str(id).ok()?.to_string();
            let eligible = canonical == id
                && place["world"].as_bool() == Some(false)
                && place["world_name"]
                    .as_str()
                    .is_none_or(|s| s.trim().is_empty())
                && place["disabled"].as_bool() != Some(true);
            let base = place["base_position"]
                .as_str()
                .filter(|s| eligible && crate::auth::pointer::parse_pointer(s).is_some());
            let deployment = place["deployment_id"]
                .as_str()
                .filter(|s| eligible && !s.is_empty());
            Some((canonical, base, deployment))
        })
        .collect();
    let mut tx = pool.begin().await?;
    let current = sqlx::query_scalar::<_, String>(
        "SELECT source_url FROM explore.mirror_state
         WHERE mirror = 'places' AND source_url = $1 AND pass_started_at = $2 FOR SHARE",
    )
    .bind(TRUSTED_SOURCE)
    .bind(pass_started_at)
    .fetch_optional(&mut *tx)
    .await?;
    if current.is_none() {
        return Ok(());
    }
    if offset == 0 {
        sqlx::query("DELETE FROM explore.genesis_place_page WHERE pass_started_at <> $1")
            .bind(pass_started_at)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO explore.genesis_place_page VALUES ($1, $2, $3, $4)
         ON CONFLICT (pass_started_at, page_offset) DO UPDATE SET
             page_rows = EXCLUDED.page_rows, reported_total = EXCLUDED.reported_total",
    )
    .bind(pass_started_at)
    .bind(offset)
    .bind(raw_places.len() as i64)
    .bind(reported_total)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM explore.genesis_place_sighting WHERE pass_started_at = $1 AND page_offset = $2")
        .bind(pass_started_at)
        .bind(offset)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO explore.genesis_place_sighting
           (pass_started_at, page_offset, ordinal, public_id, base_position, deployment_id)
         SELECT $1, $2, s.ordinal, s.public_id, s.base_position, s.deployment_id
         FROM unnest($3::text[], $4::text[], $5::text[]) WITH ORDINALITY
              AS s(public_id, base_position, deployment_id, ordinal)",
    )
    .bind(pass_started_at)
    .bind(offset)
    .bind(sightings.iter().map(|s| s.0.as_str()).collect::<Vec<_>>())
    .bind(sightings.iter().map(|s| s.1).collect::<Vec<_>>())
    .bind(sightings.iter().map(|s| s.2).collect::<Vec<_>>())
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

pub(crate) async fn clear_observations(
    pool: &PgPool,
    pass_started_at: NaiveDateTime,
    source_url: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    lock_public_id_merge(&mut tx).await?;
    let current = sqlx::query_scalar::<_, String>(
        "SELECT source_url FROM explore.mirror_state WHERE mirror = 'places'
           AND source_url = $1 AND pass_started_at = $2 FOR SHARE",
    )
    .bind(source_url)
    .bind(pass_started_at)
    .fetch_optional(&mut *tx)
    .await?;
    if current.is_some() && source_url != TRUSTED_SOURCE {
        sqlx::query("DELETE FROM explore.genesis_place_observed")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM explore.genesis_place_page")
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

/// Publish complete observations. Identical pagination repeats are one fact;
/// conflicting IDs or deployments are ineligible.
pub(crate) async fn finish_pass(
    pool: &PgPool,
    pass_started_at: NaiveDateTime,
    source_url: &str,
) -> Result<(), sqlx::Error> {
    if source_url != TRUSTED_SOURCE {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    let current = sqlx::query_scalar::<_, String>(
        "SELECT source_url FROM explore.mirror_state WHERE mirror = 'places'
           AND source_url = $1 AND pass_started_at = $2 FOR SHARE",
    )
    .bind(source_url)
    .bind(pass_started_at)
    .fetch_optional(&mut *tx)
    .await?;
    if current.is_none() {
        return Ok(());
    }
    let complete: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_and(page_offset = previous_rows AND reported_total IS NOT NULL)
                         AND min(reported_total) = max(reported_total)
                         AND sum(page_rows) = min(reported_total), false)
         FROM (
           SELECT *, COALESCE(sum(page_rows) OVER (
             ORDER BY page_offset ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING), 0) AS previous_rows
           FROM explore.genesis_place_page WHERE pass_started_at = $1
         ) pages",
    )
    .bind(pass_started_at)
    .fetch_one(&mut *tx)
    .await?;
    if !complete {
        tracing::warn!(%pass_started_at, "Places mirror did not verify a complete Genesis catalogue walk");
        return Ok(());
    }
    sqlx::query(
        "CREATE TEMP TABLE next_genesis_observations ON COMMIT DROP AS
         WITH sightings AS MATERIALIZED (
             SELECT DISTINCT public_id, base_position, deployment_id
             FROM explore.genesis_place_sighting WHERE pass_started_at = $1
         ), counts AS (
             SELECT *, count(*) OVER (PARTITION BY public_id) AS same_id,
                       count(*) OVER (PARTITION BY base_position, deployment_id) AS same_scene
             FROM sightings
         )
         SELECT public_id, base_position, deployment_id FROM counts
         WHERE same_id = 1 AND same_scene = 1 AND base_position IS NOT NULL AND deployment_id IS NOT NULL
         UNION ALL
         SELECT o.public_id, o.base_position, o.deployment_id
         FROM explore.genesis_place_observed o JOIN explore.place p ON p.id = o.public_id
         WHERE p.origin = 'upstream' AND NOT p.world AND p.world_name IS NULL
           AND p.raw->>'world' = 'false' AND COALESCE(p.raw->>'disabled', 'false') = 'false'
           AND p.fetched_at >= $1
           AND p.base_position = o.base_position AND p.raw->>'deployment_id' = o.deployment_id
           AND NOT EXISTS (SELECT 1 FROM sightings s WHERE s.public_id = o.public_id
                           OR (s.base_position, s.deployment_id) = (o.base_position, o.deployment_id))",
    )
    .bind(pass_started_at)
    .execute(&mut *tx)
    .await?;
    let observations_changed: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM explore.genesis_place_observed old
            FULL JOIN next_genesis_observations next USING (public_id, base_position, deployment_id)
            WHERE old.public_id IS NULL OR next.public_id IS NULL
        )",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !observations_changed {
        tx.commit().await?;
        return reconcile_committed(pool, None).await;
    }
    lock_public_id_merge(&mut tx).await?;
    sqlx::query(
        "DELETE FROM explore.genesis_place_observed current
         WHERE NOT EXISTS (
             SELECT 1 FROM next_genesis_observations next
             WHERE (next.public_id, next.base_position, next.deployment_id) =
                   (current.public_id, current.base_position, current.deployment_id)
         )",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO explore.genesis_place_observed (public_id, base_position, deployment_id)
         SELECT public_id, base_position, deployment_id FROM next_genesis_observations
         ON CONFLICT (public_id) DO NOTHING",
    )
    .execute(&mut *tx)
    .await?;
    super::moderation::lock_curation(&mut tx).await?;
    // Reappearing reservations become live in this transaction. Fold any
    // facts written while those aliases were unbound before publishing them.
    reconcile_existing(&mut tx, None).await?;
    tx.commit().await?;
    reconcile_committed(pool, None).await
}

pub(super) async fn reconcile_existing(
    conn: &mut PgConnection,
    local_ids: Option<&[&str]>,
) -> Result<(), sqlx::Error> {
    reconcile(conn, local_ids, false).await
}

/// Both writers try after committing their input. Whichever commits second
/// can see the other, and pages without candidates never queue an exclusive
/// interaction lock. Each batch rechecks the live evidence after locking.
pub(super) async fn reconcile_committed(
    pool: &PgPool,
    local_ids: Option<&[&str]>,
) -> Result<(), sqlx::Error> {
    let ids: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT c.local_id FROM explore.genesis_place_candidate c
         LEFT JOIN explore.genesis_place_claim a ON a.public_id = c.public_id
         WHERE ($1::text[] IS NULL OR c.local_id = ANY($1))
           AND (a.public_id IS NULL OR (a.local_id = c.local_id AND {HAS_PUBLIC_FACTS}))
         ORDER BY c.local_id",
    )))
    .bind(local_ids)
    .fetch_all(pool)
    .await?;
    for batch in ids.chunks(16) {
        let batch: Vec<_> = batch.iter().map(String::as_str).collect();
        let mut tx = pool.begin().await?;
        lock_public_id_merge(&mut tx).await?;
        super::moderation::lock_curation(&mut tx).await?;
        reconcile(&mut tx, Some(&batch), true).await?;
        tx.commit().await?;
    }
    Ok(())
}

/// Caller holds the exclusive interaction claim lock before taking row locks.
/// Reservations keep their original owner even after this live proof expires.
async fn reconcile(
    conn: &mut PgConnection,
    local_ids: Option<&[&str]>,
    allow_new_claims: bool,
) -> Result<(), sqlx::Error> {
    let pending: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.public_id, c.local_id FROM explore.genesis_place_candidate c
         WHERE $2 AND ($1::text[] IS NULL OR c.local_id = ANY($1))
           AND CASE WHEN pg_input_is_valid(c.local_id, 'uuid') THEN c.local_id::uuid::text END = c.local_id
           AND NOT EXISTS (SELECT 1 FROM explore.genesis_place_claim a WHERE a.public_id = c.public_id)",
    )
    .bind(local_ids)
    .bind(allow_new_claims)
    .fetch_all(&mut *conn)
    .await?;
    if !pending.is_empty() {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('explore.world_scene_uuid/' || id, 0))
             FROM (
                SELECT unnest($1::text[]) AS id UNION SELECT unnest($2::text[]) AS id
             ) keys ORDER BY id",
        )
        .bind(pending.iter().map(|p| p.0.as_str()).collect::<Vec<_>>())
        .bind(pending.iter().map(|p| p.1.as_str()).collect::<Vec<_>>())
        .execute(&mut *conn)
        .await?;
    }
    // Recheck under the shared namespace locks. Only pairs whose two keys
    // were locked above may create a reservation.
    let claims = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid)>(sqlx::AssertSqlSafe(format!(
        "WITH eligible AS (
           SELECT c.public_id, c.local_id,
                  CASE WHEN pg_input_is_valid(c.local_id, 'uuid') THEN c.local_id::uuid END AS local_uuid
           FROM explore.genesis_place_candidate c WHERE $1::text[] IS NULL OR c.local_id = ANY($1)
         ), claimed AS (
           INSERT INTO explore.genesis_place_claim (public_id, local_id)
           SELECT c.public_id, c.local_id FROM eligible c
           WHERE c.local_uuid IS NOT NULL
             AND EXISTS (SELECT 1 FROM unnest($2::text[], $3::text[]) AS pending(public_id, local_id)
                         WHERE pending.public_id = c.public_id AND pending.local_id = c.local_id)
             AND NOT EXISTS (SELECT 1 FROM explore.world_scene_place p
                             WHERE p.identity IN (c.public_id::uuid, c.local_uuid))
             AND NOT EXISTS (SELECT 1 FROM explore.world_scene_public_id p
                             WHERE p.public_id IN (c.public_id::uuid, c.local_uuid))
             AND NOT EXISTS (SELECT 1 FROM explore.genesis_place_claim a
                             WHERE a.local_id = c.public_id OR a.public_id = c.local_id)
           ORDER BY c.public_id, c.local_id
           ON CONFLICT (public_id) DO NOTHING
           RETURNING public_id::uuid, local_id::uuid
         )
         SELECT * FROM claimed
         UNION SELECT c.public_id::uuid, c.local_uuid FROM eligible c
         JOIN explore.genesis_place_claim a ON a.public_id = c.public_id AND a.local_id = c.local_id
         WHERE {HAS_PUBLIC_FACTS}",
    )))
    .bind(local_ids)
    .bind(pending.iter().map(|p| p.0.as_str()).collect::<Vec<_>>())
    .bind(pending.iter().map(|p| p.1.as_str()).collect::<Vec<_>>())
    .fetch_all(&mut *conn)
    .await?;
    for (public, local) in claims {
        merge_public_id(conn, public, local).await?;
    }
    Ok(())
}
