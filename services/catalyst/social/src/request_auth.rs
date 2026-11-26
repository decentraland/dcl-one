//! The social verifier for signed requests: `dcl_auth::request` with the
//! default 60 s window, the current payload, contract wallets asked through
//! the node's EIP-1654 validator, and a request a scene signed refused.

use std::sync::{Arc, LazyLock};

use crate::auth::eip1654::Eip1654Validator;
use crate::auth::request::{
    MetadataPredicate, RequestAuthError, RequestParts, VerifiedRequest, VerifyOptions,
    VerifyOptionsBuilder, reject_if_signer, verify_request,
};
use crate::shared::signed_fetch::SCENE_SIGNER;

static NOT_A_SCENE: LazyLock<MetadataPredicate> =
    LazyLock::new(|| reject_if_signer(&[SCENE_SIGNER]).expect("the scene signer is canonical"));

/// The social options, ready for a route's additions.
pub fn builder(eip1654: Option<&Arc<dyn Eip1654Validator>>) -> VerifyOptionsBuilder {
    let builder = VerifyOptions::builder().metadata_validator(NOT_A_SCENE.clone());
    match eip1654 {
        Some(validator) => builder.eip1654(validator.clone()),
        None => builder,
    }
}

pub fn options(eip1654: Option<&Arc<dyn Eip1654Validator>>) -> VerifyOptions {
    build(builder(eip1654))
}

pub fn build(builder: VerifyOptionsBuilder) -> VerifyOptions {
    builder
        .build()
        .expect("the social verifier options are valid")
}

/// [`verify_request`] for options that are not `optional`.
pub async fn require(
    parts: &RequestParts,
    options: &VerifyOptions,
) -> Result<VerifiedRequest, RequestAuthError> {
    verify_request(parts, options)
        .await?
        .ok_or_else(|| RequestAuthError::Unauthorized("Invalid Auth Chain".into()))
}
