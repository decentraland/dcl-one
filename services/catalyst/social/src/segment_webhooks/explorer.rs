//! `POST /forward`: the explorer's Segment events
//! (`forward-explorer-events.ts`, `event-parser.ts:181-450`). In order: the
//! signature; the checkpoint side effect (`otp_verification_success` records
//! CP7 reached by email and ends the request; `logged_in`/`logged_in_cached`
//! record CP7 completed by wallet and continue); the parse, whose context
//! checks are soft (logged) unless `FORCE_EXPLORER_CONTEXT_VALIDATION` and
//! whose denylist refusal is not; the auth chain; the row; the metrics.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::store::{ClientEventRow, insert_client_event};
use super::{Settings, SharedState, gate, object, parse_time, refused, reply, text};
use crate::auth::{AuthChain, AuthLinkType, is_eth_address, parse_ephemeral_payload};
use crate::explore::api::onboarding::{CheckpointRequest, OnboardingState};

const OTP_VERIFICATION_SUCCESS: &str = "otp_verification_success";
const LOGGED_IN: &str = "logged_in";
const LOGGED_IN_CACHED: &str = "logged_in_cached";

/// The explorer event ids and the stored sub-types (`Events.SubType.Client`).
const SUB_TYPES: [(&str, &str); 7] = [
    ("move_to_parcel", "move-to-parcel"),
    ("used_emote", "used-emote"),
    ("passport_opened", "passport-opened"),
    ("walked_distance", "walked-distance"),
    ("vertical_height_reached", "vertical-height-reached"),
    (LOGGED_IN, "logged-in"),
    (LOGGED_IN_CACHED, "logged-in-cached"),
];

pub(super) async fn forward(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = gate(&state, &headers, &body) {
        return response;
    }
    let Ok(event) = serde_json::from_slice::<Value>(&body) else {
        return refused(StatusCode::BAD_REQUEST, "Invalid event");
    };
    if try_forward_checkpoint(&event, &state.onboarding) {
        return reply(StatusCode::OK, json!({ "ok": true }));
    }
    let Some(parsed) = parse_client_event(&event, &state.settings) else {
        tracing::warn!(body = %event, "Segment explorer event: invalid event");
        return refused(StatusCode::BAD_REQUEST, "Invalid event");
    };
    if !auth_chain_is_valid(parsed.auth_chain.as_ref(), &parsed.row.address) {
        tracing::warn!(
            address = parsed.row.address,
            key = parsed.row.key,
            "Segment explorer event: invalid auth chain"
        );
        return refused(StatusCode::UNAUTHORIZED, "Invalid AuthChain");
    }
    if let Err(error) = insert_client_event(&state.pool, &parsed.row).await {
        tracing::error!(%error, key = parsed.row.key, "Segment explorer event: insert failed");
        return refused(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
    }
    let row = &parsed.row;
    if row.reported_at < row.received_at {
        let sub_type = row.sub_type;
        let explorer_to_segment =
            (row.received_at - row.reported_at).num_milliseconds() as f64 / 1000.0;
        let segment_to_webhook =
            (chrono::Utc::now() - row.received_at).num_milliseconds() as f64 / 1000.0;
        metrics::counter!("handled_explorer_events_count", "event_type" => sub_type).increment(1);
        metrics::histogram!("explorer_segment_event_delay_in_seconds", "event_type" => sub_type)
            .record(explorer_to_segment);
        metrics::histogram!("segment_webhook_event_delay_in_seconds", "event_type" => sub_type)
            .record(segment_to_webhook);
    }
    reply(StatusCode::OK, json!({ "ok": true }))
}

/// The CP7 side effect (`forward-explorer-events.ts:63-136`); `true` when
/// the event is handled by it alone.
fn try_forward_checkpoint(event: &Value, onboarding: &OnboardingState) -> bool {
    let name = event
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    if ![OTP_VERIFICATION_SUCCESS, LOGGED_IN, LOGGED_IN_CACHED].contains(&name.as_str()) {
        return false;
    }
    let context = event.get("context").unwrap_or(&Value::Null);
    let properties = event.get("properties").unwrap_or(&Value::Null);
    let name_value = Value::String(name.clone());
    if name == OTP_VERIFICATION_SUCCESS {
        let Some(email) = text(properties, "email") else {
            tracing::warn!("otp_verification_success missing email property");
            return true;
        };
        record(
            onboarding,
            CheckpointRequest {
                checkpoint_id: 7,
                action: "reached".into(),
                user_identifier: email.into(),
                identifier_type: "email".into(),
                email: Some(email.into()),
                wallet: None,
                source: Some("explorer".into()),
                metadata: Some(object(&[
                    ("explorerEvent", Some(&name_value)),
                    ("sessionId", context.get("session_id")),
                    ("launcherAnonymousId", context.get("launcher_anonymous_id")),
                ])),
            },
        );
        return true;
    }
    let Some(wallet) = text(context, "dcl_eth_address") else {
        return false;
    };
    let wallet = wallet.to_lowercase();
    record(
        onboarding,
        CheckpointRequest {
            checkpoint_id: 7,
            action: "completed".into(),
            user_identifier: wallet.clone(),
            identifier_type: "wallet".into(),
            email: None,
            wallet: Some(wallet),
            source: Some("explorer".into()),
            metadata: Some(object(&[
                ("explorerEvent", Some(&name_value)),
                ("loginMethod", properties.get("method")),
                ("sessionId", context.get("session_id")),
                ("launcherAnonymousId", context.get("launcher_anonymous_id")),
            ])),
        },
    );
    false
}

/// Fire-and-forget as the notifier's auth-server call: a refusal is a warning.
fn record(onboarding: &OnboardingState, request: CheckpointRequest) {
    if let Err(error) = onboarding.record_checkpoint(request) {
        tracing::warn!(error, "Segment checkpoint refused by the onboarding store");
    }
}

struct Parsed {
    row: ClientEventRow,
    /// `None` when `context.auth_chain` parses as JSON but not as a chain.
    auth_chain: Option<AuthChain>,
}

/// `parseExplorerClientEvent` (`event-parser.ts:333-450`): `None` is the 400.
fn parse_client_event(event: &Value, settings: &Settings) -> Option<Parsed> {
    let name = text(event, "event")?;
    let context = event.get("context")?;
    context.as_object()?;
    let address = text(context, "dcl_eth_address").filter(|a| *a != "NOT CONFIGURED")?;
    if !validate_context(event, context, address, settings) {
        tracing::warn!(key = ?event.get("messageId"), "Invalid explorer context, discarding event");
        return None;
    }
    let (_, sub_type) = SUB_TYPES
        .iter()
        .find(|(id, _)| name.eq_ignore_ascii_case(id))?;
    let chain_json: Value = serde_json::from_str(context.get("auth_chain")?.as_str()?).ok()?;
    let auth_chain = serde_json::from_value::<AuthChain>(chain_json).ok();
    let received_at = parse_time(event.get("receivedAt"))?;
    let reported_at = parse_time(event.get("timestamp"))?;
    let key = event.get("messageId")?.as_str()?.to_string();
    let properties = event.get("properties");
    let metadata = match *sub_type {
        "move-to-parcel" => {
            let p = properties?;
            json!({ "parcel": object(&[
                ("isEmptyParcel", p.get("is_empty_parcel")),
                ("newParcel", p.get("new_parcel")),
                ("oldParcel", p.get("old_parcel")),
                ("sceneHash", p.get("scene_hash")),
            ]) })
        }
        "used-emote" => {
            let p = properties?;
            json!({ "emote": object(&[
                ("emoteIndex", p.get("emote_index")),
                ("isBase", p.get("is_base")),
                ("itemId", p.get("item_id")),
                ("source", p.get("source")),
            ]) })
        }
        "passport-opened" => {
            json!({ "passport": object(&[("receiver", properties?.get("receiver_id"))]) })
        }
        "walked-distance" => {
            let p = properties?;
            object(&[
                ("distance", p.get("distance")),
                ("stepCount", p.get("step_count")),
            ])
        }
        "vertical-height-reached" => object(&[("height", properties?.get("height"))]),
        _ => object(&[("contextRuntime", context.get("runtime"))]),
    };
    let string = |key: &str| text(context, key).map(str::to_string);
    Some(Parsed {
        row: ClientEventRow {
            key,
            source: "client",
            sub_type,
            address: address.to_lowercase(),
            session_id: string("session_id"),
            anonymous_id: string("launcher_anonymous_id"),
            realm: string("realm"),
            ip: string("ip"),
            reported_at,
            received_at,
            metadata,
        },
        auth_chain,
    })
}

/// `validateExplorerContext` (`event-parser.ts:181-299`) without the cluster
/// bookkeeping: the denylist refuses, every other check refuses only when forced.
fn validate_context(event: &Value, context: &Value, address: &str, settings: &Settings) -> bool {
    let soft = !settings.force_context_validation;
    let denied = [
        event.get("anonymousId"),
        context.get("launcher_anonymous_id"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .any(|id| settings.anon_id_denylist.contains(&id.to_lowercase()));
    if denied {
        tracing::warn!(key = ?event.get("messageId"), "Denied client ID");
        return false;
    }
    if text(context, "dcl_renderer_type") != Some("Desktop") {
        tracing::warn!(renderer = ?context.get("dcl_renderer_type"), "Invalid renderer type");
        return soft;
    }
    match event.get("userId") {
        None | Some(Value::Null) => {}
        Some(Value::String(user_id))
            if user_id.is_empty() || user_id.eq_ignore_ascii_case(address) => {}
        other => {
            tracing::warn!(user_id = ?other, address, "User ID mismatch with eth address");
            return soft;
        }
    }
    if text(event, "type") != Some("track") {
        tracing::warn!(kind = ?event.get("type"), "Invalid event type");
        return soft;
    }
    if !is_eth_address(address) {
        tracing::warn!(address, "Invalid Ethereum address format");
        return soft;
    }
    if !chain_shape_is_valid(context, address) {
        tracing::warn!(address, "Invalid auth_chain structure or signer");
        return soft;
    }
    if let Some(expected) = settings.expected_realm.as_deref() {
        let realm = text(context, "realm").map(str::to_lowercase);
        let url_matches = settings
            .expected_realm_url
            .as_deref()
            .is_none_or(|url| text(context, "realm_url") == Some(url));
        if realm.as_deref() != Some(&expected.to_lowercase()) || !url_matches {
            tracing::warn!(realm = ?context.get("realm"), "Unknown realm");
            return soft;
        }
    }
    true
}

/// The notifier's shape check: a JSON array of two or more links, `[0]` the
/// SIGNER of `address`, `[1]` an ECDSA_EPHEMERAL link with a `0x` signature.
fn chain_shape_is_valid(context: &Value, address: &str) -> bool {
    let Some(chain) = context
        .get("auth_chain")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
    else {
        return false;
    };
    let Some(links) = chain.as_array().filter(|links| links.len() >= 2) else {
        return false;
    };
    text(&links[0], "type") == Some("SIGNER")
        && text(&links[0], "payload").is_some_and(|p| p.eq_ignore_ascii_case(address))
        && text(&links[1], "type") == Some("ECDSA_EPHEMERAL")
        && text(&links[1], "signature").is_some_and(|s| s.starts_with("0x"))
}

/// The notifier's `validateAuthChain` (a valid chain owned by `address`)
/// plus the signatures and expiry: the chain's last link must be the
/// ephemeral delegation (or signed entity) the owner's signatures reach.
fn auth_chain_is_valid(chain: Option<&AuthChain>, address: &str) -> bool {
    let Some(chain) = chain else {
        return false;
    };
    let Some(owner) = chain
        .first()
        .filter(|l| l.link_type == AuthLinkType::SIGNER)
    else {
        return false;
    };
    if !owner.payload.eq_ignore_ascii_case(address) {
        return false;
    }
    let Some(last) = chain.last() else {
        return false;
    };
    let final_authority = match last.link_type {
        AuthLinkType::EcdsaEphemeral => match parse_ephemeral_payload(&last.payload) {
            Ok((_, ephemeral, _)) => ephemeral,
            Err(_) => return false,
        },
        AuthLinkType::EcdsaSignedEntity => last.payload.clone(),
        _ => return false,
    };
    crate::auth::verify_auth_chain(chain, &final_authority, None).is_ok()
}
