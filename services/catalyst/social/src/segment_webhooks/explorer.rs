//! `POST /forward`: the explorer's Segment events
//! (`forward-explorer-events.ts`, `event-parser.ts:181-450`). In order: the
//! signature; the parse, whose context checks are soft (logged) unless
//! `FORCE_EXPLORER_CONTEXT_VALIDATION`; the auth chain, which every write
//! waits for; then, keyed on the chain owner, the CP7 checkpoint side effect
//! (`otp_verification_success` records CP7 reached and ends the request;
//! `logged_in`/`logged_in_cached` record CP7 completed and continue); the
//! row; the metrics. A checkpoint carries neither the event's email nor
//! metadata.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::store::{ClientEventRow, insert_client_event};
use super::{Settings, SharedState, gate, identity, object, parse_time, refused, reply, text};
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
    if text(&event, "event").is_some_and(|name| name.eq_ignore_ascii_case(OTP_VERIFICATION_SUCCESS))
    {
        return forward_otp(&event, &state.onboarding);
    }
    let Some(parsed) = parse_client_event(&event, &state.settings) else {
        tracing::warn!(key = ?event.get("messageId"), "Segment explorer event: invalid event");
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
    if matches!(parsed.row.sub_type, "logged-in" | "logged-in-cached") {
        record(
            &state.onboarding,
            checkpoint(&parsed.row.address, "completed"),
        );
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

/// `otp_verification_success` (`forward-explorer-events.ts:63-105`), handled
/// alone: CP7 reached by the chain owner. The event's `properties.email` is
/// the client's own word, outside the chain, so it is not kept (an event
/// without one is ignored as the notifier ignored it): the store resolves
/// the wallet to the email the auth server bound to it, and a wallet without
/// that binding reaches no nudge.
fn forward_otp(event: &Value, onboarding: &OnboardingState) -> Response {
    let context = event.get("context").unwrap_or(&Value::Null);
    let Some(wallet) = verified_owner(context) else {
        tracing::warn!(key = ?event.get("messageId"), "Segment explorer event: invalid auth chain");
        return refused(StatusCode::UNAUTHORIZED, "Invalid AuthChain");
    };
    match event.get("properties").and_then(|p| text(p, "email")) {
        Some(_) => record(onboarding, checkpoint(&wallet, "reached")),
        None => tracing::warn!("otp_verification_success missing email property"),
    }
    reply(StatusCode::OK, json!({ "ok": true }))
}

/// The owner of `context.auth_chain` when it verifies for
/// `context.dcl_eth_address`, lower-cased.
fn verified_owner(context: &Value) -> Option<String> {
    let address = text(context, "dcl_eth_address")?.to_lowercase();
    let chain = serde_json::from_str::<AuthChain>(text(context, "auth_chain")?).ok()?;
    auth_chain_is_valid(Some(&chain), &address).then_some(address)
}

/// The CP7 checkpoint of a verified wallet: the wallet is its user
/// identifier (the store resolves it to the email the auth server bound and
/// keeps no row for a wallet without one), it names no email and it carries
/// no metadata.
fn checkpoint(wallet: &str, action: &str) -> CheckpointRequest {
    CheckpointRequest {
        checkpoint_id: 7,
        action: action.into(),
        user_identifier: wallet.into(),
        identifier_type: "wallet".into(),
        email: None,
        wallet: Some(wallet.into()),
        source: Some("explorer".into()),
        metadata: None,
    }
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
    let key = identity(event, "messageId")?.to_string();
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
    let string = |key: &str| identity(context, key).map(str::to_string);
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

/// `validateExplorerContext` (`event-parser.ts:181-299`) without its cluster
/// writes and the anonymous-id denylist: every check refuses only when forced.
fn validate_context(event: &Value, context: &Value, address: &str, settings: &Settings) -> bool {
    let soft = || !settings.force_context_validation;
    if text(context, "dcl_renderer_type") != Some("Desktop") {
        tracing::warn!(renderer = ?context.get("dcl_renderer_type"), "Invalid renderer type");
        return soft();
    }
    match event.get("userId") {
        None | Some(Value::Null) => {}
        Some(Value::String(user_id))
            if user_id.is_empty() || user_id.eq_ignore_ascii_case(address) => {}
        other => {
            tracing::warn!(user_id = ?other, address, "User ID mismatch with eth address");
            return soft();
        }
    }
    if text(event, "type") != Some("track") {
        tracing::warn!(kind = ?event.get("type"), "Invalid event type");
        return soft();
    }
    if !is_eth_address(address) {
        tracing::warn!(address, "Invalid Ethereum address format");
        return soft();
    }
    if !chain_shape_is_valid(context, address) {
        tracing::warn!(address, "Invalid auth_chain structure or signer");
        return soft();
    }
    if let Some(expected) = settings.expected_realm.as_deref() {
        let realm = text(context, "realm").map(str::to_lowercase);
        let url_matches = settings
            .expected_realm_url
            .as_deref()
            .is_none_or(|url| text(context, "realm_url") == Some(url));
        if realm.as_deref() != Some(&expected.to_lowercase()) || !url_matches {
            tracing::warn!(realm = ?context.get("realm"), "Unknown realm");
            return soft();
        }
    }
    true
}

/// The notifier's shape check, narrowed to the chain [`auth_chain_is_valid`]
/// takes: a JSON array of exactly two links, `[0]` the SIGNER of `address`,
/// `[1]` an ECDSA_EPHEMERAL link with a `0x` signature.
fn chain_shape_is_valid(context: &Value, address: &str) -> bool {
    let Some(chain) = context
        .get("auth_chain")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
    else {
        return false;
    };
    let Some(links) = chain.as_array().filter(|links| links.len() == 2) else {
        return false;
    };
    text(&links[0], "type") == Some("SIGNER")
        && text(&links[0], "payload").is_some_and(|p| p.eq_ignore_ascii_case(address))
        && text(&links[1], "type") == Some("ECDSA_EPHEMERAL")
        && text(&links[1], "signature").is_some_and(|s| s.starts_with("0x"))
}

/// The notifier's `validateAuthChain` (a valid chain owned by `address`)
/// plus the signatures and expiry, for the explorer's login chain alone:
/// `[SIGNER, ECDSA_EPHEMERAL]`, the owner's signature reaching the ephemeral
/// key. A chain ending in a signed entity proves nothing about the sender:
/// content servers publish one with every profile and scene deployment.
fn auth_chain_is_valid(chain: Option<&AuthChain>, address: &str) -> bool {
    let Some(chain) = chain else {
        return false;
    };
    let [owner, delegation] = chain.as_slice() else {
        return false;
    };
    if owner.link_type != AuthLinkType::SIGNER
        || !owner.payload.eq_ignore_ascii_case(address)
        || delegation.link_type != AuthLinkType::EcdsaEphemeral
    {
        return false;
    }
    let Ok((_, ephemeral, _)) = parse_ephemeral_payload(&delegation.payload) else {
        return false;
    };
    crate::auth::verify_auth_chain(chain, &ephemeral, None).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Wallet;

    /// A context signed now by a fresh owner, as the explorer's.
    fn signed_context() -> (String, Value) {
        let owner = Wallet::random().unwrap();
        let ephemeral = Wallet::random().unwrap();
        let expiration = (chrono::Utc::now() + chrono::Duration::days(1))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let payload = format!(
            "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
            ephemeral.address()
        );
        let signature = owner.sign_message(payload.as_bytes()).unwrap();
        let chain = json!([
            { "payload": owner.address(), "signature": "", "type": "SIGNER" },
            { "payload": payload, "signature": signature, "type": "ECDSA_EPHEMERAL" },
        ]);
        let context = json!({
            "dcl_eth_address": owner.address(),
            "auth_chain": chain.to_string(),
            "session_id": "s-1",
            "launcher_anonymous_id": "anon-1",
        });
        (owner.address().to_lowercase(), context)
    }

    #[test]
    fn the_otp_checkpoint_of_a_wallet_no_email_is_bound_to_records_nothing() {
        let (owner, context) = signed_context();
        let onboarding = OnboardingState::default();
        let event = json!({
            "event": "otp_verification_success",
            "properties": { "email": "user@example.com" },
            "context": context,
        });
        assert_eq!(forward_otp(&event, &onboarding).status(), StatusCode::OK);
        assert!(
            onboarding.checkpoint_state(&owner, 7).is_none(),
            "a row no nudge can reach would only evict a user's"
        );
        assert!(
            onboarding.checkpoint_state("user@example.com", 7).is_none(),
            "the event's email is the client's word, not the user identifier"
        );
    }

    #[test]
    fn the_otp_checkpoint_takes_the_email_the_auth_server_bound_to_the_wallet() {
        let (owner, context) = signed_context();
        let onboarding = OnboardingState::default();
        onboarding
            .record_checkpoint(CheckpointRequest {
                checkpoint_id: 1,
                action: "reached".into(),
                user_identifier: "bound@example.com".into(),
                identifier_type: "email".into(),
                email: Some("bound@example.com".into()),
                wallet: Some(owner.clone()),
                source: Some("auth-server".into()),
                metadata: None,
            })
            .unwrap();
        let event = json!({
            "event": "otp_verification_success",
            "properties": { "email": "attacker@example.com" },
            "context": context,
        });
        assert_eq!(forward_otp(&event, &onboarding).status(), StatusCode::OK);
        let state = onboarding
            .checkpoint_state("bound@example.com", 7)
            .expect("recorded by the bound user");
        assert_eq!(state.email.as_deref(), Some("bound@example.com"));
        assert_eq!(state.wallet.as_deref(), Some(owner.as_str()));
        assert_eq!(
            state.metadata, None,
            "a Segment checkpoint carries no metadata"
        );
        assert!(onboarding.checkpoint_state(&owner, 7).is_none());
        assert!(
            onboarding
                .checkpoint_state("attacker@example.com", 7)
                .is_none()
        );
    }

    #[test]
    fn the_otp_checkpoint_needs_the_owners_chain() {
        let (owner, mut context) = signed_context();
        let onboarding = OnboardingState::default();
        let other = Wallet::random().unwrap().address();
        context["dcl_eth_address"] = json!(other);
        let event = json!({
            "event": "otp_verification_success",
            "properties": { "email": "user@example.com" },
            "context": context,
        });
        assert_eq!(
            forward_otp(&event, &onboarding).status(),
            StatusCode::UNAUTHORIZED
        );
        assert!(onboarding.checkpoint_state(&other, 7).is_none());
        assert!(onboarding.checkpoint_state(&owner, 7).is_none());
        let unsigned = json!({
            "event": "otp_verification_success",
            "properties": { "email": "user@example.com" },
            "context": { "session_id": "s-1" },
        });
        assert_eq!(
            forward_otp(&unsigned, &onboarding).status(),
            StatusCode::UNAUTHORIZED
        );
        assert!(onboarding.checkpoint_state("user@example.com", 7).is_none());
    }

    /// The chains a content server publishes with every deployment: the
    /// owner's delegation and the entity signed by the ephemeral key, or the
    /// entity signed by the owner. Both verify as deployment chains and
    /// neither is the explorer's login chain, so both are 401.
    #[test]
    fn a_published_deployment_chain_is_refused() {
        let owner = Wallet::random().unwrap();
        let ephemeral = Wallet::random().unwrap();
        let expiration = (chrono::Utc::now() + chrono::Duration::days(1))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let delegation = format!(
            "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
            ephemeral.address()
        );
        let entity = "bafkreiexampleprofileentity";
        let signer = json!({ "payload": owner.address(), "signature": "", "type": "SIGNER" });
        let chains = [
            json!([
                signer.clone(),
                { "payload": delegation, "signature": owner.sign_message(delegation.as_bytes()).unwrap(), "type": "ECDSA_EPHEMERAL" },
                { "payload": entity, "signature": ephemeral.sign_message(entity.as_bytes()).unwrap(), "type": "ECDSA_SIGNED_ENTITY" },
            ]),
            json!([
                signer.clone(),
                { "payload": entity, "signature": owner.sign_message(entity.as_bytes()).unwrap(), "type": "ECDSA_SIGNED_ENTITY" },
            ]),
        ];
        for chain in chains {
            let parsed: AuthChain = serde_json::from_value(chain.clone()).unwrap();
            assert!(
                crate::auth::verify_auth_chain(&parsed, entity, None).is_ok(),
                "a genuine deployment chain"
            );
            assert!(!auth_chain_is_valid(Some(&parsed), &owner.address()));
            let onboarding = OnboardingState::default();
            let event = json!({
                "event": "otp_verification_success",
                "properties": { "email": "user@example.com" },
                "context": { "dcl_eth_address": owner.address(), "auth_chain": chain.to_string() },
            });
            assert_eq!(
                forward_otp(&event, &onboarding).status(),
                StatusCode::UNAUTHORIZED
            );
            assert!(
                onboarding
                    .checkpoint_state(&owner.address().to_lowercase(), 7)
                    .is_none()
            );
        }
    }

    #[test]
    fn the_logged_in_checkpoint_completes_the_bound_wallets_cp7_without_metadata() {
        let onboarding = OnboardingState::default();
        let wallet = "0x0000000000000000000000000000000000000001";
        record(&onboarding, checkpoint(wallet, "completed"));
        assert!(
            onboarding.checkpoint_state(wallet, 7).is_none(),
            "no row for a wallet no email is bound to"
        );
        onboarding
            .record_checkpoint(CheckpointRequest {
                checkpoint_id: 1,
                action: "reached".into(),
                user_identifier: "bound@example.com".into(),
                identifier_type: "email".into(),
                email: Some("bound@example.com".into()),
                wallet: Some(wallet.into()),
                source: Some("auth-server".into()),
                metadata: None,
            })
            .unwrap();
        record(&onboarding, checkpoint(wallet, "reached"));
        record(&onboarding, checkpoint(wallet, "completed"));
        let state = onboarding
            .checkpoint_state("bound@example.com", 7)
            .expect("recorded by the bound user");
        assert!(state.completed);
        assert_eq!(state.email.as_deref(), Some("bound@example.com"));
        assert_eq!(state.wallet.as_deref(), Some(wallet));
        assert_eq!(state.metadata, None);
        assert!(onboarding.checkpoint_state(wallet, 7).is_none());
    }

    #[test]
    fn an_identity_column_past_its_bound_is_absent() {
        let long = "x".repeat(super::super::MAX_IDENTITY_BYTES + 1);
        let context = json!({ "session_id": long, "messageId": long, "realm": "main\u{1}", "ip": "10.0.0.1" });
        assert_eq!(identity(&context, "session_id"), None);
        assert_eq!(identity(&context, "messageId"), None);
        assert_eq!(
            identity(&context, "realm"),
            None,
            "a control character escapes to six bytes"
        );
        assert_eq!(identity(&context, "ip"), Some("10.0.0.1"));
    }
}
