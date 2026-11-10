//! Registry DTOs over the same local entity lookup and profile processing as
//! Catalyst Lambdas. There is no Registry upstream or separate profile cache.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use super::{
    MarketplaceState, PROFILE_IDS_MAX, SharedState, active_entities, process_profiles_positional,
    rewrite_snapshot_urls,
};

const INVALID_IDS: &str = "The ids property must be an array of non-empty strings";

/// Mount under `/asset-bundle-registry`, using the existing market state.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/profiles", post(profiles))
        .route("/profiles/metadata", post(metadata))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(tower_http::cors::CorsLayer::permissive())
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

async fn profiles(State(state): State<SharedState>, body: Bytes) -> Response {
    respond(state, body, false).await
}

async fn metadata(State(state): State<SharedState>, body: Bytes) -> Response {
    respond(state, body, true).await
}

async fn respond(state: SharedState, body: Bytes, compact: bool) -> Response {
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
    entities.sort_by_cached_key(|entity| {
        entity
            .get("pointers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(|p| pointers.iter().position(|q| *q == p.to_lowercase()))
            .min()
            .unwrap_or(usize::MAX)
    });
}

/// The processed profiles in `entities` order. A malformed entity (no id,
/// pointer, avatar or timestamp) is left out with a warning; only a failed
/// ownership pass fails the batch.
async fn project_profiles(
    state: &MarketplaceState,
    entities: &[Value],
    compact: bool,
) -> Result<Vec<Value>, ()> {
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
) -> Option<Value> {
    let id = entity.get("id").and_then(Value::as_str)?;
    // Registry always replaces snapshots. Reuse the shared rewriter,
    // pointing at this node's existing profile-images routes.
    rewrite_snapshot_urls(
        id,
        &mut profile,
        Some(&state.settings.profile_images_public_url),
        &state.settings.profile_images_public_url,
    );
    if compact {
        let pointer = entity
            .get("pointers")
            .and_then(Value::as_array)
            .and_then(|pointers| pointers.first())
            .and_then(Value::as_str)?;
        let avatar = profile
            .get("avatars")
            .and_then(Value::as_array)
            .and_then(|avatars| avatars.first())
            .and_then(Value::as_object)?;
        let mut result = json!({
            "pointer": pointer,
            "thumbnailUrl": format!(
                "{}/entities/{id}/face.png",
                state.settings.profile_images_public_url.trim_end_matches('/'),
            ),
        });
        for field in ["hasClaimedName", "name", "nameColor"] {
            if let Some(value) = avatar.get(field) {
                result[field] = value.clone();
            }
        }
        Some(result)
    } else {
        // Registry's PostgreSQL bigint is serialized as a string in
        // the pinned integration expectations and September captures,
        // despite the OpenAPI timestamp schema declaring a number.
        let timestamp = match entity.get("timestamp") {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Number(value)) => value.to_string(),
            _ => return None,
        };
        Some(json!({"timestamp": timestamp, "avatars": profile["avatars"]}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::market::tests::support::{state_with_empty_readers, state_without_db};

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
        let full = project_profiles(&state, &[broken, entity()], false)
            .await
            .unwrap_or_else(|_| panic!("profile processing failed"));
        assert_eq!(full.len(), 1);
        assert_eq!(full[0]["timestamp"], "1700000000000");
    }

    #[tokio::test]
    async fn registry_profile_preflight_and_empty_reads_allow_browser_clients() {
        let app = router(state_without_db());
        for path in ["/profiles", "/profiles/metadata"] {
            let preflight = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("OPTIONS")
                        .uri(path)
                        .header("origin", "https://explorer.example")
                        .header("access-control-request-method", "POST")
                        .header("access-control-request-headers", "content-type")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(preflight.status().is_success());
            assert_eq!(preflight.headers()["access-control-allow-origin"], "*");
            assert_eq!(preflight.headers()["access-control-allow-methods"], "*");
            assert_eq!(preflight.headers()["access-control-allow-headers"], "*");
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .header("origin", "https://explorer.example")
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"ids":[]}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["access-control-allow-origin"], "*");
            assert_eq!(
                to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
                b"[]"
            );
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
        let entities = [entity()];
        let full = project_profiles(&state, &entities, false)
            .await
            .unwrap_or_else(|_| panic!("profile processing failed"));
        let compact = project_profiles(&state, &entities, true)
            .await
            .unwrap_or_else(|_| panic!("profile processing failed"));
        assert_eq!(full[0].as_object().unwrap().len(), 2);
        assert_eq!(full[0]["timestamp"], "1700000000000");
        let avatar = &full[0]["avatars"][0];
        assert_eq!(avatar["hasClaimedName"], false);
        assert_eq!(
            avatar["avatar"]["wearables"],
            json!(["urn:decentraland:off-chain:base-avatars:eyes_00"])
        );
        assert_eq!(
            avatar["avatar"]["snapshots"]["face256"],
            "http://localhost/profile-images/entities/bafkprofile/face.png"
        );
        assert_eq!(
            compact,
            vec![json!({
                "pointer":"0xabc", "name":"Alice", "nameColor":null, "hasClaimedName":false,
                "thumbnailUrl":"http://localhost/profile-images/entities/bafkprofile/face.png"
            })]
        );
    }

    #[tokio::test]
    async fn registry_projection_preserves_omitted_name_color() {
        let state = state_without_db();
        let mut entity = entity();
        entity["metadata"]["avatars"][0]
            .as_object_mut()
            .unwrap()
            .remove("nameColor");
        let compact = project_profiles(&state, &[entity], true)
            .await
            .unwrap_or_else(|_| panic!("profile processing failed"));
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
                assert_eq!(body[0]["timestamp"], "1700000000000");
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
