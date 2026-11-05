//! Authority and listing rules as upstream (`decentraland/social-service-ea`
//! at `fb3dcb8`) applies them: the owner-only settings cell, pending-only
//! request listings, the inviter's managed communities and the viewer's bans.

use super::*;

async fn ban(rig: &Rig, id: CommunityId, banned: &Wallet, by: &Wallet) {
    sqlx::query(
        "INSERT INTO social.community_bans (community_id, banned_address, banned_by) VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(&banned.address)
    .bind(&by.address)
    .execute(&rig.pool)
    .await
    .unwrap();
}

async fn decided_request(rig: &Rig, id: CommunityId, w: &Wallet, status: &str) {
    sqlx::query(
        "INSERT INTO social.community_requests (id, community_id, member_address, status, type) \
         VALUES ($1, $2, $3, $4, 'request_to_join')",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(&w.address)
    .bind(status)
    .execute(&rig.pool)
    .await
    .unwrap();
}

fn result_ids(v: &Value, key: &str) -> Vec<String> {
    let mut ids: Vec<String> = v["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[key].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

/// `validatePermissionToEditCommunitySettings`: a moderator edits the
/// description but may not change privacy or visibility; resending the
/// stored values needs no settings permission.
#[tokio::test]
async fn a_moderator_edits_info_but_not_privacy_or_visibility() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, moderator) = (wallet(), wallet());
    let community = rig.community(&owner, false).await;
    rig.member(community, &moderator, "moderator").await;
    let path = format!("/v1/communities/{community}");

    let (body, ct) = multipart(&[
        ("description", b"by the moderator"),
        ("privacy", b"public"),
        ("visibility", b"all"),
    ]);
    let (status, v) = hit(
        &rig.app,
        Case::new("put", &path)
            .signed(&moderator)
            .await
            .body(body, &ct),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "unchanged settings: {v}");
    assert_eq!(v["data"]["description"], "by the moderator");

    for fields in [
        &[("privacy", b"private" as &[u8])] as &[(&str, &[u8])],
        &[("visibility", b"unlisted" as &[u8])],
    ] {
        let (body, ct) = multipart(fields);
        let (status, v) = hit(
            &rig.app,
            Case::new("put", &path)
                .signed(&moderator)
                .await
                .body(body, &ct),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{v}");
        assert_eq!(
            v["message"],
            format!(
                "The user {} doesn't have permission to update the community privacy",
                moderator.address
            )
        );
    }
    let stored: (bool, bool) =
        sqlx::query_as("SELECT private, unlisted FROM social.communities WHERE id = $1")
            .bind(community)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(stored, (false, false));

    let (body, ct) = multipart(&[("privacy", b"private"), ("visibility", b"unlisted")]);
    let (status, v) = hit(
        &rig.app,
        Case::new("put", &path).signed(&owner).await.body(body, &ct),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["privacy"], "private");
    rig.finish();
}

/// `getCommunityRequests`/`getMemberRequests` list pending rows only; the
/// member's page then drops inactive communities and those that ban them
/// (`aggregateRequestsWithCommunities`), the total still counting them.
#[tokio::test]
async fn request_listings_show_pending_rows_of_live_communities_that_admit_the_wallet() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, wallet_a) = (wallet(), wallet());
    rig.profile(&wallet_a, "Requester", false).await;
    let live = rig.community(&owner, true).await;
    let decided = rig.community(&owner, true).await;
    let inactive = rig.community(&owner, true).await;
    let banning = rig.community(&owner, true).await;
    let pending = rig.request(live, &wallet_a, "request_to_join").await;
    for status in ["accepted", "rejected", "cancelled"] {
        decided_request(&rig, decided, &wallet_a, status).await;
        decided_request(&rig, live, &wallet(), status).await;
    }
    rig.request(inactive, &wallet_a, "request_to_join").await;
    rig.request(banning, &wallet_a, "request_to_join").await;
    sqlx::query("UPDATE social.communities SET active = FALSE WHERE id = $1")
        .bind(inactive)
        .execute(&rig.pool)
        .await
        .unwrap();
    ban(&rig, banning, &wallet_a, &owner).await;

    for version in ["v1", "v2"] {
        let path = format!("/{version}/communities/{live}/requests");
        let (status, v) = hit(&rig.app, Case::new("get", &path).signed(&owner).await).await;
        assert_eq!(status, StatusCode::OK, "{path}: {v}");
        assert_eq!(v["data"]["total"], 1, "{path}: {v}");
        assert_eq!(result_ids(&v, "id"), vec![pending.to_string()], "{path}");
        let path = format!("/{version}/communities/{decided}/requests");
        let (_, v) = hit(&rig.app, Case::new("get", &path).signed(&owner).await).await;
        assert_eq!(v["data"]["total"], 0, "{path}: {v}");

        let path = format!("/{version}/members/{}/requests", wallet_a.address);
        let (status, v) = hit(&rig.app, Case::new("get", &path).signed(&wallet_a).await).await;
        assert_eq!(status, StatusCode::OK, "{path}: {v}");
        assert_eq!(
            result_ids(&v, "communityId"),
            vec![live.to_string()],
            "{path}: {v}"
        );
        assert_eq!(v["data"]["total"], 3, "upstream's pending count: {v}");
    }
    rig.finish();
}

/// `getCommunityInvites`: only communities the inviter owns or moderates.
#[tokio::test]
async fn invites_list_only_communities_the_inviter_manages() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, inviter, invitee) = (wallet(), wallet(), wallet());
    let owned = rig.community(&inviter, false).await;
    let moderated = rig.community(&owner, false).await;
    let joined = rig.community(&owner, false).await;
    rig.member(moderated, &inviter, "moderator").await;
    rig.member(joined, &inviter, "member").await;

    let path = format!("/v1/members/{}/invites", invitee.address);
    let (status, v) = hit(&rig.app, Case::new("get", &path).signed(&inviter).await).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    let mut expected = vec![owned.to_string(), moderated.to_string()];
    expected.sort();
    assert_eq!(ids, expected);
    rig.finish();
}

/// `getCommunities`/`getCommunitiesCount`: a signed viewer's listing leaves
/// out the communities that ban them, in the rows and the total.
#[tokio::test]
async fn a_signed_listing_hides_communities_that_ban_the_viewer() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, viewer) = (wallet(), wallet());
    let open = rig.community(&owner, false).await;
    let banning = rig.community(&owner, false).await;
    ban(&rig, banning, &viewer, &owner).await;
    let token = format!("bans{}", Uuid::new_v4().simple());
    sqlx::query("UPDATE social.communities SET description = $2 WHERE id = ANY($1)")
        .bind(vec![open.as_uuid(), banning.as_uuid()])
        .bind(&token)
        .execute(&rig.pool)
        .await
        .unwrap();

    for version in ["v1", "v2"] {
        for (who, sees_banning) in [(&viewer, false), (&owner, true)] {
            let path = format!("/{version}/communities?search={token}&limit=100");
            let (status, v) = hit(&rig.app, Case::new("get", &path).signed(who).await).await;
            assert_eq!(status, StatusCode::OK, "{path}: {v}");
            let ids = result_ids(&v, "id");
            assert!(ids.contains(&open.to_string()), "{path}: {v}");
            assert_eq!(
                ids.contains(&banning.to_string()),
                sees_banning,
                "{path}: {v}"
            );
            assert_eq!(v["data"]["total"], ids.len(), "{path}: {v}");
        }
    }
    rig.finish();
}
