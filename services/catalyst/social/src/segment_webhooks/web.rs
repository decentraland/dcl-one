//! `POST /forward-web`: the web's Segment events (`forward-web-events.ts`,
//! `event-parser.ts:316-330,452-474,496-556`). Only an `identify` of the
//! configured project is an event (`auth-identify`); with a wallet in
//! `traits.ethAddress` it is stored, then its anonymous id is checked against
//! the denylist and the answer carries `validationPassed`.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::store::{ClientEventRow, insert_client_event};
use super::{Settings, SharedState, gate, object, parse_time, refused, reply, text};
use crate::auth::is_eth_address;

pub(super) async fn forward(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = gate(&state, &headers, &body) {
        return response;
    }
    // `!body || typeof body !== 'object'`: an array passes, a scalar does not.
    let event = match serde_json::from_slice::<Value>(&body) {
        Ok(event) if event.is_object() || event.is_array() => event,
        _ => {
            tracing::warn!("Segment web event: invalid structure");
            return refused(StatusCode::BAD_REQUEST, "Invalid web event structure");
        }
    };
    let Some(parsed) = parse_web_event(&event, &state.settings) else {
        tracing::warn!(body = %event, "Segment web event: invalid event");
        return refused(StatusCode::BAD_REQUEST, "Invalid web event");
    };
    let mut validation_passed = false;
    if let Some(row) = &parsed.row {
        if is_eth_address(&row.address) {
            if let Err(error) = insert_client_event(&state.pool, row).await {
                tracing::error!(%error, key = row.key, "Segment web event: insert failed");
                return refused(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
            }
        } else {
            tracing::warn!(
                address = row.address,
                key = row.key,
                "Segment web event: wallet is not an address, no row"
            );
        }
        validation_passed = validate_web_context(&event, &state.settings);
        if !validation_passed {
            tracing::warn!(
                address = row.address,
                key = row.key,
                "Web event validation failed. User flagged"
            );
        }
        metrics::counter!("handled_web_events_count", "event_type" => parsed.event_type.clone())
            .increment(1);
    }
    reply(
        StatusCode::OK,
        json!({ "ok": true, "validationPassed": u8::from(validation_passed) }),
    )
}

struct Parsed {
    /// The `auth-identify` row; `None` when the identify carries no wallet.
    row: Option<ClientEventRow>,
    event_type: String,
}

/// `parseWebEvent`: `None` is the 400.
fn parse_web_event(event: &Value, settings: &Settings) -> Option<Parsed> {
    event.get("event").filter(|e| truthy(e))?;
    let event_type = text(event, "type")?;
    if event_type != "identify" || text(event, "projectId") != Some(&settings.web_project_id) {
        return None;
    }
    let context = event.get("context")?;
    let page = context.get("page")?;
    page.as_object()?;
    let key = event.get("messageId")?.as_str()?.to_string();
    let received_at = parse_time(event.get("receivedAt"))?;
    let reported_at = parse_time(event.get("timestamp"))?;
    let wallet = event
        .get("traits")
        .and_then(|traits| text(traits, "ethAddress"));
    let row = wallet.map(|wallet| ClientEventRow {
        key,
        source: "web",
        sub_type: "auth-identify",
        address: wallet.to_lowercase(),
        session_id: None,
        anonymous_id: text(event, "anonymousId").map(str::to_string),
        realm: None,
        ip: text(context, "ip").map(str::to_string),
        reported_at,
        received_at,
        metadata: object(&[
            ("userAgent", context.get("userAgent")),
            ("pagePath", page.get("path")),
            ("pageUrl", page.get("url")),
            ("locale", context.get("locale")),
            ("userId", event.get("userId")),
        ]),
    });
    Some(Parsed {
        row,
        event_type: event_type.to_string(),
    })
}

/// `validateWebContext` without the cluster bookkeeping: a denied anonymous id fails.
fn validate_web_context(event: &Value, settings: &Settings) -> bool {
    !text(event, "anonymousId")
        .is_some_and(|id| settings.anon_id_denylist.contains(&id.to_lowercase()))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
