//! Gatekeeper-owned LiveKit possession proof verification.
use crate::protocol::room_authority::{MAX_CLAIMS_BYTES, PROOF_DOMAIN, address_bytes};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
type HmacSha256 = Hmac<Sha256>;
/// Claims are intentionally not Debug: authentication material must never reach logs.
#[derive(Clone, Deserialize, Serialize)]
pub struct RoomClaims {
    pub iss: String,
    pub sub: String,
    pub exp: u64,
    #[serde(default)]
    pub nbf: u64,
    pub video: RoomGrant,
    #[serde(default)]
    pub metadata: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomGrant {
    pub room: String,
    #[serde(default)]
    pub room_join: bool,
    #[serde(default)]
    pub can_publish_data: bool,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
}

#[derive(Clone)]
pub struct VerifiedRoom {
    pub claims: RoomClaims,
    pub wallet: String,
    pub session: String,
    pub is_guest: bool,
    pub header_payload: String,
    pub nonce: [u8; 32],
    pub proof: Vec<u8>,
}

pub struct ProofVerifier {
    api_key: String,
    secret: Vec<u8>,
}

impl ProofVerifier {
    pub fn new(api_key: String, secret: Vec<u8>) -> Result<Self, &'static str> {
        if api_key.is_empty() || secret.len() < 32 {
            return Err("application relay requires a LiveKit key and secret of at least 32 bytes");
        }
        Ok(Self { api_key, secret })
    }

    pub fn verify(
        &self,
        header_payload: &str,
        proof: &[u8],
        nonce: &[u8; 32],
        wallet: &str,
        session: &str,
        now_secs: u64,
    ) -> Result<VerifiedRoom, &'static str> {
        if header_payload.len() > MAX_CLAIMS_BYTES || proof.len() != 32 {
            return Err("invalid room proof");
        }
        let wallet_bytes = address_bytes(wallet).ok_or("invalid identity")?;
        let session_bytes = address_bytes(session).ok_or("invalid identity")?;
        let (header, payload) = header_payload.split_once('.').ok_or("invalid room proof")?;
        let header: Header = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(header)
                .map_err(|_| "invalid room proof")?,
        )
        .map_err(|_| "invalid room proof")?;
        if header.alg != "HS256" {
            return Err("invalid room proof");
        }
        let mut signature = HmacSha256::new_from_slice(&self.secret).map_err(|_| "invalid key")?;
        signature.update(header_payload.as_bytes());
        let signature = signature.finalize().into_bytes();
        let mut expected = HmacSha256::new_from_slice(&signature).map_err(|_| "invalid key")?;
        expected.update(PROOF_DOMAIN);
        expected.update(nonce);
        expected.update(&wallet_bytes);
        expected.update(&session_bytes);
        expected.update(&Sha256::digest(header_payload.as_bytes()));
        expected
            .verify_slice(proof)
            .map_err(|_| "invalid room proof")?;
        let claims: RoomClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload)
                .map_err(|_| "invalid room proof")?,
        )
        .map_err(|_| "invalid room proof")?;
        if claims.iss != self.api_key
            || claims.exp <= now_secs
            || claims.nbf > now_secs
            || !claims.video.room_join
            || !claims.video.can_publish_data
            || claims.video.room.is_empty()
            || claims.video.room.len() > 256
            || claims.video.room.chars().any(char::is_control)
            || (claims.sub != "authoritative-server"
                && address_bytes(&claims.sub) != Some(wallet_bytes))
        {
            return Err("room is not authorized");
        }
        let metadata = if claims.metadata.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str::<serde_json::Value>(&claims.metadata)
                .map_err(|_| "invalid room metadata")?
        };
        if let Some(island) = metadata.get("catalyrstIsland")
            && (island
                .get("wallet")
                .and_then(|v| v.as_str())
                .and_then(address_bytes)
                != Some(wallet_bytes)
                || island
                    .get("session")
                    .and_then(|v| v.as_str())
                    .and_then(address_bytes)
                    != Some(session_bytes))
        {
            return Err("room session is not authorized");
        }
        let is_guest = metadata
            .get("isGuest")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        Ok(VerifiedRoom {
            claims,
            wallet: wallet.to_ascii_lowercase(),
            session: session.to_ascii_lowercase(),
            is_guest,
            header_payload: header_payload.to_owned(),
            nonce: *nonce,
            proof: proof.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const WALLET: &str = "0x1111111111111111111111111111111111111111";
    const SESSION: &str = "0x2222222222222222222222222222222222222222";
    fn material(claims: serde_json::Value, nonce: [u8; 32]) -> (String, Vec<u8>) {
        let hp = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let mut signature = HmacSha256::new_from_slice(&[7; 32]).unwrap();
        signature.update(hp.as_bytes());
        let mut proof = HmacSha256::new_from_slice(&signature.finalize().into_bytes()).unwrap();
        proof.update(PROOF_DOMAIN);
        proof.update(&nonce);
        proof.update(&address_bytes(WALLET).unwrap());
        proof.update(&address_bytes(SESSION).unwrap());
        proof.update(&Sha256::digest(hp.as_bytes()));
        (hp, proof.finalize().into_bytes().to_vec())
    }
    fn claims() -> serde_json::Value {
        serde_json::json!({"iss":"key","sub":WALLET,"exp":100,
        "video":{"room":"scene-room","roomJoin":true,"canPublishData":true}})
    }
    #[test]
    fn proof_binds_exact_claims_connection_wallet_and_session() {
        let verifier = ProofVerifier::new("key".into(), vec![7; 32]).unwrap();
        let (hp, proof) = material(claims(), [3; 32]);
        assert!(
            verifier
                .verify(&hp, &proof, &[3; 32], WALLET, SESSION, 99)
                .is_ok()
        );
        assert!(
            verifier
                .verify(&hp, &proof, &[4; 32], WALLET, SESSION, 99)
                .is_err()
        );
        assert!(
            verifier
                .verify(&hp, &proof, &[3; 32], SESSION, SESSION, 99)
                .is_err()
        );
        assert!(
            verifier
                .verify(&hp, &proof, &[3; 32], WALLET, WALLET, 99)
                .is_err()
        );
        let (changed, _) = material(
            {
                let mut c = claims();
                c["video"]["room"] = "other".into();
                c
            },
            [3; 32],
        );
        assert!(
            verifier
                .verify(&changed, &proof, &[3; 32], WALLET, SESSION, 99)
                .is_err()
        );
    }
    #[test]
    fn admission_checks_grants_time_issuer_identity_and_fenced_session() {
        let verifier = ProofVerifier::new("key".into(), vec![7; 32]).unwrap();
        let mut cases = vec![];
        for (key, value) in [("iss", "wrong"), ("sub", SESSION)] {
            let mut c = claims();
            c[key] = value.into();
            cases.push(c);
        }
        let mut c = claims();
        c["exp"] = 99.into();
        cases.push(c);
        let mut c = claims();
        c["nbf"] = 100.into();
        cases.push(c);
        for key in ["roomJoin", "canPublishData"] {
            let mut c = claims();
            c["video"][key] = false.into();
            cases.push(c);
        }
        let mut c = claims();
        c["metadata"] = serde_json::json!({"catalyrstIsland":{"wallet":WALLET,"session":WALLET}})
            .to_string()
            .into();
        cases.push(c);
        for c in cases {
            let (hp, p) = material(c, [3; 32]);
            assert!(
                verifier
                    .verify(&hp, &p, &[3; 32], WALLET, SESSION, 99)
                    .is_err()
            );
        }
        let mut c = claims();
        c["sub"] = "authoritative-server".into();
        let (hp, p) = material(c, [3; 32]);
        let room = verifier
            .verify(&hp, &p, &[3; 32], WALLET, SESSION, 99)
            .unwrap();
        assert!(room.is_guest);
    }
    #[test]
    fn unicode_addresses_are_rejected_without_panicking() {
        assert_eq!(
            address_bytes("0x\u{e9}111111111111111111111111111111111111111"),
            None
        );
    }
    #[test]
    fn canonical_claims_codec_preserves_max_headers_and_every_existing_frame_profile() {
        use crate::protocol::pulse::decentraland::pulse::{
            ApplicationJoin, ClientMessage, client_message,
        };
        let verifier = ProofVerifier::new("key".into(), vec![7; 32]).unwrap();
        let world = crate::world_identity::WorldName::parse("w").unwrap();
        for original in [
            "\u{212a}".repeat(248),
            "\u{1e9e}".repeat(124),
            "A".repeat(248),
        ] {
            let scene = crate::world_identity::SceneId::parse(&original).unwrap();
            let canonical = crate::rooms::worlds_service_scene_room(&world, &scene);
            let legacy = format!("scene-w-{}", original.to_lowercase());
            assert_eq!(legacy.len(), 256);
            assert!(canonical.len() <= 256);
            let mut c = claims();
            c["video"]["room"] = legacy.into();
            let mut padding = 0;
            loop {
                c["name"] = "p".repeat(padding + 1).into();
                if material(c.clone(), [3; 32]).0.len() > MAX_CLAIMS_BYTES {
                    break;
                }
                padding += 1;
            }
            c["name"] = "p".repeat(padding).into();
            let (old_hp, old_proof) = material(c.clone(), [3; 32]);
            assert!(old_hp.len() >= MAX_CLAIMS_BYTES - 3);
            assert!(
                verifier
                    .verify(&old_hp, &old_proof, &[3; 32], WALLET, SESSION, 99)
                    .is_ok()
            );
            c["video"]["room"] = canonical.into();
            let (hp, proof) = material(c.clone(), [3; 32]);
            assert!(
                hp.len() <= old_hp.len(),
                "canonical room must not grow JSON/base64 claims"
            );
            assert!(
                verifier
                    .verify(&hp, &proof, &[3; 32], WALLET, SESSION, 99)
                    .is_ok()
            );
            let frame = |header: String| ClientMessage {
                message: Some(client_message::Message::ApplicationJoin(ApplicationJoin {
                    request_id: u32::MAX,
                    jwt_header_payload: header,
                    proof: vec![0; 32],
                })),
            };
            let old_frame = ::prost::Message::encoded_len(&frame(old_hp));
            let new_frame = ::prost::Message::encoded_len(&frame(hp));
            assert!(
                new_frame <= old_frame,
                "every frame profile that held the old token also holds the canonical token"
            );
            c["name"] = "p".repeat(padding + 100).into();
            let (overlong, proof) = material(c, [3; 32]);
            assert!(overlong.len() > MAX_CLAIMS_BYTES);
            assert!(
                verifier
                    .verify(&overlong, &proof, &[3; 32], WALLET, SESSION, 99)
                    .is_err(),
                "the 3072-byte bound is unchanged"
            );
        }
    }
}
