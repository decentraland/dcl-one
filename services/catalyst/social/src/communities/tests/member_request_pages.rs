//! Source uses real PostgreSQL for pending-page, visibility and permission facts.
use super::*;

fn replace_seed_facts(value: &mut Value, replacements: &std::collections::HashMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(replacement) = replacements.get(text) {
                *text = replacement.clone();
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_seed_facts(value, replacements);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_seed_facts(value, replacements);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn raw_request_pages_keep_source_visibility_order_totals_and_friendship_failure_boundary() {
    let _permissions = DATABASE_PERMISSIONS.lock().await;
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let fixture:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/member-request-pages.json"))).unwrap();
    let (owner, viewer) = (wallet(), wallet());
    let mut replacements = std::collections::HashMap::new();
    replacements.insert(
        fixture["seed"]["owner"].as_str().unwrap().to_string(),
        owner.address.to_string(),
    );
    replacements.insert(
        fixture["seed"]["viewer"].as_str().unwrap().to_string(),
        viewer.address.to_string(),
    );
    for index in 0..4 {
        let id = rig.community(&owner, index == 3).await;
        sqlx::query("UPDATE social.communities SET name=$2,description='Description',unlisted=$3,active=$4,ranking_score=$5 WHERE id=$1").bind(id).bind(["Zulu visible","Inactive","Banning","Alpha private"][index]).bind(index==3).bind(index!=1).bind(if index==3 {100.0f32} else {0.0}).execute(&rig.pool).await.unwrap();
        let request = rig.request(id, &viewer, "request_to_join").await;
        sqlx::query(
            "UPDATE social.community_requests SET created_at=$2::text::timestamp WHERE id=$1",
        )
        .bind(request)
        .bind(
            [
                "2024-01-01 03:00:00",
                "2024-01-01 05:00:00",
                "2024-01-01 04:00:00",
                "2024-01-01 02:00:00",
            ][index],
        )
        .execute(&rig.pool)
        .await
        .unwrap();
        replacements.insert(
            fixture["seed"]["communityIds"][index]
                .as_str()
                .unwrap()
                .to_string(),
            id.to_string(),
        );
        replacements.insert(
            fixture["seed"]["requestIds"][index]
                .as_str()
                .unwrap()
                .to_string(),
            request.to_string(),
        );
        if index == 2 {
            sqlx::query("INSERT INTO social.community_bans(community_id,banned_address,banned_by,active) VALUES($1,$2,$3,true)").bind(id).bind(&viewer.address).bind(&owner.address).execute(&rig.pool).await.unwrap();
        }
    }
    let role = format!("request_pages_{}", Uuid::new_v4().simple());
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
    let app = Router::new().nest("/social-api", crate::social_api_gateway(router(state)));
    let mut phase = "healthy";
    let mut compared = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let next_phase = case["phase"].as_str().unwrap();
        if next_phase != phase {
            let statement = if next_phase == "friendship-permission-error" {
                format!("REVOKE SELECT ON social.friendships FROM {role}")
            } else {
                format!(
                    "GRANT SELECT ON social.friendships TO {role}; REVOKE SELECT ON social.communities FROM {role}"
                )
            };
            sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                .execute(&rig.pool)
                .await
                .unwrap();
            phase = next_phase;
        }
        let version = case["version"].as_str().unwrap();
        let limit = case["limit"].as_i64().unwrap();
        let offset = case["offset"].as_i64().unwrap();
        let path = format!(
            "/social-api/{version}/members/{}/requests?limit={limit}&offset={offset}",
            viewer.address
        );
        let (status, body) = hit(&app, Case::new("get", path).signed(&viewer).await).await;
        assert_eq!(
            status.as_u16(),
            case["response"]["status"].as_u64().unwrap() as u16,
            "{} {version} limit{limit} offset{offset}: {body}",
            case["phase"]
        );
        let mut expected = case["response"]["body"].clone();
        replace_seed_facts(&mut expected, &replacements);
        assert_eq!(
            body, expected,
            "{} {version} limit{limit} offset{offset}",
            case["phase"]
        );
        compared += 1;
    }
    assert_eq!(compared, 36);
    pool.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP OWNED BY {role}; DROP ROLE {role}"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    rig.finish();
}
