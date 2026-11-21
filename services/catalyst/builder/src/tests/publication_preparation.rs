use crate::access::granted;
use serde_json::json;
use uuid::Uuid;

use crate::drafts::CollectionDraft;

use super::support::{cleanup, contents_of, save_item, scratch_component, unique, upload};

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
    super::support::save_collection(
        &store,
        OWNER,
        serde_json::json!({"id": collection.id, "name": collection.name, "urn": collection.urn}),
    )
    .await
    .unwrap();
    assert!(
        store
            .prepare_publication(&granted::publisher(OWNER, id))
            .await
            .is_err()
    );
    let mut item = json!({
        "id": Uuid::new_v4(), "name": "Hat", "type": "wearable", "collection_id": id,
        "data": {"category": "hat", "representations": [{"mainFile": "model.glb", "contents": ["model.glb"],
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"]}]},
        "price": "0", "rarity": "rare", "thumbnail": "thumbnail.png"
    });
    let item_id = Uuid::parse_str(item["id"].as_str().unwrap()).unwrap();
    save_item(&store, OWNER, &item).await.unwrap();
    assert!(
        store
            .prepare_publication(&granted::publisher(OWNER, id))
            .await
            .is_err()
    );
    let (model, image) = (unique("model"), unique("image"));
    let files = contents_of(&[("model.glb", &model), ("thumbnail.png", &image)]);
    item["contents"] = files.clone();
    save_item(&store, OWNER, &item).await.unwrap();
    upload(&store, OWNER, item_id, &[&model, &image])
        .await
        .unwrap();
    let hashes: Vec<String> = files
        .as_object()
        .unwrap()
        .values()
        .map(|h| h.as_str().unwrap().to_string())
        .collect();
    let first = store
        .prepare_publication(&granted::publisher(OWNER, id))
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].metadata, "1:w:Hat::hat:BaseMale");
    assert!(
        store
            .prepare_publication(&granted::publisher(OTHER, id))
            .await
            .is_err()
    );
    assert_eq!(
        first.revision,
        store
            .prepare_publication(&granted::publisher(OWNER, id))
            .await
            .unwrap()
            .revision
    );
    let second_id = Uuid::new_v4();
    let first_id = item_id;
    item["id"] = json!(second_id);
    save_item(&store, OWNER, &item).await.unwrap();
    sqlx::query("UPDATE builder.items SET created_at='2026-01-01T00:00:00' WHERE collection_id=$1")
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    let prepared = store
        .prepare_publication(&granted::publisher(OWNER, id))
        .await
        .unwrap();
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
    assert!(
        store
            .prepare_publication(&granted::publisher(OWNER, id))
            .await
            .is_ok()
    );
    assert!(
        !store
            .collection_by_id(&granted::read(id))
            .await
            .unwrap()
            .unwrap()
            .is_published,
        "preparation does not claim on-chain publication"
    );
    cleanup(&store, &[id], &hashes).await;
}

async fn published_draft(store: &crate::items::ItemsComponent) -> (Uuid, Uuid) {
    let id = Uuid::new_v4();
    let draft = json!({ "id": id, "name": format!("Guarded {}", &id.simple().to_string()[..8]) });
    let draft: crate::drafts::CollectionDraft = serde_json::from_value(draft).unwrap();
    super::support::save_collection(
        store,
        OWNER,
        serde_json::json!({"id": draft.id, "name": draft.name, "urn": draft.urn}),
    )
    .await
    .unwrap();
    let item_id = Uuid::new_v4();
    super::support::save_item(
        store,
        OWNER,
        &json!({
            "id": item_id, "name": "Hat", "type": "wearable", "collection_id": id,
            "data": {"category": "hat"}, "price": "0", "rarity": "rare"
        }),
    )
    .await
    .unwrap();
    (id, item_id)
}

async fn publication_rows(store: &crate::items::ItemsComponent, id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM builder.collection_publications WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

fn already_published(err: crate::http::ApiError) {
    assert_eq!(
        (err.status(), err.to_string()),
        (400, "Collection has already been published".to_string())
    );
}

/// A collection published by the copy of the subgraph or by a minted item
/// can't start another publication, and a published receipt
/// keeps the standard flow on the publication it recorded.
#[tokio::test]
async fn a_published_collection_cannot_begin_another_publication() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let mut created = Vec::new();
    for signal in ["copy", "minted"] {
        {
            let (id, item) = published_draft(&store).await;
            created.push(id);
            let contract = if signal == "copy" {
                Some(super::support::on_chain(store.pool(), id, false).await)
            } else {
                sqlx::query("UPDATE builder.items SET blockchain_item_id = '0' WHERE id = $1")
                    .bind(item)
                    .execute(store.pool())
                    .await
                    .unwrap();
                None
            };
            already_published(
                store
                    .prepare_publication(&granted::publisher(OWNER, id))
                    .await
                    .unwrap_err(),
            );
            already_published(
                store
                    .begin_publication(&granted::publisher(OWNER, id), "any revision")
                    .await
                    .unwrap_err(),
            );
            assert_eq!(
                publication_rows(&store, id).await,
                0,
                "{signal}: nothing begun"
            );
            if let Some(contract) = contract {
                super::support::off_chain(store.pool(), &contract).await;
            }
        }
    }
    let (id, _) = published_draft(&store).await;
    created.push(id);
    sqlx::query(
        "INSERT INTO builder.collection_publications (collection_id, preparation, status, tx_hash, contract_address) \
         VALUES ($1, $4, 'published', $2, $3)",
    )
    .bind(id)
    .bind(format!("0x{}", Uuid::new_v4().simple()))
    .bind(format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2)))
    .bind(json!({
        "id": id, "revision": "recorded", "chain_id": 137, "manager": OWNER, "factory": OWNER,
        "forwarder": OWNER, "salt": "0x00", "name": "Guarded", "symbol": "DCL-GRDD",
        "base_uri": "https://example.org/", "creator": OWNER, "items": []
    }))
    .execute(store.pool())
    .await
    .unwrap();
    assert_eq!(
        store
            .begin_publication(&granted::publisher(OWNER, id), "another revision")
            .await
            .unwrap_err()
            .status(),
        409,
        "a published receipt keeps the collection on its recorded publication"
    );
    let status: String = sqlx::query_scalar(
        "SELECT status FROM builder.collection_publications WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(status, "published");
    cleanup(&store, &created, &[]).await;
}

/// How the publications see a named file: its owner uploaded it, only another
/// wallet did, nobody did, or the owner did while another wallet wrote the
/// name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Uploader {
    Owner,
    AnotherWallet,
    Nobody,
    NamedByAnother,
}

/// Saves `item` as `owner` naming `model` and `thumbnail`, and records the
/// files' uploads as `uploaded` says.
pub(super) async fn name_files(
    store: &crate::items::ItemsComponent,
    owner: &str,
    other: &str,
    item: &mut serde_json::Value,
    files: [(&str, &[u8]); 2],
    uploaded: Uploader,
) {
    let item_id = Uuid::parse_str(item["id"].as_str().unwrap()).unwrap();
    item["contents"] = contents_of(&files);
    save_item(store, owner, item).await.unwrap();
    match uploaded {
        Uploader::Owner => upload(store, owner, item_id, &[files[0].1, files[1].1])
            .await
            .unwrap(),
        Uploader::AnotherWallet => {
            for (_, bytes) in files {
                sqlx::query("INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, 1, $2)")
                    .bind(dcl_auth::hash_bytes_v1(bytes))
                    .bind(other)
                    .execute(store.pool())
                    .await
                    .unwrap();
            }
        }
        Uploader::Nobody => {}
        Uploader::NamedByAnother => {
            upload(store, owner, item_id, &[files[0].1, files[1].1])
                .await
                .unwrap();
            sqlx::query("UPDATE builder.item_contents SET named_by = $2 WHERE item_id = $1")
                .bind(item_id)
                .bind(other)
                .execute(store.pool())
                .await
                .unwrap();
        }
    }
}

#[tokio::test]
async fn a_standard_publication_takes_only_files_each_names_writer_uploaded() {
    let Some(store) = scratch_component().await else {
        return;
    };
    for uploaded in [
        Uploader::Nobody,
        Uploader::AnotherWallet,
        Uploader::NamedByAnother,
        Uploader::Owner,
    ] {
        let id = Uuid::new_v4();
        super::support::save_collection(
            &store,
            OWNER,
            json!({"id": id, "name": format!("Possession {}", &id.simple().to_string()[..8])}),
        )
        .await
        .unwrap();
        let mut item = json!({
            "id": Uuid::new_v4(), "name": "Hat", "type": "wearable", "collection_id": id,
            "data": {"category": "hat", "representations": [{"mainFile": "model.glb", "contents": ["model.glb"],
            "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"]}]},
            "price": "0", "rarity": "rare", "thumbnail": "thumbnail.png"
        });
        let (model, image) = (unique("possession model"), unique("possession image"));
        name_files(
            &store,
            OWNER,
            OTHER,
            &mut item,
            [("model.glb", &model), ("thumbnail.png", &image)],
            uploaded,
        )
        .await;
        let publisher = granted::publisher(OWNER, id);
        let prepared = store.prepare_publication(&publisher).await;
        if uploaded == Uploader::Owner {
            let revision = prepared.unwrap().revision;
            store
                .begin_publication(&publisher, &revision)
                .await
                .unwrap();
        } else {
            let refused = prepared.unwrap_err();
            assert_eq!(refused.status(), 400, "{uploaded:?}: {refused}");
            assert!(
                refused.to_string().contains("upload missing content"),
                "{uploaded:?}: {refused}"
            );
            let begun = store
                .begin_publication(&publisher, "any revision")
                .await
                .unwrap_err();
            assert_eq!(begun.status(), 400, "{uploaded:?}: {begun}");
            assert!(
                begun.to_string().contains("upload missing content"),
                "{uploaded:?}: {begun}"
            );
            assert_eq!(
                store
                    .claim_publication(&publisher, "any revision")
                    .await
                    .unwrap_err()
                    .status(),
                409,
                "{uploaded:?}: claimed"
            );
        }
        let hashes = [
            dcl_auth::hash_bytes_v1(&model),
            dcl_auth::hash_bytes_v1(&image),
        ];
        cleanup(&store, &[id], &hashes).await;
    }
}

/// A file whose upload is recorded but whose bytes the store no longer
/// holds keeps the collection from publishing, as an unuploaded one does,
/// until it is uploaded again.
#[tokio::test]
async fn a_recorded_file_without_its_bytes_keeps_the_collection_from_publishing() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let id = Uuid::new_v4();
    super::support::save_collection(
        &store,
        OWNER,
        json!({"id": id, "name": format!("Lost {}", &id.simple().to_string()[..8])}),
    )
    .await
    .unwrap();
    let mut item = json!({
        "id": Uuid::new_v4(), "name": "Hat", "type": "wearable", "collection_id": id,
        "data": {"category": "hat", "representations": [{"mainFile": "model.glb", "contents": ["model.glb"],
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"]}]},
        "price": "0", "rarity": "rare", "thumbnail": "thumbnail.png"
    });
    let (model, image) = (unique("lost model"), unique("lost image"));
    name_files(
        &store,
        OWNER,
        OTHER,
        &mut item,
        [("model.glb", &model), ("thumbnail.png", &image)],
        Uploader::Owner,
    )
    .await;
    let publisher = granted::publisher(OWNER, id);
    assert!(store.prepare_publication(&publisher).await.is_ok());
    store
        .blobs
        .delete(&dcl_auth::hash_bytes_v1(&model))
        .await
        .unwrap();
    let refused = store.prepare_publication(&publisher).await.unwrap_err();
    assert!(
        refused.to_string().contains("upload missing content"),
        "{refused}"
    );
    let begun = store
        .begin_publication(&publisher, "any revision")
        .await
        .unwrap_err();
    assert!(
        begun.to_string().contains("upload missing content"),
        "{begun}"
    );
    cleanup(&store, &[id], &[]).await;
}
