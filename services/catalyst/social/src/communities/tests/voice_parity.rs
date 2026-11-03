//! Executed source: source-contracts/community-voice.json (social b6b2cef,
//! companion gatekeeper be6a4b9, Node 26). The companion pin describes this
//! composition; it does not establish every deployed gatekeeper's behavior.
use super::*;
use crate::comms::gatekeeper::voice::db::VoiceDbConfig;

struct ConfiguredVoice(VoiceDbConfig);

impl super::super::VoiceEviction for ConfiguredVoice {
    fn presence_config(&self) -> VoiceDbConfig {
        self.0
    }
    fn evict<'a>(
        &'a self,
        _: CommunityId,
        _: &'a str,
    ) -> futures::future::BoxFuture<'a, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn configured_comms_presence_drives_detail_lists_requests_and_active_handler() {
    let _voice = VOICE_ROWS.lock().await;
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    rig.state
        .set_voice_eviction(Arc::new(ConfiguredVoice(VoiceDbConfig {
            connection_interrupted_ttl_ms: 60_000,
            initial_connection_ttl_ms: 90_000,
            ..Default::default()
        })));
    let (owner, viewer) = (wallet(), wallet());
    let live = rig.community(&owner, false).await;
    let stale = rig.community(&owner, false).await;
    let search = live.as_uuid().simple().to_string()[..12].to_owned();
    for (id, suffix) in [(live, "live"), (stale, "stale")] {
        sqlx::query("UPDATE social.communities SET name=$2 WHERE id=$1")
            .bind(id)
            .bind(format!("Voice {search} {suffix}"))
            .execute(&rig.pool)
            .await
            .unwrap();
    }
    rig.member(live, &viewer, "member").await;
    rig.request(live, &viewer, "invite").await;
    for (id, status, moderator, old) in [
        (live, "connection_interrupted", true, false),
        (live, "not_connected", false, false),
        (live, "connected", false, true),
        (live, "connection_interrupted", false, true),
        (live, "not_connected", false, true),
        (stale, "connection_interrupted", true, true),
        (stale, "connected", false, false),
    ] {
        let participant = wallet();
        rig.voice(id, &participant, status, moderator).await;
        sqlx::query("UPDATE social.community_voice_chat_users SET joined_at=timezone('UTC',now())-$3::text::interval,status_updated_at=timezone('UTC',now())-$3::text::interval WHERE community_id=$1 AND address=$2")
            .bind(id).bind(&participant.address).bind(if old {"120 seconds"} else {"1 second"}).execute(&rig.pool).await.unwrap();
    }
    let expected = json!({"isActive":true,"participantCount":3,"moderatorCount":1});
    for version in ["v1", "v2"] {
        let path = format!("/{version}/communities/{live}");
        let (status, body) = hit(&rig.app, Case::new("get", path).signed(&viewer).await).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["voiceChatStatus"], expected);
        for suffix in ["", "&onlyWithActiveVoiceChat=true"] {
            let (status, body) = hit(
                &rig.app,
                Case::new(
                    "get",
                    format!("/{version}/communities?search={search}{suffix}"),
                )
                .signed(&viewer)
                .await,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let rows = body["data"]["results"].as_array().unwrap();
            assert_eq!(
                rows.iter().find(|r| r["id"] == live.to_string()).unwrap()["voiceChatStatus"],
                expected
            );
            if !suffix.is_empty() {
                assert!(!rows.iter().any(|r| r["id"] == stale.to_string()));
            } else {
                assert_eq!(
                    rows.iter().find(|r| r["id"] == stale.to_string()).unwrap()["voiceChatStatus"],
                    json!({"isActive":false,"participantCount":1,"moderatorCount":0})
                );
            }
        }
        let (status, body) = hit(
            &rig.app,
            Case::new(
                "get",
                format!("/{version}/members/{}/requests?type=invite", viewer.address),
            )
            .signed(&viewer)
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["results"][0]["voiceChatStatus"], expected);
    }
    let (_, body) = hit(
        &rig.app,
        Case::new("get", format!("/v2/communities/{stale}")),
    )
    .await;
    assert_eq!(
        body["data"]["voiceChatStatus"],
        json!({"isActive":false,"participantCount":0,"moderatorCount":0})
    );
    let (status, body) = hit(
        &rig.app,
        Case::new("get", "/v1/community-voice-chats/active")
            .signed(&viewer)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let chats = body["data"]["activeChats"].as_array().unwrap();
    let chat = chats
        .iter()
        .find(|r| r["communityId"] == live.to_string())
        .unwrap();
    assert_eq!(chat["participantCount"], 3);
    assert_eq!(chat["moderatorCount"], 1);
    assert!(chat.get("communityImage").is_none());
    assert!(!chats.iter().any(|r| r["communityId"] == stale.to_string()));
    sqlx::query("DELETE FROM social.community_voice_chat_users WHERE community_id=ANY($1)")
        .bind(vec![live.as_uuid(), stale.as_uuid()])
        .execute(&rig.pool)
        .await
        .unwrap();
    rig.finish();
}

/// A separate real-schema database lets permission faults exercise handlers
/// without altering another test's tables or assuming the process has no rooms.
#[tokio::test]
async fn active_handler_preserves_empty_error_and_all_settled_source_contracts() {
    let Some(url) = crate::db::scratch::url().await else {
        return;
    };
    let options: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone().database("postgres"))
        .await
        .unwrap();
    let suffix = Uuid::new_v4().simple().to_string();
    let database = format!("catalyst_voice_{}_{suffix}", std::process::id());
    let role = format!("voice_{suffix}");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect_with(options.clone().database(&database))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    // The CI database uses SCRAM authentication; local trust must not hide
    // a fixture role without a usable credential. Generated for this test only.
    let password = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE {role} LOGIN PASSWORD '{password}'"
    )))
    .execute(&pool)
    .await
    .unwrap();
    for statement in [
        format!("GRANT USAGE ON SCHEMA social TO {role}"),
        format!("GRANT SELECT ON social.community_voice_chat_users TO {role}"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .unwrap();
    }
    let limited = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with(
            options
                .database(&database)
                .username(&role)
                .password(&password),
        )
        .await
        .unwrap();
    let storage_dir = std::env::temp_dir().join(format!("catalyst-voice-{suffix}"));
    let storage = Arc::new(
        crate::storage::Storage::from_config(&storage_dir)
            .await
            .unwrap(),
    );
    let state = Arc::new(State::new(
        limited.clone(),
        storage,
        Settings::for_tests(),
        None,
        &crate::http::Outbound::for_tests(),
        State::explore_for_tests(&limited),
    ));
    let app = router(state);
    let viewer = wallet();
    let request = || Case::new("get", "/v1/community-voice-chats/active");
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-voice.json"
    )))
    .unwrap();
    let source = |name: &str| {
        fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .unwrap()["result"]
            .clone()
    };
    let (status, body) = hit(&app, request().signed(&viewer).await).await;
    assert_eq!(
        status.as_u16(),
        source("empty")["status"].as_u64().unwrap() as u16
    );
    assert_eq!(body, source("empty")["body"]);
    let member = CommunityId::parse("00000000-0000-4000-8000-000000000001").unwrap();
    let public = CommunityId::parse("00000000-0000-4000-8000-000000000002").unwrap();
    for (id, name, is_member) in [
        (member, "Member community", true),
        (public, "Public community", false),
    ] {
        sqlx::query("INSERT INTO social.communities(id,name,description,owner_address,private) VALUES($1,$2,'fixture',$3,false)")
            .bind(id).bind(name).bind(&viewer.address).execute(&pool).await.unwrap();
        if is_member {
            sqlx::query("INSERT INTO social.community_members(community_id,member_address,role) VALUES($1,$2,'member')").bind(id).bind(&viewer.address).execute(&pool).await.unwrap();
        }
        for i in 0..if is_member { 2 } else { 3 } {
            sqlx::query("INSERT INTO social.community_voice_chat_users(community_id,address,is_moderator,status) VALUES($1,$2,$3,'connected')").bind(id).bind(wallet().address).bind(i==0).execute(&pool).await.unwrap();
        }
    }
    let (status, body) = hit(&app, request().signed(&viewer).await).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        json!({"message":"permission denied for table communities"})
    );
    for version in ["v1", "v2"] {
        let (status, body) = hit(
            &app,
            Case::new("get", format!("/{version}/communities/{member}")),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body,
            json!({"message":"permission denied for table communities"})
        );
    }
    for table in ["communities", "community_members", "community_bans"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT SELECT ON social.{table} TO {role}"
        )))
        .execute(&pool)
        .await
        .unwrap();
    }
    // Both enrichment tables are denied. allSettled retains the member;
    // the nonmember has no destinations and is omitted.
    let (status, body) = hit(&app, request().signed(&viewer).await).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, source("nonmember-places-error")["body"]);
    // A stored thumbnail whose blob is absent is also an omitted key.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT SELECT ON social.community_thumbnails TO {role}"
    )))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO social.community_thumbnails(community_id,thumbnail_hash) VALUES($1,$2)",
    )
    .bind(member)
    .bind("a".repeat(64))
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = hit(&app, request().signed(&viewer).await).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, source("nonmember-places-error")["body"]);
    limited.close().await;
    let (status, body) = hit(&app, request().signed(&viewer).await).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, source("gatekeeper-error")["body"]);
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {database} WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE {role}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let _ = std::fs::remove_dir_all(storage_dir);
}
