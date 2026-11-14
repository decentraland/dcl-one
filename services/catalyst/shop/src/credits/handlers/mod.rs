//! The route handlers, one file per old handler module. Every user route
//! binds the wallet through signed fetch (`x-identity-*` headers over
//! `method:path:timestamp:metadata`, the shared verifier's 5 minute window,
//! 6.x then the legacy payload as the old crypto crate accepted): the
//! signature is checked over the request's own path (`/credits/users` under
//! the gateway prefix) and the proxy's `x-original-path` when it ends with the
//! route (`/users` on a prefix-stripping host), as every other surface does. No handler
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
use crate::shared::signed_fetch::{PayloadPolicy, SignedPath, Signer, require_signer_at};

pub async fn signer_from(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    route_path: &str,
    original: &OriginalUri,
) -> Result<Signer, ApiError> {
    require_signer_at(
        headers,
        method,
        &SignedPath::at(&original.0, route_path),
        PayloadPolicy::V6ThenLegacy,
        state.eip1654.as_deref(),
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
