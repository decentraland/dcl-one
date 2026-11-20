use std::borrow::Cow;

use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_types::{scene_message_fits, CommsMessageType};

use crate::{scene_crdt::filter_scene_crdt, SceneRoomModeration};

pub(crate) struct SceneMessagesPlugin;

impl Plugin for SceneMessagesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneMessageTraffic>()
            .add_console_command::<SceneCommsCommand, _>(scene_comms_command)
            .add_debug_section(DebugSection {
                title: "Scene message policy",
                query: "/scene_comms_state",
                fields: &[
                    ("Banned sender drops", "bannedSenderDrops"),
                    ("Filtered inbound CRDT", "filteredInboundCrdt"),
                    ("Filtered outbound CRDT", "filteredOutboundCrdt"),
                    ("Invalid or oversized messages", "invalidMessages"),
                    ("Binary payload limit (bytes)", "binaryPayloadLimit"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

#[derive(Resource, Default)]
pub struct SceneMessageTraffic {
    pub(crate) banned_senders: u64,
    pub(crate) filtered_inbound: u64,
    pub filtered_outbound: u64,
    pub invalid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeerTrust {
    Banned,
    Peer,
    Trusted,
}

/// Absent metadata grants ordinary peer permissions.
pub(crate) fn peer_trust(
    moderation: Option<&SceneRoomModeration>,
    sender: Option<Address>,
    authoritative: bool,
) -> PeerTrust {
    if moderation.is_some_and(|state| sender.is_some_and(|sender| state.is_banned(sender))) {
        return PeerTrust::Banned;
    }
    if authoritative
        || moderation.is_some_and(|state| sender.is_some_and(|sender| state.is_admin(sender)))
    {
        PeerTrust::Trusted
    } else {
        PeerTrust::Peer
    }
}

/// Incoming binary payloads retain the 30,000-byte bus limit for older peers. Egress
/// additionally enforces `scene_message_fits` for receivers with a 13 KiB buffer.
pub(crate) fn filter_scene_message(data: &[u8], trusted: bool) -> Option<Cow<'_, [u8]>> {
    filter_message(data, trusted, dcl_types::MAX_COMMS_MESSAGE_BYTES)
}

/// Engine egress enforces the local ban and the receiver's binary framing limit.
pub fn outbound(data: &[u8], sender_banned: bool) -> Option<Cow<'_, [u8]>> {
    if sender_banned {
        return None;
    }
    filter_message(data, true, dcl_types::MAX_BINARY_MESSAGE_BYTES)
}

fn filter_message(data: &[u8], trusted: bool, binary_limit: usize) -> Option<Cow<'_, [u8]>> {
    let fits = match data.split_first() {
        Some((&kind, payload)) if kind == CommsMessageType::Binary as u8 => {
            payload.len() <= binary_limit
        }
        _ => scene_message_fits(data),
    };
    if !fits {
        return None;
    }
    if data[0] == CommsMessageType::Binary as u8 {
        if let Cow::Owned(filtered) = filter_scene_crdt(&data[1..], trusted)? {
            let mut frame = Vec::with_capacity(1 + filtered.len());
            frame.push(CommsMessageType::Binary as u8);
            frame.extend(filtered);
            return Some(Cow::Owned(frame));
        }
    }
    Some(Cow::Borrowed(data))
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_comms_state")]
struct SceneCommsCommand;

fn scene_comms_command(
    mut input: ConsoleCommand<SceneCommsCommand>,
    traffic: Res<SceneMessageTraffic>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(
            serde_json::json!({
                "bannedSenderDrops": traffic.banned_senders,
                "filteredInboundCrdt": traffic.filtered_inbound,
                "filteredOutboundCrdt": traffic.filtered_outbound,
                "invalidMessages": traffic.invalid,
                "binaryPayloadLimit": dcl_types::MAX_BINARY_MESSAGE_BYTES,
            })
            .to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_bans_override_admin_and_authoritative_trust() {
        let sender = Address::repeat_byte(1);
        let mut metadata = SceneRoomModeration::default();
        assert_eq!(peer_trust(None, Some(sender), false), PeerTrust::Peer);
        metadata.scene_admins.insert(sender);
        assert_eq!(
            peer_trust(Some(&metadata), Some(sender), false),
            PeerTrust::Trusted
        );
        metadata.banned_addresses.insert(sender);
        assert_eq!(
            peer_trust(Some(&metadata), Some(sender), true),
            PeerTrust::Banned
        );
    }

    #[test]
    fn ingress_accepts_older_peers_while_egress_reserves_receiver_framing() {
        let mut frame = vec![0; dcl_types::MAX_BINARY_MESSAGE_BYTES + 1];
        frame[0] = CommsMessageType::Binary as u8;
        assert!(filter_scene_message(&frame, false).is_some());
        assert!(outbound(&frame, false).is_some());
        assert!(outbound(&frame, true).is_none());
        assert_eq!(frame.len() - 1 + 1 + 42, 13 * 1024);
        frame.push(0);
        assert!(filter_scene_message(&frame, true).is_some());
        assert!(!scene_message_fits(&frame));
        assert!(outbound(&frame, false).is_none());
        frame.resize(dcl_types::MAX_COMMS_MESSAGE_BYTES + 1, 0);
        assert!(filter_scene_message(&frame, true).is_some());
        frame.push(0);
        assert!(filter_scene_message(&frame, true).is_none());
    }
}
