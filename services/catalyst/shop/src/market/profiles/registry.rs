//! Registry DTOs over the same local entity lookup and profile processing as
//! Catalyst Lambdas. There is no Registry upstream or separate profile cache.
//!
//! Both answers keep the Registry's key order: `{timestamp, avatars}` with a
//! numeric `timestamp` for `/profiles` (`captures/abr/AB-2-one-known.json`),
//! `{pointer, hasClaimedName, name, nameColor, thumbnailUrl}` for
//! `/profiles/metadata` (`captures/abr/AB-1-one-known.json`).

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::value::RawValue;
use serde_json::{Value, json};

use super::{
    MarketplaceState, PROFILE_IDS_MAX, SharedState, active_entities, process_profiles_positional,
    profile_metadata,
};

/// One `/profiles` element in the Registry's key order.
#[derive(serde::Serialize)]
struct FullProfile<'a> {
    timestamp: Value,
    avatars: &'a Value,
}

const INVALID_IDS: &str = "The ids property must be an array of non-empty strings";
/// Largest request body: 1000 ids of a few dozen bytes each fit well within it.
const BODY_LIMIT: usize = 256 * 1024;

/// Mount under `/asset-bundle-registry`, using the existing market state,
/// with the Registry's CORS like the gateway half of that prefix.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/profiles", post(profiles))
        .route("/profiles/metadata", post(metadata))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(axum::middleware::from_fn(
            crate::explore::api::registry::cors,
        ))
        .with_state(state)
}

fn parse_ids(body: &[u8]) -> Result<Vec<String>, Box<Response>> {
    // The pinned Registry captures define these error envelopes. In contrast,
    // Lambdas rejects an empty ids array; Registry accepts it as an empty read.
    let body: Value = serde_json::from_slice(body).map_err(|_| Box::new(internal_error()))?;
    let ids = body
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(invalid_ids)?;
    // Each id costs an ownership pass and nothing is cached: the Lambdas cap.
    if ids.len() > PROFILE_IDS_MAX {
        return Err(Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad request",
                    "message": format!(
                        "Too many profile ids were specified. Maximum allowed is {PROFILE_IDS_MAX}"
                    ),
                })),
            )
                .into_response(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut pointers = Vec::with_capacity(ids.len());
    for id in ids {
        let id = id
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid_ids)?;
        let id = id.to_lowercase();
        if seen.insert(id.clone()) {
            pointers.push(id);
        }
    }
    Ok(pointers)
}

/// The Registry's unhandled-error envelope (the captured malformed-JSON answer).
fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error":"Internal Server Error"})),
    )
        .into_response()
}

fn invalid_ids() -> Box<Response> {
    Box::new(
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Bad request", "message": INVALID_IDS})),
        )
            .into_response(),
    )
}

async fn profiles(
    State(state): State<SharedState>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(state, body, false).await
}

async fn metadata(
    State(state): State<SharedState>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(state, body, true).await
}

/// A body the extractor refused (over `BODY_LIMIT`, or a broken stream) in
/// the same JSON envelope as the other errors, not axum's plain text.
fn body_rejection(rejection: BytesRejection) -> Response {
    let status = rejection.status();
    let message = if status == StatusCode::PAYLOAD_TOO_LARGE {
        format!("The request body exceeds {BODY_LIMIT} bytes")
    } else {
        rejection.body_text()
    };
    let error = status.canonical_reason().unwrap_or("Bad request");
    (status, Json(json!({"error": error, "message": message}))).into_response()
}

async fn respond(
    state: SharedState,
    body: Result<Bytes, BytesRejection>,
    compact: bool,
) -> Response {
    let body = match body {
        Ok(body) => body,
        Err(rejection) => return body_rejection(rejection),
    };
    let pointers = match parse_ids(&body) {
        Ok(pointers) => pointers,
        Err(response) => return *response,
    };
    if pointers.is_empty() {
        return Json(json!([])).into_response();
    }
    let entities = match active_entities(&state, &pointers).await {
        Ok(entities) => entities,
        Err(()) => {
            tracing::warn!("Registry profile lookup failed");
            return internal_error();
        }
    };
    // Content pointers may resolve scenes or incomplete profile documents.
    // Registry's profile store only admits profiles with usable avatars.
    let mut entities: Vec<_> = entities
        .into_iter()
        .filter(|entity| {
            entity.get("type").and_then(Value::as_str) == Some("profile")
                && entity
                    .get("metadata")
                    .and_then(|metadata| metadata.get("avatars"))
                    .and_then(Value::as_array)
                    .and_then(|avatars| avatars.first())
                    .is_some_and(Value::is_object)
        })
        .collect();
    sort_by_request(&mut entities, &pointers);
    match project_profiles(&state, &entities, compact).await {
        Ok(profiles) => Json(profiles).into_response(),
        // Never unvalidated metadata, and never an empty answer that reads
        // as "no profiles": a failed ownership pass is an error.
        Err(()) => internal_error(),
    }
}

/// Request order: an entity sits at its first requested (lowercased) pointer.
fn sort_by_request(entities: &mut [Value], pointers: &[String]) {
    let position: std::collections::HashMap<&str, usize> = pointers
        .iter()
        .enumerate()
        .map(|(i, p)| (p.as_str(), i))
        .collect();
    entities.sort_by_cached_key(|entity| {
        entity
            .get("pointers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(|p| position.get(p.to_lowercase().as_str()).copied())
            .min()
            .unwrap_or(usize::MAX)
    });
}

/// The processed profiles in `entities` order, each serialized in the
/// Registry's key order. A malformed entity (no id, pointer, avatar or
/// timestamp) is left out with a warning; only a failed ownership pass fails
/// the batch.
async fn project_profiles(
    state: &MarketplaceState,
    entities: &[Value],
    compact: bool,
) -> Result<Vec<Box<RawValue>>, ()> {
    let processed = process_profiles_positional(state, entities)
        .await
        .map_err(|error| tracing::warn!(%error, "Registry ownership reader failed"))?;
    Ok(entities
        .iter()
        .zip(processed)
        .filter_map(|(entity, profile)| profile.map(|profile| (entity, profile)))
        .filter_map(|(entity, profile)| {
            let projected = project(state, entity, profile, compact);
            if projected.is_none() {
                let id = entity.get("id").and_then(Value::as_str).unwrap_or("");
                tracing::warn!(
                    entity = id,
                    "Registry profile entity is malformed; left out"
                );
            }
            projected
        })
        .collect())
}

fn project(
    state: &MarketplaceState,
    entity: &Value,
    mut profile: Value,
    compact: bool,
) -> Option<Box<RawValue>> {
    let id = entity.get("id").and_then(Value::as_str)?;
    let images = state
        .settings
        .profile_images_public_url
        .trim_end_matches('/');
    // The Registry's sanitizer replaces every avatar's `snapshots` with
    // exactly `face256` and `body` on this node's profile-images routes,
    // dropping any other deployed key (Lambdas keeps those).
    let face = format!("{images}/entities/{id}/face.png");
    let snapshots = json!({
        "face256": face,
        "body": format!("{images}/entities/{id}/body.png"),
    });
    for avatar in profile
        .get_mut("avatars")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(avatar) = avatar.get_mut("avatar").and_then(Value::as_object_mut) {
            avatar.insert("snapshots".into(), snapshots.clone());
        }
    }
    let projected = if compact {
        let pointer = entity
            .get("pointers")
            .and_then(Value::as_array)
            .and_then(|pointers| pointers.first())
            .and_then(Value::as_str)?;
        serde_json::value::to_raw_value(&profile_metadata(pointer, &profile, Some(&face))?)
    } else {
        // A number, as the Registry answers from its cache and its Catalyst
        // fallback (`captures/abr/AB-2-one-known.json`).
        let timestamp = match entity.get("timestamp") {
            Some(Value::Number(value)) => Value::Number(value.clone()),
            Some(Value::String(value)) => Value::from(value.parse::<u64>().ok()?),
            _ => return None,
        };
        serde_json::value::to_raw_value(&FullProfile {
            timestamp,
            avatars: &profile["avatars"],
        })
    };
    projected.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::market::tests::support::{state_with_empty_readers, state_without_db};

    fn values(raw: Vec<Box<RawValue>>) -> Vec<Value> {
        raw.iter()
            .map(|raw| serde_json::from_str(raw.get()).unwrap())
            .collect()
    }

    async fn call(path: &str, body: &str) -> (StatusCode, Value) {
        let response = router(state_without_db())
            .oneshot(
                Request::post(path)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["content-type"], "application/json");
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn registry_empty_ids_does_not_require_a_database_or_upstream() {
        for path in ["/profiles", "/profiles/metadata"] {
            assert_eq!(
                call(path, r#"{"ids":[]}"#).await,
                (StatusCode::OK, json!([]))
            );
        }
    }

    #[tokio::test]
    async fn registry_oversized_body_is_a_json_413() {
        let body = json!({"ids": ["a".repeat(BODY_LIMIT)]}).to_string();
        for path in ["/profiles", "/profiles/metadata"] {
            assert_eq!(
                call(path, &body).await,
                (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    json!({
                        "error": "Payload Too Large",
                        "message": "The request body exceeds 262144 bytes",
                    })
                )
            );
        }
    }

    #[tokio::test]
    async fn registry_failed_lookup_is_an_error_not_an_empty_answer() {
        for path in ["/profiles", "/profiles/metadata"] {
            assert_eq!(
                call(path, r#"{"ids":["0xmissing"]}"#).await,
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error":"Internal Server Error"})
                )
            );
        }
    }

    #[tokio::test]
    async fn registry_ids_are_capped_like_lambdas() {
        let ids: Vec<String> = (0..=PROFILE_IDS_MAX)
            .map(|i| format!("0x{i:040x}"))
            .collect();
        for path in ["/profiles", "/profiles/metadata"] {
            assert_eq!(
                call(path, &json!({"ids": ids}).to_string()).await,
                (
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Bad request",
                        "message": "Too many profile ids were specified. Maximum allowed is 1000",
                    })
                )
            );
        }
        // At the cap the request is admitted (and fails later on the missing DB).
        let ids = &ids[..PROFILE_IDS_MAX];
        let (status, _) = call("/profiles", &json!({"ids": ids}).to_string()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn registry_profiles_follow_request_order() {
        let mut entities = vec![
            json!({"id":"a", "pointers":["0xaa"]}),
            json!({"id":"b", "pointers":["0xBB"]}),
            json!({"id":"c", "pointers":["0xcc"]}),
        ];
        sort_by_request(
            &mut entities,
            &["0xcc".into(), "0xbb".into(), "0xaa".into()],
        );
        let ids: Vec<_> = entities.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
    }

    #[tokio::test]
    async fn registry_malformed_entity_is_left_out_not_the_batch() {
        let state = state_with_empty_readers(None).await;
        let mut broken = entity();
        broken.as_object_mut().unwrap().remove("timestamp");
        let full = values(
            project_profiles(&state, &[broken, entity()], false)
                .await
                .unwrap_or_else(|_| panic!("profile processing failed")),
        );
        assert_eq!(full.len(), 1);
        assert_eq!(full[0]["timestamp"], 1700000000000_u64);
    }

    /// The Registry's CORS (`captures/abr/AB-12-preflight.json`): the
    /// captured preflight headers exactly, and origin `*` on a read only when
    /// the request names an origin.
    #[tokio::test]
    async fn registry_profile_preflight_and_empty_reads_allow_browser_clients() {
        let capture: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/AB-12-preflight.json"
        )))
        .unwrap();
        let app = router(state_without_db());
        for path in ["/profiles", "/profiles/metadata"] {
            let mut request = Request::builder().method("OPTIONS").uri(path);
            for (name, value) in capture["request"]["headers"].as_object().unwrap() {
                request = request.header(name.as_str(), value.as_str().unwrap());
            }
            let preflight = app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                u64::from(preflight.status().as_u16()),
                capture["response"]["status"].as_u64().unwrap()
            );
            let headers: serde_json::Map<String, Value> = preflight
                .headers()
                .iter()
                // The capture keeps the CORS headers; axum also adds framing
                // (`content-length`) and `Allow` for a route without an
                // OPTIONS handler.
                .filter(|(name, _)| name.as_str().starts_with("access-control-") || *name == "vary")
                .map(|(name, value)| (name.to_string(), json!(value.to_str().unwrap())))
                .collect();
            assert_eq!(
                Value::Object(headers),
                capture["response"]["headers"],
                "{path}"
            );
            for origin in [Some("https://explorer.example"), None] {
                let mut request = Request::post(path).header("content-type", "application/json");
                if let Some(origin) = origin {
                    request = request.header("origin", origin);
                }
                let response = app
                    .clone()
                    .oneshot(request.body(Body::from(r#"{"ids":[]}"#)).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    response
                        .headers()
                        .get("access-control-allow-origin")
                        .map(|v| v.to_str().unwrap()),
                    origin.map(|_| "*")
                );
                assert!(
                    response
                        .headers()
                        .get("access-control-expose-headers")
                        .is_none()
                );
                assert_eq!(
                    to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
                    b"[]"
                );
            }
        }
    }

    #[tokio::test]
    async fn registry_invalid_ids_preserve_captured_error_envelope() {
        let capture: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/AB-3-missing-ids.json"
        ))).unwrap();
        let expected = capture["response"]["body"].clone();
        for path in ["/profiles", "/profiles/metadata"] {
            for body in [
                "{}",
                r#"{"ids":null}"#,
                r#"{"ids":"x"}"#,
                r#"{"ids":[""]}"#,
                r#"{"ids":[1]}"#,
            ] {
                assert_eq!(
                    call(path, body).await,
                    (StatusCode::BAD_REQUEST, expected.clone())
                );
            }
        }
    }

    #[tokio::test]
    async fn registry_invalid_json_matches_current_capture() {
        let capture: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/AB-3-not-json.json"
        ))).unwrap();
        for path in ["/profiles", "/profiles/metadata"] {
            for body in ["", "{"] {
                assert_eq!(call(path, body).await, (
                    StatusCode::from_u16(capture["response"]["status"].as_u64().unwrap() as u16).unwrap(),
                    capture["response"]["body"].clone(),
                ));
            }
        }
    }

    fn entity() -> Value {
        json!({
            "id":"bafkprofile", "type":"profile", "pointers":["0xabc"], "timestamp":1700000000000_i64,
            "metadata": {"unrelated":"omit", "timestamp":1, "avatars":[{
                "name":"Alice", "hasClaimedName":true, "nameColor":null,
                "avatar": {"wearables":[
                    "urn:decentraland:off-chain:base-avatars:eyes_00",
                    "urn:decentraland:matic:collections-v2:0xabc:1"
                ], "snapshots":{"face256":"https://external.example/face","body":"old"}}
            }]}
        })
    }

    #[tokio::test]
    async fn registry_projection_uses_shared_ownership_and_entity_snapshot_identity() {
        let state = state_with_empty_readers(None).await;
        let mut entity = entity();
        entity["metadata"]["avatars"][0]["avatar"]["snapshots"] = json!({
            "face256": "https://external.example/face", "body": "old",
            "face128": "bafkold", "face": "bafkold2",
        });
        let entities = [entity];
        let full_raw = project_profiles(&state, &entities, false)
            .await
            .unwrap_or_else(|_| panic!("profile processing failed"));
        let compact_raw = project_profiles(&state, &entities, true)
            .await
            .unwrap_or_else(|_| panic!("profile processing failed"));
        // The Registry's key order (AB-2 and AB-1 captures).
        assert!(
            full_raw[0]
                .get()
                .starts_with(r#"{"timestamp":1700000000000,"avatars":["#)
        );
        assert!(compact_raw[0].get().starts_with(r#"{"pointer":"0xabc","hasClaimedName":false,"name":"Alice","nameColor":null,"thumbnailUrl":"#));
        let (full, compact) = (values(full_raw), values(compact_raw));
        assert_eq!(full[0].as_object().unwrap().len(), 2);
        assert_eq!(full[0]["timestamp"], 1700000000000_u64);
        let avatar = &full[0]["avatars"][0];
        assert_eq!(avatar["hasClaimedName"], false);
        assert_eq!(
            avatar["avatar"]["wearables"],
            json!(["urn:decentraland:off-chain:base-avatars:eyes_00"])
        );
        // The Registry sanitizer's snapshots: exactly these two keys.
        assert_eq!(
            avatar["avatar"]["snapshots"],
            json!({
                "face256": "http://localhost/profile-images/entities/bafkprofile/face.png",
                "body": "http://localhost/profile-images/entities/bafkprofile/body.png",
            })
        );
        assert_eq!(
            compact,
            vec![json!({
                "pointer":"0xabc", "name":"Alice", "nameColor":null, "hasClaimedName":false,
                "thumbnailUrl":"http://localhost/profile-images/entities/bafkprofile/face.png"
            })]
        );
    }

    /// Like the Registry sanitizer, an avatar without an `avatar` object
    /// gets no snapshots.
    #[tokio::test]
    async fn registry_projection_leaves_an_avatar_without_avatar_object_unchanged() {
        let avatars = json!([{"name": "Bob", "userId": "0xabc"}]);
        let projected = project(
            &state_without_db(),
            &entity(),
            json!({"avatars": avatars.clone()}),
            false,
        )
        .unwrap();
        let projected: Value = serde_json::from_str(projected.get()).unwrap();
        assert_eq!(projected["avatars"], avatars);
    }

    #[tokio::test]
    async fn registry_projection_preserves_omitted_name_color() {
        let state = state_without_db();
        let mut entity = entity();
        entity["metadata"]["avatars"][0]
            .as_object_mut()
            .unwrap()
            .remove("nameColor");
        let compact = values(
            project_profiles(&state, &[entity], true)
                .await
                .unwrap_or_else(|_| panic!("profile processing failed")),
        );
        assert!(compact[0].get("nameColor").is_none());
    }

    #[tokio::test]
    async fn registry_routes_read_local_profiles_and_observe_changes_without_a_registry_cache() {
        let Some(state) =
            crate::market::tests::support::state_with_db(crate::market::Settings::for_tests())
                .await
        else {
            return;
        };
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let pointer = format!("0x{tag}00000000");
        let entity_id = format!("bafyprofile{tag}");
        let mut metadata = entity()["metadata"].clone();
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ($1, 'v3', 'profile', $2, $3, to_timestamp(1700000000), ARRAY[$1], timezone('UTC', now()), '[]'::json)",
        )
        .bind(&pointer)
        .bind(&entity_id)
        .bind(json!({"v": metadata}))
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'profile')")
            .bind(&pointer)
            .bind(&entity_id)
            .execute(&state.pool)
            .await
            .unwrap();
        let scene_pointer = format!("{},0", u64::from_str_radix(&tag[..12], 16).unwrap());
        let malformed_pointer = format!("0xmissingavatars{tag}");
        let other_entities = [
            (
                format!("bafyscene{tag}"),
                scene_pointer.clone(),
                "scene",
                json!({"scene":{"base":scene_pointer}}),
            ),
            (
                format!("bafyempty{tag}"),
                malformed_pointer.clone(),
                "profile",
                json!({"avatars":[]}),
            ),
        ];
        for (id, pointer, kind, metadata) in &other_entities {
            sqlx::query(
                "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
                 VALUES ($1, 'v3', $2, $3, $4, to_timestamp(1700000000), ARRAY[$1], timezone('UTC', now()), '[]'::json)",
            ).bind(pointer).bind(kind).bind(id).bind(json!({"v":metadata}))
                .execute(&state.pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, $3)")
                .bind(pointer).bind(id).bind(kind).execute(&state.pool).await.unwrap();
        }
        let app = router(state.clone());
        for (path, name) in [("/profiles", "Alice"), ("/profiles/metadata", "Updated")] {
            if name == "Updated" {
                metadata["avatars"][0]["name"] = json!(name);
                sqlx::query(
                    "UPDATE content.deployments SET entity_metadata = $1 WHERE entity_id = $2",
                )
                .bind(json!({"v": metadata}))
                .bind(&entity_id)
                .execute(&state.pool)
                .await
                .unwrap();
            }
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .body(Body::from(
                            json!({"ids":[scene_pointer, pointer.to_uppercase(), malformed_pointer, pointer, "0xunknown"]})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                    .unwrap();
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body.as_array().unwrap().len(), 1);
            if path == "/profiles" {
                assert_eq!(body[0]["timestamp"], 1700000000000_u64);
                assert_eq!(body[0]["avatars"][0]["name"], name);
                assert_eq!(body[0]["avatars"][0]["userId"], pointer);
            } else {
                assert_eq!(body[0]["name"], name);
                assert_eq!(body[0]["pointer"], pointer);
                assert_eq!(
                    body[0]["thumbnailUrl"],
                    format!("http://localhost/profile-images/entities/{entity_id}/face.png")
                );
            }
        }
        let unavailable = std::sync::Arc::new(MarketplaceState::new(
            state.pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings {
                marketplace_subgraph_url: Some("http://127.0.0.1:1/".into()),
                ..crate::market::Settings::for_tests()
            },
        ));
        unavailable
            .test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let app = router(unavailable);
        for path in ["/profiles", "/profiles/metadata"] {
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .body(Body::from(json!({"ids":[pointer]}).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(
                to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
                br#"{"error":"Internal Server Error"}"#,
                "unvalidated metadata must never be returned when an ownership reader fails"
            );
        }
        for (id, pointer, _, _) in &other_entities {
            sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
                .bind(pointer)
                .execute(&state.pool)
                .await
                .unwrap();
            sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
                .bind(id)
                .execute(&state.pool)
                .await
                .unwrap();
        }
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
            .bind(&pointer)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
            .bind(&entity_id)
            .execute(&state.pool)
            .await
            .unwrap();
    }
}
