//! The static-bearer admin gate the notifications broadcast and the badges
//! grant/revoke use. [`verify_admin_bearer`] maps the shared
//! [`crate::auth::admin_bearer`] gate (constant time, fails closed when the
//! secret is unset) onto the shared
//! [`AuthorityNotEstablished`] vocabulary; [`RequireAdmin`] is the axum
//! extractor a mutation handler must *name in its signature*, so the check
//! cannot be deleted from a handler body. Both services collapsed every
//! refusal to **403** with a fixed message on the wire; that contract is kept
//! by [`to_api_error`], the 401/503 distinction stays in the logs.

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::request::Parts;

use super::errors::ApiError;
use crate::auth::admin_bearer::Refusal;
use crate::identity::AuthorityNotEstablished;

/// The verified service identity: the request presented the operator's
/// secret for `environment_variable`. It says a service called, not who.
#[derive(Debug)]
pub struct AdminIdentity {
    environment_variable: &'static str,
}

impl AdminIdentity {
    /// `service-token:<VAR>`: the audit actor, built from the server-chosen
    /// variable name, never from client text.
    pub fn audit_actor_description(&self) -> String {
        format!("service-token:{}", self.environment_variable)
    }
}

/// Fails closed: an unset or empty secret refuses everything as
/// `CredentialNotConfigured`; a missing bearer is `AuthenticationMissingOrInvalid`;
/// a wrong one `PresentedSharedSecretDidNotMatch`.
pub fn verify_admin_bearer(
    environment_variable: &'static str,
    configured: Option<&str>,
    headers: &HeaderMap,
) -> Result<AdminIdentity, AuthorityNotEstablished> {
    match crate::auth::admin_bearer::check(configured, headers) {
        Ok(()) => Ok(AdminIdentity {
            environment_variable,
        }),
        Err(Refusal::NotConfigured) => Err(AuthorityNotEstablished::CredentialNotConfigured {
            environment_variable,
        }),
        Err(Refusal::NoBearer) => Err(AuthorityNotEstablished::AuthenticationMissingOrInvalid {
            detail: "no shared secret presented",
        }),
        Err(Refusal::Mismatch) => Err(AuthorityNotEstablished::PresentedSharedSecretDidNotMatch),
    }
}

/// The old wire: every refusal is a 403 with the service's two fixed
/// messages (unset secret, anything else); the 401/503 distinction goes to
/// the log.
pub fn to_api_error(
    refusal: AuthorityNotEstablished,
    unset_message: &str,
    mismatch_message: &str,
) -> ApiError {
    tracing::warn!(refusal = %refusal, status = refusal.http_status(), "admin bearer refused");
    match refusal {
        AuthorityNotEstablished::CredentialNotConfigured { .. } => {
            ApiError::forbidden(unset_message)
        }
        _ => ApiError::forbidden(mismatch_message),
    }
}

/// What a state must expose for [`RequireAdmin`] to be minted against it.
pub trait AdminSecretSource: Send + Sync {
    const ENVIRONMENT_VARIABLE: &'static str;
    /// The message a request gets while the secret is unset.
    const UNSET_MESSAGE: &'static str;
    /// The message a missing or wrong bearer gets.
    const MISMATCH_MESSAGE: &'static str;
    fn admin_token(&self) -> Option<&str>;
}

/// Proof, in a handler's signature, that the request carried the admin
/// bearer. The private field means only the extractor below mints one.
pub struct RequireAdmin(AdminIdentity);

impl RequireAdmin {
    pub fn audit_actor_description(&self) -> String {
        self.0.audit_actor_description()
    }
}

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: AdminSecretSource,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        verify_admin_bearer(S::ENVIRONMENT_VARIABLE, state.admin_token(), &parts.headers)
            .map(RequireAdmin)
            .map_err(|r| to_api_error(r, S::UNSET_MESSAGE, S::MISMATCH_MESSAGE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VAR: &str = "CATALYRST_TEST_ADMIN_TOKEN";

    fn headers(authorization: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(v) = authorization {
            h.insert("authorization", v.parse().unwrap());
        }
        h
    }

    #[test]
    fn fails_closed_when_token_unset() {
        for configured in [None, Some("")] {
            for presented in [Some("Bearer anything"), Some("Bearer "), None] {
                let err = verify_admin_bearer(VAR, configured, &headers(presented)).unwrap_err();
                assert!(
                    matches!(err, AuthorityNotEstablished::CredentialNotConfigured { environment_variable } if environment_variable == VAR)
                );
                assert_eq!(err.http_status(), 503);
                let api = to_api_error(err, "disabled", "nope");
                assert_eq!((api.status(), api.to_string().as_str()), (403, "disabled"));
            }
        }
    }

    #[test]
    fn rejects_missing_or_wrong_token() {
        let err = verify_admin_bearer(VAR, Some("secret"), &headers(None)).unwrap_err();
        assert!(matches!(
            err,
            AuthorityNotEstablished::AuthenticationMissingOrInvalid { .. }
        ));
        for wrong in [
            "Bearer nope",
            "Bearer secre",
            "Bearer ",
            "Basic secret",
            "Bearer secrets",
        ] {
            let err = verify_admin_bearer(VAR, Some("secret"), &headers(Some(wrong))).unwrap_err();
            assert!(
                matches!(
                    err,
                    AuthorityNotEstablished::PresentedSharedSecretDidNotMatch
                        | AuthorityNotEstablished::AuthenticationMissingOrInvalid { .. }
                ),
                "{wrong}: {err}"
            );
            let api = to_api_error(err, "disabled", "nope");
            assert_eq!(
                (api.status(), api.to_string().as_str()),
                (403, "nope"),
                "{wrong}"
            );
        }
    }

    #[test]
    fn accepts_matching_token_and_names_the_variable_as_actor() {
        let id = verify_admin_bearer(VAR, Some("secret"), &headers(Some("Bearer secret"))).unwrap();
        assert_eq!(id.audit_actor_description(), format!("service-token:{VAR}"));
    }
}
