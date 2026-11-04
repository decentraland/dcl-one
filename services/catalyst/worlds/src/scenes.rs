//! `GET /world/{world_name}/scenes` (browse, with the bounding-box, deployer
//! and entity filters) and `POST /world/{world_name}/scenes` (the same with
//! `{coordinates}` in the body), paged like @dcl/http-commons and answering
//! in the worlds error envelope.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use chrono::SecondsFormat;
use serde::Serialize;
use serde_json::Value;

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

/// POST /world/{world_name}/scenes: `{coordinates: [1..500 unique "x,y"]}`.
pub async fn post_list_scenes(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
    Query(q): Query<QueryPairs>,
    body: Result<Json<Value>, JsonRejection>,
) -> Result<Json<SceneListResponse>, ApiError> {
    let Json(body) = body.map_err(|rejection| match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            ApiError::http(415, "Content-Type must be application/json")
        }
        other => ApiError::bad_request(other.body_text()),
    })?;
    let coordinates = schema_coordinates(&body)?;
    scenes_page(&state, &world_name, &q, coordinates).await
}

/// Upstream's `getWorldScenesSchema`, canonicalized like `canonicalizeParcels`.
fn schema_coordinates(body: &Value) -> Result<Vec<String>, ApiError> {
    let invalid = || ApiError::bad_request("Invalid JSON body");
    let items = body
        .as_object()
        .filter(|o| o.len() == 1)
        .and_then(|o| o.get("coordinates"))
        .and_then(Value::as_array)
        .filter(|a| (1..=MAX_COORDINATES_PER_REQUEST).contains(&a.len()))
        .ok_or_else(invalid)?;
    let raw = items
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let unique: std::collections::HashSet<&str> = raw.iter().copied().collect();
    if unique.len() != raw.len() {
        return Err(invalid());
    }
    raw.iter()
        .map(|c| Parcel::parse_strict(c).map(|p| p.canonical()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())
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

    #[test]
    fn the_post_body_matches_the_upstream_schema() {
        assert_eq!(
            schema_coordinates(&json!({ "coordinates": ["00,01", "-2,3"] })).unwrap(),
            vec!["0,1".to_string(), "-2,3".into()]
        );
        let too_many: Vec<String> = (0..501).map(|i| format!("{i},0")).collect();
        assert!(schema_coordinates(&json!({ "coordinates": too_many[..500] })).is_ok());
        for bad in [
            json!({}),
            json!({ "coordinates": [] }),
            json!({ "coordinates": too_many }),
            json!({ "coordinates": ["0,0", "0,0"] }),
            json!({ "coordinates": ["0, 0"] }),
            json!({ "coordinates": [1] }),
            json!({ "coordinates": ["0,0"], "extra": 1 }),
        ] {
            assert_eq!(
                schema_coordinates(&bad).unwrap_err().public_message(),
                "Invalid JSON body",
                "{bad}"
            );
        }
    }
}
