//! Fixed test wallets and signed-fetch header builders the comms, worlds and
//! social tests share.

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::http::{HeaderMap, HeaderName, HeaderValue};

use crate::auth::{AuthLink, AuthLinkType};
use crate::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_legacy_payload,
    build_payload_v6,
};

pub const KEY: &str = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f2e3e8a5d4b8e3e3e3";
pub const AUTHORITATIVE_KEY: &str =
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
pub const OUTSIDER_KEY: &str = "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba";

pub fn wallet(key: &str) -> PrivateKeySigner {
    key.parse().unwrap()
}

/// A fresh signing key, so DB-backed tests sharing one scratch database
/// never ban or seed each other's wallet.
pub fn random_key() -> String {
    format!("0x{}", hex::encode(PrivateKeySigner::random().to_bytes()))
}

pub fn address_of(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Legacy,
    V6,
}

pub const SHAPES: [Shape; 2] = [Shape::Legacy, Shape::V6];

/// Headers `key` signs for `method path` now over `signed`, delivering
/// `delivered` as the metadata header (the same string unless a test
/// tampers with it).
pub async fn signed_as(
    key: &str,
    shape: Shape,
    method: &str,
    path: &str,
    signed: &str,
    delivered: &str,
) -> HeaderMap {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    signed_at(key, shape, method, path, signed, delivered, &timestamp).await
}

pub async fn signed_at(
    key: &str,
    shape: Shape,
    method: &str,
    path: &str,
    signed: &str,
    delivered: &str,
    timestamp: &str,
) -> HeaderMap {
    let payload = match shape {
        Shape::Legacy => build_legacy_payload(method, path, timestamp, signed),
        Shape::V6 => build_payload_v6(method, path, timestamp, signed),
    };
    let wallet = wallet(key);
    let signature = wallet
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: format!("{:#x}", wallet.address()),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature),
        },
    ];
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTH_TIMESTAMP_HEADER,
        HeaderValue::from_str(timestamp).unwrap(),
    );
    headers.insert(
        AUTH_METADATA_HEADER,
        HeaderValue::from_str(delivered).unwrap(),
    );
    for (i, link) in chain.iter().enumerate() {
        headers.insert(
            HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes()).unwrap(),
            HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
        );
    }
    headers
}

pub async fn signed(key: &str, method: &str, path: &str, metadata: &str) -> HeaderMap {
    signed_as(key, Shape::V6, method, path, metadata, metadata).await
}
