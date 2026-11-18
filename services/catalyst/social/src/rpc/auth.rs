//! The socket handshakes' verification through the social verifier
//! ([`crate::request_auth`]): a first frame ([`verify_frame`]) or the upgrade
//! request's headers ([`verify_headers`]), signed for `get` over the
//! socket's own paths. The first path is the source protocol path; each
//! further one, a mounted alias kept for shipped clients, is tried only after
//! a 401. Proxy headers never add a path. The window is
//! `WS_AUTH_SIGNATURE_EXPIRATION_IN_MS`, 60 s when unset.

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
}

fn options(state: &State) -> VerifyOptions {
    crate::request_auth::build(
        crate::request_auth::builder(state.eip1654.as_ref())
            .expiration(state.settings.auth_signature_expiration()),
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
