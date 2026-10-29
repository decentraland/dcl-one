//! Helpers shared by the write-path tests: a signed auth chain, a throwaway
//! storage tree, a one-route content server and the optional test database.

use std::sync::Arc;

use axum::http::HeaderValue;

use crate::auth::{AuthChain, AuthLink, AuthLinkType};
use crate::storage::Storage;

const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const EPHEMERAL_KEY: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// A two-link chain (SIGNER, ECDSA_SIGNED_ENTITY) over `entity_id` from a fixed test key.
pub(crate) async fn signed_chain(entity_id: &str) -> AuthChain {
    use alloy::signers::{Signer, local::PrivateKeySigner};

    let key: PrivateKeySigner = TEST_KEY.parse().unwrap();
    let signature = key
        .sign_message(entity_id.as_bytes())
        .await
        .unwrap()
        .to_string();
    vec![
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: format!("{:#x}", key.address()),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload: entity_id.to_string(),
            signature: Some(signature),
        },
    ]
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
