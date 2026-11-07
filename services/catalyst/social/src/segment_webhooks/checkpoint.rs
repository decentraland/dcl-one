//! `POST /forward-onboarding-checkpoint` (`forward-onboarding-checkpoint.ts`):
//! `properties.checkpointId` read with JavaScript's `Number` must be 1..7;
//! the properties are lifted as the auth server's checkpoint body and
//! recorded, a refusal being a warning (the notifier's fire-and-forget).

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Map, Value, json};

use super::{SharedState, gate, refused, reply};
use crate::explore::api::onboarding::CheckpointRequest;

const LIFTED: [&str; 7] = [
    "action",
    "userIdentifier",
    "identifierType",
    "email",
    "wallet",
    "source",
    "metadata",
];

pub(super) async fn forward(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = gate(&state, &headers, &body) {
        return response;
    }
    let properties = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|body| body.get("properties").cloned())
        .unwrap_or(Value::Null);
    let checkpoint_id = js_number(properties.get("checkpointId"));
    if checkpoint_id.is_nan() || !(1.0..=7.0).contains(&checkpoint_id) {
        tracing::warn!(
            checkpoint_id,
            "Invalid or missing checkpointId in event properties"
        );
        return refused(StatusCode::BAD_REQUEST, "Invalid checkpointId");
    }
    let mut lifted = Map::new();
    lifted.insert(
        "checkpointId".into(),
        if checkpoint_id.fract() == 0.0 {
            json!(checkpoint_id as i64)
        } else {
            json!(checkpoint_id)
        },
    );
    for key in LIFTED {
        if let Some(value) = properties.get(key) {
            lifted.insert(key.into(), value.clone());
        }
    }
    match serde_json::from_value::<CheckpointRequest>(Value::Object(lifted)) {
        Ok(request) => {
            if let Err(error) = state.onboarding.record_checkpoint(request) {
                tracing::warn!(
                    error,
                    checkpoint_id,
                    "Segment checkpoint refused by the onboarding store"
                );
            }
        }
        Err(error) => {
            tracing::warn!(%error, checkpoint_id, "Segment checkpoint is not a checkpoint body");
        }
    }
    reply(StatusCode::OK, json!({ "ok": true }))
}

/// JavaScript's `Number(value)` for a JSON value (`undefined` and objects are `NaN`).
fn js_number(value: Option<&Value>) -> f64 {
    match value {
        None | Some(Value::Object(_)) => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse().unwrap_or(f64::NAN)
            }
        }
        Some(Value::Array(items)) => match items.as_slice() {
            [] => 0.0,
            [one] => js_number(Some(one)),
            _ => f64::NAN,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::js_number;
    use serde_json::json;

    #[test]
    fn number_semantics_match_javascript_for_the_checkpoint_id() {
        assert!(js_number(None).is_nan());
        assert_eq!(js_number(Some(&json!(null))), 0.0);
        assert_eq!(js_number(Some(&json!(3))), 3.0);
        assert_eq!(js_number(Some(&json!("3"))), 3.0);
        assert_eq!(js_number(Some(&json!(" 7 "))), 7.0);
        assert_eq!(js_number(Some(&json!(""))), 0.0);
        assert!(js_number(Some(&json!("three"))).is_nan());
        assert!(js_number(Some(&json!({}))).is_nan());
        assert_eq!(js_number(Some(&json!(true))), 1.0);
    }
}
