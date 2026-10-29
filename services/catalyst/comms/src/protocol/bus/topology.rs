//! Pulse's view of the world for the connector's stats: the islands snapshot and the discovery
//! beacon. Both are latest-wins whole snapshots, and both carry archipelago's
//! `decentraland.kernel.comms.v3` shapes, so the generated messages are their wire form. The
//! projection of a cluster pass onto the islands snapshot stays in Pulse
//! (`pulse::cluster::feed::encode_topology`).

use prost::Message as _;

use super::BusError;
use crate::protocol::pulse::decentraland::kernel::comms::v3::{
    IslandStatusMessage, ServiceDiscoveryMessage, ServiceStatus,
};

/// Named "island", not "cluster", on purpose: `engine.islands` is archipelago's topology subject
/// carrying archipelago's `IslandStatusMessage`, and Pulse republishes that shape verbatim. The
/// island vocabulary survives only where it is archipelago's contract.
pub const ISLANDS_SUBJECT: &str = "engine.islands";

pub const DISCOVERY_SUBJECT: &str = "engine.discovery";

/// Default core NATS max_payload. A complete frame must fit; never truncate it.
pub const MAX_ISLANDS_BYTES: usize = 1024 * 1024;

pub fn encode_islands(message: &IslandStatusMessage) -> Result<Vec<u8>, BusError> {
    if message.encoded_len() > MAX_ISLANDS_BYTES {
        return Err(BusError::TooLarge);
    }
    Ok(message.encode_to_vec())
}

pub fn decode_islands(payload: &[u8]) -> Result<IslandStatusMessage, BusError> {
    if payload.len() > MAX_ISLANDS_BYTES {
        return Err(BusError::TooLarge);
    }
    Ok(IslandStatusMessage::decode(payload)?)
}

pub fn encode_discovery(
    server_name: &str,
    commit_hash: &str,
    current_time_ms: u64,
    user_count: u32,
) -> Vec<u8> {
    ServiceDiscoveryMessage {
        server_name: server_name.to_string(),
        status: Some(ServiceStatus {
            current_time: current_time_ms,
            commit_hash: Some(commit_hash.to_string()),
            user_count,
        }),
    }
    .encode_to_vec()
}

pub fn decode_discovery(payload: &[u8]) -> Result<ServiceDiscoveryMessage, BusError> {
    Ok(ServiceDiscoveryMessage::decode(payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::bus::BusError;
    use crate::protocol::pulse::decentraland::common::Position;
    use crate::protocol::pulse::decentraland::kernel::comms::v3::IslandData;

    #[test]
    fn complete_topology_cap_is_shared_by_encoder_and_decoder() {
        let message = IslandStatusMessage {
            data: vec![IslandData {
                id: "x".repeat(MAX_ISLANDS_BYTES),
                ..Default::default()
            }],
        };
        assert_eq!(encode_islands(&message), Err(BusError::TooLarge));
        assert_eq!(
            decode_islands(&message.encode_to_vec()),
            Err(BusError::TooLarge)
        );
        // Unknown fields are still a valid protobuf frame exactly at the byte ceiling.
        let mut exact = vec![0x12, 0xfc, 0xff, 0x3f];
        exact.resize(MAX_ISLANDS_BYTES, 0);
        assert_eq!(
            decode_islands(&exact).unwrap(),
            IslandStatusMessage::default()
        );
    }

    /// The beacon Pulse's feed encoded before the encoder moved here.
    #[test]
    fn discovery_bytes_are_pinned() {
        let bytes = encode_discovery("pulse", "deadbeef", 1_000_000, 42);
        assert_eq!(
            hex::encode(&bytes),
            "0a0570756c7365121008c0843d12086465616462656566182a"
        );
        let beacon = decode_discovery(&bytes).unwrap();
        assert_eq!(beacon.server_name, "pulse");
        let status = beacon.status.unwrap();
        assert_eq!(status.current_time, 1_000_000);
        assert_eq!(status.commit_hash.as_deref(), Some("deadbeef"));
        assert_eq!(status.user_count, 42);
    }

    /// Two islands, the second without a centre. The first entry is byte for byte what Pulse's
    /// projection (`encode_topology`) produced for that island, recorded before the subjects moved
    /// here; the second is the shape the connector declines to report.
    #[test]
    fn islands_bytes_are_pinned() {
        let message = IslandStatusMessage {
            data: vec![
                IslandData {
                    id: "island-C1".into(),
                    peers: vec!["0xa".into(), "0xb".into()],
                    max_peers: 0,
                    center: Some(Position {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    }),
                    radius: 8.0,
                },
                IslandData {
                    id: "centerless".into(),
                    peers: vec![],
                    max_peers: 0,
                    center: None,
                    radius: 0.0,
                },
            ],
        };
        let bytes = message.encode_to_vec();
        assert_eq!(
            hex::encode(&bytes),
            concat!(
                "0a2f0a0969736c616e642d433112033078611203307862220f0d0000803f15000000",
                "401d00004040290000000000002040",
                "0a0c0a0a63656e7465726c657373"
            )
        );
        assert_eq!(decode_islands(&bytes).unwrap(), message);
    }

    #[test]
    fn the_subjects_are_the_engine_feeds() {
        assert_eq!(ISLANDS_SUBJECT, "engine.islands");
        assert_eq!(DISCOVERY_SUBJECT, "engine.discovery");
    }

    #[test]
    fn undecodable_payloads_are_a_payload_error() {
        assert!(matches!(decode_islands(&[0xff]), Err(BusError::Payload(_))));
        assert!(matches!(
            decode_discovery(&[0xff]),
            Err(BusError::Payload(_))
        ));
    }
}
