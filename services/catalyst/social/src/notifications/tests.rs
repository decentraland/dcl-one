//! Route-level contracts over the router as `serve` mounts it (nested under
//! `/notifications`), signed with the crate's test wallet. DB-backed tests
//! use the process's scratch database (`CATALYST_TEST_DATABASE_URL`) and
//! rows scoped by fresh addresses or timestamps; without the variable they
//! skip.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use super::email::INVALID_EMAIL;
use super::store::test_support::{fresh_address, seed_notification};
use super::types::default_message_type;
use super::{Settings, SharedState, State, router};
use crate::identity::Address;
use crate::shared::signed_fetch::test_support::{
    address as signer_address, signed_headers, signed_headers_by,
};

async fn state(settings: Settings) -> Option<SharedState> {
    let pool = crate::db::scratch::pool(4).await?;
    Some(Arc::new(State::new(
        pool,
        settings,
        crate::http::Outbound::for_tests(),
        None,
        None,
    )))
}

fn app(state: SharedState) -> Router {
    Router::new().nest("/notifications", router(state))
}

async fn send(
    app: Router,
    method: &str,
    path: &str,
    headers: HeaderMap,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

async fn signed(method: &str, path: &str) -> HeaderMap {
    signed_headers(method, path, r#"{"signer":"dcl:explorer"}"#, true).await
}

fn signer() -> Address {
    Address::parse(&signer_address()).unwrap()
}

#[tokio::test]
async fn unsigned_scene_signed_and_missigned_requests_are_refused() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (status, body) = send(
        app(state.clone()),
        "GET",
        "/notifications/notifications",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body,
        json!({
            "error": "Invalid Auth Chain",
            "message": "This endpoint requires a signed fetch request. See ADR-44."
        })
    );

    for metadata in [
        r#"{"signer":"decentraland-kernel-scene"}"#,
        r#"{"Signer":"decentraland-kernel-scene"}"#,
    ] {
        for v6 in [true, false] {
            let headers = signed_headers("get", "/notifications/notifications", metadata, v6).await;
            let (status, body) = send(
                app(state.clone()),
                "GET",
                "/notifications/notifications",
                headers,
                None,
            )
            .await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "v6={v6} {metadata}: {body}"
            );
        }
    }

    let headers = signed("get", "/notifications/subscription").await;
    let (status, body) = send(
        app(state.clone()),
        "GET",
        "/notifications/notifications",
        headers,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"], json!("Unauthenticated: Invalid signature"));

    // notifications.<domain>/notifications and gateway.<domain>/notifications/notifications
    // are signed over the route alone; the front moves both under /notifications and
    // forwards the signed path.
    let mut headers = signed("get", "/notifications").await;
    headers.insert("x-original-path", "/notifications".parse().unwrap());
    let (status, body) = send(
        app(state.clone()),
        "GET",
        "/notifications/notifications",
        headers,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // A proxy that strips the prefix forwards the public path.
    let mut headers = signed("get", "/notifications/notifications").await;
    headers.insert(
        "x-original-path",
        "/notifications/notifications".parse().unwrap(),
    );
    let (status, _) = send(
        Router::new().merge(router(state)),
        "GET",
        "/notifications",
        headers,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn signed_list_read_subscription_and_opt_outs_round_trip() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let pool = state.store.pool().clone();
    let me = signer();
    // The test wallet is shared by this process's tests: scope by a window
    // far in the future, unique to this test.
    let base = chrono::Utc::now().timestamp_millis()
        + (Uuid::new_v4().as_u128() % 1_000_000_000) as i64
        + 1_000_000_000_000;
    let older = seed_notification(&pool, &me, base + 1, false).await;
    let newer = seed_notification(&pool, &me, base + 2, false).await;
    seed_notification(&pool, &fresh_address(), base + 3, false).await;

    let list_path = format!("/notifications/notifications?from={base}&limit=10");
    let (status, body) = send(
        app(state.clone()),
        "GET",
        &list_path,
        signed("get", "/notifications/notifications").await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["notifications"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec![newer.to_string(), older.to_string()]
    );
    assert_eq!(items[0]["timestamp"], json!((base + 2).to_string()));
    assert_eq!(items[0]["type"], json!("bid_received"));
    assert_eq!(items[0]["address"], json!(me.as_str()));
    assert_eq!(items[0]["read"], json!(false));
    assert!(items[0]["created_at"].as_str().unwrap().ends_with('Z'));

    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/notifications/read",
        signed("put", "/notifications/notifications/read").await,
        Some(json!({ "notificationIds": [newer.to_string(), older.to_string(), Uuid::new_v4().to_string()] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "updated": 2 }));
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/notifications/read",
        signed("put", "/notifications/notifications/read").await,
        Some(json!({ "notificationIds": ["not-a-uuid"] })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid notification id"))
    );

    let unread_path = format!("/notifications/notifications?from={base}&onlyUnread=false");
    let (_, body) = send(
        app(state.clone()),
        "GET",
        &unread_path,
        signed("get", "/notifications/notifications").await,
        None,
    )
    .await;
    assert_eq!(body["notifications"].as_array().unwrap().len(), 0);

    sqlx::query("DELETE FROM social.subscriptions WHERE address = $1")
        .bind(&me)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = send(
        app(state.clone()),
        "GET",
        "/notifications/subscription",
        signed("get", "/notifications/subscription").await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["address"], json!(me.as_str()));
    assert!(body.get("email").is_none() && body.get("unconfirmedEmail").is_none());
    assert_eq!(body["details"]["ignore_all_email"], json!(false));
    assert_eq!(
        body["details"]["message_type"]["bid_received"],
        json!({"email": true, "in_app": true})
    );

    let mut details = json!({ "ignore_all_email": true, "ignore_all_in_app": false, "message_type": default_message_type() });
    details["message_type"]["item_sold"] = json!({"email": false, "in_app": true});
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/subscription",
        signed("put", "/notifications/subscription").await,
        Some(details),
    )
    .await;
    assert_eq!((status, body), (StatusCode::NO_CONTENT, Value::Null));
    // An older client's partial list keeps every type it does not send.
    let partial = json!({ "ignore_all_email": false, "ignore_all_in_app": false,
        "message_type": { "bid_received": { "email": false, "in_app": false } } });
    let (status, _) = send(
        app(state.clone()),
        "PUT",
        "/notifications/subscription",
        signed("put", "/notifications/subscription").await,
        Some(partial),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = send(
        app(state.clone()),
        "GET",
        "/notifications/subscription",
        signed("get", "/notifications/subscription").await,
        None,
    )
    .await;
    assert_eq!(body["details"]["ignore_all_email"], json!(false));
    assert_eq!(
        body["details"]["message_type"]["item_sold"],
        json!({"email": false, "in_app": true})
    );
    assert_eq!(
        body["details"]["message_type"]["bid_received"],
        json!({"email": false, "in_app": false})
    );
    for (bad, message) in [
        (
            json!({ "ignore_all_email": "yes", "ignore_all_in_app": false, "message_type": {} }),
            "Invalid subscription. ignore_all_email must be boolean",
        ),
        (
            json!({ "ignore_all_email": false, "ignore_all_in_app": false, "message_type": { "made_up": {} } }),
            "Invalid subscription. message_type must NOT have additional properties",
        ),
    ] {
        let (status, body) = send(
            app(state.clone()),
            "PUT",
            "/notifications/subscription",
            signed("put", "/notifications/subscription").await,
            Some(bad),
        )
        .await;
        assert_eq!(
            body,
            json!({ "error": "Bad request", "message": message }),
            "{status}"
        );
    }

    let community = Uuid::new_v4().to_string();
    let scoped = format!("/notifications/subscription/opt-outs/community/{community}");
    let (status, body) = send(
        app(state.clone()),
        "POST",
        "/notifications/subscription/opt-outs",
        signed("post", "/notifications/subscription/opt-outs").await,
        Some(json!({ "scope": "community", "scopeId": community })),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::CREATED,
            json!({ "scope": "community", "scopeId": community, "optedOut": true })
        )
    );
    for (bad, keyword) in [
        (json!({ "scope": "world", "scopeId": "x" }), "enum"),
        (
            json!({ "scope": "community", "scopeId": "x", "extra": 1 }),
            "additionalProperties",
        ),
        (json!({ "scope": "community" }), "required"),
        (json!({ "scope": "community", "scopeId": "" }), "minLength"),
    ] {
        let (status, body) = send(
            app(state.clone()),
            "POST",
            "/notifications/subscription/opt-outs",
            signed("post", "/notifications/subscription/opt-outs").await,
            Some(bad.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
        assert_eq!(body["ok"], json!(false), "{body}");
        assert_eq!(body["message"], json!("Invalid JSON body"), "{body}");
        assert_eq!(body["data"][0]["keyword"], json!(keyword), "{body}");
    }
    let planet = format!("/notifications/subscription/opt-outs/planet/{community}");
    let (status, body) = send(
        app(state.clone()),
        "GET",
        &planet,
        signed("get", &planet).await,
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "Bad request", "message": "Invalid scope" })
        )
    );
    let (status, body) = send(
        app(state.clone()),
        "GET",
        &scoped,
        signed("get", &scoped).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "scope": "community", "scopeId": community, "optedOut": true })
    );
    let (status, _) = send(
        app(state.clone()),
        "DELETE",
        &scoped,
        signed("delete", &scoped).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = send(
        app(state.clone()),
        "GET",
        &scoped,
        signed("get", &scoped).await,
        None,
    )
    .await;
    assert_eq!(body["optedOut"], json!(false));
}

#[tokio::test]
async fn moderation_notices_reach_signed_recipient_reader_and_marking_stays_wallet_scoped() {
    use crate::comms::interfaces::{
        ModerationNotification, ModerationNotificationKind as Kind, SocialDirectory,
    };
    let Some(state) = state(Settings::default()).await else {
        eprintln!("GUARDED moderation signed-reader assertions: CATALYST_TEST_DATABASE_URL unset");
        return;
    };
    let wallet = alloy::signers::local::PrivateKeySigner::random();
    let recipient = Address::parse(&format!("{:#x}", wallet.address())).unwrap();
    let pool = state.store.pool().clone();
    let directory = crate::comms_directory::Directory::new(pool.clone());
    let time = chrono::DateTime::from_timestamp_millis(1_700_000_000_123)
        .unwrap()
        .naive_utc();
    let mut txn = pool.begin().await.unwrap();
    for event in [
        Kind::Banned {
            reason: "abuse".into(),
            banned_at: 123,
            expires_at: None,
            custom_message: None,
        },
        Kind::BanLifted { lifted_at: 124 },
        Kind::BanWarning {
            reason: "warning".into(),
            warned_at: 125,
        },
        Kind::UserBannedFromScene {
            place_title: "Exact Scene".into(),
        },
        Kind::UserUnbannedFromScene {
            place_title: "Exact Scene".into(),
        },
    ] {
        directory
            .notify_moderation_notification(
                &mut txn,
                &ModerationNotification {
                    source_id: Uuid::new_v4().to_string(),
                    recipient: recipient.clone(),
                    occurred_at: time,
                    event,
                },
            )
            .await
            .unwrap();
    }
    let stranger = fresh_address();
    directory
        .notify_moderation_notification(
            &mut txn,
            &ModerationNotification {
                source_id: Uuid::new_v4().to_string(),
                recipient: stranger.clone(),
                occurred_at: time,
                event: Kind::BanWarning {
                    reason: "private stranger warning".into(),
                    warned_at: 126,
                },
            },
        )
        .await
        .unwrap();
    txn.commit().await.unwrap();
    let signed = async |method: &str, path: &str| {
        signed_headers_by(
            &wallet,
            method,
            path,
            r#"{"signer":"dcl:explorer"}"#,
            true,
            &chrono::Utc::now().timestamp_millis().to_string(),
        )
        .await
    };
    let path = "/notifications/notifications";
    let (status, response) = send(
        app(state.clone()),
        "GET",
        path,
        signed("get", path).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let items = response["notifications"].as_array().unwrap();
    let mut kinds: Vec<&str> = items
        .iter()
        .map(|item| {
            assert_eq!(item["address"], recipient.as_str());
            assert_eq!(item["timestamp"], "1700000000123");
            item["type"].as_str().unwrap()
        })
        .collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        [
            "ban_lifted",
            "ban_warning",
            "banned",
            "user_banned_from_scene",
            "user_unbanned_from_scene"
        ]
    );
    let stranger_id: Uuid =
        sqlx::query_scalar("SELECT id FROM social.notifications WHERE address=$1")
            .bind(&stranger)
            .fetch_one(&pool)
            .await
            .unwrap();
    let path = "/notifications/notifications/read";
    let (status, response) = send(
        app(state),
        "PUT",
        path,
        signed("put", path).await,
        Some(json!({"notificationIds":[items[0]["id"],stranger_id.to_string()]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["updated"], 1);
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT read FROM social.notifications WHERE id=$1")
            .bind(stranger_id)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn moderator_authorization_precedes_notice_and_warning_mutation() {
    use crate::comms::gatekeeper::{CommsState, Settings as CommsSettings};
    use crate::comms::interfaces::test_support::{StubPolicy, StubScenes};
    let Some(pool) = crate::db::scratch::pool(4).await else {
        eprintln!(
            "GUARDED moderation authorization/notification assertions: CATALYST_TEST_DATABASE_URL unset"
        );
        return;
    };
    let recipient = fresh_address();
    let mut settings = CommsSettings::for_tests();
    settings.moderator_token = Some("moderation-test-token".into());
    settings.livekit.configured = false;
    let state = CommsState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(StubScenes::default()),
        Arc::new(crate::comms::presence::Presence::new()),
        Arc::new(crate::comms_directory::Directory::new(pool.clone())),
    );
    let app = crate::comms::gatekeeper::router(Arc::new(state));
    let path = format!("/users/{}/warnings?moderator=Test", recipient.as_str());
    let (status, _) = send(
        app.clone(),
        "POST",
        &path,
        HeaderMap::new(),
        Some(json!({"reason":"warning"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM social.user_warnings WHERE warned_address=$1),(SELECT count(*) FROM social.notifications WHERE address=$1)").bind(&recipient).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0));
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer moderation-test-token".parse().unwrap(),
    );
    let (status, response) = send(
        app,
        "POST",
        &path,
        headers,
        Some(json!({"reason":"warning"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    let row: (String, serde_json::Value) =
        sqlx::query_as("SELECT type,metadata FROM social.notifications WHERE address=$1")
            .bind(&recipient)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row,
        (
            "ban_warning".into(),
            json!({"reason":"warning","warnedAt":chrono::DateTime::parse_from_rfc3339(response["data"]["warnedAt"].as_str().unwrap()).unwrap().timestamp_millis()})
        )
    );
}

#[tokio::test]
async fn broadcast_is_bearer_gated_and_validated() {
    let Some(unset) = state(Settings::default()).await else {
        return;
    };
    let bearer = |token: &str| {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
        h
    };
    let body = json!({ "type": "bid_received", "metadata": {"title": "hi"}, "addresses": [fresh_address().as_str()] });
    let (status, resp) = send(
        app(unset),
        "POST",
        "/notifications/notifications/broadcast",
        bearer("s3cret"),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        resp["error"],
        json!("admin broadcast disabled: CATALYRST_NOTIFICATIONS_ADMIN_TOKEN is not set")
    );

    let state = state(Settings {
        admin_token: Some("s3cret".into()),
        ..Settings::default()
    })
    .await
    .unwrap();
    for wrong in [HeaderMap::new(), bearer("nope")] {
        let (status, resp) = send(
            app(state.clone()),
            "POST",
            "/notifications/notifications/broadcast",
            wrong,
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            resp["error"],
            json!("You are not authorized to access this resource")
        );
    }
    let (status, resp) = send(
        app(state.clone()),
        "POST",
        "/notifications/notifications/broadcast",
        bearer("s3cret"),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(
        (
            resp["ok"].as_bool(),
            resp["type"].as_str(),
            resp["recipients"].as_u64()
        ),
        (Some(true), Some("bid_received"), Some(1))
    );
    assert!(Uuid::parse_str(resp["broadcastId"].as_str().unwrap()).is_ok());

    // A client-supplied broadcastId is the idempotency key: the retried POST
    // reaches nobody twice and echoes the same id.
    let target = fresh_address();
    let key = format!("ops-{}", Uuid::new_v4());
    let keyed = json!({ "type": "bid_received", "metadata": {"title": "hi"}, "addresses": [target.as_str()], "broadcastId": key });
    let (status, first) = send(
        app(state.clone()),
        "POST",
        "/notifications/notifications/broadcast",
        bearer("s3cret"),
        Some(keyed.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, second) = send(
        app(state.clone()),
        "POST",
        "/notifications/notifications/broadcast",
        bearer("s3cret"),
        Some(keyed),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(
        (first["recipients"].as_u64(), second["recipients"].as_u64()),
        (Some(1), Some(0))
    );
    assert_eq!(
        (
            first["broadcastId"].as_str(),
            second["broadcastId"].as_str()
        ),
        (Some(key.as_str()), Some(key.as_str()))
    );
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address = $1")
            .bind(&target)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(rows, 1, "the retry inserted no second row");

    let cases = [
        (
            json!({ "type": "made_up" }),
            "unknown notification type: made_up",
        ),
        (
            json!({ "type": "bid_received", "metadata": [1] }),
            "metadata must be an object",
        ),
        (
            json!({ "type": "bid_received", "addresses": [] }),
            "addresses must be non-empty when provided (omit it to broadcast to all)",
        ),
        (
            json!({ "type": "bid_received", "addresses": ["0x12"] }),
            "invalid address: 0x12",
        ),
        (
            json!({ "type": "bid_received", "broadcastId": "  " }),
            "broadcastId must be 1 to 128 characters",
        ),
        (
            json!({ "type": "bid_received", "broadcastId": "x".repeat(129) }),
            "broadcastId must be 1 to 128 characters",
        ),
    ];
    for (body, message) in cases {
        let (status, resp) = send(
            app(state.clone()),
            "POST",
            "/notifications/notifications/broadcast",
            bearer("s3cret"),
            Some(body),
        )
        .await;
        assert_eq!(
            (status, resp["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some(message))
        );
    }
}

#[tokio::test]
async fn set_email_and_confirm_email_validate_before_touching_the_mailer() {
    let Some(state) = state(Settings {
        email: super::EmailConfig {
            domain_blacklist: vec!["spam.com".into()],
            ..Settings::default().email
        },
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    // Its own wallet: the failed send below leaves a pending email on the signer, which
    // the shared test wallet's subscription round trip must never see.
    let wallet = alloy::signers::local::PrivateKeySigner::random();
    let signed = async |method: &str, path: &str| {
        let now = chrono::Utc::now().timestamp_millis().to_string();
        signed_headers_by(
            &wallet,
            method,
            path,
            r#"{"signer":"dcl:explorer"}"#,
            true,
            &now,
        )
        .await
    };
    for email in [
        json!("nope"),
        json!("@x.com"),
        json!("x@"),
        json!("a+b@x.com"),
        json!("me@Spam.com"),
        json!("a.b.c@x.com"),
        Value::Null,
    ] {
        let (status, body) = send(
            app(state.clone()),
            "PUT",
            "/notifications/set-email",
            signed("put", "/notifications/set-email").await,
            Some(json!({ "email": email })),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some(INVALID_EMAIL)),
            "{email}"
        );
    }
    let (status, _) = send(
        app(state.clone()),
        "PUT",
        "/notifications/set-email",
        signed("put", "/notifications/set-email").await,
        Some(json!({ "email": "" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "clearing the email sends nothing"
    );
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/set-email",
        signed("put", "/notifications/set-email").await,
        Some(json!({ "email": format!("new-{}@example.com", Uuid::new_v4().simple()) })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str(), body.get("message")),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("Internal Server Error"),
            None
        ),
        "an unconfigured mailer fails loudly"
    );

    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/confirm-email",
        HeaderMap::new(),
        Some(json!({ "code": "short" })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Missing address"))
    );
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/confirm-email",
        HeaderMap::new(),
        Some(json!({ "address": format!("{:#x}", wallet.address()), "code": "short" })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Missing code"))
    );
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/confirm-email",
        HeaderMap::new(),
        Some(json!({ "address": fresh_address().as_str(), "code": "a".repeat(32) })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("No unconfirmed email for this address")
        ),
        "no pending row"
    );
}

#[tokio::test]
async fn confirm_email_answers_the_old_messages_and_rechecks_the_blacklist() {
    let Some(state) = state(Settings {
        email: super::EmailConfig {
            domain_blacklist: vec!["spam.com".into()],
            ..Settings::default().email
        },
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let pool = state.store.pool().clone();
    let confirm = async |address: &Address, code: &str| {
        send(
            app(state.clone()),
            "PUT",
            "/notifications/confirm-email",
            HeaderMap::new(),
            Some(json!({ "address": address.as_str(), "code": code })),
        )
        .await
    };
    let code = "c".repeat(32);
    let (ok, blocked) = (fresh_address(), fresh_address());
    for (address, email) in [(&ok, "me@example.com"), (&blocked, "me@spam.com")] {
        sqlx::query("INSERT INTO social.unconfirmed_emails (address, email, code, source) VALUES ($1, $2, $3, 'account')")
            .bind(address)
            .bind(email)
            .bind(&code)
            .execute(&pool)
            .await
            .unwrap();
    }
    let (status, body) = confirm(&ok, &"d".repeat(32)).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Invalid code"))
    );
    let (status, body) = confirm(&blocked, &code).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some(INVALID_EMAIL))
    );
    let (status, _) = confirm(&ok, &code).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let sub = state.store.get_subscription(&ok).await.unwrap().unwrap();
    assert_eq!(sub.email.as_deref(), Some("me@example.com"));
}

const SIGNING_KEY: &str = "test-signing-key";

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

/// The old processor's `signUrl(key, new URL(path, SERVICE_BASE_URL))`, computed independently.
fn old_signed_path(path: &str) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    let base = Settings::default().service_base_url;
    let url = url::Url::parse(&base).unwrap().join(path).unwrap();
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(SIGNING_KEY.as_bytes()).unwrap();
    mac.update(url.as_str().as_bytes());
    format!(
        "{path}?signature={}",
        hex::encode(mac.finalize().into_bytes())
    )
}

async fn get_html(app: Router, path: &str) -> (StatusCode, String, String) {
    let req = Request::builder().uri(path).body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, ct, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn unsubscribe_links_verify_the_old_signature_and_update_the_subscription() {
    let Some(state) = state(Settings {
        signing_key: Some(SIGNING_KEY.into()),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let address = fresh_address();
    let one = format!("/unsubscribe/{}/bid_received", address.as_str());
    let all = format!("/unsubscribe/{}", address.as_str());

    for path in [
        all.clone(),
        format!("{all}?signature=00"),
        format!("{}x", old_signed_path(&all)),
        old_signed_path(&one).replace("bid_received", "bid_accepted"),
    ] {
        let (status, body) = send(
            app(state.clone()),
            "GET",
            &format!("/notifications{path}"),
            HeaderMap::new(),
            None,
        )
        .await;
        assert_eq!(
            (status, body),
            (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "Not Authorized", "message": "Invalid URL." })
            ),
            "{path}"
        );
    }

    let (status, ct, page) = get_html(
        app(state.clone()),
        &format!("/notifications{}", old_signed_path(&one)),
    )
    .await;
    assert_eq!((status, ct.as_str()), (StatusCode::OK, "text/html"));
    assert!(
        page.contains("email notifications of type bid_received"),
        "{page}"
    );
    assert!(page.contains("href=\"https://account.decentraland.org\""));
    let sub = state
        .store
        .get_subscription(&address)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        sub.details["message_type"]["bid_received"]["email"],
        json!(false)
    );
    assert_eq!(
        sub.details["message_type"]["bid_accepted"]["email"],
        json!(true)
    );
    assert_eq!(sub.details["ignore_all_email"], json!(false));

    let (status, ct, page) = get_html(
        app(state.clone()),
        &format!("/notifications{}", old_signed_path(&all)),
    )
    .await;
    assert_eq!((status, ct.as_str()), (StatusCode::OK, "text/html"));
    assert!(page.contains("unsubscribed from all email notifications"));
    assert!(!page.contains("{{"), "every placeholder is filled");
    let sub = state
        .store
        .get_subscription(&address)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sub.details["ignore_all_email"], json!(true));
    assert_eq!(
        sub.details["message_type"]["bid_received"]["email"],
        json!(false)
    );

    let bad = format!("/unsubscribe/{}/not_a_type", address.as_str());
    let (status, body) = send(
        app(state.clone()),
        "GET",
        &format!("/notifications{}", old_signed_path(&bad)),
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("Invalid notification type: not_a_type")
        )
    );
}

#[tokio::test]
async fn unsubscribe_without_a_signing_key_refuses_every_link() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let path = old_signed_path(&format!("/unsubscribe/{}", fresh_address().as_str()));
    let (status, _) = send(
        app(state),
        "GET",
        &format!("/notifications{path}"),
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn common_email_is_bearer_gated_and_validated() {
    let Some(state) = state(Settings {
        notification_service_token: Some("svc".into()),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let full = json!({ "email": "a@example.com", "subject": "s", "content": "c" });
    for (headers, message) in [
        (HeaderMap::new(), "Authorization header is missing"),
        (bearer("nope"), "Invalid authorization header"),
    ] {
        let (status, body) = send(
            app(state.clone()),
            "POST",
            "/notifications/notifications/email",
            headers,
            Some(full.clone()),
        )
        .await;
        assert_eq!(
            (status, body),
            (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "Not Authorized", "message": message })
            )
        );
    }
    for (body, message) in [
        (json!({ "subject": "s", "content": "c" }), "Missing email"),
        (
            json!({ "email": "a@example.com", "subject": "s" }),
            "Missing content",
        ),
        (
            json!({ "email": "a@example.com", "content": "c", "subject": "" }),
            "Missing subject",
        ),
    ] {
        let (status, resp) = send(
            app(state.clone()),
            "POST",
            "/notifications/notifications/email",
            bearer("svc"),
            Some(body),
        )
        .await;
        assert_eq!(
            (status, resp["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some(message))
        );
    }
    let (status, _) = send(
        app(state.clone()),
        "POST",
        "/notifications/notifications/email",
        bearer("svc"),
        Some(full),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unconfigured mailer fails loudly"
    );
}

#[tokio::test]
async fn opaque_and_uuid_opt_out_ids_work_with_custom_and_generic_plans_on_one_connection() {
    let Some(pool) = crate::db::scratch::pool(1).await else {
        return;
    };
    let store = super::NotificationsStore::new(pool.clone());
    let address = fresh_address();
    let scope_id = format!("legacy-{}", Uuid::new_v4());
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&pool)
        .await
        .unwrap();
    for setting in [
        "SET plan_cache_mode=force_custom_plan",
        "SET plan_cache_mode=force_generic_plan",
    ] {
        sqlx::query(setting).execute(&pool).await.unwrap();
        for _ in 0..8 {
            store
                .create_opt_out(&address, "community", &scope_id)
                .await
                .unwrap();
            assert!(
                store
                    .is_opted_out(&address, "community", &scope_id)
                    .await
                    .unwrap()
            );
            assert!(
                store
                    .delete_opt_out(&address, "community", &scope_id)
                    .await
                    .unwrap()
            );
            assert!(
                !store
                    .is_opted_out(&address, "community", &scope_id)
                    .await
                    .unwrap()
            );
            assert!(
                !store
                    .delete_opt_out(&address, "community", &scope_id)
                    .await
                    .unwrap()
            );
        }
        let community = Uuid::new_v4();
        let upper = community.to_string().to_uppercase();
        let canonical = community.to_string();
        let other = fresh_address();
        store
            .create_opt_out(&other, "community", &canonical)
            .await
            .unwrap();
        for _ in 0..8 {
            store
                .create_opt_out(&address, "community", &upper)
                .await
                .unwrap();
            assert!(
                store
                    .is_opted_out(&address, "community", &canonical)
                    .await
                    .unwrap()
            );
            assert!(
                store
                    .is_opted_out(&address, "community", &upper)
                    .await
                    .unwrap()
            );
            assert!(
                store
                    .delete_opt_out(&address, "community", &upper)
                    .await
                    .unwrap()
            );
            assert!(
                !store
                    .is_opted_out(&address, "community", &canonical)
                    .await
                    .unwrap()
            );
            assert!(
                store
                    .is_opted_out(&other, "community", &canonical)
                    .await
                    .unwrap()
            );
        }
        let current: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            current, pid,
            "all executions use the same PostgreSQL prepared-statement cache"
        );
    }
    sqlx::query("RESET plan_cache_mode")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}

#[tokio::test]
async fn direct_community_notices_honor_legacy_uppercase_opt_outs() {
    let Some(pool) = crate::db::scratch::pool(1).await else {
        return;
    };
    let address = fresh_address();
    let community = crate::CommunityId::new();
    sqlx::query("INSERT INTO social.subscription_opt_outs(address,scope,scope_id) VALUES($1,'community',$2)")
        .bind(&address).bind(community.to_string().to_uppercase()).execute(&pool).await.unwrap();
    let event = super::community::CommunityEvent {
        kind: "community_renamed",
        key: Uuid::new_v4().to_string(),
        community_id: community,
        metadata: json!({"communityId":community.to_string()}),
    };
    assert_eq!(
        super::community::publish(&pool, &event, super::community::Audience::One(&address))
            .await
            .unwrap(),
        0
    );
    super::NotificationsStore::new(pool.clone())
        .delete_opt_out(&address, "community", &community.to_string())
        .await
        .unwrap();
    assert_eq!(
        super::community::publish(&pool, &event, super::community::Audience::One(&address))
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn the_unsigned_unsubscribe_capture_replays() {
    let Some(state) = state(Settings {
        signing_key: Some(SIGNING_KEY.into()),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let name = "unsubscribe-address-no-signature";
    let fx = crate::test_fixtures::fixture("notifications", name);
    let notes = crate::test_fixtures::replay(&app(state), "/notifications", name, &fx).await;
    assert!(notes.is_empty(), "{notes:#?}");
}

const CAPTURES: &[&str] = &[
    "status",
    "notifications-unauth",
    "subscription-unauth",
    "opt-outs-community-unauth",
    "unknown-path-404",
];

/// Captures replayed with a fresh signature by a fresh wallet (empty inbox,
/// default subscription), as the throwaway wallet of the capture run.
const SIGNED_CAPTURES: &[&str] = &[
    "notifications-signed",
    "notifications-signed-filters",
    "subscription-signed",
    "opt-outs-community-signed",
    "opt-outs-bad-scope-signed",
];

#[tokio::test]
async fn the_notifications_host_answers_as_its_captures() {
    let Some(state) = state(Settings {
        current_version: Some("3.12.2".into()),
        commit_hash: Some("0f113fba0bfc33856b8e184474b99e9c28046370".into()),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let app = app(state);
    for name in CAPTURES {
        let mut fx = crate::test_fixtures::fixture("notifications", name);
        fx.volatile_paths.push("currentTime".into());
        let notes = crate::test_fixtures::replay(&app, "/notifications", name, &fx).await;
        assert!(notes.is_empty(), "{name}: {notes:#?}");
    }
    for name in SIGNED_CAPTURES {
        let mut fx = crate::test_fixtures::fixture("notifications", name);
        fx.volatile_paths.push("address".into());
        let wallet = alloy::signers::local::PrivateKeySigner::random();
        let path = format!("/notifications{}", fx.request.path);
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let metadata = r#"{"signer":"dcl:explorer"}"#;
        let auth = signed_headers_by(&wallet, "get", &path, metadata, true, &timestamp).await;
        let notes =
            crate::test_fixtures::replay_signed(&app, "/notifications", name, &fx, &auth).await;
        // friend_first_wear and land_transfer_received are this service's
        // own types; item_discounted postdates the capture.
        let expected: &[&str] = if *name == "subscription-signed" {
            &[
                "details.message_type.friend_first_wear extra",
                "details.message_type.item_discounted extra",
                "details.message_type.land_transfer_received extra",
            ]
        } else {
            &[]
        };
        assert_eq!(notes, expected, "{name}");
    }
}

/// A request with a raw body and an optional Content-Type.
async fn send_raw(
    app: Router,
    method: &str,
    path: &str,
    headers: HeaderMap,
    body: &str,
    content_type: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    if let Some(content_type) = content_type {
        req = req.header(header::CONTENT_TYPE, content_type);
    }
    let resp = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn bodies_are_json_whatever_the_content_type_and_refusals_keep_the_envelope() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let me = signer();
    let id = seed_notification(state.store.pool(), &me, 1_000, false).await;
    let read = "/notifications/notifications/read";
    let ids = format!(r#"{{"notificationIds":["{id}"]}}"#);
    for content_type in [None, Some("text/plain;charset=UTF-8")] {
        let (status, body) = send_raw(
            app(state.clone()),
            "PUT",
            read,
            signed("put", read).await,
            &ids,
            content_type,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{content_type:?}: {body}");
    }
    for (raw, message) in [
        ("{", "Invalid body"),
        ("{}", "Missing notificationIds"),
        (r#"{"notificationIds":[]}"#, "Missing notificationIds"),
        (r#"{"notificationIds":"x"}"#, "Missing notificationIds"),
    ] {
        let (status, body) = send_raw(
            app(state.clone()),
            "PUT",
            read,
            signed("put", read).await,
            raw,
            None,
        )
        .await;
        assert_eq!(
            (status, body),
            (
                StatusCode::BAD_REQUEST,
                json!({ "error": "Bad request", "message": message })
            ),
            "{raw}"
        );
    }

    // A handler on axum's `Json`: the header is supplied, the rejection enveloped.
    let confirm = "/notifications/confirm-email";
    let (status, body) = send_raw(
        app(state.clone()),
        "PUT",
        confirm,
        HeaderMap::new(),
        "{",
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "Bad request", "message": "Invalid body" })
        )
    );
    let (status, body) = send_raw(
        app(state.clone()),
        "PUT",
        confirm,
        HeaderMap::new(),
        r#"{"code":"x"}"#,
        Some("text/plain"),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Missing address"))
    );

    let (status, body) = send(
        app(state.clone()),
        "POST",
        "/notifications/subscription/opt-outs",
        signed("post", "/notifications/subscription/opt-outs").await,
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "message": "Invalid JSON body" })
        )
    );
}

#[tokio::test]
async fn only_mark_read_accepts_the_pre_6_signed_payload() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    // Uppercase metadata is where the folded legacy payload and 6.x differ.
    let metadata = r#"{"signer":"dcl:navbar","intent":"dcl:navbar:SeeNotifications"}"#;
    let legacy = |method: &'static str, path: &'static str| async move {
        signed_headers(method, path, metadata, false).await
    };
    let read = "/notifications/notifications/read";
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        read,
        legacy("put", read).await,
        Some(json!({ "notificationIds": [Uuid::new_v4().to_string()] })),
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "updated": 0 })));
    for path in [
        "/notifications/notifications",
        "/notifications/subscription",
    ] {
        let headers = legacy("get", path).await;
        let (status, body) = send(app(state.clone()), "GET", path, headers, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {body}");
    }
}
