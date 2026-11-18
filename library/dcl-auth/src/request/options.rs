use std::sync::Arc;
use std::time::Duration;

use super::metadata::MetadataPredicate;
use super::parts::RequestParts;
use crate::auth_chain::MAX_AUTH_CHAIN_LINKS;
use crate::eip1654::Eip1654Validator;
use crate::scoped_login::{DEFAULT_MAX_SCOPED_LOGIN_LIFETIME_MS, is_operation_format, is_service};

/// How far a signed timestamp may be from now, either way.
pub const DEFAULT_EXPIRATION: Duration = Duration::from_secs(60);
pub const DEFAULT_MAX_CHAIN_LENGTH: usize = MAX_AUTH_CHAIN_LINKS;

/// A configuration the verifier refuses to start with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct OptionsError(String);

impl OptionsError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// A resolver could not tell the request's operation or resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolveFailed;

pub type Resolver =
    Arc<dyn Fn(&RequestParts) -> Result<Option<String>, ResolveFailed> + Send + Sync>;

/// A route's operation or resource: fixed, or chosen from the request.
#[derive(Clone)]
pub enum Resolve {
    Static(String),
    Dynamic(Resolver),
}

impl Resolve {
    pub(crate) fn resolve(&self, parts: &RequestParts) -> Result<Option<String>, ResolveFailed> {
        match self {
            Self::Static(value) => Ok(Some(value.clone())),
            Self::Dynamic(resolve) => resolve(parts),
        }
    }
}

/// How [`super::verify_request`] verifies and authorizes; built and checked once.
#[derive(Clone)]
pub struct VerifyOptions {
    pub(crate) expiration_ms: i64,
    pub(crate) max_chain_length: usize,
    pub(crate) canonical_metadata_keys: Option<Vec<String>>,
    pub(crate) metadata_validator: Option<MetadataPredicate>,
    pub(crate) service: Option<String>,
    pub(crate) max_scoped_login_lifetime_ms: i64,
    pub(crate) operation: Option<Resolve>,
    pub(crate) resource: Option<Resolve>,
    pub(crate) optional: bool,
    pub(crate) eip1654: Option<Arc<dyn Eip1654Validator>>,
}

impl VerifyOptions {
    pub fn builder() -> VerifyOptionsBuilder {
        VerifyOptionsBuilder::default()
    }
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            expiration_ms: DEFAULT_EXPIRATION.as_millis() as i64,
            max_chain_length: DEFAULT_MAX_CHAIN_LENGTH,
            canonical_metadata_keys: None,
            metadata_validator: None,
            service: None,
            max_scoped_login_lifetime_ms: DEFAULT_MAX_SCOPED_LOGIN_LIFETIME_MS,
            operation: None,
            resource: None,
            optional: false,
            eip1654: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct VerifyOptionsBuilder {
    expiration: Option<Duration>,
    max_chain_length: Option<usize>,
    canonical_metadata_keys: Option<Vec<String>>,
    metadata_validator: Option<MetadataPredicate>,
    service: Option<String>,
    max_scoped_login_lifetime: Option<Duration>,
    operation: Option<Resolve>,
    resource: Option<Resolve>,
    optional: bool,
    eip1654: Option<Arc<dyn Eip1654Validator>>,
}

impl VerifyOptionsBuilder {
    /// The timestamp window, applied to the past and to the future.
    pub fn expiration(mut self, window: Duration) -> Self {
        self.expiration = Some(window);
        self
    }

    /// At most this many links, the SIGNER included: 2 to 10.
    pub fn max_chain_length(mut self, links: usize) -> Self {
        self.max_chain_length = Some(links);
        self
    }

    /// The metadata fields this service authorizes on, in their canonical
    /// spelling (dotted paths for nested ones). Naming them is what accepts the
    /// pre-6.0.0 payload, and only metadata spelled exactly so.
    pub fn canonical_metadata_keys<I, K>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        self.canonical_metadata_keys = Some(keys.into_iter().map(Into::into).collect());
        self
    }

    pub fn metadata_validator(mut self, validator: MetadataPredicate) -> Self {
        self.metadata_validator = Some(validator);
        self
    }

    /// This service's identifier: accepts scoped logins (ADR-162) that grant it something.
    pub fn service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    /// With a service: the longest remaining validity a scoped login may have.
    pub fn max_scoped_login_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_scoped_login_lifetime = Some(lifetime);
        self
    }

    /// With a service: the operation this route performs.
    pub fn operation(mut self, operation: Resolve) -> Self {
        self.operation = Some(operation);
        self
    }

    /// With an operation: the resource the request acts on.
    pub fn resource(mut self, resource: Resolve) -> Self {
        self.resource = Some(resource);
        self
    }

    /// An unverifiable request passes unauthenticated; a scoped login its route
    /// does not grant is still refused.
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// Asks contract wallets (EIP-1654); without one their links are refused.
    pub fn eip1654(mut self, validator: Arc<dyn Eip1654Validator>) -> Self {
        self.eip1654 = Some(validator);
        self
    }

    pub fn build(self) -> Result<VerifyOptions, OptionsError> {
        let err = |m: &str| Err(OptionsError::new(m));
        if let Some(keys) = &self.canonical_metadata_keys {
            if keys.is_empty() {
                return err(
                    "canonicalMetadataKeys must be a non-empty array of metadata field paths",
                );
            }
            if let Some(key) = keys.iter().find(|k| k.split('.').any(str::is_empty)) {
                return Err(OptionsError::new(format!(
                    "canonicalMetadataKeys entries must be non-empty and have no empty path segments, got: \"{key}\""
                )));
            }
        }
        if self.service.as_deref().is_some_and(|s| !is_service(s)) {
            return err("service must be 1 to 64 lowercase letters, digits or hyphens");
        }
        if let Some(lifetime) = self.max_scoped_login_lifetime {
            if self.service.is_none() {
                return err("maxScopedLoginLifetime needs a service");
            }
            if lifetime.is_zero() {
                return err("maxScopedLoginLifetime must be a positive number of milliseconds");
            }
        }
        if let Some(operation) = &self.operation {
            if self.service.is_none() {
                return err("operation needs a service");
            }
            if let Resolve::Static(op) = operation
                && (op == "*" || !is_operation_format(op))
            {
                return err("operation must be 1 to 64 lowercase letters, digits or hyphens");
            }
        }
        if self.resource.is_some() && self.operation.is_none() {
            return err("resource needs an operation");
        }
        let max_chain_length = self.max_chain_length.unwrap_or(DEFAULT_MAX_CHAIN_LENGTH);
        if !(2..=MAX_AUTH_CHAIN_LINKS).contains(&max_chain_length) {
            return err("maxChainLength must be an integer from 2 to 10");
        }
        let millis = |d: Duration| i64::try_from(d.as_millis()).unwrap_or(i64::MAX);
        Ok(VerifyOptions {
            expiration_ms: millis(self.expiration.unwrap_or(DEFAULT_EXPIRATION)),
            max_chain_length,
            canonical_metadata_keys: self.canonical_metadata_keys,
            metadata_validator: self.metadata_validator,
            service: self.service,
            max_scoped_login_lifetime_ms: self
                .max_scoped_login_lifetime
                .map_or(DEFAULT_MAX_SCOPED_LOGIN_LIFETIME_MS, millis),
            operation: self.operation,
            resource: self.resource,
            optional: self.optional,
            eip1654: self.eip1654,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn misconfiguration_fails_at_build() {
        let ok = |b: VerifyOptionsBuilder| assert!(b.build().is_ok());
        let bad = |b: VerifyOptionsBuilder| assert!(b.build().is_err());
        ok(VerifyOptions::builder());
        bad(VerifyOptions::builder().canonical_metadata_keys(Vec::<String>::new()));
        bad(VerifyOptions::builder().canonical_metadata_keys([""]));
        bad(VerifyOptions::builder().canonical_metadata_keys(["realm..serverName"]));
        ok(VerifyOptions::builder().canonical_metadata_keys(["signer", "realm.serverName"]));
        bad(VerifyOptions::builder().service("Insights"));
        bad(VerifyOptions::builder().max_scoped_login_lifetime(Duration::from_secs(1)));
        bad(VerifyOptions::builder()
            .service("insights")
            .max_scoped_login_lifetime(Duration::ZERO));
        bad(VerifyOptions::builder().operation(Resolve::Static("logs-read".into())));
        bad(VerifyOptions::builder()
            .service("insights")
            .operation(Resolve::Static("*".into())));
        bad(VerifyOptions::builder()
            .service("insights")
            .resource(Resolve::Static("scene:a".into())));
        ok(VerifyOptions::builder()
            .service("insights")
            .operation(Resolve::Static("logs-read".into()))
            .resource(Resolve::Static("scene:a".into())));
        bad(VerifyOptions::builder().max_chain_length(1));
        bad(VerifyOptions::builder().max_chain_length(11));
        ok(VerifyOptions::builder().max_chain_length(2));
    }
}
