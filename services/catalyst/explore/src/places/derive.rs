//! The place catalogue derived from this node's own scene deployments (the
//! old `place-catalog-sync`, `PLACES_DERIVE_FROM_CONTENT`), a job over
//! `content.deployments` that composes with the places mirror:
//!
//! - [`DeriveScope::All`] (`PLACES_DERIVE_FROM_CONTENT` without
//!   `PLACES_MIRROR_UPSTREAM`, the old production mode): every active scene
//!   becomes a place row, `raw.source = "content"`, keyed by the UUIDv5 of
//!   its base parcel so a scene keeps one id across redeployments.
//! - [`DeriveScope::Local`] (every other node): only the scenes this node
//!   accepted itself (`content.local_entities`, not tombstoned), so a local
//!   deploy appears among the places whether or not the upstream catalogue
//!   is mirrored beside it. These rows are `origin = 'local'`, which the
//!   places mirror's retire and sweep spare (the document keeps the older
//!   marker, `raw.local = true`, by which the mirror reclassifies a row an
//!   older binary derived).
//!
//! Either scope also lists the worlds this node publishes itself in
//! `explore.place_world_local` (`raw.source = "worlds"`, keyed by the world's
//! name), from `worlds.worlds` and its local `worlds.world_scenes`; the
//! mirrored row of such a world goes, and the places mirror skips it.
//!
//! Source: `content.deployments` (active, `entity_type = 'scene'`) with
//! `content.content_files` (the navmap thumbnail) and `content.local_entities`.
//! Freshness: one pass per `PLACES_DERIVE_INTERVAL_SECS`; a pass stamps
//! `fetched_at` on every row it derives and prunes the derived rows it did not
//! stamp (a scene undeployed, overwritten or unpublished). Rebuild: the same
//! pass (every derived row is recomputed from its deployment; the operator's
//! keys in `raw` and the moderation overlay survive). A derived row never
//! overwrites a mirrored one on an id collision: the upstream is the
//! authority for the rows it lists.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

use super::query::{EXCLUDE_FROM_RANKING_SQL, raw_float8_sql};
use crate::jobs::{Jobs, PeriodicCfg};

pub const JOB_NAME: &str = "places-derive";
pub const PAGE: i64 = 1000;
/// deployments.id is a plain `integer` sequence, so the walk starts below every
/// row it can hold.
pub const BEFORE_FIRST_SCENE: i32 = i32::MIN;
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeriveScope {
    /// Every active scene deployment (the old derive mode).
    All,
    /// The scenes this node accepted itself.
    Local,
}

impl DeriveScope {
    /// The old rule: the node derives its whole catalogue from its content
    /// when asked to and when no upstream catalogue is mirrored; otherwise it
    /// derives its own scenes only.
    pub fn resolve(derive_from_content: bool, places_mirrored: bool) -> Self {
        if derive_from_content && !places_mirrored {
            Self::All
        } else {
            Self::Local
        }
    }
}

pub struct DerivedPlace {
    pub id: String,
    pub base_position: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub creator_address: Option<String>,
    pub content_rating: Option<String>,
    pub deployed_at: Option<NaiveDateTime>,
    pub raw: Value,
    /// A scene this node accepted itself (`explore.place.origin = 'local'`).
    pub local: bool,
}

fn scene_json(metadata: &Value) -> &Value {
    metadata.get("v").unwrap_or(metadata)
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn str_array(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Creator categories follow the upstream active catalogue: at most three,
/// deduplicated in tag order. Editorial categories are operator authority.
fn creator_categories(tags: &[String]) -> Vec<String> {
    let mut categories = Vec::new();
    for tag in tags {
        if tag != "poi"
            && tag != "featured"
            && super::handlers::category_i18n_en(tag).is_some()
            && !categories.contains(tag)
        {
            categories.push(tag.clone());
            if categories.len() == 3 {
                break;
            }
        }
    }
    categories
}

pub fn derive(
    deployer_address: &str,
    pointers: &[String],
    deployed_at: Option<NaiveDateTime>,
    metadata: &Value,
    thumbnail_hash: Option<&str>,
    content_public_url: &str,
    local: bool,
) -> Option<DerivedPlace> {
    let m = scene_json(metadata);
    let scene = m.get("scene")?;
    let base = non_empty(scene.get("base").and_then(Value::as_str))
        .or_else(|| pointers.first().cloned())?;
    let display = m.get("display");
    let title = non_empty(display.and_then(|d| d.get("title")).and_then(Value::as_str));
    let description = non_empty(
        display
            .and_then(|d| d.get("description"))
            .and_then(Value::as_str),
    );
    let contact = m.get("contact");
    let contact_name = non_empty(contact.and_then(|c| c.get("name")).and_then(Value::as_str));
    let contact_email = non_empty(contact.and_then(|c| c.get("email")).and_then(Value::as_str));
    let content_rating = non_empty(
        m.get("policy")
            .and_then(|p| p.get("contentRating"))
            .and_then(Value::as_str),
    );
    let mut positions = str_array(scene.get("parcels"));
    if positions.is_empty() {
        positions.push(base.clone());
    }
    let tags = str_array(m.get("tags"));
    let image = thumbnail_hash.map(|h| {
        format!(
            "{}/contents/{}",
            content_public_url.trim_end_matches('/'),
            h
        )
    });
    let creator_address = non_empty(m.get("creator").and_then(Value::as_str))
        .or_else(|| non_empty(Some(deployer_address)))
        .map(|s| s.to_lowercase());
    let owner = non_empty(m.get("owner").and_then(Value::as_str))
        .or_else(|| non_empty(Some(deployer_address)))
        .map(|s| s.to_lowercase());
    let categories = creator_categories(&tags);
    let id = super::parcel_place_id(&base);
    let at = deployed_at.map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc));
    let mut raw = json!({
        "id": id,
        "title": title,
        "description": description,
        "owner": owner,
        "creator_address": creator_address,
        "sdk": non_empty(m.get("runtimeVersion").and_then(Value::as_str)),
        "positions": positions,
        "base_position": base,
        "contact_name": contact_name,
        "contact_email": contact_email,
        "image": image,
        "tags": tags,
        "categories": categories,
        "disabled": false,
        "world": false,
        "deployed_at": at,
        "created_at": at,
        "updated_at": at,
        "source": "content",
    });
    if local {
        raw["local"] = json!(true);
    }
    Some(DerivedPlace {
        id,
        base_position: base,
        title,
        description,
        creator_address,
        content_rating,
        deployed_at,
        raw,
        local,
    })
}

/// Keyset, never OFFSET: an undeployment sets deleter_deployment mid-scan and
/// shifts every later OFFSET window back a row, so a still-served scene goes
/// unvisited and the prune then deletes it on its stale fetched_at. `$3`
/// false walks every active scene, true only the local ones; a tombstoned
/// local scene (unpublished) is never derived.
pub const SELECT_SCENES: &str = r#"
    SELECT
        d.id,
        d.entity_id,
        d.deployer_address,
        d.entity_pointers,
        d.entity_timestamp AS deployed_at,
        (d.entity_metadata::jsonb) AS meta,
        (le.entity_id IS NOT NULL) AS local,
        (SELECT cf.content_hash FROM content.content_files cf
          WHERE cf.deployment = d.id
            AND cf.key = (d.entity_metadata::jsonb)->'v'->'display'->>'navmapThumbnail'
          LIMIT 1) AS thumbnail_hash,
        (SELECT cf.content_hash FROM content.content_files cf
          WHERE cf.deployment = d.id AND cf.key = 'scene.json'
          LIMIT 1) AS scene_json_hash
    FROM content.deployments d
    LEFT JOIN content.local_entities le ON le.entity_id = d.entity_id
    WHERE d.entity_type = 'scene' AND d.deleter_deployment IS NULL AND d.id > $2
      AND (le.entity_id IS NOT NULL OR NOT $3)
      AND le.tombstoned_at IS NULL
    ORDER BY d.id
    LIMIT $1
"#;

/// Every raw key an operator writes outside this rebuild has to be named in the
/// preservation object: the pass rebuilds `raw` wholesale from the deployment,
/// so a key it does not carry forward is erased by the next pass. The
/// moderation overlay (`place_moderation`) decides `disabled`, `highlighted`
/// and `content_rating` of a new row; an existing row keeps its own. A row the
/// places mirror wrote is left to the mirror.
const UPSERT: &str = r#"
    INSERT INTO explore.place
        (id, base_position, title, description, creator_address, content_rating,
         categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
         raw, fetched_at, origin)
    SELECT u.id, u.base_position, u.title, u.description, u.creator_address,
           COALESCE(m.content_rating, u.content_rating),
           ARRAY(SELECT jsonb_array_elements_text(u.raw->'categories')), 0, 0, 0,
           u.deployed_at, COALESCE(m.disabled, false), COALESCE(m.highlighted, false),
           u.raw || COALESCE(r.overlay, '{}'::jsonb), timezone('UTC', now()), u.origin
    FROM unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::timestamp[], $8::jsonb[], $9::text[])
         AS u(id, base_position, title, description, creator_address, content_rating,
              deployed_at, raw, origin)
    LEFT JOIN explore.place_moderation m ON m.place_id = u.id
    LEFT JOIN explore.place_ranking r ON r.place_id = u.id
    ON CONFLICT (id) DO UPDATE SET
        base_position   = EXCLUDED.base_position,
        title           = EXCLUDED.title,
        description     = EXCLUDED.description,
        creator_address = EXCLUDED.creator_address,
        categories = ARRAY(SELECT DISTINCT c FROM unnest(EXCLUDED.categories || explore.place.categories) c
                           WHERE c IN ('poi', 'featured') OR c = ANY(EXCLUDED.categories)),
        deployed_at     = EXCLUDED.deployed_at,
        raw = EXCLUDED.raw || jsonb_strip_nulls(jsonb_build_object(
            'ranking',              explore.place.raw->'ranking',
            'exclude_from_ranking', explore.place.raw->'exclude_from_ranking',
            'highlighted_image',    explore.place.raw->'highlighted_image',
            'like_score',           explore.place.raw->'like_score',
            'like_rate',            explore.place.raw->'like_rate',
            'created_at',           explore.place.raw->'created_at',
            'disabled_at',          explore.place.raw->'disabled_at',
            'disabled_reason',      explore.place.raw->'disabled_reason'
        )),
        fetched_at = timezone('UTC', now()),
        origin     = EXCLUDED.origin
    WHERE explore.place.raw->>'source' = 'content'
    RETURNING id, (xmax = 0) AS inserted
"#;

/// Overlap candidates for every fresh row of a page at once: `$1` the fresh ids, `$2`
/// each one's positions as a jsonb array.
pub(crate) fn overlapping_places_sql() -> &'static str {
    static SQL: LazyLock<String> = LazyLock::new(|| {
        let ranking = raw_float8_sql("ranking");
        format!(
            r#"
    SELECT f.id AS fresh_id, p.id, p.highlighted, p.creator_address,
           {ranking} AS ranking,
           {EXCLUDE_FROM_RANKING_SQL} AS exclude_from_ranking,
           raw->>'highlighted_image' AS highlighted_image
    FROM unnest($1::text[], $2::jsonb[]) AS f(id, positions)
    JOIN explore.place p
      ON p.id <> f.id AND p.disabled IS FALSE AND p.world IS FALSE
     AND p.raw->'positions' ?| ARRAY(SELECT jsonb_array_elements_text(f.positions))
"#
        )
    });
    &SQL
}

const INHERIT_CURATION: &str = r#"
    UPDATE explore.place
    SET highlighted = u.highlighted,
        raw = raw || jsonb_build_object(
            'ranking',              u.ranking,
            'highlighted_image',    u.highlighted_image,
            'exclude_from_ranking', u.exclude_from_ranking
        )
    FROM unnest($1::text[], $2::boolean[], $3::float8[], $4::text[], $5::boolean[])
         AS u(id, highlighted, ranking, highlighted_image, exclude_from_ranking)
    WHERE explore.place.id = u.id
"#;

/// The derived rows a pass did not stamp: `$1` the pass start.
const PRUNE: &str =
    "DELETE FROM explore.place WHERE raw->>'source' = 'content' AND fetched_at < $1";

/// One `explore.place_world_local` row per unblocked world this node publishes
/// itself (no mirror source and a local scene, `worlds::core::store::deploy_scene`;
/// a blocked world is hidden as the worlds listing hides it), keyed
/// by the lowercase name as upstream keys its worlds: the world row's settings
/// with its newest local scene's base parcel, contact and deploy time. `$1` the
/// content public URL the thumbnail is served under. The moderation and ranking
/// overlays apply as in the places mirror; a row this pass did not write is
/// left alone.
const UPSERT_LOCAL_WORLDS: &str = r#"
    INSERT INTO explore.place_world_local AS l
        (id, base_position, title, description, creator_address, content_rating,
         categories, deployed_at, disabled, highlighted, raw, fetched_at, world_name, world)
    SELECT u.id, u.base_position, u.title, u.description, u.owner,
           COALESCE(m.content_rating, u.content_rating), u.categories, u.deployed_at,
           COALESCE(m.disabled, false), COALESCE(m.highlighted, false),
           u.raw || COALESCE(r.overlay, '{}'::jsonb), timezone('UTC', now()), u.world_name, true
    FROM (
        SELECT w.name AS id, w.description, w.owner, w.content_rating,
               COALESCE(w.categories, '{}') AS categories, s.created_at AS deployed_at,
               COALESCE(NULLIF(btrim(s.meta->'scene'->>'base'), ''), s.parcels[1], '0,0') AS base_position,
               COALESCE(w.title, NULLIF(btrim(s.meta->'display'->>'title'), '')) AS title,
               CASE WHEN lower(btrim(s.meta->'worldConfiguration'->>'name')) = w.name
                    THEN btrim(s.meta->'worldConfiguration'->>'name') ELSE w.name END AS world_name,
               jsonb_build_object(
                   'title', COALESCE(w.title, NULLIF(btrim(s.meta->'display'->>'title'), '')),
                   'description', w.description,
                   'image', CASE WHEN COALESCE(w.thumbnail_hash, s.thumbnail_hash) IS NOT NULL
                                 THEN rtrim($1, '/') || '/contents/' || COALESCE(w.thumbnail_hash, s.thumbnail_hash) END,
                   'owner', w.owner,
                   'content_rating', w.content_rating,
                   'categories', to_jsonb(COALESCE(w.categories, '{}')),
                   'show_in_places', w.show_in_places,
                   'single_player', w.single_player,
                   'skybox_time', w.skybox_time,
                   'is_private', w.access_type <> 'unrestricted',
                   'settings_version', w.settings_version,
                   'contact_name', NULLIF(btrim(s.meta->'contact'->>'name'), ''),
                   'contact_email', NULLIF(btrim(s.meta->'contact'->>'email'), ''),
                   'sdk', NULLIF(btrim(s.meta->>'runtimeVersion'), ''),
                   'base_position', COALESCE(NULLIF(btrim(s.meta->'scene'->>'base'), ''), s.parcels[1], '0,0'),
                   'disabled', false,
                   'world', true,
                   'deployed_at', to_char(s.created_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                   'created_at', to_char(w.created_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                   'updated_at', to_char(GREATEST(w.updated_at, s.created_at), 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                   'source', 'worlds',
                   'local', true
               ) AS raw
        FROM worlds.worlds w
        CROSS JOIN LATERAL (
            SELECT ws.entity->'metadata' AS meta, ws.parcels, ws.created_at,
                   (SELECT file->>'hash' FROM jsonb_array_elements(ws.entity->'content') file
                    WHERE file->>'file' = ws.entity->'metadata'->'display'->>'navmapThumbnail'
                    LIMIT 1) AS thumbnail_hash
            FROM worlds.world_scenes ws
            WHERE ws.world_name = w.name AND ws.mirror_source IS NULL
            ORDER BY ws.created_at DESC, ws.entity_id
            LIMIT 1
        ) s
        WHERE w.mirror_source IS NULL AND w.blocked_since IS NULL
    ) u
    LEFT JOIN explore.place_moderation m ON m.place_id = u.id
    LEFT JOIN explore.place_ranking r ON r.place_id = u.id
    ON CONFLICT (id) DO UPDATE SET
        base_position   = EXCLUDED.base_position,
        title           = EXCLUDED.title,
        description     = EXCLUDED.description,
        creator_address = EXCLUDED.creator_address,
        content_rating  = EXCLUDED.content_rating,
        categories      = EXCLUDED.categories,
        deployed_at     = EXCLUDED.deployed_at,
        disabled        = EXCLUDED.disabled,
        highlighted     = EXCLUDED.highlighted,
        raw             = EXCLUDED.raw,
        fetched_at      = EXCLUDED.fetched_at,
        world_name      = EXCLUDED.world_name
    WHERE l.raw->>'source' = 'worlds'
"#;

/// A world this node publishes is listed once, from the local leg: the mirrored
/// row of the same world goes (the places mirror skips it while the local row
/// stands, `mirror::UPSERT`).
const DISPLACE_MIRRORED_WORLDS: &str = r#"
    DELETE FROM explore.place p USING explore.place_world_local l
    WHERE l.raw->>'source' = 'worlds' AND p.world IS TRUE
      AND (p.id = l.id OR lower(p.world_name) = l.id)
"#;

/// The local world rows a pass did not stamp (every local scene unpublished,
/// the world blocked, or mirrored again): `$1` the pass start.
const PRUNE_LOCAL_WORLDS: &str =
    "DELETE FROM explore.place_world_local WHERE raw->>'source' = 'worlds' AND fetched_at < $1";

#[derive(Debug, Clone, PartialEq)]
pub struct Curation {
    pub highlighted: bool,
    pub ranking: Option<f64>,
    pub exclude_from_ranking: bool,
    pub highlighted_image: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CurationCandidate {
    pub creator_address: Option<String>,
    pub curation: Curation,
}

/// A fresh place inherits the curation of the one curated place its parcels
/// overlap when the same creator deployed both (a redeploy under a new base
/// parcel keeps its highlight); two curated predecessors, or none, inherit
/// nothing.
pub fn inherited_curation(
    candidates: &[CurationCandidate],
    creator: Option<&str>,
) -> Option<Curation> {
    let creator = creator.filter(|c| !c.is_empty())?;
    let mut curated = candidates.iter().filter(|c| {
        (c.curation.highlighted
            || c.curation.exclude_from_ranking
            || c.curation.ranking.unwrap_or(0.0) > 0.0)
            && c.creator_address.as_deref() == Some(creator)
    });
    let predecessor = curated.next()?;
    if curated.next().is_some() {
        return None;
    }
    Some(predecessor.curation.clone())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PassSummary {
    pub derived: usize,
    pub pruned: u64,
}

/// One pass: walk the scenes of `scope` by id, upsert each page, prune the
/// derived rows not stamped; then the local worlds, in one transaction.
pub async fn run_once(
    pool: &PgPool,
    scope: DeriveScope,
    content_public_url: &str,
) -> Result<PassSummary> {
    run_once_with_storage(pool, scope, content_public_url, None).await
}

/// The stored, hash-addressed scene.json supplies creator/runtimeVersion.
/// Limit each read to 2 MiB; missing, withheld or malformed metadata keeps
/// the catalogue available without turning a storage fault into an empty pass.
async fn scene_file(
    storage: &crate::storage::Storage,
    entity: &str,
    hash: &str,
) -> Result<Option<Value>> {
    const MAX_SCENE_JSON_BYTES: u64 = 2 * 1024 * 1024;
    if storage.withholds(entity) {
        return Ok(None);
    }
    let Some(mut file) = storage.open(hash).await? else {
        return Ok(None);
    };
    if file.size > MAX_SCENE_JSON_BYTES {
        tracing::warn!(
            entity,
            hash,
            size = file.size,
            "place scene.json exceeds metadata read limit"
        );
        return Ok(None);
    }
    let last = file.size.saturating_sub(1);
    let bytes = file.read_range(0, last).await?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) if value.is_object() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) => {
            tracing::debug!(entity, hash, %error, "place scene.json is not JSON metadata");
            Ok(None)
        }
    }
}

fn apply_scene_file(place: &mut DerivedPlace, scene: &Value, metadata: &Value) {
    place.creator_address =
        non_empty(scene.get("creator").and_then(Value::as_str)).map(|value| value.to_lowercase());
    place.raw["creator_address"] = json!(place.creator_address);
    place.raw["sdk"] = json!(
        non_empty(scene.get("runtimeVersion").and_then(Value::as_str)).or_else(|| non_empty(
            scene_json(metadata)
                .get("runtimeVersion")
                .and_then(Value::as_str)
        ))
    );
}

async fn run_once_with_storage(
    pool: &PgPool,
    scope: DeriveScope,
    content_public_url: &str,
    storage: Option<&crate::storage::Storage>,
) -> Result<PassSummary> {
    let started: NaiveDateTime = sqlx::query_scalar("SELECT timezone('UTC', clock_timestamp())")
        .fetch_one(pool)
        .await?;
    let mut last_id = BEFORE_FIRST_SCENE;
    let mut derived = 0usize;
    loop {
        let rows = sqlx::query(SELECT_SCENES)
            .bind(PAGE)
            .bind(last_id)
            .bind(scope == DeriveScope::Local)
            .fetch_all(pool)
            .await?;
        let fetched = rows.len() as i64;
        let mut page: Vec<DerivedPlace> = Vec::with_capacity(rows.len());
        for row in &rows {
            last_id = row.try_get("id")?;
            let deployer: String = row.try_get("deployer_address").unwrap_or_default();
            let pointers: Vec<String> = row.try_get("entity_pointers").unwrap_or_default();
            let deployed_at: Option<NaiveDateTime> = row.try_get("deployed_at").unwrap_or(None);
            let meta: Value = row.try_get("meta").unwrap_or(Value::Null);
            let local: bool = row.try_get("local").unwrap_or(false);
            let thumb: Option<String> = row.try_get("thumbnail_hash").unwrap_or(None);
            if let Some(mut p) = derive(
                &deployer,
                &pointers,
                deployed_at,
                &meta,
                thumb.as_deref(),
                content_public_url,
                local,
            ) {
                if let (Some(storage), Some(hash)) = (
                    storage,
                    row.try_get::<Option<String>, _>("scene_json_hash")?,
                ) {
                    let entity: String = row.try_get("entity_id")?;
                    if let Some(scene) = scene_file(storage, &entity, &hash).await? {
                        apply_scene_file(&mut p, &scene, &meta);
                    }
                }
                page.push(p);
                derived += 1;
            }
        }
        upsert_page(pool, page).await?;
        if fetched < PAGE {
            break;
        }
    }
    let mut pruned = sqlx::query(PRUNE)
        .bind(started)
        .execute(pool)
        .await?
        .rows_affected();
    let mut tx = pool.begin().await?;
    derived += sqlx::query(UPSERT_LOCAL_WORLDS)
        .bind(content_public_url)
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;
    sqlx::query(DISPLACE_MIRRORED_WORLDS)
        .execute(&mut *tx)
        .await?;
    pruned += sqlx::query(PRUNE_LOCAL_WORLDS)
        .bind(started)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok(PassSummary { derived, pruned })
}

fn positions_of(p: &DerivedPlace) -> Vec<String> {
    p.raw["positions"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// One transaction per page: a multi-row upsert, one overlap query for the fresh rows,
/// and one batched curation update. Later duplicates of an id win, as they did row by row.
async fn upsert_page(pool: &PgPool, page: Vec<DerivedPlace>) -> Result<()> {
    let mut slot_of: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<usize> = Vec::with_capacity(page.len());
    for (i, p) in page.iter().enumerate() {
        match slot_of.get(p.id.as_str()) {
            Some(&slot) => order[slot] = i,
            None => {
                slot_of.insert(p.id.as_str(), order.len());
                order.push(i);
            }
        }
    }
    let rows: Vec<&DerivedPlace> = order.iter().map(|&i| &page[i]).collect();
    if rows.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    let inserted: HashMap<String, bool> = sqlx::query(UPSERT)
        .bind(rows.iter().map(|p| p.id.clone()).collect::<Vec<_>>())
        .bind(
            rows.iter()
                .map(|p| p.base_position.clone())
                .collect::<Vec<_>>(),
        )
        .bind(rows.iter().map(|p| p.title.clone()).collect::<Vec<_>>())
        .bind(
            rows.iter()
                .map(|p| p.description.clone())
                .collect::<Vec<_>>(),
        )
        .bind(
            rows.iter()
                .map(|p| p.creator_address.clone())
                .collect::<Vec<_>>(),
        )
        .bind(
            rows.iter()
                .map(|p| p.content_rating.clone())
                .collect::<Vec<_>>(),
        )
        .bind(rows.iter().map(|p| p.deployed_at).collect::<Vec<_>>())
        .bind(rows.iter().map(|p| p.raw.clone()).collect::<Vec<_>>())
        .bind(
            rows.iter()
                .map(|p| if p.local { "local" } else { "upstream" })
                .collect::<Vec<_>>(),
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.try_get::<String, _>("id").unwrap_or_default(),
                r.try_get::<bool, _>("inserted").unwrap_or(false),
            )
        })
        .collect();
    let fresh: Vec<(&DerivedPlace, Vec<String>)> = rows
        .iter()
        .filter(|p| inserted.get(&p.id).copied().unwrap_or(false))
        .map(|p| (*p, positions_of(p)))
        .filter(|(_, positions)| !positions.is_empty())
        .collect();
    if fresh.is_empty() {
        tx.commit().await?;
        return Ok(());
    }
    let mut candidates: HashMap<String, Vec<(String, CurationCandidate)>> = HashMap::new();
    let found = sqlx::query(sqlx::AssertSqlSafe(overlapping_places_sql()))
        .bind(fresh.iter().map(|(p, _)| p.id.clone()).collect::<Vec<_>>())
        .bind(
            fresh
                .iter()
                .map(|(_, positions)| Value::from(positions.clone()))
                .collect::<Vec<Value>>(),
        )
        .fetch_all(&mut *tx)
        .await?;
    for r in found {
        let fresh_id: String = r.try_get("fresh_id").unwrap_or_default();
        let id: String = r.try_get("id").unwrap_or_default();
        candidates.entry(fresh_id).or_default().push((
            id,
            CurationCandidate {
                creator_address: r.try_get("creator_address").unwrap_or(None),
                curation: Curation {
                    highlighted: r.try_get("highlighted").unwrap_or(false),
                    ranking: r.try_get("ranking").unwrap_or(None),
                    exclude_from_ranking: r.try_get("exclude_from_ranking").unwrap_or(false),
                    highlighted_image: r.try_get("highlighted_image").unwrap_or(None),
                },
            },
        ));
    }
    // Fresh rows earlier in the page that just inherited count as curated for later ones,
    // exactly as when each row was committed before the next was examined.
    let mut inherited: Vec<(String, Curation)> = Vec::new();
    for (p, _) in &fresh {
        let mine: Vec<CurationCandidate> = candidates
            .remove(&p.id)
            .unwrap_or_default()
            .into_iter()
            .map(|(id, mut c)| {
                if let Some((_, cur)) = inherited.iter().find(|(i, _)| *i == id) {
                    c.curation = cur.clone();
                }
                c
            })
            .collect();
        if let Some(curation) = inherited_curation(&mine, p.creator_address.as_deref()) {
            inherited.push((p.id.clone(), curation));
        }
    }
    if !inherited.is_empty() {
        sqlx::query(INHERIT_CURATION)
            .bind(
                inherited
                    .iter()
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>(),
            )
            .bind(
                inherited
                    .iter()
                    .map(|(_, c)| c.highlighted)
                    .collect::<Vec<_>>(),
            )
            .bind(inherited.iter().map(|(_, c)| c.ranking).collect::<Vec<_>>())
            .bind(
                inherited
                    .iter()
                    .map(|(_, c)| c.highlighted_image.clone())
                    .collect::<Vec<_>>(),
            )
            .bind(
                inherited
                    .iter()
                    .map(|(_, c)| c.exclude_from_ranking)
                    .collect::<Vec<_>>(),
            )
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Registers the periodic derive job (`places-derive`).
pub fn spawn_job(
    jobs: &Jobs,
    pool: PgPool,
    scope: DeriveScope,
    interval: Duration,
    content_public_url: String,
    storage: Option<Arc<crate::storage::Storage>>,
) {
    tracing::info!(scope = ?scope, interval_secs = interval.as_secs(), "place catalogue: deriving from content deployments");
    jobs.spawn_periodic_leased(
        JOB_NAME,
        PeriodicCfg::every(interval).with_error_backoff(Duration::from_secs(3600)),
        move |ctx| {
            let (pool, url, storage) = (pool.clone(), content_public_url.clone(), storage.clone());
            async move {
                let out = run_once_with_storage(&pool, scope, &url, storage.as_deref()).await?;
                ctx.progress(format!(
                    "{} places derived, {} pruned",
                    out.derived, out.pruned
                ));
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapped() -> Value {
        json!({ "v": {
            "scene": { "base": "135,2", "parcels": ["135,2", "135,3"] },
            "display": { "title": "My Scene", "description": "hello", "navmapThumbnail": "t.png" },
            "contact": { "name": "author", "email": "a@b.c" },
            "policy": { "contentRating": "E" },
            "tags": ["game", "art"]
        }})
    }

    #[test]
    fn derives_from_v_wrapped_scene() {
        let p = derive(
            "0xABC",
            &["135,2".to_string()],
            None,
            &wrapped(),
            Some("hash123"),
            "/content",
            false,
        )
        .expect("derives");
        assert_eq!(p.base_position, "135,2");
        assert_eq!(p.title.as_deref(), Some("My Scene"));
        assert_eq!(p.creator_address.as_deref(), Some("0xabc"));
        assert_eq!(p.content_rating.as_deref(), Some("E"));
        assert_eq!(p.raw["image"], "/content/contents/hash123");
        assert_eq!(p.raw["positions"], json!(["135,2", "135,3"]));
        assert_eq!(p.raw["source"], "content");
        assert_eq!(p.raw["world"], false);
        assert!(p.raw.get("local").is_none());
        let local = derive("0xABC", &[], None, &wrapped(), None, "/content", true).unwrap();
        assert_eq!(local.raw["local"], true);
    }

    #[test]
    fn scene_file_supplies_creator_and_sdk_without_changing_land_owner() {
        let mut metadata = wrapped();
        metadata["v"]["owner"] = json!("0xLAND");
        metadata["v"]["runtimeVersion"] = json!("6");
        metadata["v"]["tags"] =
            json!(["poi", "game", "game", "art", "social", "music", "featured"]);
        let mut place = derive("0xDEPLOYER", &[], None, &metadata, None, "/content", true).unwrap();
        apply_scene_file(
            &mut place,
            &json!({ "creator": "0xAUTHOR", "runtimeVersion": "7.1" }),
            &metadata,
        );
        assert_eq!(place.creator_address.as_deref(), Some("0xauthor"));
        assert_eq!(place.raw["owner"], "0xland");
        assert_eq!(place.raw["sdk"], "7.1");
        assert_eq!(place.raw["categories"], json!(["game", "art", "social"]));
        apply_scene_file(&mut place, &json!({}), &metadata);
        assert!(place.creator_address.is_none());
        assert_eq!(place.raw["sdk"], "6");
    }

    #[tokio::test]
    async fn creator_categories_survive_rebuild_and_preserve_editorial_categories() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let token = uuid::Uuid::new_v4().simple().to_string();
        let mut meta = wrapped();
        let base = format!(
            "{},{}",
            u16::from_str_radix(&token[..4], 16).unwrap(),
            u16::from_str_radix(&token[4..8], 16).unwrap()
        );
        meta["v"]["scene"]["base"] = json!(base);
        meta["v"]["scene"]["parcels"] = json!([base]);
        let place = derive("0xauthor", &[], None, &meta, None, "/content", true).unwrap();
        let id = place.id.clone();
        upsert_page(&pool, vec![place]).await.unwrap();
        sqlx::query("UPDATE explore.place SET categories = array_append(categories, 'featured') WHERE id = $1").bind(&id).execute(&pool).await.unwrap();
        meta["v"]["tags"] = json!(["music"]);
        upsert_page(
            &pool,
            vec![derive("0xauthor", &[], None, &meta, None, "/content", true).unwrap()],
        )
        .await
        .unwrap();
        let mut categories: Vec<String> =
            sqlx::query_scalar("SELECT categories FROM explore.place WHERE id = $1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        categories.sort();
        assert_eq!(categories, vec!["featured", "music"]);
    }

    #[test]
    fn the_deployment_time_fills_both_timestamps_upstream_keeps_not_null() {
        let at = DateTime::parse_from_rfc3339("2026-03-01T10:00:00Z")
            .expect("parse")
            .naive_utc();
        let p = derive("0x1", &[], Some(at), &wrapped(), None, "/content", false).expect("derives");
        let wire = json!(DateTime::<Utc>::from_naive_utc_and_offset(at, Utc));
        assert_eq!(p.raw["deployed_at"], wire);
        assert_eq!(p.raw["created_at"], wire);
        assert_eq!(p.raw["updated_at"], wire);
    }

    #[test]
    fn id_is_stable_for_base_position() {
        let a = derive("0x1", &[], None, &wrapped(), None, "/content", false).unwrap();
        let b = derive("0x2", &[], None, &wrapped(), None, "/content", false).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.id, super::super::parcel_place_id("135,2"));
    }

    #[test]
    fn no_scene_yields_none() {
        assert!(derive("0x1", &[], None, &json!({"v": {}}), None, "/content", false).is_none());
    }

    #[test]
    fn falls_back_to_pointer_when_base_missing() {
        let meta = json!({ "v": { "scene": { "parcels": [] } } });
        let p = derive(
            "0x1",
            &["10,10".to_string()],
            None,
            &meta,
            None,
            "/content",
            false,
        )
        .unwrap();
        assert_eq!(p.base_position, "10,10");
        assert_eq!(p.raw["positions"], json!(["10,10"]));
    }

    #[test]
    fn the_scope_follows_the_old_mode_and_the_mirror() {
        assert_eq!(DeriveScope::resolve(true, false), DeriveScope::All);
        assert_eq!(DeriveScope::resolve(true, true), DeriveScope::Local);
        assert_eq!(DeriveScope::resolve(false, false), DeriveScope::Local);
        assert_eq!(DeriveScope::resolve(false, true), DeriveScope::Local);
    }

    const CREATOR: &str = "0x4f7fe261619141ffa63fefee35bba886581292f4";

    fn banner() -> Curation {
        Curation {
            highlighted: true,
            ranking: Some(1900.0),
            exclude_from_ranking: false,
            highlighted_image: Some("/images/places/banner.jpg".to_string()),
        }
    }

    fn candidate(creator: &str, curation: Curation) -> CurationCandidate {
        CurationCandidate {
            creator_address: Some(creator.to_string()),
            curation,
        }
    }

    #[test]
    fn carries_curation_from_a_predecessor_by_the_same_creator() {
        assert_eq!(
            inherited_curation(&[candidate(CREATOR, banner())], Some(CREATOR)),
            Some(banner())
        );
    }

    #[test]
    fn a_ranked_but_unhighlighted_predecessor_counts_as_curated() {
        let ranked = Curation {
            highlighted: false,
            ranking: Some(12.0),
            exclude_from_ranking: false,
            highlighted_image: None,
        };
        assert_eq!(
            inherited_curation(&[candidate(CREATOR, ranked.clone())], Some(CREATOR)),
            Some(ranked)
        );
    }

    #[test]
    fn an_excluded_but_unranked_predecessor_counts_as_curated() {
        let excluded = Curation {
            highlighted: false,
            ranking: Some(0.0),
            exclude_from_ranking: true,
            highlighted_image: None,
        };
        assert_eq!(
            inherited_curation(&[candidate(CREATOR, excluded.clone())], Some(CREATOR)),
            Some(excluded)
        );
    }

    #[test]
    fn a_predecessor_by_another_creator_inherits_nothing() {
        assert_eq!(
            inherited_curation(
                &[candidate(CREATOR, banner())],
                Some("0x0000000000000000000000000000000000000001")
            ),
            None
        );
    }

    #[test]
    fn more_than_one_curated_predecessor_bails_out() {
        assert_eq!(
            inherited_curation(
                &[candidate(CREATOR, banner()), candidate(CREATOR, banner())],
                Some(CREATOR)
            ),
            None
        );
    }

    #[test]
    fn uncurated_neighbours_do_not_count_towards_the_bail_out() {
        let plain = Curation {
            highlighted: false,
            ranking: Some(0.0),
            exclude_from_ranking: false,
            highlighted_image: None,
        };
        assert_eq!(
            inherited_curation(
                &[candidate(CREATOR, banner()), candidate(CREATOR, plain)],
                Some(CREATOR)
            ),
            Some(banner())
        );
    }

    #[test]
    fn a_deployment_without_a_known_creator_inherits_nothing() {
        assert_eq!(
            inherited_curation(&[candidate(CREATOR, banner())], None),
            None
        );
        assert_eq!(
            inherited_curation(&[candidate(CREATOR, banner())], Some("")),
            None
        );
    }

    async fn scene(
        pool: &PgPool,
        entity_id: &str,
        base: &str,
        parcels: &[&str],
        deleter: Option<i32>,
    ) -> i32 {
        let meta = json!({ "v": { "scene": { "base": base, "parcels": parcels }, "display": { "title": format!("scene {entity_id}") } } });
        sqlx::query_scalar(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata,
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain, deleter_deployment)
             VALUES ('0xAbC', 'v3', 'scene', $1, $2::json, timezone('UTC', now()), $3, timezone('UTC', now()), '[]', $4)
             RETURNING id",
        )
        .bind(entity_id)
        .bind(meta.to_string())
        .bind(parcels.iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .bind(deleter)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// The local scope derives this node's own scenes (not a synced one, not
    /// a tombstoned one), a mirrored row at the same id is left alone, the
    /// places mirror's retire keeps the local rows, and an unpublish prunes
    /// the place on the next pass.
    #[tokio::test]
    async fn local_scenes_become_places_beside_the_mirror_and_leave_when_unpublished() {
        let Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            return;
        };
        let opts: sqlx::postgres::PgConnectOptions = base.parse().expect("a postgres DSN");
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_z_derive_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect_with(opts.database(&name))
            .await
            .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        let t = uuid::Uuid::new_v4().simple().to_string();
        let (x, y) = (
            i64::from_str_radix(&t[..4], 16).unwrap() % 150,
            i64::from_str_radix(&t[4..8], 16).unwrap() % 150,
        );
        let (local_base, synced_base, gone_base) = (
            format!("{x},{y}"),
            format!("{x},{}", -y - 1),
            format!("{},{y}", -x - 1),
        );
        let (local_id, synced_id, gone_id) = (
            format!("bafylocal{t}"),
            format!("bafysynced{t}"),
            format!("bafygone{t}"),
        );
        scene(&pool, &local_id, &local_base, &[&local_base], None).await;
        scene(&pool, &synced_id, &synced_base, &[&synced_base], None).await;
        scene(&pool, &gone_id, &gone_base, &[&gone_base], None).await;
        for id in [&local_id, &gone_id] {
            sqlx::query(
                "INSERT INTO content.local_entities (entity_id, signer) VALUES ($1, '0xabc')",
            )
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let place = |base: &str| super::super::parcel_place_id(base);
        let row = |id: String| {
            let pool = pool.clone();
            async move {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT raw->>'source', origin FROM explore.place WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(&pool)
                .await
                .unwrap()
            }
        };

        let out = run_once(&pool, DeriveScope::Local, "https://node.example/content")
            .await
            .unwrap();
        assert_eq!(
            out,
            PassSummary {
                derived: 2,
                pruned: 0
            }
        );
        assert_eq!(
            row(place(&local_base)).await,
            Some(("content".into(), "local".into()))
        );
        assert_eq!(
            row(place(&gone_base)).await,
            Some(("content".into(), "local".into()))
        );
        assert_eq!(
            row(place(&synced_base)).await,
            None,
            "a synced scene is the mirror's or nobody's in the local scope"
        );

        // a binary from before `origin` derived one too: the default made it
        // 'upstream', and its raw.local is what the mirror goes by
        let legacy = format!("legacy-derived-{t}");
        sqlx::query(
            "INSERT INTO explore.place (id, base_position, raw, fetched_at) \
             VALUES ($1, '-150,-150', '{\"source\": \"content\", \"local\": true}', \
                     timezone('UTC', now()) - interval '1 day')",
        )
        .bind(&legacy)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            row(legacy.clone()).await,
            Some(("content".into(), "upstream".into()))
        );

        // the places mirror's retire keeps the local rows, and so does its
        // sweep however stale the row is (a stalled derive job)
        super::super::mirror::retire_derived(&pool).await.unwrap();
        assert!(row(place(&local_base)).await.is_some());
        assert_eq!(
            row(legacy.clone()).await,
            Some(("content".into(), "local".into())),
            "reclassified, not retired"
        );
        sqlx::query(
            "UPDATE explore.place SET fetched_at = fetched_at - interval '1 day' WHERE id = $1",
        )
        .bind(place(&local_base))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE explore.place SET origin = 'upstream' WHERE id = $1")
            .bind(&legacy)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            super::super::mirror::sweep_removed(
                &pool,
                super::super::mirror::Leg::Places,
                Utc::now() - chrono::Duration::hours(1)
            )
            .await
            .unwrap(),
            super::super::mirror::SweepOutcome::Swept(0)
        );
        assert!(row(place(&local_base)).await.is_some());
        assert_eq!(
            row(legacy.clone()).await,
            Some(("content".into(), "local".into())),
            "reclassified, not swept"
        );

        // unpublished: the next pass prunes it
        sqlx::query("UPDATE content.local_entities SET tombstoned_at = timezone('UTC', now()) WHERE entity_id = $1").bind(&gone_id).execute(&pool).await.unwrap();
        run_once(&pool, DeriveScope::Local, "https://node.example/content")
            .await
            .unwrap();
        assert_eq!(row(place(&gone_base)).await, None);
        assert!(row(place(&local_base)).await.is_some());

        // the whole-catalogue scope derives the synced scene too
        run_once(&pool, DeriveScope::All, "https://node.example/content")
            .await
            .unwrap();
        assert_eq!(
            row(place(&synced_base)).await,
            Some(("content".into(), "upstream".into()))
        );

        // a mirrored row at a derived id is the mirror's
        sqlx::query("UPDATE explore.place SET raw = raw || '{\"source\": \"mirror\"}'::jsonb, title = 'upstream', origin = 'upstream' WHERE id = $1").bind(place(&local_base)).execute(&pool).await.unwrap();
        run_once(&pool, DeriveScope::Local, "https://node.example/content")
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM explore.place WHERE id = $1")
            .bind(place(&local_base))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(title, "upstream");

        let _ = (&synced_id, &local_id);
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await
        .unwrap();
    }

    async fn world(pool: &PgPool, name: &str, mirror: Option<&str>, access: &str) {
        sqlx::query(
            "INSERT INTO worlds.worlds (name, owner, title, description, content_rating, categories,
                 show_in_places, thumbnail_hash, access_type, access_secret_hash, mirror_source)
             VALUES ($1, '0x00000000000000000000000000000000000000aa', 'Title ' || $1, 'about it',
                     'T', '{game}', true, 'bafythumb', $2,
                     CASE WHEN $2 = 'shared-secret' THEN 'hash' END, $3)",
        )
        .bind(name)
        .bind(access)
        .bind(mirror)
        .execute(pool)
        .await
        .unwrap();
        let entity = json!({ "metadata": {
            "scene": { "base": "1,1", "parcels": ["1,1", "1,2"] },
            "contact": { "name": "builder" },
            "worldConfiguration": { "name": name.to_uppercase() },
        }});
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity,
                 deployer, parcels, size, mirror_source)
             VALUES ($1, 'bafy' || $1, '[]', $2, '0xaa', '{\"1,1\",\"1,2\"}', 1, $3)",
        )
        .bind(name)
        .bind(entity)
        .bind(mirror)
        .execute(pool)
        .await
        .unwrap();
    }

    /// A world this node publishes is a catalogue row of the local leg, keyed by
    /// its name as upstream keys worlds, and the only row of that world: the
    /// mirrored one goes and the places mirror does not bring it back. A world
    /// only mirrored is never derived; blocking the world prunes the row until
    /// it is unblocked, as the worlds listing hides it; unpublishing the last
    /// local scene prunes the row and the mirror lists the world again.
    #[tokio::test]
    async fn local_worlds_reach_the_catalogue_once_and_leave_when_blocked_or_unpublished() {
        let Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            return;
        };
        let opts: sqlx::postgres::PgConnectOptions = base.parse().expect("a postgres DSN");
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_z_derive_worlds_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect_with(opts.database(&name))
            .await
            .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();

        let upstream = |id: &str| json!({ "id": id, "world_name": id, "title": "upstream" });
        for id in ["local.dcl.eth", "mirrored.dcl.eth"] {
            super::super::mirror::upsert_world(&pool, &upstream(id))
                .await
                .unwrap();
        }
        world(&pool, "local.dcl.eth", None, "unrestricted").await;
        world(&pool, "secret.dcl.eth", None, "shared-secret").await;
        world(
            &pool,
            "mirrored.dcl.eth",
            Some("https://worlds.example"),
            "unrestricted",
        )
        .await;
        let listed = |world: &str| {
            let pool = pool.clone();
            let world = world.to_string();
            async move {
                sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
                    "SELECT id, raw->>'source', title FROM explore.place_indexed
                     WHERE world IS TRUE AND lower(world_name) = $1 ORDER BY id",
                )
                .bind(world)
                .fetch_all(&pool)
                .await
                .unwrap()
            }
        };
        let local = |title: &str| {
            vec![(
                "local.dcl.eth".to_string(),
                Some("worlds".to_string()),
                Some(title.to_string()),
            )]
        };
        let mirrored = |id: &str| vec![(id.to_string(), None, Some("upstream".to_string()))];

        run_once(&pool, DeriveScope::Local, "https://node.example/content/")
            .await
            .unwrap();
        assert_eq!(listed("local.dcl.eth").await, local("Title local.dcl.eth"));
        assert_eq!(
            listed("mirrored.dcl.eth").await,
            mirrored("mirrored.dcl.eth")
        );
        let (world_name, owner, image, private, base): (String, String, String, bool, String) =
            sqlx::query_as(
                "SELECT world_name, creator_address, raw->>'image', (raw->>'is_private')::boolean,
                        base_position
                 FROM explore.place_world_local WHERE id = 'local.dcl.eth'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            (
                world_name.as_str(),
                owner.as_str(),
                image.as_str(),
                private,
                base.as_str()
            ),
            (
                "LOCAL.DCL.ETH",
                "0x00000000000000000000000000000000000000aa",
                "https://node.example/content/contents/bafythumb",
                false,
                "1,1"
            )
        );
        let private: bool = sqlx::query_scalar(
            "SELECT (raw->>'is_private')::boolean FROM explore.place_world_local WHERE id = 'secret.dcl.eth'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(private, "an access-restricted world is private");

        // the places mirror's next pass leaves the local world alone
        super::super::mirror::upsert_world(&pool, &upstream("local.dcl.eth"))
            .await
            .unwrap();
        assert_eq!(listed("local.dcl.eth").await, local("Title local.dcl.eth"));

        // a settings change shows on the next pass
        sqlx::query("UPDATE worlds.worlds SET title = 'Renamed' WHERE name = 'local.dcl.eth'")
            .execute(&pool)
            .await
            .unwrap();
        run_once(&pool, DeriveScope::Local, "https://node.example/content")
            .await
            .unwrap();
        assert_eq!(listed("local.dcl.eth").await, local("Renamed"));

        // a blocked world leaves the catalogue until it is unblocked, and the
        // places mirror's passes meanwhile do not list upstream's copy of it
        for (blocked, expected) in [
            ("timezone('UTC', now())", vec![]),
            ("NULL", local("Renamed")),
        ] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE worlds.worlds SET blocked_since = {blocked} WHERE name = 'local.dcl.eth'"
            )))
            .execute(&pool)
            .await
            .unwrap();
            run_once(&pool, DeriveScope::Local, "https://node.example/content")
                .await
                .unwrap();
            super::super::mirror::upsert_world(&pool, &upstream("local.dcl.eth"))
                .await
                .unwrap();
            assert_eq!(
                listed("local.dcl.eth").await,
                expected,
                "blocked_since {blocked}"
            );
        }

        // the last local scene unpublished: pruned, and the mirror lists it again
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = 'local.dcl.eth'")
            .execute(&pool)
            .await
            .unwrap();
        run_once(&pool, DeriveScope::Local, "https://node.example/content")
            .await
            .unwrap();
        assert_eq!(listed("local.dcl.eth").await, vec![]);
        super::super::mirror::upsert_world(&pool, &upstream("local.dcl.eth"))
            .await
            .unwrap();
        assert_eq!(listed("local.dcl.eth").await, mirrored("local.dcl.eth"));

        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await
        .unwrap();
    }
}
