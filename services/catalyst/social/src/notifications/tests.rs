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

    let unread_path = format!("/notifications/notifications?from={base}&onlyUnread=true");
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
    assert_eq!(body["email"], Value::Null);
    assert!(body.get("unconfirmedEmail").is_none());
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
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["details"]["ignore_all_email"], json!(true));
    assert_eq!(
        body["details"]["message_type"]["item_sold"],
        json!({"email": false, "in_app": true})
    );
    let (status, body) = send(
        app(state.clone()),
        "PUT",
        "/notifications/subscription",
        signed("put", "/notifications/subscription").await,
        Some(json!({ "ignore_all_email": "yes" })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("ignore_all_email must be a boolean")
        )
    );

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
    assert_eq!((status, body), (StatusCode::CREATED, json!({ "ok": true })));
    let (status, body) = send(
        app(state.clone()),
        "POST",
        "/notifications/subscription/opt-outs",
        signed("post", "/notifications/subscription/opt-outs").await,
        Some(json!({ "scope": "world", "scopeId": "x" })),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("unsupported opt-out scope"))
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
    let cases = [
        ("nope", "Invalid email"),
        ("@x.com", "Invalid email"),
        ("x@", "Invalid email"),
        ("me@Spam.com", "Email domain not allowed"),
    ];
    for (email, message) in cases {
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
            (StatusCode::BAD_REQUEST, Some(message)),
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
        (StatusCode::BAD_REQUEST, Some("Invalid confirmation code"))
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
        (StatusCode::BAD_REQUEST, Some("Invalid confirmation code")),
        "no pending row"
    );
}
