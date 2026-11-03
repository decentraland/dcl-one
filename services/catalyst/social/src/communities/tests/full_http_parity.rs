//! Source contracts: decentraland/social-service-ea b6b2cefefdf727b2ed47ddb887e77d226a41eea6,
//! controllers/routes/http.routes.ts, handlers/http/schemas.ts and logic/community/*.
use super::*;

#[tokio::test]
async fn dapps_variants_and_sdk_boolean_overrides_drive_community_permissions() {
    let _flags = DAPPS_FLAGS.lock().await;
    let mut settings = Settings::for_tests();
    settings
        .features
        .overrides
        .insert(super::super::features::AI_COMPLIANCE.into(), "true".into());
    settings.features.overrides.insert(
        super::super::features::REFERRAL_REGISTRATION_DISABLED.into(),
        "1".into(),
    );
    let Some(rig) = Rig::new(settings).await else {
        return;
    };
    let (owner, moderator) = (wallet(), wallet());
    let id = rig.community(&owner, false).await;
    let doc = json!({
        "flags": {"dapps-communities_global_moderators":true, "dapps-communities_ai_compliance":true},
        "variants": {"dapps-communities_global_moderators": {
            "payload": {"value": format!("\u{feff}{},\r\n", moderator.address.to_string().to_uppercase())}
        }}
    });
    crate::explore::api::feature_flags::store(&rig.pool, "dapps", &doc, None)
        .await
        .unwrap();
    rig.state.load_flags().await.unwrap();
    assert!(rig.state.is_global_moderator(&moderator.address));
    assert!(!rig.state.is_global_moderator(&owner.address));
    assert!(
        !rig.state
            .feature_enabled(super::super::features::AI_COMPLIANCE),
        "the SDK accepts literal 1, not true"
    );
    assert!(
        rig.state
            .feature_enabled(super::super::features::REFERRAL_REGISTRATION_DISABLED)
    );
    assert_eq!(
        hit(
            &rig.app,
            Case::new("get", "/v1/moderation/communities")
                .signed(&moderator)
                .await
        )
        .await
        .0,
        StatusCode::OK
    );
    let path = format!("/v1/communities/{id}");
    let (status, body) = hit(
        &rig.app,
        Case::new("patch", &path)
            .signed(&moderator)
            .await
            .json(&json!({"editorsChoice":true})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let disabled = json!({"flags":{}, "variants":doc["variants"]});
    crate::explore::api::feature_flags::store(&rig.pool, "dapps", &disabled, None)
        .await
        .unwrap();
    rig.state.load_flags().await.unwrap();
    assert!(
        !rig.state.is_global_moderator(&moderator.address),
        "disabled document hides its variant"
    );
    assert_eq!(
        hit(&rig.app, Case::new("delete", path).signed(&moderator).await)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("DELETE FROM explore_api.feature_flags WHERE app='dapps'")
        .execute(&rig.pool)
        .await
        .unwrap();
    rig.finish();
}

fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[tokio::test]
async fn joins_consume_requests_and_member_exits_remove_only_their_community_likes() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let community = rig.community(&owner, false).await;
    let other = rig.community(&owner, false).await;
    let post = rig.post(community, &owner).await;
    let other_post = rig.post(other, &owner).await;
    rig.request(community, &member, "invite").await;
    let join = format!("/v1/communities/{community}/members");
    let (status, body) = hit(&rig.app, Case::new("post", &join).signed(&member).await).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM social.community_requests WHERE community_id=$1 AND member_address=$2")
        .bind(community).bind(&member.address).fetch_one(&rig.pool).await.unwrap();
    assert_eq!(pending, 0);
    for (actor, method, suffix) in [
        (&member, "delete", ""),
        (&owner, "delete", ""),
        (&owner, "post", "/bans"),
    ] {
        for post in [post, other_post] {
            sqlx::query("INSERT INTO social.community_post_likes (post_id, user_address) VALUES ($1,$2) ON CONFLICT DO NOTHING")
                .bind(post).bind(&member.address).execute(&rig.pool).await.unwrap();
        }
        let path = format!(
            "/v1/communities/{community}/members/{}{suffix}",
            member.address
        );
        let (status, body) = hit(&rig.app, Case::new(method, path).signed(actor).await).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
        let likes: Vec<Uuid> = sqlx::query_scalar("SELECT post_id FROM social.community_post_likes WHERE user_address=$1 ORDER BY post_id")
            .bind(&member.address).fetch_all(&rig.pool).await.unwrap();
        assert_eq!(likes, vec![other_post]);
        if suffix == "/bans" {
            break;
        }
        rig.member(community, &member, "member").await;
    }
    rig.finish();
}

#[tokio::test]
async fn community_json_schemas_run_before_domain_authorization() {
    let Some(rig) = Rig::new(admin_settings(None)).await else {
        return;
    };
    let actor = wallet();
    let id = CommunityId::new();
    let request = Uuid::new_v4();
    let cases = [
        (
            "patch",
            format!("/v1/communities/{id}"),
            json!({"editorsChoice":true,"extra":1}),
            "additionalProperties",
        ),
        (
            "patch",
            format!("/v1/communities/{id}/members/{}", actor.address),
            json!({"role":"none"}),
            "enum",
        ),
        (
            "post",
            format!("/v1/communities/{id}/requests"),
            json!({"type":"request_to_join"}),
            "required",
        ),
        (
            "patch",
            format!("/v1/communities/{id}/requests/{request}"),
            json!({"intention":"ACCEPTED"}),
            "enum",
        ),
        (
            "post",
            format!("/v1/communities/{id}/places"),
            json!({"placeIds":vec!["a";101]}),
            "maxItems",
        ),
        (
            "post",
            format!("/v1/communities/{id}/places"),
            json!({"placeIds":["a".repeat(257)]}),
            "maxLength",
        ),
        (
            "post",
            format!("/v1/communities/{id}/posts"),
            json!({"content":" leading"}),
            "pattern",
        ),
    ];
    for (method, path, body, keyword) in cases {
        let (status, value) = hit(
            &rig.app,
            Case::new(method, &path).signed(&actor).await.json(&body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {path}: {value}");
        assert_eq!(value["ok"], false, "{value}");
        assert_eq!(value["message"], "Invalid JSON body");
        assert_eq!(value["data"][0]["keyword"], keyword, "{value}");
        let (status, value) = hit(&rig.app, Case::new(method, path).json(&body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["error"], "Invalid Auth Chain");
    }
    let (status, value) = hit(
        &rig.app,
        Case::new("post", format!("/v1/members/{}/communities", actor.address))
            .bearer(ADMIN_TOKEN)
            .json(&json!({"communityIds":["invalid"]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["data"][0]["instancePath"], "/communityIds/0");
    assert_eq!(
        value["data"][0]["schemaPath"],
        "#/properties/communityIds/items/pattern"
    );
    let (_, value) = hit(
        &rig.app,
        Case::new("post", format!("/v1/communities/{id}/places"))
            .signed(&actor)
            .await
            .body(
                br#"{"placeIds":[],"z":1,"a":2}"#.to_vec(),
                "application/json",
            ),
    )
    .await;
    assert_eq!(value["data"][0]["params"]["additionalProperty"], "z");
    rig.finish();
}

#[tokio::test]
async fn source_dtos_have_nullable_voice_and_base_write_replies() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    rig.profile(&owner, "Named owner", true).await;
    let (bytes, content_type) = multipart(&[
        ("name", b"Wire community"),
        ("description", b"description"),
        ("thumbnail", &png(1024)),
    ]);
    let (status, created) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(
        keys(&created["data"]),
        vec![
            "active",
            "description",
            "id",
            "isHostingLiveEvent",
            "name",
            "ownerAddress",
            "ownerName",
            "privacy",
            "thumbnails",
            "visibility"
        ]
    );
    assert_eq!(created["data"]["ownerName"], "Named owner");
    let id = created["data"]["id"].as_str().unwrap();
    let (status, detail) = hit(&rig.app, Case::new("get", format!("/v2/communities/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        keys(&detail["data"]),
        vec![
            "active",
            "description",
            "id",
            "membersCount",
            "name",
            "ownerAddress",
            "privacy",
            "voiceChatStatus"
        ]
    );
    assert_eq!(
        detail["data"]["voiceChatStatus"],
        json!({"isActive":false,"participantCount":0,"moderatorCount":0})
    );
    let (bytes, content_type) = multipart(&[("description", b"changed")]);
    let (status, updated) = hit(
        &rig.app,
        Case::new("put", format!("/v1/communities/{id}"))
            .signed(&owner)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        keys(&updated["data"]),
        vec![
            "active",
            "description",
            "id",
            "name",
            "ownerAddress",
            "privacy",
            "visibility"
        ]
    );
    let (status, post) = hit(
        &rig.app,
        Case::new("post", format!("/v1/communities/{id}/posts"))
            .signed(&owner)
            .await
            .json(&json!({"content":"message"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{post}");
    assert_eq!(
        keys(&post["data"]),
        vec![
            "authorAddress",
            "authorHasClaimedName",
            "authorName",
            "authorProfilePictureUrl",
            "communityId",
            "content",
            "createdAt",
            "id"
        ]
    );
    assert_eq!(post["data"]["authorName"], "Named owner");
    rig.finish();
}

#[tokio::test]
async fn self_authored_claimed_name_is_not_name_ownership() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, target) = (wallet(), wallet());
    rig.profile(&target, "Claimed but not owned", true).await;
    rig.state.chain_index.get().unwrap().graph.names.clear();
    let (bytes, content_type) =
        multipart(&[("name", b"Claim spoof"), ("description", b"description")]);
    let (status, _) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&target)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let community = rig.community(&owner, false).await;
    rig.member(community, &target, "member").await;
    let path = format!("/v1/communities/{community}/members/{}", target.address);
    let (status, _) = hit(
        &rig.app,
        Case::new("patch", &path)
            .signed(&owner)
            .await
            .json(&json!({"role":"owner"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    rig.name_owner(&target, "Owned despite unclaimed profile");
    let (status, value) = hit(
        &rig.app,
        Case::new("patch", &path)
            .signed(&owner)
            .await
            .json(&json!({"role":"owner"})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{value}");
    rig.finish();
}

#[tokio::test]
async fn http_membership_and_deletion_writes_publish_connectivity_after_commit() {
    use crate::rpc::{
        context::{Context, VoiceSettings},
        db::Db,
        profiles::Profiles,
        proto::v2::ConnectivityStatus,
        pubsub::SocialEvent,
        voice::fake::FakeVoiceRooms,
    };
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let community = rig.community(&owner, false).await;
    let context = Context::new(
        VoiceSettings {
            private_voice_chat_expiration_ms: 60_000,
            private_voice_chat_expiration_batch_size: 20,
        },
        Db::new(rig.pool.clone()),
        Profiles::new(None),
        Arc::new(FakeVoiceRooms::new(rig.pool.clone())),
    );
    rig.state.set_updates(context.pubsub().clone());
    let mut self_events = context.pubsub().subscribe(member.address.as_str());
    let mut owner_events = context.pubsub().subscribe(owner.address.as_str());
    let check = |event: Arc<SocialEvent>, status: ConnectivityStatus| {
        let SocialEvent::CommunityMemberStatus(update) = event.as_ref() else {
            panic!("membership event expected")
        };
        assert_eq!(update.community_id, community.to_string());
        assert_eq!(update.status, status as i32);
        assert_eq!(
            update.member.as_ref().unwrap().address,
            member.address.as_str()
        );
    };
    let join = format!("/v1/communities/{community}/members");
    assert_eq!(
        hit(&rig.app, Case::new("post", &join).signed(&member).await)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(rig.role_of(community, &member).await.is_some());
    check(self_events.try_recv().unwrap(), ConnectivityStatus::Online);
    check(owner_events.try_recv().unwrap(), ConnectivityStatus::Online);
    assert_eq!(
        hit(&rig.app, Case::new("post", &join).signed(&member).await)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        self_events.try_recv().is_err(),
        "idempotent join publishes no duplicate"
    );
    let leave = format!("{join}/{}", member.address);
    assert_eq!(
        hit(&rig.app, Case::new("delete", leave).signed(&member).await)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(rig.role_of(community, &member).await.is_none());
    check(self_events.try_recv().unwrap(), ConnectivityStatus::Offline);
    check(
        owner_events.try_recv().unwrap(),
        ConnectivityStatus::Offline,
    );
    let private = rig.community(&owner, true).await;
    let request = rig.request(private, &member, "invite").await;
    let decide = format!("/v1/communities/{private}/requests/{request}");
    assert_eq!(
        hit(
            &rig.app,
            Case::new("patch", decide)
                .signed(&member)
                .await
                .json(&json!({"intention":"accepted"}))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        matches!(self_events.try_recv().unwrap().as_ref(),SocialEvent::CommunityMemberStatus(update) if update.status == ConnectivityStatus::Online as i32 && update.community_id == private.to_string())
    );
    let delete = format!("/v1/communities/{private}");
    assert_eq!(
        hit(&rig.app, Case::new("delete", delete).signed(&owner).await)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        matches!(self_events.try_recv().unwrap().as_ref(),SocialEvent::CommunityMemberStatus(update) if update.status == ConnectivityStatus::Offline as i32 && update.member.as_ref().unwrap().address == member.address.as_str())
    );
    rig.finish();
}

#[tokio::test]
async fn form_parser_limits_and_field_precedence_match_the_native_upstream_parser() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    rig.profile(&owner, "Owner", true).await;
    let bad_form = json!({ "error":"Bad request", "message":"Invalid multipart/form-data body" });
    let (status, value) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(b"invalid".to_vec(), "application/octet-stream"),
    )
    .await;
    assert_eq!((status, value), (StatusCode::BAD_REQUEST, bad_form));
    let (status, value) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(
                b"name=First&name=Last&description=ok".to_vec(),
                "application/x-www-form-urlencoded",
            ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    assert_eq!(value["data"]["name"], "Last");
    let (bytes, content_type) = multipart(&[
        ("name", b""),
        ("description", b"ok"),
        ("privacy", b"INVALID"),
    ]);
    let (status, value) = hit(
        &rig.app,
        Case::new("put", format!("/v1/communities/{}", CommunityId::new()))
            .signed(&owner)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value,
        json!({"error":"Bad request","message":"Name must be a non-empty string"})
    );
    let (bytes, content_type) = multipart(&[
        ("name", b"No image"),
        ("description", b"ok"),
        ("thumbnail", b""),
    ]);
    let (status, value) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["message"],
        "Thumbnail size must be between 1KB and 500KB"
    );
    let image = png(500 * 1024);
    let padding = vec![b'x'; 64 * 1024];
    let (bytes, content_type) = multipart(&[
        ("name", b"Big valid image"),
        ("description", b"ok"),
        ("thumbnail", &image),
        ("unused", &padding),
    ]);
    let (status, value) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "valid multipart above 512KiB: {value}"
    );
    let too_large = vec![b'x'; 1024 * 1024 + 1];
    let (status, value) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(too_large.clone(), "multipart/form-data; boundary=x"),
    )
    .await;
    assert_eq!(
        (status, value),
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"Bad request","message":"Request body too large"})
        )
    );
    let (_, value) = hit(
        &rig.app,
        Case::new("post", "/v1/communities").body(too_large, "multipart/form-data; boundary=x"),
    )
    .await;
    assert_eq!(
        value["error"], "Invalid Auth Chain",
        "authentication precedes size refusal"
    );
    rig.finish();
}
