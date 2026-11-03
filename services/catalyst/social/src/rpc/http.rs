//! The HTTP twins of the RPC procedures (upstream #504-#507 at `b6b2cef`,
//! `src/controllers/routes/http.routes.ts`): friends, friendship requests,
//! blocks, social settings, private and community voice chats on the
//! social-api host (`/v1/...`, `/v2/...`, and under `/social-api`), each
//! behind the ADR-44 signed fetch and answering upstream's envelopes
//! (`{ data }`, `{ results, total, page, pages, limit }`, `{ error, message }`).
//!
//! Every write and every read runs the same operation as its procedure
//! (`service::{friends, blocks, settings, private_voice, community_voice}`
//! and the same `Db` reads), so the two transports cannot drift: the same
//! gates, the same writes and the same `pubsub` events. What is HTTP's own
//! is upstream's: the body schemas, the path checks and their messages, and
//! the refusal statuses of `friends-errors.ts` / `voice-chats-errors.ts`
//! (403, 404, 409, 429; [`Refusal`]). Responses carry addresses only, never
//! profiles, as upstream's `WithoutProfiles` variants.

use std::sync::Arc;

use crate::communities::http::DeferredBody;
use axum::extract::{OriginalUri, Path, Query, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde_json::{Map, Value, json};

use crate::communities::http::{CommError, EnvelopeData, Paginated, get_pagination_params};
use crate::communities::{auth, cors, signed_path};
use crate::rpc::State;
use crate::rpc::db::SocialSettingsRow;
use crate::rpc::service::{Action, Refusal, ops};

type Shared = Arc<State>;
type Answer = Result<Response, CommError>;

const ADDRESS_PATTERN: &str = "^0x[a-fA-F0-9]{40}$";

/// The routes, with the social-api CORS answer; `social::router` mounts them
/// at the root and under `/social-api`, behind the writes switch.
pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/v2/friends", get(get_friends))
        .route("/v1/friends/{peer}", delete(remove_friend))
        .route(
            "/v2/users/{address}/mutual-friends",
            get(get_mutual_friends),
        )
        .route(
            "/v1/users/{address}/friendship-status",
            get(get_friendship_status),
        )
        .route("/v2/friendship-requests", get(get_friendship_requests))
        .route("/v1/friendship-requests", post(create_friendship_request))
        .route(
            "/v1/friendship-requests/{address}",
            patch(update_friendship_request),
        )
        .route("/v2/blocks", get(get_blocked_users))
        .route("/v1/blocks", post(block_user))
        .route("/v1/blocks/{address}", delete(unblock_user))
        .route("/v1/blocking-status", get(get_blocking_status))
        .route(
            "/v1/social-settings",
            get(get_social_settings).patch(update_social_settings),
        )
        .route(
            "/v1/private-messages-settings",
            get(get_private_messages_settings),
        )
        .route("/v1/private-voice-chats", post(start_private_voice_chat))
        .route(
            "/v1/private-voice-chats/incoming",
            get(get_incoming_private_voice_chat),
        )
        .route(
            "/v1/private-voice-chats/{callId}",
            patch(update_private_voice_chat).delete(end_private_voice_chat),
        )
        .route(
            "/v1/community-voice-chats",
            post(start_community_voice_chat),
        )
        .route(
            "/v1/community-voice-chats/{communityId}",
            delete(end_community_voice_chat),
        )
        .route(
            "/v1/community-voice-chats/{communityId}/participants",
            post(join_community_voice_chat),
        )
        .route(
            "/v1/community-voice-chats/{communityId}/participants/{address}",
            delete(kick_participant),
        )
        .route(
            "/v1/community-voice-chats/{communityId}/speakers/{address}",
            axum::routing::put(promote_speaker)
                .patch(mute_speaker)
                .delete(demote_speaker),
        )
        .route(
            "/v1/community-voice-chats/{communityId}/speak-requests",
            post(raise_hand),
        )
        .route(
            "/v1/community-voice-chats/{communityId}/speak-requests/{address}",
            delete(remove_speak_request),
        )
        .layer(axum::middleware::from_fn(cors::social_api_cors))
        .with_state(state)
}

async fn signer(
    state: &State,
    headers: &HeaderMap,
    original: &OriginalUri,
    method: &str,
    local: String,
) -> Result<String, CommError> {
    let path = signed_path(original, local);
    Ok(
        auth::require_signer(headers, method, &path, state.eip1654.as_deref())
            .await?
            .into_string(),
    )
}

/// `friends-errors.ts` and `voice-chats-errors.ts`: `{ error: <status
/// text>, message }`; an invalid request is the http-commons `Bad request`.
fn refused(refusal: Refusal) -> CommError {
    let named = |status: StatusCode, message: String| CommError::Named(status, message);
    match refusal {
        Refusal::Invalid(m) => CommError::BadRequest(m),
        Refusal::Forbidden(m) => named(StatusCode::FORBIDDEN, m),
        Refusal::NotFound(m) => CommError::NotFound(m),
        Refusal::Conflict(m) => named(StatusCode::CONFLICT, m),
        Refusal::RateLimited(m) => named(StatusCode::TOO_MANY_REQUESTS, m),
        Refusal::Internal(detail) | Refusal::Failed(detail) => {
            tracing::error!(%detail, "social http route failed");
            CommError::Internal
        }
    }
}

fn failed(e: impl std::fmt::Display) -> CommError {
    tracing::error!(error = %e, "social http route failed");
    CommError::Internal
}

fn data(status: StatusCode, value: Value) -> Answer {
    Ok((status, Json(json!({ "data": value }))).into_response())
}

fn no_content() -> Answer {
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Upstream `normalizeAddress` then `EthAddress.validate`: lowercased, never trimmed.
fn path_address(raw: &str) -> Result<String, CommError> {
    let address = raw.to_lowercase();
    if crate::auth::is_eth_address(&address) {
        Ok(address)
    } else {
        Err(CommError::bad_request("Invalid address"))
    }
}

/// Upstream `isValidUUID`: the hyphenated form only, either case.
fn is_uuid(raw: &str) -> bool {
    raw.len() == 36
        && raw.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn path_uuid(raw: &str, name: &str) -> Result<(), CommError> {
    if is_uuid(raw) {
        Ok(())
    } else {
        Err(CommError::bad_request(format!("Invalid {name}")))
    }
}

/// One body property's schema (the AJV schemas of `handlers/http/schemas.ts`).
pub(crate) enum Kind {
    Address,
    Email,
    Uuid,
    OneOf(&'static [&'static str]),
    Text {
        max: usize,
    },
    TextRange {
        min: usize,
        max: usize,
        non_whitespace: bool,
    },
    Strings {
        min: usize,
        max: usize,
        element_max: usize,
        uuid: bool,
    },
    Bool,
}

pub(crate) fn js_whitespace(character: char) -> bool {
    character != '\u{0085}' && (character.is_whitespace() || character == '\u{FEFF}')
}

/// Upstream `UUID_PATTERN` (`src/utils/uuid.ts`).
const UUID_PATTERN: &str =
    "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$";

/// One AJV error object (`instancePath`, `schemaPath`, `keyword`, `params`, `message`).
fn ajv_error(property: Option<&str>, keyword: &str, params: Value, message: String) -> Value {
    let (instance_path, schema_path) = match property {
        Some(name) => (format!("/{name}"), format!("#/properties/{name}/{keyword}")),
        None => (String::new(), format!("#/{keyword}")),
    };
    json!({
        "instancePath": instance_path,
        "schemaPath": schema_path,
        "keyword": keyword,
        "params": params,
        "message": message,
    })
}

/// `@dcl/schema-validator-component` 1.0.1's 400: `{ ok: false, message }`
/// when the body is not JSON, `{ ok: false, message: "Invalid JSON body",
/// data: [<AJV error>] }` when it fails the schema.
fn schema_refusal(message: String, error: Option<Value>) -> CommError {
    let mut body = json!({ "ok": false, "message": message });
    if let Some(error) = error {
        body["data"] = json!([error]);
    }
    CommError::Body(StatusCode::BAD_REQUEST, body)
}

/// AJV visits object properties in JavaScript order. Keep that order when
/// selecting the first unknown field even though Value uses a sorted map.
fn input_keys(bytes: &[u8]) -> Vec<String> {
    struct Keys;
    impl<'de> serde::de::Visitor<'de> for Keys {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut keys = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                map.next_value::<serde::de::IgnoredAny>()?;
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            let index = |key: &str| {
                key.parse::<u32>()
                    .ok()
                    .filter(|n| *n < u32::MAX && n.to_string() == key)
            };
            keys.sort_by(|left, right| match (index(left), index(right)) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            });
            Ok(keys)
        }
    }
    serde::Deserializer::deserialize_map(&mut serde_json::Deserializer::from_slice(bytes), Keys)
        .unwrap_or_default()
}

/// A JSON object body checked against upstream's schema as AJV checks it
/// (one error, the first in AJV's order: the type, `minProperties`,
/// `required`, `additionalProperties` in the body's order, then each
/// property in the schema's order: its type, then its pattern, enum or
/// length). The refusal is the schema validator's 400.
pub(crate) fn body(
    bytes: &[u8],
    properties: &[(&str, Kind)],
    required: &[&str],
    min_properties: usize,
) -> Result<Map<String, Value>, CommError> {
    let invalid = |property: Option<&str>, keyword: &str, params: Value, message: String| {
        schema_refusal(
            "Invalid JSON body".into(),
            Some(ajv_error(property, keyword, params, message)),
        )
    };
    let value: Value = serde_json::from_slice(bytes).map_err(|e| {
        // V8's message for the common empty body; any other parse error
        // carries serde's wording.
        let message = if bytes.iter().all(u8::is_ascii_whitespace) {
            "Unexpected end of JSON input".to_string()
        } else {
            e.to_string()
        };
        schema_refusal(message, None)
    })?;
    let Value::Object(object) = value else {
        return Err(invalid(
            None,
            "type",
            json!({ "type": "object" }),
            "must be object".into(),
        ));
    };
    if object.len() < min_properties {
        return Err(invalid(
            None,
            "minProperties",
            json!({ "limit": min_properties }),
            format!("must NOT have fewer than {min_properties} properties"),
        ));
    }
    if let Some(name) = required.iter().find(|name| !object.contains_key(**name)) {
        return Err(invalid(
            None,
            "required",
            json!({ "missingProperty": name }),
            format!("must have required property '{name}'"),
        ));
    }
    if let Some(name) = input_keys(bytes)
        .iter()
        .find(|name| !properties.iter().any(|(p, _)| p == name))
    {
        return Err(invalid(
            None,
            "additionalProperties",
            json!({ "additionalProperty": name }),
            "must NOT have additional properties".into(),
        ));
    }
    for (name, kind) in properties {
        let Some(value) = object.get(*name) else {
            continue;
        };
        let field = Some(*name);
        let wrong_type = |expected: &str| {
            invalid(
                field,
                "type",
                json!({ "type": expected }),
                format!("must be {expected}"),
            )
        };
        if let Kind::Strings {
            min,
            max,
            element_max,
            uuid,
        } = kind
        {
            let Value::Array(items) = value else {
                return Err(wrong_type("array"));
            };
            for (keyword, limit, fails, message) in [
                (
                    "maxItems",
                    *max,
                    items.len() > *max,
                    format!("must NOT have more than {max} items"),
                ),
                (
                    "minItems",
                    *min,
                    items.len() < *min,
                    format!("must NOT have fewer than {min} items"),
                ),
            ] {
                if fails {
                    return Err(invalid(field, keyword, json!({ "limit": limit }), message));
                }
            }
            for (index, item) in items.iter().enumerate() {
                let violation = match item.as_str() {
                    None => Some((
                        "type",
                        json!({ "type": "string" }),
                        "must be string".to_string(),
                    )),
                    Some(text) if text.chars().count() > *element_max => Some((
                        "maxLength",
                        json!({ "limit": element_max }),
                        format!("must NOT have more than {element_max} characters"),
                    )),
                    Some(text) if *uuid && !is_uuid(text) => Some((
                        "pattern",
                        json!({ "pattern": UUID_PATTERN }),
                        format!("must match pattern \"{UUID_PATTERN}\""),
                    )),
                    _ => None,
                };
                if let Some((keyword, params, message)) = violation {
                    let mut error = ajv_error(field, keyword, params, message);
                    error["instancePath"] = json!(format!("/{name}/{index}"));
                    error["schemaPath"] = json!(format!("#/properties/{name}/items/{keyword}"));
                    return Err(schema_refusal("Invalid JSON body".into(), Some(error)));
                }
            }
            continue;
        }
        let text = match (kind, value) {
            (Kind::Bool, Value::Bool(_)) => continue,
            (Kind::Bool, _) => return Err(wrong_type("boolean")),
            (_, Value::String(text)) => text,
            _ => return Err(wrong_type("string")),
        };
        let pattern = |pattern: &str| {
            invalid(
                field,
                "pattern",
                json!({ "pattern": pattern }),
                format!("must match pattern \"{pattern}\""),
            )
        };
        match kind {
            Kind::Email => {
                if text.is_empty() {
                    return Err(invalid(
                        field,
                        "minLength",
                        json!({"limit":1}),
                        "must NOT have fewer than 1 characters".into(),
                    ));
                }
                if !schema_email(text) {
                    return Err(invalid(
                        field,
                        "format",
                        json!({"format":"email"}),
                        "must match format \"email\"".into(),
                    ));
                }
            }
            Kind::Address if !crate::auth::is_eth_address(text) => {
                return Err(pattern(ADDRESS_PATTERN));
            }
            Kind::Uuid if !is_uuid(text) => return Err(pattern(UUID_PATTERN)),
            Kind::OneOf(allowed) if !allowed.contains(&text.as_str()) => {
                return Err(invalid(
                    field,
                    "enum",
                    json!({ "allowedValues": allowed }),
                    "must be equal to one of the allowed values".into(),
                ));
            }
            // AJV counts code points (`ucs2length`).
            Kind::Text { max } if text.chars().count() > *max => {
                return Err(invalid(
                    field,
                    "maxLength",
                    json!({ "limit": max }),
                    format!("must NOT have more than {max} characters"),
                ));
            }
            Kind::TextRange {
                min,
                max,
                non_whitespace,
            } => {
                if text.chars().count() > *max {
                    return Err(invalid(
                        field,
                        "maxLength",
                        json!({ "limit": max }),
                        format!("must NOT have more than {max} characters"),
                    ));
                }
                if text.chars().count() < *min {
                    return Err(invalid(
                        field,
                        "minLength",
                        json!({ "limit": min }),
                        format!("must NOT have fewer than {min} characters"),
                    ));
                }
                if *non_whitespace && text.starts_with(js_whitespace) {
                    return Err(pattern("^\\S"));
                }
            }
            _ => {}
        }
    }
    Ok(object)
}

// ajv-formats default/full email regexp used by the source schema middleware.
fn schema_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    let atom = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+/=?^_`{|}~-".contains(&c))
    };
    let label = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            && part.as_bytes()[0].is_ascii_alphanumeric()
            && part.as_bytes()[part.len() - 1].is_ascii_alphanumeric()
    };
    local.split('.').all(atom) && domain.contains('.') && domain.split('.').all(label)
}

pub(crate) fn text<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    object.get(name).and_then(Value::as_str)
}

// ---- friends -------------------------------------------------------------

async fn get_friends(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Answer {
    let me = signer(&state, &headers, &original, "get", "/v2/friends".into()).await?;
    let page = get_pagination_params(&pairs);
    let (friends, total) = state
        .ctx
        .db()
        .get_friends(&me, page.limit, page.offset)
        .await
        .map_err(failed)?;
    let results = friends
        .into_iter()
        .map(|address| json!({ "address": address }))
        .collect();
    Ok(Json(EnvelopeData {
        data: Paginated::new(results, total, &page),
    })
    .into_response())
}

async fn get_mutual_friends(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Answer {
    let local = format!("/v2/users/{address}/mutual-friends");
    let me = signer(&state, &headers, &original, "get", local).await?;
    let other = path_address(&address)?;
    let page = get_pagination_params(&pairs);
    let (friends, total) = state
        .ctx
        .db()
        .get_mutual_friends(&me, &other, page.limit, page.offset)
        .await
        .map_err(failed)?;
    let results = friends
        .into_iter()
        .map(|address| json!({ "address": address }))
        .collect();
    Ok(Json(EnvelopeData {
        data: Paginated::new(results, total, &page),
    })
    .into_response())
}

async fn get_friendship_status(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
) -> Answer {
    let local = format!("/v1/users/{address}/friendship-status");
    let me = signer(&state, &headers, &original, "get", local).await?;
    let other = path_address(&address)?;
    let status = ops::friendship_status(&state.ctx, &me, &other)
        .await
        .map_err(failed)?;
    data(StatusCode::OK, json!({ "status": status as i32 }))
}

async fn get_friendship_requests(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Answer {
    let local = "/v2/friendship-requests".to_string();
    let me = signer(&state, &headers, &original, "get", local).await?;
    let incoming = match crate::communities::http::get_first(&pairs, "direction").as_deref() {
        Some("received") => true,
        Some("sent") => false,
        _ => {
            return Err(CommError::bad_request(
                "direction must be one of: received, sent",
            ));
        }
    };
    let page = get_pagination_params(&pairs);
    let (rows, total) = state
        .ctx
        .db()
        .get_friendship_requests(&me, incoming, page.limit, page.offset)
        .await
        .map_err(failed)?;
    let results = rows
        .into_iter()
        .map(|r| {
            friendship_request(
                &r.id.to_string(),
                &r.address,
                r.timestamp.timestamp_millis(),
                r.message.as_deref(),
            )
        })
        .collect();
    Ok(Json(EnvelopeData {
        data: Paginated::new(results, total, &page),
    })
    .into_response())
}

/// Upstream `parseFriendshipRequestWithoutProfile`: `message` only when non-empty.
fn friendship_request(id: &str, address: &str, created_at: i64, message: Option<&str>) -> Value {
    let mut request = json!({ "id": id, "address": address, "createdAt": created_at });
    if let Some(message) = message.filter(|m| !m.is_empty()) {
        request["message"] = json!(message);
    }
    request
}

async fn create_friendship_request(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    bytes: DeferredBody,
) -> Answer {
    let local = "/v1/friendship-requests".to_string();
    let me = signer(&state, &headers, &original, "post", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(
        &bytes,
        &[
            ("address", Kind::Address),
            (
                "message",
                Kind::Text {
                    max: ops::FRIENDSHIP_REQUEST_MESSAGE_MAX_LENGTH,
                },
            ),
        ],
        &["address"],
        0,
    )?;
    let other = text(&object, "address").unwrap_or_default().to_lowercase();
    if other == me {
        return Err(CommError::bad_request(
            "You cannot send a friendship request to yourself",
        ));
    }
    let message = text(&object, "message").filter(|m| !m.is_empty());
    let (id, created_at) =
        ops::apply_friendship_action(&state.ctx, &me, &other, Action::Request, message)
            .await
            .map_err(refused)?;
    data(
        StatusCode::CREATED,
        friendship_request(&id.to_string(), &other, created_at, message),
    )
}

async fn update_friendship_request(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
    bytes: DeferredBody,
) -> Answer {
    let local = format!("/v1/friendship-requests/{address}");
    let me = signer(&state, &headers, &original, "patch", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(
        &bytes,
        &[(
            "intention",
            Kind::OneOf(&["accepted", "rejected", "cancelled"]),
        )],
        &["intention"],
        0,
    )?;
    let other = path_address(&address)?;
    if other == me {
        return Err(CommError::bad_request(
            "There is no friendship request with yourself",
        ));
    }
    let action = match text(&object, "intention") {
        Some("accepted") => Action::Accept,
        Some("rejected") => Action::Reject,
        _ => Action::Cancel,
    };
    let (id, created_at) = ops::apply_friendship_action(&state.ctx, &me, &other, action, None)
        .await
        .map_err(refused)?;
    data(
        StatusCode::OK,
        friendship_request(&id.to_string(), &other, created_at, None),
    )
}

async fn remove_friend(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(peer): Path<String>,
) -> Answer {
    let local = format!("/v1/friends/{peer}");
    let me = signer(&state, &headers, &original, "delete", local).await?;
    let other = path_address(&peer)?;
    if other == me {
        return Err(CommError::bad_request(
            "You cannot remove yourself as a friend",
        ));
    }
    ops::apply_friendship_action(&state.ctx, &me, &other, Action::Delete, None)
        .await
        .map_err(refused)?;
    no_content()
}

// ---- blocks --------------------------------------------------------------

async fn get_blocked_users(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Answer {
    let me = signer(&state, &headers, &original, "get", "/v2/blocks".into()).await?;
    let page = get_pagination_params(&pairs);
    let (rows, total) = state
        .ctx
        .db()
        .get_blocked_users(&me, page.limit, page.offset)
        .await
        .map_err(failed)?;
    let results = rows
        .into_iter()
        .map(|r| json!({ "address": r.address, "blockedAt": r.blocked_at.timestamp_millis() }))
        .collect();
    Ok(Json(EnvelopeData {
        data: Paginated::new(results, total, &page),
    })
    .into_response())
}

async fn block_user(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    bytes: DeferredBody,
) -> Answer {
    let me = signer(&state, &headers, &original, "post", "/v1/blocks".into()).await?;
    let bytes = bytes.schema().await?;
    let object = body(&bytes, &[("address", Kind::Address)], &["address"], 0)?;
    let other = text(&object, "address").unwrap_or_default().to_lowercase();
    if other == me {
        return Err(CommError::bad_request("You cannot block yourself"));
    }
    let blocked_at = ops::block(&state.ctx, &me, &other).await.map_err(refused)?;
    data(
        StatusCode::CREATED,
        json!({ "address": other, "blockedAt": blocked_at.timestamp_millis() }),
    )
}

async fn unblock_user(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
) -> Answer {
    let local = format!("/v1/blocks/{address}");
    let me = signer(&state, &headers, &original, "delete", local).await?;
    let other = path_address(&address)?;
    if other == me {
        return Err(CommError::bad_request("You cannot unblock yourself"));
    }
    ops::unblock(&state.ctx, &me, &other)
        .await
        .map_err(refused)?;
    no_content()
}

async fn get_blocking_status(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
) -> Answer {
    let local = "/v1/blocking-status".to_string();
    let me = signer(&state, &headers, &original, "get", local).await?;
    let (blocked_users, blocked_by_users) = state
        .ctx
        .db()
        .get_blocking_status(&me)
        .await
        .map_err(failed)?;
    data(
        StatusCode::OK,
        json!({ "blockedUsers": blocked_users, "blockedByUsers": blocked_by_users }),
    )
}

// ---- settings ------------------------------------------------------------

const PRIVATE_MESSAGES_PRIVACY: &[&str] = &["all", "only_friends"];
const BLOCKED_USERS_MESSAGES_VISIBILITY: &[&str] = &["show_messages", "do_not_show_messages"];
const SITUATION_REACTIONS: &[&str] = &["show", "hide"];

/// Upstream `convertDBSettingsToHTTPSettings`: the stored values as they are.
fn settings(row: &SocialSettingsRow) -> Value {
    json!({
        "privateMessagesPrivacy": row.private_messages_privacy,
        "blockedUsersMessagesVisibility": row.blocked_users_messages_visibility,
        "showSituationReactions": row.show_situation_reactions,
    })
}

async fn get_social_settings(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
) -> Answer {
    let local = "/v1/social-settings".to_string();
    let me = signer(&state, &headers, &original, "get", local).await?;
    let row = state
        .ctx
        .db()
        .get_social_settings(&me)
        .await
        .map_err(failed)?
        .unwrap_or_default();
    data(StatusCode::OK, settings(&row))
}

async fn update_social_settings(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    bytes: DeferredBody,
) -> Answer {
    let local = "/v1/social-settings".to_string();
    let me = signer(&state, &headers, &original, "patch", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(
        &bytes,
        &[
            (
                "privateMessagesPrivacy",
                Kind::OneOf(PRIVATE_MESSAGES_PRIVACY),
            ),
            (
                "blockedUsersMessagesVisibility",
                Kind::OneOf(BLOCKED_USERS_MESSAGES_VISIBILITY),
            ),
            ("showSituationReactions", Kind::OneOf(SITUATION_REACTIONS)),
        ],
        &[],
        1,
    )?;
    let row = state
        .ctx
        .db()
        .upsert_social_settings(
            &me,
            text(&object, "privateMessagesPrivacy"),
            text(&object, "blockedUsersMessagesVisibility"),
            text(&object, "showSituationReactions"),
        )
        .await
        .map_err(failed)?;
    data(StatusCode::OK, settings(&row))
}

async fn get_private_messages_settings(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Answer {
    let local = "/v1/private-messages-settings".to_string();
    let me = signer(&state, &headers, &original, "get", local).await?;
    let addresses: Vec<String> = crate::communities::http::get_all(&pairs, "addresses")
        .iter()
        .map(|a| a.to_lowercase())
        .collect();
    if addresses.len() > ops::MAX_USER_ADDRESSES {
        return Err(CommError::bad_request(format!(
            "At most {} addresses can be requested",
            ops::MAX_USER_ADDRESSES
        )));
    }
    if let Some(invalid) = addresses.iter().find(|a| !crate::auth::is_eth_address(a)) {
        return Err(CommError::bad_request(format!(
            "Invalid address: {invalid}"
        )));
    }
    let rows = ops::private_messages_settings(&state.ctx, &me, addresses)
        .await
        .map_err(failed)?;
    let settings: Vec<Value> = rows
        .into_iter()
        .map(|(address, privacy, is_friend)| {
            json!({ "address": address, "privateMessagesPrivacy": privacy, "isFriend": is_friend })
        })
        .collect();
    data(StatusCode::OK, json!(settings))
}

// ---- private voice chats -------------------------------------------------

async fn start_private_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    bytes: DeferredBody,
) -> Answer {
    let local = "/v1/private-voice-chats".to_string();
    let me = signer(&state, &headers, &original, "post", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(
        &bytes,
        &[("calleeAddress", Kind::Address)],
        &["calleeAddress"],
        0,
    )?;
    let callee = text(&object, "calleeAddress")
        .unwrap_or_default()
        .to_lowercase();
    if callee == me {
        return Err(CommError::bad_request("You cannot call yourself"));
    }
    let id = ops::start_private_voice_chat(&state.ctx, &me, &callee)
        .await
        .map_err(refused)?;
    data(StatusCode::CREATED, json!({ "callId": id.to_string() }))
}

async fn get_incoming_private_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
) -> Answer {
    let local = "/v1/private-voice-chats/incoming".to_string();
    let me = signer(&state, &headers, &original, "get", local).await?;
    let chat = state
        .ctx
        .db()
        .incoming_private_voice_chat(&me)
        .await
        .map_err(failed)?
        .ok_or_else(|| CommError::not_found(ops::incoming_not_found(&me)))?;
    data(
        StatusCode::OK,
        json!({ "callId": chat.id.to_string(), "callerAddress": chat.caller_address }),
    )
}

async fn update_private_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(call_id): Path<String>,
    bytes: DeferredBody,
) -> Answer {
    let local = format!("/v1/private-voice-chats/{call_id}");
    let me = signer(&state, &headers, &original, "patch", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(
        &bytes,
        &[("intention", Kind::OneOf(&["accepted", "rejected"]))],
        &["intention"],
        0,
    )?;
    path_uuid(&call_id, "callId")?;
    let id = uuid::Uuid::parse_str(&call_id).map_err(failed)?;
    if text(&object, "intention") == Some("rejected") {
        ops::reject_private_voice_chat(&state.ctx, &me, id)
            .await
            .map_err(refused)?;
        return no_content();
    }
    let connection_url = ops::accept_private_voice_chat(&state.ctx, &me, id)
        .await
        .map_err(refused)?;
    data(
        StatusCode::OK,
        json!({ "callId": call_id, "connectionUrl": connection_url }),
    )
}

async fn end_private_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(call_id): Path<String>,
) -> Answer {
    let local = format!("/v1/private-voice-chats/{call_id}");
    let me = signer(&state, &headers, &original, "delete", local).await?;
    path_uuid(&call_id, "callId")?;
    let id = uuid::Uuid::parse_str(&call_id).map_err(failed)?;
    ops::end_private_voice_chat(&state.ctx, &me, id)
        .await
        .map_err(refused)?;
    no_content()
}

// ---- community voice chats -----------------------------------------------

async fn start_community_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    bytes: DeferredBody,
) -> Answer {
    let local = "/v1/community-voice-chats".to_string();
    let me = signer(&state, &headers, &original, "post", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(&bytes, &[("communityId", Kind::Uuid)], &["communityId"], 0)?;
    let community_id = text(&object, "communityId").unwrap_or_default();
    let connection_url = ops::start_community_voice_chat(&state.ctx, &me, community_id, false)
        .await
        .map_err(refused)?;
    data(
        StatusCode::CREATED,
        json!({ "connectionUrl": connection_url }),
    )
}

async fn end_community_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(community_id): Path<String>,
) -> Answer {
    let local = format!("/v1/community-voice-chats/{community_id}");
    let me = signer(&state, &headers, &original, "delete", local).await?;
    path_uuid(&community_id, "communityId")?;
    ops::end_community_voice_chat(&state.ctx, &me, &community_id)
        .await
        .map_err(refused)?;
    no_content()
}

async fn join_community_voice_chat(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(community_id): Path<String>,
) -> Answer {
    let local = format!("/v1/community-voice-chats/{community_id}/participants");
    let me = signer(&state, &headers, &original, "post", local).await?;
    path_uuid(&community_id, "communityId")?;
    let connection_url = ops::join_community_voice_chat(&state.ctx, &me, &community_id, false)
        .await
        .map_err(refused)?;
    data(
        StatusCode::CREATED,
        json!({ "connectionUrl": connection_url }),
    )
}

/// The caller, the community id and the target address of a
/// `/v1/community-voice-chats/{communityId}/<collection>/{address}` route.
async fn community_target(
    state: &State,
    headers: &HeaderMap,
    original: &OriginalUri,
    method: &str,
    (community_id, address): &(String, String),
    collection: &str,
) -> Result<(String, String), CommError> {
    let local = format!("/v1/community-voice-chats/{community_id}/{collection}/{address}");
    let me = signer(state, headers, original, method, local).await?;
    path_uuid(community_id, "communityId")?;
    Ok((me, path_address(address)?))
}

async fn kick_participant(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(path): Path<(String, String)>,
) -> Answer {
    let (me, target) =
        community_target(&state, &headers, &original, "delete", &path, "participants").await?;
    ops::kick_player(&state.ctx, &me, &path.0, &target)
        .await
        .map_err(refused)?;
    no_content()
}

async fn promote_speaker(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(path): Path<(String, String)>,
) -> Answer {
    let (me, target) =
        community_target(&state, &headers, &original, "put", &path, "speakers").await?;
    ops::set_speaker(&state.ctx, &me, &path.0, &target, true)
        .await
        .map_err(refused)?;
    no_content()
}

async fn demote_speaker(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(path): Path<(String, String)>,
) -> Answer {
    let (me, target) =
        community_target(&state, &headers, &original, "delete", &path, "speakers").await?;
    ops::set_speaker(&state.ctx, &me, &path.0, &target, false)
        .await
        .map_err(refused)?;
    no_content()
}

async fn mute_speaker(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(path): Path<(String, String)>,
    bytes: DeferredBody,
) -> Answer {
    let local = format!("/v1/community-voice-chats/{}/speakers/{}", path.0, path.1);
    let me = signer(&state, &headers, &original, "patch", local).await?;
    let bytes = bytes.schema().await?;
    let object = body(&bytes, &[("muted", Kind::Bool)], &["muted"], 0)?;
    path_uuid(&path.0, "communityId")?;
    let target = path_address(&path.1)?;
    let muted = object
        .get("muted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    ops::mute_speaker(&state.ctx, &me, &path.0, &target, muted)
        .await
        .map_err(refused)?;
    no_content()
}

async fn raise_hand(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(community_id): Path<String>,
) -> Answer {
    let local = format!("/v1/community-voice-chats/{community_id}/speak-requests");
    let me = signer(&state, &headers, &original, "post", local).await?;
    path_uuid(&community_id, "communityId")?;
    ops::request_to_speak(&state.ctx, &me, &community_id, true)
        .await
        .map_err(refused)?;
    no_content()
}

/// Lowering one's own hand, or a moderator rejecting another's request.
async fn remove_speak_request(
    AxumState(state): AxumState<Shared>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(path): Path<(String, String)>,
) -> Answer {
    let (me, target) = community_target(
        &state,
        &headers,
        &original,
        "delete",
        &path,
        "speak-requests",
    )
    .await?;
    if target == me {
        ops::request_to_speak(&state.ctx, &me, &path.0, false).await
    } else {
        ops::reject_speak_request(&state.ctx, &me, &path.0, &target).await
    }
    .map_err(refused)?;
    no_content()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_upstreams_hyphenated_form() {
        assert!(is_uuid("6f1c2a52-0f3e-4a8e-9d55-1b2c3d4e5f60"));
        assert!(is_uuid("6F1C2A52-0F3E-4A8E-9D55-1B2C3D4E5F60"));
        assert!(!is_uuid("6f1c2a520f3e4a8e9d551b2c3d4e5f60"));
        assert!(!is_uuid("{6f1c2a52-0f3e-4a8e-9d55-1b2c3d4e5f60}"));
        assert!(!is_uuid("6f1c2a52-0f3e-4a8e-9d55-1b2c3d4e5f6g"));
    }

    #[test]
    fn bodies_follow_the_upstream_schemas() {
        let block = |raw: &str| {
            body(
                raw.as_bytes(),
                &[("address", Kind::Address)],
                &["address"],
                0,
            )
        };
        assert!(block(r#"{"address":"0x0000000000000000000000000000000000000001"}"#).is_ok());
        for refused in [
            "{}",
            "[]",
            "not json",
            r#"{"address":"0x1"}"#,
            r#"{"address":7}"#,
            r#"{"address":"0x0000000000000000000000000000000000000001","extra":1}"#,
        ] {
            assert!(block(refused).is_err(), "{refused}");
        }
        let settings = |raw: &str| {
            body(
                raw.as_bytes(),
                &[("showSituationReactions", Kind::OneOf(SITUATION_REACTIONS))],
                &[],
                1,
            )
        };
        assert!(settings(r#"{"showSituationReactions":"hide"}"#).is_ok());
        assert!(settings("{}").is_err(), "minProperties 1");
        assert!(settings(r#"{"showSituationReactions":"HIDE"}"#).is_err());
        let message = "m".repeat(257);
        let request = body(
            format!(r#"{{"address":"0x0000000000000000000000000000000000000001","message":"{message}"}}"#)
                .as_bytes(),
            &[("address", Kind::Address), ("message", Kind::Text { max: 256 })],
            &["address"],
            0,
        );
        assert!(request.is_err());
    }

    fn refusal(result: Result<Map<String, Value>, CommError>) -> Value {
        match result {
            Err(CommError::Body(status, body)) => {
                assert_eq!(status, StatusCode::BAD_REQUEST);
                body
            }
            other => panic!("not a schema refusal: {other:?}"),
        }
    }

    /// `@dcl/schema-validator-component` 1.0.1 (`dist/src/component.js`):
    /// `{ ok: false, message: "Invalid JSON body", data: [<AJV error>] }`,
    /// the first error in AJV's order, and the parse error's message alone
    /// for a body that is not JSON.
    #[test]
    fn a_body_failing_the_schema_answers_as_the_schema_validator() {
        let request = |raw: &str| {
            body(
                raw.as_bytes(),
                &[
                    ("address", Kind::Address),
                    ("message", Kind::Text { max: 3 }),
                ],
                &["address"],
                0,
            )
        };
        assert_eq!(
            refusal(request("")),
            json!({ "ok": false, "message": "Unexpected end of JSON input" })
        );
        let cases = [
            (
                "[]",
                json!({ "instancePath": "", "schemaPath": "#/type", "keyword": "type",
                    "params": { "type": "object" }, "message": "must be object" }),
            ),
            (
                r#"{"extra":1}"#,
                json!({ "instancePath": "", "schemaPath": "#/required", "keyword": "required",
                    "params": { "missingProperty": "address" },
                    "message": "must have required property 'address'" }),
            ),
            (
                r#"{"address":7,"extra":1}"#,
                json!({ "instancePath": "", "schemaPath": "#/additionalProperties",
                    "keyword": "additionalProperties", "params": { "additionalProperty": "extra" },
                    "message": "must NOT have additional properties" }),
            ),
            (
                r#"{"message":"long","address":"0x1"}"#,
                json!({ "instancePath": "/address", "schemaPath": "#/properties/address/pattern",
                    "keyword": "pattern", "params": { "pattern": ADDRESS_PATTERN },
                    "message": format!("must match pattern \"{ADDRESS_PATTERN}\"") }),
            ),
            (
                r#"{"address":"0x0000000000000000000000000000000000000001","message":"long"}"#,
                json!({ "instancePath": "/message", "schemaPath": "#/properties/message/maxLength",
                    "keyword": "maxLength", "params": { "limit": 3 },
                    "message": "must NOT have more than 3 characters" }),
            ),
        ];
        for (raw, error) in cases {
            assert_eq!(
                refusal(request(raw)),
                json!({ "ok": false, "message": "Invalid JSON body", "data": [error] }),
                "{raw}"
            );
        }
        let intention = body(
            br#"{"intention":"maybe"}"#,
            &[("intention", Kind::OneOf(&["accepted", "rejected"]))],
            &["intention"],
            0,
        );
        assert_eq!(
            refusal(intention)["data"][0]["params"],
            json!({ "allowedValues": ["accepted", "rejected"] })
        );
        let settings = body(b"{}", &[("x", Kind::Bool)], &[], 1);
        assert_eq!(
            refusal(settings)["data"][0]["message"],
            "must NOT have fewer than 1 properties"
        );
        let community = body(
            br#"{"communityId":"nope"}"#,
            &[("communityId", Kind::Uuid)],
            &["communityId"],
            0,
        );
        assert_eq!(
            refusal(community)["data"][0]["params"]["pattern"],
            UUID_PATTERN
        );
    }

    /// The served `/social-api/openapi.json` documents every twin route
    /// (upstream `docs/openapi.yaml` at `b6b2cef`).
    #[test]
    fn the_served_openapi_documents_every_route() {
        let doc: Value = serde_json::from_str(crate::shared::openapi::COMMUNITIES).unwrap();
        for (path, method) in [
            ("/v2/friends", "get"),
            ("/v1/friends/{address}", "delete"),
            ("/v2/users/{address}/mutual-friends", "get"),
            ("/v1/users/{address}/friendship-status", "get"),
            ("/v2/friendship-requests", "get"),
            ("/v1/friendship-requests", "post"),
            ("/v1/friendship-requests/{address}", "patch"),
            ("/v2/blocks", "get"),
            ("/v1/blocks", "post"),
            ("/v1/blocks/{address}", "delete"),
            ("/v1/blocking-status", "get"),
            ("/v1/social-settings", "get"),
            ("/v1/social-settings", "patch"),
            ("/v1/private-messages-settings", "get"),
            ("/v1/private-voice-chats", "post"),
            ("/v1/private-voice-chats/incoming", "get"),
            ("/v1/private-voice-chats/{callId}", "patch"),
            ("/v1/private-voice-chats/{callId}", "delete"),
            ("/v1/community-voice-chats", "post"),
            ("/v1/community-voice-chats/{communityId}", "delete"),
            (
                "/v1/community-voice-chats/{communityId}/participants",
                "post",
            ),
            (
                "/v1/community-voice-chats/{communityId}/participants/{address}",
                "delete",
            ),
            (
                "/v1/community-voice-chats/{communityId}/speakers/{address}",
                "put",
            ),
            (
                "/v1/community-voice-chats/{communityId}/speakers/{address}",
                "patch",
            ),
            (
                "/v1/community-voice-chats/{communityId}/speakers/{address}",
                "delete",
            ),
            (
                "/v1/community-voice-chats/{communityId}/speak-requests",
                "post",
            ),
            (
                "/v1/community-voice-chats/{communityId}/speak-requests/{address}",
                "delete",
            ),
        ] {
            assert!(doc["paths"][path][method].is_object(), "{method} {path}");
        }
    }
}
