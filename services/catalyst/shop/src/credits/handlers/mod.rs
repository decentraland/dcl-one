//! The route handlers, one file per old handler module. Every user route
//! binds the wallet through signed fetch (`x-identity-*` headers over
//! `method:path:timestamp:metadata`, the shared verifier's 5 minute window,
//! 6.x then the legacy payload as the old crypto crate accepted): the
//! signature is checked over the public path first (the request's own path,
//! `/credits/users` under the gateway prefix) and the route-local path
//! second (`/users`, what the old service behind nginx's prefix strip saw),
//! so both the explorer's and the old proxy's signatures verify. No handler
//! reads `x-identity-metadata`: every address comes from the recovered
//! signature alone. Any auth-chain refusal answers the ADR-44 400.

pub mod admin;
pub mod authorize;
pub mod captcha;
pub mod cart;
pub mod history;
pub mod orders;
pub mod packs;
pub mod prices;
pub mod stripe;
pub mod topup;
pub mod users;

use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::OriginalUri;
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

use super::SharedState;
use super::errors::ApiError;
use crate::shared::signed_fetch::{PayloadPolicy, SignedFetchError, Signer, require_signer};

pub async fn signer_from(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    route_path: &str,
    original: &OriginalUri,
) -> Result<Signer, ApiError> {
    let validator = state.eip1654.as_deref();
    let public = original.path();
    if public != route_path {
        match require_signer(
            headers,
            method,
            public,
            PayloadPolicy::V6ThenLegacy,
            validator,
        )
        .await
        {
            Ok(signer) => return Ok(signer),
            Err(SignedFetchError::InvalidSignature(_)) => {}
            Err(e) => return Err(e.into()),
        }
    }
    require_signer(
        headers,
        method,
        route_path,
        PayloadPolicy::V6ThenLegacy,
        validator,
    )
    .await
    .map_err(Into::into)
}

static RANDOM_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn random_32_hex() -> String {
    let mut h = Sha256::new();
    h.update(std::process::id().to_le_bytes());
    h.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    h.update(RANDOM_COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    hex::encode(h.finalize())
}
