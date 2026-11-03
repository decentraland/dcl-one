//! All37 source handlers catch dependency failures locally. Exercise the real
//! routes and SQL permission errors without changing any shared relation.
use super::*;

#[tokio::test]
async fn every_community_http_database_failure_uses_the_source_message_boundary() {
    let actor = wallet();
    let target = wallet();
    let mut settings = Settings::for_tests();
    settings.admin_token = Some(ADMIN_TOKEN.into());
    settings.global_moderators = vec![actor.address.clone()];
    let Some(rig) = Rig::new(settings.clone()).await else {
        return;
    };
    let id = rig.community(&actor, true).await;
    rig.member(id, &target, "member").await;
    let post = rig.post(id, &actor).await;
    let request = rig.request(id, &target, "request_to_join").await;
    let role = format!("community_errors_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE ROLE {role}")))
        .execute(&rig.pool)
        .await
        .unwrap();
    let schemas:Vec<String>=sqlx::query_scalar("SELECT nspname FROM pg_namespace WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema'").fetch_all(&rig.pool).await.unwrap();
    for schema in schemas {
        assert!(
            schema
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("GRANT USAGE ON SCHEMA {schema} TO {role}; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA {schema} TO {role}; GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA {schema} TO {role}"))).execute(&rig.pool).await.unwrap();
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "REVOKE ALL ON social.communities FROM {role}"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    let role_on_connect = role.clone();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let role = role_on_connect.clone();
            Box::pin(async move {
                sqlx::query(sqlx::AssertSqlSafe(format!("SET ROLE {role}")))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&crate::db::scratch::url().await.unwrap())
        .await
        .unwrap();
    let state = Arc::new(State::new(
        pool.clone(),
        rig.state.storage.clone(),
        settings,
        None,
        &crate::http::Outbound::for_tests(),
        rig.state.explore.clone(),
    ));
    state.set_chain_index(rig.state.chain_index.get().unwrap().clone());
    let legacy = router(state.clone());
    let api = Router::new().nest("/social-api", crate::social_api_gateway(router(state)));
    let fixture:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-errors.json"))).unwrap();
    // The source ADR-44 boundary precedes any domain dependency failure.
    // These requests reach a real role denied the community table, so a 400
    // proves the gateway does not perform mirror SQL before authentication.
    for (method, path) in [
        ("patch", format!("/social-api/v1/communities/{id}")),
        ("post", format!("/social-api/v1/communities/{id}/requests")),
        (
            "delete",
            format!("/social-api/v1/communities/{id}/posts/{post}"),
        ),
    ] {
        let (status, body) = hit(&api, Case::new(method, path).json(&json!({}))).await;
        assert_gate_400(status, &body, "gateway auth before mirror SQL failure");
    }
    let before:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM social.communities WHERE id=$1),(SELECT count(*) FROM social.community_members WHERE community_id=$1),(SELECT count(*) FROM social.community_requests WHERE community_id=$1),(SELECT count(*) FROM social.community_posts WHERE community_id=$1)").bind(id).fetch_one(&rig.pool).await.unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let method = match case["method"].as_str().unwrap() {
            "GET" => "get",
            "POST" => "post",
            "PUT" => "put",
            "PATCH" => "patch",
            "DELETE" => "delete",
            other => panic!("unknown method {other}"),
        };
        let source_path = case["path"].as_str().unwrap();
        let address =
            if source_path.ends_with("/invites") || source_path.ends_with("/members/:address") {
                target.address.as_str()
            } else {
                actor.address.as_str()
            };
        let path = source_path
            .replace(":memberAddress", target.address.as_str())
            .replace(":address", address)
            .replace(":postId", &post.to_string())
            .replace(":requestId", &request.to_string())
            .replace(":placeId", &Uuid::new_v4().to_string())
            .replace(":id", &id.to_string());
        let mut call = Case::new(method, format!("/social-api{path}"))
            .signed(&actor)
            .await
            .bearer(ADMIN_TOKEN);
        call = match (method, source_path) {
            ("post", "/v1/communities") => call.body(
                b"name=Fixture%20community&description=fixture%20description".to_vec(),
                "application/x-www-form-urlencoded",
            ),
            ("put", _) => call.body(
                b"description=fixture%20description".to_vec(),
                "application/x-www-form-urlencoded",
            ),
            ("post", "/v1/members/:address/communities") => {
                call.json(&json!({"communityIds":[id.to_string()]}))
            }
            ("patch", "/v1/communities/:id/members/:address") => {
                call.json(&json!({"role":"moderator"}))
            }
            ("patch", "/v1/communities/:id") => call.json(&json!({"editorsChoice":true})),
            ("post", "/v1/communities/:id/places") => {
                call.json(&json!({"placeIds":[Uuid::new_v4().to_string()]}))
            }
            ("post", "/v1/communities/:id/posts") => call.json(&json!({"content":"Fixture post"})),
            ("post", "/v1/communities/:id/requests") => call
                .json(&json!({"type":"request_to_join","targetedAddress":actor.address.as_str()})),
            ("patch", "/v1/communities/:id/requests/:requestId") => {
                call.json(&json!({"intention":"accepted"}))
            }
            _ => call,
        };
        let (status, body) = hit(&api, call).await;
        assert_eq!(
            status.as_u16(),
            case["response"]["status"].as_u64().unwrap() as u16,
            "{method} {path}: {body}"
        );
        assert_eq!(body, case["response"]["body"], "{method} {path}");
    }
    let (status, body) = hit(
        &legacy,
        Case::new("patch", format!("/v1/communities/{id}"))
            .signed(&actor)
            .await
            .json(&json!({"editorsChoice":true})),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        json!({"error":"database error","message":"database error"}),
        "the historical mirror guard is outside the gateway catch"
    );
    // The existence read succeeds; the moderation witness's paired membership
    // read now fails. Keep its database message through the typed authority.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("GRANT ALL ON social.communities TO {role}; REVOKE SELECT ON social.community_members FROM {role}"))).execute(&rig.pool).await.unwrap();
    for method in ["post", "delete"] {
        let (status, body) = hit(
            &api,
            Case::new(
                method,
                format!(
                    "/social-api/v1/communities/{id}/members/{}/bans",
                    target.address
                ),
            )
            .signed(&actor)
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
        assert_eq!(
            body,
            json!({"message":"permission denied for table community_members"})
        );
    }
    let requester = wallet();
    let request_call = || {
        Case::new("post", format!("/social-api/v1/communities/{id}/requests"))
            .json(&json!({"type":"request_to_join","targetedAddress":requester.address.as_str()}))
    };
    let (status, body) = hit(&api, request_call().signed(&requester).await).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        json!({"message":"permission denied for table community_members"}),
        "auth, schema and mirror lookup pass before this handler membership read"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("GRANT SELECT ON social.community_members TO {role}; REVOKE INSERT ON social.community_requests FROM {role}"))).execute(&rig.pool).await.unwrap();
    let (status, body) = hit(&api, request_call().signed(&requester).await).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        json!({"message":"permission denied for table community_requests"}),
        "request insert fails after all preflight reads succeed"
    );
    let after:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM social.communities WHERE id=$1),(SELECT count(*) FROM social.community_members WHERE community_id=$1),(SELECT count(*) FROM social.community_requests WHERE community_id=$1),(SELECT count(*) FROM social.community_posts WHERE community_id=$1)").bind(id).fetch_one(&rig.pool).await.unwrap();
    assert_eq!(
        after, before,
        "dependency failures must leave these domain rows unchanged"
    );
    pool.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP OWNED BY {role}; DROP ROLE {role}"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    rig.finish();
}
