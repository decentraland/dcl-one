//! Shared registry reads propagate through the six profile-enriched handlers;
//! cache writes stay best effort and the six V2 routes do not read profiles.
use super::*;

#[tokio::test]
async fn community_registry_read_errors_propagate_but_cache_write_errors_do_not() {
    let source:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-registry-errors.json"))).unwrap();
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, requester, banned) = (wallet(), wallet(), wallet());
    let id = rig.community(&owner, false).await;
    for profile in [&requester, &banned] {
        rig.profile(profile, "Owner", true).await;
    }
    let search = id.as_uuid().simple().to_string();
    sqlx::query("UPDATE social.communities SET name=$2 WHERE id=$1")
        .bind(id)
        .bind(&search)
        .execute(&rig.pool)
        .await
        .unwrap();
    rig.request(id, &requester, "request_to_join").await;
    rig.post(id, &owner).await;
    sqlx::query("INSERT INTO social.community_bans(community_id,banned_address,banned_by,active) VALUES($1,$2,$3,true)").bind(id).bind(&banned.address).bind(&owner.address).execute(&rig.pool).await.unwrap();
    let role = format!("registry_errors_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE ROLE {role}")))
        .execute(&rig.pool)
        .await
        .unwrap();
    let schemas:Vec<String>=sqlx::query_scalar("SELECT nspname FROM pg_namespace WHERE nspname NOT LIKE 'pg_%' AND nspname<>'information_schema'").fetch_all(&rig.pool).await.unwrap();
    for schema in schemas {
        assert!(
            schema
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("GRANT USAGE ON SCHEMA {schema} TO {role}; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA {schema} TO {role}; GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA {schema} TO {role}"))).execute(&rig.pool).await.unwrap();
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "REVOKE SELECT ON social.registry_profile_cache FROM {role}"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    let connect_role = role.clone();
    // Connect with the original test credential, then SET ROLE. No LOGIN
    // role without a password and no assumption of local trust in CI.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let role = connect_role.clone();
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
        Settings::for_tests(),
        None,
        &crate::http::Outbound::for_tests(),
        rig.state.explore.clone(),
    ));
    state.pass_markers.insert(None, None);
    let app = Router::new().nest(
        "/social-api",
        crate::social_api_gateway(router(state.clone())),
    );
    let mut write_mode = false;
    let mut compared = 0;
    for case in source["cases"].as_array().unwrap() {
        compared += 1;
        if case["phase"] == "write-error" && !write_mode {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("GRANT SELECT ON social.registry_profile_cache TO {role}; REVOKE INSERT,UPDATE ON social.registry_profile_cache FROM {role}"))).execute(&rig.pool).await.unwrap();
            write_mode = true;
        }
        let source_path = case["path"].as_str().unwrap();
        let member_request = source_path.contains("/members/:address/requests");
        let path = source_path
            .replace(":id", &id.to_string())
            .replace(":address", requester.address.as_str());
        let path = format!("/social-api{path}?search={search}&type=request_to_join");
        let (status, body) = hit(
            &app,
            Case::new("get", path)
                .signed(if member_request { &requester } else { &owner })
                .await,
        )
        .await;
        assert_eq!(
            status.as_u16(),
            case["response"]["status"].as_u64().unwrap() as u16,
            "{} {source_path}: {body}",
            case["phase"]
        );
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            assert_eq!(
                body, case["response"]["body"],
                "cache read error must retain the source message-only boundary"
            );
            continue;
        }
        let posts = source_path.ends_with("/posts");
        let key = if posts { "posts" } else { "results" };
        let row = body["data"][key]
            .as_array()
            .unwrap()
            .first()
            .expect("the core query must still return the seeded row");
        let profile_field = if posts {
            "authorName"
        } else if source_path.ends_with("/communities") || member_request {
            "ownerName"
        } else {
            "name"
        };
        assert_eq!(
            row.get(profile_field),
            case["response"]["body"]["data"][key][0].get(profile_field),
            "profile value/presence for {source_path}"
        );
    }
    assert_eq!(compared, 18);
    let addresses = vec![
        owner.address.to_string(),
        requester.address.to_string(),
        banned.address.to_string(),
    ];
    let cached: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.registry_profile_cache WHERE profile_key=ANY($1)",
    )
    .bind(addresses)
    .fetch_one(&rig.pool)
    .await
    .unwrap();
    assert_eq!(
        cached, 0,
        "successful enrichment must not require successful cache writes"
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
