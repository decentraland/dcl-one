//! The operator bearer (`CATALYRST_ECONOMY_ADMIN_TOKEN`, read from `_FILE`)
//! in front of every money route, compared in constant time and failing
//! closed when unset; the relayer status, toggle and signer preference.
//! The old crate also accepted these routes without a bearer on a loopback
//! bind; v2 serves one public listener, so the bearer is the only door.

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::admin_bearer::{bearer_token, timing_safe_eq};
use crate::evm::SharedState;
use crate::evm::admin::SignerPreference;
use crate::evm::errors::ApiError;

const ADMIN_TOKEN_ENV: &str = "CATALYRST_ECONOMY_ADMIN_TOKEN";

fn clean_actor(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(100).collect())
    }
}

fn audit_actor(headers: &HeaderMap) -> String {
    headers
        .get("x-catalyrst-admin")
        .and_then(|v| v.to_str().ok())
        .and_then(clean_actor)
        .unwrap_or_else(|| "console".to_string())
}

pub(crate) fn require_admin(state: &SharedState, headers: &HeaderMap) -> Result<(), ApiError> {
    check_bearer(state.settings.admin_token.as_deref(), bearer_token(headers))
}

fn check_bearer(expected: Option<&str>, presented: Option<&str>) -> Result<(), ApiError> {
    let Some(expected) = expected.filter(|s| !s.is_empty()) else {
        return Err(ApiError::Forbidden(format!(
            "Admin controls are disabled ({ADMIN_TOKEN_ENV} is unset)."
        )));
    };
    match presented {
        Some(got) if timing_safe_eq(got.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => Err(ApiError::Forbidden(
            "Invalid or missing admin bearer token.".into(),
        )),
    }
}

pub struct RequireAdmin(());

impl FromRequestParts<SharedState> for RequireAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        require_admin(state, &parts.headers)?;
        Ok(RequireAdmin(()))
    }
}

fn status_json(state: &SharedState) -> Value {
    let pref = state.runtime.signer_preference();
    json!({
        "ok": true,
        "relayerEnabled": state.runtime.relayer_enabled(),
        "signerPreference": pref,
        "provisioned": {
            "oz": state.transaction.has_oz_relayer(),
            "direct": state.transaction.has_direct_signer(),
            "upstream": state.transaction.has_upstream_forwarder(),
        },
    })
}

pub async fn relayer_status(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(status_json(&state)))
}

#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

pub async fn relayer_toggle(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    headers: HeaderMap,
    body: Result<Json<ToggleBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let actor = audit_actor(&headers);
    let Json(body) = body.map_err(|e| ApiError::MalformedBody(e.body_text()))?;
    let previous = state.runtime.relayer_enabled();
    state.runtime.set_relayer_enabled(body.enabled);
    tracing::info!(
        audit = true,
        actor = %actor,
        action = "relayer.toggle",
        previous = previous,
        enabled = body.enabled,
        "admin: relayer broadcast toggle changed"
    );
    Ok(Json(status_json(&state)))
}

#[derive(Debug, Deserialize)]
pub struct SignerBody {
    pub preference: SignerPreference,
}

pub async fn relayer_signer(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    headers: HeaderMap,
    body: Result<Json<SignerBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let actor = audit_actor(&headers);
    let Json(body) = body.map_err(|e| ApiError::MalformedBody(e.body_text()))?;
    let previous = state.runtime.signer_preference();
    state.runtime.set_signer_preference(body.preference);
    tracing::info!(
        audit = true,
        actor = %actor,
        action = "relayer.signer",
        previous = ?previous,
        preference = ?body.preference,
        "admin: relayer signer preference changed"
    );
    Ok(Json(status_json(&state)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forbidden(r: Result<(), ApiError>) -> bool {
        matches!(r, Err(ApiError::Forbidden(_)))
    }

    #[test]
    fn unset_token_fails_closed() {
        assert!(forbidden(check_bearer(None, None)));
        assert!(forbidden(check_bearer(None, Some("anything"))));
        assert!(forbidden(check_bearer(Some(""), Some(""))));
    }

    #[test]
    fn unset_token_names_the_env_var() {
        let err = check_bearer(None, Some("anything")).unwrap_err();
        assert!(
            matches!(err, ApiError::Forbidden(m) if m == "Admin controls are disabled (CATALYRST_ECONOMY_ADMIN_TOKEN is unset).")
        );
    }

    #[test]
    fn missing_or_wrong_bearer_is_forbidden() {
        assert!(forbidden(check_bearer(Some("secret"), None)));
        assert!(forbidden(check_bearer(Some("secret"), Some("wrong"))));
        assert!(forbidden(check_bearer(
            Some("secret"),
            Some("secretsecret")
        )));
        let err = check_bearer(Some("secret"), Some("wrong")).unwrap_err();
        assert!(
            matches!(err, ApiError::Forbidden(m) if m == "Invalid or missing admin bearer token.")
        );
    }

    #[test]
    fn matching_bearer_is_allowed() {
        assert!(check_bearer(Some("secret"), Some("secret")).is_ok());
    }

    #[test]
    fn bearer_token_parses_scheme() {
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer tok123".parse().unwrap());
        assert_eq!(bearer_token(&h), Some("tok123"));
        let mut h2 = HeaderMap::new();
        h2.insert("authorization", "Basic tok123".parse().unwrap());
        assert_eq!(bearer_token(&h2), None);
    }

    #[test]
    fn clean_actor_trims_and_caps() {
        assert_eq!(clean_actor("  alice  ").as_deref(), Some("alice"));
        assert_eq!(clean_actor("   "), None);
        assert_eq!(clean_actor(""), None);
        let long: String = "x".repeat(250);
        assert_eq!(clean_actor(&long).map(|s| s.len()), Some(100));
    }

    #[test]
    fn audit_actor_prefers_header_then_falls_back_to_console() {
        assert_eq!(audit_actor(&HeaderMap::new()), "console");
        let mut h = HeaderMap::new();
        h.insert("x-catalyrst-admin", "0xAdmin".parse().unwrap());
        assert_eq!(audit_actor(&h), "0xAdmin");
        let mut h2 = HeaderMap::new();
        h2.insert("x-catalyrst-admin", "   ".parse().unwrap());
        assert_eq!(audit_actor(&h2), "console");
    }
}
