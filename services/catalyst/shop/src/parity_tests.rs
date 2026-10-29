//! Slice 5 (SHOP.md "Remaining parity"): the lists' management and sharing,
//! the campaigns, the listing-import check, the stores and the proceeds
//! routes over the process's scratch database (`db::scratch`, created on
//! the cluster `CATALYST_TEST_DATABASE_URL` names; without it every test
//! here skips), each seeding uuid-scoped rows and deleting them. The
//! mirrors' pass markers (`shop.mirror_state`) are shared rows: the tests
//! that need a ready mirror take `MIRROR_STATE_LOCK` while they run.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::market::tests::support::{
    MIRROR_STATE_LOCK, serve_empty_graph, state_with_db, state_without_db_settings,
};
use super::market::{
    MarketSettings, Settings, SharedState, listings, lists, market_router, mirrors,
    shop_admin_router,
};
use crate::shared::signed_fetch::test_support::{self as sf, signed_headers, signed_headers_by};

const WALLET_B_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn tag() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn contract_of(t: &str, n: u8) -> String {
    format!("0x{:0>38}{n:02}", &t[..30]).to_lowercase()
}

async fn call(
    router: Router,
    method: &str,
    uri: &str,
    headers: HeaderMap,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
    };
    (status, value)
}

async fn get(router: Router, uri: &str) -> (StatusCode, Value) {
    call(router, "GET", uri, HeaderMap::new(), None).await
}

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

async fn signed_b(method: &str, path: &str) -> HeaderMap {
    let wallet: alloy::signers::local::PrivateKeySigner = WALLET_B_KEY.parse().unwrap();
    signed_headers_by(
        &wallet,
        method,
        path,
        "{}",
        true,
        &chrono::Utc::now().timestamp_millis().to_string(),
    )
    .await
}

fn wallet_b_address() -> String {
    let wallet: alloy::signers::local::PrivateKeySigner = WALLET_B_KEY.parse().unwrap();
    format!("{:#x}", wallet.address())
}

/// Every configured network (matic only here) with a complete pass of both mirrors.
async fn mark_mirrors_ready(pool: &PgPool) {
    for mirror in ["collections", "items"] {
        sqlx::query(
            "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, last_complete_pass_rows) VALUES ($1, 'matic', 'http://test/', timezone('UTC', now()), 1) \
             ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = timezone('UTC', now())",
        )
        .bind(mirror)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn matic_state(admin_token: Option<&str>) -> Option<SharedState> {
    let url = serve_empty_graph().await;
    let mut settings = Settings {
        matic_collections_subgraph_url: Some(url),
        data_writes_enabled: true,
        ..Settings::for_tests()
    };
    settings.market = MarketSettings {
        admin_token: admin_token.map(str::to_string),
        ..MarketSettings::default()
    };
    let state = state_with_db(settings).await?;
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Some(state)
}

async fn seed_item(pool: &PgPool, contract: &str, item_id: &str, creator: &str) {
    let mut collection = mirrors::collections::fixture(contract, "Seeded", creator, true);
    collection["minters"] = json!([creator]);
    mirrors::collections::upsert(
        pool,
        super::market::graph::Network::Matic,
        137,
        &[collection],
    )
    .await
    .unwrap();
    let mut item = mirrors::items::fixture(contract, item_id, "wearable_v2", "Seeded item");
    item["creator"] = json!(creator);
    mirrors::items::upsert(pool, super::market::graph::Network::Matic, 137, &[item])
        .await
        .unwrap();
}

async fn cleanup_mirror(pool: &PgPool, contracts: &[String]) {
    sqlx::query("DELETE FROM shop.items WHERE contract_address = ANY($1)")
        .bind(contracts)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.collections WHERE contract_address = ANY($1)")
        .bind(contracts)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn lists_are_created_shared_edited_and_deleted_by_their_owner_only() {
    let Some(state) = matic_state(None).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = contract_of(&t, 1);
    let item = format!("{contract}-0");
    let owner = sf::address().to_lowercase();
    let other = wallet_b_address().to_lowercase();
    seed_item(
        &pool,
        &contract,
        "0",
        "0xcccccccccccccccccccccccccccccccccccccccc",
    )
    .await;
    let app = || market_router(state.clone());
    let name = format!("Fits {}", &t[..6]);

    // Create: 201 with the list; the same name again is upstream's 422.
    let (status, body) = call(
        app(),
        "POST",
        "/v1/lists",
        signed_headers("post", "/v1/lists", "{}", true).await,
        Some(json!({ "name": name, "private": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let private = body["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        (
            body["data"]["isPrivate"].as_bool(),
            body["data"]["name"].as_str(),
            body["data"]["userAddress"].as_str()
        ),
        (Some(true), Some(name.as_str()), Some(owner.as_str()))
    );
    assert!(
        body["data"].get("permission").is_none() && body["data"].get("itemsCount").is_none(),
        "the write answers the row alone: {body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        "/v1/lists",
        signed_headers("post", "/v1/lists", "{}", true).await,
        Some(json!({ "name": name, "private": true })),
    )
    .await;
    assert_eq!(
        (status, body["data"]["name"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some(name.as_str())),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        "/v1/lists",
        signed_headers("post", "/v1/lists", "{}", true).await,
        Some(json!({ "name": "x".repeat(33), "private": true })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = call(
        app(),
        "POST",
        "/v1/lists",
        HeaderMap::new(),
        Some(json!({ "name": "anon", "private": true })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, body) = call(
        app(),
        "POST",
        "/v1/lists",
        signed_headers("post", "/v1/lists", "{}", true).await,
        Some(json!({ "name": format!("Public {}", &t[..6]), "private": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let public = body["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["data"]["isPrivate"], false);

    // Privacy: a private list is a 404 to anyone but the owner; a public one reads for everyone.
    let (status, _) = get(app(), &format!("/v1/lists/{private}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(
        app(),
        "GET",
        &format!("/v1/lists/{private}"),
        signed_b("get", &format!("/v1/lists/{private}")).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another wallet: {body}");
    let (status, body) = call(
        app(),
        "GET",
        &format!("/v1/lists/{private}"),
        signed_headers("get", &format!("/v1/lists/{private}"), "{}", true).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["itemsCount"].as_i64(),
            body["data"]["isPrivate"].as_bool(),
            body["data"]["previewOfItemIds"].as_array().map(Vec::len)
        ),
        (Some(0), Some(true), Some(0))
    );
    let (status, body) = get(app(), &format!("/v1/lists/{public}")).await;
    assert_eq!(
        (
            status,
            body["data"]["isPrivate"].as_bool(),
            body["data"]["permission"].as_str()
        ),
        (StatusCode::OK, Some(false), Some("view")),
        "{body}"
    );

    // Sharing: a view grant reads, an edit grant picks; the owner alone grants.
    let access = format!("/v1/lists/{private}/access");
    let (status, body) = call(
        app(),
        "POST",
        &access,
        signed_b("post", &access).await,
        Some(json!({ "permission": "view", "grantee": other })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a non-owner cannot share it: {body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        &access,
        signed_headers("post", &access, "{}", true).await,
        Some(json!({ "permission": "view", "grantee": other })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = call(
        app(),
        "GET",
        &format!("/v1/lists/{private}"),
        signed_b("get", &format!("/v1/lists/{private}")).await,
        None,
    )
    .await;
    assert_eq!(
        (
            status,
            body["data"]["permission"].as_str(),
            body["data"]["isPrivate"].as_bool()
        ),
        (StatusCode::OK, Some("view"), Some(false)),
        "{body}"
    );
    let picks = format!("/v1/lists/{private}/picks");
    let (status, body) = call(
        app(),
        "POST",
        &picks,
        signed_b("post", &picks).await,
        Some(json!({ "itemId": item })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "view does not pick: {body}");
    let (status, _) = call(
        app(),
        "POST",
        &access,
        signed_headers("post", &access, "{}", true).await,
        Some(json!({ "permission": "edit", "grantee": other })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = call(
        app(),
        "POST",
        &picks,
        signed_b("post", &picks).await,
        Some(json!({ "itemId": item })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        (
            body["data"]["itemId"].as_str(),
            body["data"]["userAddress"].as_str(),
            body["data"]["listId"].as_str()
        ),
        (
            Some(item.as_str()),
            Some(other.as_str()),
            Some(private.as_str())
        )
    );
    let (status, body) = call(
        app(),
        "POST",
        &picks,
        signed_b("post", &picks).await,
        Some(json!({ "itemId": item })),
    )
    .await;
    assert_eq!(
        (status, body["data"]["itemId"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some(item.as_str())),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        &picks,
        signed_b("post", &picks).await,
        Some(json!({ "itemId": format!("{contract}-99") })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an item the mirror lacks: {body}"
    );
    assert_eq!(body["message"], lists::ITEM_NOT_FOUND_MESSAGE);

    // Who picked, and the item's stats: the signer first, pickedByUser only when signed.
    let (status, body) = call(
        app(),
        "GET",
        &format!("/v1/picks/{item}"),
        signed_b("get", &format!("/v1/picks/{item}")).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["results"][0]["userAddress"].as_str(),
            body["data"]["total"].as_i64(),
            body["data"]["pages"].as_i64()
        ),
        (Some(other.as_str()), Some(1), Some(1))
    );
    let (status, body) = get(app(), &format!("/v1/picks/{item}/stats")).await;
    assert_eq!(
        (
            status,
            body["data"]["count"].as_i64(),
            body["data"].get("pickedByUser")
        ),
        (StatusCode::OK, Some(1), None),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "GET",
        &format!("/v1/picks/{item}/stats"),
        signed_b("get", &format!("/v1/picks/{item}/stats")).await,
        None,
    )
    .await;
    assert_eq!(
        (status, body["data"]["pickedByUser"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{body}"
    );
    let (status, _) = get(app(), &format!("/v1/picks/{item}/stats?power=abc")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Revoking edit takes the pick right away; the pick itself is removed by its picker.
    let (status, body) = call(
        app(),
        "DELETE",
        &access,
        signed_headers("delete", &access, "{}", true).await,
        Some(json!({ "permission": "edit", "grantee": other })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = call(
        app(),
        "DELETE",
        &access,
        signed_headers("delete", &access, "{}", true).await,
        Some(json!({ "permission": "edit", "grantee": other })),
    )
    .await;
    assert_eq!(
        (status, body["data"]["grantee"].as_str()),
        (StatusCode::NOT_FOUND, Some(other.as_str())),
        "{body}"
    );
    let (status, _) = call(
        app(),
        "POST",
        &picks,
        signed_b("post", &picks).await,
        Some(json!({ "itemId": format!("{contract}-0") })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let pick_path = format!("/v1/lists/{private}/picks/{item}");
    let (status, body) = call(
        app(),
        "DELETE",
        &pick_path,
        signed_headers("delete", &pick_path, "{}", true).await,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the owner did not make that pick: {body}"
    );
    let (status, body) = call(
        app(),
        "DELETE",
        &pick_path,
        signed_b("delete", &pick_path).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = call(
        app(),
        "DELETE",
        &pick_path,
        signed_b("delete", &pick_path).await,
        None,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::NOT_FOUND, Some(lists::PICK_NOT_FOUND_MESSAGE))
    );

    // Update: the owner's fields and privacy; never the default list; never someone else's.
    let list_path = format!("/v1/lists/{private}");
    let new_name = format!("Fits two {}", &t[..6]);
    let (status, body) = call(
        app(),
        "PUT",
        &list_path,
        signed_headers("put", &list_path, "{}", true).await,
        Some(json!({ "name": new_name, "private": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["name"].as_str(),
            body["data"]["isPrivate"].as_bool()
        ),
        (Some(new_name.as_str()), Some(false))
    );
    assert!(body["data"]["updatedAt"].as_i64().unwrap() > 0);
    let (status, body) = call(
        app(),
        "PUT",
        &list_path,
        signed_b("put", &list_path).await,
        Some(json!({ "name": "mine now" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let default_path = format!("/v1/lists/{}", lists::DEFAULT_LIST_ID);
    let (status, body) = call(
        app(),
        "PUT",
        &default_path,
        signed_headers("put", &default_path, "{}", true).await,
        Some(json!({ "private": true })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some(lists::DEFAULT_LIST_IMMUTABLE_MESSAGE)
        )
    );
    let (status, _) = call(
        app(),
        "PUT",
        &list_path,
        signed_headers("put", &list_path, "{}", true).await,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = get(app(), &list_path).await;
    assert_eq!(
        (status, body["data"]["isPrivate"].as_bool()),
        (StatusCode::OK, Some(false)),
        "public now: {body}"
    );
    let (status, body) = call(
        app(),
        "PUT",
        &list_path,
        signed_headers("put", &list_path, "{}", true).await,
        Some(json!({ "private": true })),
    )
    .await;
    // Upstream's getListQuery counts every ACL row: the named view grant
    // still standing keeps the list shared, only the public read goes.
    assert_eq!(
        (status, body["data"]["isPrivate"].as_bool()),
        (StatusCode::OK, Some(false)),
        "{body}"
    );
    let (status, _) = get(app(), &list_path).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "private again");

    // Delete: the owner's alone; the default list never.
    let (status, _) = call(
        app(),
        "DELETE",
        &list_path,
        signed_b("delete", &list_path).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        app(),
        "DELETE",
        &default_path,
        signed_headers("delete", &default_path, "{}", true).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(
        app(),
        "DELETE",
        &list_path,
        signed_headers("delete", &list_path, "{}", true).await,
        None,
    )
    .await;
    assert_eq!(
        (status, body["ok"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{body}"
    );
    let (status, _) = call(
        app(),
        "GET",
        &list_path,
        signed_headers("get", &list_path, "{}", true).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let acl_left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.list_acl WHERE list_id = $1::uuid")
            .bind(&private)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(acl_left, 0, "grants go with the list");

    // Writes off: every mutation is the switch's 503 before any verification.
    let gated = state_without_db_settings(Settings::for_tests());
    for (method, path) in [
        ("POST", "/v1/lists".to_string()),
        ("PUT", list_path.clone()),
        ("DELETE", list_path.clone()),
        ("POST", picks.clone()),
        ("POST", access.clone()),
    ] {
        let (status, body) = call(
            market_router(gated.clone()),
            method,
            &path,
            HeaderMap::new(),
            Some(json!({})),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {body}"
        );
    }

    sqlx::query("DELETE FROM shop.lists WHERE user_address = $1 AND id::text = ANY($2)")
        .bind(&owner)
        .bind(vec![private, public])
        .execute(&pool)
        .await
        .unwrap();
    cleanup_mirror(&pool, &[contract]).await;
}

#[tokio::test]
async fn campaigns_are_operator_data_read_publicly_with_the_builder_shape() {
    let Some(state) = matic_state(Some("campaign-token")).await else {
        return;
    };
    let pool = state.pool.clone();
    let _rows = MIRROR_STATE_LOCK.lock().await;
    mark_mirrors_ready(&pool).await;
    let t = tag();
    let id = format!("season-{}", &t[..8]);
    let approved = contract_of(&t, 2);
    let unapproved = contract_of(&t, 3);
    seed_item(
        &pool,
        &approved,
        "0",
        "0xcccccccccccccccccccccccccccccccccccccccc",
    )
    .await;
    mirrors::collections::upsert(
        &pool,
        super::market::graph::Network::Matic,
        137,
        &[mirrors::collections::fixture(
            &unapproved,
            "Pending",
            "0xcccccccccccccccccccccccccccccccccccccccc",
            false,
        )],
    )
    .await
    .unwrap();
    let app = || market_router(state.clone());
    let admin = || shop_admin_router(state.clone());
    let path = format!("/v1/admin/campaigns/{id}");
    let body = json!({ "name": "Season", "mainTag": format!("tag-{}", &t[..8]), "tags": [format!("tag-{}", &t[..8]), "extra"], "enabled": true, "banner": { "desktopTitle": "Season" } });

    let (status, out) = call(app(), "PUT", &path, HeaderMap::new(), Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no bearer: {out}");
    let (status, out) = call(app(), "PUT", &path, bearer("wrong"), Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{out}");
    let (status, out) = call(
        app(),
        "PUT",
        &path,
        bearer("campaign-token"),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(
        (
            out["data"]["id"].as_str(),
            out["data"]["active"].as_bool(),
            out["data"]["collections"].as_array().map(Vec::len)
        ),
        (Some(id.as_str()), Some(true), Some(0))
    );
    let (status, out) = call(
        admin(),
        "PUT",
        &format!("/admin/campaigns/{id}"),
        bearer("campaign-token"),
        Some(
            json!({ "name": "Season two", "mainTag": format!("tag-{}", &t[..8]), "enabled": true }),
        ),
    )
    .await;
    assert_eq!(
        (status, out["data"]["name"].as_str()),
        (StatusCode::OK, Some("Season two")),
        "the canonical admin path is the same handler: {out}"
    );
    let (status, out) = call(
        app(),
        "PUT",
        "/v1/admin/campaigns/Bad%20Id",
        bearer("campaign-token"),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");

    let (status, out) = get(app(), &format!("/v1/campaigns/{id}")).await;
    assert_eq!(
        (status, out["data"]["enabled"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{out}"
    );
    let (status, out) = get(app(), "/v1/campaigns/active").await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(
        out["data"]["id"], id,
        "the newest enabled campaign in its window"
    );

    // Collections: only approved collections of the mirror; the Builder shape reads them back.
    let coll_path = format!("/v1/admin/campaigns/{id}/collections");
    let (status, out) = call(
        app(),
        "PUT",
        &coll_path,
        bearer("campaign-token"),
        Some(json!({ "contracts": [approved, unapproved] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an unapproved collection is refused by name: {out}"
    );
    assert!(out["message"].as_str().unwrap().contains(&unapproved));
    let (status, out) = call(
        app(),
        "PUT",
        &coll_path,
        bearer("campaign-token"),
        Some(json!({ "contracts": [approved.to_uppercase()] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"][0]["contractAddress"], approved);
    let (status, out) = get(
        app(),
        &format!("/v1/campaign-collections?tag=tag-{}&tag=nothing", &t[..8]),
    )
    .await;
    assert_eq!(
        (status, out["ok"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{out}"
    );
    assert_eq!(out["data"], json!([approved]));
    let (status, out) = get(app(), "/v1/campaign-collections?tag=nobody-set-this").await;
    assert_eq!(
        (status, out["data"].as_array().map(Vec::len)),
        (StatusCode::OK, Some(0)),
        "an unknown tag is an empty set, never null: {out}"
    );
    let (status, _) = get(app(), "/v1/campaign-collections").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let alias = super::campaigns::builder_alias_router(state.clone());
    let (status, out) = get(
        alias,
        &format!("/builder-api/v1/addresses?tag=tag-{}", &t[..8]),
    )
    .await;
    assert_eq!(
        (status, out["data"][0].as_str()),
        (StatusCode::OK, Some(approved.as_str())),
        "{out}"
    );
    let (status, out) = get(app(), &format!("/v1/campaigns/{id}")).await;
    assert_eq!(
        (
            status,
            out["data"]["collections"][0]["contractAddress"].as_str()
        ),
        (StatusCode::OK, Some(approved.as_str())),
        "{out}"
    );

    // A window in the past is not active; delete ends it.
    let (status, out) = call(app(), "PUT", &path, bearer("campaign-token"), Some(json!({ "name": "Over", "mainTag": format!("tag-{}", &t[..8]), "enabled": true, "startsAt": 1_000, "endsAt": 2_000 }))).await;
    assert_eq!(
        (status, out["data"]["active"].as_bool()),
        (StatusCode::OK, Some(false)),
        "{out}"
    );
    let (status, out) = call(app(), "DELETE", &path, bearer("campaign-token"), None).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    let (status, _) = get(app(), &format!("/v1/campaigns/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let gated = state_without_db_settings(Settings::for_tests());
    let (status, _) = call(
        market_router(gated),
        "PUT",
        &path,
        bearer("campaign-token"),
        Some(body),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "the write switch is in front of the bearer"
    );

    sqlx::query("DELETE FROM shop.campaign_collections WHERE tag = $1")
        .bind(format!("tag-{}", &t[..8]))
        .execute(&pool)
        .await
        .unwrap();
    cleanup_mirror(&pool, &[approved, unapproved]).await;
}

fn item_order_json(signer: &str, contract: &str, item_id: &str) -> String {
    format!(
        r#"{{ "signer": "{signer}", "signature": "0x00", "type": "public_item_order", "network": "MATIC", "chainId": 137,
             "checks": {{ "uses": 1, "expiration": 4102444800000, "effective": 0, "salt": "0x1234", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
             "sent": [ {{"assetType": 4, "contractAddress": "{contract}", "itemId": "{item_id}", "extra": "0x"}} ],
             "received": [ {{"assetType": 1, "contractAddress": "0x2222222222222222222222222222222222222222", "amount": "5000000000000000000", "extra": "0x", "beneficiary": "{signer}"}} ] }}"#
    )
}

fn token_order_json(signer: &str, contract: &str, token_id: &str) -> String {
    format!(
        r#"{{ "signer": "{signer}", "signature": "0x00", "type": "public_nft_order", "network": "MATIC", "chainId": 137,
             "checks": {{ "uses": 1, "expiration": 4102444800000, "effective": 0, "salt": "0x1234", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
             "sent": [ {{"assetType": 3, "contractAddress": "{contract}", "tokenId": "{token_id}", "extra": "0x"}} ],
             "received": [ {{"assetType": 2, "contractAddress": "0x2222222222222222222222222222222222222222", "amount": "2500000000000000000", "extra": "0x", "beneficiary": "{signer}"}} ] }}"#
    )
}

#[tokio::test]
async fn the_import_check_admits_a_creators_item_order_and_keeps_a_token_in_mana() {
    use alloy::signers::SignerSync;
    let Some(state) = matic_state(None).await else {
        return;
    };
    let pool = state.pool.clone();
    let _rows = MIRROR_STATE_LOCK.lock().await;
    let t = tag();
    let contract = contract_of(&t, 4);
    let creator = sf::address().to_lowercase();
    let path = super::import::CHECK_PATH;
    let app = || market_router(state.clone());

    // Before the mirrors have a pass the answer is the envelope, not a refusal.
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror IN ('collections', 'items') AND network = 'matic'").execute(&pool).await.unwrap();
    state.mirror_ready.clear();
    let (status, body) = call(
        app(),
        "POST",
        path,
        signed_headers("post", path, "{}", true).await,
        Some(json!({ "chainId": 137, "contractAddress": contract, "itemId": "0" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-mirror");
    mark_mirrors_ready(&pool).await;
    state.mirror_ready.clear();
    seed_item(&pool, &contract, "0", &creator).await;
    sqlx::query("INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) VALUES ('coingecko', 0.25, timezone('UTC', now()) + interval '1 day')").execute(&pool).await.unwrap();

    // By the asset: the creator's item is credit-eligible, quoted from the rate.
    let (status, body) = call(
        app(),
        "POST",
        path,
        signed_headers("post", path, "{}", true).await,
        Some(json!({ "chainId": 137, "contractAddress": contract, "itemId": "0" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let d = &body["data"];
    assert_eq!(
        (
            d["eligible"].as_bool(),
            d["saleKind"].as_str(),
            d["settlement"].as_str(),
            d["sellerIsCreator"].as_bool()
        ),
        (Some(true), Some("primaryMint"), Some("credits"), Some(true)),
        "{d}"
    );
    assert_eq!(d["quote"]["rateUsdPerMana"], "0.25");
    assert!(
        d["quote"]["manaWei"].is_null(),
        "no old listing, no old price"
    );

    // By the old trade: a MANA item order imports at ceil(5 MANA * 0.25 USD = 125 cents) = 13 credits.
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let mut trade = listings::test_support::parse(&item_order_json(&sf::address(), &contract, "0"));
    let hash = listings::signing_hash(&trade, &marketplace).unwrap();
    trade.signature = format!(
        "0x{}",
        hex::encode(sf::wallet().sign_hash_sync(&hash).unwrap().as_bytes())
    );
    let old_id = listings::create_trade(&pool, None, &trade, &creator, 1_000)
        .await
        .unwrap();
    let (status, body) = call(
        app(),
        "POST",
        path,
        signed_headers("post", path, "{}", true).await,
        Some(json!({ "oldTradeId": old_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let d = &body["data"];
    assert_eq!(
        (
            d["eligible"].as_bool(),
            d["quote"]["manaWei"].as_str(),
            d["quote"]["usdCents"].as_i64(),
            d["quote"]["credits"].as_i64()
        ),
        (Some(true), Some("5000000000000000000"), Some(125), Some(13)),
        "{d}"
    );
    assert_eq!(d["asset"]["itemId"], "0");

    // Another wallet's old listing is not the signer's to import; an unknown id is a 404.
    let (status, body) = call(
        app(),
        "POST",
        path,
        signed_b("post", path).await,
        Some(json!({ "oldTradeId": old_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _) = call(
        app(),
        "POST",
        path,
        signed_headers("post", path, "{}", true).await,
        Some(json!({ "oldTradeId": uuid::Uuid::new_v4().to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        app(),
        "POST",
        path,
        HeaderMap::new(),
        Some(json!({ "oldTradeId": old_id })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A token the creator owns (their own mint, already issued) is a resale: MANA only, at a current quote for its USD-pegged price.
    let token_id = "105312291668557186697918027683670432318895095400549111254310977537";
    let mut token_trade =
        listings::test_support::parse(&token_order_json(&sf::address(), &contract, token_id));
    let hash = listings::signing_hash(&token_trade, &marketplace).unwrap();
    token_trade.signature = format!(
        "0x{}",
        hex::encode(sf::wallet().sign_hash_sync(&hash).unwrap().as_bytes())
    );
    let token_old = listings::insert_trade(&pool, &token_trade, "MATIC", marketplace.address)
        .await
        .unwrap();
    let (status, body) = call(
        app(),
        "POST",
        path,
        signed_headers("post", path, "{}", true).await,
        Some(json!({ "oldTradeId": token_old })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let d = &body["data"];
    assert_eq!(
        (
            d["eligible"].as_bool(),
            d["saleKind"].as_str(),
            d["settlement"].as_str(),
            d["refusal"]["code"].as_str()
        ),
        (
            Some(false),
            Some("secondary"),
            Some("mana"),
            Some("creditsNotAllowed")
        ),
        "{d}"
    );
    assert!(
        d["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "secondarySale")
    );
    assert_eq!(
        (
            d["quote"]["credits"].as_i64(),
            d["quote"]["settlesNowWei"].as_str()
        ),
        (None, Some("10000000000000000000")),
        "2.5 USD pegged settles for 10 MANA at 0.25: {d}"
    );

    // Someone else's item, by the asset: not the creator, nothing to import.
    let (status, body) = call(
        app(),
        "POST",
        path,
        signed_b("post", path).await,
        Some(json!({ "chainId": 137, "contractAddress": contract, "itemId": "0" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["eligible"].as_bool(),
            body["data"]["refusal"]["code"].as_str(),
            body["data"]["settlement"].is_null()
        ),
        (Some(false), Some("notTheCreator"), true),
        "{body}"
    );

    for id in [old_id, token_old] {
        sqlx::query("DELETE FROM shop.trades WHERE id = $1::uuid")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM economy.price_snapshots WHERE mana_usd = 0.25 AND taken_at > timezone('UTC', now())").execute(&pool).await.unwrap();
    cleanup_mirror(&pool, &[contract]).await;
}

#[tokio::test]
async fn a_store_without_an_entity_is_empty_and_its_stats_read_the_mirror() {
    let Some(state) = matic_state(None).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = contract_of(&t, 5);
    let owner = format!("0x{:0>40}", &t[..12]).to_lowercase();
    seed_item(&pool, &contract, "0", &owner).await;
    let app = || market_router(state.clone());

    let (status, body) = get(app(), &format!("/v1/stores/{}", owner.to_uppercase())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["exists"].as_bool(),
            body["data"]["owner"].as_str(),
            body["data"]["cover"].as_str()
        ),
        (Some(false), Some(owner.as_str()), Some(""))
    );
    assert_eq!(body["data"]["urn"], super::stores::store_urn(&owner));
    let (status, _) = get(app(), "/v1/stores/not-an-address").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The stats need both collections readers; one network alone is the envelope naming the other.
    let (status, body) = get(app(), &format!("/v1/stores/{owner}/stats")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-subgraph:ethereum");
    let both = super::market::tests::support::state_with_empty_readers(Some(pool.clone())).await;
    let (status, body) = get(
        market_router(both.clone()),
        &format!("/v1/stores/{owner}/stats?from=1&to=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let d = &body["data"];
    assert_eq!(
        (
            d["currency"].as_str(),
            d["summary"]["total"].as_i64(),
            d["collections"][0]["contractAddress"].as_str()
        ),
        (Some("MANA"), Some(0), Some(contract.as_str())),
        "{d}"
    );
    assert_eq!(
        (
            d["collections"][0]["items"][0]["itemId"].as_str(),
            d["collections"][0]["items"][0]["sold"].as_i64(),
            d["collections"][0]["items"][0]["minted"].as_str()
        ),
        (Some("0"), Some(0), Some("10"))
    );
    let (status, _) = get(
        market_router(both.clone()),
        &format!("/v1/stores/{owner}/stats?from=5&to=1"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Proceeds over readers with nothing: empty rows, zero totals, MANA settlement stated.
    let (status, body) = get(
        market_router(both.clone()),
        &format!("/v1/proceeds?seller={owner}"),
    )
    .await;
    assert_eq!(
        (
            status,
            body["total"].as_i64(),
            body["data"].as_array().map(Vec::len)
        ),
        (StatusCode::OK, Some(0), Some(0)),
        "{body}"
    );
    let (status, body) = get(
        market_router(both),
        &format!("/v1/proceeds/summary?seller={owner}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["netWei"].as_str(),
            body["data"]["settlement"].as_str(),
            body["data"]["treasuryRouting"].as_bool()
        ),
        (Some("0"), Some("mana"), Some(false))
    );
    let (status, _) = get(app(), "/v1/proceeds").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a seller is required");

    cleanup_mirror(&pool, &[contract]).await;
}
