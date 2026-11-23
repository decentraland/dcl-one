use super::*;

async fn decide(app: &Router, actor: &Wallet, path: &str, status: &str) -> StatusCode {
    hit(
        app,
        Case::new("patch", path)
            .signed(actor)
            .await
            .json(&json!({"intention": status})),
    )
    .await
    .0
}

#[tokio::test]
async fn accepted_invitation_replay_does_not_rejoin_after_leaving() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let community = rig.community(&owner, true).await;
    let request = rig.request(community, &member, "invite").await;
    let path = format!("/v1/communities/{community}/requests/{request}");
    assert_eq!(
        decide(&rig.app, &member, &path, "accepted").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        rig.role_of(community, &member).await.as_deref(),
        Some("member")
    );
    let leave = format!("/v1/communities/{community}/members/{}", member.address);
    assert_eq!(
        hit(&rig.app, Case::new("delete", leave).signed(&member).await)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        decide(&rig.app, &member, &path, "accepted").await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        rig.role_of(community, &member).await,
        None,
        "an acknowledged retry must not grant membership again"
    );
    assert_eq!(
        decide(&rig.app, &member, &path, "rejected").await,
        StatusCode::NOT_FOUND
    );
    let fresh = rig.request(community, &member, "invite").await;
    assert_eq!(
        decide(
            &rig.app,
            &member,
            &format!("/v1/communities/{community}/requests/{fresh}"),
            "accepted"
        )
        .await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        rig.role_of(community, &member).await.as_deref(),
        Some("member"),
        "a new invitation can still grant access"
    );
}

#[tokio::test]
async fn rejected_and_cancelled_requests_cannot_be_accepted_later() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, moderator) = (wallet(), wallet());
    let community = rig.community(&owner, true).await;
    rig.member(community, &moderator, "moderator").await;
    for (kind, terminal) in [
        ("invite", "rejected"),
        ("invite", "cancelled"),
        ("request_to_join", "rejected"),
        ("request_to_join", "cancelled"),
    ] {
        let member = wallet();
        let request = rig.request(community, &member, kind).await;
        let path = format!("/v1/communities/{community}/requests/{request}");
        let terminal_actor = if (kind == "invite") == (terminal == "rejected") {
            &member
        } else {
            &moderator
        };
        assert_eq!(
            decide(&rig.app, terminal_actor, &path, terminal).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            decide(&rig.app, terminal_actor, &path, terminal).await,
            StatusCode::NOT_FOUND
        );
        let acceptor = if kind == "invite" {
            &member
        } else {
            &moderator
        };
        assert_eq!(
            decide(&rig.app, acceptor, &path, "accepted").await,
            StatusCode::NOT_FOUND,
            "{kind}/{terminal}"
        );
        assert_eq!(rig.role_of(community, &member).await, None);
    }
}

#[tokio::test]
async fn competing_request_decisions_have_one_winner_and_matching_membership() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, moderator) = (wallet(), wallet());
    let community = rig.community(&owner, true).await;
    rig.member(community, &moderator, "moderator").await;
    for _ in 0..12 {
        let member = wallet();
        let request = rig.request(community, &member, "request_to_join").await;
        let path = format!("/v1/communities/{community}/requests/{request}");
        let (accept, reject) = tokio::join!(
            decide(&rig.app, &owner, &path, "accepted"),
            decide(&rig.app, &moderator, &path, "rejected")
        );
        assert!(
            matches!(
                (accept, reject),
                (StatusCode::NO_CONTENT, StatusCode::NOT_FOUND)
                    | (StatusCode::NOT_FOUND, StatusCode::NO_CONTENT)
            ),
            "accept={accept}, reject={reject}"
        );
        let retained: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM social.community_requests WHERE id = $1")
                .bind(request)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        assert_eq!(retained, 0);
        assert_eq!(
            rig.role_of(community, &member).await.is_some(),
            accept == StatusCode::NO_CONTENT
        );
    }
}

#[tokio::test]
async fn opposite_type_auto_accept_cannot_grant_access_after_a_competing_cancellation() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let community = rig.community(&owner, true).await;
    let request = rig.request(community, &member, "request_to_join").await;
    let mut fence = rig.pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *fence)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM social.community_requests WHERE id = $1 FOR UPDATE")
        .bind(request)
        .execute(&mut *fence)
        .await
        .unwrap();
    let cancel = Case::new(
        "patch",
        format!("/v1/communities/{community}/requests/{request}"),
    )
    .signed(&member)
    .await
    .json(&json!({"intention":"cancelled"}));
    let app = rig.app.clone();
    let cancelled = tokio::spawn(async move { hit(&app, cancel).await });
    wait_for_decisions(&rig.pool, blocker, 1).await;
    let invite = Case::new("post", format!("/v1/communities/{community}/requests"))
        .signed(&owner)
        .await
        .json(&json!({"type":"invite", "targetedAddress":member.address.as_str()}));
    let app = rig.app.clone();
    let accepted = tokio::spawn(async move { hit(&app, invite).await });
    wait_for_decisions(&rig.pool, blocker, 2).await;
    fence.commit().await.unwrap();
    assert_eq!(cancelled.await.unwrap().0, StatusCode::NO_CONTENT);
    assert_eq!(accepted.await.unwrap().0, StatusCode::NOT_FOUND);
    assert_eq!(rig.role_of(community, &member).await, None);
    let retained: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM social.community_requests WHERE id = $1")
            .bind(request)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(retained, 0);
}

async fn wait_for_decisions(pool: &PgPool, blocker: i32, count: i64) {
    let queued = crate::shared::test_wait::until(async || {
        // The second updater may wait on the first updater's tuple lock,
        // so include descendants of the fence in the blocking graph.
        let waiting: i64 = sqlx::query_scalar(
            "WITH RECURSIVE waiting(pid) AS (SELECT $1::int UNION \
             SELECT a.pid FROM pg_stat_activity a JOIN waiting w ON w.pid = ANY(pg_blocking_pids(a.pid))) \
             SELECT count(*) - 1 FROM waiting",
        )
        .bind(blocker)
        .fetch_one(pool)
        .await
        .unwrap();
        waiting >= count
    })
    .await;
    assert!(
        queued,
        "{count} real handlers must reach the locked request before the cancellation is released"
    );
}

/// A deleted or suspended community answers 404 to likes, unlikes and invite
/// acceptance, and none of them writes a row.
#[tokio::test]
async fn closed_communities_refuse_likes_and_invite_acceptance() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    for closing in [
        "UPDATE social.communities SET active = FALSE WHERE id = $1",
        "UPDATE social.communities SET suspended = TRUE, suspended_at = now(), suspended_by = 'op' WHERE id = $1",
    ] {
        let (owner, member, invitee) = (wallet(), wallet(), wallet());
        let community = rig.community(&owner, false).await;
        rig.member(community, &member, "member").await;
        let post = rig.post(community, &owner).await;
        let request = rig.request(community, &invitee, "invite").await;
        let like = format!("/v1/communities/{community}/posts/{post}/like");
        sqlx::query(
            "INSERT INTO social.community_post_likes (post_id, user_address) VALUES ($1, $2)",
        )
        .bind(post)
        .bind(&member.address)
        .execute(&rig.pool)
        .await
        .unwrap();
        sqlx::query(closing)
            .bind(community)
            .execute(&rig.pool)
            .await
            .unwrap();

        let liked = hit(&rig.app, Case::new("post", &like).signed(&owner).await).await;
        assert_eq!(liked.0, StatusCode::NOT_FOUND, "{closing}: like");
        let unliked = hit(&rig.app, Case::new("delete", &like).signed(&member).await).await;
        assert_eq!(unliked.0, StatusCode::NOT_FOUND, "{closing}: unlike");
        let path = format!("/v1/communities/{community}/requests/{request}");
        assert_eq!(
            decide(&rig.app, &invitee, &path, "accepted").await,
            StatusCode::NOT_FOUND,
            "{closing}: accept"
        );

        let likes: Vec<String> = sqlx::query_scalar(
            "SELECT user_address FROM social.community_post_likes WHERE post_id = $1",
        )
        .bind(post)
        .fetch_all(&rig.pool)
        .await
        .unwrap();
        assert_eq!(likes, vec![member.address.to_string()], "{closing}");
        assert_eq!(rig.role_of(community, &invitee).await, None, "{closing}");
    }
}

/// A deleted or suspended community refuses new requests, including an invite
/// that would meet a pending join request, and writes nothing.
#[tokio::test]
async fn closed_communities_refuse_new_requests_and_auto_accept() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    for closing in [
        "UPDATE social.communities SET active = FALSE WHERE id = $1",
        "UPDATE social.communities SET suspended = TRUE, suspended_at = now(), suspended_by = 'op' WHERE id = $1",
    ] {
        let (owner, joiner, invitee) = (wallet(), wallet(), wallet());
        let community = rig.community(&owner, true).await;
        let pending = rig.request(community, &joiner, "request_to_join").await;
        sqlx::query(closing)
            .bind(community)
            .execute(&rig.pool)
            .await
            .unwrap();

        let path = format!("/v1/communities/{community}/requests");
        for target in [&joiner, &invitee] {
            let invited = hit(
                &rig.app,
                Case::new("post", &path)
                    .signed(&owner)
                    .await
                    .json(&json!({"type": "invite", "targetedAddress": target.address.as_str()})),
            )
            .await;
            assert_eq!(invited.0, StatusCode::NOT_FOUND, "{closing}: {}", invited.1);
        }

        let requests: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, status FROM social.community_requests WHERE community_id = $1",
        )
        .bind(community)
        .fetch_all(&rig.pool)
        .await
        .unwrap();
        assert_eq!(
            requests,
            vec![(pending, "pending".to_string())],
            "{closing}"
        );
        assert_eq!(rig.role_of(community, &joiner).await, None, "{closing}");
    }
}
