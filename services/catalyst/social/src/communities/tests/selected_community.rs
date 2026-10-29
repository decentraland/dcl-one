use super::*;

#[tokio::test]
async fn selected_discovery_is_keyed_and_preserves_private_and_unlisted_access() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, outsider) = (wallet(), wallet());
    let community = rig.community(&owner, true).await;
    sqlx::query("UPDATE social.communities SET name='ZZ selected community' WHERE id=$1")
        .bind(community)
        .execute(&rig.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO social.communities(id,name,owner_address) SELECT gen_random_uuid(), '000 earlier ' || n, $1 FROM generate_series(1,105) n")
        .bind(&owner.address).execute(&rig.pool).await.unwrap();
    let (_, first) = hit(
        &rig.app,
        Case::new("get", "/v1/communities?limit=100")
            .signed(&outsider)
            .await,
    )
    .await;
    assert!(
        !first["data"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == community.to_string())
    );
    for version in ["v1", "v2"] {
        let path = format!("/{version}/communities?communityId={community}&limit=1");
        let (status, found) = hit(&rig.app, Case::new("get", &path).signed(&outsider).await).await;
        assert_eq!(status, StatusCode::OK, "{found}");
        assert_eq!(found["data"]["total"], 1);
        assert_eq!(found["data"]["results"][0]["id"], community.to_string());
        assert_eq!(found["data"]["results"][0]["role"], "none");
        let (_, anonymous) = hit(&rig.app, Case::new("get", &path)).await;
        assert_eq!(anonymous["data"]["total"], 0);
        let (status, _) = hit(
            &rig.app,
            Case::new("get", format!("{path}&minimal=true"))
                .signed(&outsider)
                .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = hit(
        &rig.app,
        Case::new("get", format!("/v1/communities/{community}/posts"))
            .signed(&outsider)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    sqlx::query("UPDATE social.communities SET unlisted=true WHERE id=$1")
        .bind(community)
        .execute(&rig.pool)
        .await
        .unwrap();
    let path = format!("/v1/communities?communityId={community}&limit=1");
    let (_, hidden) = hit(&rig.app, Case::new("get", &path).signed(&outsider).await).await;
    assert_eq!(hidden["data"]["total"], 0);
    let (_, owned) = hit(
        &rig.app,
        Case::new("get", format!("{path}&onlyMemberOf=true"))
            .signed(&owner)
            .await,
    )
    .await;
    assert_eq!(owned["data"]["results"][0]["id"], community.to_string());
    let (status, _) = hit(
        &rig.app,
        Case::new("get", "/v1/communities?communityId=invalid")
            .signed(&outsider)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
