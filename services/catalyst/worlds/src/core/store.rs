//! The write statements of the worlds product surface over `worlds.*`
//! (`schemas/20_worlds.sql`), schema-qualified, on the ONE pool. Every world
//! key is `WorldName::key()` (the tables store lowercase names and every
//! statement binds the key, so the PKs serve); the access setting is the
//! three typed columns plus the `access` grants and the communities rows;
//! grants are keyed `(world_name, permission, address)`. Reads that stage 4
//! already ported (`about`, `scenes`, `index`) stay in `worlds::queries`.

use std::collections::HashSet;

use chrono::NaiveDateTime;
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::comms::world_identity::{Parcel, ParcelSet, SceneId, WorldName};
use crate::policy::{AccessType, Deploy, Permission};
use crate::shared::identity::Address;
use catalyst_storage::Storage;

use super::http::ApiError;
use super::settings_policy::{
    DESCRIPTION_MAX_LENGTH, DESCRIPTION_MIN_LENGTH, MAX_CATEGORIES, TITLE_MAX_LENGTH,
    TITLE_MIN_LENGTH, VALID_RATINGS, js_truthy, storable_skybox_time, storable_thumbnail_hash,
    text_len,
};

pub const MAX_ACCESS_WALLETS: i64 = 1000;
pub const MAX_ACCESS_COMMUNITIES: i64 = 50;

/// The world's access setting as the wire carries it (the old JSONB
/// document): the typed columns plus the `access` grants and communities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AccessSetting {
    #[default]
    Unrestricted,
    /// The bcrypt hash, never the secret.
    SharedSecret {
        secret_hash: String,
    },
    NftOwnership {
        nft: String,
    },
    AllowList {
        wallets: Vec<String>,
        communities: Vec<String>,
    },
}

impl AccessSetting {
    pub fn access_type(&self) -> AccessType {
        match self {
            Self::Unrestricted => AccessType::Unrestricted,
            Self::SharedSecret { .. } => AccessType::SharedSecret,
            Self::NftOwnership { .. } => AccessType::NftOwnership,
            Self::AllowList { .. } => AccessType::AllowList,
        }
    }

    pub fn is_shared_secret(&self) -> bool {
        matches!(self, Self::SharedSecret { .. })
    }

    pub fn to_public_json(&self) -> Value {
        match self {
            Self::Unrestricted => json!({ "type": "unrestricted" }),
            Self::SharedSecret { .. } => json!({ "type": "shared-secret" }),
            Self::NftOwnership { nft } => json!({ "type": "nft-ownership", "nft": nft }),
            Self::AllowList {
                wallets,
                communities,
            } => {
                json!({ "type": "allow-list", "wallets": wallets, "communities": communities })
            }
        }
    }

    fn from_row(r: &PgRow) -> Self {
        match AccessType::parse(&r.get::<String, _>("access_type"))
            .unwrap_or(AccessType::Unrestricted)
        {
            AccessType::Unrestricted => Self::Unrestricted,
            AccessType::SharedSecret => Self::SharedSecret {
                secret_hash: r
                    .get::<Option<String>, _>("access_secret_hash")
                    .unwrap_or_default(),
            },
            AccessType::NftOwnership => Self::NftOwnership {
                nft: r.get::<Option<String>, _>("access_nft").unwrap_or_default(),
            },
            AccessType::AllowList => Self::AllowList {
                wallets: r.get::<Vec<String>, _>("access_wallets"),
                communities: r.get::<Vec<String>, _>("access_communities"),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldRecord {
    pub name: String,
    pub owner: Option<Address>,
    pub access: AccessSetting,
    pub blocked_since: Option<NaiveDateTime>,
    pub spawn_coordinates: Option<String>,
    pub skybox_time: Option<i32>,
    pub single_player: bool,
    pub realm_name_override: Option<String>,
    pub preview_wearable_urns: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct PermissionRecord {
    pub permission: String,
    pub address: String,
    pub is_world_wide: bool,
    pub parcel_count: i64,
}

#[derive(Debug, Clone)]
pub struct WorldAdminRow {
    pub name: String,
    pub owner: Option<String>,
    pub access_type: String,
    pub blocked_since: Option<NaiveDateTime>,
    pub spawn_coordinates: Option<String>,
    pub scene_count: i64,
}

#[derive(Debug, Clone)]
pub struct BlockedRow {
    pub wallet: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AccessLogRow {
    pub id: i64,
    pub world_name: String,
    pub address: String,
    pub action: String,
    pub room: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldsOrderBy {
    Name,
    LastDeployedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Default)]
pub struct WorldsListFilters {
    pub authorized_deployer: Option<String>,
    pub search: Option<String>,
    pub has_deployed_scenes: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct WorldsListOptions {
    pub limit: i64,
    pub offset: i64,
    pub order_by: WorldsOrderBy,
    pub order_direction: OrderDirection,
}

#[derive(Debug, Clone)]
pub struct WorldInfoRow {
    pub name: String,
    pub owner: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_rating: Option<String>,
    pub spawn_coordinates: Option<String>,
    pub skybox_time: Option<i32>,
    pub categories: Option<Vec<String>>,
    pub single_player: bool,
    pub show_in_places: bool,
    pub thumbnail_hash: Option<String>,
    pub last_deployed_at: Option<NaiveDateTime>,
    pub min_x: Option<i32>,
    pub max_x: Option<i32>,
    pub min_y: Option<i32>,
    pub max_y: Option<i32>,
    pub blocked_since: Option<NaiveDateTime>,
    pub deployed_scenes: i64,
}

/// `single_player`/`show_in_places` are NOT NULL in the v2 schema (the old
/// nullable columns rendered their effective defaults on the wire anyway).
#[derive(Debug, Clone, Default)]
pub struct WorldSettingsRow {
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_rating: Option<String>,
    pub spawn_coordinates: Option<String>,
    pub skybox_time: Option<i32>,
    pub categories: Option<Vec<String>>,
    pub single_player: bool,
    pub show_in_places: bool,
    pub thumbnail_hash: Option<String>,
    pub access_type: Option<String>,
    pub realm_name_override: Option<String>,
    pub preview_wearable_urns: Option<Vec<String>>,
    pub settings_version: i64,
}

#[derive(Debug, Clone, Default)]
pub struct WorldSettingsUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_rating: Option<String>,
    pub spawn_coordinates: Option<String>,
    pub skybox_time: Option<i32>,
    pub skybox_time_provided: bool,
    pub categories: Option<Vec<String>>,
    pub categories_provided: bool,
    pub single_player: Option<bool>,
    pub show_in_places: Option<bool>,
    pub thumbnail_hash: Option<String>,
    pub realm_name_override: Option<String>,
    pub realm_name_override_provided: bool,
    pub preview_wearable_urns: Option<Vec<String>>,
    pub preview_wearable_urns_provided: bool,
}

#[derive(Debug, Clone)]
pub struct WorldManifest {
    pub parcels: Vec<String>,
    pub spawn_coordinates: Option<String>,
    pub total: i64,
}

/// One edit to an allow-list access setting; wallets arrive lowercased.
#[derive(Debug, Clone, Copy)]
pub enum AllowListEdit<'a> {
    AddWallet(&'a Address),
    RemoveWallet(&'a Address),
    AddCommunity(Uuid),
    RemoveCommunity(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowListEditOutcome {
    Applied,
    /// No worlds row, or its access type is not `allow-list`.
    NotAllowList,
    /// The list is at its maximum size.
    CapExceeded,
}

/// One scene deploy as [`deploy_scene`] persists it into `world_scenes`.
pub struct SceneDeployment<'a> {
    pub world: &'a WorldName,
    /// The NAME (or personal-world) owner a `Deploy::Owner` proved; `None`
    /// keeps the stored owner (a scoped grantee never rewrites it).
    pub name_owner: Option<&'a Address>,
    pub entity_id: &'a str,
    pub deployer: &'a Address,
    pub deployment_auth_chain: &'a Value,
    pub entity: &'a Value,
    pub parcels: &'a ParcelSet,
    pub size: i64,
    pub replacement: &'a Deploy,
    /// The upstream this publish is forwarded to (`WORLDS_FORWARD_UPSTREAM_URL`):
    /// its `worlds.forward_outbox` row commits with the scene.
    pub forward_to: Option<&'a str>,
}

pub(crate) struct DerivedSceneSettings {
    pub(crate) spawn_coordinates: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) content_rating: Option<String>,
    pub(crate) skybox_time: Option<i32>,
    pub(crate) categories: Option<Vec<String>>,
    pub(crate) single_player: Option<bool>,
    pub(crate) show_in_places: Option<bool>,
    pub(crate) thumbnail_hash: Option<String>,
}

/// What a deployed scene expresses about its world's settings; a value out
/// of policy is "not expressed" rather than a deployment failure.
pub(crate) fn scene_settings_from_entity(entity: &Value) -> DerivedSceneSettings {
    let meta = entity.get("metadata");
    let display = meta.and_then(|m| m.get("display"));
    let wc = meta.and_then(|m| m.get("worldConfiguration"));
    let scene = meta.and_then(|m| m.get("scene"));

    let title = display
        .and_then(|d| d.get("title"))
        .and_then(|v| v.as_str())
        .filter(|t| (TITLE_MIN_LENGTH..=TITLE_MAX_LENGTH).contains(&text_len(t)))
        .map(str::to_string);
    let description = display
        .and_then(|d| d.get("description"))
        .and_then(|v| v.as_str())
        .filter(|d| (DESCRIPTION_MIN_LENGTH..=DESCRIPTION_MAX_LENGTH).contains(&text_len(d)))
        .map(str::to_string);
    let content_rating = meta
        .and_then(|m| m.get("rating"))
        .and_then(|v| v.as_str())
        .filter(|r| VALID_RATINGS.contains(r))
        .map(str::to_string);
    let skybox_time = wc
        .and_then(|c| c.get("skyboxConfig"))
        .and_then(|s| s.get("fixedTime"))
        .and_then(|v| v.as_f64())
        .and_then(storable_skybox_time);
    let categories = meta
        .and_then(|m| m.get("tags"))
        .and_then(|t| t.as_array())
        .filter(|arr| !arr.is_empty() && arr.len() <= MAX_CATEGORIES)
        .and_then(|arr| {
            arr.iter()
                .map(|t| t.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
        });
    let single_player = wc
        .and_then(|c| c.get("fixedAdapter"))
        .map(|v| v.as_str() == Some("offline:offline"));
    let show_in_places = wc
        .and_then(|c| c.get("placesConfig"))
        .and_then(|p| p.get("optOut"))
        .map(|v| !js_truthy(v));
    let spawn_coordinates = scene
        .and_then(|s| s.get("base"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            scene
                .and_then(|s| s.get("parcels"))
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    let thumbnail_hash = display
        .and_then(|d| d.get("navmapThumbnail"))
        .and_then(|v| v.as_str())
        .and_then(|file| {
            entity
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find(|c| c.get("file").and_then(|f| f.as_str()) == Some(file))
                        .and_then(|c| c.get("hash").and_then(|h| h.as_str()))
                        .map(str::to_string)
                })
        });

    DerivedSceneSettings {
        spawn_coordinates,
        title,
        description,
        content_rating,
        skybox_time,
        categories,
        single_player,
        show_in_places,
        thumbnail_hash,
    }
}

/// The canonical parcel a stored scene's downstream identity uses:
/// [`ParcelSet::effective_base`] over the stored parcels.
pub fn effective_base_parcel(entity: &Value, parcels: &[String]) -> Option<String> {
    let set: ParcelSet = parcels
        .iter()
        .filter_map(|p| Parcel::parse(p).ok())
        .collect();
    let declared = entity
        .get("metadata")
        .and_then(|m| m.get("scene"))
        .and_then(|s| s.get("base"))
        .and_then(|b| b.as_str());
    set.effective_base(declared).map(|p| p.canonical())
}

const WORLD_SELECT: &str = r#"SELECT w.name, w.owner, w.access_type, w.access_secret_hash, w.access_nft, w.blocked_since,
              w.spawn_coordinates, w.skybox_time, w.single_player, w.realm_name_override, w.preview_wearable_urns,
              COALESCE((SELECT array_agg(wp.address ORDER BY wp.created_at, wp.address)
                        FROM worlds.world_permissions wp
                        WHERE wp.world_name = w.name AND wp.permission = 'access'), '{}'::text[]) AS access_wallets,
              COALESCE((SELECT array_agg(c.community_id::text ORDER BY c.created_at, c.community_id)
                        FROM worlds.world_access_communities c
                        WHERE c.world_name = w.name), '{}'::text[]) AS access_communities
       FROM worlds.worlds w WHERE w.name = $1"#;

fn world_from_row(r: &PgRow) -> WorldRecord {
    WorldRecord {
        name: r.get("name"),
        owner: r
            .get::<Option<String>, _>("owner")
            .and_then(|o| Address::parse(&o).ok()),
        access: AccessSetting::from_row(r),
        blocked_since: r.get("blocked_since"),
        spawn_coordinates: r.get("spawn_coordinates"),
        skybox_time: r.get("skybox_time"),
        single_player: r.get("single_player"),
        realm_name_override: r.get("realm_name_override"),
        preview_wearable_urns: r.get("preview_wearable_urns"),
    }
}

pub async fn get_world(pool: &PgPool, world: &WorldName) -> Result<Option<WorldRecord>, ApiError> {
    let row = sqlx::query(WORLD_SELECT)
        .bind(world.key())
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(world_from_row))
}

/// Upstream's `isWorldValid`: the world has scenes and is not disabled.
pub async fn world_is_valid(pool: &PgPool, world: &WorldName) -> Result<bool, ApiError> {
    let has_scenes: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM worlds.world_scenes WHERE world_name = $1)",
    )
    .bind(world.key())
    .fetch_one(pool)
    .await?;
    Ok(has_scenes && !is_world_disabled(pool, world).await?)
}

pub async fn ensure_world(
    pool: &PgPool,
    world: &WorldName,
    owner: &Address,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO worlds.worlds (name, owner) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING",
    )
    .bind(world.key())
    .bind(owner)
    .execute(pool)
    .await?;
    Ok(())
}

/// The deployment/streaming grants of a world with their scoping, in the
/// old `GET /permissions` order (the `access` grants render inside the
/// access setting, as the old document did).
pub async fn permission_records(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Vec<PermissionRecord>, ApiError> {
    let rows = sqlx::query(
        r#"SELECT wp.permission, wp.address,
                  count(wpp.parcel) = 0 AS is_world_wide,
                  count(wpp.parcel) AS parcel_count
           FROM worlds.world_permissions wp
           LEFT JOIN worlds.world_permission_parcels wpp
             ON (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address)
           WHERE wp.world_name = $1 AND wp.permission IN ('deployment', 'streaming')
           GROUP BY wp.permission, wp.address
           ORDER BY wp.address, wp.permission"#,
    )
    .bind(world.key())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| PermissionRecord {
            permission: r.get("permission"),
            address: r.get("address"),
            is_world_wide: r.get("is_world_wide"),
            parcel_count: r.get("parcel_count"),
        })
        .collect())
}

/// One transaction: upsert the world (owner from a proven `Deploy::Owner`,
/// an operator disable kept, settings refreshed from the entity when the
/// scene ends up the sole occupant), refuse (400) when an overlapping scene
/// is newer (greater entity timestamp, ties by byte-wise greater entity id), delete
/// what the authorization allows replacing (a
/// leftover overlap under a scoped authorization is a 409 and rolls back),
/// upsert the scene row as a local publish and apply the storage identity
/// rule (`scene_identity`). A world this node only mirrored
/// becomes local here: every mirrored scene goes, overlapping or not, and
/// the world row's `mirror_source` is cleared in the same transaction, so
/// the row says what its scenes are (the mirror refuses to write a world
/// with a local scene from then on, `worlds::mirror`).
pub async fn deploy_scene(
    pool: &PgPool,
    storage: &Storage,
    deployment: SceneDeployment<'_>,
) -> Result<(), ApiError> {
    let SceneDeployment {
        world,
        name_owner,
        entity_id,
        deployer,
        deployment_auth_chain,
        entity,
        parcels,
        size,
        replacement,
        forward_to,
    } = deployment;
    let mut s = scene_settings_from_entity(entity);
    if let Some(hash) = s.thumbnail_hash.take() {
        s.thumbnail_hash = storable_thumbnail_hash(storage, &hash).await;
    }
    let parcels = parcels.canonical();

    let mut tx = pool.begin().await?;

    let (is_insert, disabled): (bool, bool) = sqlx::query_as(
        r#"INSERT INTO worlds.worlds AS w (
               name, owner, blocked_since, spawn_coordinates,
               title, description, content_rating, skybox_time, categories,
               single_player, show_in_places, thumbnail_hash, updated_at
           )
           VALUES ($1, COALESCE($2, $12), NULL, $3, $4, $5, $6, $7, $8::text[],
                   COALESCE($9, false), COALESCE($10, true), $11, timezone('UTC', now()))
           ON CONFLICT (name) DO UPDATE SET
             owner = COALESCE($2, w.owner),
             mirror_source = NULL,
             spawn_coordinates = COALESCE(w.spawn_coordinates, EXCLUDED.spawn_coordinates),
             updated_at = timezone('UTC', now())
           RETURNING (xmax = 0), blocked_since IS NOT NULL"#,
    )
    .bind(world.key())
    .bind(name_owner)
    .bind(&s.spawn_coordinates)
    .bind(&s.title)
    .bind(&s.description)
    .bind(&s.content_rating)
    .bind(s.skybox_time)
    .bind(&s.categories)
    .bind(s.single_player)
    .bind(s.show_in_places)
    .bind(&s.thumbnail_hash)
    .bind(deployer)
    .fetch_one(&mut *tx)
    .await?;

    // Under the world row's lock, so a disable racing this publish wins.
    if disabled {
        return Err(world_disabled(world));
    }

    // Under the world row's lock, so a concurrent publish cannot slip past.
    // A stored timestamp counts no later than the row's write (a future-dated
    // entity cannot lock the parcels); a non-numeric one never blocks.
    let newer_deployed: bool = sqlx::query_scalar(
        r#"WITH stored AS (
             SELECT entity_id,
                    CASE WHEN jsonb_typeof(entity->'timestamp') = 'number'
                         THEN LEAST((entity->>'timestamp')::numeric,
                                    extract(epoch FROM updated_at) * 1000)
                    END AS ts
             FROM worlds.world_scenes
             WHERE world_name = $1 AND parcels && $2::text[] AND entity_id <> $3)
           SELECT EXISTS (SELECT 1 FROM stored
             WHERE ts > $4::bigint OR (ts = $4::bigint AND entity_id COLLATE "C" > $3))"#,
    )
    .bind(world.key())
    .bind(&parcels)
    .bind(entity_id)
    .bind(entity_timestamp(entity)?)
    .fetch_one(&mut *tx)
    .await?;
    if newer_deployed {
        return Err(ApiError::bad_request(
            "Deployment failed: a newer scene is already deployed on one or more of these parcels.",
        ));
    }

    if !is_insert {
        let sole_occupant: bool = sqlx::query_scalar(
            "SELECT count(*) FILTER (WHERE NOT (parcels && $2::text[])) = 0 FROM worlds.world_scenes WHERE world_name = $1",
        )
        .bind(world.key())
        .bind(&parcels)
        .fetch_one(&mut *tx)
        .await?;

        if sole_occupant {
            sqlx::query(
                r#"UPDATE worlds.worlds SET
                     title = COALESCE($2, title),
                     description = COALESCE($3, description),
                     content_rating = COALESCE($4, content_rating),
                     skybox_time = COALESCE($5, skybox_time),
                     categories = COALESCE($6::text[], categories),
                     single_player = COALESCE($7, single_player),
                     show_in_places = COALESCE($8, show_in_places),
                     thumbnail_hash = COALESCE($9, thumbnail_hash),
                     settings_version = settings_version + 1,
                     updated_at = timezone('UTC', now())
                   WHERE name = $1
                     AND (title, description, content_rating, skybox_time, categories,
                          single_player, show_in_places, thumbnail_hash)
                         IS DISTINCT FROM
                         (COALESCE($2, title), COALESCE($3, description),
                          COALESCE($4, content_rating), COALESCE($5, skybox_time),
                          COALESCE($6::text[], categories), COALESCE($7, single_player),
                          COALESCE($8, show_in_places), COALESCE($9, thumbnail_hash))"#,
            )
            .bind(world.key())
            .bind(&s.title)
            .bind(&s.description)
            .bind(&s.content_rating)
            .bind(s.skybox_time)
            .bind(&s.categories)
            .bind(s.single_player)
            .bind(s.show_in_places)
            .bind(&s.thumbnail_hash)
            .execute(&mut *tx)
            .await?;
        }
    }

    match replacement {
        Deploy::Owner => {
            sqlx::query(
                "DELETE FROM worlds.world_scenes WHERE world_name = $1 AND parcels && $2::text[]",
            )
            .bind(world.key())
            .bind(&parcels)
            .execute(&mut *tx)
            .await?;
        }
        Deploy::Scoped(ids) => {
            let ids: Vec<&str> = ids.iter().map(SceneId::as_str).collect();
            sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND parcels && $2::text[] AND entity_id = ANY($3::text[])")
                .bind(world.key())
                .bind(&parcels)
                .bind(&ids)
                .execute(&mut *tx)
                .await?;
            let leftover: Option<String> =
                sqlx::query_scalar("SELECT entity_id FROM worlds.world_scenes WHERE world_name = $1 AND parcels && $2::text[] LIMIT 1")
                    .bind(world.key())
                    .bind(&parcels)
                    .fetch_optional(&mut *tx)
                    .await?;
            if leftover.is_some() {
                return Err(ApiError::conflict(format!(
                    "Scene replacement authorization changed while deploying to world \"{}\". Please retry.",
                    world.key()
                )));
            }
        }
    }

    // The mirror never refreshes a world with a local scene, so a mirrored
    // scene left beside this one would stay stale for good.
    sqlx::query(
        "DELETE FROM worlds.world_scenes WHERE world_name = $1 AND mirror_source IS NOT NULL",
    )
    .bind(world.key())
    .execute(&mut *tx)
    .await?;

    let auth_chain_text = serde_json::to_string(deployment_auth_chain)
        .map_err(|e| ApiError::internal(format!("auth chain: {e}")))?;
    sqlx::query(
        r#"INSERT INTO worlds.world_scenes
             (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source)
           VALUES ($1, $2, $3::json, $4, $5, $6::text[], $7, NULL)
           ON CONFLICT (world_name, entity_id) DO UPDATE
             SET deployment_auth_chain = EXCLUDED.deployment_auth_chain,
                 entity = EXCLUDED.entity,
                 deployer = EXCLUDED.deployer,
                 parcels = EXCLUDED.parcels,
                 size = EXCLUDED.size,
                 mirror_source = NULL,
                 updated_at = timezone('UTC', now())"#,
    )
    .bind(world.key())
    .bind(entity_id)
    .bind(auth_chain_text)
    .bind(entity)
    .bind(deployer)
    .bind(&parcels)
    .bind(size)
    .execute(&mut *tx)
    .await?;
    // The scene's storage identity, and the retired ones' (replaced, mirrored).
    crate::explore::places::scene_identity::assign(&mut tx, world.key(), entity_id, &parcels)
        .await?;
    crate::explore::places::scene_identity::retire_orphans(&mut tx, &[world.key().to_string()])
        .await?;

    if let Some(target) = forward_to {
        // A re-publish of the same entity replaces its record: the new id
        // keeps a send still in flight for the old one from finishing it.
        // The world row's lock above serializes publishes of one world.
        sqlx::query("DELETE FROM worlds.forward_outbox WHERE entity_id = $1")
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO worlds.forward_outbox (entity_id, world_name, target) VALUES ($1, $2, $3)",
        )
        .bind(entity_id)
        .bind(world.key())
        .bind(target)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Refused publish to a world the operator disabled (`/admin/worlds/{w}/disable`).
pub fn world_disabled(world: &WorldName) -> ApiError {
    ApiError::forbidden(format!(
        "Deployment failed: World \"{}\" is disabled and does not accept deployments.",
        world.display()
    ))
}

pub async fn is_world_disabled(pool: &PgPool, world: &WorldName) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM worlds.worlds WHERE name = $1 AND blocked_since IS NOT NULL)",
    )
    .bind(world.key())
    .fetch_one(pool)
    .await?)
}

/// The entity's integer `timestamp`; the deploy route already required it.
fn entity_timestamp(entity: &Value) -> Result<i64, ApiError> {
    entity
        .get("timestamp")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::bad_request("The entity is missing a valid timestamp"))
}

async fn delete_scenes_locked(
    pool: &PgPool,
    world: &WorldName,
    sql: &'static str,
    bind: impl FnOnce(
        sqlx::query::Query<'static, sqlx::Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'static, sqlx::Postgres, sqlx::postgres::PgArguments>,
) -> Result<u64, ApiError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT name FROM worlds.worlds WHERE name = $1 FOR UPDATE")
        .bind(world.key())
        .execute(&mut *tx)
        .await?;
    let affected = bind(sqlx::query(sql))
        .execute(&mut *tx)
        .await?
        .rows_affected();
    crate::explore::places::scene_identity::retire_orphans(&mut tx, &[world.key().to_string()])
        .await?;
    tx.commit().await?;
    Ok(affected)
}

/// Undeploy every scene standing on `parcel`. `authorized` scopes the delete
/// to those exact identities (a parcel-scoped deployer cannot remove a scene
/// reaching into parcels it was never granted); `None` (the owner) is
/// unrestricted.
pub async fn undeploy_scene(
    pool: &PgPool,
    world: &WorldName,
    parcel: Parcel,
    authorized: Option<&[SceneId]>,
) -> Result<u64, ApiError> {
    let key = world.key().to_string();
    let parcel = parcel.canonical();
    match authorized {
        Some(ids) => {
            let ids: Vec<String> = ids.iter().map(|s| s.as_str().to_string()).collect();
            delete_scenes_locked(
                pool,
                world,
                "DELETE FROM worlds.world_scenes WHERE world_name = $1 AND $2 = ANY(parcels) AND entity_id = ANY($3::text[])",
                move |q| q.bind(key).bind(parcel).bind(ids),
            )
            .await
        }
        None => {
            delete_scenes_locked(
                pool,
                world,
                "DELETE FROM worlds.world_scenes WHERE world_name = $1 AND $2 = ANY(parcels)",
                move |q| q.bind(key).bind(parcel),
            )
            .await
        }
    }
}

pub async fn undeploy_world(pool: &PgPool, world: &WorldName) -> Result<u64, ApiError> {
    let key = world.key().to_string();
    delete_scenes_locked(
        pool,
        world,
        "DELETE FROM worlds.world_scenes WHERE world_name = $1",
        move |q| q.bind(key),
    )
    .await
}

/// `(entity_id, parcels, effective base)` of every scene, by entity id.
pub async fn list_scenes(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Vec<(String, Vec<String>, Option<String>)>, ApiError> {
    let rows = sqlx::query("SELECT entity_id, parcels, entity FROM worlds.world_scenes WHERE world_name = $1 ORDER BY entity_id")
        .bind(world.key())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let parcels: Vec<String> = r.get("parcels");
            let entity: Value = r.get("entity");
            let base = effective_base_parcel(&entity, &parcels);
            (r.get("entity_id"), parcels, base)
        })
        .collect())
}

pub async fn scene_base_parcel(
    pool: &PgPool,
    world: &WorldName,
    scene_id: &str,
) -> Result<Option<String>, ApiError> {
    let row = sqlx::query(
        "SELECT entity, parcels FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = $2",
    )
    .bind(world.key())
    .bind(scene_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| {
        let parcels: Vec<String> = r.get("parcels");
        let entity: Value = r.get("entity");
        effective_base_parcel(&entity, &parcels)
    }))
}

/// `POST /entities/active`: the newest scene of each named world, with the
/// entity id and the world owner folded into the entity as the old route did.
pub async fn entities_for_worlds(
    pool: &PgPool,
    worlds: &[WorldName],
) -> Result<Vec<Value>, ApiError> {
    if worlds.is_empty() {
        return Ok(Vec::new());
    }
    let keys: Vec<&str> = worlds.iter().map(WorldName::key).collect();
    let rows = sqlx::query(
        r#"SELECT DISTINCT ON (ws.world_name) ws.entity_id, ws.entity, w.owner
           FROM worlds.world_scenes ws
           JOIN worlds.worlds w ON w.name = ws.world_name
           WHERE ws.world_name = ANY($1::text[])
           ORDER BY ws.world_name, ws.created_at DESC"#,
    )
    .bind(&keys)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let entity_id: String = r.get("entity_id");
            let owner: Option<String> = r.get("owner");
            let mut entity: Value = r.get("entity");
            if let Some(obj) = entity.as_object_mut() {
                obj.insert("id".into(), Value::String(entity_id));
                let metadata = obj
                    .entry("metadata")
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                if let (Some(meta_obj), Some(owner)) = (metadata.as_object_mut(), owner) {
                    meta_obj.insert("owner".into(), Value::String(owner));
                }
            }
            entity
        })
        .collect())
}

/// `POST /permissions/{deployment|streaming}` in one statement: ensure the
/// worlds row (signer as owner), drop the addresses no longer listed, add the
/// new ones (existing rows keep their scoping).
pub async fn replace_world_wide_permission(
    pool: &PgPool,
    world: &WorldName,
    owner: &Address,
    permission: Permission,
    addresses: &[Address],
) -> Result<(), ApiError> {
    sqlx::query(
        r#"WITH ensured AS (
             INSERT INTO worlds.worlds (name, owner) VALUES ($1, $4) ON CONFLICT (name) DO NOTHING
           ), removed AS (
             DELETE FROM worlds.world_permissions
             WHERE world_name = $1 AND permission = $2 AND NOT (address = ANY($3::text[]))
           ), added AS (
             INSERT INTO worlds.world_permissions (world_name, permission, address)
             SELECT $1, $2, addr FROM unnest($3::text[]) AS addr
             ON CONFLICT (world_name, permission, address) DO NOTHING
           )
           SELECT 1"#,
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(addresses)
    .bind(owner)
    .execute(pool)
    .await?;
    Ok(())
}

/// Grants world-wide `permission` to `addresses`: new rows are inserted and any
/// parcel scoping the addresses had is cleared. `ensure_owner` creates the
/// worlds row first. Returns the newly added addresses.
pub async fn grant_addresses_world_wide_permission(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    addresses: &[Address],
    ensure_owner: Option<&Address>,
) -> Result<Vec<String>, ApiError> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let inserted = sqlx::query(
        r#"WITH ensured AS (
             INSERT INTO worlds.worlds (name, owner)
             SELECT $1, $4 WHERE $4::text IS NOT NULL
             ON CONFLICT (name) DO NOTHING
           ), added AS (
             INSERT INTO worlds.world_permissions (world_name, permission, address)
             SELECT $1, $2, addr FROM unnest($3::text[]) AS addr
             ON CONFLICT (world_name, permission, address) DO NOTHING
             RETURNING address
           ), cleared AS (
             DELETE FROM worlds.world_permission_parcels
             WHERE world_name = $1 AND permission = $2 AND address = ANY($3::text[])
           )
           SELECT address FROM added"#,
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(addresses)
    .bind(ensure_owner)
    .fetch_all(pool)
    .await?;
    Ok(inserted.into_iter().map(|r| r.get("address")).collect())
}

pub async fn remove_addresses_permission(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    addresses: &[Address],
) -> Result<Vec<String>, ApiError> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "DELETE FROM worlds.world_permissions WHERE world_name = $1 AND permission = $2 AND address = ANY($3::text[]) RETURNING address",
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(addresses)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.get("address")).collect())
}

/// Adds canonical parcels under a grant, creating it when absent; `true` when
/// the grant was created by this call.
pub async fn add_parcels_to_permission(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    address: &Address,
    parcels: &ParcelSet,
) -> Result<bool, ApiError> {
    let created: bool = sqlx::query_scalar(
        r#"WITH perm AS (
             INSERT INTO worlds.world_permissions (world_name, permission, address)
             VALUES ($1, $2, $3)
             ON CONFLICT (world_name, permission, address) DO UPDATE SET updated_at = timezone('UTC', now())
             RETURNING (xmax = 0) AS created
           ), added AS (
             INSERT INTO worlds.world_permission_parcels (world_name, permission, address, parcel)
             SELECT $1, $2, $3, parcel FROM unnest($4::text[]) AS parcel
             ON CONFLICT DO NOTHING
           )
           SELECT created FROM perm"#,
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(address)
    .bind(parcels.canonical())
    .fetch_one(pool)
    .await?;
    Ok(created)
}

/// `false` when the address holds no such permission (the caller's 400).
/// Removing the last parcel drops the grant: zero parcels would read as
/// world-wide (deliberate divergence from upstream, which keeps the row).
pub async fn remove_parcels_from_permission(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    address: &Address,
    parcels: &ParcelSet,
) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await?;
    let found = sqlx::query(
        "SELECT 1 FROM worlds.world_permissions
         WHERE world_name = $1 AND permission = $2 AND address = $3 FOR UPDATE",
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(address)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !found {
        return Ok(false);
    }
    let removed = sqlx::query(
        "DELETE FROM worlds.world_permission_parcels
         WHERE world_name = $1 AND permission = $2 AND address = $3 AND parcel = ANY($4::text[])",
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(address)
    .bind(parcels.canonical())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let dropped = removed > 0
        && sqlx::query(
            "DELETE FROM worlds.world_permissions wp
             WHERE world_name = $1 AND permission = $2 AND address = $3
               AND NOT EXISTS (SELECT 1 FROM worlds.world_permission_parcels wpp
                 WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address))",
        )
        .bind(world.key())
        .bind(permission.as_str())
        .bind(address)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
    if !dropped {
        sqlx::query(
            "UPDATE worlds.world_permissions SET updated_at = timezone('UTC', now())
             WHERE world_name = $1 AND permission = $2 AND address = $3",
        )
        .bind(world.key())
        .bind(permission.as_str())
        .bind(address)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// `None` when the address holds no such permission, otherwise the filtered
/// total and the page (bbox inclusive, corners in any order).
pub async fn parcels_for_permission(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    address: &Address,
    limit: i64,
    offset: i64,
    bbox: Option<(i32, i32, i32, i32)>,
) -> Result<Option<(i64, Vec<String>)>, ApiError> {
    let (has_bbox, min_x, max_x, min_y, max_y) = match bbox {
        Some((x1, y1, x2, y2)) => (true, x1.min(x2), x1.max(x2), y1.min(y2), y1.max(y2)),
        None => (false, 0, 0, 0, 0),
    };
    let rows = sqlx::query(
        r#"SELECT c.total, p.parcel
           FROM worlds.world_permissions wp
           CROSS JOIN LATERAL (
             SELECT count(*) AS total FROM worlds.world_permission_parcels wpp
             WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address)
               AND ($4::bool = false OR (
                  split_part(wpp.parcel, ',', 1)::int BETWEEN $5 AND $6
                  AND split_part(wpp.parcel, ',', 2)::int BETWEEN $7 AND $8))
           ) c
           LEFT JOIN LATERAL (
             SELECT wpp.parcel FROM worlds.world_permission_parcels wpp
             WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address)
               AND ($4::bool = false OR (
                  split_part(wpp.parcel, ',', 1)::int BETWEEN $5 AND $6
                  AND split_part(wpp.parcel, ',', 2)::int BETWEEN $7 AND $8))
             ORDER BY wpp.parcel
             LIMIT $9 OFFSET $10
           ) p ON true
           WHERE wp.world_name = $1 AND wp.permission = $2 AND wp.address = $3
           ORDER BY p.parcel"#,
    )
    .bind(world.key())
    .bind(permission.as_str())
    .bind(address)
    .bind(has_bbox)
    .bind(min_x)
    .bind(max_x)
    .bind(min_y)
    .bind(max_y)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let total: i64 = first.get("total");
    let parcels = rows
        .iter()
        .filter_map(|r| r.get::<Option<String>, _>("parcel"))
        .collect();
    Ok(Some((total, parcels)))
}

/// The grantees whose `permission` is world-wide or touches one of `parcels`.
pub async fn addresses_for_parcel_permission(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    parcels: &ParcelSet,
    limit: i64,
    offset: i64,
) -> Result<(i64, Vec<String>), ApiError> {
    const WHERE: &str = r#"FROM worlds.world_permissions wp
           WHERE wp.world_name = $1 AND wp.permission = $2
             AND (NOT EXISTS (SELECT 1 FROM worlds.world_permission_parcels wpp
                              WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address))
                  OR EXISTS (SELECT 1 FROM worlds.world_permission_parcels wpp
                             WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address)
                               AND wpp.parcel = ANY($3::text[])))"#;
    let canon = parcels.canonical();
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT wp.address, count(*) OVER () AS total {WHERE} ORDER BY wp.address LIMIT $4 OFFSET $5"
    )))
    .bind(world.key())
    .bind(permission.as_str())
    .bind(&canon)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total: i64 = match rows.first() {
        Some(r) => r.get("total"),
        None if offset > 0 => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {WHERE}")))
                .bind(world.key())
                .bind(permission.as_str())
                .bind(&canon)
                .fetch_one(pool)
                .await?
        }
        None => 0,
    };
    Ok((total, rows.into_iter().map(|r| r.get("address")).collect()))
}

/// Ensures the worlds row and replaces the access setting in one
/// transaction: the typed columns, then the `access` grants and the
/// communities (deleted on every write, whatever the previous type, so a
/// list never resurfaces after a round trip through another type). A fresh
/// row lands at settings_version 1.
pub async fn store_access_for_owner(
    pool: &PgPool,
    world: &WorldName,
    owner: &Address,
    access: &AccessSetting,
) -> Result<(), ApiError> {
    let (secret_hash, nft, wallets, communities): (
        Option<&str>,
        Option<&str>,
        Vec<Address>,
        Vec<Uuid>,
    ) = match access {
        AccessSetting::Unrestricted => (None, None, Vec::new(), Vec::new()),
        AccessSetting::SharedSecret { secret_hash } => {
            (Some(secret_hash), None, Vec::new(), Vec::new())
        }
        AccessSetting::NftOwnership { nft } => (None, Some(nft), Vec::new(), Vec::new()),
        AccessSetting::AllowList {
            wallets,
            communities,
        } => {
            let mut seen = HashSet::new();
            let wallets = wallets
                .iter()
                .filter_map(|w| Address::parse(w).ok())
                .filter(|w| seen.insert(w.clone()))
                .collect();
            let mut seen = HashSet::new();
            let communities = communities
                .iter()
                .filter_map(|c| Uuid::parse_str(c.trim()).ok())
                .filter(|c| seen.insert(*c))
                .collect();
            (None, None, wallets, communities)
        }
    };
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO worlds.worlds AS w (name, owner, access_type, access_secret_hash, access_nft, settings_version)
           VALUES ($1, $2, $3, $4, $5, 1)
           ON CONFLICT (name) DO UPDATE SET
             access_type = EXCLUDED.access_type,
             access_secret_hash = EXCLUDED.access_secret_hash,
             access_nft = EXCLUDED.access_nft,
             settings_version = w.settings_version + 1,
             updated_at = timezone('UTC', now())"#,
    )
    .bind(world.key())
    .bind(owner)
    .bind(access.access_type().as_str())
    .bind(secret_hash)
    .bind(nft)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM worlds.world_permissions WHERE world_name = $1 AND permission = 'access'",
    )
    .bind(world.key())
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM worlds.world_access_communities WHERE world_name = $1")
        .bind(world.key())
        .execute(&mut *tx)
        .await?;
    if !wallets.is_empty() {
        sqlx::query(
            "INSERT INTO worlds.world_permissions (world_name, permission, address) SELECT $1, 'access', addr FROM unnest($2::text[]) AS addr",
        )
        .bind(world.key())
        .bind(&wallets)
        .execute(&mut *tx)
        .await?;
    }
    if !communities.is_empty() {
        sqlx::query("INSERT INTO worlds.world_access_communities (world_name, community_id) SELECT $1, c FROM unnest($2::uuid[]) AS c")
            .bind(world.key())
            .bind(&communities)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// One allow-list edit under the world row's lock: refused unless the row
/// exists and is an allow-list, capped at 1000 wallets / 50 communities; the
/// settings version bumps on every applied edit, as the old upsert did.
pub async fn modify_allow_list_access(
    pool: &PgPool,
    world: &WorldName,
    edit: AllowListEdit<'_>,
) -> Result<AllowListEditOutcome, ApiError> {
    let mut tx = pool.begin().await?;
    let access_type: Option<String> =
        sqlx::query_scalar("SELECT access_type FROM worlds.worlds WHERE name = $1 FOR UPDATE")
            .bind(world.key())
            .fetch_optional(&mut *tx)
            .await?;
    if access_type.as_deref() != Some(AccessType::AllowList.as_str()) {
        return Ok(AllowListEditOutcome::NotAllowList);
    }
    match edit {
        AllowListEdit::AddWallet(wallet) => {
            let (present, count): (bool, i64) = sqlx::query_as(
                r#"SELECT EXISTS (SELECT 1 FROM worlds.world_permissions WHERE world_name = $1 AND permission = 'access' AND address = $2),
                          (SELECT count(*) FROM worlds.world_permissions WHERE world_name = $1 AND permission = 'access')"#,
            )
            .bind(world.key())
            .bind(wallet)
            .fetch_one(&mut *tx)
            .await?;
            if !present {
                if count >= MAX_ACCESS_WALLETS {
                    return Ok(AllowListEditOutcome::CapExceeded);
                }
                sqlx::query("INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, 'access', $2)")
                    .bind(world.key())
                    .bind(wallet)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        AllowListEdit::RemoveWallet(wallet) => {
            sqlx::query("DELETE FROM worlds.world_permissions WHERE world_name = $1 AND permission = 'access' AND address = $2")
                .bind(world.key())
                .bind(wallet)
                .execute(&mut *tx)
                .await?;
        }
        AllowListEdit::AddCommunity(id) => {
            let (present, count): (bool, i64) = sqlx::query_as(
                r#"SELECT EXISTS (SELECT 1 FROM worlds.world_access_communities WHERE world_name = $1 AND community_id = $2),
                          (SELECT count(*) FROM worlds.world_access_communities WHERE world_name = $1)"#,
            )
            .bind(world.key())
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if !present {
                if count >= MAX_ACCESS_COMMUNITIES {
                    return Ok(AllowListEditOutcome::CapExceeded);
                }
                sqlx::query("INSERT INTO worlds.world_access_communities (world_name, community_id) VALUES ($1, $2)")
                    .bind(world.key())
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        AllowListEdit::RemoveCommunity(id) => {
            sqlx::query("DELETE FROM worlds.world_access_communities WHERE world_name = $1 AND community_id = $2")
                .bind(world.key())
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query("UPDATE worlds.worlds SET settings_version = settings_version + 1, updated_at = timezone('UTC', now()) WHERE name = $1")
        .bind(world.key())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(AllowListEditOutcome::Applied)
}

const SETTINGS_COLUMNS: &str = "title, description, content_rating, spawn_coordinates, skybox_time, categories, single_player, \
     show_in_places, thumbnail_hash, access_type, realm_name_override, preview_wearable_urns, settings_version";

fn settings_from_row(r: &PgRow) -> WorldSettingsRow {
    WorldSettingsRow {
        title: r.get("title"),
        description: r.get("description"),
        content_rating: r.get("content_rating"),
        spawn_coordinates: r.get("spawn_coordinates"),
        skybox_time: r.get("skybox_time"),
        categories: r.get("categories"),
        single_player: r.get("single_player"),
        show_in_places: r.get("show_in_places"),
        thumbnail_hash: r.get("thumbnail_hash"),
        access_type: r.get("access_type"),
        realm_name_override: r.get("realm_name_override"),
        preview_wearable_urns: r.get("preview_wearable_urns"),
        settings_version: r.get("settings_version"),
    }
}

pub async fn get_world_settings(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Option<WorldSettingsRow>, ApiError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SETTINGS_COLUMNS} FROM worlds.worlds WHERE name = $1"
    )))
    .bind(world.key())
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(settings_from_row))
}

/// One locked transaction: the row (`FOR UPDATE`) plus, when a spawn is being
/// set, the world's bounding rectangle; the upsert, bumping the version only
/// when a versioned settings column is in the patch. Returns the row and the
/// previous spawn.
pub async fn update_world_settings(
    pool: &PgPool,
    world: &WorldName,
    owner: &Address,
    input: &WorldSettingsUpdate,
) -> Result<(WorldSettingsRow, Option<String>), ApiError> {
    let mut tx = pool.begin().await?;
    let locked = sqlx::query(
        r#"SELECT w.spawn_coordinates, br.min_x, br.max_x, br.min_y, br.max_y
           FROM worlds.worlds w
           LEFT JOIN LATERAL (
             SELECT min(split_part(p, ',', 1)::int) AS min_x,
                    max(split_part(p, ',', 1)::int) AS max_x,
                    min(split_part(p, ',', 2)::int) AS min_y,
                    max(split_part(p, ',', 2)::int) AS max_y
             FROM worlds.world_scenes ws, unnest(ws.parcels) AS p
             WHERE $2::bool AND ws.world_name = w.name
           ) br ON true
           WHERE w.name = $1
           FOR UPDATE OF w"#,
    )
    .bind(world.key())
    .bind(input.spawn_coordinates.is_some())
    .fetch_optional(&mut *tx)
    .await?;
    let old_spawn: Option<String> = locked
        .as_ref()
        .and_then(|r| r.get::<Option<String>, _>("spawn_coordinates"));

    if let Some(spawn) = input.spawn_coordinates.as_deref() {
        let rect = locked.as_ref().and_then(|r| {
            let col = |c: &str| r.get::<Option<i32>, _>(c);
            Some((col("min_x")?, col("max_x")?, col("min_y")?, col("max_y")?))
        });
        let (min_x, max_x, min_y, max_y) = rect.ok_or_else(|| {
            ApiError::bad_request(format!(
                "Invalid spawnCoordinates \"{spawn}\". The world has no deployed scenes."
            ))
        })?;
        let within = Parcel::parse(spawn)
            .map(|p| (min_x..=max_x).contains(&p.x) && (min_y..=max_y).contains(&p.y))
            .unwrap_or(false);
        if !within {
            return Err(ApiError::bad_request(format!(
                "Invalid spawnCoordinates \"{spawn}\". It must be within the world shape rectangle: ({min_x},{min_y}) to ({max_x},{max_y})."
            )));
        }
    }

    let categories: Option<Vec<String>> = input
        .categories_provided
        .then(|| input.categories.clone().unwrap_or_default());
    let has_settings_patch = input.title.is_some()
        || input.description.is_some()
        || input.content_rating.is_some()
        || input.skybox_time_provided
        || input.categories_provided
        || input.single_player.is_some()
        || input.show_in_places.is_some()
        || input.thumbnail_hash.is_some()
        || input.realm_name_override_provided
        || input.preview_wearable_urns_provided;

    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"INSERT INTO worlds.worlds AS w (
               name, owner, title, description, content_rating, spawn_coordinates,
               skybox_time, categories, single_player, show_in_places, thumbnail_hash,
               realm_name_override, preview_wearable_urns
           )
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8::text[], COALESCE($9, false), COALESCE($10, true), $11, $14, $16::text[])
           ON CONFLICT (name) DO UPDATE SET
             title = COALESCE(EXCLUDED.title, w.title),
             description = COALESCE(EXCLUDED.description, w.description),
             content_rating = COALESCE(EXCLUDED.content_rating, w.content_rating),
             spawn_coordinates = COALESCE(EXCLUDED.spawn_coordinates, w.spawn_coordinates),
             skybox_time = CASE WHEN $12::boolean THEN EXCLUDED.skybox_time
                                ELSE COALESCE(EXCLUDED.skybox_time, w.skybox_time) END,
             categories = COALESCE(EXCLUDED.categories, w.categories),
             single_player = COALESCE($9, w.single_player),
             show_in_places = COALESCE($10, w.show_in_places),
             thumbnail_hash = COALESCE(EXCLUDED.thumbnail_hash, w.thumbnail_hash),
             realm_name_override = CASE WHEN $15::boolean THEN EXCLUDED.realm_name_override
                                        ELSE w.realm_name_override END,
             preview_wearable_urns = CASE WHEN $17::boolean THEN EXCLUDED.preview_wearable_urns
                                          ELSE w.preview_wearable_urns END,
             settings_version = CASE WHEN $13::boolean THEN w.settings_version + 1
                                     ELSE w.settings_version END,
             updated_at = timezone('UTC', now())
           RETURNING {SETTINGS_COLUMNS}"#
    )))
    .bind(world.key())
    .bind(owner)
    .bind(&input.title)
    .bind(&input.description)
    .bind(&input.content_rating)
    .bind(&input.spawn_coordinates)
    .bind(input.skybox_time)
    .bind(&categories)
    .bind(input.single_player)
    .bind(input.show_in_places)
    .bind(&input.thumbnail_hash)
    .bind(input.skybox_time_provided)
    .bind(has_settings_patch)
    .bind(&input.realm_name_override)
    .bind(input.realm_name_override_provided)
    .bind(&input.preview_wearable_urns)
    .bind(input.preview_wearable_urns_provided)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((settings_from_row(&row), old_spawn))
}

pub async fn world_manifest(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Option<WorldManifest>, ApiError> {
    const PARCELS_LIMIT: i64 = 500;
    let (total, parcels, spawn_coordinates): (i64, Vec<String>, Option<String>) = sqlx::query_as(
        r#"WITH parcels AS MATERIALIZED (
               SELECT DISTINCT p AS parcel
               FROM worlds.world_scenes ws, unnest(ws.parcels) AS p
               WHERE ws.world_name = $1 AND p IS NOT NULL
           )
           SELECT (SELECT count(*) FROM parcels),
                  ARRAY(SELECT parcel FROM parcels
                        ORDER BY split_part(parcel, ',', 1)::int,
                                 split_part(parcel, ',', 2)::int LIMIT $2),
                  (SELECT spawn_coordinates FROM worlds.worlds WHERE name = $1 LIMIT 1)"#,
    )
    .bind(world.key())
    .bind(PARCELS_LIMIT)
    .fetch_one(pool)
    .await?;
    if total == 0 {
        return Ok(None);
    }
    Ok(Some(WorldManifest {
        parcels,
        spawn_coordinates,
        total,
    }))
}

pub async fn list_worlds_public(
    pool: &PgPool,
    filters: &WorldsListFilters,
    options: &WorldsListOptions,
) -> Result<(Vec<WorldInfoRow>, i64), ApiError> {
    let base_from = r#"
        FROM worlds.worlds w
        LEFT JOIN (
            SELECT ws.world_name,
                   count(DISTINCT ws.entity_id) AS deployed_scenes,
                   max(ws.created_at) AS last_deployed_at,
                   min(CASE WHEN p ~ '^-?[0-9]+,-?[0-9]+$' THEN split_part(p, ',', 1)::int END) AS min_x,
                   max(CASE WHEN p ~ '^-?[0-9]+,-?[0-9]+$' THEN split_part(p, ',', 1)::int END) AS max_x,
                   min(CASE WHEN p ~ '^-?[0-9]+,-?[0-9]+$' THEN split_part(p, ',', 2)::int END) AS min_y,
                   max(CASE WHEN p ~ '^-?[0-9]+,-?[0-9]+$' THEN split_part(p, ',', 2)::int END) AS max_y
            FROM worlds.world_scenes ws, unnest(ws.parcels) AS p
            GROUP BY ws.world_name
        ) ss ON ss.world_name = w.name
        LEFT JOIN worlds.blocked_wallets b ON w.owner = b.wallet
        WHERE ($1::text IS NULL
                OR w.owner = $1
                OR EXISTS (SELECT 1 FROM worlds.world_permissions wp
                            WHERE wp.world_name = w.name AND wp.address = $1 AND wp.permission = 'deployment'))
          AND ($2::bool IS NULL OR (COALESCE(ss.deployed_scenes, 0) > 0) = $2)
          AND ($3::text IS NULL
                OR w.name ILIKE '%' || $3 || '%'
                OR w.title ILIKE '%' || $3 || '%'
                OR w.description ILIKE '%' || $3 || '%')
    "#;
    let dir = match options.order_direction {
        OrderDirection::Desc => "DESC",
        OrderDirection::Asc => "ASC",
    };
    let order_clause = match options.order_by {
        WorldsOrderBy::LastDeployedAt => format!(
            "ORDER BY ss.last_deployed_at IS NULL ASC, ss.last_deployed_at {dir}, w.name ASC"
        ),
        WorldsOrderBy::Name => format!("ORDER BY w.name {dir}"),
    };
    let main_sql = format!(
        r#"SELECT w.name, w.owner, w.title, w.description, w.content_rating,
                  w.spawn_coordinates, w.skybox_time, w.categories, w.single_player, w.show_in_places,
                  w.thumbnail_hash, ss.last_deployed_at, ss.min_x, ss.max_x, ss.min_y, ss.max_y,
                  b.created_at AS blocked_since,
                  COALESCE(ss.deployed_scenes, 0) AS deployed_scenes,
                  count(*) OVER () AS total
           {base_from}
           {order_clause}
           LIMIT $4 OFFSET $5"#
    );
    let deployer = filters
        .authorized_deployer
        .as_deref()
        .map(str::to_lowercase);
    let rows = sqlx::query(sqlx::AssertSqlSafe(main_sql))
        .bind(&deployer)
        .bind(filters.has_deployed_scenes)
        .bind(&filters.search)
        .bind(options.limit)
        .bind(options.offset)
        .fetch_all(pool)
        .await?;
    let total: i64 = match rows.first() {
        Some(r) => r.get("total"),
        None if options.offset > 0 => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) AS total {base_from}"
            )))
            .bind(&deployer)
            .bind(filters.has_deployed_scenes)
            .bind(&filters.search)
            .fetch_one(pool)
            .await?
        }
        None => 0,
    };
    let worlds = rows
        .into_iter()
        .map(|r| WorldInfoRow {
            name: r.get("name"),
            owner: r.get("owner"),
            title: r.get("title"),
            description: r.get("description"),
            content_rating: r.get("content_rating"),
            spawn_coordinates: r.get("spawn_coordinates"),
            skybox_time: r.get("skybox_time"),
            categories: r.get("categories"),
            single_player: r.get("single_player"),
            show_in_places: r.get("show_in_places"),
            thumbnail_hash: r.get("thumbnail_hash"),
            last_deployed_at: r.get("last_deployed_at"),
            min_x: r.get("min_x"),
            max_x: r.get("max_x"),
            min_y: r.get("min_y"),
            max_y: r.get("max_y"),
            blocked_since: r.get("blocked_since"),
            deployed_scenes: r.get("deployed_scenes"),
        })
        .collect();
    Ok((worlds, total))
}

/// `GET /admin/worlds`: page and total in one statement.
pub async fn admin_list_worlds_page(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<WorldAdminRow>, i64), ApiError> {
    let rows = sqlx::query(
        r#"SELECT w.name, w.owner, w.access_type, w.blocked_since, w.spawn_coordinates,
                  COALESCE(sc.scene_count, 0)::bigint AS scene_count,
                  count(*) OVER () AS total
           FROM worlds.worlds w
           LEFT JOIN (SELECT world_name, count(*) AS scene_count FROM worlds.world_scenes GROUP BY world_name) sc
             ON sc.world_name = w.name
           ORDER BY w.name
           LIMIT $1 OFFSET $2"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total: i64 = match rows.first() {
        Some(r) => r.get("total"),
        None if offset > 0 => {
            sqlx::query_scalar("SELECT count(*) FROM worlds.worlds")
                .fetch_one(pool)
                .await?
        }
        None => 0,
    };
    let worlds = rows
        .into_iter()
        .map(|r| WorldAdminRow {
            name: r.get("name"),
            owner: r.get("owner"),
            access_type: r.get("access_type"),
            blocked_since: r.get("blocked_since"),
            spawn_coordinates: r.get("spawn_coordinates"),
            scene_count: r.get("scene_count"),
        })
        .collect();
    Ok((worlds, total))
}

/// `(entity_id, parcels)` of a world's scenes, newest first.
pub async fn admin_scenes(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Vec<(String, Vec<String>)>, ApiError> {
    let rows = sqlx::query("SELECT entity_id, parcels FROM worlds.world_scenes WHERE world_name = $1 ORDER BY created_at DESC")
        .bind(world.key())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get("entity_id"), r.get("parcels")))
        .collect())
}

pub async fn admin_set_world_blocked(
    pool: &PgPool,
    world: &WorldName,
    blocked: bool,
) -> Result<bool, ApiError> {
    let sql = if blocked {
        "UPDATE worlds.worlds SET blocked_since = timezone('UTC', now()), updated_at = timezone('UTC', now()) WHERE name = $1"
    } else {
        "UPDATE worlds.worlds SET blocked_since = NULL, updated_at = timezone('UTC', now()) WHERE name = $1"
    };
    let res = sqlx::query(sql).bind(world.key()).execute(pool).await?;
    Ok(res.rows_affected() > 0)
}

pub async fn admin_list_blocked(pool: &PgPool) -> Result<Vec<BlockedRow>, ApiError> {
    let rows = sqlx::query("SELECT wallet, created_at, updated_at FROM worlds.blocked_wallets ORDER BY created_at DESC").fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| BlockedRow {
            wallet: r.get("wallet"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn admin_block_wallet(pool: &PgPool, wallet: &Address) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1) ON CONFLICT (wallet) DO UPDATE SET updated_at = timezone('UTC', now())",
    )
    .bind(wallet)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn admin_unblock_wallet(pool: &PgPool, wallet: &Address) -> Result<bool, ApiError> {
    let res = sqlx::query("DELETE FROM worlds.blocked_wallets WHERE wallet = $1")
        .bind(wallet)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// The admin listing, one statement per filter arm so each walks its own
/// index (`world_access_log_{world,address,created}_idx`; a generic
/// `$1 IS NULL OR` plan cannot pick the arm).
pub async fn admin_query_access_log(
    pool: &PgPool,
    world: Option<&WorldName>,
    address: Option<&Address>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AccessLogRow>, ApiError> {
    const COLUMNS: &str =
        "SELECT id, world_name, address, action, room, created_at FROM worlds.world_access_log";
    const ORDER: &str = "ORDER BY created_at DESC, id DESC LIMIT $1 OFFSET $2";
    let rows = match (world, address) {
        (Some(w), Some(a)) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "{COLUMNS} WHERE world_name = $3 AND address = $4 {ORDER}"
            )))
            .bind(limit)
            .bind(offset)
            .bind(w.key())
            .bind(a)
            .fetch_all(pool)
            .await?
        }
        (Some(w), None) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "{COLUMNS} WHERE world_name = $3 {ORDER}"
            )))
            .bind(limit)
            .bind(offset)
            .bind(w.key())
            .fetch_all(pool)
            .await?
        }
        (None, Some(a)) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "{COLUMNS} WHERE address = $3 {ORDER}"
            )))
            .bind(limit)
            .bind(offset)
            .bind(a)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query(sqlx::AssertSqlSafe(format!("{COLUMNS} {ORDER}")))
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        }
    };
    Ok(rows
        .into_iter()
        .map(|r| AccessLogRow {
            id: r.get("id"),
            world_name: r.get("world_name"),
            address: r.get("address"),
            action: r.get("action"),
            room: r.get("room"),
            created_at: r.get("created_at"),
        })
        .collect())
}

/// One multi-row insert of queued access-log rows.
pub async fn insert_access_log(pool: &PgPool, rows: &[AccessLogEntry]) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }
    let (mut worlds, mut addresses, mut actions, mut rooms) = (
        Vec::with_capacity(rows.len()),
        Vec::with_capacity(rows.len()),
        Vec::with_capacity(rows.len()),
        Vec::with_capacity(rows.len()),
    );
    for e in rows {
        worlds.push(e.world_name.as_str());
        addresses.push(e.address.as_str());
        actions.push(e.action);
        rooms.push(e.room.as_str());
    }
    sqlx::query(
        "INSERT INTO worlds.world_access_log (world_name, address, action, room) SELECT * FROM unnest($1::text[], $2::text[], $3::text[], $4::text[])",
    )
    .bind(&worlds)
    .bind(&addresses)
    .bind(&actions)
    .bind(&rooms)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AccessLogEntry {
    /// Lowercase.
    pub world_name: String,
    /// Lowercase.
    pub address: String,
    /// `join` | `leave`.
    pub action: &'static str,
    pub room: String,
}

fn collect_content_hashes(
    entity_id: &str,
    content: Option<&Value>,
    out: &mut HashSet<String>,
) -> Result<(), String> {
    let items = match content {
        None | Some(Value::Null) => return Ok(()),
        Some(Value::Array(items)) => items,
        Some(_) => {
            return Err(format!(
                "scene {entity_id} stores a non-array entity.content"
            ));
        }
    };
    for item in items {
        match item.get("hash").and_then(Value::as_str) {
            Some(hash) if !hash.is_empty() => {
                out.insert(hash.to_string());
            }
            _ => {
                return Err(format!(
                    "scene {entity_id} has a content entry without a usable hash"
                ));
            }
        }
    }
    Ok(())
}

/// Every blob key `worlds.*` references: each scene entity id and its content
/// hashes, and each world's thumbnail. A row this cannot read exactly aborts
/// the whole set (a caller deletes everything the set does not name), read in
/// one repeatable-read snapshot. The auth chain lives in the row, so there is
/// no `<entity>.auth` key any more.
pub async fn active_content_keys(pool: &PgPool) -> Result<HashSet<String>, ApiError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let scenes = sqlx::query("SELECT entity_id, jsonb_typeof(entity) AS entity_type, entity -> 'content' AS content FROM worlds.world_scenes")
        .fetch_all(&mut *tx)
        .await?;
    let mut keys: HashSet<String> = HashSet::with_capacity(scenes.len() * 4);
    for row in &scenes {
        let entity_id: String = row.try_get("entity_id")?;
        let entity_type: Option<String> = row.try_get("entity_type")?;
        if entity_type.as_deref() != Some("object") {
            return Err(ApiError::internal(format!(
                "scene {entity_id} stores an entity that is not a JSON object"
            )));
        }
        let content: Option<Value> = row.try_get("content")?;
        collect_content_hashes(&entity_id, content.as_ref(), &mut keys)
            .map_err(ApiError::internal)?;
        keys.insert(entity_id);
    }
    let thumbnails =
        sqlx::query("SELECT thumbnail_hash FROM worlds.worlds WHERE thumbnail_hash IS NOT NULL")
            .fetch_all(&mut *tx)
            .await?;
    for row in &thumbnails {
        let hash: String = row.try_get("thumbnail_hash")?;
        if !hash.is_empty() {
            keys.insert(hash);
        }
    }
    tx.commit().await?;
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entity_with_wc(wc: Value) -> Value {
        json!({ "metadata": { "worldConfiguration": wc } })
    }

    fn entity_with_base(base: Value) -> Value {
        json!({ "metadata": { "scene": { "base": base } } })
    }

    #[test]
    fn effective_base_trusts_declared_base_only_when_a_member() {
        let parcels = vec!["0,0".to_string(), "0,1".to_string()];
        assert_eq!(
            effective_base_parcel(&entity_with_base(json!("0,1")), &parcels),
            Some("0,1".to_string())
        );
        assert_eq!(
            effective_base_parcel(&entity_with_base(json!(" 00 , 01 ")), &parcels),
            Some("0,1".to_string())
        );
        assert_eq!(
            effective_base_parcel(&entity_with_base(json!("9,9")), &parcels),
            Some("0,0".to_string())
        );
        assert_eq!(
            effective_base_parcel(&json!({ "metadata": { "scene": {} } }), &parcels),
            Some("0,0".to_string())
        );
        assert_eq!(
            effective_base_parcel(&entity_with_base(json!("")), &parcels),
            Some("0,0".to_string())
        );
        assert_eq!(
            effective_base_parcel(&entity_with_base(json!("0,0")), &[]),
            None
        );
    }

    #[test]
    fn opt_out_uses_js_truthiness() {
        let opt_out = |v: Value| {
            scene_settings_from_entity(&entity_with_wc(json!({ "placesConfig": { "optOut": v } })))
                .show_in_places
        };
        assert_eq!(opt_out(json!(true)), Some(false));
        assert_eq!(opt_out(json!(1)), Some(false));
        assert_eq!(opt_out(json!("false")), Some(false));
        assert_eq!(opt_out(json!({})), Some(false));
        assert_eq!(opt_out(json!(false)), Some(true));
        assert_eq!(opt_out(json!(0)), Some(true));
        assert_eq!(opt_out(json!(null)), Some(true));
        let undeclared = scene_settings_from_entity(&entity_with_wc(json!({ "placesConfig": {} })));
        assert_eq!(undeclared.show_in_places, None);
    }

    #[test]
    fn text_bounds_are_utf16_code_units() {
        let entity = json!({ "metadata": { "display": {
            "title": "\u{65E5}\u{672C}",
            "description": "\u{30C7}".repeat(400),
        } } });
        let s = scene_settings_from_entity(&entity);
        assert_eq!(s.title, None, "2 UTF-16 units is under the 3-unit minimum");
        assert_eq!(
            s.description.as_deref().map(text_len),
            Some(400),
            "400 UTF-16 units is within the 1000-unit maximum despite 1200 bytes"
        );
    }

    #[test]
    fn skybox_time_shares_the_settings_policy_coercion() {
        let skybox = |v: Value| {
            scene_settings_from_entity(&entity_with_wc(
                json!({ "skyboxConfig": { "fixedTime": v } }),
            ))
            .skybox_time
        };
        assert_eq!(skybox(json!(36000)), Some(36000));
        assert_eq!(skybox(json!(36000.0)), Some(36000));
        assert_eq!(skybox(json!(1.5)), None);
        assert_eq!(skybox(json!(99999999999i64)), None);
    }

    #[test]
    fn content_hashes_fail_closed_on_anything_unreadable() {
        let mut out = HashSet::new();
        assert!(collect_content_hashes("e", None, &mut out).is_ok());
        assert!(collect_content_hashes("e", Some(&Value::Null), &mut out).is_ok());
        assert!(out.is_empty());

        assert!(
            collect_content_hashes(
                "e",
                Some(
                    &json!([{ "file": "a.png", "hash": "h1" }, { "file": "b.png", "hash": "h2" }])
                ),
                &mut out
            )
            .is_ok()
        );
        assert_eq!(out.len(), 2);
        assert!(out.contains("h1") && out.contains("h2"));

        assert!(collect_content_hashes("e", Some(&json!({})), &mut out).is_err());
        assert!(collect_content_hashes("e", Some(&json!("nope")), &mut out).is_err());
        assert!(
            collect_content_hashes("e", Some(&json!([{ "file": "a.png" }])), &mut out).is_err()
        );
        assert!(collect_content_hashes("e", Some(&json!([{ "hash": "" }])), &mut out).is_err());
        assert!(collect_content_hashes("e", Some(&json!([{ "hash": 7 }])), &mut out).is_err());
    }

    #[test]
    fn the_access_setting_renders_the_old_public_document() {
        assert_eq!(
            AccessSetting::Unrestricted.to_public_json(),
            json!({ "type": "unrestricted" })
        );
        assert_eq!(
            AccessSetting::SharedSecret {
                secret_hash: "h".into()
            }
            .to_public_json(),
            json!({ "type": "shared-secret" })
        );
        assert_eq!(
            AccessSetting::NftOwnership {
                nft: "urn:x".into()
            }
            .to_public_json(),
            json!({ "type": "nft-ownership", "nft": "urn:x" })
        );
        assert_eq!(
            AccessSetting::AllowList {
                wallets: vec!["0xabc".into()],
                communities: vec!["c".into()]
            }
            .to_public_json(),
            json!({ "type": "allow-list", "wallets": ["0xabc"], "communities": ["c"] })
        );
        assert_eq!(
            AccessSetting::default().access_type(),
            AccessType::Unrestricted
        );
    }
}
