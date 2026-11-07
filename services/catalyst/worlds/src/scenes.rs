//! `GET /world/{world_name}/scenes` (browse, with the bounding-box, deployer
//! and entity filters) and `POST /world/{world_name}/scenes` (the same with
//! `{coordinates}` in the body), paged like @dcl/http-commons and answering
//! in the worlds error envelope.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use chrono::SecondsFormat;
use serde::Serialize;
use serde_json::{Value, json};

use super::SharedState;
use super::queries::{self, SceneFilters};
use crate::comms::world_identity::Parcel;
use crate::core::http::{ApiError, QueryPairs, pagination, query_param};
use crate::shared::identity::Address;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldSceneEntry {
    pub world_name: String,
    pub deployer: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown[]"))]
    pub deployment_auth_chain: Value,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub entity: Value,
    pub entity_id: String,
    pub parcels: Vec<String>,
    pub size: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct SceneListResponse {
    pub scenes: Vec<WorldSceneEntry>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

const MAX_COORDINATES_PER_REQUEST: usize = 500;

/// GET /world/{world_name}/scenes
pub async fn list_scenes(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
    Query(q): Query<QueryPairs>,
) -> Result<Json<SceneListResponse>, ApiError> {
    scenes_page(&state, &world_name, &q, Vec::new()).await
}

/// POST /world/{world_name}/scenes: `{coordinates: [1..500 unique "x,y"]}`,
/// refused as `@dcl/schema-validator-component` 1.0.1 refuses a body.
pub async fn post_list_scenes(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
    Query(q): Query<QueryPairs>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_json_content_type(headers.get(header::CONTENT_TYPE)) {
        return validator_refusal(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            &json!({ "ok": false, "message": "Content-Type must be application/json" }),
        );
    }
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            let message = crate::content::validation::json_parse_message(&body, &e);
            return validator_refusal(
                StatusCode::BAD_REQUEST,
                &json!({ "ok": false, "message": message }),
            );
        }
    };
    let coordinates = match schema_coordinates(&parsed, &body) {
        Ok(c) => c,
        Err(error) => {
            return validator_refusal(
                StatusCode::BAD_REQUEST,
                &json!({ "ok": false, "message": "Invalid JSON body", "data": [error] }),
            );
        }
    };
    scenes_page(&state, &world_name, &q, coordinates)
        .await
        .into_response()
}

/// The validator's body, keys in its order (`ok, message, data`).
fn validator_refusal(status: StatusCode, body: &Value) -> Response {
    (
        status,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        crate::content::validation::ordered_refusal_json(body),
    )
        .into_response()
}

/// The validator's `isJsonContentType`: `application/json` or an RFC 6839
/// `type/subtype+json`, parameters ignored.
fn is_json_content_type(value: Option<&HeaderValue>) -> bool {
    let Some(raw) = value.and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let media = raw
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    media == "application/json"
        || media.split_once('/').is_some_and(|(kind, sub)| {
            !kind.is_empty()
                && sub.len() > "+json".len()
                && sub.ends_with("+json")
                && !sub.contains('/')
                && !media.contains(char::is_whitespace)
        })
}

const COORDINATE_PATTERN: &str = r"^-?\d+,-?\d+$";

/// `COORDINATE_PATTERN` as JavaScript reads it (`\d` is ASCII, `$` the end).
fn matches_coordinate_pattern(c: &str) -> bool {
    let number = |n: &str| {
        let digits = n.strip_prefix('-').unwrap_or(n);
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    };
    c.split_once(',')
        .is_some_and(|(x, y)| number(x) && number(y))
}

/// Upstream's `getWorldScenesSchema` checked as AJV 8 (`allErrors: false`)
/// checks it, the first error as AJV reports it; then canonicalized like
/// `canonicalizeParcels`. A coordinate past `i32` is kept as sent: it matches
/// no stored parcel.
fn schema_coordinates(body: &Value, bytes: &[u8]) -> Result<Vec<String>, Value> {
    let error = |instance: &str, schema: &str, keyword: &str, params: Value, message: String| {
        json!({
            "instancePath": instance,
            "schemaPath": format!("#/{schema}"),
            "keyword": keyword,
            "params": params,
            "message": message,
        })
    };
    let Some(object) = body.as_object() else {
        return Err(error(
            "",
            "type",
            "type",
            json!({ "type": "object" }),
            "must be object".into(),
        ));
    };
    let Some(coordinates) = object.get("coordinates") else {
        return Err(error(
            "",
            "required",
            "required",
            json!({ "missingProperty": "coordinates" }),
            "must have required property 'coordinates'".into(),
        ));
    };
    let keys = crate::content::validation::object_keys_in_order(body, bytes);
    if let Some(extra) = keys.iter().find(|k| k.as_str() != "coordinates") {
        return Err(error(
            "",
            "additionalProperties",
            "additionalProperties",
            json!({ "additionalProperty": extra }),
            "must NOT have additional properties".into(),
        ));
    }
    let property = "properties/coordinates";
    let Some(items) = coordinates.as_array() else {
        return Err(error(
            "/coordinates",
            &format!("{property}/type"),
            "type",
            json!({ "type": "array" }),
            "must be array".into(),
        ));
    };
    if items.len() > MAX_COORDINATES_PER_REQUEST {
        return Err(error(
            "/coordinates",
            &format!("{property}/maxItems"),
            "maxItems",
            json!({ "limit": MAX_COORDINATES_PER_REQUEST }),
            format!("must NOT have more than {MAX_COORDINATES_PER_REQUEST} items"),
        ));
    }
    if items.is_empty() {
        return Err(error(
            "/coordinates",
            &format!("{property}/minItems"),
            "minItems",
            json!({ "limit": 1 }),
            "must NOT have fewer than 1 items".into(),
        ));
    }
    let mut raw = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let instance = format!("/coordinates/{i}");
        let Some(c) = item.as_str() else {
            return Err(error(
                &instance,
                &format!("{property}/items/type"),
                "type",
                json!({ "type": "string" }),
                "must be string".into(),
            ));
        };
        if !matches_coordinate_pattern(c) {
            return Err(error(
                &instance,
                &format!("{property}/items/pattern"),
                "pattern",
                json!({ "pattern": COORDINATE_PATTERN }),
                format!("must match pattern \"{COORDINATE_PATTERN}\""),
            ));
        }
        raw.push(c);
    }
    // AJV walks from the end, keeping each item's last index.
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, c) in raw.iter().enumerate().rev() {
        if let Some(&j) = seen.get(c) {
            return Err(error(
                "/coordinates",
                &format!("{property}/uniqueItems"),
                "uniqueItems",
                json!({ "i": i, "j": j }),
                format!("must NOT have duplicate items (items ## {j} and {i} are identical)"),
            ));
        }
        seen.insert(c, i);
    }
    Ok(raw
        .iter()
        .map(|c| Parcel::parse_strict(c).map_or_else(|_| c.to_string(), |p| p.canonical()))
        .collect())
}

/// The bounding box as the old handler read it: the present, non-empty,
/// numeric values of `x1, x2, y1, y2`; some but not four is a 400.
fn bounding_box(q: &QueryPairs) -> Result<Option<(f64, f64, f64, f64)>, ApiError> {
    let values: Vec<f64> = ["x1", "x2", "y1", "y2"]
        .iter()
        .filter_map(|k| query_param(q, k))
        .filter(|v| !v.is_empty())
        .filter_map(js_number)
        .collect();
    match values.as_slice() {
        [] => Ok(None),
        [x1, x2, y1, y2] => Ok(Some((*x1, *x2, *y1, *y2))),
        _ => Err(ApiError::bad_request(
            "Bounding box requires all of x1, x2, y1, y2 to be provided.",
        )),
    }
}

/// JS `Number(v)`: decimal, `0x`/`0o`/`0b` integers and `Infinity`; `None` for `NaN`.
fn js_number(v: &str) -> Option<f64> {
    let t = v.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(digits) = t
            .get(..2)
            .filter(|p| p.eq_ignore_ascii_case(prefix))
            .map(|_| &t[2..])
        {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return None;
            }
            return Some(digits.chars().fold(0.0, |n, c| {
                n * radix as f64 + c.to_digit(radix).unwrap_or(0) as f64
            }));
        }
    }
    match t {
        "Infinity" | "+Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    if t.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// `IPFSv2.validate`: `^(ba)[a-zA-Z0-9]{57}$`.
fn is_cid_v1(id: &str) -> bool {
    id.len() == 59 && id.starts_with("ba") && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn filters(q: &QueryPairs, coordinates: Vec<String>) -> Result<SceneFilters, ApiError> {
    let bounding_box = bounding_box(q)?;
    let authorized_deployer = match query_param(q, "authorized_deployer") {
        Some(d) if !d.is_empty() => {
            let address = Some(d)
                .filter(|d| d.starts_with("0x"))
                .and_then(|d| Address::parse_untrimmed(d).ok())
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "Invalid authorized_deployer address: {d}. Must be a valid Ethereum address."
                    ))
                })?;
            Some(address.into_string())
        }
        _ => None,
    };
    let entity_id = match query_param(q, "entity_id") {
        Some(id) if !is_cid_v1(id) => {
            return Err(ApiError::bad_request(format!(
                "Invalid entity_id: {id}. Must be a valid IPFS CIDv1."
            )));
        }
        id => id.map(str::to_string),
    };
    Ok(SceneFilters {
        entity_id,
        coordinates,
        bounding_box,
        authorized_deployer,
    })
}

async fn scenes_page(
    state: &SharedState,
    world_name: &str,
    q: &QueryPairs,
    coordinates: Vec<String>,
) -> Result<Json<SceneListResponse>, ApiError> {
    let filters = filters(q, coordinates)?;
    let (limit, offset) = pagination(q);
    let internal = |e: crate::content::errors::AppError| ApiError::internal(e.to_string());
    let (mut rows, mut total) =
        queries::list_scenes_full(&state.pool, world_name, &filters, limit, offset)
            .await
            .map_err(internal)?;
    if total == 0 && filters.is_empty() && state.fill_world(world_name).await {
        (rows, total) = queries::list_scenes_full(&state.pool, world_name, &filters, limit, offset)
            .await
            .map_err(internal)?;
    }
    let millis =
        |t: chrono::NaiveDateTime| t.and_utc().to_rfc3339_opts(SecondsFormat::Millis, true);
    let scenes = rows
        .into_iter()
        .map(|r| WorldSceneEntry {
            world_name: r.world_name,
            deployer: r.deployer,
            deployment_auth_chain: r.deployment_auth_chain,
            entity: r.entity,
            entity_id: r.entity_id,
            parcels: r.parcels,
            size: r.size.to_string(),
            status: "DEPLOYED".to_string(),
            created_at: millis(r.created_at),
            updated_at: millis(r.updated_at),
        })
        .collect();
    Ok(Json(SceneListResponse { scenes, total }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pairs(raw: &[(&str, &str)]) -> QueryPairs {
        raw.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn query_numbers_follow_js_number() {
        for (raw, expected) in [
            ("12", Some(12.0)),
            (" -3 ", Some(-3.0)),
            ("1e2", Some(100.0)),
            ("0x10", Some(16.0)),
            ("0B101", Some(5.0)),
            ("0o17", Some(15.0)),
            ("Infinity", Some(f64::INFINITY)),
            ("-Infinity", Some(f64::NEG_INFINITY)),
            ("-0x10", None),
            ("0x", None),
            ("0xg", None),
            ("inf", None),
            ("abc", None),
        ] {
            assert_eq!(js_number(raw), expected, "{raw}");
        }
    }

    #[test]
    fn the_bounding_box_needs_all_four_numeric_values() {
        assert_eq!(bounding_box(&pairs(&[])).unwrap(), None);
        assert_eq!(
            bounding_box(&pairs(&[
                ("x1", "5"),
                ("x2", "-1"),
                ("y1", "0"),
                ("y2", "2")
            ]))
            .unwrap(),
            Some((5.0, -1.0, 0.0, 2.0))
        );
        // Empty and non-numeric values are dropped, as the old handler did.
        assert_eq!(
            bounding_box(&pairs(&[("x1", ""), ("y1", "abc")])).unwrap(),
            None
        );
        for partial in [
            pairs(&[("x1", "1")]),
            pairs(&[("x1", "abc"), ("x2", "1"), ("y1", "1"), ("y2", "1")]),
        ] {
            assert_eq!(
                bounding_box(&partial).unwrap_err().public_message(),
                "Bounding box requires all of x1, x2, y1, y2 to be provided."
            );
        }
    }

    #[test]
    fn query_filters_are_validated_like_the_old_handler() {
        let deployer = filters(&pairs(&[("authorized_deployer", "0xnope")]), vec![]);
        assert_eq!(
            deployer.unwrap_err().public_message(),
            "Invalid authorized_deployer address: 0xnope. Must be a valid Ethereum address."
        );
        let ok = filters(
            &pairs(&[(
                "authorized_deployer",
                "0xAbC0000000000000000000000000000000000001",
            )]),
            vec![],
        )
        .unwrap();
        assert_eq!(
            ok.authorized_deployer.as_deref(),
            Some("0xabc0000000000000000000000000000000000001")
        );
        for bad in ["", "Qm123", "bafy"] {
            assert_eq!(
                filters(&pairs(&[("entity_id", bad)]), vec![])
                    .unwrap_err()
                    .public_message(),
                format!("Invalid entity_id: {bad}. Must be a valid IPFS CIDv1.")
            );
        }
        let cid = format!("ba{}", "f".repeat(57));
        assert_eq!(
            filters(&pairs(&[("entity_id", &cid)]), vec![])
                .unwrap()
                .entity_id,
            Some(cid)
        );
    }

    fn check(body: &str) -> Result<Vec<String>, Value> {
        schema_coordinates(&serde_json::from_str(body).unwrap(), body.as_bytes())
    }

    #[test]
    fn the_post_body_matches_the_upstream_schema() {
        assert_eq!(
            check(r#"{"coordinates": ["00,01", "-2,3", "-0,4"]}"#).unwrap(),
            vec!["0,1".to_string(), "-2,3".into(), "0,4".into()]
        );
        assert_eq!(
            check(r#"{"coordinates": ["99999999999,0"]}"#).unwrap(),
            vec!["99999999999,0".to_string()],
            "the pattern takes any digit count; such a parcel matches no scene"
        );
        let many = |n: usize| {
            let items: Vec<String> = (0..n).map(|i| format!("\"{i},0\"")).collect();
            format!(r#"{{"coordinates": [{}]}}"#, items.join(","))
        };
        assert!(check(&many(500)).is_ok());
    }

    /// The first error AJV 8.17.1 reports for upstream's schema, as captured
    /// by compiling `getWorldScenesSchema` with AJV 8.17.1 under Node 22.
    #[test]
    fn schema_errors_are_the_first_ajv_error() {
        let p = r"^-?\d+,-?\d+$";
        let cases = [
            (
                r#"[]"#,
                json!({"instancePath":"","schemaPath":"#/type","keyword":"type","params":{"type":"object"},"message":"must be object"}),
            ),
            (
                r#"{"a":1}"#,
                json!({"instancePath":"","schemaPath":"#/required","keyword":"required","params":{"missingProperty":"coordinates"},"message":"must have required property 'coordinates'"}),
            ),
            (
                r#"{"z":1,"a":2,"coordinates":["0,0"]}"#,
                json!({"instancePath":"","schemaPath":"#/additionalProperties","keyword":"additionalProperties","params":{"additionalProperty":"z"},"message":"must NOT have additional properties"}),
            ),
            (
                r#"{"coordinates":"0,0"}"#,
                json!({"instancePath":"/coordinates","schemaPath":"#/properties/coordinates/type","keyword":"type","params":{"type":"array"},"message":"must be array"}),
            ),
            (
                r#"{"coordinates":[]}"#,
                json!({"instancePath":"/coordinates","schemaPath":"#/properties/coordinates/minItems","keyword":"minItems","params":{"limit":1},"message":"must NOT have fewer than 1 items"}),
            ),
            (
                r#"{"coordinates":["1,1","0,0","1,1","0,0"]}"#,
                json!({"instancePath":"/coordinates","schemaPath":"#/properties/coordinates/uniqueItems","keyword":"uniqueItems","params":{"i":1,"j":3},"message":"must NOT have duplicate items (items ## 3 and 1 are identical)"}),
            ),
            (
                r#"{"coordinates":[1,"0,0"]}"#,
                json!({"instancePath":"/coordinates/0","schemaPath":"#/properties/coordinates/items/type","keyword":"type","params":{"type":"string"},"message":"must be string"}),
            ),
            (
                r#"{"coordinates":["a","a"]}"#,
                json!({"instancePath":"/coordinates/0","schemaPath":"#/properties/coordinates/items/pattern","keyword":"pattern","params":{"pattern":p},"message":format!("must match pattern \"{p}\"")}),
            ),
            (
                r#"{"coordinates":["0,0","0, 0"]}"#,
                json!({"instancePath":"/coordinates/1","schemaPath":"#/properties/coordinates/items/pattern","keyword":"pattern","params":{"pattern":p},"message":format!("must match pattern \"{p}\"")}),
            ),
        ];
        for (body, want) in cases {
            assert_eq!(check(body).unwrap_err(), want, "{body}");
        }
        let too_many: Vec<String> = (0..501).map(|_| "\"a\"".to_string()).collect();
        let body = format!(r#"{{"coordinates":[{}]}}"#, too_many.join(","));
        assert_eq!(
            check(&body).unwrap_err()["keyword"],
            "maxItems",
            "the item count before the items"
        );
    }

    #[test]
    fn json_content_types_follow_the_validator() {
        let ct = |v: &str| is_json_content_type(Some(&HeaderValue::from_str(v).unwrap()));
        assert!(ct("application/json"));
        assert!(ct("Application/JSON; charset=utf-8"));
        assert!(ct("application/vnd.api+json"));
        assert!(ct("text/x+json"));
        assert!(!ct("application/+json"));
        assert!(!ct("text/plain"));
        assert!(!ct("application/json-patch"));
        assert!(!is_json_content_type(None));
    }
}
