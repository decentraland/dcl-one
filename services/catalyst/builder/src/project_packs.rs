//! A project's own asset packs: upstream builder-server's AssetPack/Asset
//! upsert (`PUT /v1/assetPacks/{id}`) scoped to one project instead of one
//! wallet. The project owner writes them; `GET .../assetPacks` lists the
//! node's default packs (`builder.asset_catalog`, what `/v1/assetPacks`
//! serves) first and the project's after, in upstream's
//! `FullAssetPackAttributes` shape. Every hash (pack and asset thumbnails,
//! asset contents) names an upload.

use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::drafts::valid_filename;
use crate::http::ApiError;
use crate::projects::{Project, ProjectError, invalid_schema};
use dcl_auth::is_canonical_cid;

/// Upstream's `MAX_ASSETS_COUNT`.
pub const MAX_ASSETS: usize = 80;
/// Upstream's `MAX_ASSET_PACKS_COUNT`, enforced per project on write.
pub const MAX_PACKS: i64 = 10;
const FK_VIOLATION: &str = "23503";
const INVALID_ASSETS: &str = "One of the assets you're trying to upload is invalid";

#[derive(Debug, Deserialize)]
pub struct AssetIn {
    pub id: Uuid,
    pub name: String,
    pub model: String,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub legacy_id: Option<String>,
    pub tags: Vec<String>,
    pub category: String,
    pub contents: BTreeMap<String, String>,
    #[serde(default = "empty_object")]
    pub metrics: Value,
    #[serde(default = "empty_array")]
    pub parameters: Value,
    #[serde(default = "empty_array")]
    pub actions: Value,
}

fn empty_object() -> Value {
    json!({})
}

fn empty_array() -> Value {
    json!([])
}

/// Upstream's `assetPackSchema`; `eth_address`, the dates and every
/// asset's `asset_pack_id` are the server's.
#[derive(Debug, Deserialize)]
pub struct PackIn {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    pub assets: Vec<AssetIn>,
}

fn optional_hash<'a>(
    field: &str,
    value: &'a Option<String>,
) -> Result<Option<&'a str>, ProjectError> {
    match value.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(h) if is_canonical_cid(h) => Ok(Some(h)),
        Some(_) => Err(invalid_schema(format!(
            "{field} must be the hash of an uploaded content"
        ))),
    }
}

impl PackIn {
    pub fn parse(body: Value, url_id: Uuid) -> Result<Self, ProjectError> {
        let pack = body
            .get("assetPack")
            .cloned()
            .ok_or_else(|| invalid_schema("the body has no assetPack"))?;
        let pack: Self = serde_json::from_value(pack).map_err(invalid_schema)?;
        if !(3..=20).contains(&pack.title.chars().count()) {
            return Err(invalid_schema("title is 3 to 20 characters"));
        }
        if pack.id != url_id {
            return Err(ProjectError::Data {
                status: 400,
                message: "The body and URL assetPack ids do not match".into(),
                data: json!({ "urlId": url_id, "bodyId": pack.id }),
                etag: None,
            });
        }
        if pack.assets.len() > MAX_ASSETS {
            return Err(ProjectError::Data {
                status: 400,
                message: format!("Too many assets for Asset Pack. The max amount is {MAX_ASSETS}"),
                data: json!({ "assetPackId": url_id, "count": pack.assets.len() }),
                etag: None,
            });
        }
        let mut ids = HashSet::new();
        for asset in &pack.assets {
            if !ids.insert(asset.id) {
                return Err(invalid_schema(format!(
                    "asset {} is listed twice",
                    asset.id
                )));
            }
            if !(3..=50).contains(&asset.name.chars().count()) {
                return Err(invalid_schema("an asset name is 3 to 50 characters"));
            }
            optional_hash("an asset thumbnail", &asset.thumbnail)?;
            if !asset.metrics.is_object()
                || !asset.parameters.is_array()
                || !asset.actions.is_array()
            {
                return Err(invalid_schema(
                    "an asset's metrics is an object, its parameters and actions arrays",
                ));
            }
            if let Some((file, _)) = asset
                .contents
                .iter()
                .find(|(file, hash)| !valid_filename(file) || !is_canonical_cid(hash))
            {
                return Err(invalid_schema(format!(
                    "asset {} names {file:?}: contents map file names to uploaded hashes",
                    asset.id
                )));
            }
        }
        optional_hash("thumbnail", &pack.thumbnail)?;
        Ok(pack)
    }
}

/// One pack in upstream's shape, with `eth_address` the project owner's.
const PACK_JSON: &str = "jsonb_build_object('id', ap.id, 'title', ap.title, 'thumbnail', ap.thumbnail, \
    'eth_address', p.eth_address, 'project_id', ap.project_id, 'is_deleted', ap.is_deleted, \
    'created_at', to_char(ap.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'), \
    'updated_at', to_char(ap.updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'), \
    'assets', COALESCE((SELECT jsonb_agg(jsonb_build_object('id', a.id, 'asset_pack_id', a.asset_pack_id, \
        'legacy_id', a.legacy_id, 'name', a.name, 'model', a.model, 'script', a.script, \
        'thumbnail', a.thumbnail, 'tags', to_jsonb(a.tags), 'category', a.category, 'metrics', a.metrics, \
        'parameters', a.parameters, 'actions', a.actions, \
        'contents', COALESCE((SELECT jsonb_object_agg(c.file, c.hash) FROM builder.asset_contents c \
            WHERE c.asset_id = a.id), '{}'::jsonb)) ORDER BY a.ordinal) \
        FROM builder.assets a WHERE a.asset_pack_id = ap.id), '[]'::jsonb))";

/// The project's live packs, oldest first.
pub async fn project_packs(pool: &PgPool, project: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {PACK_JSON} FROM builder.asset_packs ap JOIN builder.projects p ON p.id = ap.project_id \
         WHERE ap.project_id = $1 AND NOT ap.is_deleted ORDER BY ap.created_at, ap.id"
    )))
    .bind(project)
    .fetch_all(pool)
    .await
}

async fn pack(pool: &PgPool, id: Uuid) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {PACK_JSON} FROM builder.asset_packs ap JOIN builder.projects p ON p.id = ap.project_id \
         WHERE ap.id = $1"
    )))
    .bind(id)
    .fetch_one(pool)
    .await
}

/// The default packs (the catalogue body's `data`) then the project's.
pub fn with_defaults(catalog_body: Option<&[u8]>, own: Vec<Value>) -> Vec<Value> {
    let mut packs = catalog_body
        .and_then(|b| serde_json::from_slice::<Value>(b).ok())
        .and_then(|mut v| match v.get_mut("data").map(Value::take) {
            Some(Value::Array(packs)) => Some(packs),
            _ => None,
        })
        .unwrap_or_default();
    packs.extend(own);
    packs
}

fn not_this_projects(pack: Uuid, owner: &str) -> ProjectError {
    ProjectError::Data {
        status: 401,
        message: "Unauthorized user".into(),
        data: json!({ "id": pack, "eth_address": owner }),
        etag: None,
    }
}

/// Upstream's upsertAssetPack: the pack upserted (revived if deleted), the
/// assets it no longer lists deleted, the listed ones upserted in order
/// with their contents. An id already used by another pack's asset, or a
/// pack of another project (deleted or not), is refused.
pub async fn put_pack(
    pool: &PgPool,
    project: &Project,
    owner: &str,
    pack_in: PackIn,
) -> Result<Value, ProjectError> {
    let mut tx = pool.begin().await?;
    let existing: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT project_id, is_deleted FROM builder.asset_packs WHERE id = $1 FOR UPDATE",
    )
    .bind(pack_in.id)
    .fetch_optional(&mut *tx)
    .await?;
    match existing {
        Some((p, _)) if p != project.id => return Err(not_this_projects(pack_in.id, owner)),
        Some((_, false)) => {}
        // A new pack, or a deleted one this PUT revives: either adds a live one.
        None | Some((_, true)) => {
            // Serialize concurrent creations on the project row for the cap.
            sqlx::query("SELECT 1 FROM builder.projects WHERE id = $1 FOR UPDATE")
                .bind(project.id)
                .execute(&mut *tx)
                .await?;
            let live: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM builder.asset_packs WHERE project_id = $1 AND NOT is_deleted",
            )
            .bind(project.id)
            .fetch_one(&mut *tx)
            .await?;
            if live >= MAX_PACKS {
                return Err(ProjectError::Data {
                    status: 400,
                    message: format!("Too many asset packs. The max amount is {MAX_PACKS}"),
                    data: json!({ "projectId": project.id, "count": live }),
                    etag: None,
                });
            }
        }
    }
    let ids: Vec<Uuid> = pack_in.assets.iter().map(|a| a.id).collect();
    let foreign: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.assets WHERE id = ANY($1) AND asset_pack_id <> $2)",
    )
    .bind(&ids)
    .bind(pack_in.id)
    .fetch_one(&mut *tx)
    .await?;
    if foreign {
        return Err(ApiError::bad_request(INVALID_ASSETS).into());
    }
    let thumbnail = optional_hash("thumbnail", &pack_in.thumbnail)?;
    let write = async {
        sqlx::query(
            "INSERT INTO builder.asset_packs (id, project_id, title, thumbnail) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, thumbnail = EXCLUDED.thumbnail, \
             is_deleted = false, updated_at = timezone('UTC', now())",
        )
        .bind(pack_in.id)
        .bind(project.id)
        .bind(&pack_in.title)
        .bind(thumbnail)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM builder.assets WHERE asset_pack_id = $1 AND NOT (id = ANY($2))")
            .bind(pack_in.id)
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM builder.asset_contents WHERE asset_id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        for (ordinal, asset) in pack_in.assets.iter().enumerate() {
            sqlx::query(
                "INSERT INTO builder.assets (id, asset_pack_id, ordinal, legacy_id, name, model, script, \
                     thumbnail, tags, category, metrics, parameters, actions) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
                 ON CONFLICT (id) DO UPDATE SET ordinal = EXCLUDED.ordinal, legacy_id = EXCLUDED.legacy_id, \
                     name = EXCLUDED.name, model = EXCLUDED.model, script = EXCLUDED.script, \
                     thumbnail = EXCLUDED.thumbnail, tags = EXCLUDED.tags, category = EXCLUDED.category, \
                     metrics = EXCLUDED.metrics, parameters = EXCLUDED.parameters, actions = EXCLUDED.actions",
            )
            .bind(asset.id)
            .bind(pack_in.id)
            .bind(i32::try_from(ordinal).unwrap_or(i32::MAX))
            .bind(&asset.legacy_id)
            .bind(&asset.name)
            .bind(&asset.model)
            .bind(&asset.script)
            .bind(asset.thumbnail.as_deref().map(str::trim).filter(|t| !t.is_empty()))
            .bind(&asset.tags)
            .bind(&asset.category)
            .bind(&asset.metrics)
            .bind(&asset.parameters)
            .bind(&asset.actions)
            .execute(&mut *tx)
            .await?;
            let (files, hashes): (Vec<&String>, Vec<&String>) = asset.contents.iter().unzip();
            sqlx::query(
                "INSERT INTO builder.asset_contents (asset_id, file, hash) \
                 SELECT $1, f, h FROM unnest($2::text[], $3::text[]) AS c(f, h)",
            )
            .bind(asset.id)
            .bind(&files)
            .bind(&hashes)
            .execute(&mut *tx)
            .await?;
        }
        Ok::<_, sqlx::Error>(())
    };
    match write.await {
        Ok(()) => {}
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some(FK_VIOLATION) => {
            return Err(ApiError::bad_request(
                "every thumbnail and content hash must be an uploaded content (PUT /v1/projects/{id}/blobs/{hash})",
            )
            .into());
        }
        Err(e) => return Err(e.into()),
    }
    tx.commit().await?;
    Ok(pack(pool, pack_in.id).await?)
}

/// Upstream's soft delete; a pack of another project is 401, an unknown
/// one a no-op.
pub async fn delete_pack(
    pool: &PgPool,
    project: &Project,
    owner: &str,
    id: Uuid,
) -> Result<(), ProjectError> {
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM builder.asset_packs WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    if existing.is_some_and(|p| p != project.id) {
        return Err(not_this_projects(id, owner));
    }
    sqlx::query(
        "UPDATE builder.asset_packs SET is_deleted = true, updated_at = timezone('UTC', now()) \
         WHERE id = $1 AND project_id = $2",
    )
    .bind(id)
    .bind(project.id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_come_first() {
        let body = br#"{"ok":true,"data":[{"id":"default"}]}"#;
        let packs = with_defaults(Some(body), vec![json!({ "id": "own" })]);
        assert_eq!(
            packs,
            vec![json!({ "id": "default" }), json!({ "id": "own" })]
        );
        assert_eq!(with_defaults(None, vec![]), Vec::<Value>::new());
    }

    #[test]
    fn pack_schema_follows_upstream() {
        let id = Uuid::new_v4();
        let asset = |name: &str| {
            json!({ "id": Uuid::new_v4(), "asset_pack_id": Uuid::new_v4(), "name": name, "model": "m.glb",
                    "tags": [], "category": "c", "contents": {} })
        };
        let ok = PackIn::parse(
            json!({ "assetPack": { "id": id, "title": "Mine", "assets": [asset("Tree")] } }),
            id,
        );
        assert!(ok.is_ok(), "{ok:?}");
        let short = PackIn::parse(
            json!({ "assetPack": { "id": id, "title": "ab", "assets": [] } }),
            id,
        );
        assert_eq!(short.unwrap_err().status(), 400);
        let other = PackIn::parse(
            json!({ "assetPack": { "id": id, "title": "Mine", "assets": [] } }),
            Uuid::new_v4(),
        );
        assert_eq!(other.unwrap_err().status(), 400);
        let many: Vec<Value> = (0..=MAX_ASSETS).map(|_| asset("Tree")).collect();
        let too_many = PackIn::parse(
            json!({ "assetPack": { "id": id, "title": "Mine", "assets": many } }),
            id,
        );
        assert_eq!(too_many.unwrap_err().status(), 400);
    }
}
