//! The routes. Reads take any address text (a malformed one reads no rows,
//! upstream's behaviour); the admin routes take the bearer-gated
//! [`RequireAdmin`] in their signature, the grant and revoke a validated
//! address, and the catalog writes a body [`catalog::validate`] accepts.

use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use super::assets;
use super::catalog::{self, AdminBadgeBody, StateBody};
use super::store::CATEGORIES;
use super::types::{
    BadgeBody, CategoriesBody, Data, GrantResult, PreviewBody, RevokeResult, StatusBody, TiersBody,
    UserBadgesBody,
};
use crate::identity::Address;
use crate::notifications::admin::RequireAdmin;
use crate::notifications::errors::ApiError;

type AppState = Arc<super::State>;

/// The first value of `key`, as `URLSearchParams.get` reads it.
fn query_param<'a>(query: &'a [(String, String)], key: &str) -> Option<&'a str> {
    query
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Upstream's 404 body on the catalog routes.
fn badge_not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "Badge not found" })),
    )
        .into_response()
}

pub async fn get_status(State(state): State<AppState>) -> Json<Data<StatusBody>> {
    Json(Data::new(StatusBody {
        version: env!("CARGO_PKG_VERSION"),
        current_time: chrono::Utc::now().timestamp_millis(),
        commit_hash: state.settings.commit_hash.clone().unwrap_or_default(),
    }))
}

pub async fn get_categories() -> Json<Data<CategoriesBody>> {
    Json(Data::new(CategoriesBody {
        categories: CATEGORIES.to_vec(),
    }))
}

pub async fn get_badges(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(Json(Data::new(state.badges.list_badges().await?)).into_response())
}

pub async fn get_badge(
    State(state): State<AppState>,
    Path(badge_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(match state.badges.badge(&badge_id).await? {
        Some(badge) => Json(Data::new(BadgeBody { badge })).into_response(),
        None => badge_not_found(),
    })
}

/// JavaScript `parseInt` of a decimal: leading whitespace, an optional
/// sign, then the longest run of digits; `None` when there are none.
fn parse_int(raw: &str) -> Option<i64> {
    let s = raw.trim_start();
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let end = digits
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(digits.len());
    let value: i64 = digits[..end].parse().ok()?;
    Some(if negative { -value } else { value })
}

/// `@dcl/http-commons` `getPaginationParams` after the preview handler's
/// default: an absent or empty limit is 5, an unusable one (not a number,
/// not positive, above 100) is 100; an unusable offset is 0.
pub fn preview_page(limit: Option<&str>, offset: Option<&str>) -> (i64, i64) {
    let limit = match limit.filter(|l| !l.is_empty()) {
        None => 5,
        Some(raw) => parse_int(raw)
            .filter(|l| (1..=100).contains(l))
            .unwrap_or(100),
    };
    let offset = offset.and_then(parse_int).filter(|o| *o >= 0).unwrap_or(0);
    (limit, offset)
}

pub async fn get_user_preview(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Json<Data<PreviewBody>>, ApiError> {
    let address = lenient_address(&address);
    let (limit, offset) = preview_page(query_param(&query, "limit"), query_param(&query, "offset"));
    let latest = state
        .badges
        .latest_achieved(&address, limit, offset)
        .await?;
    Ok(Json(Data::new(PreviewBody {
        latest_achieved_badges: latest,
    })))
}

pub async fn get_user_badges(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Json<Data<UserBadgesBody>>, ApiError> {
    let address = lenient_address(&address);
    let include_not_achieved = query_param(&query, "includeNotAchieved") == Some("true");
    let (achieved, not_achieved) = state
        .badges
        .user_badges(&address, include_not_achieved)
        .await?;
    Ok(Json(Data::new(UserBadgesBody {
        achieved,
        not_achieved,
    })))
}

pub async fn get_badge_tiers(
    State(state): State<AppState>,
    Path(badge_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(match state.badges.list_tiers(&badge_id).await? {
        Some(tiers) => Json(Data::new(TiersBody { tiers })).into_response(),
        None => badge_not_found(),
    })
}

/// The admin read of a wallet's raw progress row, upstream's
/// `get-user-badge-details` (404 bodies of `@dcl/http-commons`).
pub async fn get_user_badge(
    _admin: RequireAdmin,
    State(state): State<AppState>,
    Path((address, badge_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let address = lenient_address(&address);
    let not_found = |message: &str| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Not Found", "message": message })),
        )
            .into_response()
    };
    Ok(match state.badges.user_badge(&address, &badge_id).await? {
        None => not_found("Badge does not exists"),
        Some(None) => not_found("User progress not found for this badge"),
        Some(Some(row)) => Json(Data::new(row)).into_response(),
    })
}

#[derive(Debug, Default, Deserialize)]
pub struct GrantBody {
    #[serde(default, rename = "tierId")]
    pub tier_id: Option<String>,
}

pub async fn grant_user_badge(
    admin: RequireAdmin,
    State(state): State<AppState>,
    Path((address, badge_id)): Path<(String, String)>,
    body: Option<Json<GrantBody>>,
) -> Result<Json<Data<GrantResult>>, ApiError> {
    let actor = admin.audit_actor_description();
    let address = normalize_address(&address)?;
    let badge_id = normalize_badge_id(&badge_id)?;
    let tier_id = body
        .and_then(|Json(b)| b.tier_id)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let granted = state
        .badges
        .grant_badge(&address, &badge_id, tier_id.as_deref(), &actor)
        .await?;
    if !granted {
        return Err(ApiError::not_found(format!(
            "no badge found with id: {badge_id}"
        )));
    }
    Ok(Json(Data::new(GrantResult {
        granted: true,
        address: address.into_string(),
        badge_id,
        tier_id,
    })))
}

pub async fn revoke_user_badge(
    admin: RequireAdmin,
    State(state): State<AppState>,
    Path((address, badge_id)): Path<(String, String)>,
) -> Result<Json<Data<RevokeResult>>, ApiError> {
    let actor = admin.audit_actor_description();
    let address = normalize_address(&address)?;
    let badge_id = normalize_badge_id(&badge_id)?;
    let exists = state
        .badges
        .revoke_badge(&address, &badge_id, &actor)
        .await?;
    if !exists {
        return Err(ApiError::not_found(format!(
            "no badge found with id: {badge_id}"
        )));
    }
    Ok(Json(Data::new(RevokeResult {
        revoked: true,
        address: address.into_string(),
        badge_id,
    })))
}

/// `PUT /badges/{id}`: upserts the definition (201 created, 200 replaced).
pub async fn put_badge(
    admin: RequireAdmin,
    State(state): State<AppState>,
    Path(badge_id): Path<String>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let def = catalog::validate(&badge_id, catalog::parse_body(&body)?)?;
    let (badge, created) = state
        .badges
        .upsert_definition(&def, &admin.audit_actor_description())
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(Data::new(AdminBadgeBody { badge }))).into_response())
}

/// `PATCH /badges/{id}` `{published?, archived?}`.
pub async fn patch_badge(
    admin: RequireAdmin,
    State(state): State<AppState>,
    Path(badge_id): Path<String>,
    body: Bytes,
) -> Result<Json<Data<AdminBadgeBody>>, ApiError> {
    let target: StateBody = catalog::parse_body(&body)?;
    if target.published.is_none() && target.archived.is_none() {
        return Err(ApiError::bad_request(
            "the body sets published, archived or both",
        ));
    }
    match state
        .badges
        .set_definition_state(badge_id.trim(), &target, &admin.audit_actor_description())
        .await?
    {
        Some(badge) => Ok(Json(Data::new(AdminBadgeBody { badge }))),
        None => Err(ApiError::not_found(format!(
            "no badge found with id: {}",
            badge_id.trim()
        ))),
    }
}

pub async fn get_asset(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    assets::serve(state.settings.assets_dir.as_ref(), &path).await
}

fn normalize_badge_id(badge_id: &str) -> Result<String, ApiError> {
    let trimmed = badge_id.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("badge_id is required"));
    }
    Ok(trimmed.to_string())
}

fn normalize_address(address: &str) -> Result<Address, ApiError> {
    if address.trim().is_empty() {
        return Err(ApiError::bad_request("address is required"));
    }
    Address::parse(address).map_err(|_| ApiError::bad_request("invalid address"))
}

fn lenient_address(address: &str) -> String {
    Address::parse(address)
        .map(Address::into_string)
        .unwrap_or_else(|_| address.trim().to_string())
}

#[cfg(test)]
mod normalize_address_tests {
    use super::{lenient_address, normalize_address, preview_page};

    #[test]
    fn preview_paging_follows_upstream() {
        assert_eq!(preview_page(None, None), (5, 0));
        assert_eq!(preview_page(Some(""), Some("")), (5, 0));
        assert_eq!(preview_page(Some("2"), Some("3")), (2, 3));
        assert_eq!(preview_page(Some(" 7abc"), Some("1.9")), (7, 1));
        assert_eq!(preview_page(Some("0"), Some("-1")), (100, 0));
        assert_eq!(preview_page(Some("101"), Some("x")), (100, 0));
        assert_eq!(preview_page(Some("x"), None), (100, 0));
        assert_eq!(preview_page(Some("100"), None), (100, 0));
    }

    #[test]
    fn accepts_and_lowercases_valid_addresses() {
        assert_eq!(
            normalize_address(" 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 ")
                .unwrap()
                .as_str(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(
            lenient_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(lenient_address(" not-an-address "), "not-an-address");
    }

    #[test]
    fn rejects_malformed_addresses() {
        assert!(normalize_address("not-an-address").is_err());
        assert!(normalize_address("0x1234").is_err());
        assert!(normalize_address("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ").is_err());
        assert!(normalize_address("  ").is_err());
    }
}
