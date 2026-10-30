use serde_json::json;
use uuid::Uuid;

use crate::drafts::{CollectionDraft, ItemDraft};

use super::support::{cleanup, scratch_component, unique};

const OWNER: &str = "0x1111111111111111111111111111111111111111";
const OTHER: &str = "0x2222222222222222222222222222222222222222";

#[tokio::test]
async fn publication_uses_owned_persisted_files_and_stable_item_order() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let id = Uuid::new_v4();
    let collection: CollectionDraft =
        serde_json::from_value(json!({"id": id, "name": "Publication"})).unwrap();
    store
        .save_collection_draft(OWNER, &collection)
        .await
        .unwrap();
    assert!(store.prepare_publication(OWNER, id).await.is_err());
    let mut item: ItemDraft = serde_json::from_value(json!({
        "id": Uuid::new_v4(), "name": "Hat", "type": "wearable", "collection_id": id,
        "data": {"category": "hat", "representations": [{"mainFile": "model.glb", "contents": ["model.glb"],
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"]}]},
        "price": "0", "rarity": "rare", "thumbnail": "thumbnail.png"
    })).unwrap();
    store.save_item_draft(OWNER, &item).await.unwrap();
    assert!(store.prepare_publication(OWNER, id).await.is_err());
    let files = store
        .save_item_files(
            OWNER,
            item.id,
            vec![
                ("model.glb".into(), unique("model")),
                ("thumbnail.png".into(), unique("image")),
            ],
        )
        .await
        .unwrap();
    let hashes: Vec<String> = files.values().cloned().collect();
    item.contents = files.clone();
    store.save_item_draft(OWNER, &item).await.unwrap();
    let first = store.prepare_publication(OWNER, id).await.unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].metadata, "1:w:Hat::hat:BaseMale");
    assert!(store.prepare_publication(OTHER, id).await.is_err());
    assert_eq!(
        first.revision,
        store.prepare_publication(OWNER, id).await.unwrap().revision
    );
    let second_id = Uuid::new_v4();
    let first_id = item.id;
    item.id = second_id;
    store.save_item_draft(OWNER, &item).await.unwrap();
    sqlx::query("UPDATE builder.items SET created_at='2026-01-01T00:00:00' WHERE collection_id=$1")
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    let prepared = store.prepare_publication(OWNER, id).await.unwrap();
    let mut expected = [first_id.to_string(), second_id.to_string()];
    expected.sort();
    assert_eq!(
        prepared
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>(),
        expected,
        "equal created_at falls back to the id order"
    );
    assert_ne!(prepared.revision, first.revision);
    let refused = sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash=$1")
        .bind(&files["model.glb"])
        .execute(store.pool())
        .await;
    assert!(
        refused.is_err(),
        "a draft still names the upload, so the FK refuses the delete: the old missing_content case cannot arise"
    );
    assert!(store.prepare_publication(OWNER, id).await.is_ok());
    assert!(
        !store
            .collection_by_id(&id)
            .await
            .unwrap()
            .unwrap()
            .is_published,
        "preparation does not claim on-chain publication"
    );
    cleanup(&store, &[id], &hashes).await;
}
