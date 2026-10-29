//! `POST /livekit-webhook`: the one LiveKit webhook route and its dispatch.
//! The verified participant events drive the voice rooms' presence
//! (`gatekeeper::voice`), the ingress events the stream rows
//! (`gatekeeper::streaming`), then the live presence and the room event sink.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use super::errors::{ApiError, unauthorized};
use super::livekit::{address_from_identity, verify_webhook_token};
use super::voice::{
    DisconnectReason, handle_community_participant_joined, handle_community_participant_left,
    handle_private_participant_joined, handle_private_participant_left, now_ms, require_livekit,
};
use super::{CommsState, SharedState};
use crate::rooms::{community_id_from_room_name, private_call_id_from_room_name};

// A rolling history bounds memory without imposing a global event quota.
// At overload the oldest protection is lost; lifecycle application still runs.
// IDs expire at their authenticated JWT acceptance deadline. Active identities
// are kept independently until displaced/evicted, never expired by the join JWT.
const MAX_ENTRIES: usize = 100_000;
const LOCK_STRIPES: usize = 64;
const METADATA_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(60);

struct History<V> {
    values: HashMap<String, (V, u64)>,
    order: BTreeMap<u64, String>,
    sequence: u64,
    capacity: usize,
}

impl<V> Default for History<V> {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
            order: BTreeMap::new(),
            sequence: 0,
            capacity: MAX_ENTRIES,
        }
    }
}

impl<V> History<V> {
    fn get(&self, key: &str) -> Option<&V> {
        self.values.get(key).map(|(value, _)| value)
    }
    fn insert(&mut self, key: String, value: V) {
        if let Some((_, order)) = self.values.remove(&key) {
            self.order.remove(&order);
        }
        while self.values.len() >= self.capacity {
            if let Some((_, key)) = self.order.pop_first() {
                self.values.remove(&key);
            }
        }
        self.sequence += 1;
        self.order.insert(self.sequence, key.clone());
        self.values.insert(key, (value, self.sequence));
    }
    fn remove(&mut self, key: &str) {
        if let Some((_, order)) = self.values.remove(key) {
            self.order.remove(&order);
        }
    }
}

#[derive(Default)]
struct WebhookState {
    applied: History<u64>,
    participants: History<(i64, bool)>,
    current: History<(String, i64, bool)>,
    metadata: History<(String, tokio::time::Instant)>,
}

pub(super) struct WebhookCoordinator {
    state: Mutex<WebhookState>,
    locks: [tokio::sync::Mutex<()>; LOCK_STRIPES],
}

impl Default for WebhookCoordinator {
    fn default() -> Self {
        Self {
            state: Mutex::new(WebhookState::default()),
            locks: std::array::from_fn(|_| tokio::sync::Mutex::new(())),
        }
    }
}

fn stripe(key: &str) -> usize {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hash);
    hash.finish() as usize % LOCK_STRIPES
}

fn current_key(event: &Value) -> Option<String> {
    let room = room_name(event)?;
    let identity = participant_identity(event)?;
    // The exact host identity must not replace a retained case variant's SID.
    // All counted identities keep the existing case-insensitive session key.
    let class = if crate::presence::counts_as_player(&identity) {
        "player"
    } else {
        "server"
    };
    Some(format!(
        "{}:{room}{class}:{}",
        room.len(),
        identity.to_lowercase()
    ))
}

impl WebhookState {
    fn metadata_due(&self, room: &str, sid: Option<&str>) -> bool {
        !sid.is_some_and(|sid| {
            self.metadata.get(room).is_some_and(|(current, completed)| {
                current == sid && completed.elapsed() < METADATA_COOLDOWN
            })
        })
    }

    fn prepare(&mut self, event: &Value, kind: &str, deadline: u64, now: u64) -> bool {
        let id = event_id(event).unwrap();
        if let Some(previous) = self.applied.get(id).copied() {
            if now <= previous {
                self.applied.insert(id.to_owned(), deadline.max(previous));
                return false;
            }
            self.applied.remove(id);
        }
        if matches!(kind, "participant_joined" | "participant_left") {
            let sid = participant_sid(event).unwrap();
            let at = event_time_ms(event).unwrap();
            if self
                .participants
                .get(&sid)
                .is_some_and(|(latest, left)| at < *latest || (at == *latest && *left))
            {
                return false;
            }
            if let Some((current_sid, joined, left)) =
                self.current.get(&current_key(event).unwrap())
            {
                if kind == "participant_left" && current_sid != &sid {
                    // This session ended even though another SID owns the
                    // identity. Keep its tombstone so a delayed join cannot
                    // resurrect it after an equal-second leave.
                    self.participants.insert(sid, (at, true));
                    return false;
                }
                if kind == "participant_joined"
                    && (at < *joined
                        || (at == *joined
                            && (sid < *current_sid || (sid == *current_sid && *left))))
                {
                    return false;
                }
            }
        }
        true
    }

    fn applied(&mut self, event: &Value, kind: &str, deadline: u64) {
        let id = event_id(event).unwrap().to_owned();
        self.applied.insert(id.clone(), deadline);
        if matches!(kind, "participant_joined" | "participant_left") {
            let sid = participant_sid(event).unwrap();
            let at = event_time_ms(event).unwrap();
            let left = kind == "participant_left";
            self.participants.insert(sid.clone(), (at, left));
            self.current
                .insert(current_key(event).unwrap(), (sid, at, left));
        }
    }
}

fn event_id(event: &Value) -> Option<&str> {
    event
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
}

/// A verified event is acknowledged only after its effects have succeeded.
pub async fn livekit_webhook(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    require_livekit(&state)?;
    let auth = headers
        .get("authorization")
        .or_else(|| headers.get("x-livekit-signature"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let deadline = verify_webhook_token(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        &body,
        auth,
    )
    .ok_or_else(|| unauthorized("invalid livekit webhook signature"))?;
    let event: Value = serde_json::from_slice(&body)
        .map_err(|_| ApiError::bad_request("invalid livekit webhook JSON"))?;
    if event_id(&event).is_none_or(|id| id.len() > 128) || event_time_ms(&event).is_none() {
        return Err(ApiError::bad_request(
            "missing livekit webhook id or createdAt",
        ));
    }
    let event_kind = event
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    if matches!(event_kind, "participant_joined" | "participant_left")
        && (participant_sid(&event).is_none_or(|sid| sid.len() > 128)
            || room_name(&event).is_none_or(|room| room.len() > 512)
            || participant_identity(&event).is_none_or(|identity| identity.len() > 512))
    {
        return Err(ApiError::bad_request(
            "missing livekit participant event fields",
        ));
    }
    if event_kind == "room_started"
        && (room_name(&event).is_none_or(|room| room.is_empty() || room.len() > 512)
            || room_sid(&event).is_some_and(|sid| sid.len() > 128))
    {
        return Err(ApiError::bad_request("invalid livekit room event fields"));
    }
    tracing::info!(event = event_kind, "livekit webhook received");
    if matches!(event_kind, "room_started" | "room_finished") {
        if let Some(room) = room_name(&event) {
            state.presence.invalidate_room(&room);
        }
    }
    if event_kind == "room_finished" {
        if let (Some(room), Some(sid)) = (
            room_name(&event),
            event
                .get("room")
                .and_then(|room| room.get("sid"))
                .and_then(Value::as_str)
                .filter(|sid| !sid.is_empty()),
        ) {
            super::removal::room_closed(&state.pool, &room, sid).await?;
        }
    }

    // Unknown/track events have no handler; they consume no
    // applied-ID or participant capacity.
    if !matches!(
        event_kind,
        "room_started"
            | "participant_joined"
            | "participant_left"
            | "ingress_started"
            | "ingress_ended"
    ) {
        return Ok(axum::http::StatusCode::OK.into_response());
    }
    let resource = room_name(&event)
        .or_else(|| {
            event
                .get("ingressInfo")?
                .get("ingressId")?
                .as_str()
                .map(str::to_owned)
        })
        .unwrap_or_default();
    let id = event_id(&event).unwrap();
    let (a, b) = (stripe(id), stripe(&resource));
    let _first = state.webhooks.locks[a.min(b)].lock().await;
    let _second = if a != b {
        Some(state.webhooks.locks[a.max(b)].lock().await)
    } else {
        None
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    if now > deadline {
        return Err(unauthorized("expired livekit webhook signature"));
    }
    if !state
        .webhooks
        .state
        .lock()
        .unwrap()
        .prepare(&event, event_kind, deadline, now)
    {
        return Ok(axum::http::StatusCode::OK.into_response());
    }
    // Invalidate before any awaited side effect/resolution. Even a handler
    // retry cannot let a concurrent read certify a room whose event is known.
    if matches!(event_kind, "participant_joined" | "participant_left") {
        if let Some(room) = room_name(&event) {
            state.presence.invalidate_room(&room);
        }
    }
    if matches!(event_kind, "room_started" | "participant_joined") {
        if let Some(room) = room_name(&event) {
            let sid = room_sid(&event).filter(|sid| sid.len() <= 128);
            let due = state
                .webhooks
                .state
                .lock()
                .unwrap()
                .metadata_due(&room, sid);
            if due {
                // Already serialized by this room's webhook stripe. Record only a
                // confirmed same-incarnation write; failures remain retryable.
                let updated =
                    super::moderation::maintenance::initialize_room(state.as_ref(), &room, sid)
                        .await?;
                if let Some(updated) = updated.filter(|updated| sid == Some(updated.as_str())) {
                    state
                        .webhooks
                        .state
                        .lock()
                        .unwrap()
                        .metadata
                        .insert(room, (updated, tokio::time::Instant::now()));
                }
            }
        }
    }
    dispatch(&state, event_kind, &event).await?;
    // Presence and the event sink consume one resolved identity. A world
    // scene's complete name is resolved without guessing a hyphen boundary;
    // store failures leave the webhook unapplied for retry.
    if let (Some(room), Some(identity)) = (room_name(&event), participant_identity(&event)) {
        if matches!(event_kind, "participant_joined" | "participant_left") {
            if let Some(parsed) = crate::rooms::resolve_room_name(state.scenes.as_ref(), &room)
                .await
                .map_err(|error| {
                    tracing::warn!(%error, room, "canonical room identity unavailable");
                    ApiError::http(503, "Room identity is temporarily unavailable")
                })?
            {
                if let Some(sink) = state.room_events.as_ref() {
                    use crate::protocol::bus::room_events::{RoomEvent, RoomEventKind, RoomType};
                    sink.observe_room_event(&RoomEvent {
                        kind: if event_kind == "participant_joined" {
                            RoomEventKind::Joined
                        } else {
                            RoomEventKind::Left
                        } as i32,
                        room,
                        room_type: if parsed.realm.is_world() {
                            RoomType::World
                        } else {
                            RoomType::Scene
                        } as i32,
                        realm_name: parsed
                            .realm
                            .world()
                            .map(|w| w.key())
                            .unwrap_or_else(|| parsed.realm.as_str())
                            .to_owned(),
                        scene_id: parsed
                            .scene_id
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("")
                            .to_owned(),
                        identity,
                        participant_sid: participant_sid(&event).unwrap(),
                        event_time_ms: event_time_ms(&event).unwrap() as u64,
                        event_id: event_id(&event).unwrap().to_owned(),
                    });
                }
            }
        }
    }
    state
        .webhooks
        .state
        .lock()
        .unwrap()
        .applied(&event, event_kind, deadline);
    Ok(axum::http::StatusCode::OK.into_response())
}

async fn dispatch(state: &CommsState, event_kind: &str, event: &Value) -> Result<(), ApiError> {
    if matches!(event_kind, "ingress_started" | "ingress_ended") {
        return super::streaming::ingress_event(state, event_kind, event).await;
    }
    let (Some(room), Some(addr)) = (room_name(event), participant_address(event)) else {
        return Ok(());
    };
    match event_kind {
        "participant_joined" => {
            if let Some(call_id) = private_call_id_from_room_name(&room) {
                handle_private_participant_joined(state, &addr, call_id).await?;
            } else if let Some(community_id) = community_id_from_room_name(&room) {
                handle_community_participant_joined(
                    state,
                    &addr,
                    community_id,
                    participant_sid(event).as_deref(),
                )
                .await?;
            }
        }
        "participant_left" => {
            let reason = DisconnectReason::parse(
                event
                    .get("participant")
                    .and_then(|p| p.get("disconnectReason")),
            );
            if let Some(call_id) = private_call_id_from_room_name(&room) {
                handle_private_participant_left(state, &addr, call_id, reason).await?;
            } else if let Some(community_id) = community_id_from_room_name(&room) {
                handle_community_participant_left(
                    state,
                    &addr,
                    community_id,
                    reason,
                    participant_sid(event).as_deref(),
                    Some(leave_event_time_ms(event)),
                    event_id(event).unwrap(),
                )
                .await?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn room_name(event: &Value) -> Option<String> {
    event
        .get("room")
        .and_then(|r| r.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
}

fn room_sid(event: &Value) -> Option<&str> {
    event
        .get("room")?
        .get("sid")?
        .as_str()
        .filter(|sid| !sid.is_empty())
}

fn participant_identity(event: &Value) -> Option<String> {
    event
        .get("participant")
        .and_then(|p| p.get("identity"))
        .and_then(|i| i.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn participant_address(event: &Value) -> Option<String> {
    let identity = event
        .get("participant")
        .and_then(|p| p.get("identity"))
        .and_then(|i| i.as_str())?;
    Some(
        address_from_identity(identity)
            .unwrap_or_else(|| identity.to_lowercase().chars().take(42).collect::<String>()),
    )
}

fn participant_sid(event: &Value) -> Option<String> {
    event
        .get("participant")
        .and_then(|p| p.get("sid"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn event_time_ms(event: &Value) -> Option<i64> {
    let created_at = event
        .get("createdAt")
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(0);
    if created_at > 0 {
        if created_at < 1_000_000_000_000 {
            Some(created_at * 1000)
        } else {
            Some(created_at)
        }
    } else {
        None
    }
}

fn leave_event_time_ms(event: &Value) -> i64 {
    event_time_ms(event).unwrap_or_else(now_ms)
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use base64::Engine;
    use base64::engine::general_purpose::{STANDARD as B64_STANDARD, URL_SAFE_NO_PAD};
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::{Digest, Sha256};

    /// Mints what LiveKit actually sends: an HS256 JWT over the body digest,
    /// bounded by `exp`/`nbf`. Sign with the wrong `secret` to mint the
    /// token a 401 needs.
    pub fn webhook_jwt(body: &[u8], key: &str, secret: &str) -> String {
        let now = chrono::Utc::now().timestamp() as u64;
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
        let claims = serde_json::json!({ "iss": key, "nbf": now, "exp": now + 300, "sha256": B64_STANDARD.encode(Sha256::digest(body)) });
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let signing_input = format!("{header}.{payload}");
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signing_input.as_bytes());
        let sig = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        format!("{signing_input}.{sig}")
    }
}

#[cfg(test)]
mod initial_metadata_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn participant_event(id: &str, kind: &str, sid: &str, at: i64) -> Value {
        serde_json::json!({"id": id, "event": kind, "createdAt": at,
            "room": {"name": "world-test.dcl.eth"},
            "participant": {"sid": sid, "identity": "wallet"}})
    }

    #[test]
    fn retries_ordering_and_equal_second_leave_precedence() {
        let mut state = WebhookState::default();
        let join = participant_event("j", "participant_joined", "PA_1", 100);
        assert!(state.prepare(&join, "participant_joined", 500, 100));
        assert!(state.prepare(&join, "participant_joined", 500, 100));
        state.applied(&join, "participant_joined", 500);
        assert!(!state.prepare(&join, "participant_joined", 600, 100));
        assert_eq!(state.applied.get("j"), Some(&600));
        let old_leave = participant_event("old", "participant_left", "PA_1", 99);
        assert!(!state.prepare(&old_leave, "participant_left", 500, 100));
        let leave = participant_event("l", "participant_left", "PA_1", 100);
        assert!(state.prepare(&leave, "participant_left", 500, 100));
        state.applied(&leave, "participant_left", 500);
        let same_second_join = participant_event("j2", "participant_joined", "PA_1", 100);
        assert!(!state.prepare(&same_second_join, "participant_joined", 500, 100));
        let newer = participant_event("newer", "participant_joined", "PA_2", 101);
        assert!(state.prepare(&newer, "participant_joined", 500, 100));
        state.applied(&newer, "participant_joined", 500);
        let delayed_old = participant_event("old-sid", "participant_left", "PA_1", 102);
        assert!(!state.prepare(&delayed_old, "participant_left", 500, 100));
    }

    #[test]
    fn equal_second_same_sid_leave_wins_both_delivery_orders() {
        for reverse in [false, true] {
            let mut state = WebhookState::default();
            let mut events = [
                participant_event("join", "participant_joined", "PA_same", 100),
                participant_event("leave", "participant_left", "PA_same", 100),
            ];
            if reverse {
                events.reverse();
            }
            for event in events {
                let kind = event["event"].as_str().unwrap();
                if state.prepare(&event, kind, 500, 100) {
                    state.applied(&event, kind, 500);
                }
            }
            assert_eq!(state.participants.get("PA_same"), Some(&(100_000, true)));
        }
    }

    #[test]
    fn equal_second_session_choice_is_deterministic_in_both_orders() {
        for reverse in [false, true] {
            let mut state = WebhookState::default();
            let mut events = [
                participant_event("a", "participant_joined", "PA_A", 100),
                participant_event("b", "participant_joined", "PA_B", 100),
            ];
            if reverse {
                events.reverse();
            }
            for event in events {
                if state.prepare(&event, "participant_joined", 500, 100) {
                    state.applied(&event, "participant_joined", 500);
                }
            }
            let key =
                current_key(&participant_event("a", "participant_joined", "PA_A", 100)).unwrap();
            assert_eq!(state.current.get(&key).unwrap().0, "PA_B");
            let old_leave = participant_event("left-a", "participant_left", "PA_A", 100);
            assert!(!state.prepare(&old_leave, "participant_left", 500, 100));
        }
    }

    #[test]
    fn bounded_rolling_history_keeps_unrelated_lifecycle_events_progressing() {
        let mut state = WebhookState::default();
        state.applied.capacity = 8;
        state.participants.capacity = 8;
        state.current.capacity = 8;
        for n in 0..1000 {
            let mut event = participant_event(
                &format!("j{n}"),
                "participant_joined",
                &format!("PA_{n}"),
                100 + n,
            );
            event["participant"]["identity"] = Value::String(format!("wallet{n}"));
            assert!(state.prepare(&event, "participant_joined", 1000, 100));
            state.applied(&event, "participant_joined", 1000);
            let leave = serde_json::json!({"id": format!("l{n}"), "createdAt": 101+n,
                "room": event["room"], "participant": event["participant"]});
            assert!(state.prepare(&leave, "participant_left", 1000, 100));
            state.applied(&leave, "participant_left", 1000);
            assert!(state.applied.values.len() <= 8);
            assert!(state.participants.values.len() <= 8);
            assert!(state.current.values.len() <= 8);
        }
        let ingress = serde_json::json!({"id": "ingress"});
        assert!(state.prepare(&ingress, "ingress_started", 2000, 1001));
        state.applied(&ingress, "ingress_started", 2000);
        assert!(!state.prepare(&ingress, "ingress_started", 2000, 2000));
        assert!(state.prepare(&ingress, "ingress_started", 3000, 2001));
    }

    #[test]
    fn webhook_field_readers() {
        let event = serde_json::json!({
            "event": "participant_left",
            "room": { "name": "voice-chat-private-x" },
            "participant": { "identity": "0x1234567890ABCDEF1234567890abcdef12345678:wss", "sid": "PA_1", "disconnectReason": "CLIENT_INITIATED" },
            "createdAt": 1_718_900_000,
        });
        assert_eq!(room_name(&event).as_deref(), Some("voice-chat-private-x"));
        assert_eq!(
            participant_address(&event).as_deref(),
            Some("0x1234567890abcdef1234567890abcdef12345678")
        );
        assert_eq!(participant_sid(&event).as_deref(), Some("PA_1"));
        assert_eq!(leave_event_time_ms(&event), 1_718_900_000_000);
        let ms_event = serde_json::json!({ "createdAt": "1718900000123" });
        assert_eq!(leave_event_time_ms(&ms_event), 1_718_900_000_123);
        assert!(leave_event_time_ms(&serde_json::json!({})) > 1_700_000_000_000);
    }
}
