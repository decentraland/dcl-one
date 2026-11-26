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

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// Shared read binding: an active place refers to precisely one live
/// deployment in the same world. Catalogue freshness and discovery use this
/// same relation rather than guessing locality from the world alone.
pub const BOUND_WORLD_SCENE_FROM: &str = "FROM explore.world_scene_place p \
    JOIN worlds.world_scenes s ON p.active AND s.world_name = p.world_name AND s.entity_id = p.entity_id \
    JOIN worlds.worlds w ON w.name = p.world_name \
    LEFT JOIN explore.world_scene_public_id canonical ON canonical.identity = p.identity \
      AND canonical.entity_id IS NOT NULL AND canonical.world_name = p.world_name \
      AND canonical.entity_id = s.entity_id \
      AND canonical.source = s.mirror_source AND canonical.source = w.mirror_source";

/// Use only with the guarded live deployment join above. Durable local keys
/// (including storage, moderation, and interactions) continue to use p.identity.
pub const PUBLIC_WORLD_SCENE_ID: &str = "COALESCE(canonical.public_id, p.identity)";

fn uuid_sql(column: &str) -> String {
    let value = format!("regexp_replace({column}, '^urn:uuid:', '', 'i')");
    format!("CASE WHEN pg_input_is_valid({value}, 'uuid') THEN {value}::uuid END")
}

/// Whether catalogue id `column` names a world-scene identity, in any UUID
/// spelling (`urn:uuid:`, any case); such a scene shadows that row.
pub fn is_scene_id_sql(column: &str) -> String {
    let id = uuid_sql(column);
    format!(
        "(EXISTS (SELECT 1 FROM explore.world_scene_place scene WHERE scene.identity = {id}) \
          OR EXISTS (SELECT 1 FROM explore.world_scene_public_id claimed WHERE claimed.public_id = {id}))"
    )
}

pub const LOCAL_WORLD_SCENE: &str = "(s.mirror_source IS NULL AND w.mirror_source IS NULL)";

/// Candidates missing current proof. Scope the probe to the caller's
/// observed CIDs so an incomplete or late mirror response cannot target a
/// different deployment in the same world.
pub async fn pending_public_ids(
    pool: &PgPool,
    source: &str,
    world: &str,
    active_cids: &[&str],
) -> Result<Vec<String>, sqlx::Error> {
    let sql = format!(
        "SELECT s.entity_id {BOUND_WORLD_SCENE_FROM} \
         WHERE p.world_name = $1 AND s.entity_id = ANY($2::text[]) \
           AND s.mirror_source = $3 AND w.mirror_source = $3 \
           AND canonical.public_id IS NULL ORDER BY s.entity_id"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(world)
        .bind(active_cids)
        .bind(source)
        .fetch_all(pool)
        .await
}

#[derive(Debug, PartialEq, Eq)]
pub enum PublicIdBinding {
    Bound,
    Stale,
    Conflict,
}

/// Claim a Places UUID after the caller proves its exact source/world/CID.
/// The caller supplies a transaction. Take the same world lock as publication
/// before checking the active deployment. A UUID once claimed can only
/// refresh its proof for the original internal identity, source and world.
pub async fn bind_public_id(
    conn: &mut PgConnection,
    source: &str,
    world: &str,
    entity_id: &str,
    public_id: Uuid,
) -> Result<PublicIdBinding, sqlx::Error> {
    super::interactions::lock_public_id_merge(conn).await?;
    let world_locked = sqlx::query_scalar::<_, String>(
        "SELECT name FROM worlds.worlds WHERE name = $1 AND mirror_source = $2 FOR UPDATE",
    )
    .bind(world)
    .bind(source)
    .fetch_optional(&mut *conn)
    .await?;
    if world_locked.is_none() {
        return Ok(PublicIdBinding::Stale);
    }
    let identity = sqlx::query_scalar::<_, Uuid>(
        "SELECT p.identity FROM explore.world_scene_place p \
         JOIN worlds.world_scenes s ON s.world_name = p.world_name AND s.entity_id = p.entity_id \
         WHERE p.world_name = $1 AND p.active AND p.entity_id = $2 AND s.mirror_source = $3 \
         FOR UPDATE OF p",
    )
    .bind(world)
    .bind(entity_id)
    .bind(source)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(identity) = identity else {
        return Ok(PublicIdBinding::Stale);
    };

    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('explore.world_scene_uuid/' || $1::uuid::text, 0))")
        .bind(public_id)
        .execute(&mut *conn)
        .await?;

    let catalogue_id = uuid_sql("catalogue.id");
    let conflict_sql = format!(
        "SELECT EXISTS (SELECT 1 FROM explore.world_scene_place WHERE identity = $1 AND identity <> $2) \
          OR EXISTS (SELECT 1 FROM explore.world_scene_public_id \
                       WHERE identity = $2 AND entity_id IS NOT NULL AND public_id <> $1) \
          OR EXISTS (SELECT 1 FROM explore.place_indexed catalogue \
                       WHERE lower(translate(catalogue.id, '-{{}}', '')) = ANY(ARRAY[ \
                           replace($1::uuid::text, '-', ''), 'urn:uuid:' || replace($1::uuid::text, '-', '')]) \
                       AND {catalogue_id} = $1 \
                       AND (catalogue.world AND lower(catalogue.world_name) = $3 \
                            AND catalogue.raw->>'deployment_id' = $4) IS DISTINCT FROM true)"
    );
    let collides: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(conflict_sql))
        .bind(public_id)
        .bind(identity)
        .bind(world)
        .bind(entity_id)
        .fetch_one(&mut *conn)
        .await?;
    if collides {
        return Ok(PublicIdBinding::Conflict);
    }

    let bound = sqlx::query(
        "INSERT INTO explore.world_scene_public_id (public_id, identity, source, world_name, entity_id) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (public_id) DO UPDATE SET entity_id = EXCLUDED.entity_id \
         WHERE world_scene_public_id.identity = EXCLUDED.identity \
           AND world_scene_public_id.source = EXCLUDED.source \
           AND world_scene_public_id.world_name = EXCLUDED.world_name",
    )
    .bind(public_id)
    .bind(identity)
    .bind(source)
    .bind(world)
    .bind(entity_id)
    .execute(&mut *conn)
    .await?;
    if bound.rows_affected() == 0 {
        Ok(PublicIdBinding::Conflict)
    } else {
        super::interactions::merge_public_id(conn, public_id, identity).await?;
        Ok(PublicIdBinding::Bound)
    }
}

/// Locks the active identities of `worlds` in identity order, the order the
/// ranking writes take, so a reconcile and a ranking replace cannot deadlock.
async fn lock_worlds(conn: &mut PgConnection, worlds: &[String]) -> Result<(), sqlx::Error> {
    sqlx::query(
        "SELECT 1 FROM explore.world_scene_place WHERE world_name = ANY($1) AND active ORDER BY identity FOR UPDATE",
    )
    .bind(worlds)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

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
    lock_worlds(conn, std::slice::from_ref(&world.to_string())).await?;
    let overlapped: Vec<Uuid> = sqlx::query_scalar(
        "SELECT identity FROM explore.world_scene_place \
         WHERE world_name = $1 AND active AND parcels && $2::text[] ORDER BY identity FOR UPDATE",
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
    lock_worlds(conn, worlds).await?;
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
    use crate::db::scratch::{OwnedDatabase, Schema};
    use sqlx::PgPool;

    const SOURCE: &str = "https://worlds.example";
    const WORLD: &str = "scene.dcl.eth";

    async fn create_world(pool: &PgPool, world: &str) {
        sqlx::query("INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1,$2)")
            .bind(world)
            .bind(SOURCE)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn deploy(pool: &PgPool, world: &str, cid: &str, parcel: &str) -> Uuid {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT name FROM worlds.worlds WHERE name=$1 FOR UPDATE")
            .bind(world)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name=$1")
            .bind(world)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes (world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size,mirror_source) VALUES ($1,$2,'[]','{}','owner',ARRAY[$3::text],1,$4)")
            .bind(world)
            .bind(cid)
            .bind(parcel)
            .bind(SOURCE)
            .execute(&mut *tx)
            .await
            .unwrap();
        let id = assign(&mut tx, world, cid, &[parcel.to_string()])
            .await
            .unwrap()
            .unwrap();
        retire_orphans(&mut tx, &[world.to_string()]).await.unwrap();
        tx.commit().await.unwrap();
        id
    }

    async fn bind(
        pool: &PgPool,
        source: &str,
        world: &str,
        cid: &str,
        public_id: Uuid,
    ) -> PublicIdBinding {
        let mut tx = pool.begin().await.unwrap();
        let result = bind_public_id(&mut tx, source, world, cid, public_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        result
    }

    async fn visible_id(pool: &PgPool, world: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT {PUBLIC_WORLD_SCENE_ID} {BOUND_WORLD_SCENE_FROM} WHERE p.world_name=$1"
        )))
        .bind(world)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn public_scene_binding_requires_current_proof_and_survives_redeploy() {
        let Some(db) = OwnedDatabase::new("scene_public_lifecycle", Schema::Full, 4).await else {
            return;
        };
        let pool = &db.pool;
        create_world(pool, WORLD).await;
        let identity = deploy(pool, WORLD, "first", "0,0").await;
        let public_id = Uuid::new_v4();
        assert_eq!(
            pending_public_ids(pool, SOURCE, WORLD, &["first", "old"])
                .await
                .unwrap(),
            ["first"]
        );
        assert_eq!(
            bind(pool, "https://other.example", WORLD, "first", public_id).await,
            PublicIdBinding::Stale
        );
        let catalogue_id = format!("URN:UUID:{}", public_id.to_string().to_uppercase());
        sqlx::query("INSERT INTO explore.place (id,base_position,raw) VALUES ($1,'0,0','{}')")
            .bind(&catalogue_id)
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(
            bind(pool, SOURCE, WORLD, "first", public_id).await,
            PublicIdBinding::Conflict
        );
        sqlx::query("UPDATE explore.place SET raw = jsonb_build_object('world',true,'world_name',$2::text,'deployment_id','first') WHERE id=$1")
            .bind(&catalogue_id)
            .bind(WORLD.to_uppercase())
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(
            bind(pool, SOURCE, WORLD, "first", public_id).await,
            PublicIdBinding::Bound
        );
        assert_eq!(visible_id(pool, WORLD).await, public_id);
        assert!(
            pending_public_ids(pool, SOURCE, WORLD, &["first"])
                .await
                .unwrap()
                .is_empty()
        );
        sqlx::query("DELETE FROM explore.place WHERE id=$1")
            .bind(&catalogue_id)
            .execute(pool)
            .await
            .unwrap();

        assert_eq!(deploy(pool, WORLD, "second", "0,0").await, identity);
        assert_eq!(
            bind(pool, SOURCE, WORLD, "first", public_id).await,
            PublicIdBinding::Stale
        );
        assert_eq!(visible_id(pool, WORLD).await, identity);
        let second_public_id = Uuid::new_v4();
        assert_eq!(
            bind(pool, SOURCE, WORLD, "second", second_public_id).await,
            PublicIdBinding::Bound
        );
        assert_eq!(visible_id(pool, WORLD).await, second_public_id);
        assert_eq!(
            bind(pool, SOURCE, WORLD, "second", public_id).await,
            PublicIdBinding::Conflict
        );
        assert_eq!(deploy(pool, WORLD, "first", "0,0").await, identity);
        assert_eq!(visible_id(pool, WORLD).await, identity);
        assert_eq!(
            bind(pool, SOURCE, WORLD, "first", public_id).await,
            PublicIdBinding::Bound
        );

        let replacement = deploy(pool, WORLD, "other", "2,0").await;
        assert_ne!(replacement, identity);
        assert_eq!(
            bind(pool, SOURCE, WORLD, "other", identity).await,
            PublicIdBinding::Conflict
        );
        assert_eq!(
            bind(pool, SOURCE, WORLD, "other", public_id).await,
            PublicIdBinding::Conflict
        );
        assert_eq!(
            bind(pool, SOURCE, WORLD, "other", second_public_id).await,
            PublicIdBinding::Conflict
        );
        assert_eq!(visible_id(pool, WORLD).await, replacement);
        db.drop().await;
    }

    #[tokio::test]
    async fn concurrent_public_scene_claims_have_one_owner() {
        let Some(db) = OwnedDatabase::new("scene_public_collision", Schema::Full, 4).await else {
            return;
        };
        let pool = &db.pool;
        let other = "other.dcl.eth";
        create_world(pool, WORLD).await;
        create_world(pool, other).await;
        let local = deploy(pool, WORLD, "scene", "0,0").await;
        let other_local = deploy(pool, other, "scene", "0,0").await;
        let public_id = Uuid::new_v4();
        let (a, b) = tokio::join!(
            bind(pool, SOURCE, WORLD, "scene", public_id),
            bind(pool, SOURCE, other, "scene", public_id),
        );
        let unclaimed = match (a, b) {
            (PublicIdBinding::Bound, PublicIdBinding::Conflict) => {
                assert_eq!(visible_id(pool, WORLD).await, public_id);
                assert_eq!(visible_id(pool, other).await, other_local);
                other
            }
            (PublicIdBinding::Conflict, PublicIdBinding::Bound) => {
                assert_eq!(visible_id(pool, WORLD).await, local);
                assert_eq!(visible_id(pool, other).await, public_id);
                WORLD
            }
            outcomes => panic!("expected only one bound claim, got {outcomes:?}"),
        };
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let (a, b) = tokio::join!(
            bind(pool, SOURCE, unclaimed, "scene", first),
            bind(pool, SOURCE, unclaimed, "scene", second),
        );
        let winner = match (a, b) {
            (PublicIdBinding::Bound, PublicIdBinding::Conflict) => first,
            (PublicIdBinding::Conflict, PublicIdBinding::Bound) => second,
            outcomes => panic!("expected only one proof for the same CID, got {outcomes:?}"),
        };
        assert_eq!(visible_id(pool, unclaimed).await, winner);
        db.drop().await;
    }

    #[tokio::test]
    async fn places_import_replays_around_concurrent_and_retired_public_claims() {
        let Some(db) = OwnedDatabase::new("scene_public_import", Schema::Full, 4).await else {
            return;
        };
        let pool = &db.pool;
        create_world(pool, WORLD).await;
        let identity = deploy(pool, WORLD, "first", "0,0").await;
        let public_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let mut claim = pool.begin().await.unwrap();
        assert_eq!(
            bind_public_id(&mut claim, SOURCE, WORLD, "first", public_id)
                .await
                .unwrap(),
            PublicIdBinding::Bound
        );
        let mut import = pool.begin().await.unwrap();
        sqlx::query(
            "CREATE TEMP TABLE stage (world_name text, identity uuid, entity_id text, parcels text[],
              active boolean, created_at timestamptz, updated_at timestamptz) ON COMMIT DROP",
        )
        .execute(&mut *import)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO stage VALUES ($1, $2, 'first', ARRAY['0,0'], true, now(), now()),
             ($1, $3, NULL, ARRAY['2,0'], false, now(), now())",
        )
        .bind(WORLD)
        .bind(public_id)
        .bind(other_id)
        .execute(&mut *import)
        .await
        .unwrap();
        let load_sql = include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/tooling/migration/sources/places/world_scene_place.load.sql"
        ));
        {
            let load = async {
                sqlx::raw_sql(load_sql).execute(&mut *import).await.unwrap();
            };
            tokio::pin!(load);
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(25), &mut load)
                    .await
                    .is_err(),
                "the import must wait for the reserved public UUID to commit"
            );
            claim.commit().await.unwrap();
            load.await;
        }
        let advisory_locks: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks WHERE pid = pg_backend_pid() AND locktype = 'advisory'",
        )
        .fetch_one(&mut *import)
        .await
        .unwrap();
        assert_eq!(advisory_locks, 1, "the batch uses one shared advisory lock");
        sqlx::query(
            "UPDATE explore.world_scene_place SET active = false, entity_id = NULL WHERE identity = $1",
        )
        .bind(identity)
        .execute(&mut *import)
        .await
        .unwrap();
        sqlx::raw_sql(load_sql).execute(&mut *import).await.unwrap();
        import.commit().await.unwrap();
        let internal_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT identity FROM explore.world_scene_place WHERE world_name = $1",
        )
        .bind(WORLD)
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(internal_ids.len(), 2);
        assert!(internal_ids.contains(&identity));
        assert!(internal_ids.contains(&other_id));
        assert!(!internal_ids.contains(&public_id));
        db.drop().await;
    }
}
