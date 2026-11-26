//! Who may change a collection or an item, through the router as `serve`
//! mounts it, and what a write leaves when the answer is no.

use serde_json::json;
use uuid::Uuid;

use super::limits::{Call, node_over, send};
use super::support::{OWNER_KEY, collection_body};
use super::validator::{address, own_pool};

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
