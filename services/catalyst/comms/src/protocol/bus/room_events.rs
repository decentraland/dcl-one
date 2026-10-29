//! The room event: one LiveKit join or leave in a scene or world room, which the gatekeeper's
//! LiveKit webhook publishes for the `multiplayer-hosting` scheduler.
//!
//! Its nine fields are the ones that scheduler reads, plus the session, the time and the id it
//! uses to order and deduplicate events; they are not the set of fields the gatekeeper's former
//! SNS events carried.
//!
//! Hand-written rather than generated, in the style of the generated messages; the `.proto`
//! beside this file (`catalyst.comms.bus.v1`) documents it. Delivery is at most once: the
//! scheduler reconciles against `GET /scene-participants`, so a lost event is a late start,
//! not a wrong one.

use super::BusError;

pub const SUBJECT: &str = "comms.room_events.v1";

/// A room event is a handful of short strings; a longer one is not ours. The publisher's
/// [`RoomEvent::encode`] and the consumer's [`RoomEvent::decode`] both refuse it.
pub const MAX_BYTES: usize = 4096;

// Retired, and never to be given another meaning: fields 5 (`is_world`), 7 (`user_address`),
// 12 (`island_name`), 13 (`community_id`) and 14 (`voice_chat_id`) of the message, and the room
// types 3 (`ISLAND`), 4 (`COMMUNITY_VOICE`) and 5 (`PRIVATE_VOICE`). prost's derive has no
// `reserved`; the `.proto` beside this file reserves them. A decoder skips the fields and reads
// the types as `Unknown`.
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RoomEvent {
    #[prost(enumeration = "RoomEventKind", tag = "1")]
    pub kind: i32,
    /// The LiveKit room name.
    #[prost(string, tag = "2")]
    pub room: ::prost::alloc::string::String,
    #[prost(enumeration = "RoomType", tag = "3")]
    pub room_type: i32,
    /// The world name for a world room, else the realm.
    #[prost(string, tag = "4")]
    pub realm_name: ::prost::alloc::string::String,
    /// Empty when the room has no scene.
    #[prost(string, tag = "6")]
    pub scene_id: ::prost::alloc::string::String,
    /// The LiveKit identity as is: a wallet, `authoritative-server`, a streamer's.
    #[prost(string, tag = "8")]
    pub identity: ::prost::alloc::string::String,
    /// LiveKit's participant sid: the participant's session in the room.
    #[prost(string, tag = "9")]
    pub participant_sid: ::prost::alloc::string::String,
    /// LiveKit's `createdAt`, in milliseconds since the epoch.
    #[prost(uint64, tag = "10")]
    pub event_time_ms: u64,
    /// LiveKit's webhook event id.
    #[prost(string, tag = "11")]
    pub event_id: ::prost::alloc::string::String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RoomEventKind {
    Unspecified = 0,
    Joined = 1,
    Left = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RoomType {
    Unknown = 0,
    Scene = 1,
    World = 2,
}

impl RoomEvent {
    /// Refuses more than [`MAX_BYTES`], so the publisher never sends what [`Self::decode`] would
    /// refuse.
    pub fn encode(&self) -> Result<Vec<u8>, BusError> {
        if ::prost::Message::encoded_len(self) > MAX_BYTES {
            return Err(BusError::TooLarge);
        }
        Ok(::prost::Message::encode_to_vec(self))
    }

    /// Refuses more than [`MAX_BYTES`]; fields it does not know are skipped, so a newer
    /// publisher can add one without breaking an older consumer.
    pub fn decode(payload: &[u8]) -> Result<Self, BusError> {
        if payload.len() > MAX_BYTES {
            return Err(BusError::TooLarge);
        }
        Ok(<Self as ::prost::Message>::decode(payload)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::bus::BusError;

    fn joined() -> RoomEvent {
        RoomEvent {
            kind: RoomEventKind::Joined as i32,
            room: "world-example.dcl.eth".into(),
            room_type: RoomType::World as i32,
            realm_name: "example.dcl.eth".into(),
            scene_id: "bafyexamplescene".into(),
            identity: format!("0x{}", "AB".repeat(20)),
            participant_sid: "PA_example".into(),
            event_time_ms: 1_700_000_000_123,
            event_id: "EV_example".into(),
        }
    }

    /// Written from the protobuf wire rules (key, length, bytes; a varint for the numbers), then
    /// confirmed against prost's output once, and kept.
    #[test]
    fn room_event_bytes_are_pinned() {
        assert_eq!(
            hex::encode(joined().encode().unwrap()),
            concat!(
                "0801",
                "1215776f726c642d6578616d706c652e64636c2e657468",
                "1802",
                "220f6578616d706c652e64636c2e657468",
                "3210626166796578616d706c657363656e65",
                "422a3078414241424142414241424142414241424142414241424142414241424142",
                "41424142414241424142",
                "4a0a50415f6578616d706c65",
                "50fbd095ffbc31",
                "5a0a45565f6578616d706c65"
            )
        );
    }

    #[test]
    fn room_event_round_trips() {
        let event = joined();
        assert_eq!(RoomEvent::decode(&event.encode().unwrap()).unwrap(), event);
        let left = RoomEvent {
            kind: RoomEventKind::Left as i32,
            room_type: RoomType::Scene as i32,
            ..RoomEvent::default()
        };
        let decoded = RoomEvent::decode(&left.encode().unwrap()).unwrap();
        assert_eq!(decoded, left);
        assert_eq!(decoded.kind(), RoomEventKind::Left);
        assert_eq!(decoded.room_type(), RoomType::Scene);
    }

    #[test]
    fn room_event_over_4_kib_is_refused() {
        // Field 2 with a two-byte length: three bytes of framing, then the room name.
        let at_limit = RoomEvent {
            room: "r".repeat(MAX_BYTES - 3),
            ..RoomEvent::default()
        };
        let bytes = at_limit.encode().unwrap();
        assert_eq!(bytes.len(), MAX_BYTES);
        assert_eq!(RoomEvent::decode(&bytes).unwrap(), at_limit);

        // One byte more is refused by the publisher's `encode`.
        let over = RoomEvent {
            room: "r".repeat(MAX_BYTES - 2),
            ..RoomEvent::default()
        };
        assert!(matches!(over.encode(), Err(BusError::TooLarge)));

        // And by the consumer's `decode`: key 0x12 (field 2), length 4094 as the varint
        // 0xfe 0x1f, then the 4094 bytes of the room name.
        let mut bytes = vec![0x12, 0xfe, 0x1f];
        bytes.resize(3 + (MAX_BYTES - 2), b'r');
        assert_eq!(bytes.len(), MAX_BYTES + 1);
        assert!(matches!(RoomEvent::decode(&bytes), Err(BusError::TooLarge)));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let event = joined();
        let mut bytes = event.encode().unwrap();
        // Field 20, wire type 2 (length-delimited): key 0xa2 0x01, length 3, "new".
        bytes.extend_from_slice(&[0xa2, 0x01, 0x03, b'n', b'e', b'w']);
        assert_eq!(RoomEvent::decode(&bytes).unwrap(), event);
    }

    /// The event as a publisher sent it with fourteen fields (the vector pinned before the five
    /// went): fields 5, 7, 12, 13 and 14 are skipped as unknown, and the nine kept ones read as
    /// before.
    #[test]
    fn a_fourteen_field_event_decodes_to_its_nine_fields() {
        let fourteen_fields = hex::decode(concat!(
            "0801",
            "1215776f726c642d6578616d706c652e64636c2e657468",
            "1802",
            "220f6578616d706c652e64636c2e657468",
            "2801",
            "3210626166796578616d706c657363656e65",
            "3a2a3078616261626162616261626162616261626162616261626162616261626162",
            "61626162616261626162",
            "422a3078414241424142414241424142414241424142414241424142414241424142",
            "41424142414241424142",
            "4a0a50415f6578616d706c65",
            "50fbd095ffbc31",
            "5a0a45565f6578616d706c65",
            "620969736c616e642d6331",
            "6a11636f6d6d756e6974792d6578616d706c65",
            "720d766f6963652d6578616d706c65"
        ))
        .unwrap();
        let decoded = RoomEvent::decode(&fourteen_fields).unwrap();
        assert_eq!(decoded, joined());
        // Nothing of the skipped fields is carried on.
        assert_eq!(decoded.encode().unwrap(), joined().encode().unwrap());
    }

    /// Room types 3 (island), 4 (community voice) and 5 (private voice) are retired: a sender
    /// that still writes one is read, and the type is `Unknown`.
    #[test]
    fn a_retired_room_type_reads_as_unknown() {
        for retired in [3_u8, 4, 5] {
            // Field 3 (room_type), varint.
            let event = RoomEvent::decode(&[0x18, retired]).unwrap();
            assert_eq!(event.room_type, i32::from(retired));
            assert_eq!(event.room_type(), RoomType::Unknown);
        }
    }

    #[test]
    fn bytes_that_are_not_a_room_event_are_a_payload_error() {
        assert!(matches!(
            RoomEvent::decode(&[0xff]),
            Err(BusError::Payload(_))
        ));
    }

    #[test]
    fn the_subject_names_the_version() {
        assert_eq!(SUBJECT, "comms.room_events.v1");
    }
}
