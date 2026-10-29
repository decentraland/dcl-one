//! Scene projects: upstream builder-server's Project and Manifest routes over
//! `builder.projects` / `builder.project_manifests`, and the
//! content-addressed file tree `dcl-scene-cli sync` mirrors
//! (`builder.project_files`). The owner is the signed-fetch signer that
//! first wrote the project, re-checked on every read and write (upstream's
//! `withModelAuthorization` and `Ownable.canUpsert`); every hash names an
//! upload (`uploaded_contents`, FK) whose bytes are in the ONE store.
//!
//! Two documents carry optimistic concurrency, each with its counter on the
//! project row: the file set (`files_version`; `PUT .../files` names the
//! version it read, a stale one is 409 with the server's set) and the
//! builder manifest (`manifest_revision`, the ETag of `GET .../manifest`;
//! `If-Match` / `If-None-Match: *` on its PUT, a stale one is 412 with the
//! server's manifest). Upstream's manifest PUT is unconditional; a PUT
//! without either header still is.

use std::collections::{BTreeMap, HashMap};

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use chrono::{NaiveDateTime, SecondsFormat};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::http::ApiError;
use crate::storage::Storage;
use dcl_auth::{hash_bytes_v1, is_canonical_cid};

/// `uploaded_contents_size_check`: one blob, 20 MiB.
pub const MAX_BLOB_BYTES: usize = 20 * 1024 * 1024;
/// Paths per project file set.
pub const MAX_PROJECT_FILES: usize = 5000;
/// `project_files_path_check`.
const MAX_PATH_CHARS: usize = 512;
/// Upstream's pagination bound.
const MAX_PAGE: i64 = 100;
const FK_VIOLATION: &str = "23503";
const CHECK_VIOLATION: &str = "23514";

/// A refusal: the builder envelope, or upstream's with a `data` member (the
/// 404 naming the table, the 401 naming the signer, a conflict carrying the
/// server's copy and its version as the ETag).
#[derive(Debug)]
pub enum ProjectError {
    Api(ApiError),
    Data {
        status: u16,
        message: String,
        data: Value,
        etag: Option<i64>,
    },
}

impl From<ApiError> for ProjectError {
    fn from(e: ApiError) -> Self {
        Self::Api(e)
    }
}

impl From<sqlx::Error> for ProjectError {
    fn from(e: sqlx::Error) -> Self {
        Self::Api(ApiError::from(e))
    }
}

impl ProjectError {
    pub fn status(&self) -> u16 {
        match self {
            Self::Api(e) => e.status(),
            Self::Data { status, .. } => *status,
        }
    }

    fn data(status: u16, message: impl Into<String>, data: Value) -> Self {
        Self::Data {
            status,
            message: message.into(),
            data,
            etag: None,
        }
    }
}

impl IntoResponse for ProjectError {
    fn into_response(self) -> Response {
        match self {
            Self::Api(e) => e.into_response(),
            Self::Data {
                status,
                message,
                data,
                etag,
            } => {
                let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST);
                let body =
                    json!({ "ok": false, "data": data, "error": message, "message": message });
                let mut resp = (status, axum::Json(body)).into_response();
                if let Some(revision) = etag {
                    set_etag(&mut resp, revision);
                }
                resp
            }
        }
    }
}

pub fn set_etag(resp: &mut Response, revision: i64) {
    if let Ok(v) = HeaderValue::from_str(&format!("\"{revision}\"")) {
        resp.headers_mut().insert(header::ETAG, v);
    }
}

/// Upstream's `withModelExists` miss.
pub fn project_not_found(id: &str) -> ProjectError {
    ProjectError::data(
        404,
        format!("Couldn't find \"{id}\" on projects"),
        json!({ "id": id, "tableName": "projects" }),
    )
}

/// Upstream's `withModelExists` on an id that is not a uuid.
pub fn parse_id(raw: &str) -> Result<Uuid, ProjectError> {
    Uuid::parse_str(raw)
        .map_err(|_| ProjectError::data(400, format!("Invalid uuid {raw}"), json!({ "id": raw })))
}

fn unauthorized_for(signer: &str, table: &str, id: &str) -> ProjectError {
    ProjectError::data(
        401,
        format!("Unauthorized user {signer} for {table} {id}"),
        json!({ "ethAddress": signer, "tableName": table }),
    )
}

fn js_date<S: serde::Serializer>(t: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&t.and_utc().to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn is_violation(e: &sqlx::Error, code: &str) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some(code))
}

/// A project row as the wire renders it: upstream's `ProjectAttributes`
/// plus the two document counters.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub thumbnail: Option<String>,
    pub scene_id: Uuid,
    pub eth_address: String,
    pub cols: i32,
    pub rows: i32,
    pub parcels: i32,
    pub transforms: i32,
    pub gltf_shapes: i32,
    pub nft_shapes: i32,
    pub scripts: i32,
    pub entities: i32,
    pub is_deleted: bool,
    pub is_public: bool,
    #[serde(serialize_with = "js_date")]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "js_date")]
    pub updated_at: NaiveDateTime,
    pub creation_coords: Option<String>,
    pub is_template: bool,
    pub video: Option<String>,
    pub template_status: Option<String>,
    pub files_version: i64,
    pub manifest_revision: i64,
}

const PROJECT_COLUMNS: &str = "id, title, description, thumbnail, scene_id, eth_address, cols, rows, \
    parcels, transforms, gltf_shapes, nft_shapes, scripts, entities, is_deleted, is_public, created_at, \
    updated_at, creation_coords, is_template, video, template_status, files_version, manifest_revision";

/// The body of `PUT /v1/projects/{id}` (`{project}`) and a manifest's
/// `project`: upstream's `projectSchema` (id, title, description,
/// scene_id, cols and rows required). An absent optional field keeps the
/// stored value; `eth_address` and the dates are the server's.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectIn {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub scene_id: Uuid,
    pub cols: i32,
    pub rows: i32,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub is_public: Option<bool>,
    #[serde(default)]
    pub creation_coords: Option<String>,
    #[serde(default)]
    pub is_template: Option<bool>,
    #[serde(default)]
    pub video: Option<String>,
    #[serde(default)]
    pub template_status: Option<String>,
}

pub fn invalid_schema(e: impl std::fmt::Display) -> ProjectError {
    ProjectError::data(400, "Invalid schema", json!([{ "message": e.to_string() }]))
}

impl ProjectIn {
    pub fn parse(value: Value) -> Result<Self, ProjectError> {
        let project: Self = serde_json::from_value(value).map_err(invalid_schema)?;
        if !(1..=1000).contains(&project.cols) || !(1..=1000).contains(&project.rows) {
            return Err(invalid_schema("cols and rows must be 1..1000"));
        }
        if project.title.chars().count() > 512 || project.description.chars().count() > 4096 {
            return Err(invalid_schema(
                "title is at most 512 characters, description 4096",
            ));
        }
        Ok(project)
    }

    /// The thumbnail names an upload by its CID; an empty one is none.
    fn thumbnail(&self) -> Result<Option<&str>, ProjectError> {
        match self.thumbnail.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(h) if is_canonical_cid(h) => Ok(Some(h)),
            Some(_) => Err(ApiError::bad_request(
                "thumbnail must be the hash of an uploaded content (PUT .../blobs/{hash})",
            )
            .into()),
        }
    }
}

/// The counts upstream's `collectStatistics` derives from a manifest's
/// scene: sdk7 counts the composite's `core::Transform` and
/// `core::GltfContainer` entries, sdk6 the typed components (minus the
/// ground tiles, as upstream).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Statistics {
    pub parcels: i32,
    pub transforms: i32,
    pub gltf_shapes: i32,
    pub nft_shapes: i32,
    pub scripts: i32,
    pub entities: i32,
}

pub fn statistics(cols: i32, rows: i32, scene: &Value) -> Statistics {
    let parcels = cols.saturating_mul(rows);
    let count = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
    if let Some(sdk6) = scene.get("sdk6").filter(|v| v.is_object()) {
        let mut stats = Statistics {
            parcels,
            entities: count(
                sdk6.get("entities")
                    .and_then(Value::as_object)
                    .map_or(0, |e| e.len()),
            ) - parcels,
            ..Statistics::default()
        };
        for component in sdk6
            .get("components")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|c| c.values())
        {
            match component.get("type").and_then(Value::as_str) {
                Some("GLTFShape") => stats.gltf_shapes += 1,
                Some("NFTShape") => stats.nft_shapes += 1,
                Some("Script") => stats.scripts += 1,
                Some("Transform") => stats.transforms += 1,
                _ => {}
            }
        }
        stats.transforms -= parcels;
        stats.gltf_shapes -= 1;
        return stats;
    }
    let components = scene
        .pointer("/sdk7/composite/components")
        .and_then(Value::as_array);
    let entries = |name: &str| {
        components
            .and_then(|c| {
                c.iter()
                    .find(|c| c.get("name").and_then(Value::as_str) == Some(name))
            })
            .and_then(|c| c.get("data").and_then(Value::as_object))
            .map_or(0, |d| count(d.len()))
    };
    Statistics {
        parcels,
        transforms: entries("core::Transform"),
        gltf_shapes: entries("core::GltfContainer"),
        ..Statistics::default()
    }
}

/// The live (not soft-deleted) project, any owner.
pub async fn live_project(pool: &PgPool, id: Uuid) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {PROJECT_COLUMNS} FROM builder.projects WHERE id = $1 AND NOT is_deleted"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Upstream's `withProjectExists` + `withProjectAuthorization`: the live
/// project, 404 without one, 401 when it is another wallet's.
pub async fn owned_project(
    pool: &PgPool,
    raw_id: &str,
    signer: &str,
) -> Result<Project, ProjectError> {
    let id = parse_id(raw_id)?;
    let project = live_project(pool, id)
        .await?
        .ok_or_else(|| project_not_found(raw_id))?;
    if project.eth_address != signer {
        return Err(unauthorized_for(signer, "projects", raw_id));
    }
    Ok(project)
}

/// `GET /v1/projects` query: upstream's `limit`/`offset` (limit capped at
/// 100) and the equality filters builder clients send.
#[derive(Debug, Default)]
pub struct ProjectQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub title: Option<String>,
    pub creation_coords: Option<String>,
    pub is_template: Option<bool>,
    pub template_status: Option<String>,
}

impl ProjectQuery {
    pub fn from_params(params: &HashMap<String, String>) -> Self {
        let num = |k: &str| params.get(k).and_then(|v| v.trim().parse::<i64>().ok());
        Self {
            limit: num("limit").map(|l| l.clamp(0, MAX_PAGE)),
            offset: num("offset").map(|o| o.max(0)),
            title: params.get("title").cloned(),
            creation_coords: params.get("creation_coords").cloned(),
            is_template: params.get("is_template").and_then(|v| v.parse().ok()),
            template_status: params.get("template_status").cloned(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProjectPage {
    pub items: Vec<Project>,
    pub total: i64,
}

const OWNER_FILTER: &str = "eth_address = $1 AND NOT is_deleted \
    AND ($2::text IS NULL OR title = $2) AND ($3::text IS NULL OR creation_coords = $3) \
    AND ($4::boolean IS NULL OR is_template = $4) AND ($5::text IS NULL OR template_status = $5)";

/// The signer's live projects, newest edit first (`{items, total}`).
pub async fn list_projects(
    pool: &PgPool,
    owner: &str,
    q: &ProjectQuery,
) -> Result<ProjectPage, sqlx::Error> {
    let items: Vec<Project> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {PROJECT_COLUMNS} FROM builder.projects WHERE {OWNER_FILTER} \
         ORDER BY updated_at DESC, id LIMIT $6 OFFSET $7"
    )))
    .bind(owner)
    .bind(&q.title)
    .bind(&q.creation_coords)
    .bind(q.is_template)
    .bind(&q.template_status)
    .bind(q.limit.unwrap_or(MAX_PAGE * 10))
    .bind(q.offset.unwrap_or(0))
    .fetch_all(pool)
    .await?;
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM builder.projects WHERE {OWNER_FILTER}"
    )))
    .bind(owner)
    .bind(&q.title)
    .bind(&q.creation_coords)
    .bind(q.is_template)
    .bind(&q.template_status)
    .fetch_one(pool)
    .await?;
    Ok(ProjectPage { items, total })
}

/// The owner writing a soft-deleted project again starts a new one under
/// the same id (a /create draft re-made under a deleted one's name): the
/// old manifest, files and packs go, the optional fields and statistics
/// reset, `created_at` is now (a sync client that remembers the old project
/// tells the two apart by it) and `files_version` moves on, never reused.
/// The caller's upsert then clears `is_deleted`.
async fn restart_deleted(
    conn: &mut PgConnection,
    owner: &str,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    let restarted = sqlx::query(
        "UPDATE builder.projects SET thumbnail = NULL, is_public = false, creation_coords = NULL, \
            is_template = false, video = NULL, template_status = NULL, parcels = 0, transforms = 0, \
            gltf_shapes = 0, nft_shapes = 0, scripts = 0, entities = 0, \
            files_version = files_version + 1, created_at = timezone('UTC', now()) \
         WHERE id = $1 AND eth_address = $2 AND is_deleted",
    )
    .bind(id)
    .bind(owner)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if restarted == 0 {
        return Ok(());
    }
    for statement in [
        "DELETE FROM builder.project_files WHERE project_id = $1",
        "DELETE FROM builder.project_manifests WHERE project_id = $1",
        "DELETE FROM builder.asset_packs WHERE project_id = $1",
    ] {
        sqlx::query(statement).bind(id).execute(&mut *conn).await?;
    }
    Ok(())
}

/// Upstream's `Project.upsert` behind `Ownable.canUpsert`: insert, or
/// update the signer's own row (reviving a soft-deleted one, which
/// [`restart_deleted`] emptied first). With
/// `statistics` (a manifest PUT) the counts are rewritten and, with
/// `manifest_base`, `manifest_revision` is checked against it and bumped in
/// the same statement. `None`: the row is another wallet's, or its revision
/// moved.
async fn upsert(
    conn: &mut PgConnection,
    owner: &str,
    id: Uuid,
    p: &ProjectIn,
    statistics: Option<Statistics>,
    manifest_base: Option<i64>,
) -> Result<Option<Project>, ProjectError> {
    let thumbnail = p.thumbnail()?;
    let s = statistics;
    let result = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO builder.projects AS p \
            (id, eth_address, title, description, scene_id, cols, rows, thumbnail, is_public, \
             creation_coords, is_template, video, template_status, parcels, transforms, gltf_shapes, \
             nft_shapes, scripts, entities, manifest_revision) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9, false), $10, COALESCE($11, false), $12, \
                 $13, COALESCE($14, 0), COALESCE($15, 0), COALESCE($16, 0), COALESCE($17, 0), \
                 COALESCE($18, 0), COALESCE($19, 0), CASE WHEN $20::bigint IS NULL THEN 0 ELSE 1 END) \
         ON CONFLICT (id) DO UPDATE SET \
            title = EXCLUDED.title, description = EXCLUDED.description, \
            scene_id = EXCLUDED.scene_id, cols = EXCLUDED.cols, rows = EXCLUDED.rows, \
            thumbnail = COALESCE($8, p.thumbnail), is_public = COALESCE($9, p.is_public), \
            creation_coords = COALESCE($10, p.creation_coords), \
            is_template = COALESCE($11, p.is_template), video = COALESCE($12, p.video), \
            template_status = COALESCE($13, p.template_status), \
            parcels = COALESCE($14, p.parcels), transforms = COALESCE($15, p.transforms), \
            gltf_shapes = COALESCE($16, p.gltf_shapes), nft_shapes = COALESCE($17, p.nft_shapes), \
            scripts = COALESCE($18, p.scripts), entities = COALESCE($19, p.entities), \
            manifest_revision = p.manifest_revision + CASE WHEN $20::bigint IS NULL THEN 0 ELSE 1 END, \
            is_deleted = false, updated_at = timezone('UTC', now()) \
         WHERE p.eth_address = $2 AND ($20::bigint IS NULL OR p.manifest_revision = $20) \
         RETURNING {PROJECT_COLUMNS}"
    )))
    .bind(id)
    .bind(owner)
    .bind(&p.title)
    .bind(&p.description)
    .bind(p.scene_id)
    .bind(p.cols)
    .bind(p.rows)
    .bind(thumbnail)
    .bind(p.is_public)
    .bind(&p.creation_coords)
    .bind(p.is_template)
    .bind(&p.video)
    .bind(&p.template_status)
    .bind(s.map(|s| s.parcels))
    .bind(s.map(|s| s.transforms))
    .bind(s.map(|s| s.gltf_shapes))
    .bind(s.map(|s| s.nft_shapes))
    .bind(s.map(|s| s.scripts))
    .bind(s.map(|s| s.entities))
    .bind(manifest_base)
    .fetch_optional(&mut *conn)
    .await;
    match result {
        Ok(row) => Ok(row),
        Err(e) if is_violation(&e, FK_VIOLATION) => Err(ApiError::bad_request(format!(
            "thumbnail {} is not an uploaded content",
            thumbnail.unwrap_or_default()
        ))
        .into()),
        Err(e) if is_violation(&e, CHECK_VIOLATION) => Err(invalid_schema(e)),
        Err(e) => Err(e.into()),
    }
}

/// `PUT /v1/projects/{id}`: upstream's upsertProject.
pub async fn put_project(
    pool: &PgPool,
    owner: &str,
    id: Uuid,
    body: Value,
) -> Result<Project, ProjectError> {
    let project = body
        .get("project")
        .cloned()
        .ok_or_else(|| invalid_schema("the body has no project"))?;
    let project = ProjectIn::parse(project)?;
    if project.id != id {
        return Err(ProjectError::data(
            400,
            "The body and URL project ids do not match",
            json!({ "urlId": id, "bodyId": project.id }),
        ));
    }
    let mut tx = pool.begin().await?;
    restart_deleted(&mut tx, owner, id).await?;
    let saved = upsert(&mut tx, owner, id, &project, None, None)
        .await?
        .ok_or_else(|| {
            ProjectError::data(
                401,
                "Unauthorized user",
                json!({ "id": id, "eth_address": owner }),
            )
        })?;
    tx.commit().await?;
    Ok(saved)
}

/// `DELETE /v1/projects/{id}`: upstream's soft delete.
pub async fn delete_project(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE builder.projects SET is_deleted = true, updated_at = timezone('UTC', now()) WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// `DELETE /v1/projects/{coords}/coords`: forget the coordinates the
/// signer's projects were created at.
pub async fn clear_creation_coords(
    pool: &PgPool,
    owner: &str,
    coords: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE builder.projects SET creation_coords = NULL, updated_at = timezone('UTC', now()) \
         WHERE eth_address = $1 AND NOT is_deleted AND creation_coords = $2",
    )
    .bind(owner)
    .bind(coords)
    .execute(pool)
    .await?
    .rows_affected())
}

// ── manifest ──────────────────────────────────────────────────────────────

/// The stored manifest with its `project` rendered from the row.
fn render_manifest(mut stored: Value, project: &Project) -> Value {
    if let Some(obj) = stored.as_object_mut() {
        obj.insert(
            "project".into(),
            serde_json::to_value(project).unwrap_or(Value::Null),
        );
    }
    stored
}

/// The project's builder manifest, `None` when it has none.
pub async fn manifest(pool: &PgPool, project: &Project) -> Result<Option<Value>, sqlx::Error> {
    let stored: Option<Value> =
        sqlx::query_scalar("SELECT manifest FROM builder.project_manifests WHERE project_id = $1")
            .bind(project.id)
            .fetch_optional(pool)
            .await?;
    Ok(stored.map(|m| render_manifest(m, project)))
}

/// `GET /v1/manifests`: the signer's live projects' manifests.
pub async fn owner_manifests(pool: &PgPool, owner: &str) -> Result<Vec<Value>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct WithManifest {
        #[sqlx(flatten)]
        project: Project,
        manifest: Value,
    }
    let rows: Vec<WithManifest> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT p.*, m.manifest FROM (SELECT {PROJECT_COLUMNS} FROM builder.projects \
         WHERE eth_address = $1 AND NOT is_deleted) p \
         JOIN builder.project_manifests m ON m.project_id = p.id ORDER BY p.updated_at DESC, p.id"
    )))
    .bind(owner)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| render_manifest(r.manifest, &r.project))
        .collect())
}

/// What a manifest PUT requires of the current revision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Precondition {
    Any,
    /// `If-Match: "N"`: the live manifest is at revision N.
    Revision(i64),
    /// `If-None-Match: *`: there is no live manifest.
    Absent,
}

impl Precondition {
    pub fn from_headers(headers: &HeaderMap) -> Result<Self, ProjectError> {
        if let Some(v) = headers.get(header::IF_NONE_MATCH) {
            return match v.to_str().map(str::trim) {
                Ok("*") => Ok(Self::Absent),
                _ => Err(ApiError::bad_request("If-None-Match takes only *").into()),
            };
        }
        let Some(v) = headers.get(header::IF_MATCH) else {
            return Ok(Self::Any);
        };
        v.to_str()
            .ok()
            .map(|s| s.trim().trim_start_matches("W/").trim_matches('"'))
            .and_then(|s| s.parse::<i64>().ok())
            .map(Self::Revision)
            .ok_or_else(|| ApiError::bad_request("If-Match takes the manifest's ETag").into())
    }

    fn holds(self, live: Option<i64>) -> bool {
        match self {
            Self::Any => true,
            Self::Revision(r) => live == Some(r),
            Self::Absent => live.is_none(),
        }
    }
}

/// The 412 a stale precondition answers: the server's manifest (or null)
/// and its revision.
fn manifest_conflict(current: Option<Value>, revision: i64) -> ProjectError {
    ProjectError::Data {
        status: 412,
        message: format!(
            "The project's manifest is at revision {revision}: merge it and retry with If-Match: \"{revision}\""
        ),
        data: json!({ "revision": revision, "manifest": current }),
        etag: Some(revision),
    }
}

/// `PUT /v1/projects/{id}/manifest`: upstream's upsertManifest (the
/// project upserted from the manifest's `project` with the scene's
/// statistics, the manifest stored) under `precondition`.
pub async fn put_manifest(
    pool: &PgPool,
    owner: &str,
    id: Uuid,
    body: Value,
    precondition: Precondition,
) -> Result<Project, ProjectError> {
    let mut stored = body
        .get("manifest")
        .cloned()
        .filter(Value::is_object)
        .ok_or_else(|| invalid_schema("the body has no manifest object"))?;
    let doc = stored.as_object_mut().expect("checked object");
    if !doc.get("version").is_some_and(Value::is_number) {
        return Err(invalid_schema("manifest.version must be a number"));
    }
    if !doc.get("scene").is_some_and(Value::is_object) {
        return Err(invalid_schema("manifest.scene must be an object"));
    }
    let mut project = doc
        .remove("project")
        .filter(Value::is_object)
        .ok_or_else(|| invalid_schema("manifest.project must be an object"))?;
    project["id"] = json!(id);
    let project = ProjectIn::parse(project)?;
    let stats = statistics(project.cols, project.rows, &doc["scene"]);

    let mut tx = pool.begin().await?;
    let seen: Option<(String, bool, i64, bool)> = sqlx::query_as(
        "SELECT p.eth_address, p.is_deleted, p.manifest_revision, \
                EXISTS (SELECT 1 FROM builder.project_manifests m WHERE m.project_id = p.id) \
         FROM builder.projects p WHERE p.id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((eth_address, ..)) = &seen
        && eth_address != owner
    {
        return Err(ProjectError::data(
            401,
            "Unauthorized user",
            json!({ "id": id, "eth_address": owner }),
        ));
    }
    let base = seen.as_ref().map_or(0, |s| s.2);
    let live = seen
        .as_ref()
        .filter(|(_, deleted, _, has)| !deleted && *has)
        .map(|s| s.2);
    if !precondition.holds(live) {
        drop(tx);
        let current = match live {
            Some(_) => match live_project(pool, id).await? {
                Some(p) => manifest(pool, &p).await?,
                None => None,
            },
            None => None,
        };
        return Err(manifest_conflict(current, base));
    }
    restart_deleted(&mut tx, owner, id).await?;
    let Some(saved) = upsert(&mut tx, owner, id, &project, Some(stats), Some(base)).await? else {
        // Another writer created or moved the row between the probe and the upsert.
        drop(tx);
        return Err(manifest_conflict(None, base + 1));
    };
    sqlx::query(
        "INSERT INTO builder.project_manifests (project_id, manifest) VALUES ($1, $2) \
         ON CONFLICT (project_id) DO UPDATE SET manifest = EXCLUDED.manifest",
    )
    .bind(id)
    .bind(&stored)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(saved)
}

/// `DELETE /v1/projects/{id}/manifest`: drops the document, bumps the
/// revision (a later PUT never reuses an ETag).
pub async fn delete_manifest(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query("DELETE FROM builder.project_manifests WHERE project_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted > 0 {
        sqlx::query(
            "UPDATE builder.projects SET manifest_revision = manifest_revision + 1, \
             updated_at = timezone('UTC', now()) WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

// ── files ─────────────────────────────────────────────────────────────────

/// A project's file set: `path -> hash` at `version`, and the project's
/// `created_at`, which changes when a deleted project is started again
/// under its id ([`restart_deleted`]).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileSet {
    pub version: i64,
    #[serde(serialize_with = "js_date")]
    pub created_at: NaiveDateTime,
    pub files: BTreeMap<String, String>,
}

/// The body of `PUT .../files`.
#[derive(Debug, Deserialize)]
pub struct FileSetIn {
    pub base_version: i64,
    pub files: BTreeMap<String, String>,
}

/// `project_files_path_check`: relative, `/`-separated, no empty, `.` or
/// `..` segment, no backslash, at most 512 characters.
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.chars().count() <= MAX_PATH_CHARS
        && !path.contains(['\\', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

pub async fn file_set(
    pool: &PgPool,
    project: Uuid,
    version: i64,
    created_at: NaiveDateTime,
) -> Result<FileSet, sqlx::Error> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT path, hash FROM builder.project_files WHERE project_id = $1")
            .bind(project)
            .fetch_all(pool)
            .await?;
    Ok(FileSet {
        version,
        created_at,
        files: rows.into_iter().collect(),
    })
}

/// `PUT .../files`: replaces the set when `base_version` is still the
/// project's, else 409 with the server's set; every hash must be an upload
/// (400 naming the missing ones).
pub async fn put_files(
    pool: &PgPool,
    project: &Project,
    body: FileSetIn,
) -> Result<FileSet, ProjectError> {
    if body.files.len() > MAX_PROJECT_FILES {
        return Err(ApiError::bad_request(format!(
            "a project holds at most {MAX_PROJECT_FILES} files"
        ))
        .into());
    }
    if let Some(bad) = body.files.keys().find(|p| !valid_path(p)) {
        return Err(ApiError::bad_request(format!(
            "invalid path {bad:?}: relative, '/'-separated, no empty, '.' or '..' segment, at most {MAX_PATH_CHARS} characters"
        ))
        .into());
    }
    if let Some((path, _)) = body.files.iter().find(|(_, h)| !is_canonical_cid(h)) {
        return Err(
            ApiError::bad_request(format!("the hash of {path:?} is not a canonical CID")).into(),
        );
    }
    let mut tx = pool.begin().await?;
    let version: Option<i64> = sqlx::query_scalar(
        "UPDATE builder.projects SET files_version = files_version + 1, updated_at = timezone('UTC', now()) \
         WHERE id = $1 AND files_version = $2 AND NOT is_deleted RETURNING files_version",
    )
    .bind(project.id)
    .bind(body.base_version)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(version) = version else {
        drop(tx);
        let (current, created_at, deleted): (i64, NaiveDateTime, bool) = sqlx::query_as(
            "SELECT files_version, created_at, is_deleted FROM builder.projects WHERE id = $1",
        )
        .bind(project.id)
        .fetch_one(pool)
        .await?;
        if deleted {
            return Err(project_not_found(&project.id.to_string()));
        }
        let server = file_set(pool, project.id, current, created_at).await?;
        return Err(ProjectError::data(
            409,
            format!(
                "The project's files are at version {current}, not {}: merge the server's set and retry",
                body.base_version
            ),
            serde_json::to_value(server).unwrap_or(Value::Null),
        ));
    };
    let (paths, hashes): (Vec<String>, Vec<String>) = body.files.into_iter().unzip();
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT h FROM unnest($1::text[]) AS h \
         WHERE NOT EXISTS (SELECT 1 FROM builder.uploaded_contents u WHERE u.hash = h) ORDER BY h",
    )
    .bind(&hashes)
    .fetch_all(&mut *tx)
    .await?;
    if !missing.is_empty() {
        return Err(ProjectError::data(
            400,
            "Upload the missing contents first (PUT .../blobs/{hash})",
            json!({ "missing": missing }),
        ));
    }
    sqlx::query("DELETE FROM builder.project_files WHERE project_id = $1")
        .bind(project.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO builder.project_files (project_id, path, hash, size) \
         SELECT $1, f.path, f.hash, u.size FROM unnest($2::text[], $3::text[]) AS f(path, hash) \
         JOIN builder.uploaded_contents u ON u.hash = f.hash",
    )
    .bind(project.id)
    .bind(&paths)
    .bind(&hashes)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(FileSet {
        version,
        created_at: project.created_at,
        files: paths.into_iter().zip(hashes).collect(),
    })
}

/// `PUT .../blobs/{hash}`: the raw bytes, accepted when they hash to the
/// CIDv1 the path names; stored in the ONE store and recorded in
/// `uploaded_contents` (idempotent). Answers the size.
pub async fn accept_blob(
    pool: &PgPool,
    blobs: &Storage,
    owner: &str,
    hash: &str,
    bytes: Bytes,
) -> Result<i64, ProjectError> {
    if !is_canonical_cid(hash) {
        return Err(ApiError::bad_request("hash is not a canonical CID").into());
    }
    if bytes.len() > MAX_BLOB_BYTES {
        return Err(
            ApiError::http(413, format!("a content is at most {MAX_BLOB_BYTES} bytes")).into(),
        );
    }
    if hash_bytes_v1(&bytes) != hash {
        return Err(ApiError::bad_request(format!(
            "the body does not hash to {hash} (CIDv1, raw)"
        ))
        .into());
    }
    let size = bytes.len() as i64;
    if !blobs
        .exist(hash)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        blobs
            .store(hash, bytes)
            .await
            .map_err(|e| ApiError::internal(format!("storing {hash}: {e}")))?;
    }
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, $2, $3) \
         ON CONFLICT (hash) DO NOTHING",
    )
    .bind(hash)
    .bind(size)
    .bind(owner)
    .execute(pool)
    .await?;
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_follow_upstream_collect_statistics() {
        let sdk7 = json!({ "sdk7": { "composite": { "components": [
            { "name": "core::Transform", "data": { "512": {}, "513": {}, "514": {} } },
            { "name": "core::GltfContainer", "data": { "513": {} } },
        ] } } });
        assert_eq!(
            statistics(2, 3, &sdk7),
            Statistics {
                parcels: 6,
                transforms: 3,
                gltf_shapes: 1,
                ..Statistics::default()
            }
        );
        let sdk6 = json!({ "sdk6": {
            "entities": { "a": {}, "b": {}, "c": {} },
            "components": {
                "t1": { "type": "Transform" }, "t2": { "type": "Transform" },
                "g": { "type": "GLTFShape" }, "g2": { "type": "GLTFShape" },
                "n": { "type": "NFTShape" }, "s": { "type": "Script" },
            },
        } });
        assert_eq!(
            statistics(1, 1, &sdk6),
            Statistics {
                parcels: 1,
                transforms: 1,
                gltf_shapes: 1,
                nft_shapes: 1,
                scripts: 1,
                entities: 2,
            }
        );
        assert_eq!(statistics(1, 1, &json!({})).parcels, 1);
    }

    #[test]
    fn paths_follow_the_schema_check() {
        for ok in [
            "a",
            "a/b.txt",
            ".hidden/x",
            "x..y/z",
            "assets/scene/main.composite",
        ] {
            assert!(valid_path(ok), "{ok}");
        }
        let long = "a".repeat(513);
        for bad in [
            "",
            "../x",
            "a/./b",
            "/abs",
            "a//b",
            "a/",
            "a\\b",
            long.as_str(),
        ] {
            assert!(!valid_path(bad), "{bad}");
        }
    }

    #[test]
    fn preconditions_read_the_etag_forms() {
        let mut h = HeaderMap::new();
        assert_eq!(Precondition::from_headers(&h).unwrap(), Precondition::Any);
        h.insert(header::IF_MATCH, HeaderValue::from_static("W/\"7\""));
        assert_eq!(
            Precondition::from_headers(&h).unwrap(),
            Precondition::Revision(7)
        );
        h.insert(header::IF_NONE_MATCH, HeaderValue::from_static("*"));
        assert_eq!(
            Precondition::from_headers(&h).unwrap(),
            Precondition::Absent
        );
        assert!(Precondition::Absent.holds(None));
        assert!(!Precondition::Absent.holds(Some(1)));
        assert!(Precondition::Revision(2).holds(Some(2)));
        assert!(!Precondition::Revision(2).holds(None));
    }
}
