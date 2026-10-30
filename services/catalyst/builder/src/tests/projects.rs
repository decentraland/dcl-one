//! The project routes over the router (both mounts, signed fetch, the
//! writes gate open) and the scratch database: upstream's project and
//! manifest contracts, the file-set sync API's optimistic concurrency, the
//! per-project asset packs, and the in-place upgrade of a database that
//! predates the project tables.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{OWNER_KEY, signed_map, temp_storage, unique, wallet};
use crate::{Settings, SharedState, State};
use dcl_auth::hash_bytes_v1;

const OTHER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
/// The listing test's own wallet: the other tests' projects never enter
/// its pages.
const LIST_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";

fn owner_address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

async fn app() -> Option<(SharedState, Router)> {
    let pool = crate::db::scratch::pool(4).await?;
    let state: SharedState = Arc::new(
        State::new(
            pool,
            temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            None,
            None,
            Settings {
                writes_enabled: true,
                ..Settings::default()
            },
        )
        .unwrap(),
    );
    let router = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state.clone()));
    Some((state, router))
}

struct Reply {
    status: StatusCode,
    etag: Option<String>,
    cache: Option<String>,
    body: Value,
    raw: Vec<u8>,
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    key: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<Body>,
    json_body: bool,
) -> Reply {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        let local = path.split('?').next().unwrap();
        for (name, value) in signed_map(key, &method.to_ascii_lowercase(), local, "{}", false).await
        {
            request = request.header(name, value);
        }
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    if json_body {
        request = request.header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(request.body(body.unwrap_or_else(Body::empty)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let header = |name| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let etag = header(axum::http::header::ETAG);
    let cache = header(axum::http::header::CACHE_CONTROL);
    let raw = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap()
        .to_vec();
    Reply {
        status,
        etag,
        cache,
        body: serde_json::from_slice(&raw).unwrap_or(Value::Null),
        raw,
    }
}

async fn get(app: &Router, path: &str, key: Option<&str>) -> Reply {
    call(app, "GET", path, key, &[], None, false).await
}

async fn send(app: &Router, method: &str, path: &str, key: &str, body: Value) -> Reply {
    send_with(app, method, path, key, &[], body).await
}

async fn send_with(
    app: &Router,
    method: &str,
    path: &str,
    key: &str,
    headers: &[(&str, &str)],
    body: Value,
) -> Reply {
    call(
        app,
        method,
        path,
        Some(key),
        headers,
        Some(Body::from(body.to_string())),
        true,
    )
    .await
}

fn project_body(id: Uuid, title: &str) -> Value {
    json!({ "project": {
        "id": id, "title": title, "description": "a test scene", "scene_id": Uuid::new_v4(),
        "cols": 2, "rows": 1, "thumbnail": "", "is_public": false, "created_at": "2001-01-01T00:00:00.000Z",
    } })
}

async fn drop_projects(state: &SharedState, ids: &[Uuid], hashes: &[String]) {
    let pool = state.items.pool();
    sqlx::query("DELETE FROM builder.projects WHERE id = ANY($1)")
        .bind(ids)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = ANY($1)")
        .bind(hashes)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn project_crud_follows_upstream_on_both_mounts() {
    let Some((state, app)) = app().await else {
        return;
    };
    let owner = owner_address(LIST_KEY);
    let other = owner_address(OTHER_KEY);
    let id = Uuid::new_v4();
    let path = format!("/v1/projects/{id}");

    let put = send(&app, "PUT", &path, LIST_KEY, project_body(id, "First")).await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.body["ok"], true);
    assert_eq!(put.body["data"]["eth_address"], owner);
    assert_eq!(put.body["data"]["parcels"], 0);
    assert_eq!(put.body["data"]["manifest_revision"], 0);
    assert_ne!(
        put.body["data"]["created_at"], "2001-01-01T00:00:00.000Z",
        "the dates are the server's"
    );
    assert!(
        put.body["data"]["created_at"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );

    let read = get(&app, &format!("/builder-api{path}"), Some(LIST_KEY)).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    assert_eq!(read.body["data"]["title"], "First");
    assert_eq!(read.cache.as_deref(), Some("private, no-store"));

    let foreign = get(&app, &path, Some(OTHER_KEY)).await;
    assert_eq!(foreign.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        foreign.body["error"],
        format!("Unauthorized user {other} for projects {id}")
    );
    let stolen = send(&app, "PUT", &path, OTHER_KEY, project_body(id, "Mine")).await;
    assert_eq!(stolen.status, StatusCode::UNAUTHORIZED);
    assert_eq!(stolen.body["error"], "Unauthorized user");
    let unsigned = get(&app, &path, None).await;
    assert_eq!(
        unsigned.status,
        StatusCode::BAD_REQUEST,
        "no chain: the shared verifier's refusal, as every signed builder route: {}",
        unsigned.body
    );

    let missing = Uuid::new_v4();
    let not_found = get(&app, &format!("/v1/projects/{missing}"), Some(LIST_KEY)).await;
    assert_eq!(not_found.status, StatusCode::NOT_FOUND);
    assert_eq!(
        not_found.body,
        json!({ "ok": false, "data": { "id": missing.to_string(), "tableName": "projects" },
                "error": format!("Couldn't find \"{missing}\" on projects"),
                "message": format!("Couldn't find \"{missing}\" on projects") })
    );
    let bad = get(&app, "/v1/projects/not-a-uuid", Some(LIST_KEY)).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.body["error"], "Invalid uuid not-a-uuid");
    let mismatch = send(
        &app,
        "PUT",
        &path,
        LIST_KEY,
        project_body(Uuid::new_v4(), "x"),
    )
    .await;
    assert_eq!(mismatch.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        mismatch.body["error"],
        "The body and URL project ids do not match"
    );
    let mut invalid = project_body(id, "x");
    invalid["project"]["cols"] = json!(0);
    assert_eq!(
        send(&app, "PUT", &path, LIST_KEY, invalid).await.status,
        StatusCode::BAD_REQUEST
    );

    let second = Uuid::new_v4();
    let mut coords = project_body(second, "Second");
    coords["project"]["creation_coords"] = json!("10,20");
    coords["project"]["is_public"] = json!(true);
    assert_eq!(
        send(
            &app,
            "PUT",
            &format!("/builder-api/v1/projects/{second}"),
            LIST_KEY,
            coords
        )
        .await
        .status,
        StatusCode::OK
    );
    let page = get(&app, "/v1/projects?limit=1", Some(LIST_KEY)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert_eq!(page.body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        page.body["data"]["items"][0]["id"],
        second.to_string(),
        "newest edit first"
    );
    let total = page.body["data"]["total"].as_i64().unwrap();
    assert!(total >= 2);
    let filtered = get(&app, "/v1/projects?creation_coords=10,20", Some(LIST_KEY)).await;
    assert!(
        filtered.body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["creation_coords"] == "10,20")
    );
    let theirs = get(&app, "/v1/projects", Some(OTHER_KEY)).await;
    assert!(
        theirs.body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["eth_address"] == other)
    );

    let public = get(&app, &format!("/v1/projects/{second}/public"), None).await;
    assert_eq!(public.status, StatusCode::OK);
    assert_eq!(
        get(&app, &format!("{path}/public"), None).await.status,
        StatusCode::NOT_FOUND
    );

    let cleared = call(
        &app,
        "DELETE",
        "/v1/projects/10,20/coords",
        Some(LIST_KEY),
        &[],
        None,
        false,
    )
    .await;
    assert_eq!(cleared.body, json!({ "ok": true, "data": true }));
    let after = get(&app, &format!("/v1/projects/{second}"), Some(LIST_KEY)).await;
    assert_eq!(after.body["data"]["creation_coords"], Value::Null);

    assert_eq!(
        call(&app, "DELETE", &path, Some(OTHER_KEY), &[], None, false)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let deleted = call(&app, "DELETE", &path, Some(LIST_KEY), &[], None, false).await;
    assert_eq!(deleted.body, json!({ "ok": true, "data": true }));
    assert_eq!(
        get(&app, &path, Some(LIST_KEY)).await.status,
        StatusCode::NOT_FOUND
    );
    let fewer = get(&app, "/v1/projects", Some(LIST_KEY)).await;
    assert_eq!(fewer.body["data"]["total"].as_i64().unwrap(), total - 1);
    assert!(
        !fewer.body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == id.to_string())
    );
    let stolen_deleted = send(&app, "PUT", &path, OTHER_KEY, project_body(id, "Mine")).await;
    assert_eq!(
        stolen_deleted.status,
        StatusCode::UNAUTHORIZED,
        "a soft-deleted project stays its owner's"
    );
    let revived = send(&app, "PUT", &path, LIST_KEY, project_body(id, "Back")).await;
    assert_eq!(revived.status, StatusCode::OK);
    assert_eq!(revived.body["data"]["is_deleted"], false);

    drop_projects(&state, &[id, second], &[]).await;
}

fn manifest(title: &str, transforms: usize) -> Value {
    let data: serde_json::Map<String, Value> = (0..transforms)
        .map(|i| (format!("{}", 512 + i), json!({ "json": {} })))
        .collect();
    json!({ "manifest": {
        "version": 11,
        "project": { "id": Uuid::nil(), "title": title, "description": "", "scene_id": Uuid::new_v4(),
                     "cols": 1, "rows": 2, "eth_address": "0xspoofed" },
        "scene": { "sdk7": { "id": "s", "composite": { "version": 1, "components": [
            { "name": "core::Transform", "jsonSchema": {}, "data": data },
        ] }, "mappings": {} } },
    } })
}

#[tokio::test]
async fn manifests_round_trip_under_revision_preconditions() {
    let Some((state, app)) = app().await else {
        return;
    };
    let owner = owner_address(OWNER_KEY);
    let id = Uuid::new_v4();
    let path = format!("/v1/projects/{id}/manifest");

    let first = send_with(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        &[("if-none-match", "*")],
        manifest("Draft", 3),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(first.etag.as_deref(), Some("\"1\""));
    assert_eq!(
        first.body["data"]["id"],
        id.to_string(),
        "the URL names the project"
    );
    assert_eq!(
        first.body["data"]["eth_address"], owner,
        "the signer owns it"
    );
    assert_eq!(first.body["data"]["transforms"], 3);
    assert_eq!(first.body["data"]["parcels"], 2);
    assert_eq!(first.body["data"]["manifest_revision"], 1);

    let read = get(&app, &format!("/builder-api{path}"), Some(OWNER_KEY)).await;
    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(read.etag.as_deref(), Some("\"1\""));
    assert_eq!(read.body["version"], 11, "the raw manifest, no envelope");
    assert_eq!(read.body["project"]["title"], "Draft");
    assert_eq!(read.body["project"]["eth_address"], owner);
    assert_eq!(read.body["scene"]["sdk7"]["id"], "s");
    assert_eq!(
        get(&app, &path, Some(OTHER_KEY)).await.status,
        StatusCode::UNAUTHORIZED
    );

    let again = send_with(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        &[("if-none-match", "*")],
        manifest("Other", 1),
    )
    .await;
    assert_eq!(again.status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(again.body["data"]["revision"], 1);
    assert_eq!(again.body["data"]["manifest"]["project"]["title"], "Draft");
    assert_eq!(again.etag.as_deref(), Some("\"1\""));
    let stale = send_with(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        &[("if-match", "\"0\"")],
        manifest("Other", 1),
    )
    .await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    let next = send_with(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        &[("if-match", "\"1\"")],
        manifest("Next", 1),
    )
    .await;
    assert_eq!(next.status, StatusCode::OK, "{}", next.body);
    assert_eq!(next.etag.as_deref(), Some("\"2\""));
    let unconditional = send(&app, "PUT", &path, OWNER_KEY, manifest("Upstream", 0)).await;
    assert_eq!(
        unconditional.status,
        StatusCode::OK,
        "upstream's PUT has no precondition"
    );
    assert_eq!(unconditional.body["data"]["manifest_revision"], 3);
    assert_eq!(
        send(&app, "PUT", &path, OTHER_KEY, manifest("Theirs", 0))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    let listed = get(&app, "/v1/manifests", Some(OWNER_KEY)).await;
    assert!(
        listed.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["project"]["id"] == id.to_string() && m["project"]["title"] == "Upstream")
    );
    let project = get(&app, &format!("/v1/projects/{id}"), Some(OWNER_KEY)).await;
    assert_eq!(
        project.body["data"]["title"], "Upstream",
        "the manifest upserts the project"
    );

    let deleted = call(&app, "DELETE", &path, Some(OWNER_KEY), &[], None, false).await;
    assert_eq!(deleted.body, json!({ "ok": true, "data": true }));
    assert_eq!(
        get(&app, &path, Some(OWNER_KEY)).await.status,
        StatusCode::NOT_FOUND
    );
    let recreate = send_with(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        &[("if-none-match", "*")],
        manifest("Again", 0),
    )
    .await;
    assert_eq!(recreate.status, StatusCode::OK, "{}", recreate.body);
    assert_eq!(
        recreate.etag.as_deref(),
        Some("\"5\""),
        "a deleted manifest's revision is never reused"
    );

    let mut large = manifest("Large", 1);
    large["manifest"]["scene"]["sdk7"]["mappings"] = json!({ "blob": "x".repeat(3 << 20) });
    let saved = send(&app, "PUT", &path, OWNER_KEY, large).await;
    assert_eq!(
        saved.status,
        StatusCode::OK,
        "a composite past axum's 2 MiB default is a manifest too"
    );
    let read = get(&app, &path, Some(OWNER_KEY)).await;
    assert_eq!(
        read.body["scene"]["sdk7"]["mappings"]["blob"]
            .as_str()
            .map(str::len),
        Some(3 << 20)
    );

    drop_projects(&state, &[id], &[]).await;
}

#[tokio::test]
async fn file_sets_sync_by_hash_with_a_base_version() {
    let Some((state, app)) = app().await else {
        return;
    };
    let id = Uuid::new_v4();
    let base = format!("/v1/projects/{id}");
    assert_eq!(
        send(&app, "PUT", &base, OWNER_KEY, project_body(id, "Files"))
            .await
            .status,
        StatusCode::OK
    );
    let empty = get(&app, &format!("{base}/files"), Some(OWNER_KEY)).await;
    assert_eq!(empty.body["ok"], true);
    assert_eq!(empty.body["data"]["version"], 0);
    assert_eq!(empty.body["data"]["files"], json!({}));
    let created_at = empty.body["data"]["created_at"].clone();
    assert!(
        created_at.as_str().is_some_and(|t| t.ends_with('Z')),
        "the project's creation, which a sync client remembers: {created_at}"
    );

    let bytes = unique("scene main");
    let hash = hash_bytes_v1(&bytes);
    let wrong = call(
        &app,
        "PUT",
        &format!("{base}/blobs/{}", hash_bytes_v1(b"other")),
        Some(OWNER_KEY),
        &[],
        Some(Body::from(bytes.clone())),
        false,
    )
    .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    let foreign = call(
        &app,
        "PUT",
        &format!("{base}/blobs/{hash}"),
        Some(OTHER_KEY),
        &[],
        Some(Body::from(bytes.clone())),
        false,
    )
    .await;
    assert_eq!(foreign.status, StatusCode::UNAUTHORIZED);

    let set = json!({ "base_version": 0, "files": { "src/index.ts": hash, "assets/a.bin": hash } });
    let missing = send(
        &app,
        "PUT",
        &format!("{base}/files"),
        OWNER_KEY,
        set.clone(),
    )
    .await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST, "{}", missing.body);
    assert_eq!(missing.body["data"]["missing"], json!([hash]));
    assert_eq!(
        get(&app, &format!("{base}/files"), Some(OWNER_KEY))
            .await
            .body["data"]["version"],
        0,
        "a refused set leaves the version"
    );

    let uploaded = call(
        &app,
        "PUT",
        &format!("/builder-api{base}/blobs/{hash}"),
        Some(OWNER_KEY),
        &[],
        Some(Body::from(bytes.clone())),
        false,
    )
    .await;
    assert_eq!(uploaded.status, StatusCode::OK, "{}", uploaded.body);
    assert_eq!(
        uploaded.body["data"],
        json!({ "hash": hash, "size": bytes.len() })
    );
    let served = get(&app, &format!("/v1/storage/contents/{hash}"), None).await;
    assert_eq!(served.status, StatusCode::OK);
    assert_eq!(served.raw, bytes);

    let put = send(
        &app,
        "PUT",
        &format!("{base}/files"),
        OWNER_KEY,
        set.clone(),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.body["data"]["version"], 1);
    let conflict = send(
        &app,
        "PUT",
        &format!("{base}/files"),
        OWNER_KEY,
        json!({ "base_version": 0, "files": {} }),
    )
    .await;
    assert_eq!(conflict.status, StatusCode::CONFLICT);
    assert_eq!(conflict.body["data"]["version"], 1);
    assert_eq!(
        conflict.body["data"]["files"]["src/index.ts"],
        hash.as_str()
    );

    let read = get(&app, &format!("/builder-api{base}/files"), Some(OWNER_KEY)).await;
    assert_eq!(
        read.body["data"],
        json!({ "version": 1, "created_at": created_at,
                "files": { "assets/a.bin": hash, "src/index.ts": hash } })
    );
    let size: i64 = sqlx::query_scalar(
        "SELECT size FROM builder.project_files WHERE project_id = $1 AND path = 'src/index.ts'",
    )
    .bind(id)
    .fetch_one(state.items.pool())
    .await
    .unwrap();
    assert_eq!(size, bytes.len() as i64);

    for bad in ["../escape", "/abs", "a//b", "a\\b"] {
        let refused = send(
            &app,
            "PUT",
            &format!("{base}/files"),
            OWNER_KEY,
            json!({ "base_version": 1, "files": { bad: hash } }),
        )
        .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let removed = send(
        &app,
        "PUT",
        &format!("{base}/files"),
        OWNER_KEY,
        json!({ "base_version": 1, "files": { "src/index.ts": hash } }),
    )
    .await;
    assert_eq!(
        removed.body["data"],
        json!({ "version": 2, "created_at": created_at, "files": { "src/index.ts": hash } })
    );
    assert_eq!(
        get(&app, &format!("{base}/files"), Some(OTHER_KEY))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    drop_projects(&state, &[id], &[hash]).await;
}

#[tokio::test]
async fn project_asset_packs_follow_the_defaults() {
    let Some((state, app)) = app().await else {
        return;
    };
    let owner = owner_address(OWNER_KEY);
    let (id, other_project) = (Uuid::new_v4(), Uuid::new_v4());
    for (project, key) in [(id, OWNER_KEY), (other_project, OTHER_KEY)] {
        let put = send(
            &app,
            "PUT",
            &format!("/v1/projects/{project}"),
            key,
            project_body(project, "Packs"),
        )
        .await;
        assert_eq!(put.status, StatusCode::OK);
    }
    let bytes = unique("pack model");
    let hash = hash_bytes_v1(&bytes);
    let blob = call(
        &app,
        "PUT",
        &format!("/v1/projects/{id}/blobs/{hash}"),
        Some(OWNER_KEY),
        &[],
        Some(Body::from(bytes)),
        false,
    )
    .await;
    assert_eq!(blob.status, StatusCode::OK);

    let pack = Uuid::new_v4();
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    let asset = |asset: Uuid, name: &str| {
        json!({ "id": asset, "asset_pack_id": Uuid::new_v4(), "name": name, "model": "model.glb",
                "thumbnail": null, "tags": ["tree"], "category": "decorations",
                "contents": { "model.glb": hash }, "metrics": { "triangles": 12 } })
    };
    let body = json!({ "assetPack": { "id": pack, "title": "My pack", "thumbnail": hash,
        "assets": [asset(first, "Tree one"), asset(second, "Tree two")] } });
    let path = format!("/v1/projects/{id}/assetPacks/{pack}");
    let put = send(&app, "PUT", &path, OWNER_KEY, body.clone()).await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.body["data"]["eth_address"], owner);
    assert_eq!(put.body["data"]["assets"][1]["id"], second.to_string());
    assert_eq!(
        put.body["data"]["assets"][0]["asset_pack_id"],
        pack.to_string(),
        "the URL's pack"
    );
    assert_eq!(
        put.body["data"]["assets"][0]["contents"]["model.glb"],
        hash.as_str()
    );

    let listed = get(
        &app,
        &format!("/builder-api/v1/projects/{id}/assetPacks"),
        Some(OWNER_KEY),
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    let packs = listed.body["data"].as_array().unwrap();
    assert_eq!(
        packs.last().unwrap()["id"],
        pack.to_string(),
        "the defaults first, then the project's"
    );
    assert_eq!(packs.last().unwrap()["assets"].as_array().unwrap().len(), 2);
    assert_eq!(
        get(
            &app,
            &format!("/v1/projects/{id}/assetPacks"),
            Some(OTHER_KEY)
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );

    let shrink = json!({ "assetPack": { "id": pack, "title": "My pack", "assets": [asset(second, "Tree two")] } });
    let shrunk = send(&app, "PUT", &path, OWNER_KEY, shrink).await;
    assert_eq!(
        shrunk.body["data"]["assets"].as_array().unwrap().len(),
        1,
        "unlisted assets go"
    );
    assert_eq!(shrunk.body["data"]["thumbnail"], Value::Null);

    let elsewhere = json!({ "assetPack": { "id": Uuid::new_v4(), "title": "Thief", "assets": [asset(second, "Tree two")] } });
    let elsewhere_id = elsewhere["assetPack"]["id"].as_str().unwrap().to_string();
    let stolen = send(
        &app,
        "PUT",
        &format!("/v1/projects/{id}/assetPacks/{elsewhere_id}"),
        OWNER_KEY,
        elsewhere,
    )
    .await;
    assert_eq!(stolen.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        stolen.body["error"],
        "One of the assets you're trying to upload is invalid"
    );
    let cross = send(
        &app,
        "PUT",
        &format!("/v1/projects/{other_project}/assetPacks/{pack}"),
        OTHER_KEY,
        json!({ "assetPack": { "id": pack, "title": "Taken", "assets": [] } }),
    )
    .await;
    assert_eq!(
        cross.status,
        StatusCode::UNAUTHORIZED,
        "another project's pack"
    );
    let unknown_hash = hash_bytes_v1(&unique("never uploaded"));
    let mut unuploaded = asset(Uuid::new_v4(), "Ghost");
    unuploaded["contents"] = json!({ "model.glb": unknown_hash });
    let refused = send(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        json!({ "assetPack": { "id": pack, "title": "My pack", "assets": [unuploaded] } }),
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{}", refused.body);

    let deleted = call(&app, "DELETE", &path, Some(OWNER_KEY), &[], None, false).await;
    assert_eq!(deleted.body, json!({ "ok": true, "data": true }));
    let after = get(
        &app,
        &format!("/v1/projects/{id}/assetPacks"),
        Some(OWNER_KEY),
    )
    .await;
    assert!(
        !after.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == pack.to_string())
    );

    for _ in 0..10 {
        let fresh = Uuid::new_v4();
        let put = send(
            &app,
            "PUT",
            &format!("/v1/projects/{id}/assetPacks/{fresh}"),
            OWNER_KEY,
            json!({ "assetPack": { "id": fresh, "title": "Filler", "assets": [] } }),
        )
        .await;
        assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    }
    let revived = send(
        &app,
        "PUT",
        &path,
        OWNER_KEY,
        json!({ "assetPack": { "id": pack, "title": "My pack", "assets": [] } }),
    )
    .await;
    assert_eq!(
        revived.status,
        StatusCode::BAD_REQUEST,
        "reviving a deleted pack counts against the cap: {}",
        revived.body
    );
    assert_eq!(
        revived.body["error"],
        "Too many asset packs. The max amount is 10"
    );

    drop_projects(&state, &[id, other_project], &[hash]).await;
}

/// A deleted project PUT again under its id is a new project: nothing of
/// the old one (manifest, files, packs, optional fields) comes back, its
/// `created_at` is new and `files_version` never repeats.
#[tokio::test]
async fn a_deleted_project_comes_back_empty() {
    let Some((state, app)) = app().await else {
        return;
    };
    let id = Uuid::new_v4();
    let base = format!("/v1/projects/{id}");
    let mut first = manifest("Old", 2);
    first["manifest"]["project"]["is_public"] = json!(true);
    first["manifest"]["project"]["creation_coords"] = json!("3,4");
    let made = send_with(
        &app,
        "PUT",
        &format!("{base}/manifest"),
        OWNER_KEY,
        &[("if-none-match", "*")],
        first,
    )
    .await;
    assert_eq!(made.status, StatusCode::OK, "{}", made.body);
    let bytes = unique("old file");
    let hash = hash_bytes_v1(&bytes);
    let blob = call(
        &app,
        "PUT",
        &format!("{base}/blobs/{hash}"),
        Some(OWNER_KEY),
        &[],
        Some(Body::from(bytes)),
        false,
    )
    .await;
    assert_eq!(blob.status, StatusCode::OK);
    let files = send(
        &app,
        "PUT",
        &format!("{base}/files"),
        OWNER_KEY,
        json!({ "base_version": 0, "files": { "a.txt": hash } }),
    )
    .await;
    assert_eq!(files.status, StatusCode::OK, "{}", files.body);
    let pack = Uuid::new_v4();
    let packed = send(
        &app,
        "PUT",
        &format!("{base}/assetPacks/{pack}"),
        OWNER_KEY,
        json!({ "assetPack": { "id": pack, "title": "Old pack", "assets": [] } }),
    )
    .await;
    assert_eq!(packed.status, StatusCode::OK, "{}", packed.body);
    let created = |state: SharedState| async move {
        sqlx::query_scalar::<_, chrono::NaiveDateTime>(
            "SELECT created_at FROM builder.projects WHERE id = $1",
        )
        .bind(id)
        .fetch_one(state.items.pool())
        .await
        .unwrap()
    };
    let before = created(state.clone()).await;

    let deleted = call(&app, "DELETE", &base, Some(OWNER_KEY), &[], None, false).await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(
        get(&app, &format!("{base}/files"), Some(OWNER_KEY))
            .await
            .status,
        StatusCode::NOT_FOUND,
        "a sync client finds the project gone"
    );

    let again = send(&app, "PUT", &base, OWNER_KEY, project_body(id, "New")).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    assert_eq!(again.body["data"]["is_deleted"], false);
    assert_eq!(again.body["data"]["is_public"], false);
    assert_eq!(again.body["data"]["creation_coords"], Value::Null);
    assert_eq!(again.body["data"]["transforms"], 0);
    assert!(created(state.clone()).await > before, "a new created_at");
    assert_eq!(
        get(&app, &format!("{base}/manifest"), Some(OWNER_KEY))
            .await
            .status,
        StatusCode::NOT_FOUND,
        "the old manifest stays gone"
    );
    let set = get(&app, &format!("{base}/files"), Some(OWNER_KEY)).await;
    assert_eq!(set.body["data"]["files"], json!({}));
    assert_eq!(
        set.body["data"]["version"], 2,
        "the old set was version 1; the restart moved past it"
    );
    let packs = get(&app, &format!("{base}/assetPacks"), Some(OWNER_KEY)).await;
    assert!(
        !packs.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == pack.to_string())
    );

    let gone = call(&app, "DELETE", &base, Some(OWNER_KEY), &[], None, false).await;
    assert_eq!(gone.status, StatusCode::OK);
    let draft = send_with(
        &app,
        "PUT",
        &format!("{base}/manifest"),
        OWNER_KEY,
        &[("if-none-match", "*")],
        manifest("Draft again", 1),
    )
    .await;
    assert_eq!(
        draft.status,
        StatusCode::OK,
        "a /create draft re-made under a deleted one's id: {}",
        draft.body
    );
    let read = get(&app, &format!("{base}/manifest"), Some(OWNER_KEY)).await;
    assert_eq!(read.body["project"]["title"], "Draft again");
    assert_eq!(read.body["project"]["transforms"], 1);

    drop_projects(&state, &[id], &[hash]).await;
}

#[tokio::test]
async fn writes_stay_behind_the_gate() {
    let state = super::support::state_without_db(None).await;
    let app = crate::root_router(state);
    let id = Uuid::new_v4();
    let put = send(
        &app,
        "PUT",
        &format!("/v1/projects/{id}"),
        OWNER_KEY,
        project_body(id, "Gated"),
    )
    .await;
    assert_eq!(put.status, StatusCode::SERVICE_UNAVAILABLE);
}

/// A database stamped before the project tables existed gets them from
/// `catalyst schema migrate` (no step: new tables are created objects),
/// with its rows kept.
#[tokio::test]
async fn an_existing_database_gains_the_project_tables_in_place() {
    let Some(url) = crate::db::scratch::url().await else {
        return;
    };
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let name = format!(
        "catalyst_test_lib_{}_{}",
        std::process::id(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let mut own = url::Url::parse(&url).unwrap();
    own.set_path(&format!("/{name}"));
    let pool = crate::db::connect_pool(own.as_str(), &crate::db::PoolSettings::default())
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    sqlx::raw_sql(
        "DROP TABLE builder.asset_contents, builder.assets, builder.asset_packs, \
         builder.project_files, builder.project_manifests, builder.projects; \
         INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) \
         VALUES ('bafkreiaaaa', 1, '0x0000000000000000000000000000000000000000')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before = crate::db::schema_migrate::gaps(&pool, crate::db::SCHEMA_SQL)
        .await
        .unwrap();
    assert!(
        before
            .fatal()
            .iter()
            .any(|g| g.contains("builder.projects")),
        "{:?}",
        before.fatal()
    );
    let done =
        crate::db::schema_migrate::migrate(&pool, crate::db::SCHEMA_SQL, crate::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert!(done.steps.is_empty(), "{:?}", done.steps);
    let created: Vec<String> = done.created.iter().map(|o| format!("{o:?}")).collect();
    for table in [
        "builder.projects",
        "builder.project_manifests",
        "builder.project_files",
        "builder.asset_packs",
        "builder.assets",
        "builder.asset_contents",
    ] {
        assert!(
            created.iter().any(|c| c.contains(table)),
            "{table} not in {created:?}"
        );
    }
    let after = crate::db::schema_migrate::gaps(&pool, crate::db::SCHEMA_SQL)
        .await
        .unwrap();
    assert!(
        after.fatal().is_empty() && after.indexes().is_empty(),
        "{after:?}"
    );
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM builder.uploaded_contents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kept, 1);
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{name}\" WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
}
