//! `GET /snapshots` lists the generated time-windowed snapshots peers sync
//! from. `GET /snapshot` and `GET /snapshot/{type}` are the legacy upstream
//! catalyst <= 3.x shapes (`content/src/controller/Controller.ts`
//! `getAllSnapshots` / `getSnapshot`) that `node-fetch/1.0` era
//! `dcl-catalyst-client`s still call:
//!   SnapshotMetadata     = { hash, lastIncludedDeploymentTimestamp }
//!   FullSnapshotMetadata = SnapshotMetadata & { entities: Record<string, SnapshotMetadata> }
//! Errors: 400 `{ error: "Unrecognized type: <type>" }`, 503 `{ error: "Snapshot not yet created" }`.
//!
//! Only time-windowed snapshots are generated, never one per entity type, so
//! the global descriptor is the newest window (greatest `endTimestamp`,
//! widest on ties) and every entity type answers with that same snapshot.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Serialize;

use super::SharedState;
use super::errors::{AppError, AppResult, json_error};
use super::queries;
use super::query_params::{ENTITY_TYPES, parse_entity_type};
use super::wire_types::SnapshotMetadata;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacySnapshotMetadata {
    pub hash: String,
    pub last_included_deployment_timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyFullSnapshotMetadata {
    pub hash: String,
    pub last_included_deployment_timestamp: i64,
    pub entities: BTreeMap<&'static str, LegacySnapshotMetadata>,
}

pub fn newest_snapshot(snapshots: &[SnapshotMetadata]) -> Option<LegacySnapshotMetadata> {
    snapshots
        .iter()
        .filter(|s| !s.hash.is_empty())
        .max_by_key(|s| {
            (
                s.time_range.end_timestamp,
                std::cmp::Reverse(s.time_range.init_timestamp),
            )
        })
        .map(|s| LegacySnapshotMetadata {
            hash: s.hash.clone(),
            last_included_deployment_timestamp: s.time_range.end_timestamp,
        })
}

/// `None` until the first snapshot exists; the list is memoized with the
/// history routes because peers poll it.
async fn current_snapshots(state: &SharedState) -> AppResult<Option<Vec<SnapshotMetadata>>> {
    let bytes = state
        .deployments_cache
        .get_or_fetch("snapshots".to_string(), || async {
            let snapshots = queries::all_snapshots(&state.pool).await?;
            serde_json::to_vec(&snapshots)
                .map(Bytes::from)
                .map_err(|e| AppError::Internal(e.to_string()))
        })
        .await?;
    let snapshots: Vec<SnapshotMetadata> =
        serde_json::from_slice(&bytes).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok((!snapshots.is_empty()).then_some(snapshots))
}

pub async fn get_snapshots(State(state): State<SharedState>) -> AppResult<Response> {
    Ok(match current_snapshots(&state).await? {
        Some(metadata) => (StatusCode::OK, Json(metadata)).into_response(),
        None => json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "New Snapshots not yet created",
        ),
    })
}

fn not_yet_created() -> Response {
    json_error(StatusCode::SERVICE_UNAVAILABLE, "Snapshot not yet created")
}

async fn current(state: &SharedState) -> AppResult<Option<LegacySnapshotMetadata>> {
    Ok(current_snapshots(state)
        .await?
        .and_then(|s| newest_snapshot(&s)))
}

pub async fn get_snapshot(State(state): State<SharedState>) -> AppResult<Response> {
    let Some(newest) = current(&state).await? else {
        return Ok(not_yet_created());
    };
    let entities = ENTITY_TYPES.iter().map(|t| (*t, newest.clone())).collect();
    let body = LegacyFullSnapshotMetadata {
        hash: newest.hash,
        last_included_deployment_timestamp: newest.last_included_deployment_timestamp,
        entities,
    };
    Ok((StatusCode::OK, Json(body)).into_response())
}

pub async fn get_snapshot_by_type(
    State(state): State<SharedState>,
    Path(entity_type): Path<String>,
) -> AppResult<Response> {
    if parse_entity_type(&entity_type).is_none() {
        return Ok(json_error(
            StatusCode::BAD_REQUEST,
            &format!("Unrecognized type: {entity_type}"),
        ));
    }
    Ok(match current(&state).await? {
        Some(newest) => (StatusCode::OK, Json(newest)).into_response(),
        None => not_yet_created(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::wire_types::TimeRange;
    use super::*;

    fn snap(hash: &str, init: i64, end: i64) -> SnapshotMetadata {
        SnapshotMetadata {
            hash: hash.to_string(),
            time_range: TimeRange {
                init_timestamp: init,
                end_timestamp: end,
            },
            number_of_entities: 1,
            replaced_snapshot_hashes: None,
            generation_timestamp: end + 1,
        }
    }

    pub(crate) fn windows() -> Vec<SnapshotMetadata> {
        vec![
            snap("bafyyear", 0, 1_000),
            snap("bafyday", 1_900, 2_000),
            snap("bafymonth", 1_000, 2_000),
            snap("bafyweek", 1_500, 1_900),
        ]
    }

    #[test]
    fn newest_prefers_latest_end_then_widest_window() {
        let newest = newest_snapshot(&windows()).unwrap();
        assert_eq!(newest.hash, "bafymonth");
        assert_eq!(newest.last_included_deployment_timestamp, 2_000);
        assert!(newest_snapshot(&[]).is_none());
        assert!(newest_snapshot(&[snap("", 0, 10)]).is_none());
    }

    #[test]
    fn full_shape_lists_every_entity_type() {
        let newest = newest_snapshot(&windows()).unwrap();
        let entities: BTreeMap<&'static str, LegacySnapshotMetadata> =
            ENTITY_TYPES.iter().map(|t| (*t, newest.clone())).collect();
        let body = serde_json::to_value(LegacyFullSnapshotMetadata {
            hash: newest.hash,
            last_included_deployment_timestamp: newest.last_included_deployment_timestamp,
            entities,
        })
        .unwrap();
        assert_eq!(body.as_object().unwrap().len(), 3);
        for t in ENTITY_TYPES {
            assert_eq!(body["entities"][*t]["hash"], "bafymonth", "entity type {t}");
            assert_eq!(
                body["entities"][*t]["lastIncludedDeploymentTimestamp"],
                2_000
            );
        }
    }
}
