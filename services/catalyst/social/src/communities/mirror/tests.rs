//! The communities mirror against a stub upstream on the loopback (no live
//! network): the listing's parsing, then (DB-backed through `db::scratch`,
//! skipped without `CATALYST_TEST_DATABASE_URL`) one flow over a pass, the
//! read routes, the write refusal and the local join, a colliding local
//! community, a local suspension, resume from the persisted cursor, the
//! sweep, its fuse, and an upstream fault. The flow is one test because the sweep is scoped to
//! every mirrored row of the (shared, per-process) scratch database.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State as AxState};
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::*;
use crate::communities::{Settings, State as CommunitiesState, router};

/// Two rows of the live listing as captured (2026-09-30).
const CAPTURED: &str = r#"{"data":{"results":[{"id":"e99471aa-31c4-4952-abf6-99905445f43b","name":"Decentraland Foundation","description":"Your hub for official Decentraland updates and events.","ownerAddress":"0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda","privacy":"public","active":true,"membersCount":1310,"ownerName":"DCLOfficial","voiceChatStatus":{"isActive":false,"participantCount":0,"moderatorCount":0}},{"id":"c1bf2ebe-d1d2-4edc-9488-1047689640cb","name":"Chiri Storytelling","description":"This space brings to life Chiri's adventures.","ownerAddress":"0xfbf7e572a63ca6a4e21a83720c0f23dd644e32e0","privacy":"public","active":true,"membersCount":404,"ownerName":"Chiri","voiceChatStatus":{"isActive":false,"participantCount":0,"moderatorCount":0}}],"total":143,"page":1,"pages":72,"limit":2}}"#;

#[test]
fn the_captured_listing_parses_into_storable_rows() {
    let body: Value = serde_json::from_str(CAPTURED).unwrap();
    let (rows, total) = parse_page(&body).unwrap();
    assert_eq!(total, Some(143));
    let parsed: Vec<_> = rows.iter().filter_map(extract).collect();
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed[0],
        UpstreamCommunity {
            id: Uuid::parse_str("e99471aa-31c4-4952-abf6-99905445f43b").unwrap(),
            name: "Decentraland Foundation".into(),
            description: "Your hub for official Decentraland updates and events.".into(),
            owner_address: "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda".into(),
            private: false,
            active: true,
            members_count: 1310,
        }
    );
}

#[test]
fn a_row_without_a_uuid_owner_or_name_is_skipped_and_fields_are_normalised() {
    let ok = json!({"id": "c1bf2ebe-d1d2-4edc-9488-1047689640cb", "name": " N ",
                    "ownerAddress": "0xFBF7E572A63CA6A4E21A83720C0F23DD644E32E0",
                    "privacy": "private", "membersCount": -4});
    let r = extract(&ok).unwrap();
    assert_eq!(
        r.owner_address,
        "0xfbf7e572a63ca6a4e21a83720c0f23dd644e32e0"
    );
    assert_eq!(r.name, "N");
    assert!(r.private && r.active);
    assert_eq!((r.members_count, r.description.as_str()), (0, ""));
    for bad in [
        json!({"id": "not-a-uuid", "name": "n", "ownerAddress": "0xfbf7e572a63ca6a4e21a83720c0f23dd644e32e0"}),
        json!({"id": "c1bf2ebe-d1d2-4edc-9488-1047689640cb", "name": "n", "ownerAddress": "0x12"}),
        json!({"id": "c1bf2ebe-d1d2-4edc-9488-1047689640cb", "name": "  ", "ownerAddress": "0xfbf7e572a63ca6a4e21a83720c0f23dd644e32e0"}),
    ] {
        assert!(extract(&bad).is_none(), "{bad}");
    }
    assert!(parse_page(&json!({"data": {}})).is_err());
    assert!(parse_page(&json!({"ok": true})).is_err());
}

#[test]
fn only_a_community_scoped_path_names_a_written_community() {
    let id = Uuid::new_v4();
    let me = "0x00000000000000000000000000000000000000a1";
    for (method, p, membership) in [
        (Method::DELETE, format!("/v1/communities/{id}"), false),
        (Method::POST, format!("/v1/communities/{id}/members"), true),
        (
            Method::DELETE,
            format!("/v1/communities/{id}/members/{me}"),
            true,
        ),
        (
            Method::PATCH,
            format!("/v1/communities/{id}/members/{me}"),
            false,
        ),
        (
            Method::POST,
            format!("/v1/communities/{id}/members/{me}/bans"),
            false,
        ),
        (
            Method::POST,
            format!("/v1/communities/{id}/posts/x/like"),
            false,
        ),
    ] {
        assert_eq!(
            written_community(&method, &p),
            Some((id, membership)),
            "{method} {p}"
        );
    }
    for p in [
        "/v1/communities".to_string(),
        "/v1/communities/not-a-uuid/members".to_string(),
        format!("/v1/admin/communities/{id}/suspend"),
        format!("/v1/members/{id}/communities"),
    ] {
        assert_eq!(written_community(&Method::POST, &p), None, "{p}");
    }
}

#[derive(Default)]
struct Stub {
    rows: Vec<Value>,
    offsets: Vec<i64>,
    thumbs: HashSet<Uuid>,
    fail: bool,
}

type Shared = Arc<Mutex<Stub>>;

async fn list(AxState(s): AxState<Shared>, Query(q): Query<HashMap<String, i64>>) -> Response {
    let mut s = s.lock().unwrap();
    if s.fail {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let offset = q.get("offset").copied().unwrap_or(0);
    let limit = q.get("limit").copied().unwrap_or(100).min(PAGE);
    s.offsets.push(offset);
    let page: Vec<Value> = s
        .rows
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .cloned()
        .collect();
    axum::Json(json!({"data": {"results": page, "total": s.rows.len(), "limit": limit}}))
        .into_response()
}

async fn thumb(AxState(s): AxState<Shared>, Path(id): Path<Uuid>) -> Response {
    if s.lock().unwrap().thumbs.contains(&id) {
        (
            [(header::CONTENT_TYPE, "image/png")],
            vec![0x89u8, b'P', b'N', b'G'],
        )
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn stub(shared: Shared) -> MirrorConfig {
    let app = Router::new()
        .route("/v1/communities", get(list))
        .route("/social/communities/{id}/raw-thumbnail.png", get(thumb))
        .with_state(shared);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    MirrorConfig {
        source: MirrorSource::unchecked(&base),
        thumbnail_base: MirrorSource::unchecked(&base),
        interval: Duration::from_secs(3600),
    }
}

fn upstream_row(id: Uuid, tag: &str, i: usize) -> Value {
    json!({
        "id": id.to_string(),
        "name": format!("{tag} {i:03}"),
        "description": "mirrored",
        "ownerAddress": format!("0x{:040x}", i + 1),
        "privacy": "public",
        "active": true,
        "membersCount": 1000 + i,
        "ownerName": "someone",
        "voiceChatStatus": {"isActive": false, "participantCount": 0, "moderatorCount": 0}
    })
}

async fn call(app: &Router, method: &str, uri: &str) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// `call` signed by the test wallet (`test_support::address`).
async fn signed_call(app: &Router, method: &str, uri: &str) -> StatusCode {
    let headers = crate::shared::signed_fetch::test_support::signed_headers(
        &method.to_lowercase(),
        uri,
        "{}",
        true,
    )
    .await;
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    app.clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

async fn scalar_i64(pool: &PgPool, sql: &str, id: Uuid) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_string()))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn active(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT active FROM social.communities WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_pass_copies_the_listing_read_only_and_resumes_sweeps_and_fuses() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let storage_dir = std::env::temp_dir().join(format!("catalyst-mirror-{}", Uuid::new_v4()));
    let storage = Arc::new(
        crate::storage::Storage::from_config(&storage_dir)
            .await
            .unwrap(),
    );
    let state = Arc::new(CommunitiesState::new(
        pool.clone(),
        storage,
        Settings::for_tests(),
        None,
        &Outbound::for_tests(),
        CommunitiesState::explore_for_tests(&pool),
    ));
    let app = Router::new()
        .merge(router(state.clone()))
        .nest("/social-api", router(state));

    // A local community whose id the upstream also lists.
    let local = Uuid::new_v4();
    let local_owner = format!("0x{}", &Uuid::new_v4().simple().to_string()[..8]).to_string()
        + "00000000000000000000000000000000";
    sqlx::query(
        "INSERT INTO social.communities (id, name, owner_address) VALUES ($1, 'local one', $2)",
    )
    .bind(local)
    .bind(&local_owner)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'owner')",
    )
    .bind(local)
    .bind(&local_owner)
    .execute(&pool)
    .await
    .unwrap();

    let tag = format!("Mirror{}", &Uuid::new_v4().simple().to_string()[..8]);
    let ids: Vec<Uuid> = (0..150).map(|_| Uuid::new_v4()).collect();
    let shared: Shared = Arc::default();
    {
        let mut s = shared.lock().unwrap();
        s.rows = ids
            .iter()
            .enumerate()
            .map(|(i, id)| upstream_row(*id, &tag, i))
            .collect();
        s.rows.push(upstream_row(local, "upstream copy", 999));
        s.rows
            .push(json!({"id": "broken", "name": "x", "ownerAddress": "nope"}));
        s.thumbs = ids.iter().step_by(2).copied().collect();
    }
    let cfg = stub(shared.clone()).await;
    let client = Outbound::for_tests();
    let cancel = CancellationToken::new();
    let hour = Duration::from_secs(3600);

    // 1. A complete pass: 150 mirrored, the local collision left alone, the
    // broken row skipped, the present thumbnails found.
    let out = run_pass(&client, &pool, &cfg, hour, &cancel).await.unwrap();
    assert!(out.complete, "{out:?}");
    assert_eq!(
        (out.upserted, out.local_conflicts, out.skipped_rows),
        (150, 1, 1)
    );
    assert_eq!(out.thumbnails_found, 75);
    assert_eq!(shared.lock().unwrap().offsets, vec![0, 100]);
    let (name, owner): (String, String) =
        sqlx::query_as("SELECT name, owner_address FROM social.communities WHERE id = $1")
            .bind(local)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (name.as_str(), owner.as_str()),
        ("local one", local_owner.as_str())
    );
    let marks = "SELECT count(*) FROM social.community_mirror WHERE community_id = $1";
    assert_eq!(scalar_i64(&pool, marks, local).await, 0);
    let roles = "SELECT count(*) FROM social.community_members WHERE community_id = $1";
    assert_eq!(scalar_i64(&pool, roles, ids[0]).await, 0, "no local role");

    // 2. The read routes: the upstream count and thumbnail.
    let (status, body) = call(&app, "GET", &format!("/v1/communities/{}", ids[0])).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["membersCount"], 1000);
    assert_eq!(body["data"]["ownerAddress"], format!("0x{:040x}", 1));
    assert_eq!(
        body["data"]["thumbnailUrl"],
        format!(
            "{}/social/communities/{}/raw-thumbnail.png",
            cfg.source.base(),
            ids[0]
        )
    );
    let (_, body) = call(&app, "GET", &format!("/v1/communities/{}", ids[1])).await;
    assert!(body["data"].get("thumbnailUrl").is_none(), "{body}");
    let (status, body) = call(
        &app,
        "GET",
        &format!("/v1/communities?search={tag}%2000&limit=20"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let results = body["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 10, "{body}");
    assert_eq!(results[0]["name"], format!("{tag} 000"));
    assert_eq!(results[0]["membersCount"], 1000);
    assert!(
        results[0]["thumbnailUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1")
    );

    // 3. A community-scoped write on a mirrored row answers 409 naming the
    // upstream before any handler, under both mounts; a local community's is
    // not refused. A join and a leave are local member rows the count adds
    // to the upstream's.
    let me = crate::shared::signed_fetch::test_support::address();
    for (method, uri) in [
        ("DELETE", format!("/v1/communities/{}", ids[0])),
        ("PUT", format!("/social-api/v1/communities/{}", ids[0])),
        ("POST", format!("/v1/communities/{}/posts", ids[0])),
        ("PATCH", format!("/v1/communities/{}/members/{me}", ids[0])),
        (
            "POST",
            format!("/v1/communities/{}/members/{me}/bans", ids[0]),
        ),
    ] {
        let (status, body) = call(&app, method, &uri).await;
        assert_eq!(status, StatusCode::CONFLICT, "{method} {uri}: {body}");
        assert!(body.to_string().contains(cfg.source.base()), "{body}");
    }
    let (status, _) = call(&app, "POST", &format!("/v1/communities/{local}/members")).await;
    assert_ne!(status, StatusCode::CONFLICT);
    let members = format!("/v1/communities/{}/members", ids[0]);
    assert_eq!(
        signed_call(&app, "POST", &members).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        scalar_i64(&pool, roles, ids[0]).await,
        1,
        "a local member row"
    );
    let (_, body) = call(&app, "GET", &format!("/v1/communities/{}", ids[0])).await;
    assert_eq!(
        body["data"]["membersCount"], 1001,
        "upstream + local: {body}"
    );
    assert_eq!(
        signed_call(&app, "DELETE", &format!("{members}/{me}")).await,
        StatusCode::NO_CONTENT
    );
    let (_, body) = call(&app, "GET", &format!("/v1/communities/{}", ids[0])).await;
    assert_eq!(body["data"]["membersCount"], 1000);
    let leaver = format!("/v1/communities/{}/members", ids[145]);
    assert_eq!(
        signed_call(&app, "POST", &leaver).await,
        StatusCode::NO_CONTENT
    );

    // 4. A local suspension survives a pass; a changed upstream field lands.
    sqlx::query(
        "UPDATE social.communities SET suspended = true, suspended_at = now(), suspended_by = 'op' WHERE id = $1",
    )
    .bind(ids[2])
    .execute(&pool)
    .await
    .unwrap();
    shared.lock().unwrap().rows[3]["name"] = json!(format!("{tag} renamed"));
    shared.lock().unwrap().offsets.clear();

    // 5. An interrupted pass resumes at its cursor.
    sqlx::query(
        "UPDATE social.community_mirror_state SET pass_started_at = $2, next_offset = 100, pass_rows = 100 \
         WHERE source_url = $1",
    )
    .bind(cfg.source.base())
    .bind(Utc::now().naive_utc())
    .execute(&pool)
    .await
    .unwrap();
    let out = run_pass(&client, &pool, &cfg, hour, &cancel).await.unwrap();
    assert_eq!(out.resumed_from, Some(100));
    assert_eq!(shared.lock().unwrap().offsets, vec![100]);
    let out = run_pass(&client, &pool, &cfg, hour, &cancel).await.unwrap();
    assert!(out.complete && out.resumed_from.is_none());
    let suspended: bool =
        sqlx::query_scalar("SELECT suspended FROM social.communities WHERE id = $1")
            .bind(ids[2])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(suspended, "the pass never writes the operator axes");
    let renamed: String = sqlx::query_scalar("SELECT name FROM social.communities WHERE id = $1")
        .bind(ids[3])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(renamed, format!("{tag} renamed"));

    // 6. An upstream fault: incomplete, nothing deactivated.
    shared.lock().unwrap().fail = true;
    let out = run_pass(&client, &pool, &cfg, Duration::ZERO, &cancel)
        .await
        .unwrap();
    assert!(!out.complete && out.failed_pages > 0 && out.deactivated == 0);
    let error: Option<String> = sqlx::query_scalar(
        "SELECT last_error FROM social.community_mirror_state WHERE source_url = $1",
    )
    .bind(cfg.source.base())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(error.is_some());
    shared.lock().unwrap().fail = false;

    // 7. Communities the listing dropped are deactivated (not deleted) by a
    // complete pass, their local members kept but inert; listed again, they
    // come back with them.
    let dropped: Vec<Value> = shared.lock().unwrap().rows.drain(140..150).collect();
    let out = run_pass(&client, &pool, &cfg, Duration::ZERO, &cancel)
        .await
        .unwrap();
    assert!(out.complete && !out.sweep_refused, "{out:?}");
    assert_eq!(out.deactivated, 10);
    let mut swept = out.deactivated_ids.clone();
    swept.sort();
    let mut expected = ids[140..150].to_vec();
    expected.sort();
    assert_eq!(swept, expected, "the ids the worlds hook closes");
    assert!(!active(&pool, ids[145]).await && active(&pool, ids[0]).await);
    assert_eq!(
        scalar_i64(&pool, roles, ids[145]).await,
        1,
        "the local member stays with the inactive community"
    );
    assert!(
        active(&pool, local).await,
        "a local community is never swept"
    );
    let (status, _) = call(&app, "GET", &format!("/v1/communities/{}", ids[145])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    shared.lock().unwrap().rows.splice(140..140, dropped);
    run_pass(&client, &pool, &cfg, Duration::ZERO, &cancel)
        .await
        .unwrap();
    assert!(active(&pool, ids[145]).await);
    let (_, body) = call(&app, "GET", &format!("/v1/communities/{}", ids[145])).await;
    assert_eq!(
        body["data"]["membersCount"], 1146,
        "the upstream's 1145 and the kept local member"
    );

    // 8. The fuse: a listing that lost most rows deactivates nothing.
    shared.lock().unwrap().rows.truncate(20);
    let out = run_pass(&client, &pool, &cfg, Duration::ZERO, &cancel)
        .await
        .unwrap();
    assert!(
        out.complete && out.sweep_refused && out.deactivated == 0,
        "{out:?}"
    );
    assert!(active(&pool, ids[100]).await);

    let _ = std::fs::remove_dir_all(&storage_dir);
}
