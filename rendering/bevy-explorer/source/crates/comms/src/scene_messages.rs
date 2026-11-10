use std::{borrow::Cow, time::Duration};

use alloy_core::primitives::Address;
use bevy::{platform::collections::HashMap, prelude::*};
use common::rpc::{RpcCall, RpcCallEvent, RpcEventSender, RpcOrigin, RpcStreamSender};
use dcl_component::proto_components::kernel::comms::rfc4;
use dcl_types::{
    scene_topic::{decode_topic, valid_topic},
    CommsMessageType, MAX_COMMS_MESSAGE_BYTES,
};
use serde_json::json;

use crate::{scene_crdt::filter_scene_crdt, scene_topics::TopicInbox, SceneRoomModeration};

pub use crate::scene_topics::TopicPublishBudget;

type BinaryBusSender = RpcStreamSender<(String, Vec<u8>)>;

#[derive(Default, Resource)]
pub struct SceneMessageBus {
    strings: HashMap<String, RpcEventSender>,
    binary: HashMap<String, BinaryBusSender>,
    topics: HashMap<String, (Entity, TopicInbox)>,
    denied_senders: u64,
    filtered_crdt: u64,
}

impl SceneMessageBus {
    pub(crate) fn update_subscriptions<'a>(
        &mut self,
        events: impl Iterator<Item = &'a RpcCallEvent>,
        is_live: impl Fn(Entity) -> bool,
    ) {
        self.strings.retain(|_, sender| !sender.is_closed());
        self.binary.retain(|_, sender| !sender.is_closed());
        self.topics.retain(|_, (entity, _)| is_live(*entity));
        for event in events {
            let RpcOrigin::Scene { hash, entity } = &event.origin else {
                continue;
            };
            match &event.call {
                RpcCall::SubscribeMessageBus { sender } => {
                    self.strings.insert(hash.to_string(), sender.clone());
                }
                RpcCall::SubscribeBinaryBus { sender } => {
                    self.binary.insert(hash.to_string(), sender.clone());
                }
                RpcCall::SetTopicSubscription {
                    topic,
                    subscribed,
                    response,
                } => {
                    if is_live(*entity) && valid_topic(topic) {
                        if *subscribed {
                            let (owner, inbox) = self
                                .topics
                                .entry(hash.to_string())
                                .or_insert_with(|| (*entity, TopicInbox::default()));
                            if *owner != *entity {
                                *owner = *entity;
                                *inbox = TopicInbox::default();
                            }
                            inbox.set_subscription(topic.clone(), true);
                        } else if let Some((owner, inbox)) = self.topics.get_mut(hash.as_ref()) {
                            if *owner == *entity {
                                inbox.set_subscription(topic.clone(), false);
                            }
                        }
                    }
                    response.send(());
                }
                RpcCall::ConsumeTopicMessages { topic, response } => {
                    let messages = self
                        .topics
                        .get_mut(hash.as_ref())
                        .filter(|(owner, _)| *owner == *entity && valid_topic(topic))
                        .map(|(_, inbox)| inbox.consume(topic))
                        .unwrap_or_default();
                    response.send(messages);
                }
                _ => (),
            }
        }
    }

    pub(crate) fn receive(&mut self, scene: rfc4::Scene, address: String, trust: Option<bool>) {
        let Some(trusted) = trust else {
            self.denied_senders = self.denied_senders.saturating_add(1);
            return;
        };
        let Some((&kind, payload)) = scene.data.split_first() else {
            return;
        };
        if payload.len() > MAX_COMMS_MESSAGE_BYTES {
            return;
        }
        if kind == CommsMessageType::Topic as u8 {
            if let (Some((topic, data)), Some((_, inbox))) =
                (decode_topic(payload), self.topics.get_mut(&scene.scene_id))
            {
                inbox.receive(topic, &address, data);
            }
        } else if kind == CommsMessageType::Binary as u8 {
            if let Some(sender) = self.binary.get(&scene.scene_id) {
                let filtered = filter_scene_crdt(payload, trusted);
                if filtered.as_deref() != Some(payload) {
                    self.filtered_crdt = self.filtered_crdt.saturating_add(1);
                }
                if let Some(filtered) = filtered {
                    let _ = sender.send((address, filtered.into_owned()));
                }
            }
        } else if let Some(sender) = self.strings.get(&scene.scene_id) {
            let message = if kind == CommsMessageType::String as u8 {
                payload
            } else {
                &scene.data
            };
            let message = std::str::from_utf8(message).unwrap_or_default();
            let _ = sender.send(json!({ "message": message, "sender": address }).to_string());
        }
    }

    pub(crate) fn debug_state(&self) -> serde_json::Value {
        let (subscriptions, queued) =
            self.topics
                .values()
                .fold((0, 0), |(total, queued), (_, inbox)| {
                    let (topics, messages) = inbox.counts();
                    (total + topics, queued + messages)
                });
        json!({
            "topicSubscriptions": subscriptions,
            "queuedTopicMessages": queued,
            "bannedSenderDrops": self.denied_senders,
            "filteredInboundCrdt": self.filtered_crdt,
        })
    }
}

pub(crate) fn peer_trust(
    moderation: Option<&SceneRoomModeration>,
    sender: Address,
    local: Option<Address>,
) -> Option<bool> {
    if moderation.is_some_and(|state| state.banned_addresses.contains(&sender)) {
        return None;
    }
    Some(
        local == Some(sender)
            || moderation.is_some_and(|state| state.scene_admins.contains(&sender)),
    )
}

pub fn outgoing_scene_message(
    data: &[u8],
    topic_budget: &mut TopicPublishBudget,
    now: Duration,
) -> Option<Vec<u8>> {
    let (&kind, payload) = data.split_first()?;
    if payload.len() > MAX_COMMS_MESSAGE_BYTES {
        return None;
    }
    if kind == CommsMessageType::Topic as u8 {
        let (topic, _) = decode_topic(payload)?;
        if !topic_budget.try_publish(topic, now) {
            return None;
        }
    }
    if kind == CommsMessageType::Binary as u8 {
        if let Cow::Owned(filtered) = filter_scene_crdt(payload, true)? {
            let mut frame = Vec::with_capacity(1 + filtered.len());
            frame.push(kind);
            frame.extend(filtered);
            return Some(frame);
        }
    }
    Some(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_trust_uses_room_admins_and_ban_overrides_trust() {
        let admin = Address::repeat_byte(1);
        let regular = Address::repeat_byte(2);
        let mut metadata = SceneRoomModeration::default();
        metadata.scene_admins.insert(admin);
        assert_eq!(peer_trust(Some(&metadata), admin, None), Some(true));
        assert_eq!(peer_trust(Some(&metadata), regular, None), Some(false));
        assert_eq!(peer_trust(None, admin, None), Some(false));
        assert_eq!(peer_trust(None, regular, Some(regular)), Some(true));
        metadata.banned_addresses.insert(admin);
        assert_eq!(peer_trust(Some(&metadata), admin, Some(admin)), None);
    }

    #[test]
    fn topics_do_not_leak_into_message_bus_or_another_scenes_queue() {
        let (sender, mut strings) = RpcEventSender::channel();
        let mut bus = SceneMessageBus::default();
        let mut inbox = TopicInbox::default();
        inbox.set_subscription("news".into(), true);
        bus.strings.insert("scene-a".into(), sender);
        bus.topics
            .insert("scene-b".into(), (Entity::PLACEHOLDER, inbox));
        let packet = |scene_id: &str| rfc4::Scene {
            scene_id: scene_id.to_owned(),
            data: dcl_types::scene_topic::encode_topic("news", "hello").unwrap(),
        };
        bus.receive(packet("scene-a"), "peer".to_owned(), Some(false));
        assert!(strings.try_recv().is_err());
        bus.receive(packet("scene-b"), "banned".to_owned(), None);
        bus.receive(packet("scene-b"), "peer".to_owned(), Some(false));
        assert_eq!(
            bus.topics.get_mut("scene-b").unwrap().1.consume("news"),
            [("peer".to_owned(), "hello".to_owned())],
        );
    }

    #[test]
    fn egress_keeps_bevys_cap_and_enforces_the_topic_quota() {
        let mut budget = TopicPublishBudget::default();
        let mut message = vec![b'x'; MAX_COMMS_MESSAGE_BYTES + 1];
        message[0] = CommsMessageType::String as u8;
        assert_eq!(
            outgoing_scene_message(&message, &mut budget, Duration::ZERO),
            Some(message.clone()),
        );
        message.push(b'x');
        assert!(outgoing_scene_message(&message, &mut budget, Duration::ZERO).is_none());

        let topic = dcl_types::scene_topic::encode_topic("news", "hello").unwrap();
        for _ in 0..10 {
            assert!(outgoing_scene_message(&topic, &mut budget, Duration::ZERO).is_some());
        }
        assert!(outgoing_scene_message(&topic, &mut budget, Duration::ZERO).is_none());
        assert!(outgoing_scene_message(&topic, &mut budget, Duration::from_secs(1)).is_some());
    }
}
