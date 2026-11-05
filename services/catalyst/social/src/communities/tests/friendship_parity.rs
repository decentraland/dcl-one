//! Executed source: community-friendship-errors.json, Social b6b2cef, Node26.
use super::*;

fn source() -> Value {
    serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-friendship-errors.json"))).unwrap()
}

#[tokio::test]
async fn friendship_sql_errors_empty_pages_anonymous_and_request_status_match_source() {
    let _permissions = DATABASE_PERMISSIONS.lock().await;
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, peer, banned) = (wallet(), wallet(), wallet());
    let id = rig.community(&owner, false).await;
    let search = id.as_uuid().simple().to_string();
    sqlx::query("UPDATE social.communities SET name=$2 WHERE id=$1")
        .bind(id)
        .bind(&search)
        .execute(&rig.pool)
        .await
        .unwrap();
    rig.member(id, &peer, "member").await;
    rig.profile(&peer, "Profile", true).await;
    rig.profile(&banned, "Profile", true).await;
    rig.friends(&owner, &peer).await;
    rig.friends(&owner, &banned).await;
    rig.request(id, &peer, "request_to_join").await;
    rig.request(id, &owner, "request_to_join").await;
    sqlx::query("INSERT INTO social.community_bans(community_id,banned_address,banned_by,active) VALUES($1,$2,$3,true)").bind(id).bind(&banned.address).bind(&owner.address).execute(&rig.pool).await.unwrap();
    let role = format!("friend_errors_{}", Uuid::new_v4().simple());
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
        "REVOKE SELECT ON social.friendships FROM {role}"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    let connect_role = role.clone();
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
    let app = Router::new().nest("/social-api", crate::social_api_gateway(router(state)));
    let fixture = source();
    let mut compared = 0;
    for phase in [
        "friendship-read-error",
        "accepted-friend",
        "empty-page-error",
        "anonymous-error",
        "friend-and-profile-error",
    ] {
        if phase == "accepted-friend" {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "GRANT SELECT ON social.friendships TO {role}"
            )))
            .execute(&rig.pool)
            .await
            .unwrap();
        } else if phase == "empty-page-error" {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "REVOKE SELECT ON social.friendships FROM {role}"
            )))
            .execute(&rig.pool)
            .await
            .unwrap();
        } else if phase == "friend-and-profile-error" {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "REVOKE SELECT ON social.registry_profile_cache FROM {role}"
            )))
            .execute(&rig.pool)
            .await
            .unwrap();
        }
        for case in fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["phase"] == phase)
        {
            compared += 1;
            let source_path = case["path"].as_str().unwrap();
            let path = source_path
                .replace(":id", &id.to_string())
                .replace(":address", owner.address.as_str());
            let offset = if phase == "empty-page-error" { 100 } else { 0 };
            let path =
                format!("/social-api{path}?search={search}&type=request_to_join&offset={offset}");
            let request = Case::new("get", path);
            let request = if phase == "anonymous-error" {
                request
            } else {
                request.signed(&owner).await
            };
            let (status, body) = hit(&app, request).await;
            assert_eq!(
                status.as_u16(),
                case["response"]["status"].as_u64().unwrap() as u16,
                "{phase} {source_path}: {body}"
            );
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                assert_eq!(body, case["response"]["body"], "{phase} {source_path}");
                continue;
            }
            let rows = body["data"]["results"].as_array().unwrap();
            if phase == "empty-page-error" {
                assert!(rows.is_empty(), "{source_path}: {body}");
                continue;
            }
            assert!(!rows.is_empty(), "{source_path}: {body}");
            if source_path.ends_with("/communities")
                || source_path.contains("/members/:address/requests")
            {
                if phase == "accepted-friend" {
                    let friends = rows[0]["friends"].as_array().unwrap();
                    assert_eq!(friends.len(), 1, "{source_path}: {body}");
                    let address = if source_path.starts_with("/v1/") {
                        &friends[0]["address"]
                    } else {
                        &friends[0]
                    };
                    assert_eq!(address, peer.address.as_str(), "{source_path}");
                }
            } else {
                let target = if source_path.ends_with("/bans") {
                    &banned
                } else {
                    &peer
                };
                let row = rows
                    .iter()
                    .find(|r| r["memberAddress"] == target.address.as_str())
                    .unwrap();
                assert_eq!(
                    row["friendshipStatus"],
                    case["response"]["body"]["data"]["results"][0]["friendshipStatus"],
                    "{phase} {source_path}"
                );
            }
        }
    }
    assert_eq!(compared, 37);
    pool.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP OWNED BY {role}; DROP ROLE {role}"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    rig.finish();
}

#[tokio::test]
async fn v1_member_ban_and_request_pages_omit_missing_profiles_without_changing_totals() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, known, missing) = (wallet(), wallet(), wallet());
    let id = rig.community(&owner, false).await;
    rig.profile(&known, "Profile", true).await;
    rig.member(id, &missing, "member").await;
    rig.request(id, &owner, "request_to_join").await;
    rig.request(id, &missing, "request_to_join").await;
    for wallet in [&known, &missing] {
        sqlx::query("INSERT INTO social.community_bans(community_id,banned_address,banned_by,active) VALUES($1,$2,$3,true)").bind(id).bind(&wallet.address).bind(&owner.address).execute(&rig.pool).await.unwrap();
    }
    let fixture = source();
    let mut compared = 0;
    for case in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["phase"] == "missing-profile")
    {
        compared += 1;
        let source_path = case["path"].as_str().unwrap();
        let path = source_path.replace(":id", &id.to_string());
        let (status, body) = hit(&rig.app, Case::new("get", path).signed(&owner).await).await;
        assert_eq!(status, StatusCode::OK, "{source_path}: {body}");
        let data = &body["data"];
        let expected = &case["response"]["body"]["data"];
        let rows = data["results"].as_array().unwrap();
        assert_eq!(
            rows.len(),
            expected["results"].as_array().unwrap().len(),
            "{source_path}: {body}"
        );
        for field in ["total", "page", "pages", "limit"] {
            assert_eq!(data[field], expected[field], "{source_path} {field}");
        }
        let missing_row = rows
            .iter()
            .find(|row| row["memberAddress"] == missing.address.as_str());
        assert_eq!(
            missing_row.is_some(),
            source_path.starts_with("/v2/"),
            "{source_path}"
        );
        if source_path.starts_with("/v2/") {
            assert!(
                rows.iter()
                    .all(|row| row.get("profilePictureUrl").is_none()),
                "{source_path}"
            );
        }
    }
    assert_eq!(compared, 6);
    rig.finish();
}
