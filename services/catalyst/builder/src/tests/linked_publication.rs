use alloy::signers::{SignerSync, local::PrivateKeySigner};
use alloy_primitives::{B256, U256, address};
use alloy_sol_types::{SolStruct, eip712_domain, sol};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::drafts::{CollectionDraft, ItemDraft};
use crate::items::ItemsComponent;
use crate::linked_publication::LinkedPublicationCheque;

use super::support::{OWNER_KEY, cleanup, scratch_component, unique};

const OWNER: &str = "0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a";
const OTHER: &str = "0x2222222222222222222222222222222222222222";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:provider";

sol! {
    struct ConsumeSlots {
        string thirdPartyId;
        uint256 qty;
        bytes32 salt;
    }
}

fn authorization(salt: &str) -> LinkedPublicationCheque {
    let signer: PrivateKeySigner = OWNER_KEY.parse().unwrap();
    let domain = eip712_domain! {
        name: "Decentraland Third Party Registry",
        version: "1",
        verifying_contract: address!("1C436C1EFb4608dFfDC8bace99d2B03c314f3348"),
        salt: B256::from(U256::from(137).to_be_bytes::<32>()),
    };
    let data = ConsumeSlots {
        thirdPartyId: PROVIDER.into(),
        qty: U256::from(1),
        salt: salt.parse().unwrap(),
    };
    LinkedPublicationCheque {
        qty: 1,
        salt: salt.into(),
        signature: signer
            .sign_hash_sync(&data.eip712_signing_hash(&domain))
            .unwrap()
            .to_string(),
    }
}

#[tokio::test]
async fn linked_publication_freezes_content_and_recovers_the_same_authorization() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let id = Uuid::new_v4();
    let mut collection: CollectionDraft = serde_json::from_value(json!({
        "id": id, "name": "Linked hats", "urn": format!("{PROVIDER}:{id}")
    }))
    .unwrap();
    store
        .save_collection_draft(OWNER, &collection)
        .await
        .unwrap();
    assert!(store.prepare_linked_publication(OWNER, id).await.is_err());
    let mut item: ItemDraft = serde_json::from_value(json!({
        "id": Uuid::new_v4(), "name": "Hat", "type": "wearable", "collection_id": id,
        "thumbnail": "thumbnail.png", "data": {"category": "hat", "representations": [{
            "mainFile": "hat.glb", "contents": ["hat.glb"], "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"]
        }]}, "metrics": {"triangles": 100}
    })).unwrap();
    store.save_item_draft(OWNER, &item).await.unwrap();
    assert!(store.prepare_linked_publication(OWNER, id).await.is_err());
    item.contents = store
        .save_item_files(
            OWNER,
            item.id,
            vec![
                ("hat.glb".into(), unique("model")),
                ("thumbnail.png".into(), unique("thumbnail")),
            ],
        )
        .await
        .unwrap();
    let hashes: Vec<String> = item.contents.values().cloned().collect();
    store.save_item_draft(OWNER, &item).await.unwrap();
    let stale = store.prepare_linked_publication(OWNER, id).await.unwrap();
    collection.name = "Updated hats".into();
    store
        .save_collection_draft(OWNER, &collection)
        .await
        .unwrap();
    assert!(
        store
            .begin_linked_publication(OWNER, id, &stale.revision)
            .await
            .is_err()
    );
    assert!(store.prepare_linked_publication(OTHER, id).await.is_err());
    assert!(store.prepare_publication(OWNER, id).await.is_err());
    let prepared = store.prepare_linked_publication(OWNER, id).await.unwrap();
    assert_eq!(prepared.item_ids, vec![item.id.to_string()]);
    let (a, b) = tokio::join!(
        store.begin_linked_publication(OWNER, id, &prepared.revision),
        store.begin_linked_publication(OWNER, id, &prepared.revision)
    );
    let first = a.unwrap();
    assert_eq!(first.salt, b.unwrap().salt);
    assert_eq!(first.status, "prepared");
    assert!(
        store
            .save_collection_draft(OWNER, &collection)
            .await
            .is_err()
    );
    assert!(store.save_item_draft(OWNER, &item).await.is_err());
    assert!(
        store
            .save_item_files(OWNER, item.id, vec![("hat.glb".into(), unique("changed"))])
            .await
            .is_err()
    );
    store.cancel_publication(OWNER, id).await.unwrap();
    assert!(
        store.save_item_draft(OWNER, &item).await.is_err(),
        "standard cancellation must not unlock linked publication"
    );
    assert!(store.cancel_linked_publication(OTHER, id).await.is_err());
    store.cancel_linked_publication(OWNER, id).await.unwrap();
    store.save_item_draft(OWNER, &item).await.unwrap();
    let prepared = store.prepare_linked_publication(OWNER, id).await.unwrap();
    let second = store
        .begin_linked_publication(OWNER, id, &prepared.revision)
        .await
        .unwrap();
    assert_ne!(first.salt, second.salt);
    let cheque = authorization(&second.salt);
    assert!(
        store
            .authorize_linked_publication(OWNER, id, &cheque)
            .await
            .is_err()
    );
    let claimed = store
        .claim_linked_publication(OWNER, id, &prepared.revision)
        .await
        .unwrap();
    assert_eq!(claimed.status, "signing");
    assert_eq!(
        store
            .claim_linked_publication(OWNER, id, &prepared.revision)
            .await
            .unwrap()
            .salt,
        second.salt
    );
    assert!(
        store
            .claim_linked_publication(OTHER, id, &prepared.revision)
            .await
            .is_err()
    );
    assert!(
        store
            .claim_linked_publication(OWNER, id, "other revision")
            .await
            .is_err()
    );
    assert!(store.cancel_linked_publication(OWNER, id).await.is_err());
    let mut invalid = cheque.clone();
    invalid.qty = 2;
    assert!(
        store
            .authorize_linked_publication(OWNER, id, &invalid)
            .await
            .is_err()
    );
    invalid = cheque.clone();
    invalid.salt = first.salt;
    assert!(
        store
            .authorize_linked_publication(OWNER, id, &invalid)
            .await
            .is_err()
    );
    invalid = cheque.clone();
    invalid.signature = format!("0x{}", "00".repeat(65));
    assert!(
        store
            .authorize_linked_publication(OWNER, id, &invalid)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .linked_publication_state(OWNER, id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "signing"
    );
    assert!(
        store
            .authorize_linked_publication(OTHER, id, &cheque)
            .await
            .is_err()
    );
    let authorized = store
        .authorize_linked_publication(OWNER, id, &cheque)
        .await
        .unwrap();
    assert_eq!(authorized.status, "authorized");
    let reopened = ItemsComponent::new(store.pool().clone(), store.blobs.clone());
    let persisted = reopened
        .linked_publication_state(OWNER, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.cheque, Some(cheque.clone()));
    assert_eq!(persisted.preparation.revision, prepared.revision);
    assert_eq!(persisted.salt, second.salt);
    assert!(
        reopened
            .linked_publication_state(OTHER, id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened
            .authorize_linked_publication(OWNER, id, &cheque)
            .await
            .unwrap()
            .status,
        "authorized"
    );
    assert!(reopened.cancel_linked_publication(OWNER, id).await.is_err());
    assert!(reopened.save_item_draft(OWNER, &item).await.is_err());
    let snapshot: Value = sqlx::query_scalar(
        "SELECT snapshot FROM builder.linked_collection_publications WHERE collection_id=$1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(snapshot["items"][0]["contents"], json!(item.contents));
    assert_eq!(snapshot["collection"]["name"], "Updated hats");
    let published: bool =
        sqlx::query_scalar("SELECT is_published FROM builder.collections WHERE id=$1")
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(!published, "authorization is not proof of publication");
    cleanup(&store, &[id], &hashes).await;
}

/// Without `BUILDER_FOUNDATION_URL` (every posture but mirror) verification
/// answers 503 before anything is sent; with one, the signer check runs.
#[tokio::test]
async fn verify_answers_503_while_no_foundation_is_named() {
    use tower::ServiceExt;
    let id = Uuid::new_v4();
    let status = |state: crate::SharedState| async move {
        crate::router(state)
            .oneshot(
                axum::http::Request::post(format!(
                    "/v1/collections/{id}/linked-publication/verify"
                ))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    r#"{"collection":{},"items":{},"curations":{}}"#,
                ))
                .unwrap(),
            )
            .await
            .unwrap()
            .status()
    };
    let open = crate::Settings {
        writes_enabled: true,
        ..crate::Settings::default()
    };
    assert_eq!(
        status(super::support::state_with_settings(open.clone()).await).await,
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    let named = crate::Settings {
        foundation_url: Some("https://foundation.example.org".into()),
        ..open
    };
    // The signer check runs: an unsigned request has no auth chain (400).
    assert_eq!(
        status(super::support::state_with_settings(named).await).await,
        axum::http::StatusCode::BAD_REQUEST
    );
}
