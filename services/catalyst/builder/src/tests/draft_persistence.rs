use serde_json::json;
use uuid::Uuid;

use crate::drafts::{CollectionDraft, ItemDraft};
use crate::items::ItemsComponent;
use dcl_auth::{hash_bytes_v1, verify_hash};

use super::support::{cleanup, scratch_component, unique};

const OWNER: &str = "0x1111111111111111111111111111111111111111";
const OTHER: &str = "0x2222222222222222222222222222222222222222";

#[tokio::test]
async fn draft_delete_preserves_shared_uploads_and_serializes_with_publication() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let collection = Uuid::new_v4();
    let draft =
        serde_json::from_value(json!({ "id": collection, "name": "Delete fixture" })).unwrap();
    store.save_collection_draft(OWNER, &draft).await.unwrap();
    let mut item: ItemDraft = serde_json::from_value(json!({
        "id": Uuid::new_v4(), "name": "Delete me", "type": "wearable", "collection_id": collection,
        "data": { "category": "upper_body" },
    }))
    .unwrap();
    store.save_item_draft(OWNER, &item).await.unwrap();
    let bytes = unique("shared model");
    let files = store
        .save_item_files(OWNER, item.id, vec![("model.glb".into(), bytes.clone())])
        .await
        .unwrap();
    let deleted_id = item.id;
    item.id = Uuid::new_v4();
    item.contents = files.clone();
    store.save_item_draft(OWNER, &item).await.unwrap();
    let survivor = store.item_draft(OWNER, item.id).await.unwrap();
    assert_eq!(
        store
            .delete_item_draft(OTHER, deleted_id)
            .await
            .unwrap_err()
            .status(),
        403
    );
    let before = store.item_draft(OWNER, deleted_id).await.unwrap();

    let mut publishing = store.pool().begin().await.unwrap();
    let publisher: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *publishing)
        .await
        .unwrap();
    sqlx::query("UPDATE builder.collections SET publication_pending=true WHERE id=$1")
        .bind(collection)
        .execute(&mut *publishing)
        .await
        .unwrap();
    let delete = store.delete_item_draft(OWNER, deleted_id);
    tokio::pin!(delete);
    tokio::select! {
        biased;
        _ = &mut delete => panic!("delete must wait for the publishing parent"),
        queued = crate::shared::test_wait::until(async || {
            sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
            )
            .bind(publisher)
            .fetch_one(store.pool())
            .await
            .unwrap()
        }) => assert!(queued, "delete never queued behind the publishing parent"),
    }
    publishing.commit().await.unwrap();
    assert_eq!(delete.await.unwrap_err().status(), 403);
    assert_eq!(store.item_draft(OWNER, deleted_id).await.unwrap(), before);

    sqlx::query(
        "UPDATE builder.collections SET publication_pending=false, is_published=true WHERE id=$1",
    )
    .bind(collection)
    .execute(store.pool())
    .await
    .unwrap();
    assert_eq!(
        store
            .delete_item_draft(OWNER, deleted_id)
            .await
            .unwrap_err()
            .status(),
        403
    );
    sqlx::query("UPDATE builder.collections SET is_published=false WHERE id=$1")
        .bind(collection)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE builder.items SET is_published=true, urn_suffix='0', blockchain_item_id='0' WHERE id=$1")
        .bind(deleted_id).execute(store.pool()).await.unwrap();
    assert_eq!(
        store
            .delete_item_draft(OWNER, deleted_id)
            .await
            .unwrap_err()
            .status(),
        403
    );
    sqlx::query("UPDATE builder.items SET is_published=false WHERE id=$1")
        .bind(deleted_id)
        .execute(store.pool())
        .await
        .unwrap();

    store.delete_item_draft(OWNER, deleted_id).await.unwrap();
    assert_eq!(
        store
            .item_draft(OWNER, deleted_id)
            .await
            .unwrap_err()
            .status(),
        404
    );
    assert_eq!(
        store
            .delete_item_draft(OWNER, deleted_id)
            .await
            .unwrap_err()
            .status(),
        404
    );
    assert_eq!(store.item_draft(OWNER, item.id).await.unwrap(), survivor);
    assert_eq!(
        store
            .uploaded_content(&files["model.glb"])
            .await
            .unwrap()
            .as_deref(),
        Some(bytes.as_slice())
    );
    let links: i64 =
        sqlx::query_scalar("SELECT count(*) FROM builder.item_contents WHERE item_id=$1")
            .bind(deleted_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(links, 0);
    cleanup(&store, &[collection], &[files["model.glb"].clone()]).await;
}

#[tokio::test]
async fn drafts_and_files_persist_with_owner_and_publication_guards() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let collection = Uuid::new_v4();
    let draft: CollectionDraft =
        serde_json::from_value(json!({ "id": collection, "name": "Collection draft" })).unwrap();
    let saved = store.save_collection_draft(OWNER, &draft).await.unwrap();
    assert_eq!(saved.name, "Collection draft");
    assert!(!saved.is_published);
    assert!(saved.contract_address.is_none());
    assert!(store.save_collection_draft(OTHER, &draft).await.is_err());
    assert!(store.collection_drafts(OTHER).await.unwrap().is_empty());
    assert!(
        store
            .collection_drafts(OWNER)
            .await
            .unwrap()
            .iter()
            .any(|c| c.collection.id == collection.to_string())
    );

    let id = Uuid::new_v4();
    let mut item: ItemDraft = serde_json::from_value(json!({
        "id": id, "name": "My wearable", "collection_id": collection,
        "type": "wearable", "data": { "category": "upper_body" }, "rarity": "common", "price": "0",
        "metrics": { "triangles": 42, "meshes": 1 }
    }))
    .unwrap();
    store.save_item_draft(OWNER, &item).await.unwrap();
    assert!(store.save_item_draft(OTHER, &item).await.is_err());
    assert!(store.item_draft(OTHER, id).await.is_err());
    let bytes = unique("actual model bytes");
    let files = store
        .save_item_files(OWNER, id, vec![("model.glb".into(), bytes.clone())])
        .await
        .unwrap();
    let hash = files["model.glb"].clone();
    assert!(verify_hash(&bytes, &hash));
    assert_eq!(
        store.uploaded_content(&hash).await.unwrap().as_deref(),
        Some(bytes.as_slice()),
        "the upload is served back from the ONE blob store"
    );
    assert!(store.uploaded_content_exists(&hash).await.unwrap());
    let (size, uploaded_by): (i64, String) =
        sqlx::query_as("SELECT size, uploaded_by FROM builder.uploaded_contents WHERE hash=$1")
            .bind(&hash)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        (size as usize, uploaded_by.as_str()),
        (bytes.len(), OWNER),
        "the acceptance record"
    );
    assert!(
        store
            .save_item_files(OTHER, id, vec![("model.glb".into(), unique("overwrite"))])
            .await
            .is_err()
    );
    item.name = "Edited wearable".into();
    item.metrics = None;
    item.contents = files;
    store.save_item_draft(OWNER, &item).await.unwrap();
    let reopened = ItemsComponent::new(store.pool().clone(), store.blobs.clone())
        .item_draft(OWNER, id)
        .await
        .unwrap();
    assert!(
        store
            .item_drafts(OWNER)
            .await
            .unwrap()
            .iter()
            .any(|d| d["id"] == id.to_string()),
        "the draft is listed among the owner's item drafts"
    );
    assert!(store.item_drafts(OTHER).await.unwrap().is_empty());
    assert_eq!(reopened["name"], "Edited wearable");
    assert_eq!(reopened["metrics"], json!({ "triangles": 42, "meshes": 1 }));
    assert!(
        reopened["created_at"].as_str().unwrap().ends_with("+00:00"),
        "the draft document keeps the old timestamptz rendering: {}",
        reopened["created_at"]
    );
    item.metrics = Some(json!({ "triangles": -1 }));
    assert!(store.save_item_draft(OWNER, &item).await.is_err());
    item.metrics = None;
    assert_eq!(reopened["data"]["category"], "upper_body");
    assert_eq!(reopened["contents"]["model.glb"], hash);

    item.contents
        .insert("missing.glb".into(), hash_bytes_v1(&unique("not uploaded")));
    let refused = store.save_item_draft(OWNER, &item).await.unwrap_err();
    assert_eq!(
        refused.status(),
        400,
        "a hash this node never accepted is refused by the FK as the old probe did: {refused}"
    );
    assert_eq!(
        store.item_draft(OWNER, id).await.unwrap(),
        reopened,
        "invalid writes roll back metadata and content links"
    );
    item.contents.remove("missing.glb");
    sqlx::query("UPDATE builder.collections SET is_published=true WHERE id=$1")
        .bind(collection)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store.save_collection_draft(OWNER, &draft).await.is_err());
    assert!(
        store
            .save_item_files(OWNER, id, vec![("model.glb".into(), unique("overwrite"))])
            .await
            .is_err()
    );
    item.collection_id = None;
    assert!(
        store.save_item_draft(OWNER, &item).await.is_err(),
        "cannot detach an item to bypass publication protection"
    );
    assert_eq!(
        store.uploaded_content(&hash).await.unwrap().as_deref(),
        Some(bytes.as_slice())
    );
    let linked_id = Uuid::new_v4();
    let provider = "urn:decentraland:matic:collections-thirdparty:example";
    let urn = format!("{provider}:{linked_id}");
    let mut linked: CollectionDraft =
        serde_json::from_value(json!({ "id": linked_id, "name": "Linked draft", "urn": urn }))
            .unwrap();
    let saved = store.save_collection_draft(OWNER, &linked).await.unwrap();
    assert_eq!(saved.urn.as_deref(), Some(urn.as_str()));
    assert_eq!(saved.third_party_id.as_deref(), Some(provider));
    assert!(!saved.is_published);
    linked.name = "Renamed linked draft".into();
    assert_eq!(
        store
            .save_collection_draft(OWNER, &linked)
            .await
            .unwrap()
            .name,
        linked.name
    );
    assert!(store.save_collection_draft(OTHER, &linked).await.is_err());
    linked.urn = None;
    assert!(store.save_collection_draft(OWNER, &linked).await.is_err());
    linked.urn = Some(format!(
        "urn:decentraland:matic:collections-thirdparty:other:{linked_id}"
    ));
    assert!(store.save_collection_draft(OWNER, &linked).await.is_err());
    assert_eq!(
        store
            .collection_by_id(&linked_id)
            .await
            .unwrap()
            .unwrap()
            .urn_suffix
            .as_deref(),
        Some(urn.as_str())
    );
    linked.urn = Some("urn:decentraland:matic:collections-v2:forged".into());
    assert!(linked.validate(linked_id, OWNER).is_err());
    linked.urn = Some(format!("{provider}:bad suffix"));
    assert!(linked.validate(linked_id, OWNER).is_err());
    cleanup(&store, &[collection, linked_id], &[hash]).await;
}
