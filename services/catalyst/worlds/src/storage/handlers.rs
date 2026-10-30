//! The route handlers: every one verifies the signed fetch and resolves the
//! scene context first ([`super::resolve_authorized`]), then reads or writes
//! only the resolved [`SceneNamespace`]; an upsert answers its 411/413
//! before the authorization ([`super::resolve_authorized_upsert`]). Values
//! are served as the jsonb text Postgres returns, spliced into the envelope
//! byte for byte (a stored `0`, `false`, `""` or `null` is a 200, never a
//! 404).

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, OriginalUri, Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::shared::identity::Address;

use super::errors::ApiError;
use super::store::{StorageEntry, value_size_bytes};
use super::{AuthPolicy, SharedState, resolve_authorized, resolve_authorized_upsert};

pub const MAX_LIMIT: i64 = 100;
pub const MAX_OFFSET: i64 = 100_000;
pub const CONFIRM_DELETE_ALL_HEADER: &str = "x-confirm-delete-all";

pub const MAX_KEY_LENGTH: usize = 255;

pub const BODY_ENVELOPE_SLACK_BYTES: i64 = 1024;

/// 1..=255 code points (the old varchar(255) measured characters; the
/// schema's `octet_length(key) <= 1020` CHECK is the byte bound of that).
pub fn validate_key(key: &str) -> Result<(), ApiError> {
    if key.is_empty() || (key.len() > MAX_KEY_LENGTH && key.chars().count() > MAX_KEY_LENGTH) {
        return Err(ApiError::bad_request(format!(
            "Key must be between 1 and {MAX_KEY_LENGTH} characters"
        )));
    }
    Ok(())
}

pub fn check_content_length(headers: &HeaderMap, max_value_size: i64) -> Result<(), ApiError> {
    let content_length = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 0);
    let Some(content_length) = content_length else {
        return Err(ApiError::LengthRequired(
            "Requests with a body must include a valid Content-Length header".to_string(),
        ));
    };
    let max_body_size = max_value_size + BODY_ENVELOPE_SLACK_BYTES;
    if content_length > max_body_size {
        return Err(ApiError::PayloadTooLarge(format!(
            "Request body exceeds the maximum allowed size ({max_body_size} bytes)"
        )));
    }
    Ok(())
}

/// jsonb cannot hold `\u0000`; the escape sequence is refused unless it is
/// itself escaped (`\\u0000` is literal text).
pub fn reject_nul_characters(serialized: &str) -> Result<(), ApiError> {
    let bytes = serialized.as_bytes();
    let needle = b"\\u0000";
    let mut from = 0;
    while let Some(pos) = find_from(bytes, needle, from) {
        let backslashes_before = bytes[..pos]
            .iter()
            .rev()
            .take_while(|&&b| b == b'\\')
            .count();
        if backslashes_before % 2 == 0 {
            return Err(ApiError::bad_request(
                "Values must not contain the \\u0000 (NUL) character",
            ));
        }
        from = pos + 1;
    }
    Ok(())
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub limit: i64,
    pub offset: i64,
    pub prefix: Option<String>,
}

/// URLSearchParams.get uses the first occurrence of a repeated key.
pub(super) fn query_params(query: Option<&str>) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        params
            .entry(key.into_owned())
            .or_insert_with(|| value.into_owned());
    }
    params
}

/// `parseInt(v, 10)`: optional leading whitespace and sign, then the leading
/// digits (saturating); `None` when there are none.
fn parse_int_prefix(v: &str) -> Option<i64> {
    let v = v.trim_start_matches(|c: char| {
        matches!(c,
        '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}')
    });
    let (negative, digits) = match v.as_bytes().first() {
        Some(b'-') => (true, &v[1..]),
        Some(b'+') => (false, &v[1..]),
        _ => (false, v),
    };
    let digits = &digits[..digits.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    let n = digits.bytes().fold(0i64, |n, d| {
        n.saturating_mul(10).saturating_add(i64::from(d - b'0'))
    });
    Some(if negative { -n } else { n })
}

/// As `@dcl/http-commons` 2.0.1: `limit` in 1..=100 (anything else is 100),
/// `offset` >= 0 clamped to [`MAX_OFFSET`] (anything else 0), both parsed with
/// `parseInt` semantics; `prefix` when non-empty; a `page` parameter is
/// ignored, as the old service did.
pub fn parse_pagination(params: &HashMap<String, String>) -> Pagination {
    let int = |name: &str| params.get(name).and_then(|v| parse_int_prefix(v));
    let limit = match int("limit") {
        Some(n) if n > 0 && n <= MAX_LIMIT => n,
        _ => MAX_LIMIT,
    };
    let offset = match int("offset") {
        Some(n) if n >= 0 => n.min(MAX_OFFSET),
        _ => 0,
    };
    let prefix = params.get("prefix").cloned().filter(|s| !s.is_empty());
    Pagination {
        limit,
        offset,
        prefix,
    }
}

pub fn require_confirm_delete_all(headers: &HeaderMap) -> Result<(), ApiError> {
    if headers.get(CONFIRM_DELETE_ALL_HEADER).is_none() {
        return Err(ApiError::bad_request(
            "Missing required header: X-Confirm-Delete-All",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpsertBody {
    pub value: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpsertEnvBody {
    pub value: String,
}

pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        req: axum::http::Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|rejection: JsonRejection| ApiError::bad_request(rejection.body_text()))?;
        Ok(ValidatedJson(value))
    }
}

/// The path segment before the signed fetch is verified: trimmed and folded
/// (an empty one is refused here, an invalid one after the credential, as
/// the old handlers ordered it).
pub fn normalize_player(address: &str) -> Result<String, ApiError> {
    let t = address.trim();
    if t.is_empty() {
        return Err(ApiError::bad_request("player_address is required"));
    }
    Ok(t.to_ascii_lowercase())
}

pub fn parse_player(normalized: &str) -> Result<Address, ApiError> {
    Address::parse(normalized).map_err(|_| ApiError::bad_request("Invalid player address"))
}

#[derive(Debug)]
pub struct RawJson(pub String);

impl axum::response::IntoResponse for RawJson {
    fn into_response(self) -> axum::response::Response {
        (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            self.0,
        )
            .into_response()
    }
}

pub fn raw_value_response(serialized: &str) -> RawJson {
    RawJson(format!("{{\"value\":{serialized}}}"))
}

pub fn raw_paginated_response(
    entries: &[StorageEntry],
    limit: i64,
    offset: i64,
    total: i64,
) -> RawJson {
    let mut data = String::from("[");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            data.push(',');
        }
        data.push_str("{\"key\":");
        data.push_str(&serde_json::to_string(&e.key).expect("strings always serialize"));
        data.push_str(",\"value\":");
        data.push_str(&e.value);
        data.push('}');
    }
    data.push(']');
    RawJson(format!(
        "{{\"data\":{data},\"pagination\":{{\"limit\":{limit},\"offset\":{offset},\"total\":{total}}}}}"
    ))
}

pub fn get_value_response(value: Option<Arc<str>>) -> Result<RawJson, ApiError> {
    match value {
        Some(v) => Ok(raw_value_response(&v)),
        None => Err(ApiError::not_found("Value not found")),
    }
}

/// Body of `GET /usage/world`, `GET /usage/env` and `GET /usage/players/{player}`.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "world-storage/")
)]
pub struct UsageResponse {
    #[serde(rename = "usedBytes")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub used_bytes: i64,
    #[serde(rename = "maxTotalSizeBytes")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub max_total_size_bytes: i64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "world-storage/")
)]
pub struct PaginationInfo {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub limit: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub offset: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

/// Body of `GET /env` (variable names) and `GET /players` (addresses).
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "world-storage/")
)]
pub struct KeyListResponse {
    pub data: Vec<String>,
    pub pagination: PaginationInfo,
}

/// One row of a values listing: the stored key plus its arbitrary JSON value.
#[cfg(feature = "ts")]
#[derive(ts_rs::TS)]
#[ts(export, export_to = "world-storage/")]
pub struct StorageValueRow {
    pub key: String,
    #[ts(type = "unknown")]
    pub value: (),
}

/// Body of `GET /values` and `GET /players/{player}`, as
/// [`raw_paginated_response`] splices it.
#[cfg(feature = "ts")]
#[derive(ts_rs::TS)]
#[ts(export, export_to = "world-storage/")]
pub struct ValuesListResponse {
    pub data: Vec<StorageValueRow>,
    pub pagination: PaginationInfo,
}

fn original_of(parts: &axum::http::request::Parts) -> Option<&Uri> {
    parts.extensions.get::<OriginalUri>().map(|o| &o.0)
}

pub async fn ping() -> &'static str {
    "pong"
}

pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

pub async fn world_get(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<RawJson, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    validate_key(&key)?;
    get_value_response(state.store.world_get(&ctx.namespace, &key).await?)
}

pub async fn world_upsert(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    req: axum::extract::Request,
) -> Result<RawJson, ApiError> {
    let (parts, body) = req.into_parts();
    let ctx = resolve_authorized_upsert(
        &state,
        &parts.headers,
        &parts.uri,
        original_of(&parts),
        AuthPolicy::LOGS_ACCESS,
        state.settings.world_limits.max_value_size_bytes,
    )
    .await?;
    validate_key(&key)?;

    let req = axum::http::Request::from_parts(parts, body);
    let ValidatedJson(body) = ValidatedJson::<UpsertBody>::from_request(req, &()).await?;

    let serialized = serde_json::to_string(&body.value)
        .map_err(|e| ApiError::bad_request(format!("invalid value: {e}")))?;
    reject_nul_characters(&serialized)?;

    state
        .store
        .world_upsert_with_quota(
            &ctx.namespace,
            &key,
            &serialized,
            state.settings.world_limits,
        )
        .await?;
    Ok(raw_value_response(&serialized))
}

pub async fn world_delete(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    validate_key(&key)?;
    state.store.world_delete(&ctx.namespace, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn world_list(
    State(state): State<SharedState>,
    RawQuery(params): RawQuery,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<RawJson, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    let p = parse_pagination(&query_params(params.as_deref()));
    let (entries, total) = state
        .store
        .world_list_page(&ctx.namespace, p.limit, p.offset, p.prefix.as_deref())
        .await?;
    Ok(raw_paginated_response(&entries, p.limit, p.offset, total))
}

pub async fn world_clear(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    require_confirm_delete_all(&headers)?;
    state.store.world_delete_all(&ctx.namespace).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn player_get(
    State(state): State<SharedState>,
    Path((player, key)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<RawJson, ApiError> {
    let player = normalize_player(&player)?;
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    let player = parse_player(&player)?;
    validate_key(&key)?;
    get_value_response(
        state
            .store
            .player_get(&ctx.namespace, &player, &key)
            .await?,
    )
}

pub async fn player_upsert(
    State(state): State<SharedState>,
    Path((player, key)): Path<(String, String)>,
    req: axum::extract::Request,
) -> Result<RawJson, ApiError> {
    let player = normalize_player(&player)?;
    let (parts, body) = req.into_parts();
    let ctx = resolve_authorized_upsert(
        &state,
        &parts.headers,
        &parts.uri,
        original_of(&parts),
        AuthPolicy::LOGS_ACCESS,
        state.settings.player_limits.max_value_size_bytes,
    )
    .await?;
    let player = parse_player(&player)?;
    validate_key(&key)?;

    let req = axum::http::Request::from_parts(parts, body);
    let ValidatedJson(body) = ValidatedJson::<UpsertBody>::from_request(req, &()).await?;

    let serialized = serde_json::to_string(&body.value)
        .map_err(|e| ApiError::bad_request(format!("invalid value: {e}")))?;
    reject_nul_characters(&serialized)?;

    state
        .store
        .player_upsert_with_quota(
            &ctx.namespace,
            &player,
            &key,
            &serialized,
            state.settings.player_limits,
        )
        .await?;
    Ok(raw_value_response(&serialized))
}

pub async fn player_delete(
    State(state): State<SharedState>,
    Path((player, key)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let player = normalize_player(&player)?;
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    let player = parse_player(&player)?;
    validate_key(&key)?;
    state
        .store
        .player_delete(&ctx.namespace, &player, &key)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn player_list(
    State(state): State<SharedState>,
    Path(player): Path<String>,
    RawQuery(params): RawQuery,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<RawJson, ApiError> {
    let player = normalize_player(&player)?;
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    let player = parse_player(&player)?;
    let p = parse_pagination(&query_params(params.as_deref()));
    let (entries, total) = state
        .store
        .player_list_page(
            &ctx.namespace,
            &player,
            p.limit,
            p.offset,
            p.prefix.as_deref(),
        )
        .await?;
    Ok(raw_paginated_response(&entries, p.limit, p.offset, total))
}

pub async fn player_clear(
    State(state): State<SharedState>,
    Path(player): Path<String>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let player = normalize_player(&player)?;
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    let player = parse_player(&player)?;
    require_confirm_delete_all(&headers)?;
    state
        .store
        .player_delete_all_for_player(&ctx.namespace, &player)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn players_list(
    State(state): State<SharedState>,
    RawQuery(params): RawQuery,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<KeyListResponse>, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::LOGS_ACCESS,
    )
    .await?;
    let p = parse_pagination(&query_params(params.as_deref()));
    let (players, total) = state
        .store
        .player_list_players_page(&ctx.namespace, p.limit, p.offset)
        .await?;
    Ok(Json(KeyListResponse {
        data: players,
        pagination: PaginationInfo {
            limit: p.limit,
            offset: p.offset,
            total,
        },
    }))
}

pub async fn players_clear_all(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    require_confirm_delete_all(&headers)?;
    state.store.player_delete_all(&ctx.namespace).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn env_get(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<Value>, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::AUTHORIZED_ADDRESSES_OR_SCOPED_DELEGATION,
    )
    .await?;
    validate_key(&key)?;
    let encryptor = state.encryptor()?;
    match state.store.env_get_enc(&ctx.namespace, &key).await? {
        Some(blob) => Ok(Json(json!({ "value": encryptor.decrypt(&blob)? }))),
        None => Err(ApiError::not_found("Value not found")),
    }
}

pub async fn env_upsert(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    req: axum::extract::Request,
) -> Result<StatusCode, ApiError> {
    let (parts, body) = req.into_parts();
    let ctx = resolve_authorized_upsert(
        &state,
        &parts.headers,
        &parts.uri,
        original_of(&parts),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
        state.settings.env_limits.max_value_size_bytes,
    )
    .await?;
    validate_key(&key)?;
    let encryptor = state.encryptor()?;

    let req = axum::http::Request::from_parts(parts, body);
    let ValidatedJson(body) = ValidatedJson::<UpsertEnvBody>::from_request(req, &()).await?;

    let size = value_size_bytes(&body.value);
    let enc = encryptor.encrypt(&body.value)?;
    state
        .store
        .env_upsert_with_quota(&ctx.namespace, &key, &enc, size, state.settings.env_limits)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn env_delete(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    validate_key(&key)?;
    state.store.env_delete(&ctx.namespace, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn env_list_keys(
    State(state): State<SharedState>,
    RawQuery(params): RawQuery,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<KeyListResponse>, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    let p = parse_pagination(&query_params(params.as_deref()));
    let (keys, total) = state
        .store
        .env_list_keys_page(&ctx.namespace, p.limit, p.offset, p.prefix.as_deref())
        .await?;
    Ok(Json(KeyListResponse {
        data: keys,
        pagination: PaginationInfo {
            limit: p.limit,
            offset: p.offset,
            total,
        },
    }))
}

pub async fn env_clear(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<StatusCode, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "delete",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    require_confirm_delete_all(&headers)?;
    state.store.env_delete_all(&ctx.namespace).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn usage_world(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<UsageResponse>, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::DEFAULT,
    )
    .await?;
    let info = state.store.world_size_info(&ctx.namespace).await?;
    Ok(Json(UsageResponse {
        used_bytes: info.total_size,
        max_total_size_bytes: state.settings.world_limits.max_total_size_bytes,
    }))
}

pub async fn usage_player(
    State(state): State<SharedState>,
    Path(player): Path<String>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<UsageResponse>, ApiError> {
    let player = normalize_player(&player)?;
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::DEFAULT,
    )
    .await?;
    let player = parse_player(&player)?;
    let info = state
        .store
        .player_size_info(&ctx.namespace, &player)
        .await?;
    Ok(Json(UsageResponse {
        used_bytes: info.total_size,
        max_total_size_bytes: state.settings.player_limits.max_total_size_bytes,
    }))
}

pub async fn usage_env(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<UsageResponse>, ApiError> {
    let ctx = resolve_authorized(
        &state,
        &headers,
        "get",
        &uri,
        Some(&original),
        AuthPolicy::OWNERS_DEPLOYERS_ONLY,
    )
    .await?;
    let info = state.store.env_size_info(&ctx.namespace).await?;
    Ok(Json(UsageResponse {
        used_bytes: info.total_size,
        max_total_size_bytes: state.settings.env_limits.max_total_size_bytes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn key_length_is_validated_in_code_points() {
        assert!(validate_key("k").is_ok());
        assert!(validate_key(&"k".repeat(255)).is_ok());
        assert!(
            validate_key(&"\u{E9}".repeat(255)).is_ok(),
            "255 code points, 510 bytes"
        );
        assert!(matches!(validate_key(""), Err(ApiError::BadRequest(_))));
        assert!(matches!(
            validate_key(&"k".repeat(256)),
            Err(ApiError::BadRequest(_))
        ));
        assert!(matches!(
            validate_key(&"\u{E9}".repeat(256)),
            Err(ApiError::BadRequest(_))
        ));
    }

    #[test]
    fn nul_characters_are_rejected_but_escaped_backslash_text_is_not() {
        let with_nul = serde_json::to_string(&json!({ "a": "b\u{0}c" })).unwrap();
        assert!(matches!(
            reject_nul_characters(&with_nul),
            Err(ApiError::BadRequest(_))
        ));

        let literal_text = serde_json::to_string(&json!({ "a": "\\u0000" })).unwrap();
        assert!(reject_nul_characters(&literal_text).is_ok());

        let backslash_then_nul = serde_json::to_string(&json!({ "a": "\\\u{0}" })).unwrap();
        assert!(reject_nul_characters(&backslash_then_nul).is_err());

        assert!(reject_nul_characters(r#"{"a":"plain"}"#).is_ok());
    }

    #[test]
    fn content_length_precheck_maps_to_411_and_413() {
        let mut headers = HeaderMap::new();
        let err = check_content_length(&headers, 100).unwrap_err();
        assert!(matches!(err, ApiError::LengthRequired(_)));
        assert_eq!(err.into_response().status(), StatusCode::LENGTH_REQUIRED);

        headers.insert("content-length", "not-a-number".parse().unwrap());
        assert!(matches!(
            check_content_length(&headers, 100),
            Err(ApiError::LengthRequired(_))
        ));

        headers.insert(
            "content-length",
            (100 + BODY_ENVELOPE_SLACK_BYTES)
                .to_string()
                .parse()
                .unwrap(),
        );
        assert!(check_content_length(&headers, 100).is_ok());

        headers.insert(
            "content-length",
            (101 + BODY_ENVELOPE_SLACK_BYTES)
                .to_string()
                .parse()
                .unwrap(),
        );
        let err = check_content_length(&headers, 100).unwrap_err();
        assert!(matches!(err, ApiError::PayloadTooLarge(_)));
        assert_eq!(err.into_response().status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn pagination_uses_js_whitespace_and_the_first_query_value() {
        let p = parse_pagination(&query_params(Some("limit=5&limit=9&offset=2&offset=7")));
        assert_eq!((p.limit, p.offset), (5, 2));
        assert_eq!(parse_int_prefix("\u{feff}7"), Some(7));
        assert_eq!(parse_int_prefix("\u{0085}7"), None);
    }

    #[test]
    fn pagination_caps_and_coerces_like_upstream() {
        let p = parse_pagination(&params(&[]));
        assert_eq!((p.limit, p.offset), (MAX_LIMIT, 0));
        assert_eq!(
            parse_pagination(&params(&[("limit", "500")])).limit,
            MAX_LIMIT
        );
        let p = parse_pagination(&params(&[
            ("limit", "50"),
            ("offset", "10"),
            ("prefix", "a"),
        ]));
        assert_eq!(
            (p.limit, p.offset, p.prefix.as_deref()),
            (50, 10, Some("a"))
        );
        for bad in ["abc", "0", "-1"] {
            assert_eq!(
                parse_pagination(&params(&[("limit", bad)])).limit,
                MAX_LIMIT
            );
        }
        for bad in ["abc", "-1"] {
            assert_eq!(parse_pagination(&params(&[("offset", bad)])).offset, 0);
        }
        assert_eq!(parse_pagination(&params(&[("prefix", "")])).prefix, None);
    }

    #[test]
    fn pagination_parses_leading_integers_and_clamps_the_offset_like_http_commons() {
        let huge = "1".repeat(21);
        for ((limit, offset), expected) in [
            (("10abc", "0"), (10, 0)),
            (("10", "5.9"), (10, 5)),
            (("10", "999999999999"), (10, MAX_OFFSET)),
            (("10", huge.as_str()), (10, MAX_OFFSET)),
            ((" +7", "-3"), (7, 0)),
            (("-0", "100000"), (MAX_LIMIT, MAX_OFFSET)),
        ] {
            let p = parse_pagination(&params(&[("limit", limit), ("offset", offset)]));
            assert_eq!(
                (p.limit, p.offset),
                expected,
                "limit={limit} offset={offset}"
            );
        }
    }

    #[test]
    fn pagination_ignores_a_page_parameter() {
        let p = parse_pagination(&params(&[("limit", "10"), ("page", "3")]));
        assert_eq!(
            (p.limit, p.offset),
            (10, 0),
            "world-storage listing paginates on limit/offset only"
        );
    }

    #[test]
    fn player_addresses_are_folded_then_validated() {
        assert_eq!(normalize_player(" 0xAbC ").unwrap(), "0xabc");
        assert!(matches!(
            normalize_player("  "),
            Err(ApiError::BadRequest(_))
        ));
        assert_eq!(
            parse_player("0xabcdef0123456789abcdef0123456789abcdef01")
                .unwrap()
                .as_str(),
            "0xabcdef0123456789abcdef0123456789abcdef01"
        );
        for bad in [
            "",
            "0x123",
            "not-an-address",
            "0x000000000000000000000000000000000000000g",
            "0000000000000000000000000000000000000000",
        ] {
            assert!(
                matches!(parse_player(bad), Err(ApiError::BadRequest(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn stored_falsy_value_is_200_not_404() {
        for v in [json!(0), json!(false), json!(""), json!(null)] {
            let serialized: Arc<str> = Arc::from(serde_json::to_string(&v).unwrap());
            let body = get_value_response(Some(serialized))
                .expect("stored falsy value should yield a 200 body")
                .0;
            assert_eq!(body, serde_json::to_string(&json!({ "value": v })).unwrap());
        }
    }

    fn gnarly_values() -> Vec<Value> {
        vec![
            json!({"quotes":"a\"b","backslash":"c\\d","nl":"e\nf","tab":"g\th"}),
            json!({"unicode":"h\u{E9}llo \u{1F30D} \u{2028}","empty":{},"nested":{"n":[1,2.5,null,true],"s":""}}),
            json!([1, "two", {"three": 3}, []]),
            json!("\\u0041 literal escape text"),
            json!(-0.5),
        ]
    }

    #[test]
    fn raw_value_splice_is_byte_identical_to_the_decoded_path() {
        for v in gnarly_values() {
            let serialized = serde_json::to_string(&v).unwrap();
            assert_eq!(
                raw_value_response(&serialized).0,
                serde_json::to_string(&json!({ "value": v })).unwrap()
            );
        }
    }

    #[test]
    fn raw_paginated_splice_is_byte_identical_to_the_decoded_path() {
        let keys = ["plain", "with\"quote", "back\\slash", "uni\u{1F511}\n"];
        let entries: Vec<StorageEntry> = keys
            .iter()
            .zip(gnarly_values())
            .map(|(k, v)| StorageEntry {
                key: k.to_string(),
                value: serde_json::to_string(&v).unwrap(),
            })
            .collect();
        let decoded: Vec<Value> = keys
            .iter()
            .zip(gnarly_values())
            .map(|(k, v)| json!({ "key": k, "value": v }))
            .collect();
        assert_eq!(
            raw_paginated_response(&entries, 100, 0, 4).0,
            serde_json::to_string(
                &json!({ "data": decoded, "pagination": { "limit": 100, "offset": 0, "total": 4 } })
            )
            .unwrap()
        );
        assert_eq!(
            raw_paginated_response(&[], 10, 5, 0).0,
            r#"{"data":[],"pagination":{"limit":10,"offset":5,"total":0}}"#
        );
    }

    #[test]
    fn raw_value_splice_passes_postgres_spaced_text_verbatim() {
        let pg_text = "{\"a\": 1, \"b\": [1, 2]}";
        let body = raw_value_response(pg_text).0;
        assert!(body.contains(pg_text));
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            json!({ "value": { "a": 1, "b": [1, 2] } })
        );
    }

    #[test]
    fn absent_key_is_404() {
        let err = get_value_response(None).expect_err("absent key should be an error");
        assert!(matches!(err, ApiError::NotFound(_)));
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn usage_and_key_list_serialize_like_the_upstream_json() {
        assert_eq!(
            serde_json::to_value(UsageResponse {
                used_bytes: 5,
                max_total_size_bytes: 10
            })
            .unwrap(),
            json!({ "usedBytes": 5, "maxTotalSizeBytes": 10 })
        );
        assert_eq!(
            serde_json::to_value(KeyListResponse {
                data: vec!["a".into()],
                pagination: PaginationInfo {
                    limit: 100,
                    offset: 0,
                    total: 1
                }
            })
            .unwrap(),
            json!({ "data": ["a"], "pagination": { "limit": 100, "offset": 0, "total": 1 } })
        );
    }

    #[test]
    fn upsert_body_schema_matches_ajv() {
        assert!(serde_json::from_str::<UpsertBody>(r#"{"value": {"a": 1}}"#).is_ok());
        assert!(serde_json::from_str::<UpsertBody>(r#"{"value": null}"#).is_ok());
        assert!(serde_json::from_str::<UpsertBody>(r#"{"value": [1,2,3]}"#).is_ok());
        assert!(serde_json::from_str::<UpsertBody>(r#"{"value": 0}"#).is_ok());
        assert!(serde_json::from_str::<UpsertBody>(r#"{}"#).is_err());
        assert!(serde_json::from_str::<UpsertBody>(r#"{"value": 1, "extra": 2}"#).is_err());
    }

    #[test]
    fn upsert_env_body_requires_string_value() {
        assert!(serde_json::from_str::<UpsertEnvBody>(r#"{"value": "secret"}"#).is_ok());
        assert!(serde_json::from_str::<UpsertEnvBody>(r#"{"value": 5}"#).is_err());
        assert!(serde_json::from_str::<UpsertEnvBody>(r#"{"value": true}"#).is_err());
        assert!(serde_json::from_str::<UpsertEnvBody>(r#"{"value": null}"#).is_err());
        assert!(serde_json::from_str::<UpsertEnvBody>(r#"{}"#).is_err());
        assert!(serde_json::from_str::<UpsertEnvBody>(r#"{"value": "x", "extra": 1}"#).is_err());
    }

    fn json_put(body: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("PUT")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn validated_json_rejects_bad_body_with_400_bad_request() {
        let res =
            ValidatedJson::<UpsertBody>::from_request(json_put(r#"{"value":1,"extra":2}"#), &())
                .await;
        let err = res.err().expect("extra property should be rejected");
        assert!(matches!(err, ApiError::BadRequest(_)));
        assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);

        let res = ValidatedJson::<UpsertBody>::from_request(json_put(r#"{}"#), &()).await;
        assert!(matches!(res.err(), Some(ApiError::BadRequest(_))));

        let res =
            ValidatedJson::<UpsertEnvBody>::from_request(json_put(r#"{"value":5}"#), &()).await;
        assert!(matches!(res.err(), Some(ApiError::BadRequest(_))));
    }

    #[tokio::test]
    async fn validated_json_accepts_valid_body() {
        let ValidatedJson(body) =
            ValidatedJson::<UpsertBody>::from_request(json_put(r#"{"value":{"a":1}}"#), &())
                .await
                .expect("a valid body should extract");
        assert_eq!(body.value, json!({"a": 1}));
    }
}
