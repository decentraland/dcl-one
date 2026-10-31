//! The ONE blob store's collector. Every domain writes into the same
//! hash-addressed store (content files and entities, world scenes and
//! thumbnails, posters, satellite regions, world manifests, community
//! thumbnails and federated post bodies, camera-reel photos, builder item contents, project files and asset packs,
//! player-report evidence, MLS commits and ciphertexts, snapshot files), so
//! a file may go only when no domain names it. The store is walked once;
//! its aged files (untouched for the age floor: at least an hour and the
//! upload + processing window, the widest gap between a writer storing a
//! blob and committing the row that names it) are checked in chunks
//! against every [`REFERENCE_SOURCES`] column (`= ANY`, index-served on the
//! large tables) and the JSON documents that embed keys (the world scenes'
//! content lists, the asset-pack catalogue) plus the global scenes the
//! configuration names, so memory holds a chunk, never the corpus. A scene
//! whose content list cannot be read exactly aborts the run, a candidate
//! is re-checked right before it is unlinked (a deployment may reuse a
//! stored blob), an empty reference set with candidates is refused unless
//! asked for, and the dry run reports without deleting.
//!
//! `catalog_is_complete` (the test below) walks the schema: every column
//! whose name says hash, cid, entity id, thumbnail or image must be listed
//! as a reference or as [`NOT_STORE_KEYS`] with its reason, so a new table
//! cannot hold store keys the collector does not know about.

use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;

use super::{Storage, StorageError};

pub const MIN_ORPHAN_AGE: Duration = Duration::from_secs(3600);

/// Every plain column (`schema.table.column`) holding store keys. A chunk
/// of candidates is checked against each with `column = ANY($1)`.
pub const REFERENCE_SOURCES: &[&str] = &[
    "content.deployments.entity_id",
    "content.content_files.content_hash",
    "content.active_pointers.entity_id",
    "content.local_entities.entity_id",
    "content.snapshots.hash",
    "content.processed_snapshots.hash",
    "content.failed_deployments.snapshot_hash",
    "content.failed_deployments.entity_id",
    "explore.satellite_region.blob_cid",
    "explore.world_manifest.blob_cid",
    "explore.poster.blob_cid",
    "worlds.worlds.thumbnail_hash",
    "worlds.federation_worlds.thumbnail_hash",
    "worlds.world_scenes.entity_id",
    "social.community_thumbnails.thumbnail_hash",
    "social.community_contents.cid",
    "social.player_report_evidence.content_hash",
    "social.mls_commits.commit_cid",
    "social.mls_commits.welcome_cid",
    "social.mls_message_refs.ciphertext_cid",
    "social.profile_images.blob_cid",
    "builder.items.thumbnail",
    "builder.items.local_content_hash",
    "builder.items.content_hash",
    "builder.items.catalyst_content_hash",
    "builder.uploaded_contents.hash",
    "builder.item_contents.hash",
    "builder.projects.thumbnail",
    "builder.project_files.hash",
    "builder.asset_packs.thumbnail",
    "builder.assets.thumbnail",
    "builder.asset_contents.hash",
    "camera_reel.images.image_hash",
    "camera_reel.images.thumbnail_hash",
    "shop.outfit_thumbnails.blob_cid",
];

/// Columns whose names look like store keys but are not, with the reason.
pub const NOT_STORE_KEYS: &[(&str, &str)] = &[
    (
        "content.snapshots.replaced_hashes",
        "the snapshot files a newer snapshot replaced: garbage for this collector",
    ),
    (
        "camera_reel.withheld_blobs.hash",
        "the blobs of rejected or deleted photos: garbage for this collector",
    ),
    ("explore.place.image_url", "a URL"),
    ("explore.place_world_local.image_url", "a URL"),
    (
        "explore.world_scene_place.entity_id",
        "a non-owning deployment link: worlds.world_scenes holds the blob reference; \
         retiring a place clears the link, while its UUID and storage remain",
    ),
    (
        "explore.place_indexed.image_url",
        "a URL (the view over the place tables)",
    ),
    ("explore.schedules_local.image", "a URL"),
    (
        "content.forward_outbox.entity_id",
        "a forwarding record: content.deployments holds the reference, and a deployment gone \
         from it is superseded, never sent",
    ),
    (
        "worlds.forward_outbox.entity_id",
        "a forwarding record: worlds.world_scenes holds the reference, and a scene gone from \
         it is superseded, never sent",
    ),
    (
        "worlds.replaced_world_scenes.entity_id",
        "a replaced scene's identity for its scene comms mint, which reads only the stored \
         parcels and base: the entity file is garbage once world_scenes drops it",
    ),
    ("explore.place_reports.entity_id", "a place id"),
    ("explore.user_favorites.entity_id", "a place id"),
    ("explore.user_likes.entity_id", "a place id"),
    ("explore.place_interaction_counts.entity_id", "a place id"),
    (
        "explore.signed_actions.signature_hash",
        "an envelope digest",
    ),
    ("worlds.worlds.access_secret_hash", "a bcrypt hash"),
    ("shop.items.image", "a URL"),
    ("shop.trades.hashed_signature", "an order digest"),
    (
        "shop.trade_cancellations.target_hashed_signature",
        "an order digest",
    ),
    ("shop.moderation_flags.target_hash", "an order digest"),
    ("shop.disputes.trade_hash", "an order digest"),
    ("shop.admin_audit.target_hash", "an order digest"),
    ("shop.chain_cursor.last_block_hash", "a block hash"),
    ("shop.trade_event.hashed_signature", "an order digest"),
    ("shop.trade_event.tx_hash", "a transaction hash"),
    ("shop.mana_transfer.transaction_hash", "a transaction hash"),
    ("shop.coupons.hashed_signature", "an order digest"),
    ("shop.fed_actions.signature_hash", "an envelope digest"),
    ("shop.fed_actions.target_hash", "an envelope digest"),
    ("shop.fed_actions.tx_hash", "a transaction hash"),
    ("economy.transactions.tx_hash", "a transaction hash"),
    ("shop.broker_purchases.tx_hash", "a transaction hash"),
    (
        "shop.broker_purchases.forward_tx_hash",
        "a transaction hash",
    ),
    (
        "shop.broker_purchases.trade_hashed_signature",
        "an order digest",
    ),
    ("shop.escrow_actions.tx_hash", "a transaction hash"),
    ("shop.name_transfers.tx_hash", "a transaction hash"),
    ("shop.rental_event.tx_hash", "a transaction hash"),
    ("shop.executions.tx_hash", "a transaction hash"),
    (
        "credits.authorizations.credit_hash",
        "a CreditsManager credit digest",
    ),
    (
        "credits.authorizations.submitted_tx_hash",
        "a transaction hash",
    ),
    (
        "credits.revocations.credit_hash",
        "a CreditsManager credit digest",
    ),
    ("credits.revocations.tx_hash", "a transaction hash"),
    (
        "social.community_signed_actions.signature_hash",
        "an envelope digest",
    ),
    (
        "social.community_signed_actions.content_hash",
        "a post body's sha256; its blob is social.community_contents.cid",
    ),
    (
        "social.community_contents.content_hash",
        "a post body's sha256; its blob is the row's cid",
    ),
    (
        "social.mls_key_packages.ref_hash",
        "a key package digest; the bytes are in the row",
    ),
    (
        "social.mls_groups.last_commit_hash",
        "the wire sha256 of a commit (its bytes are commit_cid)",
    ),
    (
        "social.mls_commits.commit_hash",
        "the wire sha256 of a commit (its bytes are commit_cid)",
    ),
    (
        "social.mls_message_refs.signature_hash",
        "a message ref digest",
    ),
    (
        "social.mls_message_refs.ciphertext_hash",
        "the wire sha256 of a ciphertext (its bytes are ciphertext_cid)",
    ),
    (
        "builder.collection_publications.tx_hash",
        "a transaction hash",
    ),
    (
        "social.community_mirror.thumbnail_url",
        "a URL (the upstream CDN's thumbnail of a mirrored community)",
    ),
    ("credits.usages.tx_hash", "a transaction hash"),
    (
        "social.profile_images.entity_id",
        "the profile entity an image was drawn for; its blob is the row's blob_cid",
    ),
    (
        "shop.outfits.thumbnail_hash",
        "the sha256 wire id of an outfit thumbnail; its blob is shop.outfit_thumbnails.blob_cid",
    ),
    (
        "shop.outfit_thumbnails.hash",
        "the sha256 wire id of the bytes; the blob is the row's blob_cid",
    ),
    (
        "shop.treasury_pending_refill.swap_tx_hash",
        "a transaction hash",
    ),
    (
        "shop.treasury_pending_refill.transfer_tx_hash",
        "a transaction hash",
    ),
];

/// JSON documents that embed store keys, read by [`reference_set`] beside
/// the plain sources: the world scene entities' `content[].hash` and the
/// asset-pack catalogue's thumbnails and asset contents.
pub const JSON_SOURCES: &[&str] = &["worlds.world_scenes.entity", "builder.asset_catalog.packs"];

/// The age floor: at least an hour, and at least the widest store-then-row
/// gap (a multipart upload plus its processing).
pub fn orphan_age_floor(
    multipart_upload_timeout_ms: u64,
    deployment_processing_timeout_ms: u64,
) -> Duration {
    Duration::from_millis(
        multipart_upload_timeout_ms.saturating_add(deployment_processing_timeout_ms),
    )
    .max(MIN_ORPHAN_AGE)
}

#[derive(Debug, thiserror::Error)]
pub enum GcError {
    #[error("reading the reference set: {0}")]
    Database(#[from] sqlx::Error),
    #[error("the reference set cannot be read exactly: {0}")]
    Unreadable(String),
    #[error("enumerating the store: {0}")]
    Store(#[from] StorageError),
    #[error("reading {0}: {1}")]
    Io(String, std::io::Error),
    #[error(
        "refusing to collect: no domain names any stored key, which usually means the database is empty or misconfigured; re-run with allow_empty_reference_set to collect anyway"
    )]
    EmptyReferenceSet,
}

#[derive(Debug, Clone, Copy)]
pub struct GcOptions {
    pub dry_run: bool,
    pub min_age: Duration,
    pub allow_empty_reference_set: bool,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GcReport {
    pub dry_run: bool,
    pub scanned: usize,
    pub candidates: usize,
    pub removed: usize,
    pub failed: usize,
    /// Aged files kept because a domain names them.
    pub references: usize,
    pub min_age_seconds: u64,
}

fn content_hashes(
    owner: &str,
    content: Option<&Value>,
    out: &mut HashSet<String>,
) -> Result<(), GcError> {
    let items = match content {
        None | Some(Value::Null) => return Ok(()),
        Some(Value::Array(items)) => items,
        Some(_) => {
            return Err(GcError::Unreadable(format!(
                "{owner} stores a non-array content list"
            )));
        }
    };
    for item in items {
        match item.get("hash").and_then(Value::as_str) {
            Some(hash) if !hash.is_empty() => {
                out.insert(hash.to_string());
            }
            _ => {
                return Err(GcError::Unreadable(format!(
                    "{owner} has a content entry without a usable hash"
                )));
            }
        }
    }
    Ok(())
}

fn catalogue_hashes(packs: &Value, out: &mut HashSet<String>) -> Result<(), GcError> {
    let packs = match packs {
        Value::Array(packs) => packs,
        _ => {
            return Err(GcError::Unreadable(
                "builder.asset_catalog.packs is not an array".into(),
            ));
        }
    };
    for pack in packs {
        if let Some(thumbnail) = pack
            .get("thumbnail")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            out.insert(thumbnail.to_string());
        }
        for asset in pack
            .get("assets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for hash in asset
                .get("contents")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|c| c.values())
            {
                match hash.as_str() {
                    Some(hash) if !hash.is_empty() => {
                        out.insert(hash.to_string());
                    }
                    _ => {
                        return Err(GcError::Unreadable(
                            "an asset-pack content entry has no usable hash".into(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// The entity id of each `urn:decentraland:entity:<id>[?...]` the
/// configuration names (the global scenes every world loads).
pub fn global_scene_entity_ids(urns: &[String]) -> Vec<String> {
    urns.iter()
        .filter_map(|urn| {
            urn.split_once("urn:decentraland:entity:")
                .map(|(_, rest)| rest)
        })
        .map(|rest| rest.split('?').next().unwrap_or(rest).trim().to_string())
        .filter(|cid| !cid.is_empty())
        .collect()
}

/// `(table, column)` of a `schema.table.column` source name.
fn split_source(source: &str) -> (&str, &str) {
    let (table, column) = source.rsplit_once('.').expect("schema.table.column");
    (table, column)
}

/// Refuses a run over world scenes whose content lists cannot be read
/// exactly (a caller deletes what the references do not name).
async fn check_json_sources(pool: &PgPool) -> Result<(), GcError> {
    let bad: Option<String> = sqlx::query_scalar(
        "SELECT entity_id FROM worlds.world_scenes s \
         WHERE jsonb_typeof(s.entity) <> 'object' \
            OR jsonb_typeof(s.entity -> 'content') NOT IN ('array', 'null') \
            OR (jsonb_typeof(s.entity -> 'content') = 'array' AND EXISTS ( \
                SELECT 1 FROM jsonb_array_elements(s.entity -> 'content') e \
                WHERE jsonb_typeof(e -> 'hash') IS DISTINCT FROM 'string' OR e ->> 'hash' = '')) \
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    match bad {
        Some(entity_id) => Err(GcError::Unreadable(format!(
            "scene {entity_id} stores an entity whose content list is not an array of hashed files"
        ))),
        None => Ok(()),
    }
}

/// The small reference documents read whole: the asset-pack catalogue and
/// the global scenes the configuration names (their entity file and its
/// content list).
async fn fixed_references(
    pool: &PgPool,
    storage: &Storage,
    global_scenes_urn: &[String],
) -> Result<HashSet<String>, GcError> {
    let mut keys = HashSet::new();
    let catalogue: Option<Value> =
        sqlx::query_scalar("SELECT packs FROM builder.asset_catalog WHERE id = 1")
            .fetch_optional(pool)
            .await?;
    if let Some(packs) = catalogue {
        catalogue_hashes(&packs, &mut keys)?;
    }
    for entity_id in global_scene_entity_ids(global_scenes_urn) {
        if let Some(bytes) = storage.retrieve(&entity_id).await? {
            let entity: Value = serde_json::from_slice(&bytes).map_err(|e| {
                GcError::Unreadable(format!(
                    "global scene entity {entity_id} does not parse: {e}"
                ))
            })?;
            content_hashes(
                &format!("global scene {entity_id}"),
                entity.get("content"),
                &mut keys,
            )?;
        }
        keys.insert(entity_id);
    }
    Ok(keys)
}

/// Whether any domain names any key at all (the empty-set refusal).
async fn any_reference(pool: &PgPool) -> Result<bool, GcError> {
    for source in REFERENCE_SOURCES {
        let (table, column) = split_source(source);
        let present: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE {column} IS NOT NULL AND {column} <> '')"
        )))
        .fetch_one(pool)
        .await?;
        if present {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The keys among `keys` some domain names: one `= ANY` probe per source
/// (index-served on the large tables) and the world scenes' content lists.
pub async fn referenced_among(pool: &PgPool, keys: &[String]) -> Result<HashSet<String>, GcError> {
    let mut found = HashSet::new();
    for source in REFERENCE_SOURCES {
        let (table, column) = split_source(source);
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT {column} FROM {table} WHERE {column} = ANY($1)"
        )))
        .bind(keys)
        .fetch_all(pool)
        .await?;
        found.extend(rows);
    }
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT e ->> 'hash' FROM worlds.world_scenes s \
         CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(s.entity -> 'content') = 'array' THEN s.entity -> 'content' ELSE '[]'::jsonb END) e \
         WHERE e ->> 'hash' = ANY($1)",
    )
    .bind(keys)
    .fetch_all(pool)
    .await?;
    found.extend(rows);
    Ok(found)
}

const CHUNK: usize = 2_000;

/// One collection pass: the store is walked in chunks of aged files, each
/// checked against every domain's references; a candidate is re-checked
/// right before it is unlinked (a deployment may reuse a stored blob).
pub async fn collect(
    pool: &PgPool,
    storage: &Storage,
    global_scenes_urn: &[String],
    options: GcOptions,
) -> Result<GcReport, GcError> {
    check_json_sources(pool).await?;
    let fixed = fixed_references(pool, storage, global_scenes_urn).await?;
    let refuse_empty =
        !options.allow_empty_reference_set && fixed.is_empty() && !any_reference(pool).await?;
    let now = SystemTime::now();
    let mut report = GcReport {
        dry_run: options.dry_run,
        min_age_seconds: options.min_age.as_secs(),
        ..GcReport::default()
    };
    let mut orphans: Vec<String> = Vec::new();
    let mut chunk: Vec<String> = Vec::with_capacity(CHUNK);
    let mut ids = storage.all_file_ids(None);
    loop {
        let next = ids.next().await?;
        if let Some(id) = &next {
            report.scanned += 1;
            if let Some(path) = storage.file_path(id).await? {
                let modified = match tokio::fs::metadata(&path).await {
                    Ok(meta) => meta.modified().ok(),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(GcError::Io(path.display().to_string(), e)),
                };
                if modified
                    .and_then(|m| now.duration_since(m).ok())
                    .is_some_and(|age| age >= options.min_age)
                    && !fixed.contains(id)
                {
                    chunk.push(id.clone());
                } else if fixed.contains(id) {
                    report.references += 1;
                }
            }
        }
        if chunk.len() >= CHUNK || (next.is_none() && !chunk.is_empty()) {
            chunk.sort();
            chunk.dedup();
            let referenced = referenced_among(pool, &chunk).await?;
            report.references += referenced.len();
            orphans.extend(chunk.drain(..).filter(|id| !referenced.contains(id)));
        }
        if next.is_none() {
            break;
        }
    }
    report.candidates = orphans.len();
    if refuse_empty && !orphans.is_empty() {
        return Err(GcError::EmptyReferenceSet);
    }
    if !options.dry_run {
        for batch in orphans.chunks(CHUNK) {
            let referenced_now = referenced_among(pool, batch).await?;
            for id in batch.iter().filter(|id| !referenced_now.contains(*id)) {
                match storage.delete_strict(id).await {
                    Ok(()) => report.removed += 1,
                    Err(e) => {
                        report.failed += 1;
                        tracing::warn!(key = %id, error = %e, "blob store collector: could not remove an unreferenced file");
                    }
                }
            }
        }
    }
    tracing::info!(?report, "blob store collection finished");
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use serde_json::json;

    #[test]
    fn the_floor_is_at_least_an_hour_and_the_upload_window() {
        assert_eq!(orphan_age_floor(1_000, 1_000), MIN_ORPHAN_AGE);
        assert_eq!(
            orphan_age_floor(3_000_000, 1_000_000),
            Duration::from_millis(4_000_000)
        );
    }

    #[test]
    fn a_content_list_without_a_usable_hash_aborts_the_set() {
        let mut out = HashSet::new();
        content_hashes("s", Some(&json!([{ "file": "a", "hash": "h1" }])), &mut out).unwrap();
        assert!(out.contains("h1"));
        assert!(content_hashes("s", Some(&json!([{ "file": "a" }])), &mut out).is_err());
        assert!(content_hashes("s", Some(&json!({ "a": "h" })), &mut out).is_err());
        let mut packs = HashSet::new();
        catalogue_hashes(
            &json!([{ "thumbnail": "t1", "assets": [{ "contents": { "a.glb": "g1" } }] }]),
            &mut packs,
        )
        .unwrap();
        assert_eq!(packs, HashSet::from(["t1".to_string(), "g1".to_string()]));
        assert!(catalogue_hashes(&json!({}), &mut packs).is_err());
        assert_eq!(
            global_scene_entity_ids(&[
                "urn:decentraland:entity:bafyx?baseUrl=https://x/".into(),
                "nonsense".into()
            ]),
            vec!["bafyx"]
        );
    }

    /// Every hash/cid/entity/thumbnail/image column of the schema is either a
    /// reference source or a declared non-key, and every declared source
    /// statement runs against the real schema.
    #[tokio::test]
    async fn catalog_is_complete() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let columns: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT table_schema, table_name, column_name FROM information_schema.columns \
             WHERE table_schema NOT IN ('pg_catalog', 'information_schema') \
               AND data_type IN ('text', 'character varying', 'ARRAY', 'jsonb', 'json') \
               AND column_name ~ '(hash|cid|entity_id|thumbnail|image)'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let known: HashSet<&str> = REFERENCE_SOURCES
            .iter()
            .copied()
            .chain(NOT_STORE_KEYS.iter().map(|(c, _)| *c))
            .chain(JSON_SOURCES.iter().copied())
            .collect();
        let unclassified: Vec<String> = columns
            .iter()
            .map(|(s, t, c)| format!("{s}.{t}.{c}"))
            .filter(|name| !known.contains(name.as_str()))
            .collect();
        assert!(
            unclassified.is_empty(),
            "classify these columns in storage::gc (REFERENCE_SOURCES or NOT_STORE_KEYS): {unclassified:?}"
        );
        referenced_among(&pool, &["none".to_string()])
            .await
            .expect("every source statement runs against the schema");
    }

    /// On a database of its own: an aged unreferenced file goes, a
    /// referenced one (a content file, a poster, a scene's content, a global
    /// scene) and a fresh one stay; the dry run deletes nothing; an empty
    /// reference set is refused unless allowed.
    #[tokio::test]
    async fn only_aged_unreferenced_files_are_collected() {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        let Ok(base_url) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            return;
        };
        let opts: PgConnectOptions = base_url.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_test_gc_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut url = url::Url::parse(&base_url).unwrap();
        url.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            url.as_str(),
            &crate::db::PoolSettings {
                max_connections: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        let dir =
            std::env::temp_dir().join(format!("catalyst-gc-{}", uuid::Uuid::new_v4().simple()));
        let storage = Storage::from_config(&dir).await.unwrap();
        let put = |bytes: Vec<u8>| {
            let storage = &storage;
            async move {
                let key = crate::auth::hash::hash_bytes_v1(&bytes);
                storage.store(&key, Bytes::from(bytes)).await.unwrap();
                key
            }
        };
        let orphan = put(b"orphan".to_vec()).await;
        let poster = put(b"poster".to_vec()).await;
        let scene_file = put(b"scene file".to_vec()).await;
        let global_file = put(b"global file".to_vec()).await;
        let fresh = put(b"fresh".to_vec()).await;
        let global = put(json!({ "content": [{ "file": "a", "hash": global_file }] })
            .to_string()
            .into_bytes())
        .await;
        let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);
        for key in [&orphan, &poster, &scene_file, &global, &global_file] {
            let path = storage.file_path(key).await.unwrap().unwrap();
            std::fs::File::open(path)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(two_hours_ago))
                .unwrap();
        }
        let options = GcOptions {
            dry_run: true,
            min_age: MIN_ORPHAN_AGE,
            allow_empty_reference_set: false,
        };
        let globals = vec![format!(
            "urn:decentraland:entity:{global}?baseUrl=https://x/contents/"
        )];
        assert!(matches!(
            collect(&pool, &storage, &[], options).await,
            Err(GcError::EmptyReferenceSet)
        ));

        sqlx::query("INSERT INTO explore.poster (kind, file, blob_cid, content_type, byte_size, origin, fetched_at) VALUES ('poster', 'p.png', $1, 'image/png', 1, 'local', timezone('UTC', now()))")
            .bind(&poster)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ('gc.dcl.eth')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) \
             VALUES ('gc.dcl.eth', 'bafkreiscene', '[]'::json, $1, $2, '{\"0,0\"}', 1)",
        )
        .bind(json!({ "content": [{ "file": "a.glb", "hash": scene_file }] }))
        .bind(crate::shared::identity::ZERO_ADDRESS)
        .execute(&pool)
        .await
        .unwrap();

        let dry = collect(&pool, &storage, &globals, options).await.unwrap();
        assert_eq!((dry.scanned, dry.candidates, dry.removed), (6, 1, 0));
        assert!(
            storage.exist(&orphan).await.unwrap(),
            "the dry run deletes nothing"
        );
        let wet = collect(
            &pool,
            &storage,
            &globals,
            GcOptions {
                dry_run: false,
                ..options
            },
        )
        .await
        .unwrap();
        assert_eq!((wet.candidates, wet.removed, wet.failed), (1, 1, 0));
        assert!(!storage.exist(&orphan).await.unwrap());
        for key in [&poster, &scene_file, &global, &global_file, &fresh] {
            assert!(storage.exist(key).await.unwrap(), "{key} stays");
        }
        pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
