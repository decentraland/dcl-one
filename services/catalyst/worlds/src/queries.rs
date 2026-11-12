//! The read queries over `worlds.*`, schema-qualified, on the shared pool.
//! The old `WorldsComponent` trait/struct forwarded to these one for one; the
//! `about_memo` it kept is gone: one indexed statement per request.

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

use crate::content::errors::AppResult;

#[derive(Debug, Clone)]
pub struct WorldRecord {
    pub name: String,
    /// The operator disable.
    pub blocked_since: Option<NaiveDateTime>,
    /// The owner's storage quota block.
    pub quota_blocked_since: Option<NaiveDateTime>,
    pub spawn_coordinates: Option<String>,
    pub skybox_time: Option<i32>,
    pub single_player: bool,
    pub realm_name_override: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorldScene {
    pub entity_id: String,
    pub entity: Value,
    pub parcels: Vec<String>,
    /// The zero address on mirrored scenes; `mirror_source` is the authority.
    pub deployer: String,
    pub mirror_source: Option<String>,
}

impl WorldScene {
    pub fn is_local(&self) -> bool {
        self.mirror_source.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct WorldAbout {
    pub world: Option<WorldRecord>,
    pub scenes: Vec<WorldScene>,
}

#[derive(Debug, Clone)]
pub struct WorldSceneRow {
    pub world_name: String,
    pub entity_id: String,
    pub deployment_auth_chain: Value,
    pub entity: Value,
    pub deployer: String,
    pub parcels: Vec<String>,
    pub size: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorldsCount {
    pub ens: i64,
    pub dcl: i64,
}

fn world_scene_from_row(r: &PgRow) -> WorldScene {
    WorldScene {
        entity_id: r.get("entity_id"),
        entity: r.get("entity"),
        parcels: r.get("parcels"),
        deployer: r.get("deployer"),
        mirror_source: r.get("mirror_source"),
    }
}

/// The world row (if any) and its scenes newest-first in one statement;
/// either side may be absent independently.
pub async fn world_about(pool: &PgPool, world_name: &str) -> AppResult<WorldAbout> {
    let rows = sqlx::query(concat!(
        r#"SELECT w.name, w.blocked_since, q.created_at AS quota_blocked_since,
                  w.spawn_coordinates, w.skybox_time,
                  w.single_player, w.realm_name_override,
                  s.entity_id, s.entity, s.parcels, s.deployer, s.mirror_source
           FROM (SELECT * FROM worlds.worlds WHERE name = lower($1)) w
           LEFT JOIN worlds.quota_blocks q ON q.wallet = w.owner AND "#,
        crate::core::quota::quota_blockable_world!(),
        r#"
           FULL OUTER JOIN (
             SELECT entity_id, entity, parcels, deployer, mirror_source, created_at
             FROM worlds.world_scenes WHERE world_name = lower($1)
           ) s ON true
           ORDER BY s.created_at DESC NULLS LAST"#,
    ))
    .bind(world_name)
    .fetch_all(pool)
    .await?;

    let world = rows.first().and_then(|r| {
        r.get::<Option<String>, _>("name").map(|name| WorldRecord {
            name,
            blocked_since: r.get("blocked_since"),
            quota_blocked_since: r.get("quota_blocked_since"),
            spawn_coordinates: r.get("spawn_coordinates"),
            skybox_time: r.get("skybox_time"),
            single_player: r.get::<Option<bool>, _>("single_player").unwrap_or(false),
            realm_name_override: r.get("realm_name_override"),
        })
    });
    let scenes = rows
        .iter()
        .filter(|r| r.get::<Option<String>, _>("entity_id").is_some())
        .map(world_scene_from_row)
        .collect();
    Ok(WorldAbout { world, scenes })
}

/// The filters of `GET`/`POST /world/{name}/scenes` (the old `GetWorldScenesFilters`).
#[derive(Debug, Clone, Default)]
pub struct SceneFilters {
    pub entity_id: Option<String>,
    /// Canonical `x,y`: scenes standing on any of them.
    pub coordinates: Vec<String>,
    /// `(x1, x2, y1, y2)`: scenes with a parcel inside the rectangle.
    pub bounding_box: Option<(f64, f64, f64, f64)>,
    /// Lowercase address: scenes of worlds it owns or holds a deployment grant in.
    pub authorized_deployer: Option<String>,
}

impl SceneFilters {
    pub fn is_empty(&self) -> bool {
        self.entity_id.is_none()
            && self.coordinates.is_empty()
            && self.bounding_box.is_none()
            && self.authorized_deployer.is_none()
    }
}

fn push_scene_filters(qb: &mut QueryBuilder<Postgres>, world_name: &str, f: &SceneFilters) {
    qb.push(" FROM worlds.world_scenes WHERE world_name = lower(")
        .push_bind(world_name)
        .push(")");
    if let Some(id) = &f.entity_id {
        qb.push(" AND entity_id = ").push_bind(id);
    }
    if !f.coordinates.is_empty() {
        qb.push(" AND parcels && ")
            .push_bind(&f.coordinates)
            .push("::text[]");
    }
    if let Some((x1, x2, y1, y2)) = f.bounding_box {
        qb.push(
            " AND EXISTS (SELECT 1 FROM unnest(parcels) AS coord, \
               LATERAL (SELECT string_to_array(coord, ',') AS arr) a \
             WHERE (a.arr)[1]::int BETWEEN ",
        )
        .push_bind(x1.min(x2))
        .push(" AND ")
        .push_bind(x1.max(x2))
        .push(" AND (a.arr)[2]::int BETWEEN ")
        .push_bind(y1.min(y2))
        .push(" AND ")
        .push_bind(y1.max(y2))
        .push(")");
    }
    if let Some(deployer) = &f.authorized_deployer {
        qb.push(
            " AND EXISTS (SELECT 1 FROM worlds.worlds w \
             WHERE w.name = world_scenes.world_name AND (w.owner = ",
        )
        .push_bind(deployer)
        .push(
            " OR EXISTS (SELECT 1 FROM worlds.world_permissions wp \
             WHERE wp.world_name = w.name AND wp.permission = 'deployment' AND wp.address = ",
        )
        .push_bind(deployer)
        .push(")))");
    }
}

/// One page of a world's matching scenes, oldest first, and the full match count.
pub async fn list_scenes_full(
    pool: &PgPool,
    world_name: &str,
    filters: &SceneFilters,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<WorldSceneRow>, i64)> {
    let mut count = QueryBuilder::new("SELECT count(*)");
    push_scene_filters(&mut count, world_name, filters);
    let total: i64 = count.build_query_scalar().fetch_one(pool).await?;

    let mut page = QueryBuilder::new(
        "SELECT world_name, entity_id, deployment_auth_chain, entity, deployer, \
         parcels, size, created_at, updated_at",
    );
    push_scene_filters(&mut page, world_name, filters);
    page.push(" ORDER BY created_at, entity_id LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = page.build().fetch_all(pool).await?;
    let scenes = rows
        .into_iter()
        .map(|r| WorldSceneRow {
            world_name: r.get("world_name"),
            entity_id: r.get("entity_id"),
            deployment_auth_chain: r.get("deployment_auth_chain"),
            entity: r.get("entity"),
            deployer: r.get("deployer"),
            parcels: r.get("parcels"),
            size: r.get("size"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();
    Ok((scenes, total))
}

/// The scenes of one page of unblocked worlds (name order), projected to
/// what the index summary reads: timestamp, display, runtimeVersion (only
/// when the metadata has the key) and the one content entry the thumbnail
/// names. `banned` is upstream's `getBannedNames` (the `BLOCKED_WORLD_NAMES`
/// full names and the suffix-stripped deny list), compared exactly with the
/// stored name before paging, as `getRawWorldRecords` does (WR-17).
pub async fn list_index_scenes(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    banned: &[String],
) -> AppResult<Vec<(String, WorldScene)>> {
    let rows = sqlx::query(
        r#"WITH paged_worlds AS (
             SELECT DISTINCT ws.world_name
             FROM worlds.world_scenes ws
             JOIN worlds.worlds w ON lower(w.name) = lower(ws.world_name)
             WHERE w.blocked_since IS NULL AND w.name <> ALL($3::text[])
             ORDER BY ws.world_name
             LIMIT $1 OFFSET $2
           )
           SELECT ws.world_name, ws.entity_id, ws.parcels, ws.deployer, ws.mirror_source,
                  jsonb_build_object(
                    'timestamp', ws.entity->'timestamp',
                    'metadata', jsonb_build_object('display', ws.entity->'metadata'->'display')
                      || CASE WHEN jsonb_typeof(ws.entity->'metadata') = 'object'
                                   AND ws.entity->'metadata' ? 'runtimeVersion'
                              THEN jsonb_build_object('runtimeVersion', ws.entity->'metadata'->'runtimeVersion')
                              ELSE '{}'::jsonb END,
                    'content', (SELECT jsonb_agg(c) FROM jsonb_array_elements(
                                  CASE WHEN jsonb_typeof(ws.entity->'content') = 'array'
                                       THEN ws.entity->'content' ELSE '[]'::jsonb END) c
                                WHERE c->>'file' = ws.entity->'metadata'->'display'->>'navmapThumbnail')
                  ) AS entity
           FROM worlds.world_scenes ws
           JOIN paged_worlds pw ON pw.world_name = ws.world_name
           ORDER BY ws.world_name, ws.created_at DESC"#,
    )
    .bind(limit)
    .bind(offset)
    .bind(banned)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get("world_name"), world_scene_from_row(r)))
        .collect())
}

/// Worlds with at least one scene, split the way the old /status did:
/// `.dcl.eth` names versus everything else (ENS).
pub async fn deployed_world_count(pool: &PgPool) -> AppResult<WorldsCount> {
    let row = sqlx::query(
        r#"SELECT count(*) FILTER (WHERE name LIKE '%.dcl.eth') AS dcl,
                  count(*) FILTER (WHERE name NOT LIKE '%.dcl.eth') AS ens
           FROM (SELECT DISTINCT world_name AS name FROM worlds.world_scenes) n"#,
    )
    .fetch_one(pool)
    .await?;
    Ok(WorldsCount {
        ens: row.get("ens"),
        dcl: row.get("dcl"),
    })
}

/// One `worlds.mirror_passes` row as `/worlds/status` reports it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorPassRow {
    pub mirror: String,
    pub source: String,
    pub pass_number: i64,
    pub pass_started_at: Option<NaiveDateTime>,
    pub pass_completed_at: Option<NaiveDateTime>,
    pub last_attempt_at: Option<NaiveDateTime>,
    pub cursor: Option<String>,
    pub stats: Value,
    pub last_error: Option<String>,
}

pub async fn mirror_passes(pool: &PgPool) -> AppResult<Vec<MirrorPassRow>> {
    let rows = sqlx::query(
        r#"SELECT mirror, source, pass_number, pass_started_at, pass_completed_at,
                  last_attempt_at, cursor, stats, last_error
           FROM worlds.mirror_passes ORDER BY mirror"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| MirrorPassRow {
            mirror: r.get("mirror"),
            source: r.get("source"),
            pass_number: r.get("pass_number"),
            pass_started_at: r.get("pass_started_at"),
            pass_completed_at: r.get("pass_completed_at"),
            last_attempt_at: r.get("last_attempt_at"),
            cursor: r.get("cursor"),
            stats: r.get("stats"),
            last_error: r.get("last_error"),
        })
        .collect())
}

/// Catalogue counts for `/worlds/status`: listed upstream; mirrored (this
/// listing was mirrored without error); pending (not yet mirrored since it
/// was last listed); failed (the last attempt left an error, including a
/// refused local publish); unlisted but not yet retired.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct CatalogueCounts {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub listed: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub mirrored: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub pending: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub failed: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub unlisted: i64,
}

pub async fn catalogue_counts(pool: &PgPool) -> AppResult<CatalogueCounts> {
    let row = sqlx::query(
        r#"SELECT count(*) FILTER (WHERE unlisted_at IS NULL) AS listed,
                  count(*) FILTER (WHERE unlisted_at IS NULL AND last_error IS NULL
                                     AND mirrored_at IS NOT NULL AND mirrored_at >= listed_at) AS mirrored,
                  count(*) FILTER (WHERE unlisted_at IS NULL AND last_error IS NULL
                                     AND (mirrored_at IS NULL OR mirrored_at < listed_at)) AS pending,
                  count(*) FILTER (WHERE unlisted_at IS NULL AND last_error IS NOT NULL) AS failed,
                  count(*) FILTER (WHERE unlisted_at IS NOT NULL) AS unlisted
           FROM worlds.remote_worlds"#,
    )
    .fetch_one(pool)
    .await?;
    Ok(CatalogueCounts {
        listed: row.get("listed"),
        mirrored: row.get("mirrored"),
        pending: row.get("pending"),
        failed: row.get("failed"),
        unlisted: row.get("unlisted"),
    })
}
