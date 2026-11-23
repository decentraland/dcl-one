//! Helpers shared by the write-path tests: a signed auth chain, a throwaway
//! storage tree, one-route content servers and the optional test database.

use std::sync::Arc;

use axum::http::HeaderValue;

use crate::auth::{AuthChain, AuthLink, AuthLinkType};
use crate::storage::Storage;

const EPHEMERAL_KEY: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// A two-link chain (SIGNER, ECDSA_SIGNED_ENTITY) over `entity_id` from the shared test key.
pub(crate) async fn signed_chain(entity_id: &str) -> AuthChain {
    catalyst_shared::test_keys::signed_chain(catalyst_shared::test_keys::ENTITY_KEY, entity_id)
        .await
}

/// A contract-wallet chain as a peer serves it: SIGNER (a contract), an
/// ECDSA_EIP_1654_EPHEMERAL link whose signature only the contract could judge
/// (here `0x00`), and the entity signed by that ephemeral key.
pub(crate) async fn signed_chain_behind_contract(entity_id: &str, expiration: &str) -> AuthChain {
    use alloy::signers::{Signer, local::PrivateKeySigner};

    let ephemeral: PrivateKeySigner = EPHEMERAL_KEY.parse().unwrap();
    let signature = ephemeral
        .sign_message(entity_id.as_bytes())
        .await
        .unwrap()
        .to_string();
    vec![
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: "0x0000000000000000000000000000000000000002".into(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaEip1654Ephemeral,
            payload: format!(
                "Decentraland Login\nEphemeral address: {:#x}\nExpiration: {expiration}",
                ephemeral.address()
            ),
            signature: Some("0x00".into()),
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload: entity_id.to_string(),
            signature: Some(signature),
        },
    ]
}

pub(crate) fn unique() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// `None` (skip) without CATALYST_TEST_DATABASE_URL; otherwise a pool over
/// the process's scratch database (`db::scratch`), never the URL's own.
pub(crate) async fn test_pool(max_connections: u32) -> Option<sqlx::PgPool> {
    crate::db::scratch::pool(max_connections).await
}

pub(crate) async fn temp_storage(tag: &str) -> (Arc<Storage>, std::path::PathBuf) {
    let tmp = std::env::temp_dir().join(format!(
        "catalyst-sync-test-{tag}-{}-{}",
        std::process::id(),
        unique()
    ));
    let storage = Storage::from_config(&tmp).await.unwrap();
    (Arc::new(storage), tmp)
}

/// Removes a test's directory when dropped, so a test that panics leaves no tree behind.
pub(crate) struct RemoveOnDrop(pub std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) async fn spawn_content_server(
    body: Vec<u8>,
    encoding: Option<&'static str>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new().route(
        "/contents/{hash}",
        axum::routing::get(move || async move {
            let mut resp = axum::response::Response::new(axum::body::Body::from(body));
            if let Some(enc) = encoding {
                resp.headers_mut().insert(
                    axum::http::header::CONTENT_ENCODING,
                    HeaderValue::from_static(enc),
                );
            }
            resp
        }),
    );
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), handle)
}

/// A loopback peer whose `redirect` path answers 307 to `target(port)` on any
/// method, and the number of requests the redirect target received.
pub(crate) async fn redirecting_peer(
    redirect: &'static str,
    target: impl Fn(u16) -> String + Send + Sync + 'static,
) -> (u16, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::routing::any;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = hits.clone();
    let location = target(port);
    let app = axum::Router::new()
        .route(
            redirect,
            any(move || async move { axum::response::Redirect::temporary(&location) }),
        )
        .route(
            "/target",
            any(move || async move {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "[]"
            }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (port, hits)
}

/// A loopback peer whose `/contents/{hash}` answers `blobs[hash]`, 404 otherwise.
pub(crate) async fn blob_peer(
    blobs: std::collections::HashMap<String, Vec<u8>>,
) -> (String, tokio::task::JoinHandle<()>) {
    use axum::extract::Path;
    use axum::response::IntoResponse;
    let blobs = Arc::new(blobs);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new().route(
        "/contents/{hash}",
        axum::routing::get(move |Path(hash): Path<String>| {
            let blobs = blobs.clone();
            async move {
                match blobs.get(&hash) {
                    Some(bytes) => bytes.clone().into_response(),
                    None => axum::http::StatusCode::NOT_FOUND.into_response(),
                }
            }
        }),
    );
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), handle)
}
