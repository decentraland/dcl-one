//! `~system/SceneRewards.grantReward`: a scene's authoritative server asks credits-server to pay a
//! player a reward out of a Credits program.
//!
//! Only an orchestrated headless server can, and only for a scene it holds a valid Credits delegation
//! for. The request is this operation's alone, never a scene's signed fetch: the engine builds the
//! body, signs it with the delegation's ephemeral key (the signed metadata carries the body digest)
//! and sends the delegation's claim as `x-authoritative-scope` to the one credits-server route.
//! Whether the recipient is connected comes from the scene's own room, the room the orchestrator
//! assigned it. Credits-server decides everything else; this answers with its verdict mapped to the
//! protocol's `GrantRewardError`.

use std::sync::Arc;

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{
    rpc::{
        GrantRewardAnswer, GrantRewardError, GrantRewardRequest, RpcCall, RpcCallEvent, RpcOrigin,
    },
    structs::server_mode,
    util::{TaskCompat, TaskExt},
};
use comms::{global_crdt::ForeignPlayer, scene_rooms::SceneRooms};
use wallet::delegation::CreditsDelegations;

use crate::ScenePresence;

/// A scene's reward requests on their way to credits-server at once; more are refused as
/// `RateLimited` rather than queued, so a busy scene cannot pile up payments behind a slow server.
const MAX_IN_FLIGHT_PER_SCENE: usize = 4;

/// What the scene sent, checked before anything is signed or sent.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ValidRequest {
    /// lowercased
    pub program_id: String,
    /// lowercased
    pub recipient: String,
    pub outcome_id: String,
}

/// The formats credits-server accepts: an address and an outcome of 1 to 128 characters from
/// `A-Za-z0-9._:-` (else `InvalidRequest`), and a UUID in any ASCII case for the program. A program
/// id that is not one is answered as credits-server answers it, `NotAuthorized`, as an unknown
/// program would be.
pub(crate) fn validate(request: &GrantRewardRequest) -> Result<ValidRequest, GrantRewardError> {
    let address = request
        .recipient
        .strip_prefix("0x")
        .is_some_and(|hex| hex.len() == 40 && hex.chars().all(|c| c.is_ascii_hexdigit()));
    let outcome = (1..=128).contains(&request.outcome_id.len())
        && request
            .outcome_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'));
    if !(address && outcome) {
        return Err(GrantRewardError::InvalidRequest);
    }
    let uuid = request.program_id.len() == 36
        && request.program_id.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        });
    if !uuid {
        return Err(GrantRewardError::NotAuthorized);
    }
    Ok(ValidRequest {
        program_id: request.program_id.to_ascii_lowercase(),
        recipient: request.recipient.to_ascii_lowercase(),
        outcome_id: request.outcome_id.clone(),
    })
}

/// Building, signing and sending the request, and reading credits-server's answer.
mod remote {
    use bevy::log::warn;
    use common::rpc::{GrantRewardAnswer, GrantRewardError};
    use serde::Deserialize;

    use super::ValidRequest;

    /// The exact bytes sent and signed.
    pub(super) fn request_body(request: &ValidRequest, recipient_connected: bool) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "recipient": request.recipient,
            "outcomeId": request.outcome_id,
            "recipientConnected": recipient_connected,
        }))
        .expect("a json value always serializes")
    }

    /// Credits-server's refusal code as the protocol number. A code this engine does not know is
    /// `Unspecified`: a refusal, but not one the scene can act on.
    pub(super) fn error_for_code(code: &str) -> GrantRewardError {
        match code {
            "NOT_AVAILABLE" => GrantRewardError::NotAvailable,
            "INVALID_REQUEST" => GrantRewardError::InvalidRequest,
            "RATE_LIMITED" => GrantRewardError::RateLimited,
            "UNAVAILABLE" => GrantRewardError::Unavailable,
            "NOT_AUTHORIZED" => GrantRewardError::NotAuthorized,
            "RECIPIENT_NOT_CONNECTED" => GrantRewardError::RecipientNotConnected,
            "PROGRAM_PAUSED" => GrantRewardError::ProgramPaused,
            "OUTSIDE_SCHEDULE" => GrantRewardError::OutsideSchedule,
            "BUDGET_EXHAUSTED" => GrantRewardError::BudgetExhausted,
            "PROGRAM_CAP_REACHED" => GrantRewardError::ProgramCapReached,
            "PLAYER_CAP_REACHED" => GrantRewardError::PlayerCapReached,
            "REQUIREMENT_NOT_MET" => GrantRewardError::RequirementNotMet,
            "RECIPIENT_NOT_ELIGIBLE" => GrantRewardError::RecipientNotEligible,
            _ => GrantRewardError::Unspecified,
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GrantResponse {
        ok: bool,
        replayed: Option<bool>,
        receipt_id: Option<String>,
        credits: Option<serde_json::Value>,
        code: Option<String>,
    }

    /// A receipt id as credits-server issues them: 1 to 128 visible ASCII characters.
    fn receipt_id(value: Option<String>) -> Option<String> {
        value.filter(|id| (1..=128).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_graphic()))
    }

    /// Credits-server's answer. A payment is a success status with `ok: true` and the whole
    /// receipt: `replayed`, the receipt id and the whole Credits paid, more than zero. Only a stated
    /// refusal code is a refusal. Anything else (a success missing part of its receipt, a gateway
    /// error page, a body cut short) says nothing certain about whether the reward was paid, so it
    /// is `Unavailable`: the scene retries with the same outcome, which credits-server answers with
    /// the receipt if it was.
    pub(super) fn interpret(status: u16, body: &[u8]) -> GrantRewardAnswer {
        let Ok(response) = serde_json::from_slice::<GrantResponse>(body) else {
            return GrantRewardAnswer::refused(GrantRewardError::Unavailable);
        };
        let success = (200..300).contains(&status);
        match response {
            GrantResponse {
                ok: true,
                replayed: Some(replayed),
                receipt_id: id,
                credits: Some(credits),
                ..
            } if success => {
                match (
                    receipt_id(id),
                    credits.as_u64().filter(|credits| *credits > 0),
                ) {
                    (Some(id), Some(credits)) => GrantRewardAnswer::paid(replayed, id, credits),
                    _ => GrantRewardAnswer::refused(GrantRewardError::Unavailable),
                }
            }
            GrantResponse {
                ok: false,
                code: Some(code),
                ..
            } if !success => GrantRewardAnswer::refused(error_for_code(&code)),
            _ => GrantRewardAnswer::refused(GrantRewardError::Unavailable),
        }
    }

    fn http_client() -> &'static reqwest::Client {
        static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
        CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                // a redirect would carry the claim header to wherever it points
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("a client with a timeout and no redirects always builds")
        })
    }

    /// Every header of one reward request: the delegation's two-link signed-fetch chain over POST, the
    /// URL's path and metadata carrying the body's SHA-256, the delegation's claim as
    /// `x-authoritative-scope`, and the content type.
    pub(super) async fn signed_headers(
        url: &str,
        body: &[u8],
        delegation: &wallet::delegation::CreditsDelegation,
    ) -> Result<Vec<(String, String)>, anyhow::Error> {
        use sha2::{Digest, Sha256};

        let uri = url.parse::<http::Uri>()?;
        let mut meta = serde_json::from_str::<serde_json::Value>(&delegation.meta)?;
        meta["hashPayload"] = format!("{:x}", Sha256::digest(body)).into();
        let mut headers =
            wallet::sign_request("post", &uri, &delegation.wallet, meta.to_string()).await?;
        headers.push((
            "x-authoritative-scope".to_owned(),
            delegation.scope_header.clone(),
        ));
        headers.push(("content-type".to_owned(), "application/json".to_owned()));
        Ok(headers)
    }

    /// Sign and send one reward request. Every failure after the request may have left is
    /// `Unavailable`; failures before it are too, as the scene can only retry either way.
    pub(super) async fn send(
        url: String,
        body: Vec<u8>,
        delegation: wallet::delegation::CreditsDelegation,
    ) -> GrantRewardAnswer {
        let unavailable = || GrantRewardAnswer::refused(GrantRewardError::Unavailable);
        let headers = match signed_headers(&url, &body, &delegation).await {
            Ok(headers) => headers,
            Err(error) => {
                warn!("scene reward: signing failed: {error}");
                return unavailable();
            }
        };
        let mut request = http_client().post(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = match request.body(body).send().await {
            Ok(response) => response,
            Err(error) => {
                warn!("scene reward: credits-server did not answer: {error}");
                return unavailable();
            }
        };
        let status = response.status().as_u16();
        match response.bytes().await {
            Ok(bytes) => interpret(status, &bytes),
            Err(_) => unavailable(),
        }
    }

    /// The credits-server route, over https only: the claim never goes out in cleartext.
    pub(super) fn grant_url(program_id: &str) -> Option<String> {
        let url = common::base_domain::url(
            common::base_domain::Service::Credits,
            &format!("/scene-rewards/programs/{program_id}/grants"),
        );
        url.starts_with("https://").then_some(url)
    }
}

type InFlight = Vec<(
    Arc<str>,
    common::rpc::RpcResultSender<GrantRewardAnswer>,
    Task<GrantRewardAnswer>,
)>;

/// Whether `recipient` is connected to THIS scene's room, in this scene's crdt context: the room
/// the orchestrator assigned the scene, so a player in a co-tenant scene never counts, even one
/// whose scene shares the entity id.
fn recipient_connected(
    recipient: &str,
    scene: Entity,
    hash: &str,
    players: &Query<&ForeignPlayer>,
    scene_rooms: &SceneRooms,
    presence: &ScenePresence,
) -> bool {
    let Ok(recipient) = recipient.parse::<alloy_core::primitives::Address>() else {
        return false;
    };
    let Some(room) = scene_rooms
        .room(hash)
        .filter(|_| scene_rooms.connected(hash))
    else {
        return false;
    };
    let context = presence.context_of(scene);
    players.iter().any(|player| {
        player.address == recipient
            && player.context == context
            && player.transports.contains(&room)
    })
}

pub(crate) fn handle_grant_reward(
    mut events: EventReader<RpcCallEvent>,
    delegations: Option<Res<CreditsDelegations>>,
    players: Query<&ForeignPlayer>,
    scene_rooms: SceneRooms,
    presence: ScenePresence,
    mut in_flight: Local<InFlight>,
) {
    let now_millis = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(i64::MAX);

    for ev in events.read() {
        let RpcCall::GrantReward(request) = &ev.call else {
            continue;
        };
        let refuse = |error| request.response.send(GrantRewardAnswer::refused(error));
        let RpcOrigin::Scene { entity, hash } = &ev.origin else {
            refuse(GrantRewardError::NotAvailable);
            continue;
        };
        // a client, or a server holding no Credits delegation for this scene, cannot pay
        let Some(delegations) = delegations
            .as_ref()
            .filter(|d| server_mode() && d.is_assigned(hash))
        else {
            refuse(GrantRewardError::NotAvailable);
            continue;
        };
        let valid = match validate(request) {
            Ok(valid) => valid,
            Err(error) => {
                refuse(error);
                continue;
            }
        };
        // held but lapsed: its renewal is on the way, so the scene can retry
        let Some(delegation) = delegations.valid_for(hash, now_millis) else {
            refuse(GrantRewardError::Unavailable);
            continue;
        };
        if in_flight.iter().filter(|(scene, ..)| scene == hash).count() >= MAX_IN_FLIGHT_PER_SCENE {
            refuse(GrantRewardError::RateLimited);
            continue;
        }
        let Some(url) = remote::grant_url(&valid.program_id) else {
            warn!("scene reward: the credits service is not an https url; not sending");
            refuse(GrantRewardError::NotAvailable);
            continue;
        };

        let connected = recipient_connected(
            &valid.recipient,
            *entity,
            hash,
            &players,
            &scene_rooms,
            &presence,
        );
        let body = remote::request_body(&valid, connected);
        let task = IoTaskPool::get().spawn_compat(remote::send(url, body, delegation.clone()));
        in_flight.push((hash.clone(), request.response.clone(), task));
    }

    in_flight.retain_mut(|(_, response, task)| match task.complete() {
        Some(answer) => {
            response.send(answer);
            false
        }
        None => true,
    });
}

#[cfg(test)]
mod tests {
    use super::{remote::*, *};
    use common::rpc::RpcResultSender;

    fn request(program_id: &str, recipient: &str, outcome_id: &str) -> GrantRewardRequest {
        GrantRewardRequest {
            program_id: program_id.to_owned(),
            recipient: recipient.to_owned(),
            outcome_id: outcome_id.to_owned(),
            response: RpcResultSender::channel().0,
        }
    }

    const PROGRAM: &str = "0b5c8d7e-1f2a-4b3c-9d4e-5f6a7b8c9d0e";
    const RECIPIENT: &str = "0xAbCdEf0123456789aBcDeF0123456789AbCdEf01";

    #[test]
    fn accepts_a_valid_request_and_lowercases_program_and_recipient() {
        let expected = ValidRequest {
            program_id: PROGRAM.to_owned(),
            recipient: RECIPIENT.to_ascii_lowercase(),
            outcome_id: "2026-12-03".to_owned(),
        };
        for program in [PROGRAM, "0B5C8D7E-1F2A-4B3C-9D4E-5F6A7B8C9D0E"] {
            assert_eq!(
                validate(&request(program, RECIPIENT, "2026-12-03")),
                Ok(ValidRequest {
                    program_id: expected.program_id.clone(),
                    recipient: expected.recipient.clone(),
                    outcome_id: expected.outcome_id.clone(),
                })
            );
        }
    }

    #[test]
    fn refuses_a_malformed_recipient_or_outcome_as_an_invalid_request() {
        let long = "a".repeat(129);
        for (program, recipient, outcome) in [
            (PROGRAM, "", "x"),
            (PROGRAM, "0xabc", "x"),
            (PROGRAM, "AbCdEf0123456789aBcDeF0123456789AbCdEf0100", "x"),
            (PROGRAM, "0xzzcdef0123456789abcdef0123456789abcdef01", "x"),
            (PROGRAM, RECIPIENT, ""),
            (PROGRAM, RECIPIENT, "door 3"),
            (PROGRAM, RECIPIENT, "door/3"),
            (PROGRAM, RECIPIENT, long.as_str()),
            // a bad program id does not mask a bad recipient
            ("", "0xabc", "x"),
        ] {
            assert_eq!(
                validate(&request(program, recipient, outcome)),
                Err(GrantRewardError::InvalidRequest),
                "{program} {recipient} {outcome}"
            );
        }
    }

    #[test]
    fn refuses_a_malformed_program_id_as_not_authorized() {
        for program in [
            "",
            "0b5c8d7e1f2a4b3c9d4e5f6a7b8c9d0e0000",
            "0b5c8d7e-1f2a-4b3c-9d4e-5f6a7b8c9d0g",
            "0b5c8d7e-1f2a-4b3c-9d4e-5f6a7b8c9d0e0",
        ] {
            assert_eq!(
                validate(&request(program, RECIPIENT, "x")),
                Err(GrantRewardError::NotAuthorized),
                "{program}"
            );
        }
    }

    #[test]
    fn the_body_holds_the_three_fields_credits_server_reads() {
        let valid = validate(&request(PROGRAM, RECIPIENT, "match-7")).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&request_body(&valid, true)).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "recipient": RECIPIENT.to_ascii_lowercase(),
                "outcomeId": "match-7",
                "recipientConnected": true,
            })
        );
    }

    #[test]
    fn maps_every_credits_server_code_to_its_protocol_number() {
        for (code, number) in [
            ("NOT_AVAILABLE", 1),
            ("INVALID_REQUEST", 2),
            ("RATE_LIMITED", 3),
            ("UNAVAILABLE", 4),
            ("NOT_AUTHORIZED", 5),
            ("RECIPIENT_NOT_CONNECTED", 6),
            ("PROGRAM_PAUSED", 7),
            ("OUTSIDE_SCHEDULE", 8),
            ("BUDGET_EXHAUSTED", 9),
            ("PROGRAM_CAP_REACHED", 10),
            ("PLAYER_CAP_REACHED", 11),
            ("REQUIREMENT_NOT_MET", 12),
            ("RECIPIENT_NOT_ELIGIBLE", 13),
            ("SOMETHING_NEW", 0),
            ("not_authorized", 0),
        ] {
            assert_eq!(error_for_code(code) as i32, number, "{code}");
        }
    }

    #[test]
    fn a_payment_carries_the_whole_receipt() {
        let first = interpret(
            201,
            br#"{"ok":true,"replayed":false,"receiptId":"9f1c-r","credits":5,"usdCents":50}"#,
        );
        assert_eq!(
            first,
            GrantRewardAnswer {
                ok: true,
                error: None,
                replayed: Some(false),
                receipt_id: Some("9f1c-r".to_owned()),
                credits: Some("5".to_owned()),
            }
        );
        // the recovered payment: the same receipt and amount, marked replayed
        let recovered = interpret(
            200,
            br#"{"ok":true,"replayed":true,"receiptId":"9f1c-r","credits":5,"usdCents":50}"#,
        );
        assert_eq!(recovered.replayed, Some(true));
        assert_eq!(
            (recovered.receipt_id, recovered.credits),
            (first.receipt_id, first.credits)
        );
    }

    #[test]
    fn the_answer_reaches_the_scene_under_the_protocol_field_names() {
        assert_eq!(
            serde_json::to_value(GrantRewardAnswer::paid(true, "9f1c-r".to_owned(), 5)).unwrap(),
            serde_json::json!({
                "ok": true,
                "error": null,
                "replayed": true,
                "receiptId": "9f1c-r",
                "credits": "5",
            })
        );
    }

    #[test]
    fn a_refusal_is_an_error_status_with_its_code() {
        assert_eq!(
            interpret(409, br#"{"ok":false,"code":"PLAYER_CAP_REACHED"}"#),
            GrantRewardAnswer::refused(GrantRewardError::PlayerCapReached)
        );
        assert_eq!(
            interpret(503, br#"{"ok":false,"code":"UNAVAILABLE"}"#),
            GrantRewardAnswer::refused(GrantRewardError::Unavailable)
        );
    }

    #[test]
    fn anything_else_is_unavailable_never_paid() {
        for (status, body) in [
            (502, &b"<html>bad gateway</html>"[..]),
            (200, br#"{"ok":false,"code":"NOT_AUTHORIZED"}"#),
            (500, br#"{"ok":true}"#),
            (409, br#"{"ok":false}"#),
            // a success missing part of its receipt is not a confirmed payment
            (201, br#"{"ok":true}"#),
            (201, br#"{"ok":true,"receiptId":"r","credits":5}"#),
            (201, br#"{"ok":true,"replayed":false,"credits":5}"#),
            (
                201,
                br#"{"ok":true,"replayed":false,"receiptId":"","credits":5}"#,
            ),
            (201, br#"{"ok":true,"replayed":false,"receiptId":"r"}"#),
            (
                201,
                br#"{"ok":true,"replayed":false,"receiptId":"r","credits":0}"#,
            ),
            (
                201,
                br#"{"ok":true,"replayed":false,"receiptId":"r","credits":1.5}"#,
            ),
            (
                201,
                br#"{"ok":true,"replayed":false,"receiptId":"r","credits":"5"}"#,
            ),
            (
                201,
                br#"{"ok":true,"replayed":false,"receiptId":"r r","credits":5}"#,
            ),
            (201, br#"{"ok":tr"#),
            (200, b""),
        ] {
            assert_eq!(
                interpret(status, body),
                GrantRewardAnswer::refused(GrantRewardError::Unavailable),
                "{status} {}",
                String::from_utf8_lossy(body)
            );
        }
    }
}
