use axum::http::HeaderMap;

use crate::auth_chain::MAX_AUTH_CHAIN_LINKS;
use crate::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, ORIGINAL_PATH_HEADER,
    SignedPath, candidates,
};

/// What a signed request carries, as received: from HTTP headers, or from the
/// fields of a WebSocket auth frame or a Pulse handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestParts {
    pub method: String,
    /// The path the request arrived on and the route's own path.
    pub path: SignedPath,
    /// `x-original-path`, set by the gateway that mounts a service under a prefix.
    pub original_path: Option<String>,
    /// `x-identity-auth-chain-0` through `x-identity-auth-chain-10`.
    pub chain: Vec<Option<String>>,
    pub timestamp: Option<String>,
    pub metadata: Option<String>,
}

/// A header as Node reads it: every byte one Latin-1 character, and repeated
/// values joined with `", "`.
pub fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let values: Vec<String> = headers
        .get_all(name)
        .iter()
        .map(|value| value.as_bytes().iter().map(|&b| char::from(b)).collect())
        .collect();
    (!values.is_empty()).then(|| values.join(", "))
}

impl RequestParts {
    pub fn from_headers(method: &str, path: SignedPath, headers: &HeaderMap) -> Self {
        Self::from_fields(method, path, |name| header_value(headers, name))
    }

    /// `field` answers a header name (`x-identity-timestamp`, ...) with its raw value.
    pub fn from_fields(
        method: &str,
        path: SignedPath,
        field: impl Fn(&str) -> Option<String>,
    ) -> Self {
        Self {
            method: method.to_string(),
            path,
            original_path: field(ORIGINAL_PATH_HEADER),
            chain: (0..=MAX_AUTH_CHAIN_LINKS)
                .map(|index| field(&format!("{AUTH_CHAIN_HEADER_PREFIX}{index}")))
                .collect(),
            timestamp: field(AUTH_TIMESTAMP_HEADER),
            metadata: field(AUTH_METADATA_HEADER),
        }
    }

    /// The paths the signature may be over ([`crate::signed_fetch::candidate_paths`]).
    pub fn candidate_paths(&self) -> Vec<&str> {
        candidates(self.original_path.as_deref(), &self.path)
    }

    pub(crate) fn link(&self, index: usize) -> Option<&str> {
        self.chain
            .get(index)
            .and_then(Option::as_deref)
            .filter(|raw| !raw.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue};

    #[test]
    fn headers_read_as_latin1_and_repeats_join() {
        let mut headers = HeaderMap::new();
        let name = HeaderName::from_static("x-identity-metadata");
        headers.append(
            &name,
            HeaderValue::from_bytes(b"{\"n\":\"ni\xf1o\"}").unwrap(),
        );
        assert_eq!(
            header_value(&headers, "x-identity-metadata").as_deref(),
            Some("{\"n\":\"ni\u{f1}o\"}")
        );
        headers.append(&name, HeaderValue::from_static("{}"));
        assert_eq!(
            header_value(&headers, "x-identity-metadata").as_deref(),
            Some("{\"n\":\"ni\u{f1}o\"}, {}")
        );
        assert_eq!(header_value(&headers, "x-identity-timestamp"), None);
        headers.insert(
            HeaderName::from_static("x-identity-auth-chain-1"),
            HeaderValue::from_static(""),
        );
        let parts = RequestParts::from_headers("GET", SignedPath::local("/a"), &headers);
        assert_eq!(parts.chain.len(), MAX_AUTH_CHAIN_LINKS + 1);
        assert_eq!(parts.chain[1].as_deref(), Some(""));
        assert_eq!(parts.link(1), None);
    }
}
