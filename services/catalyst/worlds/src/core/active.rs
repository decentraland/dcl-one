//! `POST /entities/active` over worlds: the newest scene entity of each
//! named world (the pointers are world names here), minus the DAO deny list.

use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use bytes::Bytes;
use serde_json::Value;

use crate::comms::world_identity::WorldName;

use super::SharedState;
use super::http::ApiError;
use super::store;

const MAX_POINTERS: usize = 50;

pub async fn active_entities(
    State(state): State<SharedState>,
    body: Bytes,
) -> Result<Json<Vec<Value>>, ApiError> {
    let parsed: Value = serde_json::from_slice(&body)
        .map_err(|_| ApiError::bad_request("Invalid request. Request body is not valid"))?;
    let Some(Value::Array(items)) = parsed.get("pointers") else {
        return Err(ApiError::bad_request(
            "Invalid request. Request body is not valid",
        ));
    };

    let pointers: Vec<&str> = items
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .collect();
    if pointers.len() > MAX_POINTERS {
        return Err(ApiError::bad_request(format!(
            "Maximum {MAX_POINTERS} pointers allowed per request"
        )));
    }

    let banned = state.names.snapshot(&state.pool).await;
    let mut seen = HashSet::new();
    let worlds: Vec<WorldName> = pointers
        .into_iter()
        .filter_map(|p| WorldName::parse(p).ok())
        .filter(|w| seen.insert(w.key().to_string()))
        .filter(|w| banned.allows(w.display()))
        .collect();

    Ok(Json(
        store::entities_for_worlds(&state.pool, &worlds).await?,
    ))
}
