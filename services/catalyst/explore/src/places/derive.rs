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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

use super::moderation::{self, CreatorRating, RatedRow};
use super::query::{EXCLUDE_FROM_RANKING_SQL, raw_float8_sql};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::land_authz::LandAuthzState;

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
    // Stored as a rating code (`M` as `A`); an unknown code reads as none.
    let content_rating = non_empty(
        m.get("policy")
            .and_then(|p| p.get("contentRating"))
            .and_then(Value::as_str),
    )
    .and_then(|c| moderation::creator_code(&c))
    .map(str::to_string);
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
    // Curation identity comes from the signed deployment, never scene-authored metadata.
    let creator_address = non_empty(Some(deployer_address)).map(|s| s.to_lowercase());
    let owner = creator_address.clone();
    let categories = creator_categories(&tags);
    let id = super::parcel_place_id(&base);
    let at = deployed_at.map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc));
    let mut raw = json!({
        "id": id,
        "title": title,
        "description": description,
        "owner": owner,
        "metadata_owner": non_empty(m.get("owner").and_then(Value::as_str)),
        "metadata_creator": non_empty(m.get("creator").and_then(Value::as_str)),
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
/// and `content_rating` of a new row; an existing row keeps its own. The
/// creator rating rule then settles the rating of a new deployment or a new
/// row ([`upsert_page`]). A row the
/// places mirror wrote is left to the mirror.
const UPSERT: &str = r#"
    INSERT INTO explore.place
        (id, base_position, title, description, creator_address, content_rating,
         categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
         raw, fetched_at, origin)
    SELECT u.id, u.base_position, u.title, u.description, u.creator_address,
           -- a scene's policy.contentRating may spell pending PR; the stored
           -- code is RP (places::moderation)
           COALESCE(m.content_rating, CASE WHEN u.content_rating = 'PR' THEN 'RP' ELSE u.content_rating END),
           ARRAY(SELECT jsonb_array_elements_text(u.raw->'categories')), 0, 0, 0,
           u.deployed_at, COALESCE(m.disabled, false), COALESCE(m.highlighted, false),
           u.raw || COALESCE(r.overlay, '{}'::jsonb)
               -- a new row disabled by an operator carries the disable's instant and reason
               || CASE WHEN m.disabled IS TRUE THEN jsonb_build_object(
                           'disabled_at', to_char(m.disabled_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                           'disabled_reason', m.reason)
                       ELSE '{}'::jsonb END,
           timezone('UTC', now()), u.origin
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
        categories = CASE WHEN cardinality(EXCLUDED.categories) = 0 THEN explore.place.categories ELSE
            ARRAY(SELECT c FROM unnest(EXCLUDED.categories || explore.place.categories) WITH ORDINALITY AS tags(c, position)
                  WHERE c IN ('poi', 'featured') OR c = ANY(EXCLUDED.categories)
                  GROUP BY c ORDER BY min(position)) END,
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
           u.raw || COALESCE(r.overlay, '{}'::jsonb)
               -- an operator's disable carries its instant and reason (places::moderation)
               || CASE WHEN m.disabled IS TRUE THEN jsonb_build_object(
                           'disabled_at', to_char(m.disabled_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                           'disabled_reason', m.reason)
                       ELSE '{}'::jsonb END,
           timezone('UTC', now()), u.world_name, true
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
    run_once_with_storage(pool, scope, content_public_url, None, None, None).await
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
    place.raw["metadata_creator"] = json!(
        non_empty(scene.get("creator").and_then(Value::as_str))
            .or_else(|| non_empty(scene_json(metadata).get("creator").and_then(Value::as_str)))
    );
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
    notify: Option<&str>,
    land: Option<&LandAuthzState>,
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
        let mut candidates = Vec::with_capacity(rows.len());
        for row in &rows {
            last_id = row.try_get("id")?;
            let deployer: String = row.try_get("deployer_address").unwrap_or_default();
            let pointers: Vec<String> = row.try_get("entity_pointers").unwrap_or_default();
            let deployed_at: Option<NaiveDateTime> = row.try_get("deployed_at").unwrap_or(None);
            let meta: Value = row.try_get("meta").unwrap_or(Value::Null);
            let local: bool = row.try_get("local").unwrap_or(false);
            let thumb: Option<String> = row.try_get("thumbnail_hash").unwrap_or(None);
            if let Some(place) = derive(
                &deployer,
                &pointers,
                deployed_at,
                &meta,
                thumb.as_deref(),
                content_public_url,
                local,
            ) {
                candidates.push((
                    place,
                    row.try_get::<String, _>("entity_id")?,
                    row.try_get::<Option<String>, _>("scene_json_hash")?,
                    meta,
                ));
            }
        }
        // One primary-key lookup per bounded page. Immutable scene blobs are re-read only
        // when their deployment or hash changes; a failed read is retried next pass.
        let ids: Vec<&str> = candidates
            .iter()
            .map(|(place, ..)| place.id.as_str())
            .collect();
        let cached: HashMap<String, Value> = sqlx::query_as::<_, (String, Value)>(
            "SELECT id, raw FROM explore.place WHERE id = ANY($1) AND raw->>'source' = 'content'",
        )
        .bind(&ids)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
        let mut page = Vec::with_capacity(candidates.len());
        let mut failed = Vec::new();
        for (mut place, entity, hash, meta) in candidates {
            if let (Some(storage), Some(hash)) = (storage, hash) {
                let previous = cached.get(&place.id).filter(|raw| {
                    raw["scene_metadata_entity"].as_str() == Some(entity.as_str())
                        && raw["scene_metadata_hash"].as_str() == Some(hash.as_str())
                });
                if let Some(previous) = previous {
                    for key in ["sdk", "metadata_creator"] {
                        place.raw[key] = previous[key].clone();
                    }
                } else {
                    match scene_file(storage, &entity, &hash).await {
                        Ok(Some(scene)) => apply_scene_file(&mut place, &scene, &meta),
                        Ok(None) => {
                            // Missing or withheld blobs may become readable without redeployment.
                            page.push(place);
                            derived += 1;
                            continue;
                        }
                        Err(error) => {
                            tracing::warn!(%entity, %hash, %error, "skipping place with unreadable scene metadata");
                            failed.push(place.id);
                            continue;
                        }
                    }
                }
                place.raw["scene_metadata_entity"] = json!(entity);
                place.raw["scene_metadata_hash"] = json!(hash);
            }
            page.push(place);
            derived += 1;
        }
        // Preserve the last usable catalogue row on transient I/O failures; pruning still
        // removes deployments absent from this pass, and failed rows remain due next pass.
        if !failed.is_empty() {
            sqlx::query("UPDATE explore.place SET fetched_at = timezone('UTC', clock_timestamp()) WHERE id = ANY($1) AND raw->>'source' = 'content'")
                .bind(&failed).execute(pool).await?;
        }
        upsert_page(pool, page, notify, land).await?;
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

/// A derived row's rating before this page: `(content_rating, deployed_at)`
/// by id, locked until the page commits.
const LOCK_RATINGS: &str = "SELECT id, content_rating, deployed_at FROM explore.place \
     WHERE id = ANY($1) AND raw->>'source' = 'content' ORDER BY id FOR UPDATE";

/// The ratings a new deployment or a new row is held to: its own operator
/// rating; the rating recorded on each of its parcels by the same place (the
/// same id or exactly the same parcel set), the same creator or under the same
/// LAND owner; and the rating of every other derived place by the same creator
/// its parcels overlap, or with exactly its parcels (the rows it replaces,
/// which `PRUNE` removes; that row's operator rating first), disabled ones
/// included. A place by another creator on another owner's LAND and another
/// parcel set starts fresh; an empty creator or owner matches none. `$1` the
/// ids, `$2` each one's positions; `$3` to `$7` one entry per parcel: the id,
/// the position, the lowercase creator, the parcel's LAND owner and the
/// place's parcel set ([`parcel_set`]). The overlapping places are found one
/// parcel at a time (`@>` on one position, behind `OFFSET 0`), so each probe
/// uses `place_positions_path_gin`.
const HELD_RATINGS: &str = r#"
    SELECT f.id, m.content_rating AS rating
    FROM unnest($1::text[]) AS f(id)
    JOIN explore.place_moderation m ON m.place_id = f.id AND m.content_rating IS NOT NULL
    UNION ALL
    SELECT c.id, r.content_rating
    FROM unnest($3::text[], $4::text[], $5::text[], $6::text[], $7::text[])
         AS c(id, position, creator, owner, parcel_set)
    JOIN explore.place_parcel_rating r ON r.position = c.position
     AND (r.place_id = c.id OR r.parcel_set_hash = c.parcel_set
          OR (c.creator <> '' AND r.creator_address = c.creator)
          OR (c.owner <> '' AND r.owner_address = c.owner))
    UNION ALL
    SELECT c.id, COALESCE(m.content_rating, p.content_rating)
    FROM unnest($3::text[], $4::text[], $5::text[]) AS c(id, position, creator)
    CROSS JOIN LATERAL (
        SELECT p.id, p.creator_address, p.content_rating, p.raw FROM explore.place p
        WHERE p.raw->'positions' @> jsonb_build_array(c.position)
          AND p.world IS FALSE AND p.raw->>'source' = 'content'
        OFFSET 0) p
    JOIN unnest($1::text[], $2::jsonb[]) AS f(id, positions) ON f.id = c.id
    LEFT JOIN explore.place_moderation m ON m.place_id = p.id
    WHERE p.id <> c.id
      AND ((c.creator <> '' AND lower(btrim(p.creator_address)) = c.creator)
           OR ((p.raw->'positions') @> f.positions AND f.positions @> (p.raw->'positions')))
"#;

/// The page's parcel ratings, locked after its places as an operator's
/// rating locks them (`moderation::apply_with`).
const LOCK_PARCEL_RATINGS: &str = "SELECT 1 FROM explore.place_parcel_rating \
     WHERE position = ANY($1) ORDER BY position FOR UPDATE";

/// Of the parcels `$1` of the unchanged places `$2`: those with no recorded
/// rating of a rated place (`$4`; `missing`, which the page fills), and with
/// a LAND index (`$3`) those recorded with an unknown owner by their own
/// place, or with a known owner by another place (`missing` false). Either
/// means the place was settled while a parcel's owner was unknown; once the
/// index knows it the page settles the place again. An unlocked read: the
/// page then locks only what it writes.
const PARCEL_STATES: &str = "SELECT u.pos, u.id, r.position IS NULL AS missing \
     FROM unnest($1::text[], $2::text[], $4::bool[]) AS u(pos, id, rated) \
     LEFT JOIN explore.place_parcel_rating r ON r.position = u.pos \
     WHERE (r.position IS NULL AND u.rated) \
        OR ($3 AND ((r.place_id = u.id AND r.owner_address IS NULL) \
                 OR (r.place_id <> u.id AND r.owner_address IS NOT NULL)))";

/// Locks the parcels a page writes, recorded or not, as an operator's rating
/// does (`moderation::LOCK_PARCELS`), then the recorded rows.
async fn lock_page_parcels(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    parcels: &[String],
) -> Result<()> {
    for sql in [moderation::LOCK_PARCELS, LOCK_PARCEL_RATINGS] {
        sqlx::query(sql).bind(parcels).execute(&mut **tx).await?;
    }
    Ok(())
}

/// Records `$2` on each parcel `$1` for the place `$3` by the creator `$4`
/// under the LAND owner `$6`, with the place's parcel set `$7`: `$5` true
/// overwrites a recorded rating, false only fills a parcel with none. A place
/// with an operator rating records an operator's rating. An operator's record
/// is replaced only by a place held to it ([`HELD_RATINGS`], which keeps it an
/// operator's) or under a known owner other than its known one; another
/// place with an unknown owner replaces no record with a known owner, which
/// the page settles again once the owner is known ([`PARCEL_STATES`]). An
/// unknown owner keeps the owner the same place (or parcel set) recorded, as
/// `moderation::OPERATOR_PARCEL_RATINGS` does.
const RECORD_PARCEL_RATINGS: &str = r#"
    INSERT INTO explore.place_parcel_rating AS r
        (position, content_rating, place_id, parcel_set_hash, creator_address, owner_address, operator)
    SELECT DISTINCT ON (u.position) u.position, u.rating, u.place_id, u.parcel_set,
           NULLIF(lower(btrim(u.creator)), ''), NULLIF(u.owner, ''),
           EXISTS (SELECT 1 FROM explore.place_moderation m
                   WHERE m.place_id = u.place_id AND m.content_rating IS NOT NULL)
    FROM unnest($1::text[], $2::text[], $3::text[], $4::text[], $6::text[], $7::text[])
         AS u(position, rating, place_id, creator, owner, parcel_set)
    ORDER BY u.position
    ON CONFLICT (position) DO UPDATE
    SET content_rating = EXCLUDED.content_rating, place_id = EXCLUDED.place_id,
        parcel_set_hash = EXCLUDED.parcel_set_hash,
        creator_address = EXCLUDED.creator_address,
        owner_address = COALESCE(EXCLUDED.owner_address,
            CASE WHEN r.place_id = EXCLUDED.place_id OR r.parcel_set_hash = EXCLUDED.parcel_set_hash
                 THEN r.owner_address END),
        operator = EXCLUDED.operator OR (r.operator AND COALESCE(
            r.place_id = EXCLUDED.place_id OR r.parcel_set_hash = EXCLUDED.parcel_set_hash
            OR r.creator_address = EXCLUDED.creator_address
            OR r.owner_address = EXCLUDED.owner_address, false)),
        updated_at = timezone('UTC', now())
    WHERE $5
      AND (r.place_id = EXCLUDED.place_id OR r.parcel_set_hash = EXCLUDED.parcel_set_hash
           OR r.creator_address = EXCLUDED.creator_address
           OR (r.owner_address IS NOT NULL AND EXCLUDED.owner_address IS NOT NULL)
           OR (NOT r.operator AND r.owner_address IS NULL))
      AND (r.content_rating, r.place_id, r.parcel_set_hash, r.creator_address, r.owner_address)
          IS DISTINCT FROM (EXCLUDED.content_rating, EXCLUDED.place_id, EXCLUDED.parcel_set_hash,
                            EXCLUDED.creator_address,
                            COALESCE(EXCLUDED.owner_address,
                                CASE WHEN r.place_id = EXCLUDED.place_id
                                       OR r.parcel_set_hash = EXCLUDED.parcel_set_hash
                                     THEN r.owner_address END))
"#;

/// Clears the parcels `$1` of a place left with no rating, so a later place
/// on them is held to none it did not hold. Such a place is held to no
/// record. A record with a known owner stays while `$2`, the parcel's LAND
/// owner, is unknown; an operator's record stays unless both owners are known
/// and differ (the LAND changed owner) and the record is of another parcel
/// set than `$3`.
const CLEAR_PARCEL_RATINGS: &str = "DELETE FROM explore.place_parcel_rating r \
     USING unnest($1::text[], $2::text[], $3::text[]) AS c(position, owner, parcel_set) \
     WHERE r.position = c.position \
       AND ((NOT r.operator AND (r.owner_address IS NULL OR c.owner <> '')) \
            OR (r.owner_address IS NOT NULL AND c.owner <> '' \
                AND r.owner_address <> c.owner AND r.parcel_set_hash <> c.parcel_set))";

/// The page's operator rows, locked before its places as an operator's
/// decision locks them (`moderation::apply_with`).
const LOCK_OVERLAYS: &str = "SELECT 1 FROM explore.place_moderation \
     WHERE place_id = ANY($1) ORDER BY place_id FOR UPDATE";

const SET_RATINGS: &str = "UPDATE explore.place SET content_rating = u.rating \
     FROM unnest($1::text[], $2::text[]) AS u(id, rating) WHERE explore.place.id = u.id";

/// An operator's rating follows the rating the row takes, so a later
/// re-insert of the row (`UPSERT`'s overlay) keeps it.
const FOLLOW_RATINGS: &str = "UPDATE explore.place_moderation m SET content_rating = u.rating \
     FROM unnest($1::text[], $2::text[]) AS u(id, rating) \
     WHERE m.place_id = u.id AND m.content_rating IS NOT NULL \
       AND m.content_rating IS DISTINCT FROM u.rating";

/// One transaction per page: a multi-row upsert, the creator rating rule
/// ([`settle_ratings`]), one overlap query for the fresh rows, and one batched
/// curation update. Later duplicates of an id win, as they did row by row.
async fn upsert_page(
    pool: &PgPool,
    page: Vec<DerivedPlace>,
    notify: Option<&str>,
    land: Option<&LandAuthzState>,
) -> Result<()> {
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
    sqlx::query(LOCK_OVERLAYS)
        .bind(rows.iter().map(|p| p.id.clone()).collect::<Vec<_>>())
        .execute(&mut *tx)
        .await?;
    let before: HashMap<String, (Option<String>, Option<NaiveDateTime>)> =
        sqlx::query(LOCK_RATINGS)
            .bind(rows.iter().map(|p| p.id.clone()).collect::<Vec<_>>())
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| {
                (
                    r.try_get::<String, _>("id").unwrap_or_default(),
                    (
                        r.try_get("content_rating").unwrap_or(None),
                        r.try_get("deployed_at").unwrap_or(None),
                    ),
                )
            })
            .collect();
    let PageParcels {
        locked: parcels,
        owners,
        recheck,
    } = page_parcels(&mut tx, &rows, &before, land).await?;
    lock_page_parcels(&mut tx, &parcels).await?;
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
    let page = Page {
        before: &before,
        inserted: &inserted,
        locked: &parcels,
        owners: &owners,
        recheck: &recheck,
    };
    settle_ratings(&mut tx, &rows, &page, notify).await?;
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

/// What a page writes and reads before it settles: the parcels it may
/// write, sorted and locked; their LAND owners (one lookup per page); and the
/// unchanged places it settles again.
struct PageParcels {
    locked: Vec<String>,
    owners: HashMap<String, String>,
    recheck: HashSet<String>,
}

/// The parcels a page may write: those of a new row or a new deployment;
/// the unrecorded ones of a rated row it fills; and every parcel of an
/// unchanged row it settles again ([`PARCEL_STATES`]: one settled while a
/// parcel's owner was unknown, once the index knows it). Only these are
/// locked, so a page of unchanged rows does not hold an operator's rating of
/// another place. One owner lookup covers them and the parcels whose owner
/// decides a second settle; a parcel the index never knows is looked up
/// again on each pass.
async fn page_parcels(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rows: &[&DerivedPlace],
    before: &HashMap<String, (Option<String>, Option<NaiveDateTime>)>,
    land: Option<&LandAuthzState>,
) -> Result<PageParcels> {
    let changed = |p: &DerivedPlace| before.get(&p.id).is_none_or(|(_, at)| *at != p.deployed_at);
    let mut parcels: Vec<String> = rows
        .iter()
        .filter(|p| changed(p))
        .flat_map(|p| positions_of(p))
        .collect();
    let (mut positions, mut places, mut rated) = (Vec::new(), Vec::new(), Vec::new());
    for p in rows.iter().filter(|p| !changed(p)) {
        let is_rated = before
            .get(&p.id)
            .and_then(|(rating, _)| rating.as_deref())
            .and_then(moderation::creator_code)
            .is_some();
        if !is_rated && land.is_none() {
            continue;
        }
        for position in positions_of(p) {
            positions.push(position);
            places.push(p.id.clone());
            rated.push(is_rated);
        }
    }
    let mut suspect: HashMap<String, Vec<String>> = HashMap::new();
    if !positions.is_empty() {
        for r in sqlx::query(PARCEL_STATES)
            .bind(&positions)
            .bind(&places)
            .bind(land.is_some())
            .bind(&rated)
            .fetch_all(&mut **tx)
            .await?
        {
            let position: String = r.try_get("pos")?;
            if r.try_get::<bool, _>("missing")? {
                parcels.push(position);
            } else {
                suspect.entry(r.try_get("id")?).or_default().push(position);
            }
        }
    }
    let of = |id: &str| {
        rows.iter()
            .find(|p| p.id == id)
            .map(|p| positions_of(p))
            .unwrap_or_default()
    };
    let mut lookup = parcels.clone();
    for id in suspect.keys() {
        lookup.extend(of(id));
    }
    lookup.sort();
    lookup.dedup();
    let owners = moderation::parcel_owners(tx, land, &lookup).await?;
    let recheck: HashSet<String> = suspect
        .into_iter()
        .filter(|(_, at)| at.iter().any(|p| owners.contains_key(p)))
        .map(|(id, _)| id)
        .collect();
    for id in &recheck {
        parcels.extend(of(id));
    }
    parcels.sort();
    parcels.dedup();
    Ok(PageParcels {
        locked: parcels,
        owners,
        recheck,
    })
}

/// A page's state before its ratings settle: each row's rating and
/// deployment time before it, which rows the upsert inserted, the locked
/// parcels, their LAND owners and the unchanged rows settled again.
struct Page<'a> {
    before: &'a HashMap<String, (Option<String>, Option<NaiveDateTime>)>,
    inserted: &'a HashMap<String, bool>,
    locked: &'a [String],
    owners: &'a HashMap<String, String>,
    recheck: &'a HashSet<String>,
}

/// The creator rating rule (`places::moderation::CreatorRating`) over a page:
/// a new deployment of an existing row, or a new row, takes the scene's rating
/// when it raises the one held, never a laxer one. It is held to its stored
/// or operator rating, and to the ratings of the same place (the same id or
/// exactly the same parcels, whoever deploys it), the same creator or the
/// same LAND owner (`owners`) on its parcels: those recorded there and those
/// of the places it replaces; a place by another creator on another owner's
/// LAND and other parcels starts fresh. The rating it takes is recorded on its
/// parcels, and a place left with no rating clears them, an operator's
/// record only once the LAND changed owner. A pass over the same deployment
/// leaves the stored rating, an operator's included, and only fills its
/// parcels' missing records among `locked`, unless the page settles it again
/// (`recheck`: settled while a parcel's owner was unknown), which may raise
/// it to a rating now held by owner, and then reports only a change. A scene
/// that declares no rating keeps the held one, with no notice.
async fn settle_ratings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rows: &[&DerivedPlace],
    page: &Page<'_>,
    notify: Option<&str>,
) -> Result<()> {
    let Page {
        before,
        inserted,
        locked,
        owners,
        recheck,
    } = *page;
    let redeployed = |p: &DerivedPlace| match inserted.get(&p.id) {
        Some(true) => true,
        Some(false) => before
            .get(&p.id)
            .is_some_and(|(_, at)| *at != p.deployed_at),
        None => false,
    };
    let settled: Vec<&DerivedPlace> = rows
        .iter()
        .copied()
        .filter(|p| redeployed(p) || (inserted.contains_key(&p.id) && recheck.contains(&p.id)))
        .collect();
    let mut held: HashMap<String, Vec<String>> = HashMap::new();
    if !settled.is_empty() {
        let ids: Vec<String> = settled.iter().map(|p| p.id.clone()).collect();
        let positions: Vec<Value> = settled
            .iter()
            .map(|p| Value::from(positions_of(p)))
            .collect();
        let (mut cell_ids, mut cells, mut cell_creators, mut cell_owners, mut cell_sets) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for p in &settled {
            let set = parcel_set(p);
            for position in positions_of(p) {
                cell_ids.push(p.id.clone());
                cell_creators.push(creator_of(p));
                cell_owners.push(owners.get(&position).cloned().unwrap_or_default());
                cell_sets.push(set.clone());
                cells.push(position);
            }
        }
        for r in sqlx::query(HELD_RATINGS)
            .bind(&ids)
            .bind(&positions)
            .bind(&cell_ids)
            .bind(&cells)
            .bind(&cell_creators)
            .bind(&cell_owners)
            .bind(&cell_sets)
            .fetch_all(&mut **tx)
            .await?
        {
            let entry = held
                .entry(r.try_get::<String, _>("id").unwrap_or_default())
                .or_default();
            if let Some(rating) = r.try_get::<Option<String>, _>("rating").unwrap_or(None) {
                entry.push(rating);
            }
        }
    }
    let (mut ids, mut ratings) = (Vec::new(), Vec::new());
    let (mut recorded, mut missing) = (Records::default(), Records::default());
    let (mut cleared, mut cleared_sets): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for p in rows {
        let fresh = inserted.get(&p.id) == Some(&true);
        if !settled.iter().any(|s| s.id == p.id) {
            // A row stored before the rule may hold any code: `M` is recorded
            // as `A`, an off-scale code not at all.
            if let Some(rating) = before
                .get(&p.id)
                .filter(|_| inserted.contains_key(&p.id))
                .and_then(|(rating, _)| rating.as_deref())
                .and_then(moderation::creator_code)
            {
                let set = parcel_set(p);
                for position in positions_of(p) {
                    if locked.binary_search(&position).is_ok() {
                        missing.push(position, rating, p, &set, owners);
                    }
                }
            }
            continue;
        }
        // The rating held, and whether a creator's change to it is reported.
        let held_to = held.remove(&p.id);
        let is_held = held_to.is_some();
        let mut codes = held_to.unwrap_or_default();
        let own = if fresh {
            None
        } else {
            before.get(&p.id).and_then(|(rating, _)| rating.clone())
        };
        codes.extend(own.clone());
        let stored = moderation::strictest(codes.iter().map(String::as_str)).map(str::to_string);
        // Settled again, the deployment was already applied: only a held
        // rating moves it, never past an operator's lowering.
        let declared = p.content_rating.as_deref();
        let rating = match declared.filter(|_| redeployed(p)) {
            Some(d) if CreatorRating::of(stored.as_deref(), d).applies() => Some(d.to_string()),
            _ => stored.clone(),
        };
        let code = |r: &Option<String>| r.as_deref().and_then(moderation::creator_code);
        // Settled again, only a changed rating that refused the scene's is reported.
        let refused = declared.is_some_and(|d| !CreatorRating::of(stored.as_deref(), d).applies());
        let report =
            (!fresh || is_held) && (redeployed(p) || (code(&rating) != code(&own) && refused));
        if let (Some(base), Some(stored), Some(declared), true) =
            (notify, stored.as_deref(), declared, report)
        {
            let row = RatedRow::Place {
                id: &p.id,
                base_position: &p.base_position,
            };
            moderation::capture_creator_rating(tx, base, row, stored, declared).await?;
        }
        // A held `M` (stored before the rule) is taken as `A`.
        let Some(rating) = rating
            .as_deref()
            .and_then(moderation::creator_code)
            .map(str::to_string)
        else {
            let set = parcel_set(p);
            for position in positions_of(p) {
                cleared.push(position);
                cleared_sets.push(set.clone());
            }
            continue;
        };
        let set = parcel_set(p);
        for position in positions_of(p) {
            recorded.push(position, &rating, p, &set, owners);
        }
        if fresh || Some(&rating) != own.as_ref() {
            ids.push(p.id.clone());
            ratings.push(rating);
        }
    }
    if !ids.is_empty() {
        for sql in [SET_RATINGS, FOLLOW_RATINGS] {
            sqlx::query(sql)
                .bind(&ids)
                .bind(&ratings)
                .execute(&mut **tx)
                .await?;
        }
    }
    if !cleared.is_empty() {
        let cleared_owners: Vec<String> = cleared
            .iter()
            .map(|p| owners.get(p).cloned().unwrap_or_default())
            .collect();
        sqlx::query(CLEAR_PARCEL_RATINGS)
            .bind(&cleared)
            .bind(&cleared_owners)
            .bind(&cleared_sets)
            .execute(&mut **tx)
            .await?;
    }
    for (records, overwrite) in [(&recorded, true), (&missing, false)] {
        if !records.positions.is_empty() {
            sqlx::query(RECORD_PARCEL_RATINGS)
                .bind(&records.positions)
                .bind(&records.ratings)
                .bind(&records.places)
                .bind(&records.creators)
                .bind(overwrite)
                .bind(&records.owners)
                .bind(&records.sets)
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(())
}

/// A place's creator as its parcel records keep it: the lowercase deployer
/// address, empty when unknown (which matches no record).
fn creator_of(p: &DerivedPlace) -> String {
    p.creator_address
        .as_deref()
        .map(|c| c.trim().to_lowercase())
        .unwrap_or_default()
}

/// A place's whole parcel set as its parcel records keep it
/// ([`super::parcel_set_hash`]).
fn parcel_set(p: &DerivedPlace) -> String {
    super::parcel_set_hash(&positions_of(p))
}

/// Parcel records to write: each parcel's rating, place, creator, LAND
/// owner (empty when unknown) and the place's parcel set.
#[derive(Default)]
struct Records {
    positions: Vec<String>,
    ratings: Vec<String>,
    places: Vec<String>,
    creators: Vec<String>,
    owners: Vec<String>,
    sets: Vec<String>,
}

impl Records {
    fn push(
        &mut self,
        position: String,
        rating: &str,
        place: &DerivedPlace,
        set: &str,
        owners: &HashMap<String, String>,
    ) {
        self.owners
            .push(owners.get(&position).cloned().unwrap_or_default());
        self.positions.push(position);
        self.ratings.push(rating.to_string());
        self.places.push(place.id.clone());
        self.creators.push(creator_of(place));
        self.sets.push(set.to_string());
    }
}

/// Registers the periodic derive job (`places-derive`), reading parcel owners
/// from `land`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_job(
    jobs: &Jobs,
    pool: PgPool,
    scope: DeriveScope,
    interval: Duration,
    content_public_url: String,
    storage: Option<Arc<crate::storage::Storage>>,
    notify: Option<String>,
    land: Option<Arc<LandAuthzState>>,
) {
    tracing::info!(scope = ?scope, interval_secs = interval.as_secs(), "place catalogue: deriving from content deployments");
    jobs.spawn_periodic_leased(
        JOB_NAME,
        PeriodicCfg::every(interval).with_error_backoff(Duration::from_secs(3600)),
        move |ctx| {
            let (pool, url, storage, notify, land) = (
                pool.clone(),
                content_public_url.clone(),
                storage.clone(),
                notify.clone(),
                land.clone(),
            );
            async move {
                let out = run_once_with_storage(
                    &pool,
                    scope,
                    &url,
                    storage.as_deref(),
                    notify.as_deref(),
                    land.as_deref(),
                )
                .await?;
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
mod tests;
