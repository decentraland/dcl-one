//! The signed-fetch verifier of the worlds routes. The verifier itself
//! ([`require_verified`]: the shared primitives of `shared::signed_fetch`, a
//! per-route list of metadata keys that switches the folded-payload fallback
//! on, empty = the 6.x payload only, as upstream keeps the owner routes, and
//! the scene-signer gate that answers before any signature work) lives in
//! `comms::signed_fetch`, shared with the world comms mints in comms, and is
//! re-exported here; this module keeps the permissions route's key list and
//! the operator bearer. The `AuthChainError` of the old crate is
//! `SignedFetchError`; its variants and 400/401 split are the same.

use axum::http::HeaderMap;

// The verifier the worlds routes call, with the types it takes and returns.
pub use crate::comms::signed_fetch::{
    EXPLORER_METADATA_KEYS, VerifiedAuth, map_auth_error_api, require_verified,
};

use super::http::ApiError;

/// `POST /world/:name/permissions/:permission` only: creator-hub still signs the folded
/// payload here and `post_permissions` reads every one of these. A key list pins
/// spellings, never values, which is why this reaches one route.
pub const PERMISSIONS_METADATA_KEYS: &[&str] =
    &["signer", "type", "secret", "wallets", "communities", "nft"];

pub(crate) fn admin_authorized(expected: Option<&str>, headers: &HeaderMap) -> bool {
    crate::auth::admin_bearer::matches(expected, headers)
}

/// `CATALYRST_WORLDS_ADMIN_TOKEN`: unset fails closed, as the old routes did.
pub fn authorize_admin(expected: Option<&str>, headers: &HeaderMap) -> Result<(), ApiError> {
    if admin_authorized(expected, headers) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "You are not authorized to access this resource",
        ))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::auth::{AuthChain, AuthLink, AuthLinkType};
    use crate::shared::identity::Address;
    use crate::shared::signed_fetch::{
        AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
    };
    use alloy::signers::{Signer as _, local::PrivateKeySigner};

    pub(crate) struct Wallet {
        pub(crate) signer: PrivateKeySigner,
        pub(crate) address: Address,
    }

    impl Wallet {
        pub(crate) fn random() -> Self {
            let signer = PrivateKeySigner::random();
            let address = Address::parse(&format!("{:#x}", signer.address())).unwrap();
            Self { signer, address }
        }

        pub(crate) fn from_hex(key: &str) -> Self {
            let signer: PrivateKeySigner = key.parse().unwrap();
            let address = Address::parse(&format!("{:#x}", signer.address())).unwrap();
            Self { signer, address }
        }

        /// The checksummed spelling the explorers send.
        pub(crate) fn display(&self) -> String {
            self.address.checksummed()
        }

        pub(crate) async fn sign(&self, payload: &str) -> String {
            self.signer
                .sign_message(payload.as_bytes())
                .await
                .unwrap()
                .to_string()
        }

        /// A two-link chain over `payload` (an entity id or a signed-fetch payload).
        pub(crate) async fn chain(&self, payload: &str) -> AuthChain {
            vec![
                AuthLink {
                    link_type: AuthLinkType::SIGNER,
                    payload: self.display(),
                    signature: None,
                },
                AuthLink {
                    link_type: AuthLinkType::EcdsaSignedEntity,
                    payload: payload.to_string(),
                    signature: Some(self.sign(payload).await),
                },
            ]
        }
    }

    /// The signed-fetch headers over an already built payload.
    pub(crate) async fn headers_for(
        wallet: &Wallet,
        payload: &str,
        ts: &str,
        metadata: &str,
    ) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (i, link) in wallet.chain(payload).await.iter().enumerate() {
            headers.insert(
                axum::http::HeaderName::from_bytes(
                    format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes(),
                )
                .unwrap(),
                serde_json::to_string(link).unwrap().parse().unwrap(),
            );
        }
        headers.insert(AUTH_TIMESTAMP_HEADER, ts.parse().unwrap());
        headers.insert(AUTH_METADATA_HEADER, metadata.parse().unwrap());
        headers
    }

    pub(crate) fn now_ms() -> String {
        chrono::Utc::now().timestamp_millis().to_string()
    }

    /// Headers signed now, in the 6.x shape, over `method path` with `metadata`.
    pub(crate) async fn signed_headers(
        wallet: &Wallet,
        method: &str,
        path: &str,
        metadata: &str,
    ) -> HeaderMap {
        let ts = now_ms();
        let payload = build_payload_v6(method, path, &ts, metadata);
        headers_for(wallet, &payload, &ts, metadata).await
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::comms::signed_fetch::SIGNED_FETCH_EXPIRATION_SECS;
    use crate::shared::signed_fetch::{
        AUTH_TIMESTAMP_HEADER, ORIGINAL_PATH_HEADER, SCENE_SIGNER as KERNEL_SCENE_SIGNER,
        SignedFetchError, SignedPath, build_legacy_payload, build_payload_v6, is_refused_signer,
    };
    use serde_json::{Map, Value, json};

    const EXPLORER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const METHOD: &str = "post";
    const COMMS_PATH: &str = "/worlds/some.dcl.eth/comms";
    const PERMISSIONS_PATH: &str = "/world/some.dcl.eth/permissions/access";
    const SETTINGS_PATH: &str = "/world/some.dcl.eth/settings";

    fn permits(metadata: Value) -> bool {
        !is_refused_signer(&metadata)
    }

    fn message(err: &SignedFetchError) -> String {
        err.http_status_and_message().1
    }

    /// The handshake metadata the bevy, unity and godot explorers send: camelCase,
    /// so the folded payload and the delivered bytes differ.
    fn explorer_metadata(signer: &str) -> String {
        json!({
            "origin": "https://play.decentraland.org",
            "intent": "dcl:explorer:comms-handshake",
            "signer": signer,
            "isGuest": "false",
        })
        .to_string()
    }

    async fn verify(
        headers: &HeaderMap,
        path: &str,
        keys: &[&str],
    ) -> Result<VerifiedAuth, SignedFetchError> {
        require_verified(headers, METHOD, &SignedPath::local(path), keys, None).await
    }

    #[test]
    fn the_signed_fetch_window_is_upstreams_default_expiration() {
        assert_eq!(SIGNED_FETCH_EXPIRATION_SECS, 60);
    }

    #[test]
    fn every_route_policy_pins_the_gate_key() {
        for keys in [EXPLORER_METADATA_KEYS, PERMISSIONS_METADATA_KEYS] {
            assert!(keys.contains(&"signer"), "{keys:?} must pin the gate's key");
        }
    }

    #[test]
    fn scene_signer_is_refused_however_it_is_spelled() {
        for spelling in [
            "decentraland-kernel-scene",
            "Decentraland-Kernel-Scene",
            " decentraland-kernel-scene",
            "decentraland-kernel-scene ",
            "\tDECENTRALAND-KERNEL-SCENE\n",
        ] {
            assert!(
                !permits(json!({ "signer": spelling })),
                "{spelling:?} must not read as a user-signed request"
            );
        }
    }

    #[test]
    fn a_key_that_folds_to_signer_is_refused_rather_than_read_as_absent() {
        for key in ["Signer", "SIGNER", "sIgNeR"] {
            assert!(
                !permits(json!({ key: KERNEL_SCENE_SIGNER })),
                "{key:?} must not walk past the gate"
            );
        }
        assert!(!permits(
            json!({ "signer": "dcl:explorer", "Signer": KERNEL_SCENE_SIGNER })
        ));
    }

    #[test]
    fn other_signers_are_untouched() {
        assert!(permits(json!({ "signer": "dcl:explorer" })));
        assert!(permits(json!({ "signer": "0xabc" })));
        assert!(permits(json!({ "intent": "dcl:explorer:comms-handshake" })));
        assert!(permits(json!({})));
    }

    #[tokio::test]
    async fn accepts_a_v6_signed_handshake_with_cased_metadata() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let metadata = explorer_metadata("dcl:explorer");
        let payload = build_payload_v6(METHOD, COMMS_PATH, &ts, &metadata);
        let auth = verify(
            &headers_for(&wallet, &payload, &ts, &metadata).await,
            COMMS_PATH,
            EXPLORER_METADATA_KEYS,
        )
        .await
        .expect("v6-signed handshake must verify");
        assert_eq!(auth.signer, wallet.address);
    }

    #[tokio::test]
    async fn still_accepts_the_folded_payload_every_explorer_signs() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let metadata = explorer_metadata("dcl:explorer");
        let payload = build_legacy_payload(METHOD, COMMS_PATH, &ts, &metadata);
        let auth = verify(
            &headers_for(&wallet, &payload, &ts, &metadata).await,
            COMMS_PATH,
            EXPLORER_METADATA_KEYS,
        )
        .await
        .expect("legacy-signed handshake must still verify");
        assert_eq!(auth.signer, wallet.address);
        assert_eq!(auth.metadata["isGuest"], "false");
    }

    #[tokio::test]
    async fn refuses_a_legacy_handshake_that_respells_a_declared_key() {
        for (declared, respelled, guard) in [
            ("signer", "Signer", "invalid metadata content"),
            ("intent", "Intent", "expected \"intent\""),
            ("secret", "Secret", "expected \"secret\""),
        ] {
            let wallet = Wallet::from_hex(EXPLORER_KEY);
            let ts = now_ms();
            let mut object: Map<String, Value> =
                serde_json::from_str(&explorer_metadata("dcl:explorer")).unwrap();
            let value = object.remove(declared).unwrap_or_else(|| json!("hunter2"));
            object.insert(respelled.to_string(), value);
            let metadata = Value::Object(object).to_string();
            let payload = build_legacy_payload(METHOD, COMMS_PATH, &ts, &metadata);
            let err = verify(
                &headers_for(&wallet, &payload, &ts, &metadata).await,
                COMMS_PATH,
                EXPLORER_METADATA_KEYS,
            )
            .await
            .expect_err("a re-spelled declared key must not be served");
            assert!(
                matches!(err, SignedFetchError::MalformedChain { .. }),
                "{respelled:?} produced {err:?}"
            );
            assert!(
                message(&err).contains(guard),
                "{respelled:?} must be refused by the {guard:?} guard, got {}",
                message(&err)
            );
        }
    }

    #[tokio::test]
    async fn leaves_undeclared_legacy_keys_alone() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let metadata = json!({
            "Origin": "https://play.decentraland.org",
            "intent": "dcl:explorer:comms-handshake",
            "signer": "dcl:explorer",
            "IsGuest": "false",
        })
        .to_string();
        let payload = build_legacy_payload(METHOD, COMMS_PATH, &ts, &metadata);
        assert!(
            verify(
                &headers_for(&wallet, &payload, &ts, &metadata).await,
                COMMS_PATH,
                EXPLORER_METADATA_KEYS
            )
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn refuses_a_folded_signer_key_signed_in_the_current_format() {
        for key in ["Signer", "SIGNER", "sIgNeR"] {
            let wallet = Wallet::from_hex(EXPLORER_KEY);
            let ts = now_ms();
            let metadata = json!({
                "origin": "https://play.decentraland.org",
                "intent": "dcl:explorer:comms-handshake",
                "isGuest": "false",
                key: KERNEL_SCENE_SIGNER,
            })
            .to_string();
            let payload = build_payload_v6(METHOD, COMMS_PATH, &ts, &metadata);
            let err = verify(
                &headers_for(&wallet, &payload, &ts, &metadata).await,
                COMMS_PATH,
                EXPLORER_METADATA_KEYS,
            )
            .await
            .expect_err("a key folding to signer must not read as having no signer");
            assert!(
                matches!(err, SignedFetchError::MalformedChain { .. })
                    && message(&err).contains("invalid metadata content"),
                "{key:?} produced {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn refuses_a_folded_duplicate_beside_a_canonical_signer() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let metadata = json!({
            "origin": "https://play.decentraland.org",
            "intent": "dcl:explorer:comms-handshake",
            "signer": "dcl:explorer",
            "isGuest": "false",
            "Signer": KERNEL_SCENE_SIGNER,
        })
        .to_string();
        let payload = build_payload_v6(METHOD, COMMS_PATH, &ts, &metadata);
        let err = verify(
            &headers_for(&wallet, &payload, &ts, &metadata).await,
            COMMS_PATH,
            EXPLORER_METADATA_KEYS,
        )
        .await
        .expect_err("a folded duplicate must not be authorized on the exact key");
        assert!(message(&err).contains("invalid metadata content"));
    }

    #[tokio::test]
    async fn refuses_the_scene_signer_under_every_spelling() {
        for spelling in [
            KERNEL_SCENE_SIGNER,
            "Decentraland-Kernel-Scene",
            " decentraland-kernel-scene",
            "decentraland-kernel-scene ",
            "\tDECENTRALAND-KERNEL-SCENE",
        ] {
            let wallet = Wallet::from_hex(EXPLORER_KEY);
            let ts = now_ms();
            let metadata = explorer_metadata(spelling);
            let payload = build_payload_v6(METHOD, COMMS_PATH, &ts, &metadata);
            let err = verify(
                &headers_for(&wallet, &payload, &ts, &metadata).await,
                COMMS_PATH,
                EXPLORER_METADATA_KEYS,
            )
            .await
            .expect_err("a scene-signed request must never mint a comms token");
            assert!(
                message(&err).contains("invalid metadata content"),
                "signer {spelling:?} produced {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_scene_gate_answers_before_signature_verification() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let metadata = explorer_metadata(KERNEL_SCENE_SIGNER);
        let err = verify(
            &headers_for(&wallet, "not-the-payload", &ts, &metadata).await,
            COMMS_PATH,
            EXPLORER_METADATA_KEYS,
        )
        .await
        .expect_err("a scene-signed request must be refused");
        assert!(
            matches!(err, SignedFetchError::MalformedChain { .. }),
            "the gate must win over the signature check, got {err:?}"
        );
    }

    #[tokio::test]
    async fn a_stale_handshake_is_refused_before_the_gate_runs() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = (chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000).to_string();
        let metadata = explorer_metadata(KERNEL_SCENE_SIGNER);
        let payload = build_payload_v6(METHOD, COMMS_PATH, &ts, &metadata);
        let err = verify(
            &headers_for(&wallet, &payload, &ts, &metadata).await,
            COMMS_PATH,
            EXPLORER_METADATA_KEYS,
        )
        .await
        .expect_err("a ten-minute-old handshake must not be served");
        assert!(matches!(err, SignedFetchError::Expired { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn an_absent_timestamp_header_expires_like_an_empty_one() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let metadata = explorer_metadata(KERNEL_SCENE_SIGNER);
        let payload = build_payload_v6(METHOD, COMMS_PATH, "0", &metadata);
        let mut headers = headers_for(&wallet, &payload, "0", &metadata).await;
        headers.remove(AUTH_TIMESTAMP_HEADER);
        let err = verify(&headers, COMMS_PATH, EXPLORER_METADATA_KEYS)
            .await
            .expect_err("a request with no timestamp must not be served");
        assert!(
            matches!(err, SignedFetchError::Expired { signed_at: 0, .. }),
            "an absent timestamp produced {err:?}"
        );
    }

    #[tokio::test]
    async fn an_empty_timestamp_header_expires_instead_of_reading_as_malformed() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let metadata = explorer_metadata(KERNEL_SCENE_SIGNER);
        let payload = build_payload_v6(METHOD, COMMS_PATH, "", &metadata);
        let err = verify(
            &headers_for(&wallet, &payload, "", &metadata).await,
            COMMS_PATH,
            EXPLORER_METADATA_KEYS,
        )
        .await
        .expect_err("an empty timestamp must not be served");
        assert!(
            matches!(err, SignedFetchError::Expired { signed_at: 0, .. }),
            "an empty timestamp produced {err:?}"
        );
    }

    #[tokio::test]
    async fn accepts_the_folded_permissions_payload_creator_hub_signs() {
        for metadata in [
            json!({ "type": "shared-secret", "secret": "MySecret" }),
            json!({ "type": "allow-list", "wallets": ["0xAbC0000000000000000000000000000000000001"] }),
            json!({ "type": "allow-list", "wallets": [], "communities": ["Community-One"] }),
            json!({ "type": "nft-ownership", "nft": "urn:decentraland:matic:collections-v2:0xAbC" }),
        ] {
            let wallet = Wallet::from_hex(EXPLORER_KEY);
            let ts = now_ms();
            let raw = metadata.to_string();
            let payload = build_legacy_payload(METHOD, PERMISSIONS_PATH, &ts, &raw);
            let auth = verify(
                &headers_for(&wallet, &payload, &ts, &raw).await,
                PERMISSIONS_PATH,
                PERMISSIONS_METADATA_KEYS,
            )
            .await
            .unwrap_or_else(|err| panic!("{raw} must verify on the permissions route: {err:?}"));
            assert_eq!(auth.signer, wallet.address);
            assert_eq!(auth.metadata, metadata);
            assert_eq!(
                auth.secret(),
                metadata["secret"].as_str().map(str::to_string)
            );
        }
    }

    #[tokio::test]
    async fn refuses_a_legacy_permissions_request_that_respells_a_declared_key() {
        for (respelled, guard) in [
            ("Type", "expected \"type\""),
            ("Secret", "expected \"secret\""),
            ("Wallets", "expected \"wallets\""),
            ("Communities", "expected \"communities\""),
            ("Nft", "expected \"nft\""),
        ] {
            let wallet = Wallet::from_hex(EXPLORER_KEY);
            let ts = now_ms();
            let metadata = json!({ respelled: "shared-secret" }).to_string();
            let payload = build_legacy_payload(METHOD, PERMISSIONS_PATH, &ts, &metadata);
            let err = verify(
                &headers_for(&wallet, &payload, &ts, &metadata).await,
                PERMISSIONS_PATH,
                PERMISSIONS_METADATA_KEYS,
            )
            .await
            .expect_err("a re-spelled permissions key must not be served");
            assert!(
                message(&err).contains(guard),
                "{respelled:?} produced {}",
                message(&err)
            );
        }
    }

    #[tokio::test]
    async fn owner_routes_verify_the_current_format_only() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let metadata =
            json!({ "Wallets": ["0xAbC"], "origin": "https://catalyst.example.com" }).to_string();
        let payload = build_legacy_payload("put", SETTINGS_PATH, &ts, &metadata);
        let err = require_verified(
            &headers_for(&wallet, &payload, &ts, &metadata).await,
            "put",
            &SignedPath::local(SETTINGS_PATH),
            &[],
            None,
        )
        .await
        .expect_err("a folded signature over cased metadata has no fallback here");
        assert!(
            matches!(err, SignedFetchError::InvalidSignature(_)),
            "the strict route must answer 401, got {err:?}"
        );

        let metadata = json!({ "Signer": "dcl:explorer" }).to_string();
        let payload = build_legacy_payload("put", SETTINGS_PATH, &ts, &metadata);
        let err = require_verified(
            &headers_for(&wallet, &payload, &ts, &metadata).await,
            "put",
            &SignedPath::local(SETTINGS_PATH),
            &[],
            None,
        )
        .await
        .expect_err("a re-spelled signer must be refused on every route");
        assert!(message(&err).contains("invalid metadata content"));

        let payload = build_legacy_payload("put", SETTINGS_PATH, &ts, "{}");
        let auth = require_verified(
            &headers_for(&wallet, &payload, &ts, "{}").await,
            "put",
            &SignedPath::local(SETTINGS_PATH),
            &[],
            None,
        )
        .await
        .expect("the folded payload over {} is the 6.x payload");
        assert_eq!(auth.signer, wallet.address);
    }

    #[tokio::test]
    async fn an_empty_metadata_request_verifies_on_the_current_format() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let payload = build_payload_v6("put", SETTINGS_PATH, &ts, "{}");
        assert_eq!(
            payload,
            build_legacy_payload("put", SETTINGS_PATH, &ts, "{}")
        );
        let auth = require_verified(
            &headers_for(&wallet, &payload, &ts, "{}").await,
            "put",
            &SignedPath::local(SETTINGS_PATH),
            &[],
            None,
        )
        .await
        .expect("an empty-metadata request must verify with no fallback declared");
        assert_eq!(auth.signer, wallet.address);
    }

    #[tokio::test]
    async fn a_forwarded_original_path_is_accepted_only_when_it_ends_with_the_route() {
        let wallet = Wallet::from_hex(EXPLORER_KEY);
        let ts = now_ms();
        let public = format!("/worlds-content-server{SETTINGS_PATH}");
        let payload = build_payload_v6("put", &public, &ts, "{}");
        let mut headers = headers_for(&wallet, &payload, &ts, "{}").await;
        headers.insert(
            ORIGINAL_PATH_HEADER,
            format!("{public}?x=1").parse().unwrap(),
        );
        assert!(
            require_verified(
                &headers,
                "put",
                &SignedPath::local(SETTINGS_PATH),
                &[],
                None
            )
            .await
            .is_ok()
        );
        headers.insert(ORIGINAL_PATH_HEADER, "/other/route".parse().unwrap());
        let err = require_verified(
            &headers,
            "put",
            &SignedPath::local(SETTINGS_PATH),
            &[],
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, SignedFetchError::InvalidSignature(_)));
    }

    fn with_auth(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("authorization", value.parse().unwrap());
        h
    }

    #[test]
    fn fails_closed_when_token_unset() {
        assert!(!admin_authorized(None, &with_auth("Bearer anything")));
        assert!(!admin_authorized(None, &HeaderMap::new()));
        assert!(!admin_authorized(None, &with_auth("Bearer ")));
    }

    #[test]
    fn rejects_missing_or_wrong_bearer() {
        let expected = Some("s3cret");
        assert!(!admin_authorized(expected, &HeaderMap::new()));
        assert!(!admin_authorized(expected, &with_auth("Bearer wrong")));
        assert!(!admin_authorized(expected, &with_auth("s3cret")));
        assert!(!admin_authorized(expected, &with_auth("Basic s3cret")));
        assert!(!admin_authorized(expected, &with_auth("Bearer ")));
    }

    #[test]
    fn accepts_matching_bearer_and_refuses_an_empty_secret() {
        assert!(admin_authorized(
            Some("s3cret"),
            &with_auth("Bearer s3cret")
        ));
        assert!(!admin_authorized(Some(""), &with_auth("Bearer ")));
        assert!(!admin_authorized(Some(""), &HeaderMap::new()));
        assert_eq!(
            authorize_admin(None, &HeaderMap::new())
                .unwrap_err()
                .status(),
            403
        );
    }
}
