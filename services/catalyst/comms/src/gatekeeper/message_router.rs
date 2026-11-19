//! The community-chat message router's join token: `POST /internal/message-router/token/v1`
//! ([`TOKEN_PATH`]). The router (`catalyst-message-router`) runs as its own process and holds no
//! LiveKit secret; it fetches its token here on every connection. The request is authenticated
//! with the dedicated `MESSAGE_ROUTER_KEY` (contract in `shared::message_router`), replay-protected
//! by nonce. The answer is one grant only: a join to the private-messages room for
//! `<MESSAGE_ROUTER_IDENTITY_PREFIX>-<replica>` that may subscribe and publish data, never a
//! track and never its own metadata. The route is mounted only when the key is configured;
//! the gateway keeps `/internal/message-router` off the public front.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio::time::Instant;

use crate::shared::livekit::{AccessToken, LiveKit, VideoGrants};
use crate::shared::message_router::{
    self as contract, HEADER_NONCE, HEADER_SIGNATURE, HEADER_TIMESTAMP, MAX_BODY, MAX_SKEW_SECS,
    TOKEN_PATH, TokenRequest, TokenResponse,
};
use crate::shared::routing::any;
use crate::shared::signed_request::{self, Replay, ReplayCache, one_header};

/// A nonce is remembered past both ends of the accepted clock window.
const REPLAY_TTL: Duration = Duration::from_secs(2 * MAX_SKEW_SECS + 1);
/// The router asks at start and on each reconnect; this bounds the replay table.
const MAX_REPLAYS: usize = 1024;

pub struct MessageRouterAuthority {
    key: [u8; 32],
    identity_prefix: String,
    room: String,
    livekit: LiveKit,
    replays: ReplayCache,
}

impl MessageRouterAuthority {
    /// `encoded_key` is `MESSAGE_ROUTER_KEY` (32 base64url bytes, distinct from the LiveKit
    /// secret); `identity_prefix` is `MESSAGE_ROUTER_IDENTITY_PREFIX`.
    pub fn new(
        encoded_key: &str,
        identity_prefix: &str,
        room: &str,
        livekit: LiveKit,
    ) -> anyhow::Result<Self> {
        let key = contract::parse_key(encoded_key, Some(livekit.api_secret.as_bytes()))?;
        anyhow::ensure!(
            contract::valid_identity_prefix(identity_prefix),
            "MESSAGE_ROUTER_IDENTITY_PREFIX must start with a lowercase letter and hold only \
             lowercase letters, digits and '-' (at most 64 bytes)"
        );
        anyhow::ensure!(!room.is_empty(), "the private messages room id is empty");
        Ok(Self {
            key,
            identity_prefix: identity_prefix.to_owned(),
            room: room.to_owned(),
            livekit,
            replays: ReplayCache::new(REPLAY_TTL, MAX_REPLAYS),
        })
    }

    async fn handle(&self, request: Request<Body>) -> Response {
        if request.method() != Method::POST {
            return refusal(StatusCode::METHOD_NOT_ALLOWED, "POST only");
        }
        if request.uri().path() != TOKEN_PATH || request.uri().query().is_some() {
            return refusal(StatusCode::NOT_FOUND, "not found");
        }
        let (parts, body) = request.into_parts();
        let Ok(body) = to_bytes(body, MAX_BODY).await else {
            return refusal(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
        };
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return refusal(StatusCode::SERVICE_UNAVAILABLE, "clock unavailable");
        };
        if let Err(response) = self.authenticate(&parts.headers, &body, now.as_secs()) {
            return response;
        }
        let Ok(TokenRequest { replica }) = serde_json::from_slice::<TokenRequest>(&body) else {
            return refusal(StatusCode::BAD_REQUEST, "body must be {\"replica\": <u16>}");
        };
        if !self.livekit.configured {
            return refusal(
                StatusCode::SERVICE_UNAVAILABLE,
                "LiveKit is not configured (LIVEKIT_API_KEY / LIVEKIT_API_SECRET unset)",
            );
        }
        let identity = contract::identity(&self.identity_prefix, replica);
        let token = match AccessToken::new(
            &self.livekit.api_key,
            &self.livekit.api_secret,
            &identity,
            router_grants(&self.room),
        )
        .to_jwt()
        {
            Ok(token) => token,
            Err(e) => {
                tracing::error!(error = %e, "message router token");
                return refusal(StatusCode::INTERNAL_SERVER_ERROR, "token unavailable");
            }
        };
        tracing::info!(%identity, room = %self.room, "minted the message router's join token");
        let answer = TokenResponse {
            url: self.livekit.client_url.clone(),
            token,
            room: self.room.clone(),
            identity,
        };
        (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "no-store")],
            axum::Json(answer),
        )
            .into_response()
    }

    fn authenticate(&self, headers: &HeaderMap, body: &[u8], now: u64) -> Result<(), Response> {
        let unauthorized = || refusal(StatusCode::UNAUTHORIZED, "unauthorized");
        let (Some(timestamp), Some(nonce_text), Some(signature)) = (
            one_header(headers, HEADER_TIMESTAMP),
            one_header(headers, HEADER_NONCE),
            one_header(headers, HEADER_SIGNATURE),
        ) else {
            return Err(unauthorized());
        };
        if !signed_request::fresh_timestamp(timestamp, now, MAX_SKEW_SECS) {
            return Err(unauthorized());
        }
        let Some(nonce) = signed_request::decode_fixed::<16>(nonce_text) else {
            return Err(unauthorized());
        };
        if !contract::verify(&self.key, timestamp, nonce_text, body, signature) {
            return Err(unauthorized());
        }
        match self.replays.admit(nonce, Instant::now()) {
            Ok(()) => {}
            Err(Replay::Seen) => return Err(unauthorized()),
            Err(Replay::Full) => return Err(refusal(StatusCode::SERVICE_UNAVAILABLE, "busy")),
        }
        Ok(())
    }
}

/// Join the private-messages room, subscribe and publish data: no track, no own metadata, no
/// room list.
pub fn router_grants(room: &str) -> VideoGrants {
    VideoGrants {
        room_join: true,
        room: room.to_owned(),
        can_publish: false,
        can_subscribe: true,
        can_publish_data: true,
        can_update_own_metadata: false,
        room_list: Some(false),
        can_publish_sources: None,
    }
}

/// The token route, or nothing when `MESSAGE_ROUTER_KEY` is unset; `serve` merges it at the root.
pub fn router<S: Clone + Send + Sync + 'static>(
    authority: Option<Arc<MessageRouterAuthority>>,
) -> Router<S> {
    let Some(authority) = authority else {
        return Router::new();
    };
    Router::new().route(
        TOKEN_PATH,
        any(move |request: Request<Body>| {
            let authority = Arc::clone(&authority);
            async move { authority.handle(request).await }
        }),
    )
}

fn refusal(status: StatusCode, message: &'static str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::livekit::decode_jwt_payload;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use tower::ServiceExt;

    const KEY: [u8; 32] = [3; 32];

    fn livekit(configured: bool) -> LiveKit {
        LiveKit {
            api_key: "APIrouter".into(),
            api_secret: "router-test-livekit-secret".into(),
            client_url: "wss://livekit.example.test".into(),
            api_url: "https://livekit.example.test".into(),
            configured,
        }
    }

    fn app(configured: bool) -> Router {
        let authority = MessageRouterAuthority::new(
            &URL_SAFE_NO_PAD.encode(KEY),
            "message-router-dev",
            "private-messages",
            livekit(configured),
        )
        .unwrap();
        router(Some(Arc::new(authority)))
    }

    fn now() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    fn signed(body: &str, timestamp: &str, nonce_byte: u8, key: &[u8; 32]) -> Request<Body> {
        let nonce = contract::nonce([nonce_byte; 16]);
        let signature = contract::sign(key, timestamp, &nonce, body.as_bytes());
        Request::post(TOKEN_PATH)
            .header("content-type", "application/json")
            .header(HEADER_TIMESTAMP, timestamp)
            .header(HEADER_NONCE, nonce)
            .header(HEADER_SIGNATURE, signature)
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    async fn call(app: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        let bytes = to_bytes(response.into_body(), 1 << 16).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn a_signed_request_gets_only_the_private_room_data_grant_for_the_fixed_identity() {
        let app = app(true);
        let (status, body) = call(&app, signed(r#"{"replica":0}"#, &now(), 1, &KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let answer: TokenResponse = serde_json::from_value(body).unwrap();
        assert_eq!(answer.url, "wss://livekit.example.test");
        assert_eq!(answer.room, "private-messages");
        assert_eq!(answer.identity, "message-router-dev-0");
        let claims = decode_jwt_payload(&answer.token);
        assert_eq!(claims["sub"], "message-router-dev-0");
        assert_eq!(claims["iss"], "APIrouter");
        assert_eq!(
            claims["video"],
            serde_json::json!({
                "roomJoin": true,
                "room": "private-messages",
                "canPublish": false,
                "canSubscribe": true,
                "canPublishData": true,
                "canUpdateOwnMetadata": false,
                "roomList": false,
            })
        );
        assert!(claims.get("metadata").is_none());

        let (status, body) = call(&app, signed(r#"{"replica":7}"#, &now(), 2, &KEY)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["identity"], "message-router-dev-7");
    }

    #[tokio::test]
    async fn every_request_without_a_valid_signature_is_refused() {
        let app = app(true);
        let body = r#"{"replica":0}"#;
        let unsigned = Request::post(TOKEN_PATH)
            .body(Body::from(body.to_owned()))
            .unwrap();
        assert_eq!(call(&app, unsigned).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            call(&app, signed(body, &now(), 3, &[4; 32])).await.0,
            StatusCode::UNAUTHORIZED
        );
        let stale = (now().parse::<u64>().unwrap() - MAX_SKEW_SECS - 5).to_string();
        assert_eq!(
            call(&app, signed(body, &stale, 4, &KEY)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let padded = format!("0{}", now());
        assert_eq!(
            call(&app, signed(body, &padded, 5, &KEY)).await.0,
            StatusCode::UNAUTHORIZED
        );
        // a signature over another body
        let mut tampered = signed(body, &now(), 6, &KEY);
        *tampered.body_mut() = Body::from(r#"{"replica":1}"#);
        assert_eq!(call(&app, tampered).await.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_replayed_nonce_is_refused() {
        let app = app(true);
        let timestamp = now();
        let first = signed(r#"{"replica":0}"#, &timestamp, 9, &KEY);
        let replay = signed(r#"{"replica":0}"#, &timestamp, 9, &KEY);
        assert_eq!(call(&app, first).await.0, StatusCode::OK);
        assert_eq!(call(&app, replay).await.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn the_body_names_a_replica_and_nothing_else() {
        let app = app(true);
        for (byte, body) in [
            (
                10,
                r#"{"replica":0,"identity":"0x1111111111111111111111111111111111111111"}"#,
            ),
            (11, r#"{"replica":-1}"#),
            (12, r#"{}"#),
            (13, "not json"),
        ] {
            assert_eq!(
                call(&app, signed(body, &now(), byte, &KEY)).await.0,
                StatusCode::BAD_REQUEST,
                "{body}"
            );
        }
        let large = format!(r#"{{"replica":0,"pad":"{}"}}"#, "x".repeat(MAX_BODY));
        assert_eq!(
            call(&app, signed(&large, &now(), 14, &KEY)).await.0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[tokio::test]
    async fn only_post_on_the_exact_path_and_only_with_livekit_configured() {
        let app = app(true);
        let get = Request::get(TOKEN_PATH).body(Body::empty()).unwrap();
        assert_eq!(call(&app, get).await.0, StatusCode::METHOD_NOT_ALLOWED);
        let unconfigured = super::tests::app(false);
        assert_eq!(
            call(&unconfigured, signed(r#"{"replica":0}"#, &now(), 15, &KEY))
                .await
                .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn without_a_key_nothing_is_mounted() {
        let response = router::<()>(None)
            .oneshot(signed(r#"{"replica":0}"#, &now(), 16, &KEY))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// With a key the identity prefix is required: a default would let a node mint an identity
    /// its Explorers do not trust and drop community chat without a trace.
    #[tokio::test]
    async fn a_key_without_an_identity_prefix_refuses_to_start() {
        use crate::gatekeeper::moderation::ban::test_support::{StubBanStatus, state};
        let fresh = || {
            state(
                crate::interfaces::test_support::StubPolicy::default(),
                StubBanStatus::default(),
            )
        };
        let key = URL_SAFE_NO_PAD.encode(KEY);
        for prefix in [None, Some(""), Some("  ")] {
            let error = fresh()
                .with_message_router_key(Some(&key), prefix)
                .err()
                .expect("refused")
                .to_string();
            assert!(error.contains("MESSAGE_ROUTER_IDENTITY_PREFIX"), "{error}");
        }
        assert!(
            fresh()
                .with_message_router_key(None, None)
                .unwrap()
                .message_router
                .is_none()
        );
    }

    #[test]
    fn the_key_and_prefix_are_checked() {
        let lk = livekit(true);
        assert!(
            MessageRouterAuthority::new("short", "message-router-dev", "r", lk.clone()).is_err()
        );
        let reused = URL_SAFE_NO_PAD.encode([b'k'; 32]);
        let mut same = lk.clone();
        same.api_secret = "k".repeat(32);
        assert!(MessageRouterAuthority::new(&reused, "message-router-dev", "r", same).is_err());
        for prefix in ["", "0x", "Message-Router"] {
            assert!(
                MessageRouterAuthority::new(&URL_SAFE_NO_PAD.encode(KEY), prefix, "r", lk.clone())
                    .is_err(),
                "{prefix}"
            );
        }
    }
}
