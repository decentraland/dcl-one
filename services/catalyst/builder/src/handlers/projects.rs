//! `/v1/projects*` and `/v1/manifests`: upstream builder-server's Project,
//! Manifest and (per project) AssetPack routes plus the file-set sync API;
//! the store is [`crate::projects`].

use std::collections::HashMap;

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, Uri};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde_json::{Value, json};

use super::drafts::signer;
use crate::SharedState;
use crate::http::ApiData;
use crate::project_packs::{self, PackIn};
use crate::projects::{
    self as store, FileSet, FileSetIn, Precondition, Project, ProjectError, ProjectPage,
    ProjectQuery, invalid_schema, owned_project, parse_id, project_not_found, set_etag,
};

type Reply<T> = Result<Json<ApiData<T>>, ProjectError>;

async fn owner(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    original: &Uri,
    uri: &Uri,
) -> Result<String, ProjectError> {
    Ok(signer(state, headers, method, original, uri)
        .await?
        .as_str()
        .to_string())
}

pub async fn list(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Reply<ProjectPage> {
    let owner = owner(&state, &headers, "get", &original, &uri).await?;
    let query = ProjectQuery::from_params(&params);
    Ok(Json(ApiData::ok(
        store::list_projects(state.items.pool(), &owner, &query).await?,
    )))
}

pub async fn get(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<Project> {
    let owner = owner(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        owned_project(state.items.pool(), &id, &owner).await?,
    )))
}

/// Upstream's `GET /projects/:id/public`: any caller, public projects only.
pub async fn get_public(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Reply<Project> {
    let uuid = parse_id(&id)?;
    match store::live_project(state.items.pool(), uuid).await? {
        Some(p) if p.is_public => Ok(Json(ApiData::ok(p))),
        _ => Err(project_not_found(&id)),
    }
}

pub async fn put(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Reply<Project> {
    let owner = owner(&state, &headers, "put", &original, &uri).await?;
    let id = parse_id(&id)?;
    Ok(Json(ApiData::ok(
        store::put_project(state.items.pool(), &owner, id, body).await?,
    )))
}

pub async fn delete(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<bool> {
    let owner = owner(&state, &headers, "delete", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    store::delete_project(state.items.pool(), project.id).await?;
    Ok(Json(ApiData::ok(true)))
}

/// Upstream's `DELETE /projects/:coords/coords` (the segment is the
/// coordinates, `x,y`).
pub async fn delete_coords(
    State(state): State<SharedState>,
    Path(coords): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<bool> {
    let owner = owner(&state, &headers, "delete", &original, &uri).await?;
    store::clear_creation_coords(state.items.pool(), &owner, &coords).await?;
    Ok(Json(ApiData::ok(true)))
}

// ── manifest ──────────────────────────────────────────────────────────────

/// The raw manifest (upstream answers it unwrapped, from its bucket), the
/// revision as the ETag a conditional PUT names.
pub async fn get_manifest(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ProjectError> {
    let owner = owner(&state, &headers, "get", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    let manifest = store::manifest(state.items.pool(), &project)
        .await?
        .ok_or_else(|| project_not_found(&id))?;
    let mut resp = Json(manifest).into_response();
    set_etag(&mut resp, project.manifest_revision);
    Ok(resp)
}

pub async fn get_manifests(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<Vec<Value>> {
    let owner = owner(&state, &headers, "get", &original, &uri).await?;
    Ok(Json(ApiData::ok(
        store::owner_manifests(state.items.pool(), &owner).await?,
    )))
}

pub async fn put_manifest(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response, ProjectError> {
    let owner = owner(&state, &headers, "put", &original, &uri).await?;
    let id = parse_id(&id)?;
    let precondition = Precondition::from_headers(&headers)?;
    let project = store::put_manifest(state.items.pool(), &owner, id, body, precondition).await?;
    let revision = project.manifest_revision;
    let mut resp = Json(ApiData::ok(project)).into_response();
    set_etag(&mut resp, revision);
    Ok(resp)
}

pub async fn delete_manifest(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<bool> {
    let owner = owner(&state, &headers, "delete", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    store::delete_manifest(state.items.pool(), project.id).await?;
    Ok(Json(ApiData::ok(true)))
}

// ── files ─────────────────────────────────────────────────────────────────

pub async fn get_files(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<FileSet> {
    let owner = owner(&state, &headers, "get", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    Ok(Json(ApiData::ok(
        store::file_set(
            state.items.pool(),
            project.id,
            project.files_version,
            project.created_at,
        )
        .await?,
    )))
}

pub async fn put_files(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Reply<FileSet> {
    let owner = owner(&state, &headers, "put", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    let body: FileSetIn = serde_json::from_value(body).map_err(invalid_schema)?;
    Ok(Json(ApiData::ok(
        store::put_files(state.items.pool(), &project, body).await?,
    )))
}

/// The raw bytes of one file, named by their CIDv1. The project's owner
/// uploads; the bytes read back through `/v1/storage/contents/{hash}`.
pub async fn put_blob(
    State(state): State<SharedState>,
    Path((id, hash)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Reply<Value> {
    let owner = owner(&state, &headers, "put", &original, &uri).await?;
    owned_project(state.items.pool(), &id, &owner).await?;
    let size =
        store::accept_blob(state.items.pool(), &state.items.blobs, &owner, &hash, body).await?;
    Ok(Json(ApiData::ok(json!({ "hash": hash, "size": size }))))
}

// ── asset packs ───────────────────────────────────────────────────────────

/// The default packs every project sees, then the project's own.
pub async fn get_asset_packs(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<Vec<Value>> {
    let owner = owner(&state, &headers, "get", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    let own = project_packs::project_packs(state.items.pool(), project.id).await?;
    let defaults = state.catalog.asset_packs().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "builder catalogue unreadable; project packs only");
        None
    });
    Ok(Json(ApiData::ok(project_packs::with_defaults(
        defaults.as_deref(),
        own,
    ))))
}

pub async fn put_asset_pack(
    State(state): State<SharedState>,
    Path((id, pack)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Reply<Value> {
    let owner = owner(&state, &headers, "put", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    let pack = PackIn::parse(body, parse_id(&pack)?)?;
    Ok(Json(ApiData::ok(
        project_packs::put_pack(state.items.pool(), &project, &owner, pack).await?,
    )))
}

pub async fn delete_asset_pack(
    State(state): State<SharedState>,
    Path((id, pack)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Reply<bool> {
    let owner = owner(&state, &headers, "delete", &original, &uri).await?;
    let project = owned_project(state.items.pool(), &id, &owner).await?;
    project_packs::delete_pack(state.items.pool(), &project, &owner, parse_id(&pack)?).await?;
    Ok(Json(ApiData::ok(true)))
}
