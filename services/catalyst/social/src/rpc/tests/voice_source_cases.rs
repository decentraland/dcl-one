//! Wire differential against the executable upstream capture, with PostgreSQL-backed
//! membership/call intents and the same injected voice outcomes as the capture.
use super::*;
use crate::comms::gatekeeper::voice::db::{PrivateVoiceChatRow, StartOutcome, VoiceDbConfig};
use crate::comms::gatekeeper::voice::rooms::{CommunityVoiceAction, VoiceRooms, VoiceUnavailable};
use serde_json::Value;
use std::collections::HashMap;

struct FixtureVoice {
    inner: FakeVoiceRooms,
    missing: bool,
}
impl FixtureVoice {
    fn outcome(&self) -> Result<(), VoiceUnavailable> {
        if self.missing {
            Err(VoiceUnavailable::NotFound)
        } else {
            Ok(())
        }
    }
}
#[async_trait::async_trait]
impl VoiceRooms for FixtureVoice {
    fn presence_config(&self) -> VoiceDbConfig {
        self.inner.presence_config()
    }
    async fn create_pending_private_call(
        &self,
        a: &str,
        b: &str,
        ttl: i64,
    ) -> Result<StartOutcome, VoiceUnavailable> {
        self.inner.create_pending_private_call(a, b, ttl).await
    }
    async fn finish_pending_private_call(
        &self,
        call: PendingPrivateCall,
    ) -> Result<Option<PrivateVoiceChatRow>, VoiceUnavailable> {
        self.inner.finish_pending_private_call(call).await
    }
    async fn expire_pending_private_calls(
        &self,
        ttl: i64,
        limit: i64,
    ) -> Result<Vec<PrivateVoiceChatRow>, VoiceUnavailable> {
        self.inner.expire_pending_private_calls(ttl, limit).await
    }
    async fn private_voice_credentials(
        &self,
        id: Uuid,
        callee: &str,
        caller: &str,
    ) -> Result<HashMap<String, String>, VoiceUnavailable> {
        self.inner
            .private_voice_credentials(id, callee, caller)
            .await
    }
    async fn end_private_voice_chat(
        &self,
        id: Option<Uuid>,
        address: &str,
    ) -> Result<Option<PrivateVoiceChatRow>, VoiceUnavailable> {
        self.inner.end_private_voice_chat(id, address).await
    }
    async fn community_voice_chat_active(&self, id: Uuid) -> Result<bool, VoiceUnavailable> {
        self.inner.community_voice_chat_active(id).await
    }
    async fn community_voice_credentials(
        &self,
        id: Uuid,
        address: &str,
        role: &str,
        action: CommunityVoiceAction,
        profile: Option<Value>,
    ) -> Result<String, VoiceUnavailable> {
        self.outcome()?;
        self.inner
            .community_voice_credentials(id, address, role, action, profile)
            .await
    }
    async fn end_community_voice_chat(
        &self,
        id: Uuid,
        address: &str,
    ) -> Result<(), VoiceUnavailable> {
        self.outcome()?;
        self.inner.end_community_voice_chat(id, address).await
    }
    async fn request_to_speak(
        &self,
        id: Uuid,
        address: &str,
        raised: bool,
    ) -> Result<(), VoiceUnavailable> {
        self.outcome()?;
        self.inner.request_to_speak(id, address, raised).await
    }
    async fn reject_speak_request(&self, id: Uuid, address: &str) -> Result<(), VoiceUnavailable> {
        self.outcome()?;
        self.inner.reject_speak_request(id, address).await
    }
    async fn set_speaker(
        &self,
        id: Uuid,
        address: &str,
        promote: bool,
    ) -> Result<(), VoiceUnavailable> {
        self.outcome()?;
        self.inner.set_speaker(id, address, promote).await
    }
    async fn kick_player(&self, id: Uuid, address: &str) -> Result<(), VoiceUnavailable> {
        self.outcome()?;
        self.inner.kick_player(id, address).await
    }
    async fn mute_speaker(
        &self,
        id: Uuid,
        address: &str,
        muted: bool,
    ) -> Result<(), VoiceUnavailable> {
        self.outcome()?;
        self.inner.mute_speaker(id, address, muted).await
    }
}

fn fixture() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/voice-source-cases.json"
    )))
    .unwrap()
}

#[tokio::test]
async fn voice_rpc_wire_answers_match_executed_upstream_handlers() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let fixture = fixture();
    let a = fixture["caller"].as_str().unwrap();
    let b = fixture["target"].as_str().unwrap();
    let community: Uuid = fixture["community"].as_str().unwrap().parse().unwrap();
    let call: Uuid = fixture["call"].as_str().unwrap().parse().unwrap();
    for case in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["group"] == "rpc")
    {
        let id = case["id"].as_str().unwrap();
        let method = case["method"].as_str().unwrap();
        let scenario = case["scenario"].as_str().unwrap();
        let caller = case["caller"].as_str().unwrap();
        let request = &case["request"];
        drop_community(&pool, community).await;
        sqlx::query("DELETE FROM social.private_voice_chats WHERE caller_address=ANY($1) OR callee_address=ANY($1)")
            .bind(vec![a,b]).execute(&pool).await.unwrap();
        cleanup_pair(&Db::new(pool.clone()), a, b).await;
        let working_pool = if scenario == "database-failure" {
            sqlx::postgres::PgPoolOptions::new()
                .acquire_timeout(Duration::from_millis(20))
                .connect_lazy("postgres://postgres@127.0.0.1:1/unreachable")
                .unwrap()
        } else {
            pool.clone()
        };
        let voice = Arc::new(FixtureVoice {
            inner: FakeVoiceRooms::new(working_pool.clone()),
            missing: scenario == "runtime-not-found",
        });
        if method.contains("Community") {
            sqlx::query(
                "INSERT INTO social.communities (id,name,owner_address) VALUES ($1,'Community',$2)",
            )
            .bind(community)
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();
            for (address, role) in [(a, "owner"), (b, "member")] {
                if scenario == "forbidden" && address == a {
                    continue;
                }
                sqlx::query("INSERT INTO social.community_members(community_id,member_address,role) VALUES($1,$2,$3)")
                    .bind(community).bind(address).bind(role).execute(&pool).await.unwrap();
            }
            if method != "startCommunityVoiceChat" || scenario == "conflict" {
                voice.inner.live.lock().insert(community);
            }
        } else if scenario == "success" && method != "startPrivateVoiceChat" {
            let (from, to) = if method == "rejectPrivateVoiceChat"
                || method == "getIncomingPrivateVoiceChatRequests"
            {
                (b, a)
            } else {
                (a, b)
            };
            sqlx::query("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,$3)")
                .bind(call).bind(from).bind(to).execute(&pool).await.unwrap();
        }
        let runtime: Arc<dyn VoiceRooms> = if scenario == "runtime-failure" {
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime)
        } else {
            voice
        };
        let ctx = test_ctx(Db::new(working_pool), runtime);
        let pctx = procedure_ctx(&ctx, caller, 6001);
        let cid = request["communityId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let target = request["userAddress"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let call_id = request["callId"].as_str().unwrap_or_default().to_string();
        let bytes = match method {
            "startCommunityVoiceChat" => SocialServiceImpl
                .start_community_voice_chat(
                    StartCommunityVoiceChatPayload { community_id: cid },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "joinCommunityVoiceChat" => SocialServiceImpl
                .join_community_voice_chat(
                    JoinCommunityVoiceChatPayload { community_id: cid },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "requestToSpeakInCommunityVoiceChat" => SocialServiceImpl
                .request_to_speak_in_community_voice_chat(
                    RequestToSpeakInCommunityVoiceChatPayload {
                        community_id: cid,
                        is_raising_hand: request["isRaisingHand"].as_bool().unwrap_or_default(),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "promoteSpeakerInCommunityVoiceChat" => SocialServiceImpl
                .promote_speaker_in_community_voice_chat(
                    PromoteSpeakerInCommunityVoiceChatPayload {
                        community_id: cid,
                        user_address: target,
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "demoteSpeakerInCommunityVoiceChat" => SocialServiceImpl
                .demote_speaker_in_community_voice_chat(
                    DemoteSpeakerInCommunityVoiceChatPayload {
                        community_id: cid,
                        user_address: target,
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "kickPlayerFromCommunityVoiceChat" => SocialServiceImpl
                .kick_player_from_community_voice_chat(
                    KickPlayerFromCommunityVoiceChatPayload {
                        community_id: cid,
                        user_address: target,
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "rejectSpeakRequestInCommunityVoiceChat" => SocialServiceImpl
                .reject_speak_request_in_community_voice_chat(
                    RejectSpeakRequestInCommunityVoiceChatPayload {
                        community_id: cid,
                        user_address: target,
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "endCommunityVoiceChat" => SocialServiceImpl
                .end_community_voice_chat(EndCommunityVoiceChatPayload { community_id: cid }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "muteSpeakerFromCommunityVoiceChat" => SocialServiceImpl
                .mute_speaker_from_community_voice_chat(
                    MuteSpeakerFromCommunityVoiceChatPayload {
                        community_id: cid,
                        user_address: target,
                        muted: request["muted"].as_bool().unwrap_or_default(),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "startPrivateVoiceChat" => {
                let mut response = SocialServiceImpl
                    .start_private_voice_chat(
                        StartPrivateVoiceChatPayload {
                            callee: request["callee"]["address"]
                                .as_str()
                                .map(|v| User { address: v.into() }),
                        },
                        pctx,
                    )
                    .await
                    .unwrap();
                if let Some(start_private_voice_chat_response::Response::Ok(ok)) =
                    response.response.as_mut()
                {
                    Uuid::parse_str(&ok.call_id).unwrap();
                    ok.call_id = call.to_string(); // Opaque generated identity, normalized only after validation.
                }
                response.encode_to_vec()
            }
            "acceptPrivateVoiceChat" => SocialServiceImpl
                .accept_private_voice_chat(AcceptPrivateVoiceChatPayload { call_id }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "rejectPrivateVoiceChat" => SocialServiceImpl
                .reject_private_voice_chat(RejectPrivateVoiceChatPayload { call_id }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "endPrivateVoiceChat" => SocialServiceImpl
                .end_private_voice_chat(EndPrivateVoiceChatPayload { call_id }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "getIncomingPrivateVoiceChatRequests" => SocialServiceImpl
                .get_incoming_private_voice_chat_request(pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            other => panic!("unhandled captured method {other}"),
        };
        assert_eq!(
            hex::encode(bytes),
            case["expected_hex"].as_str().unwrap(),
            "{id}"
        );
    }
    drop_community(&pool, community).await;
    sqlx::query("DELETE FROM social.private_voice_chats WHERE caller_address=ANY($1) OR callee_address=ANY($1)")
        .bind(vec![a,b]).execute(&pool).await.unwrap();
}

#[tokio::test]
async fn all_six_rpc_generators_and_json_mappers_match_executed_source_parsers() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let source = fixture();
    let me = source["caller"].as_str().unwrap();
    let actor = source["target"].as_str().unwrap();
    let entity = seed_profile(&pool, actor, "Stream Person").await;
    let metadata = serde_json::json!({"v":{"avatars":[{"userId":actor,"name":"Stream Person","hasClaimedName":true,"avatar":{"snapshots":{"face256":"https://profiles.example/face.png"}}}]}});
    sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=$1")
        .bind(&entity)
        .bind(metadata)
        .execute(&pool)
        .await
        .unwrap();
    let ctx = test_ctx(
        Db::new(pool.clone()),
        Arc::new(FakeVoiceRooms::new(pool.clone())),
    );
    ctx.db()
        .apply_friendship_action(me, actor, "accept", true, None, None)
        .await
        .unwrap();
    ctx.topology()
        .receive(&pulse_frame(vec![me.into(), actor.into()]))
        .unwrap();
    macro_rules! check {
        ($case:expr,$ty:ty,$method:ident,$variant:ident) => {{
            let case = $case;
            let expected = <$ty>::decode(
                hex::decode(case["expected_hex"].as_str().unwrap())
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
            let event = SocialEvent::$variant(expected.clone());
            let token = tokio_util::sync::CancellationToken::new();
            let mut generator = SocialServiceImpl
                .$method(procedure_ctx(&ctx, me, 7001), token.clone())
                .await
                .unwrap();
            ctx.pubsub().publish(me, event.clone());
            let actual = next_update(&mut generator).await;
            assert_eq!(
                hex::encode(actual.encode_to_vec()),
                case["expected_hex"].as_str().unwrap(),
                "{}",
                case["id"]
            );
            assert_eq!(
                crate::rpc::stream::captured_event_data(
                    &ctx,
                    me,
                    case["id"].as_str().unwrap(),
                    &event
                )
                .await
                .unwrap(),
                case["json"],
                "JSON {}",
                case["id"]
            );
            token.cancel();
        }};
    }
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["group"] == "stream")
    {
        match case["id"].as_str().unwrap() {
            "friendship" => check!(
                case,
                FriendshipUpdate,
                subscribe_to_friendship_updates,
                Friendship
            ),
            "friend-connectivity" => check!(
                case,
                FriendConnectivityUpdate,
                subscribe_to_friend_connectivity_updates,
                FriendConnectivity
            ),
            "block" => check!(case, BlockUpdate, subscribe_to_block_updates, Block),
            "private-voice" => check!(
                case,
                PrivateVoiceChatUpdate,
                subscribe_to_private_voice_chat_updates,
                PrivateVoice
            ),
            "community-member" => check!(
                case,
                CommunityMemberConnectivityUpdate,
                subscribe_to_community_member_connectivity_updates,
                CommunityMemberStatus
            ),
            "community-voice" => check!(
                case,
                CommunityVoiceChatUpdate,
                subscribe_to_community_voice_chat_updates,
                CommunityVoice
            ),
            other => panic!("unhandled stream {other}"),
        }
    }
    drop_profile(&pool, actor, &entity).await;
    cleanup_pair(ctx.db(), me, actor).await;
}

#[tokio::test]
async fn missing_profile_source_sequence_keeps_json_request_and_later_rpc_cancellation() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let source = fixture();
    let missing = source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["group"] == "stream-missing-profile")
        .unwrap();
    let me = missing["caller"].as_str().unwrap();
    let actor = missing["target"].as_str().unwrap();
    let barrier_profile = seed_profile(&pool, me, "Barrier").await;
    let ctx = test_ctx(
        Db::new(pool.clone()),
        Arc::new(FakeVoiceRooms::new(pool.clone())),
    );
    let token = tokio_util::sync::CancellationToken::new();
    let mut stream = SocialServiceImpl
        .subscribe_to_friendship_updates(procedure_ctx(&ctx, me, 7101), token.clone())
        .await
        .unwrap();
    let request = FriendshipUpdate::decode(
        hex::decode(missing["request_hex"].as_str().unwrap())
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    let raw = SocialEvent::Friendship(request);
    assert_eq!(
        crate::rpc::stream::captured_event_data(&ctx, me, "friendship", &raw)
            .await
            .unwrap(),
        missing["json"]
    );
    ctx.pubsub().publish(me, raw);
    let cancel = |address: &str| FriendshipUpdate {
        stream_closed: None,
        update: Some(friendship_update::Update::Cancel(
            friendship_update::CancelResponse {
                user: Some(User {
                    address: address.into(),
                }),
            },
        )),
    };
    ctx.pubsub()
        .publish(me, SocialEvent::Friendship(cancel(me)));
    // A known-profile barrier proves the earlier missing request has been processed.
    assert_eq!(next_update(&mut stream).await, cancel(me));
    let actor_profile = seed_profile(&pool, actor, "Available now").await;
    ctx.pubsub()
        .publish(me, SocialEvent::Friendship(cancel(actor)));
    let actual = next_update(&mut stream).await;
    assert_eq!(
        hex::encode(actual.encode_to_vec()),
        missing["expected_hex"].as_str().unwrap()
    );
    token.cancel();
    drop_profile(&pool, actor, &actor_profile).await;
    drop_profile(&pool, me, &barrier_profile).await;
}

/// HTTP twins execute their own schema/path/error middleware. Domain outcomes
/// are shared, but transport error mapping and address-only responses are tested
/// against actual upstream controllers rather than inferred from RPC answers.
#[tokio::test]
async fn voice_http_answers_match_executed_upstream_controllers_and_middlewares() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/voice-http-source-cases.json"
    )))
    .unwrap();
    let a = fixture["caller"].as_str().unwrap();
    let b = fixture["target"].as_str().unwrap();
    let first = PrivateKeySigner::random();
    let second = PrivateKeySigner::random();
    let address_a = format!("{:#x}", first.address());
    let address_b = format!("{:#x}", second.address());
    let community: Uuid = fixture["community"].as_str().unwrap().parse().unwrap();
    let call: Uuid = fixture["call"].as_str().unwrap().parse().unwrap();
    let mut mismatches = Vec::new();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 76);
    for source in fixture["cases"].as_array().unwrap() {
        let encoded = serde_json::to_string(source)
            .unwrap()
            .replace(a, &address_a)
            .replace(b, &address_b);
        let case: Value = serde_json::from_str(&encoded).unwrap();
        let label = case["id"].as_str().unwrap();
        let operation = case["operation"].as_str().unwrap();
        let scenario = case["scenario"].as_str().unwrap();
        drop_community(&pool, community).await;
        sqlx::query("DELETE FROM social.private_voice_chats WHERE caller_address=ANY($1) OR callee_address=ANY($1)")
            .bind(vec![&address_a,&address_b]).execute(&pool).await.unwrap();
        cleanup_pair(&Db::new(pool.clone()), &address_a, &address_b).await;
        let working_pool = if operation == "private-incoming" && scenario == "runtime-failure" {
            sqlx::postgres::PgPoolOptions::new()
                .acquire_timeout(Duration::from_millis(20))
                .connect_lazy("postgres://postgres@127.0.0.1:1/unreachable")
                .unwrap()
        } else {
            pool.clone()
        };
        let voice = Arc::new(FixtureVoice {
            inner: FakeVoiceRooms::new(working_pool.clone()),
            missing: scenario == "runtime-not-found",
        });
        if operation.starts_with("community-") {
            sqlx::query(
                "INSERT INTO social.communities(id,name,owner_address) VALUES($1,'Community',$2)",
            )
            .bind(community)
            .bind(&address_a)
            .execute(&pool)
            .await
            .unwrap();
            for (actor, role) in [(&address_a, "owner"), (&address_b, "member")] {
                sqlx::query("INSERT INTO social.community_members(community_id,member_address,role) VALUES($1,$2,$3)")
                    .bind(community).bind(actor).bind(role).execute(&pool).await.unwrap();
            }
            if operation != "community-start" {
                voice.inner.live.lock().insert(community);
            }
        } else if operation != "private-start"
            && ["success", "runtime-failure", "uppercase-id"].contains(&scenario)
        {
            let (from, to) = if operation == "private-incoming" {
                (&address_b, &address_a)
            } else {
                (&address_a, &address_b)
            };
            sqlx::query("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,$3)")
                .bind(call).bind(from).bind(to).execute(&pool).await.unwrap();
        }
        let runtime: Arc<dyn VoiceRooms> = if scenario == "runtime-failure" {
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime)
        } else {
            voice.clone()
        };
        let ctx = test_ctx(Db::new(working_pool), runtime);
        let app = crate::rpc::http::router(Arc::new(State::with_context(
            Settings::default(),
            ctx,
            None,
        )));
        let path = case["path"].as_str().unwrap();
        let method = case["method"].as_str().unwrap();
        let key = if case["caller"] == address_a {
            &first
        } else {
            &second
        };
        let timestamp = chrono::Utc::now().timestamp_millis();
        let payload = crate::shared::signed_fetch::build_payload_v6(
            &method.to_lowercase(),
            path,
            &timestamp.to_string(),
            "{}",
        );
        let (_, signed) = frame_signed(
            key,
            &PrivateKeySigner::random(),
            FAR_EXPIRATION,
            timestamp,
            &payload,
            "{}",
        )
        .await;
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in
            serde_json::from_str::<serde_json::Map<String, Value>>(&signed).unwrap()
        {
            request = request.header(name, value.as_str().unwrap());
        }
        let body = if case["body"].is_null() {
            Body::empty()
        } else {
            request = request.header("content-type", "application/json");
            Body::from(case["body"].to_string())
        };
        let response = app.oneshot(request.body(body).unwrap()).await.unwrap();
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        let mut body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if operation == "private-start" && scenario == "success" {
            assert!(Uuid::parse_str(body["data"]["callId"].as_str().unwrap()).is_ok());
            body["data"]["callId"] = Value::String(call.to_string());
        }
        if status != case["response"]["status"].as_u64().unwrap() as u16
            || body != case["response"]["body"]
        {
            mismatches.push(format!(
                "{label}: actual {status} {body}; source {} {}",
                case["response"]["status"], case["response"]["body"]
            ));
        }
        if ["community-start", "community-join"].contains(&operation) && scenario == "success" {
            assert!(
                voice
                    .inner
                    .profiles
                    .lock()
                    .get(&address_a)
                    .is_some_and(Option::is_none),
                "{label}: HTTP must omit profile metadata"
            );
        }
    }
    drop_community(&pool, community).await;
    sqlx::query("DELETE FROM social.private_voice_chats WHERE caller_address=ANY($1) OR callee_address=ANY($1)")
        .bind(vec![&address_a,&address_b]).execute(&pool).await.unwrap();
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
