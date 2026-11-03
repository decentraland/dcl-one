//! Social mutation signalling over the existing core-NATS connection. This has the same
//! transient, at-most-once delivery as upstream Redis pub/sub; no replay or retry ledger.
use super::{State, context::Context, proto::v2::*, pubsub::SocialEvent};
use crate::comms::gatekeeper::bus::NatsBus;
use crate::comms::protocol::bus::{SOCIAL_EVENTS_SUBJECT, wallet_token};
use crate::jobs::Jobs;
use base64::{Engine, engine::general_purpose::STANDARD};
use futures::StreamExt;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub(crate) const MAX_BYTES: usize = 64 * 1024 - SOCIAL_EVENTS_SUBJECT.len();
// Redis strings, used by the pinned publisher, have a 512 MiB limit. The
// transient SQL spill cache retains this upper bound without shrinking client metadata.
const MAX_STORED_BYTES: usize = 512 * 1024 * 1024;
const OUTBOUND_QUEUE: usize = 256;
const INBOUND_QUEUE: usize = 1000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scope {
    All,
    Members,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TargetKind {
    Friendship,
    Block,
    PrivateVoice,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Event {
    Target {
        recipient: String,
        event: TargetKind,
        payload: String,
    },
    CommunityVoice {
        scope: Scope,
        exclude: Option<String>,
        payload: String,
    },
    CommunityMember {
        community: Uuid,
        actor: String,
        status: i32,
    },
    CommunityDeleted {
        community: Uuid,
    },
    Reference {
        event_id: Uuid,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    source: Uuid,
    event: Event,
}

/// Binary spill form avoids base64 expansion for a legitimate large metadata
/// snapshot. It contains no call credentials and cannot refer to another row.
#[derive(Clone, PartialEq, Message)]
struct StoredCommunityVoice {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(bool, tag = "2")]
    everyone: bool,
    #[prost(string, optional, tag = "3")]
    exclude: Option<String>,
    #[prost(message, optional, tag = "4")]
    update: Option<CommunityVoiceChatUpdate>,
}

fn encode_proto(value: &impl Message) -> Option<String> {
    (value.encoded_len() <= MAX_STORED_BYTES).then(|| STANDARD.encode(value.encode_to_vec()))
}
fn decode_proto<T: Message + Default>(payload: &str) -> Option<T> {
    if payload.len() > MAX_STORED_BYTES.div_ceil(3) * 4 {
        return None;
    }
    let bytes = STANDARD.decode(payload).ok()?;
    if bytes.len() > MAX_STORED_BYTES {
        return None;
    }
    T::decode(bytes.as_slice()).ok()
}
fn address(value: &str) -> bool {
    wallet_token(value).as_deref() == Some(value)
}

impl Event {
    pub(crate) fn target(recipient: &str, event: &SocialEvent) -> Option<Self> {
        let (event, payload) = match event {
            SocialEvent::Friendship(value) => {
                let mut value = value.clone();
                if let Some(friendship_update::Update::Request(request)) = value.update.as_mut() {
                    if let Some(friend) = request.friend.as_mut() {
                        *friend = FriendProfile {
                            address: friend.address.clone(),
                            ..Default::default()
                        };
                    }
                }
                (TargetKind::Friendship, encode_proto(&value)?)
            }
            SocialEvent::Block(value) => (TargetKind::Block, encode_proto(value)?),
            SocialEvent::PrivateVoice(value) => (TargetKind::PrivateVoice, encode_proto(value)?),
            _ => return None, // Every topology event is already derived on every replica.
        };
        Some(Self::Target {
            recipient: recipient.to_lowercase(),
            event,
            payload,
        })
    }
    pub(crate) fn fits_queue(&self) -> bool {
        match self {
            Self::Target { payload, .. } | Self::CommunityVoice { payload, .. }
                if payload.len() > MAX_BYTES =>
            {
                false
            }
            _ => self
                .encode(Uuid::nil())
                .is_some_and(|bytes| bytes.len() <= MAX_BYTES),
        }
    }
    pub(crate) fn community_voice(
        scope: Scope,
        exclude: Option<&str>,
        update: &CommunityVoiceChatUpdate,
    ) -> Option<Self> {
        Some(Self::CommunityVoice {
            scope,
            exclude: exclude.map(str::to_lowercase),
            payload: encode_proto(update)?,
        })
    }
    fn validate(&self) -> bool {
        match self {
            Self::Target {
                recipient,
                event,
                payload,
            } => {
                address(recipient)
                    && match event {
                        TargetKind::Friendship => decode_proto::<FriendshipUpdate>(payload)
                            .is_some_and(|update| {
                                update.stream_closed.is_none()
                                    && update.update.as_ref().is_some_and(|value| {
                                        use friendship_update::Update::*;
                                        match value {
                                            Request(r) => {
                                                r.friend
                                                    .as_ref()
                                                    .is_some_and(|f| address(&f.address))
                                                    && Uuid::parse_str(&r.id).is_ok()
                                                    && r.message
                                                        .as_ref()
                                                        .is_none_or(|m| m.chars().count() <= 256)
                                            }
                                            Accept(r) => {
                                                r.user.as_ref().is_some_and(|u| address(&u.address))
                                            }
                                            Reject(r) => {
                                                r.user.as_ref().is_some_and(|u| address(&u.address))
                                            }
                                            Cancel(r) => {
                                                r.user.as_ref().is_some_and(|u| address(&u.address))
                                            }
                                            Delete(r) => {
                                                r.user.as_ref().is_some_and(|u| address(&u.address))
                                            }
                                            Block(r) => {
                                                r.user.as_ref().is_some_and(|u| address(&u.address))
                                            }
                                        }
                                    })
                            }),
                        TargetKind::Block => decode_proto::<BlockUpdate>(payload)
                            .is_some_and(|v| v.stream_closed.is_none() && address(&v.address)),
                        TargetKind::PrivateVoice => decode_proto::<PrivateVoiceChatUpdate>(payload)
                            .is_some_and(|v| {
                                v.stream_closed.is_none()
                                    && Uuid::parse_str(&v.call_id).is_ok()
                                    && PrivateVoiceChatStatus::try_from(v.status).is_ok()
                                    && v.caller.as_ref().is_none_or(|u| address(&u.address))
                                    && v.callee.is_none()
                                    && (v.status
                                        == PrivateVoiceChatStatus::VoiceChatRequested as i32)
                                        == v.caller.is_some()
                                    && (v.credentials.is_none()
                                        || v.status
                                            == PrivateVoiceChatStatus::VoiceChatAccepted as i32)
                            }),
                    }
            }
            Self::CommunityVoice {
                exclude, payload, ..
            } => {
                exclude.as_ref().is_none_or(|v| address(v))
                    && decode_proto::<CommunityVoiceChatUpdate>(payload).is_some_and(|v| {
                        v.stream_closed.is_none()
                            && Uuid::parse_str(&v.community_id).is_ok()
                            && CommunityVoiceChatStatus::try_from(v.status).is_ok()
                    })
            }
            Self::CommunityMember { actor, status, .. } => {
                address(actor) && ConnectivityStatus::try_from(*status).is_ok()
            }
            Self::CommunityDeleted { .. } | Self::Reference { .. } => true,
        }
    }
    pub(crate) fn encode(&self, source: Uuid) -> Option<Vec<u8>> {
        if !self.validate() {
            return None;
        }
        let bytes = serde_json::to_vec(&Envelope {
            version: 1,
            source,
            event: self.clone(),
        })
        .ok()?;
        (bytes.len() <= MAX_STORED_BYTES).then_some(bytes)
    }
    fn decode(bytes: &[u8], local: Uuid) -> Option<Self> {
        if bytes.len() > MAX_BYTES {
            return None;
        }
        let envelope: Envelope = serde_json::from_slice(bytes).ok()?;
        (envelope.version == 1 && envelope.source != local && envelope.event.validate())
            .then_some(envelope.event)
    }
    pub(crate) fn encode_stored(&self) -> Option<Vec<u8>> {
        let Self::CommunityVoice {
            scope,
            exclude,
            payload,
        } = self
        else {
            return None;
        };
        if !self.validate() {
            return None;
        }
        let update = decode_proto(payload)?;
        let stored = StoredCommunityVoice {
            version: 1,
            everyone: *scope == Scope::All,
            exclude: exclude.clone(),
            update: Some(update),
        };
        (stored.encoded_len() <= MAX_STORED_BYTES).then(|| stored.encode_to_vec())
    }
    fn decode_stored(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_STORED_BYTES {
            return None;
        }
        let stored = StoredCommunityVoice::decode(bytes).ok()?;
        if stored.version != 1 {
            return None;
        }
        let event = Self::community_voice(
            if stored.everyone {
                Scope::All
            } else {
                Scope::Members
            },
            stored.exclude.as_deref(),
            stored.update.as_ref()?,
        )?;
        event.validate().then_some(event)
    }
    /// Address-targeted updates need no SQL, so they enter their subscription
    /// queues directly in the broker callback instead of an earlier shared queue.
    fn deliver_target(&self, ctx: &Context) -> bool {
        let Self::Target {
            recipient,
            event,
            payload,
        } = self
        else {
            return false;
        };
        let event = match event {
            TargetKind::Friendship => decode_proto(payload).map(SocialEvent::Friendship),
            TargetKind::Block => decode_proto(payload).map(SocialEvent::Block),
            TargetKind::PrivateVoice => decode_proto(payload).map(SocialEvent::PrivateVoice),
        };
        if let Some(event) = event {
            ctx.pubsub().publish_local(recipient, event);
        }
        true
    }
    pub(crate) async fn deliver(self, ctx: &Context) {
        if self.deliver_target(ctx) {
            return;
        }
        match self {
            Self::Target { .. } => unreachable!("target delivered synchronously"),
            Self::CommunityVoice {
                scope,
                exclude,
                payload,
            } => {
                if let Some(update) = decode_proto(&payload) {
                    ctx.pubsub()
                        .deliver_community_voice(ctx.db().pool(), scope, exclude.as_deref(), update)
                        .await;
                }
            }
            Self::CommunityMember {
                community,
                actor,
                status,
            } => {
                if let Ok(status) = ConnectivityStatus::try_from(status) {
                    ctx.pubsub()
                        .deliver_community_member(ctx.db().pool(), community, &actor, status)
                        .await;
                }
            }
            Self::CommunityDeleted { community } => {
                ctx.pubsub()
                    .deliver_community_deleted(ctx.db().pool(), community)
                    .await
            }
            Self::Reference { event_id } => match ctx.db().social_event_payload(event_id).await {
                Ok(Some(bytes)) => {
                    if let Some(Self::CommunityVoice {
                        scope,
                        exclude,
                        payload,
                    }) = Self::decode_stored(&bytes)
                    {
                        if let Some(update) = decode_proto(&payload) {
                            ctx.pubsub()
                                .deliver_community_voice(
                                    ctx.db().pool(),
                                    scope,
                                    exclude.as_deref(),
                                    update,
                                )
                                .await;
                        }
                    }
                }
                Ok(None) => {
                    metrics::counter!("social_event_delivery_dropped", "reason" => "expired_payload").increment(1);
                }
                Err(error) => {
                    tracing::warn!(%error, "social event payload lookup failed");
                }
            },
        }
    }
}

impl State {
    /// Every serve consumes the same subject without a queue group. The connection is comms'
    /// existing link; social owns only these bounded application queues and local delivery.
    pub fn start_fanout(self: &Arc<Self>, jobs: &Jobs, bus: Arc<dyn NatsBus>) {
        let (outbound, mut outgoing) = mpsc::channel::<Event>(OUTBOUND_QUEUE);
        self.ctx.pubsub().attach_outbound(outbound);
        let (incoming, inbound) = mpsc::channel::<Event>(INBOUND_QUEUE);
        let source = Uuid::new_v4();
        let target_context = self.ctx.clone();
        let subscription = bus.subscribe(SOCIAL_EVENTS_SUBJECT, None, Arc::new(move |_, payload| {
            if let Some(event) = Event::decode(payload, source) {
                if event.deliver_target(&target_context) { return; }
                if incoming.try_send(event).is_err() {
                    metrics::counter!("social_event_delivery_dropped", "reason" => "inbound_queue").increment(1);
                }
            }
        }));
        let context = self.ctx.clone();
        // The callback stays synchronous and never awaits SQL on a bus task.
        // Source async handlers may complete out of order; bound independent
        // community work to four tasks and never exceed the shared pool size.
        let concurrency = context
            .db()
            .pool()
            .options()
            .get_max_connections()
            .clamp(1, 4) as usize;
        jobs.spawn("social-event-fanout", move |job| async move {
            let _subscription = subscription;
            let publish = async {
                while let Some(event) = outgoing.recv().await {
                    let Some(bytes) = event.encode(source) else {
                        metrics::counter!("social_event_delivery_dropped", "reason" => "invalid_outbound").increment(1);
                        continue;
                    };
                    if bytes.len() > MAX_BYTES {
                        metrics::counter!("social_event_delivery_dropped", "reason" => "oversized_outbound").increment(1);
                        continue;
                    }
                    if bus.publish(SOCIAL_EVENTS_SUBJECT, bytes).await != crate::comms::gatekeeper::bus::PublishOutcome::Submitted {
                        metrics::counter!("social_event_delivery_dropped", "reason" => "bus").increment(1);
                    }
                }
            };
            let deliver = futures::stream::unfold(inbound, |mut receiver| async move {
                receiver.recv().await.map(|event| (event, receiver))
            }).for_each_concurrent(concurrency, |event| {
                let context = &context;
                async move {
                    if tokio::time::timeout(std::time::Duration::from_secs(5), event.deliver(context)).await.is_err() {
                        metrics::counter!("social_event_delivery_dropped", "reason" => "delivery_timeout").increment(1);
                    }
                }
            });
            tokio::select! { _ = job.cancel.cancelled() => {}, _ = publish => {}, _ = deliver => {} }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const WALLET: &str = "0x1111111111111111111111111111111111111111";
    fn started(image_len: usize) -> Event {
        Event::community_voice(
            Scope::All,
            Some(WALLET),
            &CommunityVoiceChatUpdate {
                community_id: Uuid::new_v4().to_string(),
                status: CommunityVoiceChatStatus::CommunityVoiceChatStarted as i32,
                community_image: Some("x".repeat(image_len)),
                ..Default::default()
            },
        )
        .unwrap()
    }
    #[test]
    fn envelope_checks_version_route_identity_enum_and_loopback() {
        let source = Uuid::new_v4();
        let local = Uuid::new_v4();
        let event = Event::CommunityMember {
            community: Uuid::new_v4(),
            actor: WALLET.into(),
            status: ConnectivityStatus::Offline as i32,
        };
        let bytes = event.encode(source).unwrap();
        assert!(Event::decode(&bytes, local).is_some());
        assert!(Event::decode(&bytes, source).is_none());
        for (field, value) in [
            ("version", serde_json::json!(2)),
            ("source", serde_json::json!("bad")),
        ] {
            let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            invalid[field] = value;
            assert!(Event::decode(&serde_json::to_vec(&invalid).unwrap(), local).is_none());
        }
        for (field, value) in [
            ("actor", serde_json::json!("0xinvalid")),
            ("status", serde_json::json!(99)),
            ("kind", serde_json::json!("unknown")),
        ] {
            let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            invalid["event"][field] = value;
            assert!(Event::decode(&serde_json::to_vec(&invalid).unwrap(), local).is_none());
        }
    }
    #[test]
    fn wire_cap_is_inclusive_and_large_metadata_uses_lossless_nonrecursive_storage() {
        let event = started(0);
        let mut value = serde_json::to_value(Envelope {
            version: 1,
            source: Uuid::nil(),
            event,
        })
        .unwrap();
        let base = serde_json::to_vec(&value).unwrap();
        // JSON trailing whitespace is legal; test the decoder's exact byte boundary.
        let mut exact = base;
        exact.resize(MAX_BYTES, b' ');
        assert!(Event::decode(&exact, Uuid::new_v4()).is_some());
        exact.push(b' ');
        assert!(Event::decode(&exact, Uuid::new_v4()).is_none());
        value["event"]["extra"] = serde_json::json!(true);
        assert!(Event::decode(&serde_json::to_vec(&value).unwrap(), Uuid::new_v4()).is_none());
        let large = started(MAX_BYTES * 2);
        assert!(!large.fits_queue());
        let stored = large.encode_stored().unwrap();
        let decoded = Event::decode_stored(&stored).unwrap();
        assert_eq!(large.encode(Uuid::nil()), decoded.encode(Uuid::nil()));
        assert!(
            Event::Reference {
                event_id: Uuid::new_v4()
            }
            .encode_stored()
            .is_none()
        );
        assert!(
            Event::decode_stored(
                &Event::Reference {
                    event_id: Uuid::new_v4()
                }
                .encode(Uuid::nil())
                .unwrap()
            )
            .is_none()
        );
    }
    #[test]
    fn topology_events_stay_local_and_friend_profiles_are_resolved_at_rpc_delivery() {
        assert!(
            Event::target(
                WALLET,
                &SocialEvent::FriendConnectivity(FriendConnectivityUpdate::default())
            )
            .is_none()
        );
        let update = FriendshipUpdate {
            stream_closed: None,
            update: Some(friendship_update::Update::Request(
                friendship_update::RequestResponse {
                    id: Uuid::new_v4().to_string(),
                    created_at: 123,
                    message: Some("🦊".repeat(256)),
                    friend: Some(FriendProfile {
                        address: WALLET.into(),
                        profile_picture_url: "u".repeat(MAX_BYTES * 2),
                        ..Default::default()
                    }),
                },
            )),
        };
        let event = Event::target(WALLET, &SocialEvent::Friendship(update)).unwrap();
        assert!(event.fits_queue());
        let Event::Target { payload, .. } = event else {
            unreachable!()
        };
        let request = decode_proto::<FriendshipUpdate>(&payload).unwrap();
        let Some(friendship_update::Update::Request(request)) = request.update else {
            unreachable!()
        };
        assert_eq!(request.message.unwrap().chars().count(), 256);
        assert_eq!(request.friend.unwrap().profile_picture_url, "");
    }
}
