//! The gatekeeper client: the signed fetches an explorer makes (scene adapters) and the moderator
//! calls (ban, unban), with errors that carry the HTTP status so a scenario can assert a 401 or a
//! 503.

use std::time::Duration;

use anyhow::{Result, anyhow};
use url::Url;

use crate::engine::Ctx;
use crate::engine::timeline::redact_text;
use crate::identity::{Identity, now_ms};

/// Where the API hangs the gatekeeper.
pub const PREFIX: &str = "/comms-gatekeeper";
/// The metadata `signer` of an explorer.
pub const EXPLORER_SIGNER: &str = "dcl:explorer";
/// The metadata `intent` of the signed fetch that asks for a scene room.
pub const SCENE_INTENT: &str = "dcl:explorer:comms-handshake";
/// Who a ban made by this client names as its moderator.
pub const MODERATOR_NAME: &str = "comms-e2e";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const SHOWN_BODY: usize = 200;

/// A gatekeeper call that did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatekeeperError {
    /// The gatekeeper answered with a status that is not a success.
    Status { status: u16, message: String },
    /// The request did not get an answer.
    Unreachable(String),
    /// The answer was not what the route documents, or the client could not be set up.
    Unexpected(String),
}

impl GatekeeperError {
    /// The HTTP status, when the gatekeeper answered.
    pub fn status(&self) -> Option<u16> {
        match self {
            GatekeeperError::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl std::fmt::Display for GatekeeperError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatekeeperError::Status { status, message } if message.is_empty() => {
                write!(formatter, "the gatekeeper answered {status}")
            }
            GatekeeperError::Status { status, message } => {
                write!(formatter, "the gatekeeper answered {status}: {message}")
            }
            GatekeeperError::Unreachable(why) => {
                write!(formatter, "the gatekeeper did not answer: {why}")
            }
            GatekeeperError::Unexpected(what) => {
                write!(formatter, "the gatekeeper answered unexpectedly: {what}")
            }
        }
    }
}

impl std::error::Error for GatekeeperError {}

/// The metadata a scene-adapter fetch signs, as the text the header carries.
pub fn scene_adapter_metadata(realm_name: &str, scene_id: &str) -> String {
    serde_json::json!({
        "signer": EXPLORER_SIGNER,
        "intent": SCENE_INTENT,
        "realmName": realm_name,
        "sceneId": scene_id,
    })
    .to_string()
}

/// The path a gatekeeper route is signed over: the path the request arrives on, prefix included.
pub fn signed_path(route: &str) -> String {
    format!("{PREFIX}{route}")
}

/// Whether the moderator token may be sent to `url`: over https to any host, over plain http only
/// to loopback. The error is the reason, for a ban or unban that is then not sent.
pub fn token_may_travel_to(url: &Url) -> Result<(), String> {
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    if url.scheme() == "https" || crate::stack::is_loopback_host(host) {
        Ok(())
    } else {
        Err(format!(
            "the moderator token is not sent over plain {} to a host that is not loopback: \
             give the stack file an https `api`",
            url.scheme()
        ))
    }
}

#[derive(Clone)]
pub struct Gatekeeper {
    http: reqwest::Client,
    base: Url,
    /// The moderator token, or why there is none: asked for only when a ban is made or lifted.
    moderator_token: Result<String, String>,
}

impl Gatekeeper {
    /// A client for the API of the stack file.
    pub fn new(ctx: &Ctx) -> Result<Gatekeeper> {
        let stack = ctx.stack();
        let api = stack
            .api
            .clone()
            .ok_or_else(|| anyhow!("the stack file has no API address (`api`)"))?;
        Ok(Gatekeeper {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            base: api,
            moderator_token: match stack.moderator_token_env.as_deref() {
                Some(variable) => stack.secret(variable).map_err(|error| error.to_string()),
                None => Err("the stack file names no moderator token".to_owned()),
            },
        })
    }

    fn url(&self, route: &str) -> Result<Url, GatekeeperError> {
        let path = signed_path(route);
        self.base
            .join(path.trim_start_matches('/'))
            .map_err(|error| GatekeeperError::Unexpected(format!("{route} is not a path: {error}")))
    }

    /// The scene-room adapter (`livekit:<url>?access_token=<jwt>`) for the wallet `identity`, as
    /// a signed fetch of `POST /comms-gatekeeper/get-scene-adapter`. The string carries a token.
    pub async fn scene_adapter(
        &self,
        identity: &Identity,
        realm_name: &str,
        scene_id: &str,
    ) -> Result<String, GatekeeperError> {
        let route = "/get-scene-adapter";
        let metadata = scene_adapter_metadata(realm_name, scene_id);
        let mut request = self.http.post(self.url(route)?);
        for (name, value) in identity.signed_fetch("post", &signed_path(route), &metadata, now_ms())
        {
            request = request.header(name, value);
        }
        let (status, body) = send(request).await?;
        if status != 200 {
            return Err(refusal(status, &body));
        }
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("adapter")
                    .and_then(|adapter| adapter.as_str().map(str::to_owned))
            })
            .ok_or_else(|| GatekeeperError::Unexpected("the reply has no `adapter`".to_owned()))
    }

    /// The moderator token, for a request to `url`.
    fn moderator_token(&self, url: &Url) -> Result<String, GatekeeperError> {
        token_may_travel_to(url).map_err(GatekeeperError::Unexpected)?;
        self.moderator_token
            .clone()
            .map_err(GatekeeperError::Unexpected)
    }

    fn ban_url(&self, address: &str) -> Result<Url, GatekeeperError> {
        let mut url = self.url(&format!("/users/{address}/bans"))?;
        url.query_pairs_mut()
            .append_pair("moderator", MODERATOR_NAME);
        Ok(url)
    }

    /// Bans `address` platform-wide (`POST /users/{address}/bans`) with the moderator token.
    /// Answers 409 when the wallet is already banned.
    pub async fn ban(&self, address: &str, reason: &str) -> Result<(), GatekeeperError> {
        let url = self.ban_url(address)?;
        let token = self.moderator_token(&url)?;
        let request = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "reason": reason }));
        let (status, body) = send(request).await?;
        match status {
            200 | 201 => Ok(()),
            _ => Err(refusal(status, &body)),
        }
    }

    /// Lifts the ban of `address` (`DELETE /users/{address}/bans`). Answers 404 when there is
    /// none, which a cleanup may ignore: see [`GatekeeperError::status`].
    pub async fn unban(&self, address: &str) -> Result<(), GatekeeperError> {
        let url = self.ban_url(address)?;
        let token = self.moderator_token(&url)?;
        let request = self.http.delete(url).bearer_auth(token);
        let (status, body) = send(request).await?;
        match status {
            200 | 204 => Ok(()),
            _ => Err(refusal(status, &body)),
        }
    }

    /// Whether `address` is banned right now (`GET /users/{address}/bans`, public).
    pub async fn is_banned(&self, address: &str) -> Result<bool, GatekeeperError> {
        let (status, body) =
            send(self.http.get(self.url(&format!("/users/{address}/bans"))?)).await?;
        if status != 200 {
            return Err(refusal(status, &body));
        }
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/data/isBanned")
                    .and_then(serde_json::Value::as_bool)
            })
            .ok_or_else(|| {
                GatekeeperError::Unexpected("the reply has no `data.isBanned`".to_owned())
            })
    }
}

async fn send(request: reqwest::RequestBuilder) -> Result<(u16, String), GatekeeperError> {
    let response = request
        .send()
        .await
        .map_err(|error| GatekeeperError::Unreachable(redact_text(&error.to_string())))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|error| GatekeeperError::Unreachable(redact_text(&error.to_string())))?;
    Ok((status, body))
}

/// A refusal with the start of the body, the way the gatekeeper words it, and no token.
pub fn refusal(status: u16, body: &str) -> GatekeeperError {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            ["message", "error"].iter().find_map(|key| {
                value
                    .get(key)
                    .and_then(|text| text.as_str().map(str::to_owned))
            })
        })
        .unwrap_or_else(|| body.to_owned());
    let message = redact_text(&message);
    let end = message
        .char_indices()
        .nth(SHOWN_BODY)
        .map_or(message.len(), |(index, _)| index);
    GatekeeperError::Status {
        status,
        message: message[..end].to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_moderator_token_goes_only_over_https_or_to_loopback() {
        for allowed in [
            "https://203.0.113.5:5210/comms-gatekeeper/users/0xa/bans",
            "https://stack.example/comms-gatekeeper/users/0xa/bans",
            "http://127.0.0.1:5210/comms-gatekeeper/users/0xa/bans",
            "http://localhost:5210/comms-gatekeeper/users/0xa/bans",
            "http://[::1]:5210/comms-gatekeeper/users/0xa/bans",
        ] {
            assert!(
                token_may_travel_to(&Url::parse(allowed).unwrap()).is_ok(),
                "{allowed}"
            );
        }
        for refused in [
            "http://203.0.113.5:5210/comms-gatekeeper/users/0xa/bans",
            "http://stack.example/comms-gatekeeper/users/0xa/bans",
            "http://10.0.0.1:5210/comms-gatekeeper/users/0xa/bans",
        ] {
            let reason = token_may_travel_to(&Url::parse(refused).unwrap()).unwrap_err();
            assert!(reason.contains("plain http"), "{refused}: {reason}");
            assert!(reason.contains("not loopback"), "{refused}: {reason}");
        }
    }

    #[tokio::test]
    async fn a_ban_over_plain_http_to_another_host_sends_nothing() {
        let mut stack = crate::stack::Stack::pulse_only("127.0.0.1:9".parse().unwrap());
        stack.api = Some("http://203.0.113.5:5210".parse().unwrap());
        stack.moderator_token_env = Some("CE2E_GATEKEEPER_TEST_TOKEN".to_owned());
        stack.give_secret("CE2E_GATEKEEPER_TEST_TOKEN", "a-moderator-token");
        let ctx = Ctx::new(
            std::sync::Arc::new(stack),
            "unit",
            "gatekeeper-http",
            0,
            1,
            std::env::temp_dir().join("comms-e2e-unit"),
        );
        let gatekeeper = Gatekeeper::new(&ctx).unwrap();
        // refused before any connection is made: the reason is the scheme, not the network
        for error in [
            gatekeeper.ban("0xa", "test").await.unwrap_err(),
            gatekeeper.unban("0xa").await.unwrap_err(),
        ] {
            assert!(error.to_string().contains("plain http"), "{error}");
            assert!(!error.to_string().contains("a-moderator-token"), "{error}");
        }
    }

    #[test]
    fn the_scene_metadata_names_what_the_gatekeeper_reads() {
        let metadata: serde_json::Value =
            serde_json::from_str(&scene_adapter_metadata("main", "scene-1")).unwrap();
        assert_eq!(metadata["signer"], "dcl:explorer");
        assert_eq!(metadata["intent"], "dcl:explorer:comms-handshake");
        assert_eq!(metadata["realmName"], "main");
        assert_eq!(metadata["sceneId"], "scene-1");
    }

    #[test]
    fn a_refusal_carries_the_status_and_the_gatekeepers_words() {
        let error = refusal(
            401,
            r#"{"error":"Access denied, invalid signed-fetch request"}"#,
        );
        assert_eq!(error.status(), Some(401));
        assert!(error.to_string().contains("Access denied"), "{error}");
        assert_eq!(refusal(503, "").status(), Some(503));
        assert_eq!(refusal(503, "").to_string(), "the gatekeeper answered 503");
        assert_eq!(GatekeeperError::Unreachable("x".into()).status(), None);
    }

    #[test]
    fn a_refusal_shows_no_token_and_a_bounded_body() {
        let error = refusal(500, "see livekit:ws://h?access_token=SECRETVALUE&x=1");
        assert!(!error.to_string().contains("SECRETVALUE"), "{error}");
        let long = refusal(500, &"é".repeat(1000)).to_string();
        assert!(long.chars().count() < 260, "{}", long.chars().count());
    }

    #[test]
    fn a_route_is_signed_with_its_gateway_prefix() {
        assert_eq!(
            signed_path("/get-scene-adapter"),
            "/comms-gatekeeper/get-scene-adapter"
        );
    }
}
