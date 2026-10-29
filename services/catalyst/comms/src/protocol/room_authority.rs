//! The room authority between Pulse's application relay and the comms
//! gatekeeper, one copy for both sides: `POST /internal/pulse/room-authority/v1`
//! ([`AUTHORITY_PATH`]), the protobuf frames Pulse's client
//! (`pulse::application_relay::http::HttpAuthority`) sends and the gatekeeper
//! (`gatekeeper::admission::relay_authority`) answers, the limits both hold,
//! and the verified admission projection returned by the gatekeeper.
//! Proof verification and LiveKit secrets belong exclusively to the gatekeeper.
pub const AUTHORITY_PATH: &str = "/internal/pulse/room-authority/v1";
/// Pulse sends no larger request and reads no larger response; the gatekeeper
/// reads no larger request.
pub const MAX_BODY: usize = 16_384;
/// The most leases each side tracks at once (Pulse's client registry, the
/// gatekeeper's lease store).
pub const MAX_LEASES: usize = 16_384;
pub const PROOF_DOMAIN: &[u8] = b"dcl-pulse-application-relay-v1\0";
pub const MAX_CLAIMS_BYTES: usize = 3072;

/// A join, or the renewal or release of a lease.
#[derive(Clone, PartialEq, prost::Message)]
pub struct AuthorityRequest {
    #[prost(oneof = "authority_request::Operation", tags = "1, 2, 3")]
    pub operation: Option<authority_request::Operation>,
}

pub mod authority_request {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Operation {
        #[prost(message, tag = "1")]
        Join(super::Join),
        #[prost(message, tag = "2")]
        Renew(super::Context),
        #[prost(message, tag = "3")]
        Release(super::Context),
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Join {
    #[prost(bytes = "vec", tag = "1")]
    pub pulse_instance: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub connection_generation: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub wallet: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub session: Vec<u8>,
    #[prost(string, tag = "5")]
    pub header_payload: String,
    #[prost(bytes = "vec", tag = "6")]
    pub nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "7")]
    pub proof: Vec<u8>,
}

/// The lease a renewal or a release names, with the Pulse instance and
/// connection generation it is bound to.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Context {
    #[prost(bytes = "vec", tag = "1")]
    pub pulse_instance: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub connection_generation: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub lease_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub authority_incarnation: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AuthorityResponse {
    #[prost(oneof = "authority_response::Result", tags = "1, 2, 3")]
    pub result: Option<authority_response::Result>,
}

pub mod authority_response {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Lease(super::Lease),
        #[prost(message, tag = "2")]
        Denied(super::Denied),
        #[prost(message, tag = "3")]
        Released(super::Released),
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Lease {
    #[prost(bytes = "vec", tag = "1")]
    pub lease_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub authority_incarnation: Vec<u8>,
    #[prost(uint32, tag = "3")]
    pub ttl_ms: u32,
    #[prost(message, optional, tag = "4")]
    pub admitted: Option<AdmittedRoom>,
}

/// Only the room authority derives this projection from verified claims.
#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AdmittedRoom {
    #[prost(string, tag = "1")]
    pub room: String,
    #[prost(string, tag = "2")]
    pub identity: String,
    #[prost(bool, optional, tag = "3")]
    pub is_guest: Option<bool>,
}

impl AdmittedRoom {
    pub fn is_valid(&self) -> bool {
        !self.room.is_empty()
            && self.room.len() <= 256
            && !self.room.chars().any(char::is_control)
            && (self.identity == "authoritative-server" || address_bytes(&self.identity).is_some())
            && self.is_guest.is_some()
    }
}

impl Lease {
    pub fn is_valid(&self) -> bool {
        self.lease_id.len() == 16
            && self.authority_incarnation.len() == 16
            && (1..=2000).contains(&self.ttl_ms)
            && self.admitted.as_ref().is_some_and(AdmittedRoom::is_valid)
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Denied {
    #[prost(enumeration = "denied::Reason", tag = "1")]
    pub reason: i32,
}

pub mod denied {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
    #[repr(i32)]
    pub enum Reason {
        Unspecified = 0,
        NotAuthorized = 1,
        Expired = 2,
        Capacity = 3,
        Unavailable = 4,
        Invalid = 5,
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Released {}

pub fn address_bytes(value: &str) -> Option<[u8; 20]> {
    let value = value.strip_prefix("0x")?;
    if value.len() != 40 || !value.is_ascii() {
        return None;
    }
    let mut result = [0; 20];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_room_codec_preserves_existing_frame_body_and_bus_limits() {
        use crate::protocol::pulse::decentraland::pulse::{
            ApplicationJoin, ClientMessage, client_message,
        };
        assert_eq!(MAX_CLAIMS_BYTES, 3072);
        let application = ClientMessage {
            message: Some(client_message::Message::ApplicationJoin(ApplicationJoin {
                request_id: u32::MAX,
                jwt_header_payload: "h".repeat(MAX_CLAIMS_BYTES),
                proof: vec![0; 32],
            })),
        };
        assert_eq!(::prost::Message::encoded_len(&application), 3118);
        let authority = AuthorityRequest {
            operation: Some(authority_request::Operation::Join(Join {
                pulse_instance: vec![0; 16],
                connection_generation: vec![0; 16],
                wallet: vec![0; 20],
                session: vec![0; 20],
                header_payload: "h".repeat(MAX_CLAIMS_BYTES),
                nonce: vec![0; 32],
                proof: vec![0; 32],
            })),
        };
        assert_eq!(::prost::Message::encoded_len(&authority), 3226);
        assert_eq!(MAX_BODY, 16384);
        assert_eq!(MAX_LEASES, 16384);
        let mut largest_event = 0;
        for world_bytes in 1..=504 {
            let world = crate::world_identity::WorldName::parse(&"w".repeat(world_bytes)).unwrap();
            let scene =
                crate::world_identity::SceneId::parse(&"\u{212a}".repeat(505 - world_bytes))
                    .unwrap();
            let legacy = format!("scene-{}-{}", world.key(), scene.as_str().to_lowercase());
            assert_eq!(legacy.len(), 512);
            let room = crate::rooms::worlds_service_scene_room(&world, &scene);
            assert!(room.len() <= legacy.len());
            let event = crate::protocol::bus::room_events::RoomEvent {
                kind: 1,
                room,
                room_type: 2,
                realm_name: world.key().into(),
                scene_id: scene.as_str().into(),
                identity: "i".repeat(512),
                participant_sid: "s".repeat(128),
                event_id: "e".repeat(128),
                event_time_ms: u64::MAX,
            };
            largest_event = largest_event.max(
                event
                    .encode()
                    .expect("all formerly valid tuples fit the unchanged event limit")
                    .len(),
            );
        }
        assert!(largest_event <= crate::protocol::bus::room_events::MAX_BYTES);
        assert_eq!(crate::protocol::bus::room_events::MAX_BYTES, 4096);
    }

    const WALLET: &str = "0x1111111111111111111111111111111111111111";
    // One test per frame, pinning today's bytes. Before system-change-3 merged
    // them, Pulse's client declaration and the gatekeeper's server declaration
    // each encoded and decoded exactly these vectors (the renewal and release
    // context was one Pulse type and two gatekeeper types, `Renew`, `Release`).

    fn vector(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    /// `value` encodes to exactly `expected`, and `expected` decodes to exactly `value`.
    fn pins<M: prost::Message + Default + PartialEq + std::fmt::Debug>(value: M, expected: &[u8]) {
        assert_eq!(value.encode_to_vec(), expected);
        assert_eq!(M::decode(expected).unwrap(), value);
    }

    fn context() -> Context {
        Context {
            pulse_instance: vec![1; 16],
            connection_generation: vec![2; 16],
            lease_id: vec![7; 16],
            authority_incarnation: vec![8; 16],
        }
    }

    /// The operation's tag and length, then the context: `pulse_instance` 1,
    /// `connection_generation` 2, `lease_id` 3, `authority_incarnation` 4 (bytes).
    fn context_vector(operation_tag: u8) -> Vec<u8> {
        vector(&[
            &[operation_tag, 0x48],
            &[0x0a, 0x10],
            &[1; 16],
            &[0x12, 0x10],
            &[2; 16],
            &[0x1a, 0x10],
            &[7; 16],
            &[0x22, 0x10],
            &[8; 16],
        ])
    }

    /// `operation` 1, `Join`: `pulse_instance` 1, `connection_generation` 2,
    /// `wallet` 3, `session` 4 (bytes), `header_payload` 5 (string), `nonce` 6,
    /// `proof` 7 (bytes).
    #[test]
    fn join_frame_pins_todays_bytes() {
        pins(
            AuthorityRequest {
                operation: Some(authority_request::Operation::Join(Join {
                    pulse_instance: vec![1; 16],
                    connection_generation: vec![2; 16],
                    wallet: vec![0x11; 20],
                    session: vec![0x22; 20],
                    header_payload: "h.p".into(),
                    nonce: vec![3; 32],
                    proof: vec![4; 32],
                })),
            },
            &vector(&[
                &[0x0a, 0x99, 0x01],
                &[0x0a, 0x10],
                &[1; 16],
                &[0x12, 0x10],
                &[2; 16],
                &[0x1a, 0x14],
                &[0x11; 20],
                &[0x22, 0x14],
                &[0x22; 20],
                &[0x2a, 0x03],
                b"h.p",
                &[0x32, 0x20],
                &[3; 32],
                &[0x3a, 0x20],
                &[4; 32],
            ]),
        );
    }

    /// `operation` 2, `Renew`: a [`Context`].
    #[test]
    fn renew_frame_pins_todays_bytes() {
        pins(
            AuthorityRequest {
                operation: Some(authority_request::Operation::Renew(context())),
            },
            &context_vector(0x12),
        );
    }

    /// `operation` 3, `Release`: the same [`Context`].
    #[test]
    fn release_frame_pins_todays_bytes() {
        pins(
            AuthorityRequest {
                operation: Some(authority_request::Operation::Release(context())),
            },
            &context_vector(0x1a),
        );
    }

    /// `result` 1, `Lease`: `lease_id` 1, `authority_incarnation` 2 (bytes),
    /// `ttl_ms` 3 (uint32).
    #[test]
    fn lease_frame_pins_todays_bytes() {
        use prost::Message;
        let bare = vector(&[
            &[0x0a, 0x27, 0x0a, 0x10],
            &[7; 16],
            &[0x12, 0x10],
            &[8; 16],
            &[0x18, 0xd0, 0x0f],
        ]);
        let response = AuthorityResponse::decode(bare.as_slice()).unwrap();
        let Some(authority_response::Result::Lease(lease)) = response.result else {
            panic!()
        };
        assert!(
            !lease.is_valid(),
            "legacy wire decode is not successful admission"
        );
        pins(
            AuthorityResponse {
                result: Some(authority_response::Result::Lease(Lease {
                    lease_id: vec![7; 16],
                    authority_incarnation: vec![8; 16],
                    ttl_ms: 2000,
                    admitted: None,
                })),
            },
            &vector(&[
                &[0x0a, 0x27],
                &[0x0a, 0x10],
                &[7; 16],
                &[0x12, 0x10],
                &[8; 16],
                &[0x18, 0xd0, 0x0f],
            ]),
        );
    }

    #[test]
    fn admitted_projection_pins_bytes_and_requires_explicit_guest() {
        use prost::Message;
        let admitted = AdmittedRoom {
            room: "world-a.eth".into(),
            identity: WALLET.into(),
            is_guest: Some(true),
        };
        let lease = Lease {
            lease_id: vec![7; 16],
            authority_incarnation: vec![8; 16],
            ttl_ms: 2000,
            admitted: Some(admitted.clone()),
        };
        assert!(lease.is_valid());
        let bytes = vector(&[
            &[0x0a, 0x64, 0x0a, 0x10],
            &[7; 16],
            &[0x12, 0x10],
            &[8; 16],
            &[0x18, 0xd0, 0x0f, 0x22, 0x3b, 0x0a, 0x0b],
            b"world-a.eth",
            &[0x12, 0x2a],
            WALLET.as_bytes(),
            &[0x18, 0x01],
        ]);
        assert_eq!(bytes.len(), 102);
        pins(
            AuthorityResponse {
                result: Some(authority_response::Result::Lease(lease.clone())),
            },
            &bytes,
        );
        let mut false_guest = admitted.clone();
        false_guest.is_guest = Some(false);
        assert!(false_guest.encode_to_vec().ends_with(&[0x18, 0x00]));
        assert_eq!(
            AdmittedRoom::decode(false_guest.encode_to_vec().as_slice()).unwrap(),
            false_guest
        );
        let mut unknown = bytes.clone();
        unknown.extend_from_slice(&[0x20, 0x01]);
        assert_eq!(
            AuthorityResponse::decode(unknown.as_slice()).unwrap(),
            AuthorityResponse {
                result: Some(authority_response::Result::Lease(lease.clone()))
            }
        );
        let mut invalid = lease.clone();
        invalid.admitted = None;
        assert!(!invalid.is_valid());
        for projection in [
            AdmittedRoom {
                is_guest: None,
                ..admitted.clone()
            },
            AdmittedRoom {
                identity: "invalid".into(),
                ..admitted.clone()
            },
            AdmittedRoom {
                room: "x".repeat(257),
                ..admitted.clone()
            },
            AdmittedRoom {
                room: "a\n".into(),
                ..admitted.clone()
            },
        ] {
            invalid = lease.clone();
            invalid.admitted = Some(projection);
            assert!(!invalid.is_valid());
        }
        for ttl in [0, 2001] {
            invalid = lease.clone();
            invalid.ttl_ms = ttl;
            assert!(!invalid.is_valid());
        }
        invalid = lease.clone();
        invalid.lease_id.pop();
        assert!(!invalid.is_valid());
        invalid = lease;
        invalid.authority_incarnation.pop();
        assert!(!invalid.is_valid());
    }

    /// `result` 2, `Denied`: `reason` 1, a varint (`Unspecified`, the default,
    /// leaves the message empty).
    #[test]
    fn denied_frame_pins_todays_bytes_for_every_reason() {
        use super::denied::Reason;
        for (reason, expected) in [
            (Reason::Unspecified, vector(&[&[0x12, 0x00]])),
            (Reason::NotAuthorized, vector(&[&[0x12, 0x02, 0x08, 0x01]])),
            (Reason::Expired, vector(&[&[0x12, 0x02, 0x08, 0x02]])),
            (Reason::Capacity, vector(&[&[0x12, 0x02, 0x08, 0x03]])),
            (Reason::Unavailable, vector(&[&[0x12, 0x02, 0x08, 0x04]])),
            (Reason::Invalid, vector(&[&[0x12, 0x02, 0x08, 0x05]])),
        ] {
            pins(
                AuthorityResponse {
                    result: Some(authority_response::Result::Denied(Denied {
                        reason: reason as i32,
                    })),
                },
                &expected,
            );
        }
    }

    /// An undefined reason decodes as its raw value: neither side rejects it
    /// (Pulse reads only `Lease`).
    #[test]
    fn undefined_denied_reason_decodes_as_its_raw_value() {
        use prost::Message as _;
        assert_eq!(
            AuthorityResponse::decode(&[0x12, 0x02, 0x08, 0x07][..]).unwrap(),
            AuthorityResponse {
                result: Some(authority_response::Result::Denied(Denied { reason: 7 })),
            }
        );
    }

    /// `result` 3, `Released`: an empty message.
    #[test]
    fn released_frame_pins_todays_bytes() {
        pins(
            AuthorityResponse {
                result: Some(authority_response::Result::Released(Released {})),
            },
            &[0x1a, 0x00],
        );
    }
}
