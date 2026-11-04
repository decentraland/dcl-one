//! `GET /world/{world_name}/limits`: the parcel and size caps a publish to
//! the world is checked against, from the same `Policy::world_limits` the
//! deploy validator reads. `parcels` (repeatable) is checked the way a
//! deployment's pointers are; it names the scenes a publish would replace,
//! which no per-wallet allowance credits back yet, so it does not change
//! the answer.

use axum::Json;
use axum::extract::{Path, RawQuery, State};
use serde::Serialize;
use serde_json::Value;

use super::SharedState;
use super::deploy_validate::{is_canonical_parcel_set, validate_parcel_in_bounds};
use super::http::ApiError;
use super::permissions::parse_world;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldLimitsResponse {
    /// A decimal string, as the old service's bigint.
    pub max_scene_size_in_bytes: String,
    pub max_parcels: u32,
}

/// The repeated `parcels` values; `None` when the query has none.
fn requested_parcels(query: Option<&str>) -> Option<Vec<String>> {
    let parcels: Vec<String> = url::form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .filter(|(key, _)| key == "parcels")
        .map(|(_, value)| value.into_owned())
        .collect();
    (!parcels.is_empty()).then_some(parcels)
}

fn check_parcels(parcels: &[String]) -> Result<(), ApiError> {
    let values: Vec<Value> = parcels.iter().cloned().map(Value::String).collect();
    if !is_canonical_parcel_set(&values) {
        return Err(ApiError::bad_request(
            "Parcels must be unique canonical \"x,y\" coordinates, as in a scene's parcels.",
        ));
    }
    for parcel in parcels {
        validate_parcel_in_bounds(parcel).map_err(ApiError::bad_request)?;
    }
    Ok(())
}

pub async fn get_world_limits(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Json<WorldLimitsResponse>, ApiError> {
    let world = parse_world(&world_name)?;
    if let Some(parcels) = requested_parcels(query.as_deref()) {
        check_parcels(&parcels)?;
    }
    if !state
        .names
        .check_name_deny_list(&state.pool, world.key())
        .await
    {
        return Err(ApiError::not_found(format!(
            "World \"{}\" not found.",
            world.key()
        )));
    }
    let limits = state.policy.world_limits(&world).await?;
    if !limits.size_overridden && state.policy.limits_owner(&world).await?.is_none() {
        return Err(ApiError::not_found(format!(
            "Could not determine owner for world {}",
            world.key()
        )));
    }
    Ok(Json(WorldLimitsResponse {
        max_scene_size_in_bytes: limits.max_size_bytes.to_string(),
        max_parcels: limits.max_parcels,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parcels_come_from_every_repeated_key() {
        assert_eq!(requested_parcels(None), None);
        assert_eq!(requested_parcels(Some("other=1")), None);
        assert_eq!(
            requested_parcels(Some("parcels=0%2C0&parcels=1,0")),
            Some(vec!["0,0".to_string(), "1,0".to_string()])
        );
        assert_eq!(
            requested_parcels(Some("parcels")),
            Some(vec![String::new()])
        );
    }

    #[test]
    fn parcels_are_checked_like_a_deployments_pointers() {
        let ok = |p: &[&str]| check_parcels(&p.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(ok(&["0,0", "0,1"]).is_ok());
        for bad in [&["0,0", "0,0"][..], &["00,1"], &[""], &[" 0,0"]] {
            assert_eq!(ok(bad).unwrap_err().status(), 400, "{bad:?}");
        }
        let out = ok(&["151,0"]).unwrap_err();
        assert!(out.public_message().contains("out of bounds"), "{out:?}");
    }
}
