//! `POST /reprocess-ab`: worlds-content-server's `reprocessABHandler`
//! (`src/controllers/handlers/reprocess-ab-handler.ts`). The route checks the
//! admin bearer, then validates the body against `reprocessABSchema`
//! (`src/controllers/schemas/reprocess-ab-schemas.ts`) and answers with the
//! validator's 415 and 400 bodies. It then takes every deployed scene of the
//! named worlds, optionally only the listed `entityIds`, minus the worlds
//! `getRawWorldRecords` leaves out, and answers upstream's
//! `{baseUrl, batch, successful, failed}`.
//!
//! Upstream publishes one `WorldDeploymentEvent` per scene to SNS. This tree
//! publishes no SNS events, so each scene goes to the local asset-bundle
//! converter instead (abgen, `ABGEN_ADMIN_URL`). abgen converts on request:
//! for each platform the route asks for the scene's manifest
//! (`GET /manifest/{entity}_{platform}.json`), and abgen converts a manifest
//! it does not hold before it answers. A manifest it already holds is served
//! as it is and not converted again. A scene counts as `successful` when
//! abgen took every platform: it answered 2xx, or it was still converting
//! when the hand-off timed out (abgen keeps building after the caller stops
//! waiting). It counts as `failed` when abgen answered an error or could not
//! be reached. With no converter configured the route answers upstream's
//! 500, as upstream does without `AWS_SNS_ARN`.

use std::collections::HashMap;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};

use crate::content::validation;

use super::SharedState;
use super::auth::authorize_admin;
use super::http::ApiError;

/// The platforms abgen converts a scene for (its conversion lambda's default
/// `windows,mac`).
pub const PLATFORMS: [&str; 2] = ["windows", "mac"];

/// How long one manifest hand-off waits for abgen. A conversion still running
/// at that point keeps running in abgen and counts as handed off.
const HAND_OFF_TIMEOUT: Duration = Duration::from_secs(10);

/// Scenes handed to abgen at once; abgen bounds its own build concurrency.
const HAND_OFF_CONCURRENCY: usize = 8;

/// `reprocess-ab-schemas.ts`'s `WORLD_NAME_PATTERN`.
pub const WORLD_NAME_PATTERN: &str = r"^[a-zA-Z0-9-]+\.(dcl\.eth|eth)$";

pub const NO_SCENES_MESSAGE: &str = "No scenes found for reprocessing";

/// The local asset-bundle converter: abgen's base URL (`ABGEN_ADMIN_URL`, a
/// loopback address `serve` validated) and the client that reaches it.
#[derive(Clone)]
pub struct Converter {
    base: String,
    client: reqwest::Client,
}

impl std::fmt::Debug for Converter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Converter")
            .field("base", &self.base)
            .finish()
    }
}

impl Converter {
    pub fn new(base: &str, client: reqwest::Client) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Asks abgen for `entity_id`'s manifest on `platform`; `true` when abgen
    /// took the conversion (see the module doc).
    async fn hand_off(&self, entity_id: &str, platform: &str) -> bool {
        if entity_id.is_empty() || !entity_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            tracing::warn!(entity_id, "reprocess-ab: an entity id that is not a CID");
            return false;
        }
        let url = format!("{}/manifest/{entity_id}_{platform}.json", self.base);
        match self.client.get(&url).timeout(HAND_OFF_TIMEOUT).send().await {
            Ok(response) if response.status().is_success() => true,
            Ok(response) => {
                tracing::warn!(
                    entity_id,
                    platform,
                    status = response.status().as_u16(),
                    "reprocess-ab: abgen refused the conversion"
                );
                false
            }
            Err(e) if e.is_timeout() && !e.is_connect() => true,
            Err(e) => {
                tracing::warn!(
                    entity_id,
                    platform,
                    error = %e.without_url(),
                    "reprocess-ab: abgen is unreachable"
                );
                false
            }
        }
    }

    /// Whether abgen took every platform of `entity_id`.
    async fn reprocess(&self, entity_id: &str) -> bool {
        let handed = futures::future::join_all(
            PLATFORMS
                .iter()
                .map(|platform| self.hand_off(entity_id, platform)),
        )
        .await;
        handed.into_iter().all(|ok| ok)
    }
}

/// One `worlds` item of a valid body: the lowercased world name and its
/// optional entity filter.
#[derive(Debug, PartialEq)]
pub struct ReprocessWorld {
    pub world_name: String,
    pub entity_ids: Option<Vec<String>>,
}

/// A refused body, answered as `@dcl/schema-validator-component` answers it.
#[derive(Debug)]
pub struct Refusal {
    pub status: StatusCode,
    pub body: Value,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json")],
            validation::ordered_refusal_json(&self.body),
        )
            .into_response()
    }
}

/// `withSchemaValidatorMiddleware(reprocessABSchema)`: a JSON content type
/// (415), a JSON body (400 with the parse error), then the schema (400
/// `Invalid JSON body` with AJV's first error).
pub fn reprocess_body(headers: &HeaderMap, body: &[u8]) -> Result<Vec<ReprocessWorld>, Refusal> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    if !validation::is_json_content_type(content_type) {
        return Err(Refusal {
            status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            body: json!({ "ok": false, "message": "Content-Type must be application/json" }),
        });
    }
    let value: Value = serde_json::from_slice(body).map_err(|e| Refusal {
        status: StatusCode::BAD_REQUEST,
        body: json!({ "ok": false, "message": validation::json_parse_message(body, &e) }),
    })?;
    if let Some(error) = schema_error(&value, body) {
        return Err(Refusal {
            status: StatusCode::BAD_REQUEST,
            body: json!({ "ok": false, "message": "Invalid JSON body", "data": [error] }),
        });
    }
    Ok(value["worlds"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| ReprocessWorld {
            world_name: item["worldName"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase(),
            entity_ids: item["entityIds"].as_array().map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            }),
        })
        .collect())
}

fn ajv_error(
    instance: String,
    schema: String,
    keyword: &str,
    params: Value,
    message: String,
) -> Value {
    json!({
        "instancePath": instance,
        "schemaPath": format!("#/{schema}"),
        "keyword": keyword,
        "params": params,
        "message": message,
    })
}

fn type_error(instance: String, schema: &str, expected: &str) -> Value {
    ajv_error(
        instance,
        format!("{schema}type"),
        "type",
        json!({ "type": expected }),
        format!("must be {expected}"),
    )
}

/// The object keys of `raw` in body order, as `JSON.parse` keeps them, which
/// is the order AJV names the first extra key in.
fn keys_in_order(value: &Value, raw: &[u8]) -> Vec<String> {
    validation::object_keys_in_order(value, raw)
}

/// The raw text of each member of `raw`'s `key` array, when `raw` is an object
/// whose `key` is an array; the nested objects' key order needs their text.
fn raw_items(raw: &[u8], key: &str) -> Vec<Box<RawValue>> {
    serde_json::from_slice::<HashMap<String, Box<RawValue>>>(raw)
        .ok()
        .and_then(|members| members.get(key).map(|v| v.get().to_string()))
        .and_then(|array| serde_json::from_str::<Vec<Box<RawValue>>>(&array).ok())
        .unwrap_or_default()
}

/// The first error AJV 8 (`allErrors: false`) reports for `reprocessABSchema`,
/// in its keyword order: type, required, additionalProperties, then each
/// property in schema order (an array's minItems before its items, a string's
/// pattern after its type).
fn schema_error(body: &Value, raw: &[u8]) -> Option<Value> {
    let Some(root) = body.as_object() else {
        return Some(type_error(String::new(), "", "object"));
    };
    let Some(worlds) = root.get("worlds") else {
        return Some(ajv_error(
            String::new(),
            "required".into(),
            "required",
            json!({ "missingProperty": "worlds" }),
            "must have required property 'worlds'".into(),
        ));
    };
    if let Some(extra) = keys_in_order(body, raw).into_iter().find(|k| k != "worlds") {
        return Some(additional_property(String::new(), String::new(), &extra));
    }
    let schema = "properties/worlds/";
    let Some(items) = worlds.as_array() else {
        return Some(type_error("/worlds".into(), schema, "array"));
    };
    if items.is_empty() {
        return Some(ajv_error(
            "/worlds".into(),
            format!("{schema}minItems"),
            "minItems",
            json!({ "limit": 1 }),
            "must NOT have fewer than 1 items".into(),
        ));
    }
    let raw_items = raw_items(raw, "worlds");
    items.iter().enumerate().find_map(|(i, item)| {
        let item_raw = raw_items.get(i).map(|r| r.get().as_bytes()).unwrap_or(b"");
        world_item_error(i, item, item_raw)
    })
}

fn additional_property(instance: String, schema: String, extra: &str) -> Value {
    ajv_error(
        instance,
        format!("{schema}additionalProperties"),
        "additionalProperties",
        json!({ "additionalProperty": extra }),
        "must NOT have additional properties".into(),
    )
}

/// AJV's first error for `worlds[i]`: `{worldName: pattern, entityIds?: string[]}`.
fn world_item_error(i: usize, item: &Value, raw: &[u8]) -> Option<Value> {
    let instance = format!("/worlds/{i}");
    let schema = "properties/worlds/items/";
    let Some(object) = item.as_object() else {
        return Some(type_error(instance, schema, "object"));
    };
    let Some(world_name) = object.get("worldName") else {
        return Some(ajv_error(
            instance,
            format!("{schema}required"),
            "required",
            json!({ "missingProperty": "worldName" }),
            "must have required property 'worldName'".into(),
        ));
    };
    if let Some(extra) = keys_in_order(item, raw)
        .into_iter()
        .find(|k| k != "worldName" && k != "entityIds")
    {
        return Some(additional_property(instance, schema.into(), &extra));
    }
    let name_schema = format!("{schema}properties/worldName/");
    match world_name.as_str() {
        None => {
            return Some(type_error(
                format!("{instance}/worldName"),
                &name_schema,
                "string",
            ));
        }
        Some(name) if !matches_world_name_pattern(name) => {
            return Some(ajv_error(
                format!("{instance}/worldName"),
                format!("{name_schema}pattern"),
                "pattern",
                json!({ "pattern": WORLD_NAME_PATTERN }),
                format!("must match pattern \"{WORLD_NAME_PATTERN}\""),
            ));
        }
        Some(_) => {}
    }
    let ids_schema = format!("{schema}properties/entityIds/");
    let ids = object.get("entityIds")?;
    let Some(ids) = ids.as_array() else {
        return Some(type_error(
            format!("{instance}/entityIds"),
            &ids_schema,
            "array",
        ));
    };
    ids.iter().position(|id| !id.is_string()).map(|j| {
        type_error(
            format!("{instance}/entityIds/{j}"),
            &format!("{ids_schema}items/"),
            "string",
        )
    })
}

/// [`WORLD_NAME_PATTERN`] as JavaScript reads it: one or more ASCII letters,
/// digits or hyphens, then `.dcl.eth` or `.eth` at the very end.
pub fn matches_world_name_pattern(name: &str) -> bool {
    let label_ok = |label: &str| {
        !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    name.strip_suffix(".dcl.eth").is_some_and(label_ok)
        || name.strip_suffix(".eth").is_some_and(label_ok)
}

#[derive(Serialize)]
struct BatchEntity<'a> {
    #[serde(rename = "entityId")]
    entity_id: &'a str,
    #[serde(rename = "authChain")]
    auth_chain: &'a RawValue,
}

#[derive(Serialize)]
struct BatchItem<'a> {
    entity: BatchEntity<'a>,
    #[serde(rename = "contentServerUrls")]
    content_server_urls: [&'a str; 1],
}

#[derive(Serialize)]
struct ReprocessAnswer<'a> {
    #[serde(rename = "baseUrl")]
    base_url: &'a str,
    batch: Vec<BatchItem<'a>>,
    successful: usize,
    failed: usize,
}

/// A deployed scene the request names.
#[derive(sqlx::FromRow)]
struct SceneRow {
    world_name: String,
    entity_id: String,
    auth_chain: String,
}

/// Upstream's `HTTP_BASE_URL || <request origin>`: this node's worlds content
/// server base (`<public_url>/worlds-content-server`), or the request's host
/// when no public URL is set.
fn base_url(state: &SharedState, headers: &HeaderMap) -> String {
    const PREFIX: &str = "/worlds-content-server";
    let root = if state.settings.public_url.is_empty() {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost");
        format!("http://{host}")
    } else {
        state.settings.public_url.clone()
    };
    if root.ends_with(PREFIX) {
        root
    } else {
        format!("{root}{PREFIX}")
    }
}

/// POST /reprocess-ab
pub async fn reprocess_ab(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(e) = authorize_admin(state.settings.admin_token.as_deref(), &headers) {
        return e.into_response();
    }
    let worlds = match reprocess_body(&headers, &body) {
        Ok(worlds) => worlds,
        Err(refusal) => return refusal.into_response(),
    };
    let Some(converter) = state.converter.clone() else {
        return ApiError::internal(
            "POST /reprocess-ab: no asset-bundle converter is configured (ABGEN_ADMIN_URL)",
        )
        .into_response();
    };
    match reprocess(&state, &converter, &headers, worlds).await {
        Ok(response) => response,
        Err(e) => e.into_response(),
    }
}

async fn reprocess(
    state: &SharedState,
    converter: &Converter,
    headers: &HeaderMap,
    worlds: Vec<ReprocessWorld>,
) -> Result<Response, ApiError> {
    let base_url = base_url(state, headers);
    // A later item for the same world replaces an earlier one (a JS `Map`).
    let mut wanted: HashMap<String, Option<Vec<String>>> = HashMap::new();
    for world in worlds {
        wanted.insert(world.world_name, world.entity_ids);
    }
    let names: Vec<String> = wanted.keys().cloned().collect();
    let banned = state.names.snapshot(&state.pool).await.banned_names();
    let rows: Vec<SceneRow> = sqlx::query_as(
        "SELECT s.world_name, s.entity_id, s.deployment_auth_chain::text AS auth_chain \
         FROM worlds.world_scenes s JOIN worlds.worlds w ON w.name = s.world_name \
         WHERE s.world_name = ANY($1) AND NOT (w.name = ANY($2)) \
         ORDER BY s.world_name, s.created_at, s.entity_id",
    )
    .bind(&names)
    .bind(&banned)
    .fetch_all(&state.pool)
    .await?;
    let scenes: Vec<SceneRow> = rows
        .into_iter()
        .filter(|scene| match wanted.get(&scene.world_name) {
            Some(Some(ids)) if !ids.is_empty() => ids.contains(&scene.entity_id),
            Some(_) => true,
            None => false,
        })
        .collect();
    if scenes.is_empty() {
        return Err(ApiError::bad_request(NO_SCENES_MESSAGE));
    }
    let chains: Vec<Box<RawValue>> = scenes
        .iter()
        .map(|scene| {
            RawValue::from_string(scene.auth_chain.clone())
                .map_err(|e| ApiError::internal(format!("stored auth chain: {e}")))
        })
        .collect::<Result<_, _>>()?;
    let hand_offs: Vec<_> = scenes
        .iter()
        .map(|scene| {
            let (converter, entity_id) = (converter.clone(), scene.entity_id.clone());
            async move { converter.reprocess(&entity_id).await }
        })
        .collect();
    let handed: Vec<bool> = futures::stream::iter(hand_offs)
        .buffered(HAND_OFF_CONCURRENCY)
        .collect()
        .await;
    let successful = handed.iter().filter(|ok| **ok).count();
    let failed = handed.len() - successful;
    tracing::info!(
        successful,
        failed,
        "reprocess-ab: scenes handed to the converter"
    );
    let answer = ReprocessAnswer {
        base_url: &base_url,
        batch: scenes
            .iter()
            .zip(&chains)
            .map(|(scene, chain)| BatchItem {
                entity: BatchEntity {
                    entity_id: &scene.entity_id,
                    auth_chain: chain,
                },
                content_server_urls: [&base_url],
            })
            .collect(),
        successful,
        failed,
    };
    let body = serde_json::to_string(&answer)
        .map_err(|e| ApiError::internal(format!("reprocess-ab answer: {e}")))?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        h
    }

    fn refusal(body: &str) -> Value {
        let r = reprocess_body(&json_headers(), body.as_bytes()).unwrap_err();
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}");
        r.body
    }

    fn first_error(body: &str) -> Value {
        let r = refusal(body);
        assert_eq!(r["message"], "Invalid JSON body", "{r}");
        r["data"][0].clone()
    }

    #[test]
    fn the_world_name_pattern_reads_as_javascript_does() {
        for ok in ["a.dcl.eth", "My-World.eth", "dcl.eth", "0x1.dcl.eth"] {
            assert!(matches_world_name_pattern(ok), "{ok}");
        }
        for bad in [
            "",
            ".eth",
            "a.b.eth",
            "a_b.dcl.eth",
            "a.dcl.eth\n",
            "a.ETH",
            "a.com",
        ] {
            assert!(!matches_world_name_pattern(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_valid_body_is_lowercased_and_keeps_its_entity_filter() {
        let worlds = reprocess_body(
            &json_headers(),
            br#"{"worlds":[{"worldName":"Foo.dcl.eth","entityIds":["bafy1"]},{"worldName":"bar.eth"}]}"#,
        )
        .unwrap();
        assert_eq!(
            worlds,
            vec![
                ReprocessWorld {
                    world_name: "foo.dcl.eth".into(),
                    entity_ids: Some(vec!["bafy1".into()]),
                },
                ReprocessWorld {
                    world_name: "bar.eth".into(),
                    entity_ids: None,
                },
            ]
        );
    }

    #[test]
    fn a_body_that_is_not_json_answers_the_validator_bodies() {
        let r = reprocess_body(&HeaderMap::new(), b"{}").unwrap_err();
        assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            validation::ordered_refusal_json(&r.body),
            r#"{"ok":false,"message":"Content-Type must be application/json"}"#
        );
        assert_eq!(refusal("")["message"], "Unexpected end of JSON input");
    }

    #[test]
    fn schema_errors_follow_ajv_keyword_order() {
        let cases: [(&str, &str, &str, &str); 11] = [
            ("[]", "", "#/type", "type"),
            ("{}", "", "#/required", "required"),
            (
                r#"{"worlds":[],"x":1}"#,
                "",
                "#/additionalProperties",
                "additionalProperties",
            ),
            (
                r#"{"worlds":{}}"#,
                "/worlds",
                "#/properties/worlds/type",
                "type",
            ),
            (
                r#"{"worlds":[]}"#,
                "/worlds",
                "#/properties/worlds/minItems",
                "minItems",
            ),
            (
                r#"{"worlds":[1]}"#,
                "/worlds/0",
                "#/properties/worlds/items/type",
                "type",
            ),
            (
                r#"{"worlds":[{}]}"#,
                "/worlds/0",
                "#/properties/worlds/items/required",
                "required",
            ),
            (
                r#"{"worlds":[{"z":1,"worldName":"a.eth","b":2}]}"#,
                "/worlds/0",
                "#/properties/worlds/items/additionalProperties",
                "additionalProperties",
            ),
            (
                r#"{"worlds":[{"worldName":"a.eth"},{"worldName":"a.com"}]}"#,
                "/worlds/1/worldName",
                "#/properties/worlds/items/properties/worldName/pattern",
                "pattern",
            ),
            (
                r#"{"worlds":[{"worldName":"a.eth","entityIds":"x"}]}"#,
                "/worlds/0/entityIds",
                "#/properties/worlds/items/properties/entityIds/type",
                "type",
            ),
            (
                r#"{"worlds":[{"worldName":"a.eth","entityIds":["x",2]}]}"#,
                "/worlds/0/entityIds/1",
                "#/properties/worlds/items/properties/entityIds/items/type",
                "type",
            ),
        ];
        for (body, instance, schema, keyword) in cases {
            let e = first_error(body);
            assert_eq!(
                (
                    e["instancePath"].as_str(),
                    e["schemaPath"].as_str(),
                    e["keyword"].as_str()
                ),
                (Some(instance), Some(schema), Some(keyword)),
                "{body}: {e}"
            );
        }
        // The first extra key in body order, not alphabetical order.
        assert_eq!(
            first_error(r#"{"worlds":[{"z":1,"worldName":"a.eth","b":2}]}"#)["params"],
            json!({ "additionalProperty": "z" })
        );
        assert_eq!(
            first_error(r#"{"worlds":[{"worldName":"a.com"}]}"#)["message"],
            format!("must match pattern \"{WORLD_NAME_PATTERN}\"")
        );
    }
}
