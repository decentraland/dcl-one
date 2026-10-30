//! `GET /v1/contracts/{address}`: whitelisted or a mirrored collection
//! answers `{ok: true}`, a known non-member 404; while the collections
//! mirror has no complete pass on the relay's network the answer is the
//! unavailable envelope, never a 404 out of an empty table.

use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::evm::SharedState;
use crate::evm::contracts::Membership;
use crate::evm::errors::ApiError;
use crate::market::{Source, unavailable};

pub async fn contracts_address(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> Result<Response, ApiError> {
    let addr = address.to_lowercase();
    Ok(match state.contracts.membership(&addr).await? {
        Membership::Allowed => Json(json!({ "ok": true })).into_response(),
        Membership::Unknown => ApiError::NotFound("Address is not valid".into()).into_response(),
        Membership::MirrorUnavailable => unavailable(
            Source::CollectionsMirror,
            "the collections mirror has no complete pass yet, so contract membership cannot be decided",
        ),
    })
}
