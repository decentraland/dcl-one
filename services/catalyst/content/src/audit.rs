use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::queries;
use super::query_params::parse_entity_type;

/// GET /audit/{type}/{entityId}: the type is canonicalized (`scenes` ->
/// `scene`) and validated before the lookup, and a deployment of another
/// type is `404` like an unknown id.
pub async fn get_audit(
    State(state): State<SharedState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    let Some(canonical) = parse_entity_type(&entity_type) else {
        return Err(AppError::invalid(format!(
            "Unrecognized type: {entity_type}"
        )));
    };

    let audit_info = queries::audit_info(&state.pool, canonical, &entity_id)
        .await?
        .ok_or_else(|| AppError::not_found("No deployment found"))?;

    Ok(Json(audit_info))
}
