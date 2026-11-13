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
    let minted = super::identity::new_place_id(Some(world), entity_id)
        .ok_or_else(|| sqlx::Error::Protocol("invalid place identity inputs".into()))?;
    let existing_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM explore.world_scene_place WHERE identity=$1 AND active)",
    )
    .bind(minted)
    .fetch_one(&mut *conn)
    .await?;
    if existing_active {
        return Err(sqlx::Error::Protocol(
            "place identity is already bound to another active scene".into(),
        ));
    }
    sqlx::query(
        "UPDATE explore.world_scene_place SET active = false, entity_id = NULL, updated_at = timezone('UTC', now()) \
         WHERE identity = ANY($1)",
    )
    .bind(&overlapped)
    .execute(&mut *conn)
    .await?;
    let applied: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO explore.world_scene_place (world_name, identity, parcels, entity_id) VALUES ($1, $2, $3::text[], $4) \
         ON CONFLICT (identity) DO UPDATE SET active = true, parcels = EXCLUDED.parcels, entity_id = EXCLUDED.entity_id, \
             updated_at = timezone('UTC', now()) \
         WHERE explore.world_scene_place.world_name = EXCLUDED.world_name AND NOT explore.world_scene_place.active \
         RETURNING identity",
    )
    .bind(world)
    .bind(minted)
    .bind(parcels)
    .bind(entity_id)
    .fetch_optional(&mut *conn)
    .await?;
    if applied != Some(minted) {
        return Err(sqlx::Error::Protocol(
            "place identity is already bound to another active scene".into(),
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deterministic_world_identity_preserves_legacy_and_reactivates() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let world = format!("identity-{}.dcl.eth", Uuid::new_v4().simple());
        let mut tx = pool.begin().await.unwrap();
        let legacy = Uuid::new_v4();
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES($1,$2,'legacy',ARRAY['0,0'])")
            .bind(&world).bind(legacy).execute(&mut *tx).await.unwrap();
        assert_eq!(
            assign(
                &mut tx,
                &world,
                "replacement",
                &["0,0".into(), "0,1".into()]
            )
            .await
            .unwrap(),
            Some(legacy)
        );
        let id = assign(&mut tx, &world, "bafybeigdyrzt", &["9,9".into()])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            id,
            super::super::identity::new_place_id(Some(&world), "bafybeigdyrzt").unwrap()
        );
        sqlx::query(
            "UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE identity=$1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            assign(&mut tx, &world, "bafybeigdyrzt", &["9,9".into()])
                .await
                .unwrap(),
            Some(id)
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn deterministic_world_identity_does_not_steal_an_active_binding() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let world = format!("conflict-{}.dcl.eth", Uuid::new_v4().simple());
        let id = super::super::identity::new_place_id(Some(&world), "original").unwrap();
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES($1,$2,'newer',ARRAY['0,0'])")
            .bind(&world).bind(id).execute(&mut *tx).await.unwrap();
        assert!(
            assign(&mut tx, &world, "original", &["9,9".into()])
                .await
                .is_err()
        );
        let entity: String =
            sqlx::query_scalar("SELECT entity_id FROM explore.world_scene_place WHERE identity=$1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(entity, "newer");
        let other = Uuid::new_v4();
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES($1,$2,'other',ARRAY['0,1'])")
            .bind(&world).bind(other).execute(&mut *tx).await.unwrap();
        assert!(
            assign(&mut tx, &world, "original", &["0,0".into(), "0,1".into()])
                .await
                .is_err()
        );
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.world_scene_place WHERE world_name=$1 AND active",
        )
        .bind(&world)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(active, 2, "a conflict must not retire either overlap");
        tx.rollback().await.unwrap();
    }
}
