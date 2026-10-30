use super::*;

async fn edit(app: &Router, actor: &Wallet, path: &str, content: &str) -> StatusCode {
    hit(
        app,
        Case::new("patch", path)
            .signed(actor)
            .await
            .json(&json!({"content": content})),
    )
    .await
    .0
}

async fn saved(rig: &Rig, post: Uuid) -> Value {
    sqlx::query_scalar("SELECT to_jsonb(p) FROM social.community_posts p WHERE id = $1")
        .bind(post)
        .fetch_one(&rig.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn edits_keep_identity_time_reactions_and_unrelated_posts() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, author, reader) = (wallet(), wallet(), wallet());
    let community = rig.community(&owner, true).await;
    rig.member(community, &author, "moderator").await;
    rig.member(community, &reader, "member").await;
    let post = rig.post(community, &author).await;
    let other = rig.post(community, &owner).await;
    let path = format!("/v1/communities/{community}/posts/{post}");
    assert_eq!(
        hit(
            &rig.app,
            Case::new("post", format!("{path}/like"))
                .signed(&reader)
                .await
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let mut expected = saved(&rig, post).await;
    let untouched = saved(&rig, other).await;
    assert_eq!(
        edit(&rig.app, &author, &path, "  revised text  ").await,
        StatusCode::NO_CONTENT
    );
    expected["content"] = json!("revised text");
    assert_eq!(saved(&rig, post).await, expected);
    assert_eq!(saved(&rig, other).await, untouched);
    let (_, feed) = hit(
        &rig.app,
        Case::new("get", format!("/v1/communities/{community}/posts"))
            .signed(&reader)
            .await,
    )
    .await;
    let row = feed["data"]["posts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == post.to_string())
        .unwrap();
    assert_eq!(row["content"], "revised text");
    assert_eq!(row["likesCount"], 1);
    assert_eq!(row["isLikedByUser"], true);
    assert_eq!(
        edit(
            &rig.app,
            &owner,
            &format!("/social-api/v1/communities/{community}/posts/{other}"),
            "owner edit"
        )
        .await,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn edits_require_the_signed_current_author_and_valid_content() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, author, other) = (wallet(), wallet(), wallet());
    let community = rig.community(&owner, true).await;
    rig.member(community, &author, "moderator").await;
    rig.member(community, &other, "moderator").await;
    let post = rig.post(community, &author).await;
    let path = format!("/v1/communities/{community}/posts/{post}");
    let before = saved(&rig, post).await;
    for actor in [&owner, &other] {
        assert_eq!(
            edit(&rig.app, actor, &path, "not my words").await,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, body) = hit(
        &rig.app,
        Case::new("patch", &path).json(&json!({"content":"unsigned"})),
    )
    .await;
    assert_gate_400(status, &body, "post edit unsigned");
    let (status, body) = hit(
        &rig.app,
        Case::new("patch", &path)
            .forged(&author)
            .await
            .json(&json!({"content":"forged"})),
    )
    .await;
    assert_forged_401(status, &body, "post edit forged");
    for body in [json!({}), json!({"content":42}), json!({"content":" \n "})] {
        assert_eq!(
            hit(
                &rig.app,
                Case::new("patch", &path).signed(&author).await.json(&body)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let other_community = rig.community(&author, false).await;
    assert_eq!(
        edit(
            &rig.app,
            &author,
            &format!("/v1/communities/{other_community}/posts/{post}"),
            "wrong scope"
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(saved(&rig, post).await, before);
    for role in ["member"] {
        sqlx::query("UPDATE social.community_members SET role = $3 WHERE community_id = $1 AND member_address = $2")
            .bind(community)
            .bind(&author.address)
            .bind(role)
            .execute(&rig.pool)
            .await
            .unwrap();
        assert_eq!(
            edit(&rig.app, &author, &path, "stale authority").await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(saved(&rig, post).await, before);
    }
}

#[tokio::test]
async fn inactive_removed_and_banned_authors_cannot_edit() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    for action in ["remove", "ban", "deactivate"] {
        let (owner, author) = (wallet(), wallet());
        let community = rig.community(&owner, true).await;
        rig.member(community, &author, "moderator").await;
        let post = rig.post(community, &author).await;
        let before = saved(&rig, post).await;
        if action == "deactivate" {
            sqlx::query("UPDATE social.communities SET active = FALSE WHERE id = $1")
                .bind(community)
                .execute(&rig.pool)
                .await
                .unwrap();
        } else {
            let suffix = if action == "ban" { "/bans" } else { "" };
            let method = if action == "ban" { "post" } else { "delete" };
            assert_eq!(
                hit(
                    &rig.app,
                    Case::new(
                        method,
                        format!(
                            "/v1/communities/{community}/members/{}{suffix}",
                            author.address
                        )
                    )
                    .signed(&owner)
                    .await
                )
                .await
                .0,
                StatusCode::NO_CONTENT
            );
        }
        assert_eq!(
            edit(
                &rig.app,
                &author,
                &format!("/v1/communities/{community}/posts/{post}"),
                "stale"
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(saved(&rig, post).await, before);
    }
}

#[tokio::test]
async fn an_edit_waiting_behind_demotion_checks_the_committed_role() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, author) = (wallet(), wallet());
    let community = rig.community(&owner, true).await;
    rig.member(community, &author, "moderator").await;
    let post = rig.post(community, &author).await;
    let before = saved(&rig, post).await;
    let mut demotion = rig.pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *demotion)
        .await
        .unwrap();
    sqlx::query("UPDATE social.community_members SET role = 'member' WHERE community_id = $1 AND member_address = $2")
        .bind(community)
        .bind(&author.address)
        .execute(&mut *demotion)
        .await
        .unwrap();
    let app = rig.app.clone();
    let case = Case::new("patch", format!("/v1/communities/{community}/posts/{post}"))
        .signed(&author)
        .await
        .json(&json!({"content":"stale edit"}));
    let editing = tokio::spawn(async move { hit(&app, case).await });
    assert!(
        crate::shared::test_wait::until(async || {
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))")
                .bind(blocker)
                .fetch_one(&rig.pool)
                .await
                .unwrap()
        })
        .await,
        "edit must reach the real membership lock before demotion commits"
    );
    demotion.commit().await.unwrap();
    assert_eq!(editing.await.unwrap().0, StatusCode::UNAUTHORIZED);
    assert_eq!(saved(&rig, post).await, before);
}
