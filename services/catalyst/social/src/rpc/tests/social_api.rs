//! The procedures' HTTP twins on the social-api host (`rpc::http`), the
//! routes of upstream #504-#507 (`decentraland/social-service-ea` at
//! `b6b2cef`, `src/controllers/routes/http.routes.ts` and the
//! `test/integration/{friends,blocks,social-settings,voice-chats}-controller`
//! specs): the signed-fetch gate, the body and path checks, the envelopes,
//! the refusal statuses, and that a route and its procedure write the same
//! rows and publish the same events.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;

struct Rig {
    pool: PgPool,
    ctx: Context,
    voice: Arc<FakeVoiceRooms>,
    app: Router,
}

impl Rig {
    async fn new() -> Option<Self> {
        let pool = scratch_pool().await?;
        let voice = Arc::new(FakeVoiceRooms::new(pool.clone()));
        let ctx = test_ctx(Db::new(pool.clone()), voice.clone());
        let state = Arc::new(State::with_context(Settings::default(), ctx.clone(), None));
        // Mounted as `social::router` mounts it: at the root and under the gateway prefix.
        let app = Router::new()
            .merge(crate::rpc::http::router(state.clone()))
            .nest("/social-api", crate::rpc::http::router(state));
        Some(Self {
            pool,
            ctx,
            voice,
            app,
        })
    }

    /// One request, signed by `who` over its path when given.
    async fn call(
        &self,
        method: &str,
        path: &str,
        who: Option<&PrivateKeySigner>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(who) = who {
            for (name, value) in signed_headers(who, method, path.split('?').next().unwrap()).await
            {
                request = request.header(name, value);
            }
        }
        let body = match body {
            Some(body) => {
                request = request.header("content-type", "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }
}

/// A wallet made for the call and its lowercase address.
fn wallet() -> (PrivateKeySigner, String) {
    let signer = PrivateKeySigner::random();
    let address = format!("{:#x}", signer.address());
    (signer, address)
}

/// The signed-fetch headers of `method path`, as `signedFetch` sends them.
async fn signed_headers(who: &PrivateKeySigner, method: &str, path: &str) -> Vec<(String, String)> {
    let ts = chrono::Utc::now().timestamp_millis();
    let payload =
        crate::shared::signed_fetch::build_payload_v6(method, path, &ts.to_string(), "{}");
    let (_, frame) = frame_signed(
        who,
        &PrivateKeySigner::random(),
        FAR_EXPIRATION,
        ts,
        &payload,
        "{}",
    )
    .await;
    let frame: serde_json::Map<String, Value> = serde_json::from_str(&frame).unwrap();
    frame
        .into_iter()
        .map(|(k, v)| (k, v.as_str().unwrap().to_string()))
        .collect()
}

fn error(status: &str, message: &str) -> Value {
    json!({ "error": status, "message": message })
}

#[tokio::test]
async fn every_route_requires_a_signed_fetch_over_its_own_path() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (me, _) = wallet();
    let (status, body) = rig.call("GET", "/v2/friends", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Invalid Auth Chain");
    let (status, body) = rig
        .call(
            "POST",
            "/v1/blocks",
            None,
            Some(json!({ "address": "0x1" })),
        )
        .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &json!("Invalid Auth Chain")),
        "the gate answers before the body is read"
    );
    // A chain signed for another route does not open this one.
    let headers = signed_headers(&me, "get", "/v2/blocks").await;
    let mut request = Request::builder().method("GET").uri("/v2/friends");
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = rig
        .app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // Under the gateway prefix the signature is over the prefixed path.
    let (status, body) = rig
        .call("GET", "/social-api/v2/friends", Some(&me), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "data": { "results": [], "total": 0, "page": 1, "pages": 0, "limit": 100 } })
    );
}

#[tokio::test]
async fn a_friendship_runs_through_the_http_routes() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a_key, a) = wallet();
    let (b_key, b) = wallet();
    let mut to_b = rig.ctx.pubsub().subscribe(&b);
    let mut to_a = rig.ctx.pubsub().subscribe(&a);

    let (status, body) = rig
        .call(
            "POST",
            "/v1/friendship-requests",
            Some(&a_key),
            Some(json!({ "address": b.to_uppercase().replace("0X", "0x"), "message": "hi" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let request_id = body["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["data"]["address"], b);
    assert_eq!(body["data"]["message"], "hi");
    assert!(body["data"]["createdAt"].is_i64());
    match &*to_b.try_recv().unwrap() {
        SocialEvent::Friendship(FriendshipUpdate {
            update: Some(friendship_update::Update::Request(r)),
        }) => {
            assert_eq!(r.friend.as_ref().unwrap().address, a);
            assert_eq!(
                (r.id.as_str(), r.message.as_deref()),
                (request_id.as_str(), Some("hi"))
            );
        }
        other => panic!("{:?}", std::mem::discriminant(other)),
    }

    let (status, body) = rig
        .call(
            "GET",
            "/v2/friendship-requests?direction=received",
            Some(&b_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["results"][0]["id"], request_id);
    assert_eq!(body["data"]["results"][0]["address"], a);
    assert_eq!(body["data"]["results"][0]["message"], "hi");
    let (_, sent) = rig
        .call(
            "GET",
            "/v2/friendship-requests?direction=sent",
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(sent["data"]["results"][0]["address"], b);

    let (status, body) = rig
        .call(
            "PATCH",
            &format!("/v1/friendship-requests/{a}"),
            Some(&b_key),
            Some(json!({ "intention": "accepted" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["address"], a);
    assert!(body["data"].get("message").is_none());
    assert!(matches!(
        &*to_a.try_recv().unwrap(),
        SocialEvent::Friendship(FriendshipUpdate {
            update: Some(friendship_update::Update::Accept(_))
        })
    ));

    let (status, body) = rig.call("GET", "/v2/friends", Some(&a_key), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        json!({ "results": [{ "address": b }], "total": 1, "page": 1, "pages": 1, "limit": 100 })
    );
    let (_, body) = rig
        .call(
            "GET",
            &format!("/v1/users/{b}/friendship-status"),
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        body,
        json!({ "data": { "status": FriendshipStatus::Accepted as i32 } })
    );
    let (status, body) = rig
        .call(
            "GET",
            &format!("/v2/users/{b}/mutual-friends"),
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        (status, &body["data"]["total"]),
        (StatusCode::OK, &json!(0))
    );

    let (status, _) = rig
        .call("DELETE", &format!("/v1/friends/{b}"), Some(&a_key), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = rig
        .call(
            "GET",
            &format!("/v1/users/{a}/friendship-status"),
            Some(&b_key),
            None,
        )
        .await;
    assert_eq!(body["data"]["status"], FriendshipStatus::Deleted as i32);
    cleanup_pair(rig.ctx.db(), &a, &b).await;
}

#[tokio::test]
async fn friendship_routes_refuse_as_upstream_does() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a_key, a) = wallet();
    let (_, b) = wallet();
    let cases = [
        (
            "POST",
            "/v1/friendship-requests".to_string(),
            Some(json!({ "address": a })),
            error(
                "Bad request",
                "You cannot send a friendship request to yourself",
            ),
        ),
        (
            "PATCH",
            "/v1/friendship-requests/0x123".to_string(),
            Some(json!({ "intention": "accepted" })),
            error("Bad request", "Invalid address"),
        ),
        (
            "PATCH",
            format!("/v1/friendship-requests/{a}"),
            Some(json!({ "intention": "rejected" })),
            error(
                "Bad request",
                "There is no friendship request with yourself",
            ),
        ),
        (
            "DELETE",
            format!("/v1/friends/{a}"),
            None,
            error("Bad request", "You cannot remove yourself as a friend"),
        ),
        (
            "GET",
            "/v2/friendship-requests?direction=both".to_string(),
            None,
            error("Bad request", "direction must be one of: received, sent"),
        ),
        (
            "GET",
            "/v1/users/not-an-address/friendship-status".to_string(),
            None,
            error("Bad request", "Invalid address"),
        ),
    ];
    for (method, path, body, expected) in cases {
        let (status, answer) = rig.call(method, &path, Some(&a_key), body).await;
        assert_eq!(
            (status, answer),
            (StatusCode::BAD_REQUEST, expected),
            "{method} {path}"
        );
    }
    // The body schemas: a missing, extra, malformed or too long property.
    for body in [
        json!({}),
        json!({ "address": b, "extra": true }),
        json!({ "address": "0x1" }),
        json!({ "address": b, "message": "m".repeat(257) }),
    ] {
        let (status, _) = rig
            .call(
                "POST",
                "/v1/friendship-requests",
                Some(&a_key),
                Some(body.clone()),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let (status, _) = rig
        .call(
            "PATCH",
            &format!("/v1/friendship-requests/{b}"),
            Some(&a_key),
            Some(json!({ "intention": "maybe" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // No pending request to accept: the state machine's 409.
    let (status, body) = rig
        .call(
            "PATCH",
            &format!("/v1/friendship-requests/{b}"),
            Some(&a_key),
            Some(json!({ "intention": "accepted" })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::CONFLICT,
            error(
                "Conflict",
                "The friendship action is not valid for the current friendship status"
            )
        )
    );

    // Ten actions on a pair a minute (the refused accept above was one), then 429.
    for i in 0..9 {
        let (method, path, body) = if i % 2 == 0 {
            (
                "POST",
                "/v1/friendship-requests".to_string(),
                Some(json!({ "address": b })),
            )
        } else {
            (
                "PATCH",
                format!("/v1/friendship-requests/{b}"),
                Some(json!({ "intention": "cancelled" })),
            )
        };
        let (status, answer) = rig.call(method, &path, Some(&a_key), body).await;
        assert!(status.is_success(), "action {i}: {status} {answer}");
    }
    let (status, body) = rig
        .call(
            "POST",
            "/v1/friendship-requests",
            Some(&a_key),
            Some(json!({ "address": b })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::TOO_MANY_REQUESTS,
            error(
                "Too Many Requests",
                "Too many friendship or block actions. Please try again later"
            )
        )
    );
    cleanup_pair(rig.ctx.db(), &a, &b).await;
}

#[tokio::test]
async fn blocks_run_through_the_http_routes() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a_key, a) = wallet();
    let (b_key, b) = wallet();
    let mut to_b = rig.ctx.pubsub().subscribe(&b);

    let (status, body) = rig
        .call(
            "POST",
            "/v1/blocks",
            Some(&a_key),
            Some(json!({ "address": a })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "You cannot block yourself")
        )
    );
    let (status, body) = rig
        .call(
            "POST",
            "/v1/blocks",
            Some(&a_key),
            Some(json!({ "address": b })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["address"], b);
    assert!(body["data"]["blockedAt"].is_i64());
    assert!(matches!(
        &*to_b.try_recv().unwrap(),
        SocialEvent::Block(BlockUpdate { address, is_blocked: true }) if *address == a
    ));
    // A stranger has no friendship to end: no FriendshipUpdate::Block
    // follows (upstream `applyBlock` publishes it only with a friendship).
    assert!(to_b.try_recv().is_err());

    let (_, body) = rig
        .call("GET", "/v1/blocking-status", Some(&a_key), None)
        .await;
    assert_eq!(
        body,
        json!({ "data": { "blockedUsers": [b], "blockedByUsers": [] } })
    );
    let (_, body) = rig
        .call("GET", "/v1/blocking-status", Some(&b_key), None)
        .await;
    assert_eq!(
        body,
        json!({ "data": { "blockedUsers": [], "blockedByUsers": [a] } })
    );
    let (_, body) = rig
        .call("GET", "/v2/blocks?limit=10", Some(&a_key), None)
        .await;
    assert_eq!(body["data"]["results"][0]["address"], b);
    assert_eq!(
        (&body["data"]["total"], &body["data"]["limit"]),
        (&json!(1), &json!(10))
    );

    // A blocked pair cannot befriend: the block's 403.
    let (status, body) = rig
        .call(
            "POST",
            "/v1/friendship-requests",
            Some(&b_key),
            Some(json!({ "address": a })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::FORBIDDEN,
            error(
                "Forbidden",
                "This action is not allowed because either you blocked this user or this user blocked you"
            )
        )
    );

    let (status, _) = rig
        .call("DELETE", &format!("/v1/blocks/{b}"), Some(&a_key), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let lifted = to_b.try_recv().unwrap();
    assert!(
        matches!(
            &*lifted,
            SocialEvent::Block(BlockUpdate {
                is_blocked: false,
                ..
            })
        ),
        "{}",
        match &*lifted {
            SocialEvent::Block(b) => format!("block {b:?}"),
            SocialEvent::Friendship(f) => format!("friendship {f:?}"),
            _ => "other".into(),
        }
    );
    // Idempotent: nothing left to lift, nothing told.
    let (status, _) = rig
        .call("DELETE", &format!("/v1/blocks/{b}"), Some(&a_key), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(to_b.try_recv().is_err());
    let (status, body) = rig
        .call("DELETE", "/v1/blocks/0xnope", Some(&a_key), None)
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "Invalid address")
        )
    );
    cleanup_pair(rig.ctx.db(), &a, &b).await;
}

/// The same mutation through the route and through its procedure leaves the
/// same rows and tells the other party the same thing.
#[tokio::test]
async fn a_route_and_its_procedure_write_and_publish_alike() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a_key, a) = wallet();
    let (_, b) = wallet();
    let (c, d) = (addr("par1"), addr("par2"));
    let mut to_b = rig.ctx.pubsub().subscribe(&b);
    let mut to_d = rig.ctx.pubsub().subscribe(&d);

    let (status, http) = rig
        .call(
            "POST",
            "/v1/friendship-requests",
            Some(&a_key),
            Some(json!({ "address": b, "message": "hello" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let rpc = SocialServiceImpl
        .upsert_friendship(
            UpsertFriendshipPayload {
                action: Some(upsert_friendship_payload::Action::Request(
                    upsert_friendship_payload::RequestPayload {
                        user: Some(User { address: d.clone() }),
                        message: Some("hello".into()),
                    },
                )),
            },
            procedure_ctx(&rig.ctx, &c, 501),
        )
        .await
        .unwrap();
    let Some(upsert_friendship_response::Response::Accepted(accepted)) = rpc.response else {
        panic!("{rpc:?}");
    };

    let request_of = |event: &SocialEvent| match event {
        SocialEvent::Friendship(FriendshipUpdate {
            update: Some(friendship_update::Update::Request(r)),
        }) => (
            r.friend.as_ref().unwrap().address.clone(),
            r.id.clone(),
            r.message.clone(),
        ),
        _ => panic!("not a request"),
    };
    let (from_b, id_b, message_b) = request_of(&to_b.try_recv().unwrap());
    let (from_d, id_d, message_d) = request_of(&to_d.try_recv().unwrap());
    assert_eq!((from_b, message_b), (a.clone(), Some("hello".to_string())));
    assert_eq!((from_d, message_d), (c.clone(), Some("hello".to_string())));
    assert_eq!(id_b, http["data"]["id"].as_str().unwrap());
    assert_eq!(id_d, accepted.id);

    let db = rig.ctx.db();
    for (sender, receiver) in [(&a, &b), (&c, &d)] {
        let last = db
            .last_friendship_action(sender, receiver)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (last.action.as_str(), last.acting_user.as_str()),
            ("request", sender.as_str())
        );
        let (rows, total) = db
            .get_friendship_requests(receiver, true, 10, 0)
            .await
            .unwrap();
        assert_eq!((rows[0].message.as_deref(), total), (Some("hello"), 1));
    }
    cleanup_pair(db, &a, &b).await;
    cleanup_pair(db, &c, &d).await;
}

#[tokio::test]
async fn settings_run_through_the_http_routes() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a_key, a) = wallet();
    let (_, b) = wallet();
    let (_, body) = rig
        .call("GET", "/v1/social-settings", Some(&a_key), None)
        .await;
    assert_eq!(
        body,
        json!({ "data": {
            "privateMessagesPrivacy": "all",
            "blockedUsersMessagesVisibility": "show_messages",
            "showSituationReactions": "show"
        } })
    );
    for refused in [
        json!({}),
        json!({ "privateMessagesPrivacy": "nobody" }),
        json!({ "showSituationReactions": "hide", "extra": 1 }),
    ] {
        let (status, _) = rig
            .call(
                "PATCH",
                "/v1/social-settings",
                Some(&a_key),
                Some(refused.clone()),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    }
    let (status, body) = rig
        .call(
            "PATCH",
            "/v1/social-settings",
            Some(&a_key),
            Some(json!({ "privateMessagesPrivacy": "only_friends", "showSituationReactions": "hide" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        json!({
            "privateMessagesPrivacy": "only_friends",
            "blockedUsersMessagesVisibility": "show_messages",
            "showSituationReactions": "hide"
        })
    );
    // The procedure reads what the route wrote.
    let rpc = SocialServiceImpl
        .get_social_settings(procedure_ctx(&rig.ctx, &a, 502))
        .await
        .unwrap();
    let Some(get_social_settings_response::Response::Ok(ok)) = rpc.response else {
        panic!("{rpc:?}");
    };
    let settings = ok.settings.unwrap();
    assert_eq!(
        (
            settings.private_messages_privacy,
            settings.show_situation_reactions
        ),
        (
            PrivateMessagePrivacySetting::OnlyFriends as i32,
            SituationReactionsVisibility::Hide as i32
        )
    );

    let (status, body) = rig
        .call(
            "GET",
            &format!("/v1/private-messages-settings?addresses={b}&addresses={a}&addresses={b}"),
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        json!([
            { "address": b, "privateMessagesPrivacy": "all", "isFriend": false },
            { "address": a, "privateMessagesPrivacy": "only_friends", "isFriend": false }
        ])
    );
    let (_, body) = rig
        .call("GET", "/v1/private-messages-settings", Some(&a_key), None)
        .await;
    assert_eq!(body, json!({ "data": [] }));
    let (status, body) = rig
        .call(
            "GET",
            "/v1/private-messages-settings?addresses=0x123",
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "Invalid address: 0x123")
        )
    );
    let many: String = (0..51).map(|_| format!("addresses={b}&")).collect();
    let (status, body) = rig
        .call(
            "GET",
            &format!("/v1/private-messages-settings?{many}"),
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "At most 50 addresses can be requested")
        )
    );
    let _ = sqlx::query("DELETE FROM social.user_settings WHERE address = $1")
        .bind(&a)
        .execute(&rig.pool)
        .await;
}

#[tokio::test]
async fn a_private_call_runs_through_the_http_routes() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (a_key, a) = wallet();
    let (b_key, b) = wallet();
    let mut to_a = rig.ctx.pubsub().subscribe(&a);
    let mut to_b = rig.ctx.pubsub().subscribe(&b);

    let (status, body) = rig
        .call(
            "POST",
            "/v1/private-voice-chats",
            Some(&a_key),
            Some(json!({ "calleeAddress": a })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "You cannot call yourself")
        )
    );
    let (status, body) = rig
        .call(
            "POST",
            "/v1/private-voice-chats",
            Some(&a_key),
            Some(json!({ "calleeAddress": b })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let call_id = body["data"]["callId"].as_str().unwrap().to_string();
    assert!(matches!(
        &*to_b.try_recv().unwrap(),
        SocialEvent::PrivateVoice(u) if u.call_id == call_id
            && u.status == PrivateVoiceChatStatus::VoiceChatRequested as i32
    ));
    // Calling again while ringing: the busy 409.
    let (status, body) = rig
        .call(
            "POST",
            "/v1/private-voice-chats",
            Some(&b_key),
            Some(json!({ "calleeAddress": a })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::CONFLICT,
            error("Conflict", "One of the users is busy calling someone else")
        )
    );

    let (_, body) = rig
        .call(
            "GET",
            "/v1/private-voice-chats/incoming",
            Some(&b_key),
            None,
        )
        .await;
    assert_eq!(
        body,
        json!({ "data": { "callId": call_id, "callerAddress": a } })
    );
    let (status, body) = rig
        .call(
            "GET",
            "/v1/private-voice-chats/incoming",
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::NOT_FOUND,
            error(
                "Not Found",
                &format!("The incoming voice chat for the address {a} was not found")
            )
        )
    );

    let (status, body) = rig
        .call(
            "PATCH",
            &format!("/v1/private-voice-chats/{call_id}"),
            Some(&a_key),
            Some(json!({ "intention": "accepted" })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only the callee accepts: {body}"
    );
    let (status, body) = rig
        .call(
            "PATCH",
            &format!("/v1/private-voice-chats/{call_id}"),
            Some(&b_key),
            Some(json!({ "intention": "accepted" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["callId"], call_id);
    assert!(
        body["data"]["connectionUrl"]
            .as_str()
            .unwrap()
            .ends_with(&format!("for={b}"))
    );
    assert!(matches!(
        &*to_a.try_recv().unwrap(),
        SocialEvent::PrivateVoice(u) if u.credentials.as_ref().is_some_and(|c| c.connection_url.ends_with(&format!("for={a}")))
    ));

    let (status, _) = rig
        .call(
            "DELETE",
            &format!("/v1/private-voice-chats/{call_id}"),
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = rig
        .call(
            "DELETE",
            &format!("/v1/private-voice-chats/{call_id}"),
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::NOT_FOUND,
            error(
                "Not Found",
                &format!("The voice chat with id {call_id} was not found")
            )
        )
    );
    let (status, body) = rig
        .call(
            "DELETE",
            "/v1/private-voice-chats/not-a-uuid",
            Some(&a_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "Invalid callId")
        )
    );
    let (status, _) = rig
        .call(
            "PATCH",
            &format!("/v1/private-voice-chats/{call_id}"),
            Some(&b_key),
            Some(json!({ "intention": "ended" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_community_room_runs_through_the_http_routes() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (owner_key, owner) = wallet();
    let (guest_key, guest) = wallet();
    let community = seed_community(&rig.pool, true, &owner, "owner").await;
    let id = community.to_string();

    let (status, body) = rig
        .call(
            "POST",
            "/v1/community-voice-chats",
            Some(&owner_key),
            Some(json!({ "communityId": "nope" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = rig
        .call(
            "POST",
            "/v1/community-voice-chats",
            Some(&guest_key),
            Some(json!({ "communityId": id })),
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::FORBIDDEN,
            error(
                "Forbidden",
                &format!("User {guest} is not a member of community {id}")
            )
        )
    );
    let (status, body) = rig
        .call(
            "POST",
            "/v1/community-voice-chats",
            Some(&owner_key),
            Some(json!({ "communityId": id })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(
        body["data"]["connectionUrl"]
            .as_str()
            .unwrap()
            .contains(&format!("voice-chat-community-{id}"))
    );
    let (status, _) = rig
        .call(
            "POST",
            &format!("/v1/community-voice-chats/{id}/participants"),
            Some(&owner_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // A guest of a public community may raise a hand; the owner promotes,
    // mutes and demotes them.
    let (status, _) = rig
        .call(
            "POST",
            &format!("/v1/community-voice-chats/{id}/speak-requests"),
            Some(&guest_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let speaker = format!("/v1/community-voice-chats/{id}/speakers/{guest}");
    for (method, body) in [
        ("PUT", None),
        ("PATCH", Some(json!({ "muted": true }))),
        ("DELETE", None),
    ] {
        let (status, answer) = rig.call(method, &speaker, Some(&owner_key), body).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{method}: {answer}");
    }
    let (status, _) = rig
        .call(
            "PATCH",
            &speaker,
            Some(&owner_key),
            Some(json!({ "muted": "yes" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let calls = rig.voice.calls.lock().clone();
    for expected in [
        format!("request-to-speak {id} {guest} true"),
        format!("set-speaker {id} {guest} true"),
        format!("mute {id} {guest} true"),
        format!("set-speaker {id} {guest} false"),
    ] {
        assert!(calls.contains(&expected), "{expected} not in {calls:?}");
    }

    // The runtime's room has no such participant: 404, on both transports;
    // lowering one's own hand stays idempotent.
    rig.voice.absent.lock().push(guest.clone());
    let not_found = error(
        "Not Found",
        &format!("Community voice chat or participant not found for community {id}"),
    );
    let (status, body) = rig
        .call(
            "DELETE",
            &format!("/v1/community-voice-chats/{id}/participants/{guest}"),
            Some(&owner_key),
            None,
        )
        .await;
    assert_eq!((status, body), (StatusCode::NOT_FOUND, not_found.clone()));
    let rpc = SocialServiceImpl
        .kick_player_from_community_voice_chat(
            KickPlayerFromCommunityVoiceChatPayload {
                community_id: id.clone(),
                user_address: guest.clone(),
            },
            procedure_ctx(&rig.ctx, &owner, 503),
        )
        .await
        .unwrap();
    assert!(matches!(
        &rpc.response,
        Some(kick_player_from_community_voice_chat_response::Response::NotFoundError(e))
            if e.message.as_deref() == not_found["message"].as_str()
    ));
    let (status, _) = rig
        .call(
            "POST",
            &format!("/v1/community-voice-chats/{id}/speak-requests"),
            Some(&guest_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = rig
        .call(
            "DELETE",
            &format!("/v1/community-voice-chats/{id}/speak-requests/{guest}"),
            Some(&guest_key),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "lowering a hand is idempotent"
    );

    let (status, body) = rig
        .call(
            "DELETE",
            &format!("/v1/community-voice-chats/{id}/participants/0x12"),
            Some(&owner_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            error("Bad request", "Invalid address")
        )
    );
    let (status, _) = rig
        .call(
            "DELETE",
            &format!("/v1/community-voice-chats/{id}"),
            Some(&guest_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = rig
        .call(
            "DELETE",
            &format!("/v1/community-voice-chats/{id}"),
            Some(&owner_key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // the room ended: a second end is not found
    let (status, body) = rig
        .call(
            "DELETE",
            &format!("/v1/community-voice-chats/{id}"),
            Some(&owner_key),
            None,
        )
        .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::NOT_FOUND,
            error(
                "Not Found",
                &format!("Community voice chat not found for community {id}")
            )
        )
    );
    drop_community(&rig.pool, community).await;
}
