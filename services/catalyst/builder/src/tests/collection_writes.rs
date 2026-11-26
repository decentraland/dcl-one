//! Who may change a collection or an item, through the router as `serve`
//! mounts it, and what a write leaves when the answer is no.

use serde_json::json;
use uuid::Uuid;

use super::limits::{Call, node_over, send};
use super::support::address;
use super::support::{OWNER_KEY, collection_body};
use super::validator::own_pool;

async fn contract(pool: &sqlx::PgPool, id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT contract_address FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn put_collection(key: &str, id: Uuid, body: serde_json::Value) -> Call<'_> {
    Call {
        method: "PUT",
        key: Some(key),
        body: serde_json::to_vec(&body).unwrap(),
        ..Call::get(format!("/v1/collections/{id}"))
    }
}

#[tokio::test]
async fn a_node_without_a_polygon_rpc_cannot_tell_an_address_change_is_safe() {
    let Some(pool) = own_pool().await else {
        return;
    };
    let (app, _) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let owner = address(OWNER_KEY);
    let id = Uuid::new_v4();
    let named = |name: &str| {
        collection_body(
            json!({ "id": id, "name": name, "data": crate::collections::contract::initialize_calldata_named(&owner, name) }),
            &owner,
        )
    };
    let (status, body, _) = send(&app, &put_collection(OWNER_KEY, id, named("Hats"))).await;
    assert_eq!(status, 200, "{body}");
    let stored = contract(&pool, id).await;
    assert!(stored.is_some());
    let (status, body, _) = send(&app, &put_collection(OWNER_KEY, id, named("Caps"))).await;
    assert_eq!(status, 503, "{body}");
    assert_eq!(contract(&pool, id).await, stored);
}

fn put_item<'a>(key: &'a str, id: Uuid, body: &serde_json::Value) -> Call<'a> {
    Call {
        method: "PUT",
        key: Some(key),
        body: serde_json::to_vec(body).unwrap(),
        ..Call::get(format!("/v1/items/{id}"))
    }
}

fn delete_item(key: &str, id: Uuid) -> Call<'_> {
    Call {
        method: "DELETE",
        key: Some(key),
        ..Call::get(format!("/v1/items/{id}"))
    }
}

async fn waiting_on_a_lock(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() \
         AND wait_event_type IN ('Lock', 'Advisory') AND pid <> pg_backend_pid()",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn until_waiting(pool: &sqlx::PgPool, at_least: i64) {
    for _ in 0..200 {
        if waiting_on_a_lock(pool).await >= at_least {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("no request waited on a lock");
}

/// A save that names a file while the delete of the last item naming it
/// runs: whichever order they take, the save's name keeps the file's
/// upload record, so the collector never takes bytes an item names.
#[tokio::test]
async fn a_file_a_save_names_keeps_its_upload_record_through_a_concurrent_delete() {
    let Some(pool) = own_pool().await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let owner = address(OWNER_KEY);
    let (named, renamed) = (Uuid::new_v4(), Uuid::new_v4());
    let shared = super::support::contents_of(&[("model.glb", b"shared")]);
    let other = super::support::contents_of(&[("model.glb", b"other")]);
    let (shared_hash, other_hash) = (
        shared["model.glb"].as_str().unwrap().to_string(),
        other["model.glb"].as_str().unwrap().to_string(),
    );
    for (id, contents) in [(named, &shared), (renamed, &other)] {
        let body = super::support::item_body(
            &state.items,
            &owner,
            &json!({ "id": id, "name": "Hat", "contents": contents }),
        )
        .await;
        let (status, answer, _) = send(&app, &put_item(OWNER_KEY, id, &body)).await;
        assert_eq!(status, 200, "{answer}");
    }
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) SELECT h, 1, $2 FROM unnest($1::text[]) h",
    )
    .bind([&shared_hash, &other_hash])
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();
    let mut parked = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.uploaded_contents WHERE hash = $1 FOR UPDATE")
        .bind(&other_hash)
        .execute(&mut *parked)
        .await
        .unwrap();
    let to_shared = super::support::item_body(
        &state.items,
        &owner,
        &json!({ "id": renamed, "name": "Hat", "contents": shared }),
    )
    .await;
    let save = tokio::spawn({
        let app = app.clone();
        async move { send(&app, &put_item(OWNER_KEY, renamed, &to_shared)).await }
    });
    until_waiting(&pool, 1).await;
    let delete = tokio::spawn({
        let app = app.clone();
        async move { send(&app, &delete_item(OWNER_KEY, named)).await }
    });
    let _ = tokio::time::timeout(std::time::Duration::from_millis(500), async {
        while !delete.is_finished() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    parked.rollback().await.unwrap();
    let (saved, answer, _) = save.await.unwrap();
    assert_eq!(saved, 200, "{answer}");
    let (deleted, answer, _) = delete.await.unwrap();
    assert_eq!(deleted, 200, "{answer}");
    let kept: Vec<String> =
        sqlx::query_scalar("SELECT hash FROM builder.uploaded_contents ORDER BY hash")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(kept, [shared_hash]);
}

const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

async fn collection_named(pool: &sqlx::PgPool, id: Uuid) -> (String, String) {
    sqlx::query_as("SELECT name, eth_address FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn items_left(pool: &sqlx::PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM builder.items WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn unauthorized(body: &str, signer: &str, table: &str) -> bool {
    let answer: serde_json::Value = serde_json::from_str(body).unwrap();
    answer["error"]
        .as_str()
        .is_some_and(|e| e.starts_with(&format!("Unauthorized user {signer} for {table} ")))
}

fn refused_upsert(body: &str) -> bool {
    let answer: serde_json::Value = serde_json::from_str(body).unwrap();
    answer["error"] == "Unauthorized to upsert collection"
}

#[tokio::test]
async fn a_stranger_cannot_save_over_a_collection_someone_else_owns() {
    let Some(pool) = own_pool().await else {
        return;
    };
    let (app, _) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let (owner, stranger) = (address(OWNER_KEY), address(STRANGER_KEY));
    let id = Uuid::new_v4();
    let (status, body, _) = send(
        &app,
        &put_collection(
            OWNER_KEY,
            id,
            collection_body(json!({ "id": id, "name": "Hats" }), &owner),
        ),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body, _) = send(
        &app,
        &put_collection(
            STRANGER_KEY,
            id,
            collection_body(json!({ "id": id, "name": "Taken" }), &stranger),
        ),
    )
    .await;
    assert_eq!(status, 401, "{body}");
    assert!(refused_upsert(&body), "{body}");
    assert_eq!(collection_named(&pool, id).await, ("Hats".into(), owner));
}

#[tokio::test]
async fn only_the_owner_of_a_loose_item_or_of_its_collection_deletes_it() {
    let Some(pool) = own_pool().await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let (owner, stranger) = (address(OWNER_KEY), address(STRANGER_KEY));
    let collection = Uuid::new_v4();
    let (status, body, _) = send(
        &app,
        &put_collection(
            OWNER_KEY,
            collection,
            collection_body(json!({ "id": collection, "name": "Hats" }), &owner),
        ),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (loose, kept) = (Uuid::new_v4(), Uuid::new_v4());
    for (id, parent) in [(loose, None), (kept, Some(collection))] {
        let body = super::support::item_body(
            &state.items,
            &owner,
            &json!({ "id": id, "name": "Hat", "collection_id": parent, "contents": super::support::contents_of(&[("model.glb", b"model")]) }),
        )
        .await;
        let (status, answer, _) = send(&app, &put_item(OWNER_KEY, id, &body)).await;
        assert_eq!(status, 200, "{answer}");
    }
    for id in [loose, kept] {
        let (status, body, _) = send(&app, &delete_item(STRANGER_KEY, id)).await;
        assert_eq!(status, 401, "{body}");
        assert!(unauthorized(&body, &stranger, "items"), "{body}");
        assert_eq!(items_left(&pool, id).await, 1);
    }
    for id in [loose, kept] {
        let (status, body, _) = send(&app, &delete_item(OWNER_KEY, id)).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(items_left(&pool, id).await, 0);
    }
}

/// A collection that changes hands after the route admitted its owner's
/// request and before the write's transaction holds the row: the write
/// decides again inside its transaction and refuses the former owner.
#[tokio::test]
async fn a_write_refuses_a_caller_who_lost_the_collection_after_the_route_admitted_it() {
    let Some(pool) = own_pool().await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let (owner, stranger) = (address(OWNER_KEY), address(STRANGER_KEY));
    let (collection, item) = (Uuid::new_v4(), Uuid::new_v4());
    let (status, body, _) = send(
        &app,
        &put_collection(
            OWNER_KEY,
            collection,
            collection_body(json!({ "id": collection, "name": "Hats" }), &owner),
        ),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let body = super::support::item_body(
        &state.items,
        &owner,
        &json!({ "id": item, "name": "Hat", "collection_id": collection, "contents": super::support::contents_of(&[("model.glb", b"model")]) }),
    )
    .await;
    let (status, answer, _) = send(&app, &put_item(OWNER_KEY, item, &body)).await;
    assert_eq!(status, 200, "{answer}");

    let mut handover = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(collection)
        .execute(&mut *handover)
        .await
        .unwrap();
    let rename = tokio::spawn({
        let (app, owner) = (app.clone(), owner.clone());
        async move {
            send(
                &app,
                &put_collection(
                    OWNER_KEY,
                    collection,
                    collection_body(json!({ "id": collection, "name": "Renamed" }), &owner),
                ),
            )
            .await
        }
    });
    let delete = tokio::spawn({
        let app = app.clone();
        async move { send(&app, &delete_item(OWNER_KEY, item)).await }
    });
    until_waiting(&pool, 2).await;
    sqlx::query("UPDATE builder.collections SET eth_address = $2 WHERE id = $1")
        .bind(collection)
        .bind(&stranger)
        .execute(&mut *handover)
        .await
        .unwrap();
    handover.commit().await.unwrap();
    let (status, body, _) = rename.await.unwrap();
    assert_eq!(status, 401, "{body}");
    assert!(refused_upsert(&body), "{body}");
    let (status, body, _) = delete.await.unwrap();
    assert_eq!(status, 401, "{body}");
    assert!(unauthorized(&body, &owner, "items"), "{body}");
    assert_eq!(
        collection_named(&pool, collection).await,
        ("Hats".into(), stranger)
    );
    assert_eq!(items_left(&pool, item).await, 1);
}
