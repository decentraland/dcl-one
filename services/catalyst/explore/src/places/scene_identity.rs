//! The stable identity of a world scene (`explore.world_scene_place`), its
//! shared place UUID used by discovery and storage, kept by
//! Places' overlap rule (`decentraland/places` at
//! `ba6674f6f13c2715be3e480d4b01b6979cde79fc`,
//! `src/entities/CheckScenes/task/resolveWorldDeployment.ts`, the source of the
//! place id the old world-storage keyed rows on): a scene now standing on a
//! footprint that overlaps exactly one active identity of its world keeps it,
//! and the identity takes the new footprint (a moved base included); one
//! overlapping none or several gets a new identity, and every overlapped one
//! is deactivated. Every `worlds.world_scenes` write calls [`assign`] for the
//! scenes it wrote and [`retire_orphans`] after its deletes, in its own
//! transaction under the world row; storage rows are never deleted here.

use sqlx::PgConnection;
use uuid::Uuid;

/// Shared read binding: an active place refers to precisely one live
/// deployment in the same world. Catalogue freshness and discovery use this
/// same relation rather than guessing locality from the world alone.
pub const BOUND_WORLD_SCENE_FROM: &str = "FROM explore.world_scene_place p \
    JOIN worlds.world_scenes s ON p.active AND s.world_name = p.world_name AND s.entity_id = p.entity_id \
    JOIN worlds.worlds w ON w.name = p.world_name";

pub const LOCAL_WORLD_SCENE: &str = "(s.mirror_source IS NULL AND w.mirror_source IS NULL)";

/// Applies the overlap rule to a scene of `world` now standing on `parcels`
/// and returns its identity; `None` for an empty footprint (no scope).
pub async fn assign(
    conn: &mut PgConnection,
    world: &str,
    entity_id: &str,
    parcels: &[String],
) -> Result<Option<Uuid>, sqlx::Error> {
    if parcels.is_empty() {
        return Ok(None);
    }
    let overlapped: Vec<Uuid> = sqlx::query_scalar(
        "SELECT identity FROM explore.world_scene_place \
         WHERE world_name = $1 AND active AND parcels && $2::text[] FOR UPDATE",
    )
    .bind(world)
    .bind(parcels)
    .fetch_all(&mut *conn)
    .await?;
    if let [kept] = overlapped[..] {
        sqlx::query(
            "UPDATE explore.world_scene_place SET parcels = $2::text[], entity_id = $3, updated_at = timezone('UTC', now()) \
             WHERE identity = $1 AND (parcels IS DISTINCT FROM $2::text[] OR entity_id IS DISTINCT FROM $3)",
        )
        .bind(kept)
        .bind(parcels)
        .bind(entity_id)
        .execute(&mut *conn)
        .await?;
        return Ok(Some(kept));
    }
    sqlx::query(
        "UPDATE explore.world_scene_place SET active = false, entity_id = NULL, updated_at = timezone('UTC', now()) \
         WHERE identity = ANY($1)",
    )
    .bind(&overlapped)
    .execute(&mut *conn)
    .await?;
    let minted = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO explore.world_scene_place (world_name, identity, parcels, entity_id) VALUES ($1, $2, $3::text[], $4)",
    )
    .bind(world)
    .bind(minted)
    .bind(parcels)
    .bind(entity_id)
    .execute(&mut *conn)
    .await?;
    Ok(Some(minted))
}

/// Deactivates the active identities of `worlds` no scene stands on any more
/// (an undeploy, an unlist, a retired mirrored scene).
pub async fn retire_orphans(
    conn: &mut PgConnection,
    worlds: &[String],
) -> Result<u64, sqlx::Error> {
    if worlds.is_empty() {
        return Ok(0);
    }
    Ok(sqlx::query(
        "UPDATE explore.world_scene_place i SET active = false, entity_id = NULL, updated_at = timezone('UTC', now()) \
         WHERE i.world_name = ANY($1) AND i.active AND NOT EXISTS ( \
           SELECT 1 FROM worlds.world_scenes s WHERE s.world_name = i.world_name AND s.entity_id = i.entity_id)",
    )
    .bind(worlds)
    .execute(conn)
    .await?
    .rows_affected())
}
