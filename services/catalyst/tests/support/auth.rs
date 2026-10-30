//! A signed auth chain for the sync contracts: every entity the sync deployer accepts must
//! carry a chain whose ECDSA links verify.

use catalyst::auth::{AuthChain, AuthLink, AuthLinkType};

/// A two-link chain (SIGNER, ECDSA_SIGNED_ENTITY) over `entity_id` from a fixed test key.
pub async fn signed_chain(entity_id: &str) -> AuthChain {
    use alloy::signers::{Signer, local::PrivateKeySigner};

    let key: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        .parse()
        .unwrap();
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
