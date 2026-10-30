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
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        rig.role_of(community, &member).await,
        None,
        "an acknowledged retry must not grant membership again"
    );
    assert_eq!(
        decide(&rig.app, &member, &path, "rejected").await,
        StatusCode::CONFLICT
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
            StatusCode::NO_CONTENT
        );
        let acceptor = if kind == "invite" {
            &member
        } else {
            &moderator
        };
        assert_eq!(
            decide(&rig.app, acceptor, &path, "accepted").await,
            StatusCode::CONFLICT,
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
                (StatusCode::NO_CONTENT, StatusCode::CONFLICT)
                    | (StatusCode::CONFLICT, StatusCode::NO_CONTENT)
            ),
            "accept={accept}, reject={reject}"
        );
        let persisted: String =
            sqlx::query_scalar("SELECT status FROM social.community_requests WHERE id = $1")
                .bind(request)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        assert_eq!(
            persisted,
            if accept == StatusCode::NO_CONTENT {
                "accepted"
            } else {
                "rejected"
            }
        );
        assert_eq!(
            rig.role_of(community, &member).await.is_some(),
            persisted == "accepted"
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
    assert_eq!(accepted.await.unwrap().0, StatusCode::CONFLICT);
    assert_eq!(rig.role_of(community, &member).await, None);
    let status: String =
        sqlx::query_scalar("SELECT status FROM social.community_requests WHERE id=$1")
            .bind(request)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
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
