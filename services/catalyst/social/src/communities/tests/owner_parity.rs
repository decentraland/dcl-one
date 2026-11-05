//! Both strict owner callsites replay the actual pinned registry/component/
//! HTTP handler composition, including historical pointer-to-userId aliases.
use super::*;

#[tokio::test]
async fn detail_and_create_use_strict_owner_profiles_without_changing_the_actor() {
    let source:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-owners.json"))).unwrap();
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    for case in source["cases"].as_array().unwrap() {
        rig.profile(&owner, "Source profile", true).await;
        let id = rig.community(&owner, false).await;
        sqlx::query("UPDATE social.communities SET name='Fixture community' WHERE id=$1")
            .bind(id)
            .execute(&rig.pool)
            .await
            .unwrap();
        let documents = case["documents"].as_array().unwrap();
        if let Some(document) = documents.first() {
            let document: Value = serde_json::from_str(
                &document
                    .to_string()
                    .replace(source["owner"].as_str().unwrap(), owner.address.as_str()),
            )
            .unwrap();
            sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=(SELECT entity_id FROM content.active_pointers WHERE pointer=$1)")
                .bind(&owner.address).bind(json!({"v":document})).execute(&rig.pool).await.unwrap();
        } else {
            sqlx::query("DELETE FROM content.active_pointers WHERE pointer=$1")
                .bind(&owner.address)
                .execute(&rig.pool)
                .await
                .unwrap();
        }
        rig.state.profiles.clear().await;
        rig.name_owner(&owner, "Fixture");
        let normalize = |value: &Value| -> Value {
            serde_json::from_str(
                &value
                    .to_string()
                    .replace(source["owner"].as_str().unwrap(), owner.address.as_str()),
            )
            .unwrap()
        };
        let (status, body) = hit(&rig.app, Case::new("get", format!("/v1/communities/{id}"))).await;
        let mut expected = normalize(&case["get"]["body"]);
        if status == StatusCode::OK {
            expected["data"]["id"] = json!(id.as_uuid());
        }
        assert_eq!(
            status.as_u16(),
            case["get"]["status"].as_u64().unwrap() as u16,
            "{}: {body}",
            case["name"]
        );
        assert_eq!(body, expected, "{} detail", case["name"]);
        let before: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.communities WHERE owner_address=$1")
                .bind(&owner.address)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("name", "Fixture created community")
            .append_pair("description", "fixture description")
            .finish();
        let (status, body) = hit(
            &rig.app,
            Case::new("post", "/v1/communities")
                .signed(&owner)
                .await
                .body(form.into_bytes(), "application/x-www-form-urlencoded"),
        )
        .await;
        let mut expected = normalize(&case["create"]["body"]);
        if status == StatusCode::CREATED {
            expected["data"]["id"] = body["data"]["id"].clone();
        }
        assert_eq!(
            status.as_u16(),
            case["create"]["status"].as_u64().unwrap() as u16,
            "{}: {body}",
            case["name"]
        );
        assert_eq!(body, expected, "{} create", case["name"]);
        let after: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.communities WHERE owner_address=$1")
                .bind(&owner.address)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        assert_eq!(
            after - before,
            if status == StatusCode::CREATED { 1 } else { 0 }
        );
    }
    rig.finish();
}
