//! `~system/SceneCredits.grantCredits`: a scene's authoritative server asks credits-server to pay a
//! player Credits out of a Credits program.
//!
//! Only an orchestrated headless server can, and only for a scene it holds a valid Credits delegation
//! for. The request is this operation's alone, never a scene's signed fetch: the engine builds the
//! body, signs it with the delegation's ephemeral key (the signed metadata carries the body digest)
//! and sends the delegation's claim as `x-authoritative-scope` to the one credits-server route.
//! Whether the recipient is connected comes from the scene's own room, the room the orchestrator
//! assigned it. Credits-server decides everything else; this answers with its verdict mapped to the
//! protocol's `GrantCreditsError`.

use bevy::{
    platform::collections::HashMap,
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{
    rpc::{
        GrantCreditsAnswer, GrantCreditsError, GrantCreditsRequest, RpcCall, RpcCallEvent,
        RpcOrigin, RpcResultSender,
    },
    sets::SceneSets,
    structs::server_mode,
    util::{DespawnWith, TaskCompat, TaskExt},
};
use comms::{global_crdt::ForeignPlayer, scene_rooms::SceneRooms};
use wallet::delegation::CreditsDelegations;

use crate::ScenePresence;

/// A scene's Credits requests on their way to credits-server at once; more are refused as
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
pub(crate) fn validate(request: &GrantCreditsRequest) -> Result<ValidRequest, GrantCreditsError> {
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
        return Err(GrantCreditsError::InvalidRequest);
    }
    let uuid = request.program_id.len() == 36
        && request.program_id.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        });
    if !uuid {
        return Err(GrantCreditsError::NotAuthorized);
    }
    Ok(ValidRequest {
        program_id: request.program_id.to_ascii_lowercase(),
        recipient: request.recipient.to_ascii_lowercase(),
        outcome_id: request.outcome_id.clone(),
    })
}

/// Building, signing and sending the request, and reading credits-server's answer.
mod remote {
    use bevy::log::{error, warn};
    use common::rpc::{GrantCreditsAnswer, GrantCreditsError};
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
    pub(super) fn error_for_code(code: &str) -> GrantCreditsError {
        match code {
            "NOT_AVAILABLE" => GrantCreditsError::NotAvailable,
            "INVALID_REQUEST" => GrantCreditsError::InvalidRequest,
            "RATE_LIMITED" => GrantCreditsError::RateLimited,
            "UNAVAILABLE" => GrantCreditsError::Unavailable,
            "NOT_AUTHORIZED" => GrantCreditsError::NotAuthorized,
            "RECIPIENT_NOT_CONNECTED" => GrantCreditsError::RecipientNotConnected,
            "PROGRAM_PAUSED" => GrantCreditsError::ProgramPaused,
            "OUTSIDE_SCHEDULE" => GrantCreditsError::OutsideSchedule,
            "BUDGET_EXHAUSTED" => GrantCreditsError::BudgetExhausted,
            "PROGRAM_CAP_REACHED" => GrantCreditsError::ProgramCapReached,
            "PLAYER_CAP_REACHED" => GrantCreditsError::PlayerCapReached,
            "REQUIREMENT_NOT_MET" => GrantCreditsError::RequirementNotMet,
            "RECIPIENT_NOT_ELIGIBLE" => GrantCreditsError::RecipientNotEligible,
            _ => GrantCreditsError::Unspecified,
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
    /// error page, a body cut short) says nothing certain about whether the Credits were paid, so
    /// it is `Unavailable`: the scene retries with the same outcome, which credits-server answers
    /// with the receipt if they were.
    pub(super) fn interpret(status: u16, body: &[u8]) -> GrantCreditsAnswer {
        let Ok(response) = serde_json::from_slice::<GrantResponse>(body) else {
            return GrantCreditsAnswer::refused(GrantCreditsError::Unavailable);
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
                    (Some(id), Some(credits)) => GrantCreditsAnswer::paid(replayed, id, credits),
                    _ => GrantCreditsAnswer::refused(GrantCreditsError::Unavailable),
                }
            }
            GrantResponse {
                ok: false,
                code: Some(code),
                ..
            } if !success => GrantCreditsAnswer::refused(error_for_code(&code)),
            _ => GrantCreditsAnswer::refused(GrantCreditsError::Unavailable),
        }
    }

    /// A response body is a few hundred bytes; anything past this is not credits-server's answer.
    const MAX_RESPONSE_BYTES: usize = 16 * 1024;

    /// The client for credits-server: no redirects, since a redirect would carry the claim header
    /// wherever it points, and a 15 s timeout. None if it cannot be built, which only refuses.
    fn http_client() -> Option<&'static reqwest::Client> {
        static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();
        CLIENT
            .get_or_init(|| {
                platform::http_client_builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                    .map_err(|error| error!("scene credits: cannot build the HTTP client: {error}"))
                    .ok()
            })
            .as_ref()
    }

    /// Every header of one Credits request: the delegation's signed-fetch chain over POST, the
    /// URL's path and metadata carrying the body's SHA-256, its claim as `x-authoritative-scope`,
    /// and the content type.
    pub(super) async fn signed_headers(
        url: &str,
        body: &[u8],
        delegation: &wallet::delegation::CreditsDelegation,
    ) -> Result<Vec<(String, String)>, anyhow::Error> {
        use sha2::{Digest, Sha256};

        let uri = url.parse::<http::Uri>()?;
        let mut headers = delegation
            .sign("post", &uri, Some(format!("{:x}", Sha256::digest(body))))
            .await?;
        headers.push(("content-type".to_owned(), "application/json".to_owned()));
        Ok(headers)
    }

    /// The response body, or None when it is longer than credits-server ever answers.
    async fn capped_body(mut response: reqwest::Response) -> Option<Vec<u8>> {
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return None;
            }
            body.extend_from_slice(&chunk);
        }
        Some(body)
    }

    /// Sign and send one Credits request. Every failure after the request may have left is
    /// `Unavailable`; failures before it are too, as the scene can only retry either way.
    pub(super) async fn send(
        url: String,
        body: Vec<u8>,
        delegation: wallet::delegation::CreditsDelegation,
    ) -> GrantCreditsAnswer {
        let unavailable = || GrantCreditsAnswer::refused(GrantCreditsError::Unavailable);
        let Some(client) = http_client() else {
            return unavailable();
        };
        let headers = match signed_headers(&url, &body, &delegation).await {
            Ok(headers) => headers,
            Err(error) => {
                // a local key failing to sign is this engine's bug, never the scene's
                error!("scene credits: signing failed: {error}");
                return unavailable();
            }
        };
        let mut request = client.post(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = match request.body(body).send().await {
            Ok(response) => response,
            Err(error) => {
                warn!("scene credits: credits-server did not answer: {error}");
                return unavailable();
            }
        };
        let status = response.status().as_u16();
        match capped_body(response).await {
            Some(bytes) => interpret(status, &bytes),
            None => unavailable(),
        }
    }

    /// The credits-server route under `base`, over https only: the claim never goes out in
    /// cleartext, so an http override sends nothing.
    pub(super) fn grant_url_for(base: &str, program_id: &str) -> Option<String> {
        base.starts_with("https://").then(|| {
            format!(
                "{}/scene-rewards/programs/{program_id}/grants",
                base.trim_end_matches('/')
            )
        })
    }

    pub(super) fn grant_url(program_id: &str) -> Option<String> {
        grant_url_for(
            &common::base_domain::service(common::base_domain::Service::Credits),
            program_id,
        )
    }
}

/// `~system/SceneCredits`: sends each scene's `grantCredits` to credits-server and answers the
/// scene with the verdict.
pub struct SceneCreditsPlugin;

impl Plugin for SceneCreditsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_grant_credits, answer_grant_credits)
                .chain()
                .in_set(SceneSets::RestrictedActions),
        );
    }
}

/// One Credits request on its way to credits-server, as an entity of its own that is despawned
/// with the scene that asked (`DespawnWith`). A scene that goes away cancels its requests and
/// drops their answer senders, which its module reads as `Unavailable`.
#[derive(Component)]
pub(crate) struct PendingGrantCredits {
    scene: Entity,
    response: RpcResultSender<GrantCreditsAnswer>,
    task: Task<GrantCreditsAnswer>,
}

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

pub(crate) fn handle_grant_credits(
    mut commands: Commands,
    mut events: EventReader<RpcCallEvent>,
    delegations: Option<Res<CreditsDelegations>>,
    players: Query<&ForeignPlayer>,
    scene_rooms: SceneRooms,
    presence: ScenePresence,
    pending: Query<&PendingGrantCredits>,
) {
    // requests this pass starts, which `pending` cannot see yet: spawning is deferred
    let mut started: HashMap<Entity, usize> = HashMap::default();

    let now_millis = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(i64::MAX);

    for ev in events.read() {
        let RpcCall::GrantCredits(request) = &ev.call else {
            continue;
        };
        let refuse = |error| request.response.send(GrantCreditsAnswer::refused(error));
        let RpcOrigin::Scene { entity, hash } = &ev.origin else {
            refuse(GrantCreditsError::NotAvailable);
            continue;
        };
        // a client, or a server holding no Credits delegation for this scene, cannot pay
        let Some(delegations) = delegations
            .as_ref()
            .filter(|d| server_mode() && d.is_assigned(hash))
        else {
            refuse(GrantCreditsError::NotAvailable);
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
            refuse(GrantCreditsError::Unavailable);
            continue;
        };
        let in_flight = pending.iter().filter(|p| p.scene == *entity).count()
            + started.get(entity).copied().unwrap_or(0);
        if in_flight >= MAX_IN_FLIGHT_PER_SCENE {
            refuse(GrantCreditsError::RateLimited);
            continue;
        }
        let Some(url) = remote::grant_url(&valid.program_id) else {
            warn!("scene credits: the credits service is not an https url; not sending");
            refuse(GrantCreditsError::NotAvailable);
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
        commands.spawn((
            PendingGrantCredits {
                scene: *entity,
                response: request.response.clone(),
                task,
            },
            DespawnWith(*entity),
        ));
        *started.entry(*entity).or_default() += 1;
    }
}

/// Answers each request credits-server has settled, and despawns it.
pub(crate) fn answer_grant_credits(
    mut commands: Commands,
    mut pending: Query<(Entity, &mut PendingGrantCredits)>,
) {
    for (entity, mut request) in &mut pending {
        if let Some(answer) = request.task.complete() {
            request.response.send(answer);
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use bevy::{
        prelude::*,
        tasks::{IoTaskPool, TaskPool},
    };
    use common::{
        rpc::{GrantCreditsAnswer, RpcResultSender},
        util::{DespawnWith, UtilsPlugin},
    };

    use super::{answer_grant_credits, PendingGrantCredits};

    fn app() -> App {
        IoTaskPool::get_or_init(TaskPool::new);
        let mut app = App::new();
        app.add_plugins(UtilsPlugin)
            .add_systems(Update, answer_grant_credits);
        app
    }

    #[test]
    fn a_settled_request_answers_its_scene_and_goes_away() {
        let mut app = app();
        let scene = app.world_mut().spawn_empty().id();
        let (response, mut answer) = RpcResultSender::channel();
        let task = IoTaskPool::get()
            .spawn(async { GrantCreditsAnswer::paid(false, "receipt-1".to_owned(), 5) });
        let request = app
            .world_mut()
            .spawn((
                PendingGrantCredits {
                    scene,
                    response,
                    task,
                },
                DespawnWith(scene),
            ))
            .id();

        for _ in 0..500 {
            app.update();
            if app.world().get_entity(request).is_err() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(app.world().get_entity(request).is_err());
        let received = answer.poll_once().expect("answered").expect("answered");
        assert_eq!(
            serde_json::to_value(received).unwrap(),
            serde_json::to_value(GrantCreditsAnswer::paid(false, "receipt-1".to_owned(), 5))
                .unwrap()
        );
    }

    #[test]
    fn a_scene_that_goes_away_drops_its_requests_and_their_answers() {
        let mut app = app();
        let scene = app.world_mut().spawn_empty().id();
        let (response, mut answer) = RpcResultSender::channel();
        let task = IoTaskPool::get().spawn(std::future::pending::<GrantCreditsAnswer>());
        let request = app
            .world_mut()
            .spawn((
                PendingGrantCredits {
                    scene,
                    response,
                    task,
                },
                DespawnWith(scene),
            ))
            .id();
        app.update();
        assert!(app.world().get_entity(request).is_ok());

        app.world_mut().despawn(scene);
        app.update();

        assert!(app.world().get_entity(request).is_err());
        // the scene's module reads a dropped answer as GCE_UNAVAILABLE
        assert!(answer.poll_once().is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::{remote::*, *};
    use common::rpc::RpcResultSender;

    fn request(program_id: &str, recipient: &str, outcome_id: &str) -> GrantCreditsRequest {
        GrantCreditsRequest {
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
                Err(GrantCreditsError::InvalidRequest),
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
                Err(GrantCreditsError::NotAuthorized),
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
            GrantCreditsAnswer {
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
            serde_json::to_value(GrantCreditsAnswer::paid(true, "9f1c-r".to_owned(), 5)).unwrap(),
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
            GrantCreditsAnswer::refused(GrantCreditsError::PlayerCapReached)
        );
        assert_eq!(
            interpret(503, br#"{"ok":false,"code":"UNAVAILABLE"}"#),
            GrantCreditsAnswer::refused(GrantCreditsError::Unavailable)
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
                GrantCreditsAnswer::refused(GrantCreditsError::Unavailable),
                "{status} {}",
                String::from_utf8_lossy(body)
            );
        }
    }

    // ---- the network boundary, against local listeners ----

    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
    };

    /// What a listener received: the request line, lowercased headers and the body.
    #[derive(Debug, Default, Clone)]
    struct Received {
        line: String,
        headers: std::collections::HashMap<String, String>,
        body: Vec<u8>,
    }

    /// A one-request-per-connection HTTP listener on loopback that answers every request with
    /// `reply` and records what it received.
    fn listener(reply: Vec<u8>) -> (String, Arc<Mutex<Vec<Received>>>) {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", socket.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        std::thread::spawn(move || {
            for stream in socket.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let head_end = loop {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break None;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break Some(i);
                    }
                };
                let Some(head_end) = head_end else { continue };
                let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
                let mut lines = head.split("\r\n");
                let line = lines.next().unwrap_or_default().to_owned();
                let headers: std::collections::HashMap<_, _> = lines
                    .filter_map(|l| l.split_once(':'))
                    .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
                    .collect();
                let length: usize = headers
                    .get("content-length")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let mut body = raw[head_end + 4..].to_vec();
                while body.len() < length {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&buf[..n]);
                }
                record.lock().unwrap().push(Received {
                    line,
                    headers,
                    body,
                });
                let _ = stream.write_all(&reply);
            }
        });
        (base, seen)
    }

    fn http_reply(status: &str, extra: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\n{extra}content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    /// A Credits delegation for throwaway key 1, its claim naming that key.
    fn delegation() -> wallet::delegation::CreditsDelegation {
        use base64::Engine;
        let address = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf";
        let payload = [
            "Decentraland Authoritative Delegation",
            "Audience: credits",
            &format!("Ephemeral: {address}"),
            "World: boundary.dcl.eth",
            "SceneId: bafkreiboundary",
            "Parcel: 0,0",
            "Expiration: 2099-01-01T00:00:00.000Z",
        ]
        .join("\n");
        let envelope = serde_json::json!({
            "v": 1,
            "ephemeral": {
                "privateKey": "0000000000000000000000000000000000000000000000000000000000000001",
                "publicKey": "unused",
                "address": address,
            },
            "scope": { "payload": payload, "signature": "0xsig" },
        });
        wallet::delegation::CreditsDelegation::parse(
            &base64::engine::general_purpose::STANDARD.encode(envelope.to_string()),
        )
        .unwrap()
    }

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    const ROUTE: &str = "/scene-rewards/programs/0b5c8d7e-1f2a-4b3c-9d4e-5f6a7b8c9d0e/grants";

    #[test]
    fn the_server_receives_the_signed_path_the_hashed_body_and_the_claim() {
        let (base, seen) = listener(http_reply(
            "201 Created",
            "",
            r#"{"ok":true,"replayed":false,"receiptId":"9f1c-r","credits":5,"usdCents":50}"#,
        ));
        let valid = validate(&request(PROGRAM, RECIPIENT, "door-1")).unwrap();
        let body = request_body(&valid, true);
        let answer = run(send(format!("{base}{ROUTE}"), body.clone(), delegation()));
        assert_eq!(
            answer,
            GrantCreditsAnswer::paid(false, "9f1c-r".to_owned(), 5)
        );

        let received = seen
            .lock()
            .unwrap()
            .pop()
            .expect("the request reached the server");
        assert_eq!(received.line, format!("POST {ROUTE} HTTP/1.1"));
        assert_eq!(received.body, body);
        // the digest the signature covers is the digest of the bytes that arrived
        use sha2::{Digest, Sha256};
        let meta: serde_json::Value =
            serde_json::from_str(&received.headers["x-identity-metadata"]).unwrap();
        assert_eq!(
            meta["hashPayload"],
            format!("{:x}", Sha256::digest(&received.body))
        );
        assert_eq!(meta["signer"], "dcl:authoritative-server");
        // two links, the ephemeral signing this method and this path
        let link: serde_json::Value =
            serde_json::from_str(&received.headers["x-identity-auth-chain-1"]).unwrap();
        assert!(link["payload"]
            .as_str()
            .unwrap()
            .starts_with(&format!("post:{ROUTE}:")));
        assert!(!received.headers.contains_key("x-identity-auth-chain-2"));
        assert!(!received.headers["x-authoritative-scope"].is_empty());
    }

    #[test]
    fn a_redirect_is_never_followed_with_the_claim() {
        let (elsewhere, stolen) = listener(http_reply("200 OK", "", "{}"));
        let (base, seen) = listener(http_reply(
            "302 Found",
            &format!("location: {elsewhere}/steal\r\n"),
            "",
        ));
        let valid = validate(&request(PROGRAM, RECIPIENT, "door-1")).unwrap();
        let answer = run(send(
            format!("{base}{ROUTE}"),
            request_body(&valid, true),
            delegation(),
        ));
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(stolen.lock().unwrap().is_empty());
        assert_eq!(
            answer,
            GrantCreditsAnswer::refused(GrantCreditsError::Unavailable)
        );
    }

    #[test]
    fn an_answer_longer_than_credits_server_gives_is_unavailable() {
        let padding = "x".repeat(20 * 1024);
        let (base, _) = listener(http_reply(
            "201 Created",
            "",
            &format!(
                r#"{{"ok":true,"replayed":false,"receiptId":"r","credits":5,"pad":"{padding}"}}"#
            ),
        ));
        let valid = validate(&request(PROGRAM, RECIPIENT, "door-1")).unwrap();
        let answer = run(send(
            format!("{base}{ROUTE}"),
            request_body(&valid, true),
            delegation(),
        ));
        assert_eq!(
            answer,
            GrantCreditsAnswer::refused(GrantCreditsError::Unavailable)
        );
    }

    #[test]
    fn only_an_https_credits_service_is_ever_sent_to() {
        let (base, seen) = listener(http_reply("200 OK", "", "{}"));
        assert_eq!(grant_url_for(&base, PROGRAM), None);
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(
            grant_url_for("https://credits.example/", PROGRAM).as_deref(),
            Some("https://credits.example/scene-rewards/programs/0b5c8d7e-1f2a-4b3c-9d4e-5f6a7b8c9d0e/grants")
        );
    }
}
