use std::borrow::Cow;

use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_types::{scene_message_fits, CommsMessageType};

use crate::{scene_crdt::filter_scene_crdt, SceneRoomModeration};

pub struct SceneMessagesPlugin;

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

/// None denies delivery; absent metadata grants ordinary peer permissions.
pub(crate) fn peer_trust(
    moderation: Option<&SceneRoomModeration>,
    sender: Option<Address>,
    authoritative: bool,
) -> Option<bool> {
    if moderation
        .is_some_and(|state| sender.is_some_and(|sender| state.banned_addresses.contains(&sender)))
    {
        return None;
    }
    Some(
        authoritative
            || moderation.is_some_and(|state| {
                sender.is_some_and(|sender| state.scene_admins.contains(&sender))
            }),
    )
}

pub fn filter_scene_message(data: &[u8], trusted: bool) -> Option<Cow<'_, [u8]>> {
    if !scene_message_fits(data) {
        return None;
    }
    if data[0] == CommsMessageType::Binary as u8 {
        if let Cow::Owned(filtered) = filter_scene_crdt(&data[1..], trusted)? {
            let mut frame = Vec::with_capacity(1 + filtered.len());
            frame.push(CommsMessageType::Binary as u8);
            frame.extend(filtered);
            return scene_message_fits(&frame).then_some(Cow::Owned(frame));
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
        assert_eq!(peer_trust(None, Some(sender), false), Some(false));
        metadata.scene_admins.insert(sender);
        assert_eq!(peer_trust(Some(&metadata), Some(sender), false), Some(true));
        metadata.banned_addresses.insert(sender);
        assert_eq!(peer_trust(Some(&metadata), Some(sender), true), None);
    }

    #[test]
    fn scene_binary_limit_includes_receiver_sender_framing() {
        let mut frame = vec![0; dcl_types::MAX_BINARY_MESSAGE_BYTES + 1];
        frame[0] = CommsMessageType::Binary as u8;
        assert!(filter_scene_message(&frame, false).is_some());
        assert_eq!(frame.len() - 1 + 1 + 42, 13 * 1024);
        frame.push(0);
        assert!(filter_scene_message(&frame, true).is_none());
    }
}
