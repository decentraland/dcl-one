use super::*;
use crate::tests::support::{OWNER_KEY, cleanup, scratch_component, signed_map};
use axum::{Json, Router, extract::State, response::IntoResponse, routing::get};
use serde_json::json;
use std::sync::{Arc, Mutex};

const OWNER: &str = "0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a";
const URN: &str = "urn:decentraland:matic:collections-thirdparty:provider:hats";

fn fixture(id: &str, item: &str) -> (Value, Value, Value, Value) {
    let collection =
        json!({"id":id,"name":"Hats","eth_address":OWNER,"urn_suffix":URN,"contract_address":null});
    let item_doc = json!({"id":item,"name":"Hat","type":"wearable","description":"","eth_address":OWNER,"collection_id":id,
        "thumbnail":"thumbnail.png","contents":{"hat.glb":"model-cid","thumbnail.png":"image-cid"},
        "data":{"category":"hat"},"metrics":{"triangles":10}});
    let snapshot = json!({"collection":collection,"items":[item_doc]});
    let mut remote_collection = collection;
    remote_collection["urn"] = json!(URN);
    remote_collection["is_published"] = json!(true);
    remote_collection["forum_link"] = json!("https://forum.decentraland.org/t/hats/123");
    let mut remote_item = item_doc;
    remote_item["urn"] = json!(format!("{URN}:{item}"));
    remote_item["is_published"] = json!(true);
    remote_item["local_content_hash"] = json!("entity-hash");
    remote_item["is_approved"] = json!(false);
    (
        snapshot,
        remote_collection,
        json!([remote_item]),
        json!([{"item_id":item,"content_hash":"entity-hash","status":"pending"}]),
    )
}

const ID: &str = "11111111-1111-4111-8111-111111111111";
const ITEM: &str = "22222222-2222-4222-8222-222222222222";

/// The old fixtures minted the pre-6.0.0 folded payload; the shared verifier
/// accepts it behind the canonical-`signer` guard.
async fn signed(path: &str) -> BTreeMap<String, String> {
    signed_map(OWNER_KEY, "get", path, "{}", true).await
}

#[test]
fn only_matching_authoritative_content_curations_prove_submission() {
    let (snapshot, collection, items, curations) = fixture(ID, ITEM);
    let review = verify_review(&snapshot, &collection, &items, &curations)
        .unwrap()
        .unwrap();
    assert_eq!(review.items[0].content_hash, "entity-hash");
    assert_eq!(review.items[0].status, "pending");
    assert!(!review.items[0].approved);
    assert_eq!(
        review.forum_url.as_deref(),
        Some("https://forum.decentraland.org/t/hats/123")
    );
    for status in ["approved", "rejected"] {
        let mut changed = curations.clone();
        changed[0]["status"] = json!(status);
        assert_eq!(
            verify_review(&snapshot, &collection, &items, &changed)
                .unwrap()
                .unwrap()
                .items[0]
                .status,
            status
        );
    }
    let mut draft_collection = collection.clone();
    draft_collection["is_published"] = json!(false);
    let mut draft_items = items.clone();
    draft_items[0]["is_published"] = json!(false);
    assert!(
        verify_review(&snapshot, &draft_collection, &draft_items, &json!([]))
            .unwrap()
            .is_none()
    );
    assert!(verify_review(&snapshot, &collection, &items, &json!([])).is_err());
}

#[test]
fn rejects_mismatched_collections_files_metadata_review_hashes_and_duplicate_rows() {
    let (snapshot, collection, items, curations) = fixture(ID, ITEM);
    for (field, value) in [
        ("id", json!(ITEM)),
        ("urn", json!("other")),
        ("name", json!("other")),
        (
            "eth_address",
            json!("0x2222222222222222222222222222222222222222"),
        ),
        ("is_published", json!(false)),
        ("contract_address", json!(OWNER)),
        ("forum_link", json!("https://evil.test/t/review/1")),
    ] {
        let mut changed = collection.clone();
        changed[field] = value;
        assert!(
            verify_review(&snapshot, &changed, &items, &curations).is_err(),
            "accepted collection {field}"
        );
    }
    for (field, value) in [
        ("urn", json!("other")),
        ("collection_id", json!(ITEM)),
        ("eth_address", json!("another owner")),
        ("contents", json!({"hat.glb":"changed"})),
        ("data", json!({"category":"hair"})),
        ("metrics", json!({"triangles":999})),
        ("local_content_hash", json!("other hash")),
        ("is_published", json!(false)),
    ] {
        let mut changed = items.clone();
        changed[0][field] = value;
        assert!(
            verify_review(&snapshot, &collection, &changed, &curations).is_err(),
            "accepted item {field}"
        );
    }
    for (field, value) in [
        ("item_id", json!(ID)),
        ("content_hash", json!("other hash")),
        ("status", json!("unknown")),
    ] {
        let mut changed = curations.clone();
        changed[0][field] = value;
        assert!(
            verify_review(&snapshot, &collection, &items, &changed).is_err(),
            "accepted curation {field}"
        );
    }
    assert!(
        verify_review(
            &snapshot,
            &collection,
            &json!([items[0], items[0]]),
            &curations
        )
        .is_err()
    );
    assert!(
        verify_review(
            &snapshot,
            &collection,
            &items,
            &json!([curations[0], curations[0]])
        )
        .is_err()
    );
}

#[tokio::test]
async fn signatures_bind_the_exact_read_path_and_owner_and_cannot_forward_other_headers() {
    let path = format!("/v1/collections/{ID}");
    let valid = signed(&path).await;
    signed_headers(OWNER, &path, &valid).await.unwrap();
    assert!(
        signed_headers(OWNER, &format!("{path}/items"), &valid)
            .await
            .is_err()
    );
    assert!(
        signed_headers("0x2222222222222222222222222222222222222222", &path, &valid)
            .await
            .is_err()
    );
    for key in ["cookie", "authorization", "x-original-path", "host"] {
        let mut invalid = valid.clone();
        invalid.insert(key.into(), "not-forwarded".into());
        assert!(signed_headers(OWNER, &path, &invalid).await.is_err());
    }
    let mut invalid = valid;
    invalid.insert("x-identity-timestamp".into(), "1".into());
    assert!(signed_headers(OWNER, &path, &invalid).await.is_err());
}

#[derive(Clone)]
struct Mock {
    responses: Arc<Mutex<BTreeMap<String, Value>>>,
    redirect: Arc<Mutex<bool>>,
}
async fn reply(State(mock): State<Mock>, uri: axum::http::Uri) -> axum::response::Response {
    if *mock.redirect.lock().unwrap() {
        return (
            axum::http::StatusCode::TEMPORARY_REDIRECT,
            [("location", "https://example.invalid/")],
        )
            .into_response();
    }
    match mock.responses.lock().unwrap().get(uri.path()).cloned() {
        Some(data) => Json(json!({"data":data})).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error":"missing"})),
        )
            .into_response(),
    }
}

#[tokio::test]
async fn verifies_over_http_before_marking_the_frozen_collection_published_and_recovers() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let id = Uuid::new_v4();
    let item = Uuid::new_v4();
    let (snapshot, collection, items, curations) = fixture(&id.to_string(), &item.to_string());
    sqlx::query("INSERT INTO builder.collections(id,name,eth_address,urn_suffix,third_party_id,publication_pending) VALUES ($1,'Hats',$2,$3,'urn:decentraland:matic:collections-thirdparty:provider',true)")
        .bind(id).bind(OWNER).bind(URN).execute(store.pool()).await.unwrap();
    sqlx::query("INSERT INTO builder.items(id,name,eth_address,collection_id,type) VALUES ($1,'Hat',$2,$3,'wearable')")
        .bind(item).bind(OWNER).bind(id).execute(store.pool()).await.unwrap();
    let preparation = json!({"id":id,"revision":"reviewed","name":"Hats","creator":OWNER,"urn":URN,"third_party_id":"urn:decentraland:matic:collections-thirdparty:provider","item_ids":[item]});
    sqlx::query("INSERT INTO builder.linked_collection_publications(collection_id,preparation,snapshot,salt,cheque,status) VALUES ($1,$2,$3,$4,$5,'authorized')")
        .bind(id).bind(preparation).bind(snapshot).bind(format!("0x{}", hex::encode(Uuid::new_v4().as_bytes()).repeat(2)))
        .bind(json!({"qty":1,"salt":format!("0x{}","ab".repeat(32)),"signature":"verified-by-authorization-endpoint"})).execute(store.pool()).await.unwrap();
    let path = format!("/v1/collections/{id}");
    let mock = Mock {
        responses: Arc::new(Mutex::new(BTreeMap::new())),
        redirect: Arc::new(Mutex::new(false)),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/v1/collections/{id}", get(reply))
        .route("/v1/collections/{id}/items", get(reply))
        .route("/v1/collections/{id}/itemCurations", get(reply))
        .with_state(mock.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let verifier = FoundationLinkedReview {
        base,
        client: foundation_client().allowing_private_hosts(),
    };
    let signed = FoundationReviewSignatures {
        collection: signed(&path).await,
        items: signed(&format!("{path}/items")).await,
        curations: signed(&format!("{path}/itemCurations")).await,
    };
    assert_eq!(
        verifier
            .verify(&store, OWNER, id, &signed)
            .await
            .unwrap()
            .status,
        "authorized"
    );
    mock.responses.lock().unwrap().extend([
        (path.clone(), collection.clone()),
        (format!("{path}/items"), items.clone()),
        (format!("{path}/itemCurations"), curations.clone()),
    ]);
    *mock.redirect.lock().unwrap() = true;
    assert!(
        verifier.verify(&store, OWNER, id, &signed).await.is_err(),
        "a redirecting Foundation is never followed"
    );
    *mock.redirect.lock().unwrap() = false;
    let mut mismatch = items.clone();
    mismatch[0]["local_content_hash"] = json!("changed");
    mock.responses
        .lock()
        .unwrap()
        .insert(format!("{path}/items"), mismatch);
    assert!(verifier.verify(&store, OWNER, id, &signed).await.is_err());
    assert_eq!(
        store
            .linked_publication_state(OWNER, id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "authorized"
    );
    mock.responses
        .lock()
        .unwrap()
        .insert(format!("{path}/items"), items);
    let (first, second) = tokio::join!(
        verifier.verify(&store, OWNER, id, &signed),
        verifier.verify(&store, OWNER, id, &signed)
    );
    let result = first.unwrap();
    assert_eq!(second.unwrap().status, "submitted");
    assert_eq!(result.status, "submitted");
    assert_eq!(
        result.forum_url.as_deref(),
        Some("https://forum.decentraland.org/t/hats/123")
    );
    let saved: (bool, String, String, bool) = sqlx::query_as(
        "SELECT is_published,urn_suffix,curation_status,is_approved FROM builder.items WHERE id=$1",
    )
    .bind(item)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        saved,
        (true, format!("{URN}:{item}"), "pending".into(), false)
    );
    let saved: (bool, bool, bool) = sqlx::query_as(
        "SELECT is_published,publication_pending,is_approved FROM builder.collections WHERE id=$1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(saved, (true, false, false));
    let cards = store.collection_drafts(OWNER).await.unwrap();
    let card = cards
        .iter()
        .find(|c| c.collection.id == id.to_string())
        .expect("the published linked collection stays listed");
    assert!(card.collection.is_published);
    assert_eq!(card.item_count, 1);
    assert!(
        store
            .collection_drafts("0x2222222222222222222222222222222222222222")
            .await
            .unwrap()
            .is_empty()
    );
    let nonsale: (String, Option<String>, i64) =
        sqlx::query_as("SELECT price,rarity,total_supply FROM builder.items WHERE id=$1")
            .bind(item)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(nonsale, ("0".into(), None, 0));
    server.abort();
    let reopened = ItemsComponent::new(store.pool().clone(), store.blobs.clone());
    assert_eq!(
        verifier
            .verify(&reopened, OWNER, id, &signed)
            .await
            .unwrap()
            .status,
        "submitted"
    );
    cleanup(&store, &[id], &[]).await;
}
