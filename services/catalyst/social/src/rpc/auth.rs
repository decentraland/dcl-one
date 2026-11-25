//! The socket handshakes' verification through the social verifier
//! ([`crate::request_auth`]): a first frame ([`verify_frame`]) or the upgrade
//! request's headers ([`verify_headers`]), signed for `get` over the
//! socket's own paths. The first path is the source protocol path; each
//! further one, a mounted alias kept for shipped clients, is tried only after
//! a 401. Proxy headers never add a path. The window is
//! `WS_AUTH_SIGNATURE_EXPIRATION_IN_MS`, else `AUTH_WINDOW_SECS` (300 s), on
//! both sides of now, as the source service's crypto-middleware applies it.

use axum::http::HeaderMap;
use thiserror::Error;

use super::State;
use crate::Address;
use crate::auth::request::{RequestAuthError, RequestParts, VerifyOptions};
use crate::shared::rpc::handshake::{HandshakeError, frame_fields, frame_parts};
use crate::shared::signed_fetch::SignedPath;

#[derive(Debug, Error)]
pub enum SocketAuthError {
    #[error(transparent)]
    Envelope(#[from] HandshakeError),
    #[error(transparent)]
    Refused(#[from] RequestAuthError),
}

impl SocketAuthError {
    /// The check could not run (a contract wallet unreachable): the server's fault.
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Refused(RequestAuthError::Unavailable))
    }

    /// Canonical admission reason and Social's existing authentication error type.
    pub(super) fn rejection_labels(&self) -> (&'static str, &'static str) {
        if self.is_unavailable() {
            ("unavailable", "server_error")
        } else if matches!(self, Self::Refused(RequestAuthError::RateLimited)) {
            // The client source spent its contract-wallet checks (EIP-1654 budget).
            ("rate_limited", "client_rejected")
        } else {
            ("authentication", "client_rejected")
        }
    }
}

fn options(state: &State) -> VerifyOptions {
    crate::request_auth::build(
        crate::request_auth::builder(state.eip1654.as_ref())
            .expiration(state.settings.auth_signature_expiration())
            .future_bound_follows_window(true),
    )
}

async fn over_paths(
    paths: &[String],
    options: &VerifyOptions,
    parts: impl Fn(SignedPath) -> RequestParts,
) -> Result<Address, RequestAuthError> {
    // The source path's refusal is the one reported.
    let mut refusal = None;
    for path in paths {
        match crate::request_auth::require(&parts(SignedPath::local(path.as_str())), options).await
        {
            Ok(verified) => return Ok(Address::from_verified(&verified)),
            Err(e) if e.status() == 401 => {
                refusal.get_or_insert(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(refusal.unwrap_or_else(|| RequestAuthError::Unauthorized("no signed path".into())))
}

pub async fn verify_frame(
    state: &State,
    paths: &[String],
    frame: &str,
) -> Result<Address, SocketAuthError> {
    let fields = frame_fields(frame)?;
    Ok(over_paths(paths, &options(state), |path| {
        frame_parts(&fields, "get", path)
    })
    .await?)
}

/// The upgrade request's `x-identity-*` headers; `x-original-path` is not read.
pub async fn verify_headers(
    state: &State,
    paths: &[String],
    headers: &HeaderMap,
) -> Result<Address, SocketAuthError> {
    Ok(over_paths(paths, &options(state), |path| {
        let mut parts = RequestParts::from_headers("get", path, headers);
        parts.original_path = None;
        parts
    })
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_outages_and_invalid_credentials_have_distinct_admission_reasons() {
        let outage = SocketAuthError::Refused(RequestAuthError::Unavailable);
        let refusal = SocketAuthError::Refused(RequestAuthError::Unauthorized("signature".into()));
        assert_eq!(outage.rejection_labels(), ("unavailable", "server_error"));
        assert_eq!(
            refusal.rejection_labels(),
            ("authentication", "client_rejected")
        );
    }
}
