use axum::Json;
use axum::extract::{Request, State};
use axum::response::IntoResponse;

use super::SharedState;
use super::errors::AppResult;
use super::queries;
use super::query_params::{parse_query_string, qs_get_number};

const MAX_FAILED_DEPLOYMENTS_PAGE_SIZE: i64 = 1000;

/// Without `offset`/`limit` the whole table comes back (the legacy answer);
/// with either, a page of at most 1000.
pub async fn get_failed_deployments(
    State(state): State<SharedState>,
    request: Request,
) -> AppResult<impl IntoResponse> {
    let params = parse_query_string(request.uri().query().unwrap_or(""));
    let offset = qs_get_number(&params, "offset");
    let limit = qs_get_number(&params, "limit");

    let (safe_offset, safe_limit) = if offset.is_none() && limit.is_none() {
        (0, None)
    } else {
        let safe_offset = offset.filter(|&o| o > 0).unwrap_or(0);
        let safe_limit = match limit {
            Some(l) => l.clamp(0, MAX_FAILED_DEPLOYMENTS_PAGE_SIZE),
            None => MAX_FAILED_DEPLOYMENTS_PAGE_SIZE,
        };
        (safe_offset, Some(safe_limit))
    };

    let failed = queries::failed_deployments(&state.pool, safe_offset, safe_limit).await?;
    Ok(Json(failed))
}
