//! Test support shared by the shop-server fold's route tests (outfits,
//! subscriptions, the treasury admin read): a scratch wallet, the ADR-44
//! headers it signs over a method and path, and a oneshot request helper.

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use serde_json::Value;
use tower::ServiceExt;

use crate::auth::{AuthLink, AuthLinkType};
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};

pub struct Wallet {
    signer: PrivateKeySigner,
    pub address: String,
}

pub fn scratch_wallet() -> Wallet {
    let signer = PrivateKeySigner::random();
    let address = format!("{:#x}", signer.address());
    Wallet { signer, address }
}

/// The 6.x signed-fetch headers over `method` (lowercase) and `path`, with `metadata` as the identity metadata header.
pub async fn signed_headers_with(
    w: &Wallet,
    method: &str,
    path: &str,
    metadata: &str,
) -> HeaderMap {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = build_payload_v6(method, path, &timestamp, metadata);
    let signature = w
        .signer
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: w.address.clone(),
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
        HeaderValue::from_str(&timestamp).unwrap(),
    );
    headers.insert(
        AUTH_METADATA_HEADER,
        HeaderValue::from_str(metadata).unwrap(),
    );
    for (i, link) in chain.iter().enumerate() {
        headers.insert(
            axum::http::HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes())
                .unwrap(),
            HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
        );
    }
    headers
}

pub async fn signed_headers(w: &Wallet, method: &str, path: &str) -> HeaderMap {
    signed_headers_with(w, method, path, "{}").await
}

/// One request; the body is JSON when a value is given, raw bytes when given as bytes.
pub async fn send(
    app: &Router,
    method: &str,
    path: &str,
    headers: HeaderMap,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let (status, headers, bytes) = send_raw(
        app,
        method,
        path,
        headers,
        body.map(|v| (v.to_string().into_bytes(), "application/json")),
    )
    .await;
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, headers, value)
}

/// One request with an arbitrary body (`(bytes, content type)`), the response bytes returned as they are.
pub async fn send_raw(
    app: &Router,
    method: &str,
    path: &str,
    headers: HeaderMap,
    body: Option<(Vec<u8>, &str)>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    let req = match body {
        Some((bytes, content_type)) => {
            let len = bytes.len();
            req.header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, len)
                .body(Body::from(bytes))
                .unwrap()
        }
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 1 << 22).await.unwrap();
    (status, headers, bytes.to_vec())
}
