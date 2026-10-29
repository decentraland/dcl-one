use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::queries;
use super::query_params::parse_deployable_entity_type;

/// GET /audit/{type}/{entityId}: the type is canonicalized (`scenes` ->
/// `scene`) and validated before the lookup; outfits are not deployable
/// through /content and are rejected like any unknown type.
pub async fn get_audit(
    State(state): State<SharedState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    if parse_deployable_entity_type(&entity_type).is_none() {
        return Err(AppError::invalid(format!(
            "Unrecognized type: {entity_type}"
        )));
    }

    let audit_info = queries::audit_info(&state.pool, &entity_id)
        .await?
        .ok_or_else(|| AppError::not_found("No deployment found"))?;

    Ok(Json(audit_info))
}
