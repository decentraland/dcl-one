use super::*;

async fn publish(
    rig: &Rig,
    community: CommunityId,
    author: &Wallet,
    key: Option<&str>,
    content: &str,
) -> (StatusCode, Value) {
    let mut case = Case::new("post", format!("/v1/communities/{community}/posts"))
        .signed(author)
        .await
        .json(&json!({ "content": content }));
    if let Some(key) = key {
        case.headers
            .push(("idempotency-key".to_string(), key.to_string()));
    }
    hit(&rig.app, case).await
}

async fn stored(rig: &Rig, community: CommunityId) -> Vec<(Uuid, String, String)> {
    sqlx::query_as(
        "SELECT id, author_address, content FROM social.community_posts \
         WHERE community_id = $1 ORDER BY created_at, id",
    )
    .bind(community)
    .fetch_all(&rig.pool)
    .await
    .unwrap()
}

/// A publish whose answer was lost is retried with the same Idempotency-Key:
/// the retry answers with the first attempt's post and stores nothing new.
#[tokio::test]
async fn a_retried_publish_with_the_same_key_stores_one_post() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, moderator) = (wallet(), wallet());
    let community = rig.community(&owner, false).await;
    rig.member(community, &moderator, "moderator").await;

    let (status, first) = publish(&rig, community, &owner, Some("draft-1"), "Hello").await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let (status, retry) = publish(&rig, community, &owner, Some("draft-1"), "Hello").await;
    assert_eq!(status, StatusCode::CREATED, "{retry}");
    assert_eq!(retry, first);
    let id = Uuid::parse_str(first["data"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        stored(&rig, community).await,
        vec![(id, owner.address.as_str().to_string(), "Hello".to_string())]
    );

    // A new key, another author with the same key, and no key at all are new posts.
    let (status, _) = publish(&rig, community, &owner, Some("draft-2"), "Hello").await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, theirs) = publish(&rig, community, &moderator, Some("draft-1"), "Hello").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(theirs["data"]["id"], first["data"]["id"]);
    for _ in 0..2 {
        let (status, _) = publish(&rig, community, &owner, None, "Unkeyed").await;
        assert_eq!(status, StatusCode::CREATED);
    }
    assert_eq!(stored(&rig, community).await.len(), 5);

    // The key is scoped to the community too.
    let other = rig.community(&owner, false).await;
    let (status, elsewhere) = publish(&rig, other, &owner, Some("draft-1"), "Hello").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(elsewhere["data"]["id"], first["data"]["id"]);
    assert_eq!(stored(&rig, other).await.len(), 1);
}

#[tokio::test]
async fn an_empty_or_oversized_idempotency_key_is_refused_before_storing() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    let community = rig.community(&owner, false).await;
    for key in [" ", &"k".repeat(256)] {
        let (status, body) = publish(&rig, community, &owner, Some(key), "Hello").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    assert!(stored(&rig, community).await.is_empty());
}
