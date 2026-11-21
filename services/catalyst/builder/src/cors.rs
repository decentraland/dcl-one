//! Who may read the builder's answers from a web page on another origin.
//!
//! `CORS_ORIGIN` is builder-server's setting, in its format: entries
//! separated by `;` and trimmed; an entry `regex:<pattern>` matches an
//! origin it finds anywhere in (anchor it), `*` matches every origin, any
//! other entry is compared exactly. Unset, the Decentraland front-ends
//! (`shared::cors::is_decentraland_origin`, `http://localhost:<port>`
//! included). A pattern the node's `regex` can't compile, or a list that
//! names nothing, stops the node at startup. `CORS_METHOD` is the methods a
//! preflight allows, `,`-separated; unset or `*`, the methods the routes
//! answer.
//!
//! A listed origin is echoed in `Access-Control-Allow-Origin` with
//! builder-server's exposed headers; any other origin gets no allow-origin;
//! every answer varies by `Origin`; no answer allows credentials, since
//! signatures, not cookies, authorize. A preflight is answered here, before
//! the request budgets, the signature and the database, allowing the
//! signed-fetch and validator headers the routes read. `GET` and `HEAD`
//! of `/v1/storage/contents/:hash` and `GET /v1/info` answer every origin,
//! as builder-server's permissive routes do.

use std::time::Duration;

use axum::http::{HeaderName, HeaderValue, Method, request::Parts};
use regex::Regex;
use tower_http::cors::{AllowOrigin, CorsLayer};

const ROUTE_METHODS: [Method; 6] = [
    Method::GET,
    Method::HEAD,
    Method::POST,
    Method::PUT,
    Method::PATCH,
    Method::DELETE,
];

const ALLOWED_HEADERS: [&str; 17] = [
    "content-type",
    "range",
    "x-identity-timestamp",
    "x-identity-metadata",
    "x-identity-auth-chain-0",
    "x-identity-auth-chain-1",
    "x-identity-auth-chain-2",
    "x-identity-auth-chain-3",
    "x-identity-auth-chain-4",
    "x-identity-auth-chain-5",
    "x-identity-auth-chain-6",
    "x-identity-auth-chain-7",
    "x-identity-auth-chain-8",
    "x-identity-auth-chain-9",
    "x-wearable-validator-signature",
    "x-wearable-validator-timestamp",
    "authorization",
];

const EXPOSED_HEADERS: [&str; 7] = [
    "etag",
    "cache-control",
    "content-language",
    "content-type",
    "expires",
    "last-modified",
    "pragma",
];

#[derive(Clone, Debug)]
enum Origins {
    Decentraland,
    Any,
    Listed {
        exact: Vec<String>,
        patterns: Vec<Regex>,
    },
}

/// `CORS_ORIGIN` and `CORS_METHOD`, read.
#[derive(Clone, Debug)]
pub struct Cors {
    origins: Origins,
    methods: Vec<Method>,
}

impl Default for Cors {
    fn default() -> Self {
        Self {
            origins: Origins::Decentraland,
            methods: ROUTE_METHODS.to_vec(),
        }
    }
}

impl Cors {
    /// Reads the two settings; unset or blank is the default.
    pub fn parse(origin: Option<&str>, method: Option<&str>) -> anyhow::Result<Self> {
        let origins = match origin.map(str::trim).filter(|o| !o.is_empty()) {
            None => Origins::Decentraland,
            Some(listed) => {
                let entries: Vec<&str> = listed
                    .split(';')
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                    .collect();
                if entries.is_empty() {
                    anyhow::bail!("CORS_ORIGIN names no origin");
                }
                if entries.contains(&"*") {
                    Origins::Any
                } else {
                    let mut exact = Vec::new();
                    let mut patterns = Vec::new();
                    for (n, entry) in entries.into_iter().enumerate() {
                        match entry.strip_prefix("regex:") {
                            Some(pattern) => patterns.push(Regex::new(pattern).map_err(|_| {
                                anyhow::anyhow!(
                                    "CORS_ORIGIN entry {} is not a pattern this node can match",
                                    n + 1
                                )
                            })?),
                            None => exact.push(entry.to_string()),
                        }
                    }
                    Origins::Listed { exact, patterns }
                }
            }
        };
        let methods = match method.map(str::trim).filter(|m| !m.is_empty() && *m != "*") {
            None => ROUTE_METHODS.to_vec(),
            Some(listed) => listed
                .split(',')
                .map(|m| {
                    let m = m.trim().to_ascii_uppercase();
                    ROUTE_METHODS
                        .iter()
                        .find(|known| known.as_str() == m)
                        .cloned()
                        .ok_or_else(|| {
                            anyhow::anyhow!("CORS_METHOD lists a method the routes don't answer")
                        })
                })
                .collect::<anyhow::Result<_>>()?,
        };
        Ok(Self { origins, methods })
    }

    /// Whether `origin` may read the listed routes' answers.
    pub fn allows(&self, origin: &str) -> bool {
        match &self.origins {
            Origins::Decentraland => crate::shared::cors::is_decentraland_origin(origin),
            Origins::Any => true,
            Origins::Listed { exact, patterns } => {
                exact.iter().any(|e| e == origin) || patterns.iter().any(|p| p.is_match(origin))
            }
        }
    }

    /// The methods a preflight allows, as sent.
    pub fn methods(&self) -> String {
        self.methods
            .iter()
            .map(Method::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The layer over both mounts.
    pub fn layer(&self) -> CorsLayer {
        let cors = self.clone();
        CorsLayer::new()
            .allow_origin(AllowOrigin::predicate(
                move |origin: &HeaderValue, parts: &Parts| {
                    public(parts) || origin.to_str().is_ok_and(|o| cors.allows(o))
                },
            ))
            .allow_methods(self.methods.clone())
            .allow_headers(ALLOWED_HEADERS.map(HeaderName::from_static))
            .expose_headers(EXPOSED_HEADERS.map(HeaderName::from_static))
            .max_age(Duration::from_secs(86_400))
    }
}

fn public(parts: &Parts) -> bool {
    let path = parts.uri.path();
    path == "/v1/info"
        || path
            .strip_prefix("/v1/storage/contents/")
            .is_some_and(|hash| !hash.contains('/'))
}
