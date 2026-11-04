//! The messages the Pulse process and `catalyst serve` (the gatekeeper and the `/ws`
//! connector) exchange over core NATS, with their subjects, wire forms, builders and
//! parsers defined once. A producer builds a message from these types and a consumer
//! parses with them: no service names a subject or decodes a payload of its own.
//!
//! | Message | Subject | Producer -> consumer | Type |
//! |---|---|---|---|
//! | cluster change | `peer.{wallet}.cluster_change` | Pulse -> gatekeeper | [`cluster::ClusterChange`] |
//! | cluster lookup | `peer.{wallet}.cluster_lookup`, reply on a private `_INBOX.` subject | gatekeeper -> Pulse | [`cluster::ClusterLookup`] |
//! | islands snapshot | `engine.islands` | Pulse -> connector | [`topology`] |
//! | discovery beacon | `engine.discovery` | Pulse -> connector | [`topology`] |
//! | control position | `archipelago.control.position.v1` | connector -> Pulse | [`control_position::ControlPosition`] |
//! | room event | `comms.room_events.v1` | gatekeeper -> `multiplayer-hosting` | [`room_events::RoomEvent`] |
//!
//! A session's connect and its island assignment are not messages: inside `serve` the
//! connector and the gatekeeper's island subscriber exchange them by call. Their former
//! subjects (`peer.{wallet}.connect`, `peer.{wallet}.disconnect`,
//! `engine.peer.{wallet}.island_changed[.{session}]`) are retired.
//!
//! Delivery is core NATS, at most once, for every subject, so no message is a promise.
//! Each is a hint that the next message or a read recovers: the lookup for a cluster
//! change, the next snapshot or heartbeat, the next position, `GET /scene-participants`
//! for a room event. Subjects are internal to the monorepo, so a payload changes with both
//! of its ends in one commit and has no compatibility form.
//!
//! Wallets and sessions travel as lower-case `0x` and 40 hex digits. Every builder
//! lower-cases; every parser accepts upper-case hex and a `0X` prefix and hands back the
//! lower-case form ([`wallet_token`]). The cluster parsers refuse a subject of another shape
//! and a payload over [`MAX_PAYLOAD_BYTES`]; a control position is held to
//! [`control_position::MAX_BYTES`] (640) and a room event to [`room_events::MAX_BYTES`]
//! (4 KiB). The complete islands frame is capped at the broker default 1 MiB.

use std::time::Duration;

pub mod cluster;
pub mod control_position;
pub mod room_events;
pub mod topology;

/// The most a cluster message may carry. A lookup reply is held to the same size
/// ([`cluster::MAX_REPLY_BYTES`]).
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// The longest subject a consumer parses; checked before the subject is split.
pub const MAX_SUBJECT_BYTES: usize = 256;

/// The deadline of a lookup exchange: Pulse bounds its subscription and each reply publish by
/// it, and a requester reads silence past it as "unknown".
pub const REQUEST_DEADLINE: Duration = Duration::from_secs(2);

/// Why a bus message was refused. A refused message costs only itself: the consumer drops it
/// and carries on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BusError {
    /// The subject is not one this message uses: another shape, or an address token that is
    /// not an address (a lookup's session, which travels as its payload, counts as one).
    #[error("subject is not one this message uses")]
    Subject,
    #[error("message is over its size limit")]
    TooLarge,
    #[error("payload is not a valid message: {0}")]
    Payload(#[from] prost::DecodeError),
}

/// A wallet or session token in the form every bus consumer works with: ASCII-lower-cased
/// `0x` followed by exactly 40 hex digits. Accepts what the consumers it replaced accepted,
/// upper-case digits and a `0X` prefix among them.
pub fn wallet_token(token: &str) -> Option<String> {
    if token.len() != 42 {
        return None;
    }
    let token = token.to_ascii_lowercase();
    let valid = token.strip_prefix("0x").is_some_and(|digits| {
        digits
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    });
    valid.then_some(token)
}

/// The wallet of a `peer.{wallet}.{event}` subject: exactly those three tokens, the middle
/// one an address.
fn peer_subject_wallet(subject: &str, event: &str) -> Option<String> {
    let mut tokens = subject.split('.');
    let (Some("peer"), Some(wallet), Some(last), None) =
        (tokens.next(), tokens.next(), tokens.next(), tokens.next())
    else {
        return None;
    };
    if last != event {
        return None;
    }
    wallet_token(wallet)
}

#[cfg(test)]
mod tests {
    use super::wallet_token;
    use crate::protocol::bus::control_position::{ControlPosition, MAX_BYTES, SUBJECT};

    #[test]
    fn wallet_tokens_are_lower_cased_addresses() {
        let lower = format!("0x{}", "ab".repeat(20));
        for accepted in [
            lower.clone(),
            lower.to_uppercase().replacen("0X", "0x", 1),
            lower.replacen("0x", "0X", 1),
            format!("0X{}", "AB".repeat(20)),
        ] {
            assert_eq!(wallet_token(&accepted).as_deref(), Some(lower.as_str()));
        }
        // Forty-two bytes, but the last character takes two of them.
        let multibyte = format!("0x{}{}", &lower[2..40], char::from(0xe9_u8));
        assert_eq!(multibyte.len(), 42);
        for refused in [
            String::new(),
            lower[..41].to_string(),
            format!("{lower}0"),
            lower[2..].to_string(),
            format!("1x{}", &lower[2..]),
            format!("0x{}g", &lower[2..41]),
            format!(" {}", &lower[..41]),
            multibyte,
        ] {
            assert_eq!(wallet_token(&refused), None, "{refused:?}");
        }
    }

    /// Recorded from `ControlPosition::encode` before the module moved under `bus`: the presence
    /// flag, the wallet and session (20 bytes each), the epoch and sequence (u64 little-endian),
    /// the audience and realm (u16 little-endian length, then UTF-8), then the position (three
    /// f32 little-endian).
    #[test]
    fn control_position_bytes_are_pinned() {
        let position = ControlPosition {
            audience: "realm.example".into(),
            wallet: format!("0x{}", "01".repeat(20)),
            session: format!("0x{}", "02".repeat(20)),
            epoch: 1,
            sequence: 2,
            realm: "realm".into(),
            position: Some([1.0, 2.0, -3.0]),
        };
        let bytes = position.encode().unwrap();
        assert_eq!(
            hex::encode(&bytes),
            concat!(
                "01010101010101010101010101010101010101010102020202020202020202020202",
                "02020202020202010000000000000002000000000000000d007265616c6d2e657861",
                "6d706c6505007265616c6d0000803f00000040000040c0"
            )
        );
        assert_eq!(ControlPosition::decode(&bytes), Some(position));
        assert!(bytes.len() <= MAX_BYTES);
        assert_eq!(SUBJECT, "archipelago.control.position.v1");
    }
}
