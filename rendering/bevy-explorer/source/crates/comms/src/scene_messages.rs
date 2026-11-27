use std::borrow::Cow;

use alloy_core::primitives::Address;
use bevy::prelude::*;
use dcl_types::{scene_message_fits, CommsMessageType};

use crate::{scene_crdt::filter_scene_crdt, SceneRoomModeration};

/// Scene message policy and topic-routing counters, exposed together by `/scene_topics`.
#[derive(Resource, Default)]
pub struct SceneMessageTraffic {
    pub(crate) published: u64,
    pub(crate) publish_dropped: u64,
    pub(crate) received: u64,
    pub(crate) receive_dropped: u64,
    pub(crate) banned_senders: u64,
    pub(crate) filtered_inbound: u64,
    pub(crate) filtered_outbound: u64,
    pub(crate) invalid: u64,
}

/// On a scene root: the peer messages handed to the scene's inbound queues (string bus, binary
/// bus, topic inbox) since the component was inserted.
#[derive(Component, Default)]
pub struct SceneMessagesDelivered(pub u64);

impl SceneMessageTraffic {
    /// Topic publishes (sent, dropped).
    pub fn publishes(&self) -> (u64, u64) {
        (self.published, self.publish_dropped)
    }

    pub fn record_publish(&mut self, sent: bool) {
        if sent {
            self.published += 1;
        } else {
            self.publish_dropped += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeerTrust {
    Banned,
    Peer,
    Trusted,
}

/// Without a trusted source or development realm, absent metadata grants peer permissions.
pub(crate) fn peer_trust(
    moderation: Option<&SceneRoomModeration>,
    sender: Option<Address>,
    trusted_source: bool,
) -> PeerTrust {
    if moderation.is_some_and(|state| sender.is_some_and(|sender| state.is_banned(sender))) {
        return PeerTrust::Banned;
    }
    if trusted_source
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
pub fn outbound<'a>(
    data: &'a [u8],
    sender_banned: bool,
    traffic: &mut SceneMessageTraffic,
) -> Option<Cow<'a, [u8]>> {
    if sender_banned {
        return None;
    }
    let filtered = filter_message(data, true, dcl_types::MAX_BINARY_MESSAGE_BYTES);
    match &filtered {
        None => traffic.invalid += 1,
        Some(Cow::Owned(_)) => traffic.filtered_outbound += 1,
        _ => (),
    }
    filtered
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
    fn outbound_counts_filtering_and_invalid_frames_in_the_shared_traffic() {
        let mut traffic = SceneMessageTraffic::default();
        let mut frame = vec![CommsMessageType::Binary as u8, 1];
        for word in [
            24_u32,
            1,
            1,
            dcl_component::SceneComponentId::PHYSICS_COMBINED_FORCE.0,
            1,
            0,
        ] {
            frame.extend(word.to_le_bytes());
        }
        assert_eq!(
            outbound(&frame, false, &mut traffic).unwrap().as_ref(),
            &[2, 1]
        );
        assert!(outbound(&[2, 1, 0], false, &mut traffic).is_none());
        assert!(outbound(&frame, true, &mut traffic).is_none());
        assert_eq!((traffic.filtered_outbound, traffic.invalid), (1, 1));
    }

    #[test]
    fn ingress_accepts_older_peers_while_egress_reserves_receiver_framing() {
        let mut traffic = SceneMessageTraffic::default();
        let mut frame = vec![0; dcl_types::MAX_BINARY_MESSAGE_BYTES + 1];
        frame[0] = CommsMessageType::Binary as u8;
        assert!(filter_scene_message(&frame, false).is_some());
        assert!(outbound(&frame, false, &mut traffic).is_some());
        assert!(outbound(&frame, true, &mut traffic).is_none());
        assert_eq!(frame.len() - 1 + 1 + 42, 13 * 1024);
        frame.push(0);
        assert!(filter_scene_message(&frame, true).is_some());
        assert!(!scene_message_fits(&frame));
        assert!(outbound(&frame, false, &mut traffic).is_none());
        frame.resize(dcl_types::MAX_COMMS_MESSAGE_BYTES + 1, 0);
        assert!(filter_scene_message(&frame, true).is_some());
        frame.push(0);
        assert!(filter_scene_message(&frame, true).is_none());
        assert_eq!(traffic.invalid, 1);
        assert_eq!(traffic.filtered_outbound, 0);
    }
}
