//! What happens to one data packet on the room, as upstream's `logic/data-received-handler.ts`
//! and `logic/message-routing.ts`:
//!
//! 1. [`admit`]: dropped without a count when it has no sender, comes from this participant or
//!    has no topic; counted `rejected_oversized` above the size cap and `rate_limited` when the
//!    sender's bucket is empty;
//! 2. [`route`]: decoded as an rfc4 `Packet`; only `chat` and `chat_reaction` go on. The room
//!    must be connected and the sender a member of the community the topic names (one read,
//!    [`Membership::audience`]); the packet is re-encoded with the sender stamped in and
//!    published, reliable, on `community:<id>` to the other members of the active community,
//!    [`BATCH_SIZE`] destinations per publish, each publish bounded by [`PUBLISH_TIMEOUT`]. A
//!    failed or timed-out batch is logged and counted (`message_batch_failures_total`) and the
//!    next ones still go; the room dropping mid-way stops the rest. Counted `delivered` once
//!    every batch was tried and at least one was published, `failed` otherwise.
//!
//! The participant counts `overloaded` for an admitted packet that finds every routing slot
//! taken: it drops the packet rather than stop reading the room's events.

use std::time::Duration;

use async_trait::async_trait;
use prost::Message;
use uuid::Uuid;

use crate::proto::rfc4;

/// Destination identities per publish.
pub const BATCH_SIZE: usize = 100;
pub const TOPIC_PREFIX: &str = "community:";
/// The longest one publish may take before its batch counts as failed and its routing slot
/// moves on (a full reliable buffer or a reconnect in progress can hold it indefinitely).
pub const PUBLISH_TIMEOUT: Duration = Duration::from_secs(5);

/// The `outcome` label of `message_delivery_total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Delivered,
    Failed,
    RejectedOversized,
    RateLimited,
    /// Admitted, but every routing slot was taken; dropped unrouted.
    Overloaded,
}

impl Outcome {
    pub const ALL: [Outcome; 5] = [
        Self::Delivered,
        Self::Failed,
        Self::RejectedOversized,
        Self::RateLimited,
        Self::Overloaded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Failed => "failed",
            Self::RejectedOversized => "rejected_oversized",
            Self::RateLimited => "rate_limited",
            Self::Overloaded => "overloaded",
        }
    }
}

/// Why [`admit`] stopped a packet: the first three are dropped without a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NoSender,
    FromSelf,
    NoTopic,
    Counted(Outcome),
}

/// The checks before decoding. `allow` asks the sender's rate limit bucket, only once the
/// cheaper checks passed.
pub fn admit<'a>(
    sender: Option<&'a str>,
    own_identity: &str,
    topic: Option<&'a str>,
    payload_len: usize,
    max_packet_bytes: usize,
    allow: impl FnOnce(&str) -> bool,
) -> Result<(&'a str, &'a str), Refused> {
    let sender = sender.filter(|s| !s.is_empty()).ok_or(Refused::NoSender)?;
    if sender == own_identity {
        return Err(Refused::FromSelf);
    }
    let topic = topic.filter(|t| !t.is_empty()).ok_or(Refused::NoTopic)?;
    if payload_len > max_packet_bytes {
        return Err(Refused::Counted(Outcome::RejectedOversized));
    }
    if !allow(sender) {
        return Err(Refused::Counted(Outcome::RateLimited));
    }
    Ok((sender, topic))
}

/// The community a topic names: `community:<uuid>`.
pub fn community_of(topic: &str) -> Option<Uuid> {
    Uuid::parse_str(topic.strip_prefix(TOPIC_PREFIX)?).ok()
}

/// The topic the router publishes a community's packets on, with the id in the lowercase
/// hyphenated form the Explorer keys its channels by.
pub fn topic_of(community: Uuid) -> String {
    format!("{TOPIC_PREFIX}{}", community.hyphenated())
}

/// Why a packet was not routed; each counts `failed`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("undecodable packet: {0}")]
    Undecodable(String),
    #[error("not a community id in the topic {0:?}")]
    BadTopic(String),
    #[error("not a chat message or chat reaction")]
    NotChat,
    #[error("room not connected")]
    NotConnected,
    #[error("sender is not a member of the community")]
    NotMember,
    #[error("no other member of an active community")]
    NoRecipients,
    #[error("membership read failed: {0}")]
    Store(String),
    #[error("no batch was published")]
    NothingPublished,
}

/// The packet with the sender stamped in, encoded: `Chat.forwarded_from` or
/// `ChatReaction.address`, whatever the client put there.
pub fn stamp(mut packet: rfc4::Packet, sender: &str) -> Result<Vec<u8>, Failure> {
    match packet.message.as_mut() {
        Some(rfc4::packet::Message::Chat(chat)) => chat.forwarded_from = Some(sender.to_owned()),
        Some(rfc4::packet::Message::ChatReaction(reaction)) => {
            reaction.address = sender.to_owned();
        }
        _ => return Err(Failure::NotChat),
    }
    Ok(packet.encode_to_vec())
}

/// The members' read ([`crate::membership`]).
#[async_trait]
pub trait Membership: Send + Sync {
    /// `None` when `sender` is not a member of `community`; otherwise the lowercase addresses of
    /// the other members when the community is active and not suspended, none when it is not.
    async fn audience(&self, community: Uuid, sender: &str)
    -> Result<Option<Vec<String>>, String>;
}

/// The room, as routing uses it.
#[async_trait]
pub trait Room: Send + Sync {
    fn connected(&self) -> bool;
    async fn publish(
        &self,
        payload: Vec<u8>,
        topic: &str,
        destinations: Vec<String>,
    ) -> Result<(), String>;
}

/// Routes one admitted packet; the outcome to count.
pub async fn route(
    room: &dyn Room,
    members: &dyn Membership,
    payload: &[u8],
    sender: &str,
    topic: &str,
) -> Outcome {
    match try_route(room, members, payload, sender, topic).await {
        Ok(recipients) => {
            tracing::info!(
                topic,
                sender,
                recipients,
                "routed a community message to its members"
            );
            Outcome::Delivered
        }
        Err(failure) => {
            tracing::warn!(topic, sender, error = %failure, "community message not routed");
            Outcome::Failed
        }
    }
}

async fn try_route(
    room: &dyn Room,
    members: &dyn Membership,
    payload: &[u8],
    sender: &str,
    topic: &str,
) -> Result<usize, Failure> {
    let packet = rfc4::Packet::decode(payload).map_err(|e| Failure::Undecodable(e.to_string()))?;
    let community = community_of(topic).ok_or_else(|| Failure::BadTopic(topic.to_owned()))?;
    if !matches!(
        packet.message,
        Some(rfc4::packet::Message::Chat(_) | rfc4::packet::Message::ChatReaction(_))
    ) {
        return Err(Failure::NotChat);
    }
    if !room.connected() {
        return Err(Failure::NotConnected);
    }
    let recipients = members
        .audience(community, sender)
        .await
        .map_err(Failure::Store)?
        .ok_or(Failure::NotMember)?;
    if recipients.is_empty() {
        return Err(Failure::NoRecipients);
    }
    let encoded = stamp(packet, sender)?;
    let topic = topic_of(community);
    let batches = recipients.chunks(BATCH_SIZE).count();
    let mut published = 0;
    for (index, batch) in recipients.chunks(BATCH_SIZE).enumerate() {
        // never publish on a room a reconnect is tearing down
        if !room.connected() {
            return Err(Failure::NotConnected);
        }
        let sent = tokio::time::timeout(
            PUBLISH_TIMEOUT,
            room.publish(encoded.clone(), &topic, batch.to_vec()),
        )
        .await;
        let error = match sent {
            Ok(Ok(())) => {
                published += batch.len();
                continue;
            }
            Ok(Err(error)) => {
                crate::metrics::batch_failed("error");
                error
            }
            Err(_) => {
                crate::metrics::batch_failed("timeout");
                format!("no answer within {PUBLISH_TIMEOUT:?}")
            }
        };
        tracing::error!(
            %topic,
            batch = index + 1,
            batches,
            recipients = batch.len(),
            %error,
            "failed to publish a batch"
        );
    }
    if published == 0 {
        return Err(Failure::NothingPublished);
    }
    Ok(published)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const SENDER: &str = "0x1111111111111111111111111111111111111111";
    const COMMUNITY: &str = "3f2a0c1e-9b8d-4e7f-a6c5-0123456789ab";

    fn topic() -> String {
        format!("community:{COMMUNITY}")
    }

    fn chat(forwarded_from: Option<&str>) -> rfc4::Packet {
        rfc4::Packet {
            message: Some(rfc4::packet::Message::Chat(rfc4::Chat {
                message: "hello".into(),
                timestamp: 1.5,
                forwarded_from: forwarded_from.map(str::to_owned),
            })),
            protocol_version: 100,
        }
    }

    fn reaction(address: &str) -> rfc4::Packet {
        rfc4::Packet {
            message: Some(rfc4::packet::Message::ChatReaction(rfc4::ChatReaction {
                emoji_index: 3,
                message_id: "m1".into(),
                address: address.into(),
                emoji: "x".into(),
                remove: false,
            })),
            protocol_version: 100,
        }
    }

    struct Members {
        member: bool,
        recipients: Vec<String>,
        fail: bool,
    }

    #[async_trait]
    impl Membership for Members {
        async fn audience(
            &self,
            community: Uuid,
            sender: &str,
        ) -> Result<Option<Vec<String>>, String> {
            assert_eq!(community.to_string(), COMMUNITY);
            assert_eq!(sender, SENDER);
            if self.fail {
                return Err("db down".into());
            }
            Ok(self.member.then(|| self.recipients.clone()))
        }
    }

    #[derive(Default)]
    struct FakeRoom {
        down: AtomicBool,
        /// the publish with this index fails
        fail_publish: Option<usize>,
        /// every publish fails
        fail_all: bool,
        /// the publish with this index never answers
        hang_publish: Option<usize>,
        /// the room drops after this many publishes
        drop_after: Option<usize>,
        calls: AtomicUsize,
        published: Mutex<Vec<Published>>,
    }

    /// One publish: payload, topic, destinations.
    type Published = (Vec<u8>, String, Vec<String>);

    #[async_trait]
    impl Room for FakeRoom {
        fn connected(&self) -> bool {
            !self.down.load(Ordering::SeqCst)
        }
        async fn publish(
            &self,
            payload: Vec<u8>,
            topic: &str,
            destinations: Vec<String>,
        ) -> Result<(), String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.drop_after == Some(n + 1) {
                self.down.store(true, Ordering::SeqCst);
            }
            if self.fail_all || self.fail_publish == Some(n) {
                return Err("sfu said no".into());
            }
            if self.hang_publish == Some(n) {
                std::future::pending::<()>().await;
            }
            self.published
                .lock()
                .unwrap()
                .push((payload, topic.to_owned(), destinations));
            Ok(())
        }
    }

    fn addresses(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("0x{i:040x}")).collect()
    }

    fn members(n: usize) -> Members {
        Members {
            member: true,
            recipients: addresses(n),
            fail: false,
        }
    }

    #[test]
    fn admit_drops_and_counts_as_upstream() {
        let yes = |_: &str| true;
        let max = 8192;
        assert_eq!(
            admit(None, "router", Some("t"), 1, max, yes),
            Err(Refused::NoSender)
        );
        assert_eq!(
            admit(Some("router"), "router", Some("t"), 1, max, yes),
            Err(Refused::FromSelf)
        );
        assert_eq!(
            admit(Some(SENDER), "router", None, 1, max, yes),
            Err(Refused::NoTopic)
        );
        assert_eq!(
            admit(Some(SENDER), "router", Some(""), 1, max, yes),
            Err(Refused::NoTopic)
        );
        assert_eq!(
            admit(Some(SENDER), "router", Some("t"), max + 1, max, |_| panic!(
                "an oversized packet takes no token"
            )),
            Err(Refused::Counted(Outcome::RejectedOversized))
        );
        assert_eq!(
            admit(Some(SENDER), "router", Some("t"), max, max, |s| {
                assert_eq!(s, SENDER);
                false
            }),
            Err(Refused::Counted(Outcome::RateLimited))
        );
        assert_eq!(
            admit(Some(SENDER), "router", Some("t"), max, max, yes),
            Ok((SENDER, "t"))
        );
    }

    #[test]
    fn topics_name_a_community_by_uuid() {
        let id = Uuid::parse_str(COMMUNITY).unwrap();
        assert_eq!(community_of(&topic()), Some(id));
        assert_eq!(
            community_of("community:3F2A0C1E-9B8D-4E7F-A6C5-0123456789AB"),
            Some(id)
        );
        for bad in [
            "community:",
            "community:abc-1",
            COMMUNITY,
            "other:3f2a0c1e-9b8d-4e7f-a6c5-0123456789ab",
            "0x1111111111111111111111111111111111111111",
        ] {
            assert_eq!(community_of(bad), None, "{bad}");
        }
        assert_eq!(topic_of(id), topic());
    }

    #[test]
    fn stamping_sets_the_author_and_keeps_the_rest() {
        let stamped = stamp(chat(Some("0xforged")), SENDER).unwrap();
        assert_eq!(
            rfc4::Packet::decode(stamped.as_slice()).unwrap(),
            chat(Some(SENDER))
        );
        let stamped = stamp(reaction("0xforged"), SENDER).unwrap();
        assert_eq!(
            rfc4::Packet::decode(stamped.as_slice()).unwrap(),
            reaction(SENDER)
        );
        let position = rfc4::Packet {
            message: Some(rfc4::packet::Message::Position(Default::default())),
            protocol_version: 100,
        };
        assert_eq!(stamp(position, SENDER), Err(Failure::NotChat));
        assert_eq!(
            stamp(rfc4::Packet::default(), SENDER),
            Err(Failure::NotChat)
        );
    }

    #[tokio::test]
    async fn a_member_s_chat_reaches_the_others_in_batches_of_100() {
        let room = FakeRoom::default();
        let outcome = route(
            &room,
            &members(250),
            &chat(None).encode_to_vec(),
            SENDER,
            &topic(),
        )
        .await;
        assert_eq!(outcome, Outcome::Delivered);
        let published = room.published.lock().unwrap();
        assert_eq!(
            published.iter().map(|p| p.2.len()).collect::<Vec<_>>(),
            [100, 100, 50]
        );
        assert_eq!(
            published
                .iter()
                .flat_map(|p| p.2.clone())
                .collect::<Vec<_>>(),
            addresses(250)
        );
        for (payload, topic, _) in published.iter() {
            assert_eq!(topic, &super::tests::topic());
            assert_eq!(
                rfc4::Packet::decode(payload.as_slice()).unwrap(),
                chat(Some(SENDER))
            );
        }
    }

    #[tokio::test]
    async fn a_failed_batch_is_logged_and_the_rest_still_go() {
        let room = FakeRoom {
            fail_publish: Some(1),
            ..Default::default()
        };
        let outcome = route(
            &room,
            &members(301),
            &reaction(SENDER).encode_to_vec(),
            SENDER,
            &topic(),
        )
        .await;
        assert_eq!(outcome, Outcome::Delivered);
        assert_eq!(room.calls.load(Ordering::SeqCst), 4);
        assert_eq!(
            room.published
                .lock()
                .unwrap()
                .iter()
                .map(|p| p.2.len())
                .collect::<Vec<_>>(),
            [100, 100, 1]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_publish_that_never_answers_times_out_and_the_rest_still_go() {
        let room = FakeRoom {
            hang_publish: Some(0),
            ..Default::default()
        };
        let started = tokio::time::Instant::now();
        let outcome = route(
            &room,
            &members(150),
            &chat(None).encode_to_vec(),
            SENDER,
            &topic(),
        )
        .await;
        assert_eq!(outcome, Outcome::Delivered);
        assert_eq!(started.elapsed(), PUBLISH_TIMEOUT);
        assert_eq!(room.calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            room.published
                .lock()
                .unwrap()
                .iter()
                .map(|p| p.2.len())
                .collect::<Vec<_>>(),
            [50]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_published_is_not_delivered() {
        let failing = FakeRoom {
            fail_all: true,
            ..Default::default()
        };
        let outcome = route(
            &failing,
            &members(250),
            &chat(None).encode_to_vec(),
            SENDER,
            &topic(),
        )
        .await;
        assert_eq!(outcome, Outcome::Failed);
        assert_eq!(failing.calls.load(Ordering::SeqCst), 3);

        let hanging = FakeRoom {
            hang_publish: Some(0),
            ..Default::default()
        };
        let outcome = route(
            &hanging,
            &members(100),
            &chat(None).encode_to_vec(),
            SENDER,
            &topic(),
        )
        .await;
        assert_eq!(outcome, Outcome::Failed);
    }

    #[tokio::test]
    async fn the_room_dropping_mid_way_stops_the_rest_and_fails() {
        let room = FakeRoom {
            drop_after: Some(1),
            ..Default::default()
        };
        let outcome = route(
            &room,
            &members(150),
            &chat(None).encode_to_vec(),
            SENDER,
            &topic(),
        )
        .await;
        assert_eq!(outcome, Outcome::Failed);
        assert_eq!(room.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn everything_else_fails_and_publishes_nothing() {
        let chat_bytes = chat(None).encode_to_vec();
        let cases: Vec<(&str, FakeRoom, Members, Vec<u8>, String)> = vec![
            (
                "not a member",
                FakeRoom::default(),
                Members {
                    member: false,
                    ..members(3)
                },
                chat_bytes.clone(),
                topic(),
            ),
            (
                "no other member or inactive community",
                FakeRoom::default(),
                members(0),
                chat_bytes.clone(),
                topic(),
            ),
            (
                "store down",
                FakeRoom::default(),
                Members {
                    fail: true,
                    ..members(3)
                },
                chat_bytes.clone(),
                topic(),
            ),
            (
                "room down",
                FakeRoom {
                    down: AtomicBool::new(true),
                    ..Default::default()
                },
                members(3),
                chat_bytes.clone(),
                topic(),
            ),
            (
                "not chat",
                FakeRoom::default(),
                members(3),
                rfc4::Packet {
                    message: Some(rfc4::packet::Message::Position(Default::default())),
                    protocol_version: 100,
                }
                .encode_to_vec(),
                topic(),
            ),
            (
                "undecodable",
                FakeRoom::default(),
                members(3),
                vec![0xff, 0xff, 0xff],
                topic(),
            ),
            (
                "bad topic",
                FakeRoom::default(),
                members(3),
                chat_bytes.clone(),
                "community:abc-1".into(),
            ),
        ];
        for (name, room, members, payload, topic) in cases {
            assert_eq!(
                route(&room, &members, &payload, SENDER, &topic).await,
                Outcome::Failed,
                "{name}"
            );
            assert_eq!(room.calls.load(Ordering::SeqCst), 0, "{name}");
        }
    }

    #[test]
    fn outcome_labels_are_upstream_s_and_overloaded() {
        assert_eq!(
            Outcome::ALL.map(Outcome::label),
            [
                "delivered",
                "failed",
                "rejected_oversized",
                "rate_limited",
                "overloaded"
            ]
        );
    }
}
