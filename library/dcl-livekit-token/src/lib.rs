//! LiveKit access tokens, minted offline: an HS256 JWT over the server's API
//! key and secret, as livekit-server's `/rtc/validate` accepts it. Shared by the
//! catalyst node (comms, worlds, social) and dcl-scene-cli's preview; neither
//! links a LiveKit SDK for this. Which grant a caller gets is policy and stays
//! with the caller.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum LivekitError {
    #[error("hmac key error: {0}")]
    HmacKey(String),
    #[error("clock skew before unix epoch")]
    Clock,
    #[error("json encode: {0}")]
    Json(#[from] serde_json::Error),
    #[error("malformed token: {0}")]
    Malformed(&'static str),
}

/// The wire string LiveKit compares `canPublishSources` against, NOT the
/// proto enum name: LiveKit lower-cases the proto source names before the
/// case-sensitive comparison, so `"MICROPHONE"` matches no source.
pub const TRACK_SOURCE_MICROPHONE: &str = "microphone";

pub fn sign_hs256(
    api_secret: &str,
    header_json: &[u8],
    payload_json: &[u8],
) -> Result<String, LivekitError> {
    let header_b64 = URL_SAFE_NO_PAD.encode(header_json);
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json);
    let signing_input = format!("{header_b64}.{payload_b64}");
    let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
        .map_err(|e| LivekitError::HmacKey(e.to_string()))?;
    mac.update(signing_input.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(format!("{signing_input}.{sig_b64}"))
}

pub fn jwt_header() -> serde_json::Value {
    serde_json::json!({ "alg": "HS256", "typ": "JWT" })
}

pub fn unix_now() -> Result<u64, LivekitError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LivekitError::Clock)?
        .as_secs())
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoGrants {
    #[serde(rename = "roomJoin")]
    pub room_join: bool,
    pub room: String,
    #[serde(rename = "canPublish")]
    pub can_publish: bool,
    #[serde(rename = "canSubscribe")]
    pub can_subscribe: bool,
    #[serde(rename = "canPublishData")]
    pub can_publish_data: bool,
    #[serde(rename = "canUpdateOwnMetadata")]
    pub can_update_own_metadata: bool,
    /// `None` omits the claim (the worlds grant shape); `Some` serializes it (comms').
    #[serde(rename = "roomList", skip_serializing_if = "Option::is_none")]
    pub room_list: Option<bool>,
    #[serde(rename = "canPublishSources", skip_serializing_if = "Option::is_none")]
    pub can_publish_sources: Option<Vec<String>>,
}

/// The comms join grant: publish/subscribe, no room list, and publishing
/// restricted to the microphone so a routine signed-fetch token can never
/// push camera or screen-share tracks into a scene or community room.
pub fn join_grants(room: impl Into<String>) -> VideoGrants {
    VideoGrants {
        room_join: true,
        room: room.into(),
        can_publish: true,
        can_subscribe: true,
        can_publish_data: true,
        can_update_own_metadata: true,
        room_list: Some(false),
        can_publish_sources: Some(vec![TRACK_SOURCE_MICROPHONE.to_string()]),
    }
}

pub struct AccessToken {
    pub api_key: String,
    pub api_secret: String,
    pub identity: String,
    pub name: Option<String>,
    pub metadata: Option<String>,
    pub grants: VideoGrants,
    pub ttl: Duration,
    /// How far before the minting second `nbf` sits, for verifiers whose clock
    /// runs behind this one.
    pub clock_skew: Duration,
}

impl AccessToken {
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        identity: impl Into<String>,
        grants: VideoGrants,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            identity: identity.into(),
            name: None,
            metadata: None,
            grants,
            ttl: Duration::from_secs(5 * 60),
            clock_skew: Duration::ZERO,
        }
    }

    pub fn with_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_clock_skew(mut self, skew: Duration) -> Self {
        self.clock_skew = skew;
        self
    }

    pub fn to_jwt(&self) -> Result<String, LivekitError> {
        self.to_jwt_at(unix_now()?)
    }

    /// The token as minted at `now` (unix seconds).
    pub fn to_jwt_at(&self, now: u64) -> Result<String, LivekitError> {
        let mut payload = serde_json::Map::new();
        payload.insert("exp".into(), serde_json::json!(now + self.ttl.as_secs()));
        payload.insert("iss".into(), serde_json::json!(self.api_key));
        payload.insert("sub".into(), serde_json::json!(self.identity));
        payload.insert(
            "nbf".into(),
            serde_json::json!(now.saturating_sub(self.clock_skew.as_secs())),
        );
        if let Some(n) = &self.name {
            payload.insert("name".into(), serde_json::json!(n));
        }
        if let Some(m) = &self.metadata {
            payload.insert("metadata".into(), serde_json::json!(m));
        }
        payload.insert("video".into(), serde_json::to_value(&self.grants)?);
        sign_hs256(
            &self.api_secret,
            &serde_json::to_vec(&jwt_header())?,
            &serde_json::to_vec(&serde_json::Value::Object(payload))?,
        )
    }
}

pub fn room_admin_token(
    api_key: &str,
    api_secret: &str,
    room: &str,
) -> Result<String, LivekitError> {
    let now = unix_now()?;
    let payload = serde_json::json!({
        "exp": now + 60,
        "iss": api_key,
        "sub": api_key,
        "nbf": now,
        "video": { "roomList": true, "roomAdmin": true, "room": room },
    });
    sign_hs256(
        api_secret,
        &serde_json::to_vec(&jwt_header())?,
        &serde_json::to_vec(&payload)?,
    )
}

/// The twirp Ingress admin token: `ingressAdmin` over every room, 60 s.
pub fn ingress_admin_token(api_key: &str, api_secret: &str) -> Result<String, LivekitError> {
    let now = unix_now()?;
    let payload = serde_json::json!({
        "exp": now + 60,
        "iss": api_key,
        "sub": api_key,
        "nbf": now,
        "video": { "ingressAdmin": true, "roomList": true },
    });
    sign_hs256(
        api_secret,
        &serde_json::to_vec(&jwt_header())?,
        &serde_json::to_vec(&payload)?,
    )
}

/// Re-signs a token [`AccessToken`] built, replacing only its `nbf`: on
/// LiveKit Cloud a replacement minted in the same second as the one being
/// revoked would be revoked with it unless its `nbf` moves to the boundary.
/// Self-hosted LiveKit does not enforce that cutoff.
pub fn with_not_before(
    jwt: &str,
    api_secret: &str,
    not_before_unix: u64,
) -> Result<String, LivekitError> {
    let mut parts = jwt.split('.');
    let (Some(header_b64), Some(payload_b64)) = (parts.next(), parts.next()) else {
        return Err(LivekitError::Malformed("not a JWT"));
    };
    let header = URL_SAFE_NO_PAD
        .decode(header_b64)
        .map_err(|_| LivekitError::Malformed("header is not base64url"))?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| LivekitError::Malformed("payload is not base64url"))?;
    let mut claims: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&payload)?;
    claims.insert("nbf".into(), serde_json::json!(not_before_unix));
    sign_hs256(
        api_secret,
        &header,
        &serde_json::to_vec(&serde_json::Value::Object(claims))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_jwt_payload(jwt: &str) -> serde_json::Value {
        let payload_b64 = jwt.split('.').nth(1).expect("jwt payload segment");
        let bytes = URL_SAFE_NO_PAD.decode(payload_b64).expect("base64url");
        serde_json::from_slice(&bytes).expect("payload json")
    }

    fn grants(room: &str) -> VideoGrants {
        VideoGrants {
            room_join: true,
            room: room.into(),
            can_publish: true,
            can_subscribe: true,
            can_publish_data: true,
            can_update_own_metadata: false,
            room_list: None,
            can_publish_sources: None,
        }
    }

    #[test]
    fn jwt_has_three_parts_and_omits_absent_claims() {
        let tok = AccessToken::new("k", "s", "0xabc", grants("world-foo.eth"))
            .to_jwt()
            .unwrap();
        assert_eq!(tok.split('.').count(), 3);
        let payload = decode_jwt_payload(&tok);
        assert!(payload["video"].get("roomList").is_none());
        assert!(payload["video"].get("canPublishSources").is_none());
        assert!(payload.get("metadata").is_none());
    }

    #[test]
    fn signature_matches_recomputed_hmac() {
        let tok = AccessToken::new("k", "secret", "0xabc", grants("r"))
            .to_jwt()
            .unwrap();
        let parts: Vec<&str> = tok.split('.').collect();
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let mut mac = HmacSha256::new_from_slice(b"secret").unwrap();
        mac.update(signing_input.as_bytes());
        let want = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        assert_eq!(parts[2], want);
    }

    #[test]
    fn private_voice_grants_match_upstream_generate_credentials() {
        let mut grants = join_grants("voice-chat-private-call1");
        grants.can_update_own_metadata = false;
        let p = decode_jwt_payload(
            &AccessToken::new("devkey", "devsecret", "0xabc", grants)
                .to_jwt()
                .unwrap(),
        );
        assert_eq!(p["iss"], "devkey");
        assert_eq!(p["sub"], "0xabc");
        let v = &p["video"];
        assert_eq!(v["roomJoin"], true);
        assert_eq!(v["room"], "voice-chat-private-call1");
        assert_eq!(v["canPublish"], true);
        assert_eq!(v["canSubscribe"], true);
        assert_eq!(v["canPublishData"], true);
        assert_eq!(v["canUpdateOwnMetadata"], false);
        assert_eq!(v["roomList"], false);
        assert_eq!(v["canPublishSources"], serde_json::json!(["microphone"]));
        assert!(p["exp"].as_i64().unwrap() > p["nbf"].as_i64().unwrap());
    }

    #[test]
    fn community_speaker_grant_is_restricted_to_microphone_and_listener_cannot_publish() {
        let mut grants = join_grants("voice-chat-community-c1");
        grants.can_update_own_metadata = false;
        let p = decode_jwt_payload(
            &AccessToken::new("devkey", "devsecret", "0xabc", grants)
                .with_metadata(r#"{"role":"owner","isSpeaker":true,"muted":false}"#)
                .to_jwt()
                .unwrap(),
        );
        assert_eq!(p["video"]["canPublish"], true);
        assert_eq!(
            p["video"]["canPublishSources"],
            serde_json::json!([TRACK_SOURCE_MICROPHONE])
        );
        assert_eq!(
            p["metadata"],
            r#"{"role":"owner","isSpeaker":true,"muted":false}"#
        );

        let mut grants = join_grants("voice-chat-community-c1");
        grants.can_publish = false;
        let v = decode_jwt_payload(
            &AccessToken::new("devkey", "devsecret", "0xabc", grants)
                .to_jwt()
                .unwrap(),
        )["video"]
            .clone();
        assert_eq!(v["canPublish"], false);
        assert_eq!(v["canSubscribe"], true);
    }

    #[test]
    fn room_admin_token_grants_room_admin_and_list() {
        let p = decode_jwt_payload(&room_admin_token("devkey", "devsecret", "some-room").unwrap());
        assert_eq!(p["iss"], "devkey");
        assert_eq!(p["sub"], "devkey");
        assert_eq!(p["video"]["roomAdmin"], true);
        assert_eq!(p["video"]["roomList"], true);
        assert_eq!(p["video"]["room"], "some-room");
        assert_eq!(p["exp"].as_i64().unwrap() - p["nbf"].as_i64().unwrap(), 60);
    }

    #[test]
    fn with_not_before_moves_only_the_nbf() {
        let minted = AccessToken::new("key", "secret", "0xabc", join_grants("island-c1"))
            .to_jwt()
            .unwrap();
        let moved = with_not_before(&minted, "secret", 1_700_000_000).unwrap();
        let (before, after) = (decode_jwt_payload(&minted), decode_jwt_payload(&moved));
        assert_eq!(after["nbf"], 1_700_000_000);
        assert_eq!(
            (
                after["sub"].clone(),
                after["exp"].clone(),
                after["video"].clone()
            ),
            (
                before["sub"].clone(),
                before["exp"].clone(),
                before["video"].clone()
            )
        );
    }
}
