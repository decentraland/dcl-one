//! The ONE room resolver: from a realm/world/scene request to the active
//! deployment (`worlds.world_scenes` for a world, `content.active_pointers`
//! for Genesis City), its stable storage namespace and its canonical room.
//! The client's adapter request
//! (comms), the authoritative host (which used to read `scenesUrn[0]` of
//! `/world/{name}/about`) and the storage routes resolve through this one
//! reading of the same rows, so they select the same room and the same
//! namespace (DATABASE.md: "Resolve room identity from the same active
//! deployment for both clients and authoritative hosts").
//!
//! The room names and the resolver's request, answer and error types are
//! comms' (`comms::rooms`, re-exported here for the worlds callers); comms
//! asks this resolver through [`SceneResolver`], its `SceneIdentity`.

use sqlx::PgPool;
use sqlx::Row;

pub use crate::comms::rooms::{
    ActiveDeployment, ResolveError, ResolvedRoom, RoomRequest, parse_room,
};
use crate::comms::rooms::{
    gatekeeper_scene_room, worlds_service_scene_room, worlds_service_world_room,
};
use crate::comms::world_identity::{
    Parcel, ParcelSet, RealmId, SceneId, SceneNamespace, WorldName,
};

fn footprint(
    stored: &[String],
    declared_base: Option<&str>,
    scene: &SceneId,
) -> Result<(ParcelSet, Parcel), ResolveError> {
    let parcels: ParcelSet = stored
        .iter()
        .filter_map(|p| Parcel::parse(p).ok())
        .collect();
    let base = parcels
        .effective_base(declared_base)
        .ok_or_else(|| ResolveError::NoParcels {
            scene: scene.to_string(),
        })?;
    Ok((parcels, base))
}

fn deployment(
    realm: &RealmId,
    scene: SceneId,
    stored: &[String],
    declared_base: Option<&str>,
    title: Option<String>,
    local: bool,
    identity: Option<uuid::Uuid>,
) -> Result<ActiveDeployment, ResolveError> {
    let (parcels, base) = footprint(stored, declared_base, &scene)?;
    Ok(ActiveDeployment {
        scene_id: scene,
        title,
        parcels,
        base,
        namespace: SceneNamespace::new(realm, base, identity),
        local,
    })
}

/// Refuse serving an upgraded catalogue while mapped entity-id bans still need
/// the explicit release conversion. Use the resolver's parser so malformed or
/// unmapped legacy/Genesis rows do not block startup, and Unicode bases have the
/// same meaning as admission. Stream the startup audit instead of loading the
/// catalogue into memory; this does not modify any stored ban.
pub async fn ensure_world_scene_ban_release(pool: &PgPool) -> anyhow::Result<()> {
    use futures::TryStreamExt;

    let mut rows = sqlx::query_as::<_, (String, String, Vec<String>, Option<String>)>(
        "SELECT ws.world_name, ws.entity_id, ws.parcels, ws.entity #>> '{metadata,scene,base}' \
         FROM (SELECT world_name, entity_id, parcels, entity FROM worlds.world_scenes \
               UNION ALL SELECT world_name, entity_id, parcels, entity FROM worlds.retired_world_scenes) ws \
         WHERE EXISTS (SELECT 1 FROM social.scene_bans b WHERE b.place_id=ws.entity_id)",
    )
    .fetch(pool);
    while let Some((world, entity, parcels, declared_base)) = rows.try_next().await? {
        let scene = SceneId::from_stored(entity.clone());
        if let Ok((_, base)) = footprint(&parcels, declared_base.as_deref(), &scene)
            && entity != format!("{world}:{}", base.canonical())
        {
            anyhow::bail!(
                "world scene bans require release conversion before serving: stop old replicas, \
                 back up the database, and run schemas/releases/world-scene-ban-keys.sql \
                 with the administrative database role; then restart"
            );
        }
    }
    Ok(())
}

/// Refuse serving while a scene admin grant of an active deployment still has
/// the entity id for its key: requests read grants by place, so that grant
/// would be lost. `schemas/releases/scene-admin-place-keys.sql` moves them.
pub async fn ensure_scene_admin_place_release(pool: &PgPool) -> anyhow::Result<()> {
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM social.scene_admins a WHERE \
           EXISTS (SELECT 1 FROM explore.world_scene_place i WHERE i.active AND i.entity_id = a.place_id) \
           OR EXISTS (SELECT 1 FROM content.deployments d \
                      WHERE d.entity_id = a.place_id AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL \
                        AND social.scene_base_parcel(d.entity_pointers, d.entity_metadata #>> '{scene,base}') IS NOT NULL))",
    )
    .fetch_one(pool)
    .await?;
    anyhow::ensure!(
        !pending,
        "scene admin grants require release conversion before serving: stop old replicas, \
         back up the database, and run schemas/releases/scene-admin-place-keys.sql \
         with the administrative database role; then restart"
    );
    Ok(())
}

/// Refuse serving while a Genesis scene ban of an active deployment still has
/// the entity id for its key: requests read bans by place, so that ban would
/// be lost. `schemas/releases/genesis-scene-ban-keys.sql` moves them.
pub async fn ensure_genesis_scene_ban_release(pool: &PgPool) -> anyhow::Result<()> {
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM social.scene_bans b WHERE EXISTS (\
           SELECT 1 FROM content.deployments d \
            WHERE d.entity_id = b.place_id AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL \
              AND social.scene_base_parcel(d.entity_pointers, d.entity_metadata #>> '{scene,base}') IS NOT NULL))",
    )
    .fetch_one(pool)
    .await?;
    anyhow::ensure!(
        !pending,
        "Genesis scene bans require release conversion before serving: stop old replicas, \
         back up the database, and run schemas/releases/genesis-scene-ban-keys.sql \
         with the administrative database role; then restart"
    );
    Ok(())
}

/// The shared Places UUID explicitly bound to this active (world, entity)
/// deployment. Discovery and storage consume this same identity.
macro_rules! identity_column {
    () => {
        "(SELECT i.identity FROM explore.world_scene_place i \
          WHERE i.world_name = s.world_name AND i.active AND i.entity_id = s.entity_id) AS identity"
    };
}

/// The world's active scene: the newest `worlds.world_scenes` row, which is
/// the `scenesUrn[0]` `/world/{name}/about` advertises and the old host
/// loaded. `Ok(None)` when the world has no scene here.
pub async fn world_active_scene(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Option<ActiveDeployment>, ResolveError> {
    let row = sqlx::query(concat!(
        "SELECT entity_id, parcels, entity #>> '{metadata,scene,base}' AS base, entity #>> '{metadata,display,title}' AS title, mirror_source, ",
        identity_column!(),
        " FROM worlds.world_scenes s WHERE world_name = $1 ORDER BY created_at DESC LIMIT 1",
    ))
    .bind(world.key())
    .fetch_optional(pool)
    .await?;
    row.map(|r| world_row(world, &r)).transpose()
}

/// How many scenes the world has, up to two: the rows `world_active_scene`
/// chooses from.
pub async fn world_scene_count(pool: &PgPool, world: &WorldName) -> Result<usize, ResolveError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT 1 FROM worlds.world_scenes WHERE world_name = $1 LIMIT 2) s",
    )
    .bind(world.key())
    .fetch_one(pool)
    .await?;
    Ok(count as usize)
}

/// A named scene of a world, when deployed there or retired within
/// `SCENE_EVICTION_TTL_MS` (WCS resolves recently undeployed scenes' comms);
/// a retired scene has no identity, so no storage scope.
pub async fn world_scene(
    pool: &PgPool,
    world: &WorldName,
    scene: &SceneId,
) -> Result<Option<ActiveDeployment>, ResolveError> {
    let row = sqlx::query(concat!(
        "SELECT entity_id, parcels, entity #>> '{metadata,scene,base}' AS base, entity #>> '{metadata,display,title}' AS title, mirror_source, ",
        identity_column!(),
        " FROM worlds.world_scenes s WHERE world_name = $1 AND entity_id = $2 \
         UNION ALL SELECT entity_id, parcels, entity #>> '{metadata,scene,base}', entity #>> '{metadata,display,title}', mirror_source, NULL::uuid \
         FROM worlds.retired_world_scenes WHERE world_name = $1 AND entity_id = $2 \
         LIMIT 1",
    ))
    .bind(world.key())
    .bind(scene)
    .fetch_optional(pool)
    .await?;
    row.map(|r| world_row(world, &r)).transpose()
}

pub async fn replaced_world_scene(
    pool: &PgPool,
    world: &WorldName,
    scene: &SceneId,
) -> Result<Option<crate::comms::rooms::ReplacedScene>, ResolveError> {
    let row: Option<(Vec<String>, Option<String>)> = sqlx::query_as(
        "SELECT parcels, entity #>> '{metadata,scene,base}' FROM worlds.retired_world_scenes \
         WHERE world_name = $1 AND entity_id = $2",
    )
    .bind(world.key())
    .bind(scene)
    .fetch_optional(pool)
    .await?;
    let Some((parcels, base)) = row else {
        return Ok(None);
    };
    let Ok((footprint_parcels, base)) = footprint(&parcels, base.as_deref(), scene) else {
        return Ok(None);
    };
    let current: Vec<(String, Vec<String>, Option<String>)> = sqlx::query_as(
        "SELECT entity_id, parcels, entity #>> '{metadata,scene,base}' FROM worlds.world_scenes \
         WHERE world_name = $1 AND parcels && $2::text[]",
    )
    .bind(world.key())
    .bind(footprint_parcels.canonical())
    .fetch_all(pool)
    .await?;
    let mut successors: Vec<Parcel> = current
        .iter()
        .filter_map(|(id, parcels, base)| {
            footprint(parcels, base.as_deref(), &SceneId::from_stored(id.clone()))
                .ok()
                .map(|(_, base)| base)
        })
        .collect();
    successors.sort();
    successors.dedup();
    Ok(Some(crate::comms::rooms::ReplacedScene {
        base,
        successors,
    }))
}

/// The world scene standing on `parcel` (newest first, like the about).
pub async fn world_scene_at(
    pool: &PgPool,
    world: &WorldName,
    parcel: Parcel,
) -> Result<Option<ActiveDeployment>, ResolveError> {
    let row = sqlx::query(concat!(
        "SELECT entity_id, parcels, entity #>> '{metadata,scene,base}' AS base, entity #>> '{metadata,display,title}' AS title, mirror_source, ",
        identity_column!(),
        " FROM worlds.world_scenes s WHERE world_name = $1 AND $2 = ANY(parcels) ORDER BY created_at DESC LIMIT 1",
    ))
    .bind(world.key())
    .bind(parcel.canonical())
    .fetch_optional(pool)
    .await?;
    row.map(|r| world_row(world, &r)).transpose()
}

fn world_row(
    world: &WorldName,
    r: &sqlx::postgres::PgRow,
) -> Result<ActiveDeployment, ResolveError> {
    let stored: Vec<String> = r.get("parcels");
    let base: Option<String> = r.get("base");
    let local = r.get::<Option<String>, _>("mirror_source").is_none();
    deployment(
        &RealmId::World(world.clone()),
        SceneId::from_stored(r.get("entity_id")),
        &stored,
        base.as_deref(),
        r.get("title"),
        local,
        r.get("identity"),
    )
}

/// The Genesis City scene active at `parcel` (`content.active_pointers`).
pub async fn genesis_scene_at(
    pool: &PgPool,
    realm: &RealmId,
    parcel: Parcel,
) -> Result<Option<ActiveDeployment>, ResolveError> {
    let row = sqlx::query(
        "SELECT d.entity_id, d.entity_pointers, d.entity_metadata #>> '{scene,base}' AS base, d.entity_metadata #>> '{display,title}' AS title \
         FROM content.active_pointers ap \
         INNER JOIN content.deployments d ON d.entity_id = ap.entity_id \
         WHERE ap.pointer = $1 AND d.deleter_deployment IS NULL LIMIT 1",
    )
    .bind(parcel.canonical())
    .fetch_optional(pool)
    .await?;
    row.map(|r| genesis_row(realm, &r)).transpose()
}

/// A Genesis deployment by id, when it is active here: a scene whose
/// `deleter_deployment` is unset, whether or not it still holds a pointer.
pub async fn genesis_scene(
    pool: &PgPool,
    realm: &RealmId,
    scene: &SceneId,
) -> Result<Option<ActiveDeployment>, ResolveError> {
    let row = sqlx::query(
        "SELECT d.entity_id, d.entity_pointers, d.entity_metadata #>> '{scene,base}' AS base, d.entity_metadata #>> '{display,title}' AS title \
         FROM content.deployments d WHERE d.entity_id = $1 AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL",
    )
    .bind(scene)
    .fetch_optional(pool)
    .await?;
    row.map(|r| genesis_row(realm, &r)).transpose()
}

fn genesis_row(
    realm: &RealmId,
    r: &sqlx::postgres::PgRow,
) -> Result<ActiveDeployment, ResolveError> {
    let stored: Vec<String> = r.get("entity_pointers");
    let base: Option<String> = r.get("base");
    deployment(
        realm,
        SceneId::from_stored(r.get("entity_id")),
        &stored,
        base.as_deref(),
        r.get("title"),
        true,
        None,
    )
}

/// Resolves `request` under the worlds-service room contract. Errors are the old routes' answers:
/// a scene not deployed in the world (worlds service 404), a world without a
/// scene where the gatekeeper needed one (400), no scene at a parcel.
pub async fn resolve(
    pool: &PgPool,
    request: RoomRequest<'_>,
) -> Result<ResolvedRoom, ResolveError> {
    match request {
        RoomRequest::NamedWorldScene { room } => resolve_named_world_scene(pool, room).await,
        RoomRequest::WorldLobby { world } => {
            let room = worlds_service_world_room(world);
            match classify_room(pool, &room).await? {
                crate::comms::rooms::RoomClassification::Canonical(resolved) => Ok(resolved),
                _ => Err(ResolveError::UnresolvedRoom(room)),
            }
        }
        RoomRequest::WorldScene { world, scene } => {
            let dep = world_scene(pool, world, scene).await?.ok_or_else(|| {
                ResolveError::SceneNotInWorld {
                    world: world.display().to_string(),
                    scene: scene.to_string(),
                }
            })?;
            named(pool, RealmId::World(world.clone()), dep).await
        }
        RoomRequest::Adapter { realm, scene } => match realm {
            RealmId::World(world) if scene.is_world_shaped() => {
                let dep = world_active_scene(pool, world)
                    .await?
                    .ok_or_else(|| ResolveError::WorldHasNoScene(world.display().to_string()))?;
                named(pool, realm.clone(), dep).await
            }
            RealmId::World(world) => {
                let dep = world_scene(pool, world, scene).await?;
                match dep {
                    Some(dep) => named(pool, realm.clone(), dep).await,
                    None => Ok(as_given(realm.clone(), scene.clone(), None)),
                }
            }
            RealmId::Shared(_) => Ok(as_given(
                realm.clone(),
                scene.clone(),
                genesis_scene(pool, realm, scene).await?,
            )),
        },
        RoomRequest::Parcel { realm, parcel } => {
            let dep = match realm {
                RealmId::World(world) => world_scene_at(pool, world, parcel).await?,
                RealmId::Shared(_) => genesis_scene_at(pool, realm, parcel).await?,
            }
            .ok_or_else(|| ResolveError::NoSceneAtParcel {
                realm: realm.to_string(),
                parcel: parcel.to_string(),
            })?;
            named(pool, realm.clone(), dep).await
        }
    }
}

/// Complete world identity is read in one statement snapshot. Legacy matches
/// participate in collisions but can never be returned as canonical scenes.
pub async fn classify_room(
    pool: &PgPool,
    room: &str,
) -> Result<crate::comms::rooms::RoomClassification, ResolveError> {
    use crate::comms::rooms::{
        ParsedRoom, RoomClassification as C, RoomRefusal as R, refused_room,
    };
    if room.starts_with(crate::comms::rooms::SCENE_ROOM_PREFIX) {
        let Some((world, scene)) = crate::comms::rooms::decode_world_scene_room(room) else {
            return Ok(refused_room(room, R::Malformed));
        };
        // Identity is reversible, but authority requires this exact current (or retained) tuple.
        let current = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            world_scene(pool, &world, &scene),
        )
        .await
        .map_err(|_| ResolveError::Db(sqlx::Error::PoolTimedOut))?;
        return match current {
            Ok(Some(deployment)) => Ok(C::Canonical(resolved(RealmId::World(world), deployment))),
            Ok(None) => Ok(refused_room(room, R::Unresolved)),
            Err(ResolveError::NoParcels { .. }) => Ok(refused_room(room, R::Malformed)),
            Err(error) => Err(error),
        };
    }
    let parsed = crate::comms::rooms::parse_room(room);
    if let Some(ParsedRoom::Local(resolved)) = &parsed {
        return Ok(C::Canonical(resolved.clone()));
    }
    let lobby = room.strip_prefix("world-");
    let scene = room.strip_prefix("scene-");
    if lobby.is_none() && scene.is_none() {
        return Ok(if room.starts_with("scene:") {
            refused_room(room, R::Malformed)
        } else {
            C::Unrelated
        });
    }
    if room.to_lowercase() != room {
        return Ok(refused_room(room, R::Noncanonical));
    }
    if parsed.is_none() {
        return Ok(refused_room(room, R::Malformed));
    }
    let rest = lobby.or(scene).unwrap();
    let world = lobby.and_then(|raw| WorldName::parse(raw).ok());
    let candidates: Vec<String> = rest
        .match_indices('-')
        .filter_map(|(at, _)| {
            let world = WorldName::parse(&rest[..at]).ok()?;
            SceneId::parse(&rest[at + 1..]).ok()?;
            Some(world.key().to_owned())
        })
        .collect();
    let rows = tokio::time::timeout(std::time::Duration::from_secs(2), sqlx::query(concat!(
        "SELECT name AS world_name, NULL::text AS entity_id, NULL::text[] AS parcels, NULL::text AS base, NULL::text AS title, NULL::text AS mirror_source, NULL::uuid AS identity \
         FROM worlds.worlds WHERE $3 AND name = $4 AND 'world-' || name = $2 \
         UNION ALL SELECT world_name, entity_id, parcels, entity #>> '{metadata,scene,base}' AS base, entity #>> '{metadata,display,title}' AS title, mirror_source, ",
        identity_column!(),
        " FROM worlds.world_scenes s WHERE world_name = ANY($1) \
         AND (CASE WHEN $3 THEN 'world-' ELSE 'scene-' END) || world_name || '-' || lower(entity_id) = $2 LIMIT 2"
    )).bind(candidates).bind(room).bind(lobby.is_some()).bind(world.as_ref().map(WorldName::key)).fetch_all(pool)).await
        .map_err(|_| ResolveError::Db(sqlx::Error::PoolTimedOut))??;
    if rows.len() > 1 {
        return Ok(refused_room(room, R::Ambiguous));
    }
    let Some(row) = rows.first() else {
        return Ok(refused_room(
            room,
            if parsed.is_none() {
                R::Malformed
            } else {
                R::Unresolved
            },
        ));
    };
    let world = WorldName::parse(row.get::<&str, _>("world_name"))
        .map_err(|error| ResolveError::Db(sqlx::Error::Protocol(error.to_string())))?;
    if lobby.is_some() {
        if row.get::<Option<String>, _>("entity_id").is_some() {
            return Ok(refused_room(room, R::Legacy));
        }
        return Ok(C::Canonical(ResolvedRoom {
            realm: RealmId::World(world),
            scene_id: None,
            deployment: None,
            room: room.to_owned(),
        }));
    }
    match world_row(&world, row) {
        Ok(dep) => Ok(C::Canonical(resolved(RealmId::World(world), dep))),
        Err(ResolveError::NoParcels { .. }) => Ok(refused_room(room, R::Malformed)),
        Err(error) => Err(error),
    }
}

async fn resolve_named_world_scene(
    pool: &PgPool,
    room: &str,
) -> Result<ResolvedRoom, ResolveError> {
    match classify_room(pool, room).await? {
        crate::comms::rooms::RoomClassification::Canonical(resolved)
            if resolved.realm.is_world() && resolved.scene_id.is_some() =>
        {
            Ok(resolved)
        }
        _ => Err(ResolveError::UnresolvedRoom(room.to_owned())),
    }
}

fn room_for(realm: &RealmId, scene: &SceneId) -> String {
    match realm {
        RealmId::World(world) => worlds_service_scene_room(world, scene),
        RealmId::Shared(name) => gatekeeper_scene_room(name, scene.as_str()),
    }
}

async fn named(
    pool: &PgPool,
    realm: RealmId,
    dep: ActiveDeployment,
) -> Result<ResolvedRoom, ResolveError> {
    let selected = resolved(realm, dep);
    if !selected.realm.is_world() {
        return Ok(selected);
    }
    let unique = resolve_named_world_scene(pool, &selected.room).await?;
    if unique.realm != selected.realm || unique.scene_id != selected.scene_id {
        return Err(ResolveError::UnresolvedRoom(selected.room));
    }
    Ok(unique)
}

fn resolved(realm: RealmId, dep: ActiveDeployment) -> ResolvedRoom {
    let room = room_for(&realm, &dep.scene_id);
    ResolvedRoom {
        realm,
        scene_id: Some(dep.scene_id.clone()),
        deployment: Some(dep),
        room,
    }
}

fn as_given(realm: RealmId, scene: SceneId, dep: Option<ActiveDeployment>) -> ResolvedRoom {
    let room = room_for(&realm, &scene);
    ResolvedRoom {
        realm,
        scene_id: Some(scene),
        deployment: dep,
        room,
    }
}

/// The resolver over the ONE pool as comms asks it
/// (`comms::interfaces::SceneIdentity`): each method is the free function of
/// the same name.
pub struct SceneResolver {
    pool: PgPool,
}

impl SceneResolver {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl crate::comms::interfaces::SceneIdentity for SceneResolver {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::comms::rooms::RoomClassification, ResolveError> {
        classify_room(&self.pool, room).await
    }

    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        resolve(&self.pool, request).await
    }

    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        world_scene(&self.pool, world, scene).await
    }

    async fn replaced_world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<crate::comms::rooms::ReplacedScene>, ResolveError> {
        replaced_world_scene(&self.pool, world, scene).await
    }

    async fn world_active_scene(
        &self,
        world: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        world_active_scene(&self.pool, world).await
    }

    async fn world_scene_count(&self, world: &WorldName) -> Result<usize, ResolveError> {
        world_scene_count(&self.pool, world).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn scene(id: &str) -> SceneId {
        SceneId::parse(id).unwrap()
    }

    #[tokio::test]
    async fn canonical_room_lookup_preserves_exact_identity_and_rejects_retired_spelling() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = unique_world();
        seed_world_scene(&pool, &world, "CaseHash", &["0,0"], None, false, 0).await;
        seed_world_scene(&pool, &world, "casehash", &["invalid"], None, false, 0).await;
        let room = worlds_service_scene_room(&world, &scene("CaseHash"));
        let resolved = resolve(&pool, RoomRequest::NamedWorldScene { room: &room })
            .await
            .unwrap();
        assert_eq!(resolved.scene_id.unwrap().as_str(), "CaseHash");
        assert_eq!(resolved.realm.world().unwrap(), &world);
        let malformed = worlds_service_scene_room(&world, &scene("casehash"));
        assert_ne!(room, malformed);
        assert!(matches!(
            resolve(&pool, RoomRequest::NamedWorldScene { room: &malformed }).await,
            Err(ResolveError::UnresolvedRoom(_))
        ));
        let retired = format!("scene-{}-casehash", world.key());
        assert!(matches!(
            resolve(&pool, RoomRequest::NamedWorldScene { room: &retired }).await,
            Err(ResolveError::UnresolvedRoom(_))
        ));

        let prefix = format!("canonical-{}", Uuid::new_v4().simple());
        let first = WorldName::parse(&prefix).unwrap();
        let second = WorldName::parse(&format!("{prefix}-a")).unwrap();
        seed_world_scene(&pool, &first, "a-b", &["1,1"], None, false, 0).await;
        seed_world_scene(&pool, &second, "b", &["invalid"], None, false, 0).await;
        let a = worlds_service_scene_room(&first, &scene("a-b"));
        let b = worlds_service_scene_room(&second, &scene("b"));
        assert_ne!(a, b, "delimiter-shaped competitors name separate rooms");
        let selected = resolve(&pool, RoomRequest::NamedWorldScene { room: &a })
            .await
            .unwrap();
        assert_eq!(selected.realm.world(), Some(&first));
        assert_eq!(selected.scene_id.unwrap().as_str(), "a-b");
        assert!(matches!(
            resolve(&pool, RoomRequest::NamedWorldScene { room: &b }).await,
            Err(ResolveError::UnresolvedRoom(_))
        ));
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = 'a-b'")
            .bind(first.key())
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            matches!(
                resolve(&pool, RoomRequest::NamedWorldScene { room: &a }).await,
                Err(ResolveError::UnresolvedRoom(_))
            ),
            "decoding is not active-deployment authority"
        );
        assert!(
            crate::comms::rooms::is_world_room(&a, &first),
            "ownership survives replacement"
        );
        assert!(!crate::comms::rooms::is_world_room(&b, &first));
        for key in [world.key(), first.key(), second.key()] {
            sqlx::query("DELETE FROM worlds.worlds WHERE name = $1")
                .bind(key)
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn complete_lobby_authority_checks_catalogue_legacy_and_every_boundary() {
        use crate::comms::rooms::{RoomClassification as C, RoomRefusal as R};
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let prefix = format!("classifier-{}", Uuid::new_v4().simple());
        let short = WorldName::parse(&format!("{prefix}.dcl.eth")).unwrap();
        let full = WorldName::parse(&format!("{prefix}.dcl.eth-scene")).unwrap();
        let room = worlds_service_world_room(&full);
        assert!(matches!(
            classify_room(&pool, &room).await.unwrap(),
            C::RelevantRefused {
                reason: R::Unresolved,
                ..
            }
        ));
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
            .bind(full.key())
            .execute(&pool)
            .await
            .unwrap();
        let C::Canonical(lobby) = classify_room(&pool, &room).await.unwrap() else {
            panic!("catalogue-backed no-scene lobby");
        };
        assert_eq!(lobby.scene_id, None);
        assert_eq!(lobby.room, room);
        assert_eq!(
            resolve(&pool, RoomRequest::WorldLobby { world: &full })
                .await
                .unwrap(),
            lobby
        );
        seed_world_scene(&pool, &short, "Scene", &["invalid"], None, false, 0).await;
        assert!(matches!(
            classify_room(&pool, &room).await.unwrap(),
            C::RelevantRefused {
                reason: R::Ambiguous,
                ..
            }
        ));
        assert!(matches!(
            resolve(&pool, RoomRequest::WorldLobby { world: &full }).await,
            Err(ResolveError::UnresolvedRoom(_))
        ));
        sqlx::query("DELETE FROM worlds.worlds WHERE name = $1")
            .bind(full.key())
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            classify_room(&pool, &room).await.unwrap(),
            C::RelevantRefused {
                reason: R::Legacy,
                ..
            }
        ));
        assert!(matches!(
            classify_room(&pool, &format!("world-{}-Scene", short.key()))
                .await
                .unwrap(),
            C::RelevantRefused {
                reason: R::Noncanonical,
                ..
            }
        ));
        // All boundaries count, even when an earlier competing footprint is malformed.
        let other = WorldName::parse(&format!("{}-scene", short.key())).unwrap();
        seed_world_scene(&pool, &short, "scene-tail", &["invalid"], None, false, 0).await;
        seed_world_scene(&pool, &other, "tail", &["0,0"], None, false, 0).await;
        assert!(matches!(
            classify_room(&pool, &format!("world-{}-scene-tail", short.key()))
                .await
                .unwrap(),
            C::RelevantRefused {
                reason: R::Ambiguous,
                ..
            }
        ));
        for world in [&short, &other] {
            sqlx::query("DELETE FROM worlds.worlds WHERE name = $1")
                .bind(world.key())
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn complete_classification_distinguishes_refusal_unrelated_and_store_failure() {
        use crate::comms::rooms::{RoomClassification as C, RoomRefusal as R};
        let pool = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        pool.close().await;
        assert!(matches!(
            classify_room(&pool, "voice-chat-private-example")
                .await
                .unwrap(),
            C::Unrelated
        ));
        assert!(matches!(
            classify_room(&pool, "island-example").await.unwrap(),
            C::Unrelated
        ));
        assert!(matches!(
            classify_room(&pool, "scene:").await.unwrap(),
            C::RelevantRefused {
                reason: R::Malformed,
                ..
            }
        ));
        assert!(matches!(
            classify_room(&pool, "world-Mixed.eth").await.unwrap(),
            C::RelevantRefused {
                reason: R::Noncanonical,
                ..
            }
        ));
        assert!(matches!(
            classify_room(&pool, "world-known.eth").await,
            Err(ResolveError::Db(_))
        ));
        let C::Canonical(shared) = classify_room(&pool, "scene:main:Case-ID").await.unwrap() else {
            panic!("shared scene");
        };
        assert_eq!(shared.scene_id.unwrap().as_str(), "Case-ID");
    }

    fn unique_world() -> WorldName {
        WorldName::parse(&format!(
            "R{}.dcl.eth",
            &Uuid::new_v4().simple().to_string()[..10]
        ))
        .unwrap()
    }

    async fn seed_world_scene(
        pool: &PgPool,
        world: &WorldName,
        id: &str,
        parcels: &[&str],
        base: Option<&str>,
        mirrored: bool,
        age_secs: i64,
    ) {
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(world.key())
            .execute(pool)
            .await
            .unwrap();
        let entity = match base {
            Some(b) => serde_json::json!({ "metadata": { "scene": { "base": b } } }),
            None => serde_json::json!({}),
        };
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source, created_at) \
             VALUES ($1, $2, '[]'::json, $3, $4, $5, 1, $6, timezone('UTC', now()) - ($7 || ' seconds')::interval)",
        )
        .bind(world.key())
        .bind(id)
        .bind(entity)
        .bind(crate::ZERO_ADDRESS)
        .bind(parcels.iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .bind(mirrored.then_some("https://upstream.example"))
        .bind(age_secs.to_string())
        .execute(pool)
        .await
        .unwrap();
        let parcels: Vec<String> = parcels.iter().map(|p| p.to_string()).collect();
        let mut conn = pool.acquire().await.unwrap();
        crate::explore::places::scene_identity::assign(&mut conn, world.key(), id, &parcels)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_world_resolves_to_its_newest_scene_and_stable_namespace() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let w = unique_world();
        seed_world_scene(&pool, &w, "bafyOld", &["0,0"], None, false, 100).await;
        seed_world_scene(&pool, &w, "bafyNew", &["1,1", "0,1"], Some("1,1"), true, 0).await;
        sqlx::query("UPDATE worlds.world_scenes SET entity=jsonb_set(entity,'{metadata,display}',jsonb_build_object('title',entity_id)) WHERE world_name=$1")
            .bind(w.key()).execute(&pool).await.unwrap();
        let realm = RealmId::World(w.clone());
        let world_shaped = SceneId::parse(w.display()).unwrap();
        let gk = resolve(
            &pool,
            RoomRequest::Adapter {
                realm: &realm,
                scene: &world_shaped,
            },
        )
        .await
        .unwrap();
        assert_eq!(gk.room, worlds_service_scene_room(&w, &scene("bafyNew")));
        let dep = gk.deployment.as_ref().unwrap();
        assert_eq!(dep.title.as_deref(), Some("bafyNew"));
        assert_eq!((dep.base.canonical().as_str(), dep.local), ("1,1", false));
        let ns = dep.namespace.as_ref().unwrap();
        assert_eq!(ns.realm(), w.key());
        assert!(
            Uuid::parse_str(&ns.scene_namespace()).is_ok(),
            "the scene's identity, not its base"
        );
        let ws = resolve(
            &pool,
            RoomRequest::WorldScene {
                world: &w,
                scene: &scene("bafyNew"),
            },
        )
        .await
        .unwrap();
        assert_eq!(ws.room, worlds_service_scene_room(&w, &scene("bafyNew")));
        assert_eq!(
            ws.deployment, gk.deployment,
            "one deployment through both requests"
        );
        // The lobby carries no deployment.
        let lobby = resolve(&pool, RoomRequest::WorldLobby { world: &w })
            .await
            .unwrap();
        assert_eq!(
            (lobby.room.as_str(), lobby.scene_id.is_none()),
            (format!("world-{}", w.key()).as_str(), true)
        );
        // A scene not in the world is the worlds service's 404; the
        // gatekeeper takes an explicit id as given.
        assert!(matches!(
            resolve(
                &pool,
                RoomRequest::WorldScene {
                    world: &w,
                    scene: &scene("bafyNope")
                }
            )
            .await,
            Err(ResolveError::SceneNotInWorld { .. })
        ));
        let given = resolve(
            &pool,
            RoomRequest::Adapter {
                realm: &realm,
                scene: &scene("bafyNope"),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            (given.room.as_str(), given.deployment.is_none()),
            (
                worlds_service_scene_room(&w, &scene("bafyNope")).as_str(),
                true
            )
        );
        // By parcel: the older scene still stands on 0,0 with its own
        // namespace.
        let at = resolve(
            &pool,
            RoomRequest::Parcel {
                realm: &realm,
                parcel: Parcel { x: 0, y: 0 },
            },
        )
        .await
        .unwrap();
        let dep = at.deployment.unwrap();
        assert_eq!(
            (
                dep.scene_id.as_str(),
                dep.base.canonical().as_str(),
                dep.local
            ),
            ("bafyOld", "0,0", true)
        );
        assert_ne!(
            dep.namespace,
            gk.deployment.as_ref().unwrap().namespace,
            "the scenes of a world have their own scopes"
        );
        // A redeploy overlapping that one scene keeps its namespace, base moved.
        sqlx::query(
            "DELETE FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = 'bafyOld'",
        )
        .bind(w.key())
        .execute(&pool)
        .await
        .unwrap();
        seed_world_scene(
            &pool,
            &w,
            "bafyOld2",
            &["0,-1", "0,0"],
            Some("0,-1"),
            false,
            0,
        )
        .await;
        let again = resolve(
            &pool,
            RoomRequest::Parcel {
                realm: &realm,
                parcel: Parcel { x: 0, y: -1 },
            },
        )
        .await
        .unwrap();
        let again = again.deployment.unwrap();
        assert_eq!(
            (again.base.canonical().as_str(), &again.namespace),
            ("0,-1", &dep.namespace)
        );
        let empty = unique_world();
        assert!(matches!(
            resolve(
                &pool,
                RoomRequest::Adapter {
                    realm: &RealmId::World(empty.clone()),
                    scene: &SceneId::parse(empty.display()).unwrap()
                }
            )
            .await,
            Err(ResolveError::WorldHasNoScene(_))
        ));
    }

    /// The place key of the scene `request` resolves to, and whether the
    /// policy takes `actor` for its admin.
    async fn place_admin(
        pool: &PgPool,
        request: RoomRequest<'_>,
        actor: &crate::comms::Address,
    ) -> (String, bool) {
        use crate::comms::access::SceneRef;
        let policy = crate::policy::Policy::new(
            pool.clone(),
            crate::policy::PersonalWorldsPolicy::disabled(),
            None,
        );
        let room = resolve(pool, request).await.unwrap();
        let deployment = room.deployment.unwrap();
        let place = deployment.place_key();
        let scene = match room.realm.world() {
            Some(world) => SceneRef::World {
                world,
                place: &place,
            },
            None => SceneRef::Genesis {
                place: &place,
                parcels: &deployment.parcels,
            },
        };
        let admin = policy.is_admin(&scene, actor).await.is_allowed();
        (place, admin)
    }

    /// A Genesis scene; those that held its parcels are marked as replaced, as
    /// the deploy write does.
    async fn deploy_genesis(pool: &PgPool, parcels: &[&String], base: &str) -> String {
        let id = format!("bafyG{}", Uuid::new_v4().simple());
        sqlx::query(
            "WITH ins AS (INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ('0x0', 'v3', 'scene', $1, $2::json, timezone('UTC', now()), $3, timezone('UTC', now()), '[]'::json) RETURNING id) \
             UPDATE content.deployments old SET deleter_deployment = ins.id FROM ins \
             WHERE old.entity_id IN (SELECT entity_id FROM content.active_pointers WHERE pointer = ANY($3))",
        )
        .bind(&id)
        .bind(serde_json::json!({ "scene": { "base": base } }))
        .bind(parcels.iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .execute(pool)
        .await
        .unwrap();
        for parcel in parcels {
            sqlx::query(
                "INSERT INTO content.active_pointers (pointer, entity_id) VALUES ($1, $2) \
                 ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id",
            )
            .bind(parcel)
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        }
        id
    }

    /// Scene admins are granted on the place (`ActiveDeployment::place_key`):
    /// a redeploy of the same place keeps them (a world scene that overlaps
    /// one scene, a moved base included; a Genesis scene on the same base);
    /// a scene deployed after a removal, or on another Genesis base, is
    /// another place without them.
    #[tokio::test]
    async fn a_redeploy_of_the_same_place_keeps_its_scene_admins() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let admin =
            crate::comms::Address::parse(&format!("0x{:040x}", Uuid::new_v4().as_u128())).unwrap();
        let grant = async |place: &str| {
            sqlx::query("INSERT INTO social.scene_admins (place_id, admin_address, added_by) VALUES ($1, $2, $2)")
                .bind(place)
                .bind(&admin)
                .execute(&pool)
                .await
                .unwrap();
        };

        let w = unique_world();
        let undeploy = async |id: &str| {
            sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = $2")
                .bind(w.key())
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            let mut conn = pool.acquire().await.unwrap();
            crate::explore::places::scene_identity::retire_orphans(
                &mut conn,
                &[w.key().to_owned()],
            )
            .await
            .unwrap();
        };
        let (a, b, c) = (scene("bafyA"), scene("bafyB"), scene("bafyC"));
        let at = |scene| RoomRequest::WorldScene { world: &w, scene };
        seed_world_scene(&pool, &w, "bafyA", &["0,0", "0,1"], Some("0,0"), false, 0).await;
        let (place, _) = place_admin(&pool, at(&a), &admin).await;
        grant(&place).await;
        seed_world_scene(&pool, &w, "bafyB", &["0,1", "0,2"], Some("0,2"), false, 0).await;
        undeploy("bafyA").await;
        assert_eq!(
            place_admin(&pool, at(&b), &admin).await,
            (place.clone(), true),
            "one overlapped scene: the same place"
        );
        undeploy("bafyB").await;
        seed_world_scene(&pool, &w, "bafyC", &["0,1", "0,2"], Some("0,2"), false, 0).await;
        let (after_removal, is_admin) = place_admin(&pool, at(&c), &admin).await;
        assert!(
            after_removal != place && !is_admin,
            "a removed scene's place does not return"
        );

        let realm = RealmId::parse("main").unwrap();
        let x = 30_000 + (Uuid::new_v4().as_u128() % 10_000) as i32;
        let (p1, p2) = (format!("{x},7"), format!("{x},8"));
        let on = |parcel: &String| RoomRequest::Parcel {
            realm: &realm,
            parcel: Parcel::parse(parcel).unwrap(),
        };
        let mut ids = vec![deploy_genesis(&pool, &[&p1, &p2], &p2).await];
        let (place, _) = place_admin(&pool, on(&p1), &admin).await;
        grant(&place).await;
        ids.push(deploy_genesis(&pool, &[&p2], &p2).await);
        assert_eq!(
            place_admin(&pool, on(&p2), &admin).await,
            (p2.clone(), true),
            "the same base parcel: the same place"
        );
        ids.push(deploy_genesis(&pool, &[&p1, &p2], &p1).await);
        assert_eq!(
            place_admin(&pool, on(&p2), &admin).await,
            (p1.clone(), false),
            "another base parcel: another place"
        );
        sqlx::query("DELETE FROM content.active_pointers WHERE entity_id = ANY($1)")
            .bind(&ids)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id = ANY($1)")
            .bind(&ids)
            .execute(&pool)
            .await
            .unwrap();
    }

    /// `social.scene_base_parcel` is the SQL copy of the base the resolver
    /// computes: the same trim, bounds, order and declared-base rule.
    #[tokio::test]
    async fn the_sql_base_parcel_is_the_resolvers() {
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let table: [(&[&str], Option<&str>); 10] = [
            (&["0,0", "0,1"], Some("0,1")),
            (&["0,0", "0,1"], Some("5,5")),
            (&["10,2", "9,3", "-3,5", "-3,-5"], None),
            (&[" 07 , 08 ", "7,7"], Some("7,8")),
            (&["junk", "9,9"], Some("junk")),
            (
                &[
                    "junk",
                    "+1,2",
                    "1,2,3",
                    "1;2",
                    "",
                    "-,1",
                    "1,-",
                    "1\u{200b},2",
                ],
                None,
            ),
            (&[], Some("0,0")),
            (
                &["2147483647,-2147483648", "2147483648,0"],
                Some("2147483648,0"),
            ),
            (
                &["0,-2147483649", "000000000000000000000001,-0"],
                Some("1,0"),
            ),
            (&["99999999999999999999,1", "3,4"], Some("")),
        ];
        let mut cases: Vec<(Vec<String>, Option<String>)> = table
            .into_iter()
            .map(|(parcels, base)| {
                let parcels = parcels.iter().map(|p| p.to_string()).collect();
                (parcels, base.map(str::to_owned))
            })
            .collect();
        for space in "\t\n\u{b}\u{c}\r \u{85}\u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}".chars() {
            let parcel = format!("{space}02{space},{space}003{space}");
            cases.push((vec!["0,0".into(), parcel.clone()], Some(parcel)));
        }
        for (parcels, declared) in cases {
            let resolver = footprint(&parcels, declared.as_deref(), &scene("bafyS"))
                .ok()
                .map(|(_, base)| base.canonical());
            let sql: Option<String> = sqlx::query_scalar("SELECT social.scene_base_parcel($1, $2)")
                .bind(&parcels)
                .bind(&declared)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(sql, resolver, "{parcels:?} {declared:?}");
        }
    }

    /// The release that moves the grants from the scene entity to its place:
    /// the admins of an active deployment keep their scene, with the key the
    /// resolver reads, in every place where the entity is active; a place
    /// keeps the active or newest grant of a wallet; a grant on an entity
    /// that is not deployed, or has no parcel, stays. The node does not
    /// serve before the release ran.
    #[tokio::test]
    async fn the_place_key_release_moves_the_grants_of_active_deployments() {
        const RELEASE: &str = include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/schemas/releases/scene-admin-place-keys.sql"
        ));
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let wallet = || {
            crate::comms::Address::parse(&format!("0x{:040x}", Uuid::new_v4().as_u128())).unwrap()
        };
        let (admin, other, late) = (wallet(), wallet(), wallet());
        // `days` ago; a revoked grant was removed one day after it was added.
        let grant = async |place: &str, who: &crate::comms::Address, days: i32, active: bool| {
            sqlx::query(
                "INSERT INTO social.scene_admins (place_id, admin_address, added_by, added_at, active, removed_at) \
                 SELECT $1, $2, $2, t, $4, CASE WHEN NOT $4 THEN t + interval '1 day' END \
                 FROM (SELECT timezone('UTC', now()) - make_interval(days => $3) AS t) x",
            )
            .bind(place)
            .bind(who)
            .bind(days)
            .bind(active)
            .execute(&pool)
            .await
            .unwrap();
        };
        let (w, w2, realm) = (
            unique_world(),
            unique_world(),
            RealmId::parse("main").unwrap(),
        );
        let [p1, p2, p3, p4] = ["7,7", "7,8", "7,9", "8,8"].map(str::to_owned);
        // One entity on Genesis (base "7,8") and in two worlds.
        let shared = deploy_genesis(&pool, &[&p1, &p2], " 07 , 08 ").await;
        seed_world_scene(&pool, &w, &shared, &["0,0"], None, false, 0).await;
        seed_world_scene(&pool, &w2, &shared, &["0,0"], None, false, 0).await;
        // A second entity of the base "7,8": it takes the parcel "7,8". The
        // first entity stays active, so two entities name one place.
        let second = deploy_genesis(&pool, &[&p2], &p2).await;
        sqlx::query(
            "UPDATE content.deployments SET deleter_deployment = NULL WHERE entity_id = $1",
        )
        .bind(&shared)
        .execute(&pool)
        .await
        .unwrap();
        // No base that parses: one parcel that parses, then none.
        let junk = "junk".to_string();
        let partial = deploy_genesis(&pool, &[&junk, &p3], "junk").await;
        let no_parcel = deploy_genesis(&pool, &[&"none".to_string()], "junk").await;
        let third = deploy_genesis(&pool, &[&p4], &p4).await;

        grant(&shared, &admin, 9, true).await;
        grant(&second, &admin, 2, false).await;
        grant(&shared, &other, 9, false).await;
        grant(&second, &other, 5, true).await;
        grant(&partial, &admin, 1, true).await;
        grant(&no_parcel, &admin, 1, true).await;
        grant("bafyGone", &admin, 1, true).await;
        // Rows that the place has: an older revoked one loses to the active
        // grant, a newer revocation wins over it.
        grant(&third, &admin, 3, true).await;
        grant(&p4, &admin, 9, false).await;
        grant(&third, &late, 3, true).await;
        grant(&p4, &late, 2, false).await;

        let unmoved = ensure_scene_admin_place_release(&pool).await.unwrap_err();
        assert!(
            unmoved.to_string().contains("scene-admin-place-keys.sql"),
            "{unmoved}"
        );
        for _ in 0..2 {
            sqlx::raw_sql(RELEASE).execute(&pool).await.unwrap();
        }
        ensure_scene_admin_place_release(&pool).await.unwrap();

        let in_world = SceneId::from_stored(shared.clone());
        let mut places = Vec::new();
        for world in [&w, &w2] {
            let request = RoomRequest::WorldScene {
                world,
                scene: &in_world,
            };
            let (place, is_admin) = place_admin(&pool, request, &admin).await;
            assert!(is_admin, "{place}");
            places.push(place);
        }
        let on = |parcel: &str| RoomRequest::Parcel {
            realm: &realm,
            parcel: Parcel::parse(parcel).unwrap(),
        };
        for (parcel, place, who, is_admin) in [
            (&p1, &p2, &admin, true),
            (&p2, &p2, &other, true),
            (&p3, &p3, &admin, true),
            (&p4, &p4, &admin, true),
            (&p4, &p4, &late, false),
        ] {
            assert_eq!(
                place_admin(&pool, on(parcel), who).await,
                (place.clone(), is_admin),
                "{parcel}"
            );
        }
        let mut keys: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT place_id FROM social.scene_admins")
                .fetch_all(&pool)
                .await
                .unwrap();
        keys.sort();
        places.extend([p2, p3, p4, no_parcel, "bafyGone".to_string()]);
        places.sort();
        assert_eq!(keys, places);
    }

    /// The release that moves Genesis scene bans from the scene entity to its
    /// place: a ban of an active deployment is read where the resolver keys
    /// it, two entities of one place keep the newest ban of a wallet, and a
    /// ban on an entity that is not deployed, or has no parcel, stays. The
    /// node does not serve before the release ran.
    #[tokio::test]
    async fn the_genesis_ban_release_moves_the_bans_of_active_deployments() {
        const RELEASE: &str = include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/schemas/releases/genesis-scene-ban-keys.sql"
        ));
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let wallet = || format!("0x{:040x}", Uuid::new_v4().as_u128());
        let (banned, other) = (wallet(), wallet());
        let ban = async |place: &str, who: &str, by: &str, days: i32| {
            sqlx::query(
                "INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at) \
                 VALUES ($1, $2, $3, timezone('UTC', now()) - make_interval(days => $4))",
            )
            .bind(place)
            .bind(who)
            .bind(by)
            .bind(days)
            .execute(&pool)
            .await
            .unwrap();
        };
        let [p1, p2, p3] = ["9,7", "9,8", "9,9"].map(str::to_owned);
        // Two active entities of the base "9,8".
        let first = deploy_genesis(&pool, &[&p1, &p2], " 09 , 08 ").await;
        let second = deploy_genesis(&pool, &[&p2], &p2).await;
        sqlx::query(
            "UPDATE content.deployments SET deleter_deployment = NULL WHERE entity_id = $1",
        )
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();
        let no_parcel = deploy_genesis(&pool, &[&"none".to_string()], "junk").await;
        let third = deploy_genesis(&pool, &[&p3], &p3).await;
        ban(&first, &banned, "older", 9).await;
        ban(&second, &banned, "newer", 2).await;
        ban(&first, &other, "first", 5).await;
        ban(&no_parcel, &banned, "kept", 1).await;
        ban("bafyGone", &banned, "kept", 1).await;
        // Rows the place has: a newer moved ban wins, a newer place row stays.
        ban(&third, &banned, "moved", 1).await;
        ban(&p3, &banned, "stale", 4).await;
        ban(&third, &other, "older-move", 6).await;
        ban(&p3, &other, "place-newer", 3).await;

        let unmoved = ensure_genesis_scene_ban_release(&pool).await.unwrap_err();
        assert!(
            unmoved.to_string().contains("genesis-scene-ban-keys.sql"),
            "{unmoved}"
        );
        for _ in 0..2 {
            sqlx::raw_sql(RELEASE).execute(&pool).await.unwrap();
        }
        ensure_genesis_scene_ban_release(&pool).await.unwrap();

        let realm = RealmId::parse("main").unwrap();
        for parcel in [&p1, &p3] {
            let resolved = resolve(
                &pool,
                RoomRequest::Parcel {
                    realm: &realm,
                    parcel: Parcel::parse(parcel).unwrap(),
                },
            )
            .await
            .unwrap();
            let key = resolved.ban_key().unwrap();
            let banned_here: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM social.scene_bans WHERE place_id = $1 AND banned_address = $2)",
            )
            .bind(key.as_str())
            .bind(&banned)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(banned_here, "{parcel}: {}", key.as_str());
        }
        let mut rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT place_id, banned_address, banned_by FROM social.scene_bans")
                .fetch_all(&pool)
                .await
                .unwrap();
        rows.sort();
        let mut expected = vec![
            (p2.clone(), banned.clone(), "newer".to_owned()),
            (p2.clone(), other.clone(), "first".to_owned()),
            (p3.clone(), banned.clone(), "moved".to_owned()),
            (p3.clone(), other.clone(), "place-newer".to_owned()),
            (no_parcel.clone(), banned.clone(), "kept".to_owned()),
            ("bafyGone".to_owned(), banned.clone(), "kept".to_owned()),
        ];
        expected.sort();
        assert_eq!(rows, expected);
    }

    #[tokio::test]
    async fn a_genesis_parcel_resolves_through_the_active_pointers() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let id = format!("bafyG{}", Uuid::new_v4().simple());
        let x = 20_000 + (Uuid::new_v4().as_u128() % 10_000) as i32;
        let (p1, p2) = (format!("{x},7"), format!("{x},8"));
        let dep_id: i32 = sqlx::query_scalar(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ('0x0', 'v3', 'scene', $1, $2::json, timezone('UTC', now()), $3, timezone('UTC', now()), '[]'::json) RETURNING id",
        )
        .bind(&id)
        .bind(serde_json::json!({ "scene": { "base": p2 }, "display": { "title": "Exact Genesis" } }))
        .bind(vec![p1.clone(), p2.clone()])
        .fetch_one(&pool)
        .await
        .unwrap();
        for p in [&p1, &p2] {
            sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id) VALUES ($1, $2)")
                .bind(p)
                .bind(&id)
                .execute(&pool)
                .await
                .unwrap();
        }
        let realm = RealmId::parse("main").unwrap();
        let at = resolve(
            &pool,
            RoomRequest::Parcel {
                realm: &realm,
                parcel: Parcel::parse(&p1).unwrap(),
            },
        )
        .await
        .unwrap();
        assert_eq!(at.room, format!("scene:main:{id}"));
        let dep = at.deployment.unwrap();
        assert_eq!(dep.title.as_deref(), Some("Exact Genesis"));
        let ns = dep.namespace.as_ref().unwrap();
        assert_eq!(
            (ns.realm(), ns.scene_namespace().as_str()),
            ("main", p2.as_str())
        );
        assert!(dep.local);
        let by_id = resolve(
            &pool,
            RoomRequest::Adapter {
                realm: &RealmId::parse("artemis").unwrap(),
                scene: &scene(&id),
            },
        )
        .await
        .unwrap();
        assert_eq!(by_id.room, format!("scene:artemis:{id}"));
        assert_eq!(
            by_id.deployment.as_ref().unwrap().namespace,
            dep.namespace,
            "every shared realm shares the Genesis scope"
        );
        // A preview realm's unknown scene is named, not resolved.
        let preview = resolve(
            &pool,
            RoomRequest::Adapter {
                realm: &RealmId::parse("localpreview").unwrap(),
                scene: &scene("b64"),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            (preview.room.as_str(), preview.deployment.is_none()),
            ("scene:localpreview:b64", true)
        );
        sqlx::query("DELETE FROM content.active_pointers WHERE entity_id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE id = $1")
            .bind(dep_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}
