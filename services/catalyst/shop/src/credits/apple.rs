// from: credits-server@d9c6a261f:src/logic/apple-store-notifications.ts, src/logic/iap-account-token.ts; @apple/app-store-server-library@3.1.0:dist/jws_verification.js (verifyJWT, verifyCertificateChainWithoutCaching, checkDates)
//! App Store signed data: the server notifications (`signedPayload`) and the
//! StoreKit transactions (`jwsRepresentation`) are JWS signed with ES256 by a
//! leaf certificate whose chain (`x5c`: leaf, intermediate, root) must end in
//! one of the configured Apple root certificates. A payload is trusted only
//! when:
//!
//! - the header names `ES256` and carries exactly three certificates;
//! - the intermediate is a CA signed by a configured root (the `x5c` root is
//!   never trusted on its own) and the leaf is signed by the intermediate,
//!   each issuer matching its signer's subject;
//! - the leaf carries Apple's receipt-signing marker
//!   (`1.2.840.113635.100.6.11.1`) and the intermediate Apple's WWDR marker
//!   (`1.2.840.113635.100.6.2.1`);
//! - every certificate is valid now (60 s skew);
//! - the leaf's P-256 key verifies the signature over `header.payload`;
//! - the bundle id and environment (and, in production, the app's Apple id)
//!   match the configuration. A notification's nested
//!   `signedTransactionInfo` is verified the same way on its own;
//! - a notification's `signedDate` is at most [`NOTIFICATION_MAX_AGE_SECS`]
//!   old and at most [`NOTIFICATION_MAX_FUTURE_SECS`] ahead, and it carries a
//!   `notificationUUID`.
//!
//! OCSP revocation checks of the chain are not performed.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use x509_parser::certificate::X509Certificate;
use x509_parser::der_parser::oid::Oid;
use x509_parser::prelude::FromDer;

const MAX_SKEW_SECS: i64 = 60;
/// How old a notification's `signedDate` may be. A retry keeps the original
/// `signedDate`, and Apple retries an undelivered V2 notification five times,
/// at 1, 12, 24, 48 and 72 hours after the previous attempt (157 hours in all),
/// so 7 days keeps every retry.
pub const NOTIFICATION_MAX_AGE_SECS: i64 = 7 * 24 * 3600;
/// How far ahead of this node's clock a notification's `signedDate` may be;
/// one further ahead is answered for a retry (the node's clock is behind).
pub const NOTIFICATION_MAX_FUTURE_SECS: i64 = 5 * 60;

/// Whether a `signedDate` is more than [`NOTIFICATION_MAX_FUTURE_SECS`] ahead of `now_ms`.
pub fn signed_ahead(signed_date_ms: i64, now_ms: i64) -> bool {
    signed_date_ms > now_ms.saturating_add(NOTIFICATION_MAX_FUTURE_SECS * 1000)
}

/// How long `credits.iap_notifications` keeps a record: the accepted age plus
/// a day, so a node whose clock differs from the purging node's still finds it.
pub const NOTIFICATION_RECORD_SECS: i64 = NOTIFICATION_MAX_AGE_SECS + 24 * 3600;
const LEAF_MARKER: &[u64] = &[1, 2, 840, 113_635, 100, 6, 11, 1];
const INTERMEDIATE_MARKER: &[u64] = &[1, 2, 840, 113_635, 100, 6, 2, 1];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    /// `APPLE_ENVIRONMENT`: `production` or `sandbox`, with no default.
    pub fn parse(raw: Option<&str>) -> anyhow::Result<Self> {
        match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("sandbox") => Ok(Self::Sandbox),
            Some("production") => Ok(Self::Production),
            None | Some("") => {
                anyhow::bail!(
                    "APPLE_ENVIRONMENT (production or sandbox) is required with APPLE_BUNDLE_ID"
                )
            }
            Some(_) => anyhow::bail!("APPLE_ENVIRONMENT must be production or sandbox"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Production => "Production",
            Self::Sandbox => "Sandbox",
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VerifyError {
    #[error("signed payload is malformed")]
    Malformed,
    #[error("certificate chain is not an App Store chain")]
    Chain,
    #[error("signature does not verify")]
    Signature,
    #[error("signed for another app")]
    App,
    #[error("signed for another environment")]
    Environment,
    /// Signed more than [`NOTIFICATION_MAX_AGE_SECS`] ago: a retry carries the same date.
    #[error("signedDate is older than the accepted window")]
    Expired {
        notification_uuid: String,
        signed_date_ms: i64,
    },
    /// Signed more than [`NOTIFICATION_MAX_FUTURE_SECS`] ahead of this node's clock.
    #[error("signedDate is ahead of this node's clock")]
    Ahead {
        notification_uuid: String,
        signed_date_ms: i64,
    },
}

impl VerifyError {
    /// The verified uuid and `signedDate` of a notification outside the window.
    pub fn out_of_window(&self) -> Option<(&str, i64)> {
        match self {
            Self::Expired {
                notification_uuid,
                signed_date_ms,
            }
            | Self::Ahead {
                notification_uuid,
                signed_date_ms,
            } => Some((notification_uuid, *signed_date_ms)),
            _ => None,
        }
    }
}

/// The fields of a verified transaction the credits flow reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub transaction_id: String,
    pub original_transaction_id: Option<String>,
    pub product_id: String,
    pub quantity: i64,
    pub app_account_token: Option<String>,
    /// Apple revoked it (a refund): `revocationDate` or `revocationReason` is present.
    pub revoked: bool,
    /// The share Apple refunded, in thousandths of a percent (`revocationPercentage`); `None` is all of it.
    pub revocation_percentage: Option<i64>,
    /// `signedDate`, in milliseconds since the epoch.
    pub signed_date_ms: Option<i64>,
}

/// A verified notification; `transaction` is `None` for one that carries none (TEST, summaries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub notification_type: String,
    pub subtype: Option<String>,
    pub notification_uuid: String,
    /// `signedDate`, in milliseconds since the epoch.
    pub signed_date_ms: i64,
    pub transaction: Option<Transaction>,
}

/// Verifies App Store signed data against the configured roots and app.
#[derive(Clone)]
pub struct Verifier {
    roots: Vec<Vec<u8>>,
    bundle_id: String,
    app_apple_id: Option<i64>,
    environment: Environment,
}

impl Verifier {
    /// `roots` are DER certificates; production requires the app's Apple id.
    pub fn new(
        roots: Vec<Vec<u8>>,
        bundle_id: String,
        app_apple_id: Option<i64>,
        environment: Environment,
    ) -> anyhow::Result<Self> {
        if roots.is_empty() {
            anyhow::bail!("APPLE_ROOT_CERTS_DIR holds no .cer or .der certificate");
        }
        for der in &roots {
            match X509Certificate::from_der(der) {
                Ok(([], _)) => {}
                _ => {
                    anyhow::bail!("APPLE_ROOT_CERTS_DIR holds a file that is not a DER certificate")
                }
            }
        }
        if bundle_id.trim().is_empty() {
            anyhow::bail!("APPLE_BUNDLE_ID is required");
        }
        if environment == Environment::Production && app_apple_id.is_none() {
            anyhow::bail!("APPLE_APP_APPLE_ID is required when APPLE_ENVIRONMENT is production");
        }
        Ok(Self {
            roots,
            bundle_id: bundle_id.trim().to_string(),
            app_apple_id,
            environment,
        })
    }

    /// Verifies a server notification and its nested transaction.
    pub fn notification(&self, signed: &str, now: i64) -> Result<Notification, VerifyError> {
        let payload = self.verify_jws(signed, now)?;
        let signed_date_ms = payload
            .get("signedDate")
            .and_then(JsonValue::as_i64)
            .ok_or(VerifyError::Malformed)?;
        let notification_uuid = str_field(&payload, "notificationUUID")
            .filter(|s| !s.is_empty())
            .ok_or(VerifyError::Malformed)?
            .to_string();
        let app = ["data", "summary", "appData"]
            .iter()
            .find_map(|k| payload.get(*k).filter(|v| v.is_object()))
            .ok_or(VerifyError::App)?;
        if str_field(app, "bundleId") != Some(self.bundle_id.as_str())
            || (self.environment == Environment::Production
                && app.get("appAppleId").and_then(JsonValue::as_i64) != self.app_apple_id)
        {
            return Err(VerifyError::App);
        }
        if str_field(app, "environment") != Some(self.environment.as_str()) {
            return Err(VerifyError::Environment);
        }
        let now_ms = now.saturating_mul(1000);
        if signed_date_ms < now_ms - NOTIFICATION_MAX_AGE_SECS * 1000 {
            return Err(VerifyError::Expired {
                notification_uuid,
                signed_date_ms,
            });
        }
        if signed_ahead(signed_date_ms, now_ms) {
            return Err(VerifyError::Ahead {
                notification_uuid,
                signed_date_ms,
            });
        }
        let transaction = match payload
            .get("data")
            .and_then(|d| str_field(d, "signedTransactionInfo"))
        {
            Some(inner) => Some(self.transaction(inner, now)?),
            None => None,
        };
        Ok(Notification {
            notification_type: str_field(&payload, "notificationType")
                .ok_or(VerifyError::Malformed)?
                .to_string(),
            subtype: str_field(&payload, "subtype").map(str::to_string),
            notification_uuid,
            signed_date_ms,
            transaction,
        })
    }

    /// Verifies a StoreKit transaction (`signedTransactionInfo` or a device's `jwsRepresentation`).
    pub fn transaction(&self, signed: &str, now: i64) -> Result<Transaction, VerifyError> {
        let p = self.verify_jws(signed, now)?;
        if str_field(&p, "bundleId") != Some(self.bundle_id.as_str()) {
            return Err(VerifyError::App);
        }
        if str_field(&p, "environment") != Some(self.environment.as_str()) {
            return Err(VerifyError::Environment);
        }
        let required = |k: &str| {
            str_field(&p, k)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or(VerifyError::Malformed)
        };
        Ok(Transaction {
            transaction_id: required("transactionId")?,
            original_transaction_id: str_field(&p, "originalTransactionId").map(str::to_string),
            product_id: required("productId")?,
            quantity: p.get("quantity").and_then(JsonValue::as_i64).unwrap_or(1),
            app_account_token: str_field(&p, "appAccountToken").map(str::to_string),
            revoked: !p.get("revocationDate").is_none_or(JsonValue::is_null)
                || !p.get("revocationReason").is_none_or(JsonValue::is_null),
            revocation_percentage: p
                .get("revocationPercentage")
                .and_then(JsonValue::as_i64)
                .filter(|v| (1..100_000).contains(v)),
            signed_date_ms: p.get("signedDate").and_then(JsonValue::as_i64),
        })
    }

    fn verify_jws(&self, jws: &str, now: i64) -> Result<JsonValue, VerifyError> {
        let mut parts = jws.split('.');
        let (Some(header_b64), Some(payload_b64), Some(sig_b64), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(VerifyError::Malformed);
        };
        let header: JsonValue = decode_json(header_b64)?;
        if str_field(&header, "alg") != Some("ES256") {
            return Err(VerifyError::Malformed);
        }
        let chain = header
            .get("x5c")
            .and_then(JsonValue::as_array)
            .filter(|c| c.len() == 3)
            .ok_or(VerifyError::Chain)?;
        let der = |i: usize| {
            chain[i]
                .as_str()
                .and_then(|s| STANDARD.decode(s).ok())
                .ok_or(VerifyError::Chain)
        };
        let (leaf_der, intermediate_der) = (der(0)?, der(1)?);
        let leaf = parse_cert(&leaf_der)?;
        let intermediate = parse_cert(&intermediate_der)?;

        let root = self
            .roots
            .iter()
            .filter_map(|r| parse_cert(r).ok())
            .find(|root| {
                intermediate.issuer().as_raw() == root.subject().as_raw()
                    && intermediate
                        .verify_signature(Some(root.public_key()))
                        .is_ok()
            })
            .ok_or(VerifyError::Chain)?;
        if leaf.issuer().as_raw() != intermediate.subject().as_raw()
            || leaf
                .verify_signature(Some(intermediate.public_key()))
                .is_err()
            || !intermediate
                .basic_constraints()
                .ok()
                .flatten()
                .is_some_and(|bc| bc.value.ca)
            || !has_extension(&leaf, LEAF_MARKER)
            || !has_extension(&intermediate, INTERMEDIATE_MARKER)
            || ![&leaf, &intermediate, &root]
                .iter()
                .all(|c| valid_at(c, now))
        {
            return Err(VerifyError::Chain);
        }

        let signature = URL_SAFE_NO_PAD
            .decode(sig_b64)
            .map_err(|_| VerifyError::Malformed)?;
        let signing_input = &jws[..header_b64.len() + 1 + payload_b64.len()];
        ring::signature::UnparsedPublicKey::new(
            &ring::signature::ECDSA_P256_SHA256_FIXED,
            &leaf.public_key().subject_public_key.data,
        )
        .verify(signing_input.as_bytes(), &signature)
        .map_err(|_| VerifyError::Signature)?;
        decode_json(payload_b64)
    }
}

fn parse_cert(der: &[u8]) -> Result<X509Certificate<'_>, VerifyError> {
    match X509Certificate::from_der(der) {
        Ok(([], cert)) => Ok(cert),
        _ => Err(VerifyError::Chain),
    }
}

fn has_extension(cert: &X509Certificate<'_>, oid: &[u64]) -> bool {
    let Ok(oid) = Oid::from(oid) else {
        return false;
    };
    cert.extensions().iter().any(|e| e.oid == oid)
}

fn valid_at(cert: &X509Certificate<'_>, now: i64) -> bool {
    let v = cert.validity();
    v.not_before.timestamp() <= now + MAX_SKEW_SECS
        && v.not_after.timestamp() >= now - MAX_SKEW_SECS
}

fn decode_json(b64: &str) -> Result<JsonValue, VerifyError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|_| VerifyError::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| VerifyError::Malformed)
}

fn str_field<'a>(v: &'a JsonValue, key: &str) -> Option<&'a str> {
    v.get(key).and_then(JsonValue::as_str)
}

/// The `appAccountToken` the iOS app derives from a wallet: the first 16
/// bytes of `SHA-256("dcl-iap:" + lowercase wallet)` as a UUID string (no
/// version or variant bits set), byte-identical to the app's derivation.
pub fn account_token(wallet: &str) -> String {
    let hash = Sha256::digest(format!("dcl-iap:{}", wallet.to_lowercase()).as_bytes());
    let h = hex::encode(&hash[..16]);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

#[cfg(test)]
pub(crate) mod testkit {
    //! A throwaway App Store-shaped chain generated per test run.
    use super::*;
    use rcgen::{
        BasicConstraints, CertificateParams, CustomExtension, DnType, IsCa, Issuer, KeyPair,
        PKCS_ECDSA_P384_SHA384,
    };
    use ring::rand::SystemRandom;
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};

    pub struct Chain {
        pub root: Vec<u8>,
        pub intermediate: Vec<u8>,
        pub leaf: Vec<u8>,
        leaf_pkcs8: Vec<u8>,
    }

    pub struct ChainSpec {
        pub intermediate_marker: bool,
        pub leaf_marker: bool,
        pub intermediate_ca: bool,
        pub leaf_expired: bool,
        pub intermediate_expired: bool,
        pub root_expired: bool,
    }

    impl Default for ChainSpec {
        fn default() -> Self {
            Self {
                intermediate_marker: true,
                leaf_marker: true,
                intermediate_ca: true,
                leaf_expired: false,
                intermediate_expired: false,
                root_expired: false,
            }
        }
    }

    fn params(cn: &str) -> CertificateParams {
        let mut p = CertificateParams::new(Vec::<String>::new()).unwrap();
        p.distinguished_name.push(DnType::CommonName, cn);
        p
    }

    fn expired(p: &mut CertificateParams) {
        p.not_before = rcgen::date_time_ymd(2000, 1, 1);
        p.not_after = rcgen::date_time_ymd(2001, 1, 1);
    }

    fn marker(oid: &[u64]) -> CustomExtension {
        // DER NULL: Apple's markers carry no meaningful value.
        CustomExtension::from_oid_content(oid, vec![0x05, 0x00])
    }

    pub fn chain(spec: ChainSpec) -> Chain {
        let root_key = KeyPair::generate_for(&PKCS_ECDSA_P384_SHA384).unwrap();
        let mut root_params = params("Test Root CA");
        root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        if spec.root_expired {
            expired(&mut root_params);
        }
        let root = root_params.self_signed(&root_key).unwrap();
        let root_issuer = Issuer::new(root_params, &root_key);

        let int_key = KeyPair::generate_for(&PKCS_ECDSA_P384_SHA384).unwrap();
        let mut int_params = params("Test Intermediate CA");
        if spec.intermediate_ca {
            int_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        }
        if spec.intermediate_expired {
            expired(&mut int_params);
        }
        if spec.intermediate_marker {
            int_params
                .custom_extensions
                .push(marker(INTERMEDIATE_MARKER));
        }
        let intermediate = int_params.signed_by(&int_key, &root_issuer).unwrap();
        let int_issuer = Issuer::new(int_params, &int_key);

        let rng = SystemRandom::new();
        let leaf_pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .unwrap()
            .as_ref()
            .to_vec();
        let leaf_key = KeyPair::try_from(leaf_pkcs8.as_slice()).unwrap();
        let mut leaf_params = params("Test Receipt Signing");
        if spec.leaf_marker {
            leaf_params.custom_extensions.push(marker(LEAF_MARKER));
        }
        if spec.leaf_expired {
            expired(&mut leaf_params);
        }
        let leaf = leaf_params.signed_by(&leaf_key, &int_issuer).unwrap();
        Chain {
            root: root.der().to_vec(),
            intermediate: intermediate.der().to_vec(),
            leaf: leaf.der().to_vec(),
            leaf_pkcs8,
        }
    }

    impl Chain {
        pub fn verifier(&self, env: Environment) -> Verifier {
            Verifier::new(
                vec![self.root.clone()],
                BUNDLE.into(),
                Some(APP_APPLE_ID),
                env,
            )
            .unwrap()
        }

        pub fn sign(&self, payload: &JsonValue) -> String {
            self.sign_with_header(&serde_json::json!({
                "alg": "ES256",
                "x5c": [STANDARD.encode(&self.leaf), STANDARD.encode(&self.intermediate), STANDARD.encode(&self.root)],
            }), payload)
        }

        pub fn sign_with_header(&self, header: &JsonValue, payload: &JsonValue) -> String {
            let input = format!(
                "{}.{}",
                URL_SAFE_NO_PAD.encode(header.to_string()),
                URL_SAFE_NO_PAD.encode(payload.to_string())
            );
            let rng = SystemRandom::new();
            let key =
                EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &self.leaf_pkcs8, &rng)
                    .unwrap();
            let sig = key.sign(&rng, input.as_bytes()).unwrap();
            format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))
        }

        /// A signed transaction for `wallet`'s account token.
        pub fn transaction(&self, id: &str, product: &str, token: Option<&str>) -> String {
            self.sign(&transaction_payload(id, product, token))
        }

        /// A signed notification of `kind` nesting `transaction`.
        pub fn notification(&self, kind: &str, transaction: Option<String>) -> String {
            self.notification_with(kind, transaction, Self::app_data())
        }

        /// A signed notification of `kind` with its own `data` app fields.
        pub fn notification_with(
            &self,
            kind: &str,
            transaction: Option<String>,
            data: JsonValue,
        ) -> String {
            let now_ms = chrono::Utc::now().timestamp_millis();
            self.notification_signed_at(kind, transaction, data, now_ms)
        }

        /// A signed notification of `kind` whose `signedDate` is `signed_date_ms`.
        pub fn notification_signed_at(
            &self,
            kind: &str,
            transaction: Option<String>,
            mut data: JsonValue,
            signed_date_ms: i64,
        ) -> String {
            if let Some(t) = transaction {
                data["signedTransactionInfo"] = JsonValue::String(t);
            }
            self.sign(&serde_json::json!({
                "notificationType": kind,
                "notificationUUID": format!("uuid-{}", uuid::Uuid::new_v4()),
                "signedDate": signed_date_ms,
                "data": data,
            }))
        }

        /// The `data` app fields `notification` signs.
        pub fn app_data() -> JsonValue {
            serde_json::json!({
                "bundleId": BUNDLE,
                "appAppleId": APP_APPLE_ID,
                "environment": "Sandbox",
            })
        }
    }

    pub const BUNDLE: &str = "org.example.app";
    pub const APP_APPLE_ID: i64 = 1_234_567_890;

    pub fn transaction_payload(id: &str, product: &str, token: Option<&str>) -> JsonValue {
        let mut p = serde_json::json!({
            "transactionId": id,
            "originalTransactionId": id,
            "productId": product,
            "bundleId": BUNDLE,
            "environment": "Sandbox",
            "quantity": 1,
            "type": "Consumable",
        });
        if let Some(t) = token {
            p["appAccountToken"] = JsonValue::String(t.to_string());
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[test]
    fn a_transaction_signed_through_the_configured_root_verifies() {
        let c = chain(ChainSpec::default());
        let t = c
            .verifier(Environment::Sandbox)
            .transaction(&c.transaction("1000", "credits_100", Some("tok")), now())
            .unwrap();
        assert_eq!(t.transaction_id, "1000");
        assert_eq!(t.product_id, "credits_100");
        assert_eq!(t.app_account_token.as_deref(), Some("tok"));
        assert_eq!(t.quantity, 1);
        assert!(!t.revoked);
    }

    #[test]
    fn a_chain_ending_in_another_root_is_refused_even_when_x5c_carries_it() {
        let ours = chain(ChainSpec::default());
        let other = chain(ChainSpec::default());
        let err = ours
            .verifier(Environment::Sandbox)
            .transaction(&other.transaction("1", "p", None), now())
            .unwrap_err();
        assert_eq!(err, VerifyError::Chain);
    }

    #[test]
    fn an_altered_payload_fails_the_signature() {
        let c = chain(ChainSpec::default());
        let jws = c.transaction("1", "p", None);
        let mut parts: Vec<&str> = jws.split('.').collect();
        let altered = URL_SAFE_NO_PAD.encode(transaction_payload("2", "p", None).to_string());
        parts[1] = &altered;
        let err = c
            .verifier(Environment::Sandbox)
            .transaction(&parts.join("."), now())
            .unwrap_err();
        assert_eq!(err, VerifyError::Signature);
    }

    #[test]
    fn only_es256_with_three_certificates_is_accepted() {
        let c = chain(ChainSpec::default());
        let v = c.verifier(Environment::Sandbox);
        let payload = transaction_payload("1", "p", None);
        let hs = c.sign_with_header(
            &serde_json::json!({ "alg": "none", "x5c": [STANDARD.encode(&c.leaf), STANDARD.encode(&c.intermediate), STANDARD.encode(&c.root)] }),
            &payload,
        );
        assert_eq!(
            v.transaction(&hs, now()).unwrap_err(),
            VerifyError::Malformed
        );
        let two = c.sign_with_header(
            &serde_json::json!({ "alg": "ES256", "x5c": [STANDARD.encode(&c.leaf), STANDARD.encode(&c.intermediate)] }),
            &payload,
        );
        assert_eq!(v.transaction(&two, now()).unwrap_err(), VerifyError::Chain);
        assert_eq!(
            v.transaction("a.b", now()).unwrap_err(),
            VerifyError::Malformed
        );
    }

    #[test]
    fn the_apple_markers_and_a_ca_intermediate_are_required() {
        for spec in [
            ChainSpec {
                leaf_marker: false,
                ..Default::default()
            },
            ChainSpec {
                intermediate_marker: false,
                ..Default::default()
            },
            ChainSpec {
                intermediate_ca: false,
                ..Default::default()
            },
        ] {
            let c = chain(spec);
            let err = c
                .verifier(Environment::Sandbox)
                .transaction(&c.transaction("1", "p", None), now())
                .unwrap_err();
            assert_eq!(err, VerifyError::Chain);
        }
    }

    #[test]
    fn an_expired_intermediate_or_root_is_refused() {
        for spec in [
            ChainSpec {
                intermediate_expired: true,
                ..Default::default()
            },
            ChainSpec {
                root_expired: true,
                ..Default::default()
            },
        ] {
            let c = chain(spec);
            let err = c
                .verifier(Environment::Sandbox)
                .transaction(&c.transaction("1", "p", None), now())
                .unwrap_err();
            assert_eq!(err, VerifyError::Chain);
        }
    }

    #[test]
    fn the_environment_has_no_default() {
        assert!(Environment::parse(None).is_err());
        assert!(Environment::parse(Some(" ")).is_err());
        assert!(Environment::parse(Some("staging")).is_err());
        assert_eq!(
            Environment::parse(Some("Sandbox")).unwrap(),
            Environment::Sandbox
        );
        assert_eq!(
            Environment::parse(Some("production")).unwrap(),
            Environment::Production
        );
    }

    #[test]
    fn a_notification_must_name_this_app_and_environment() {
        let c = chain(ChainSpec::default());
        let sandbox = c.verifier(Environment::Sandbox);
        let app = |bundle: &str, app_id: i64, env: &str| serde_json::json!({ "bundleId": bundle, "appAppleId": app_id, "environment": env });
        let n = |data| c.notification_with("TEST", None, data);
        assert_eq!(
            sandbox
                .notification(&n(app("org.example.other", APP_APPLE_ID, "Sandbox")), now())
                .unwrap_err(),
            VerifyError::App
        );
        assert_eq!(
            sandbox
                .notification(&n(app(BUNDLE, APP_APPLE_ID, "Production")), now())
                .unwrap_err(),
            VerifyError::Environment
        );

        let prod = c.verifier(Environment::Production);
        assert_eq!(
            prod.notification(&n(app(BUNDLE, 42, "Production")), now())
                .unwrap_err(),
            VerifyError::App
        );
        assert_eq!(
            prod.notification(&n(app(BUNDLE, APP_APPLE_ID, "Sandbox")), now())
                .unwrap_err(),
            VerifyError::Environment
        );
        let mut t = transaction_payload("9", "p", None);
        t["environment"] = "Production".into();
        let ok = prod
            .notification(
                &c.notification_with(
                    "ONE_TIME_CHARGE",
                    Some(c.sign(&t)),
                    app(BUNDLE, APP_APPLE_ID, "Production"),
                ),
                now(),
            )
            .unwrap();
        assert_eq!(ok.transaction.unwrap().transaction_id, "9");
        // A production notification nesting a sandbox transaction is refused.
        assert_eq!(
            prod.notification(
                &c.notification_with(
                    "ONE_TIME_CHARGE",
                    Some(c.transaction("9", "p", None)),
                    app(BUNDLE, APP_APPLE_ID, "Production"),
                ),
                now(),
            )
            .unwrap_err(),
            VerifyError::Environment
        );
    }

    #[test]
    fn a_partial_refund_reports_its_share() {
        let c = chain(ChainSpec::default());
        let v = c.verifier(Environment::Sandbox);
        let mut p = transaction_payload("1", "p", None);
        p["revocationDate"] = 1_700_000_000_000i64.into();
        p["revocationPercentage"] = 25_000.into();
        assert_eq!(
            v.transaction(&c.sign(&p), now())
                .unwrap()
                .revocation_percentage,
            Some(25_000)
        );
        p["revocationPercentage"] = 100_000.into();
        assert_eq!(
            v.transaction(&c.sign(&p), now())
                .unwrap()
                .revocation_percentage,
            None
        );
    }

    #[test]
    fn an_expired_leaf_is_refused() {
        let c = chain(ChainSpec {
            leaf_expired: true,
            ..Default::default()
        });
        let err = c
            .verifier(Environment::Sandbox)
            .transaction(&c.transaction("1", "p", None), now())
            .unwrap_err();
        assert_eq!(err, VerifyError::Chain);
    }

    #[test]
    fn the_bundle_and_environment_must_match() {
        let c = chain(ChainSpec::default());
        let v = c.verifier(Environment::Sandbox);
        let mut other_app = transaction_payload("1", "p", None);
        other_app["bundleId"] = "org.example.other".into();
        assert_eq!(
            v.transaction(&c.sign(&other_app), now()).unwrap_err(),
            VerifyError::App
        );
        let prod = c.verifier(Environment::Production);
        assert_eq!(
            prod.transaction(&c.transaction("1", "p", None), now())
                .unwrap_err(),
            VerifyError::Environment
        );
    }

    #[test]
    fn a_notification_verifies_its_nested_transaction() {
        let c = chain(ChainSpec::default());
        let other = chain(ChainSpec::default());
        let v = c.verifier(Environment::Sandbox);
        let n = v
            .notification(
                &c.notification("REFUND", Some(c.transaction("7", "p", None))),
                now(),
            )
            .unwrap();
        assert_eq!(n.notification_type, "REFUND");
        assert_eq!(n.transaction.unwrap().transaction_id, "7");
        let nested_foreign = c.notification("REFUND", Some(other.transaction("7", "p", None)));
        assert_eq!(
            v.notification(&nested_foreign, now()).unwrap_err(),
            VerifyError::Chain
        );
        let test = v
            .notification(&c.notification("TEST", None), now())
            .unwrap();
        assert!(test.transaction.is_none());
    }

    #[test]
    fn a_notification_signed_outside_the_window_is_refused() {
        let c = chain(ChainSpec::default());
        let v = c.verifier(Environment::Sandbox);
        let now = now();
        let at = |secs: i64| {
            let signed = c.notification_signed_at("TEST", None, Chain::app_data(), secs * 1000);
            v.notification(&signed, now)
        };
        // The uuid is random: only its presence is compared.
        let refused = |r: Result<Notification, VerifyError>| {
            let e = r.unwrap_err();
            let (uuid, ms) = e.out_of_window().expect("outside the window");
            assert!(!uuid.is_empty());
            (matches!(e, VerifyError::Expired { .. }), ms)
        };
        // The oldest retry Apple sends and a small clock skew pass.
        assert_eq!(
            at(now - 157 * 3600).unwrap().signed_date_ms,
            (now - 157 * 3600) * 1000
        );
        assert!(at(now - NOTIFICATION_MAX_AGE_SECS).is_ok());
        assert!(at(now + NOTIFICATION_MAX_FUTURE_SECS).is_ok());
        let old = now - NOTIFICATION_MAX_AGE_SECS - 1;
        assert_eq!(refused(at(old)), (true, old * 1000));
        let ahead = now + NOTIFICATION_MAX_FUTURE_SECS + 1;
        assert_eq!(refused(at(ahead)), (false, ahead * 1000));
    }

    #[test]
    fn signed_ahead_allows_the_skew_and_refuses_beyond_it() {
        let now_ms = 1_700_000_000_000;
        let skew_ms = NOTIFICATION_MAX_FUTURE_SECS * 1000;
        assert!(!signed_ahead(now_ms + skew_ms, now_ms));
        assert!(signed_ahead(now_ms + skew_ms + 1, now_ms));
        assert!(!signed_ahead(i64::MAX, i64::MAX));
    }

    #[test]
    fn a_notification_without_a_signed_date_or_uuid_is_malformed() {
        let c = chain(ChainSpec::default());
        let v = c.verifier(Environment::Sandbox);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let undated = c.sign(&serde_json::json!({
            "notificationType": "TEST", "notificationUUID": "u-1", "data": Chain::app_data(),
        }));
        let unnamed = c.sign(&serde_json::json!({
            "notificationType": "TEST", "notificationUUID": "", "signedDate": now_ms,
            "data": Chain::app_data(),
        }));
        for signed in [undated, unnamed] {
            assert_eq!(
                v.notification(&signed, now()).unwrap_err(),
                VerifyError::Malformed
            );
        }
    }

    #[test]
    fn a_revoked_transaction_is_reported() {
        let c = chain(ChainSpec::default());
        let mut p = transaction_payload("1", "p", None);
        p["revocationDate"] = 1_700_000_000_000i64.into();
        let t = c
            .verifier(Environment::Sandbox)
            .transaction(&c.sign(&p), now())
            .unwrap();
        assert!(t.revoked);
    }

    #[test]
    fn production_requires_the_app_apple_id() {
        let c = chain(ChainSpec::default());
        assert!(
            Verifier::new(
                vec![c.root.clone()],
                BUNDLE.into(),
                None,
                Environment::Production
            )
            .is_err()
        );
        assert!(Verifier::new(vec![], BUNDLE.into(), None, Environment::Sandbox).is_err());
    }

    #[test]
    fn the_account_token_matches_the_app_derivation() {
        // SHA-256("dcl-iap:0x00…01")[..16] formatted as a UUID; the wallet is lowercased first.
        assert_eq!(
            account_token("0x0000000000000000000000000000000000000001"),
            "b6f6d6be-a49d-16cc-d4c2-425b671e63c7"
        );
        assert_eq!(
            account_token("0xABCDEF0000000000000000000000000000000001"),
            account_token("0xabcdef0000000000000000000000000000000001")
        );
    }
}
