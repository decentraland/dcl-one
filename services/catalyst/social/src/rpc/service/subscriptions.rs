use super::helpers::{SocialError, stream_for};
use super::{ServerStreamResponse, SocialServiceImpl};
use crate::rpc::context::Context;
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::drpc::stream_protocol::Generator;
use crate::rpc::proto::v2::*;
use crate::rpc::pubsub::SocialEvent;
use tokio::sync::broadcast::error::RecvError;

impl SocialServiceImpl {
    pub(crate) async fn subscribe_to_friendship_updates(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<ServerStreamResponse<FriendshipUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::Friendship(u) => Some(u.clone()),
                _ => None,
            },
        ))
    }

    /// Replays both current statuses for every eligible friend before live updates.
    /// ACK-paced admission repairs a retained client cache without a reconnect/reset.
    pub(crate) async fn subscribe_to_friend_connectivity_updates(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<ServerStreamResponse<FriendConnectivityUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = context.server_context.clone();
        let mut rx = ctx.pubsub().subscribe(&me);

        let (generator, yielder) =
            Generator::bounded(16, crate::comms::topology::TOPOLOGY_RECEIPT_TTL);
        tokio::spawn(async move {
            let mut after = String::new();
            loop {
                if rx.len() > crate::rpc::pubsub::CHANNEL_CAP {
                    return;
                }
                let Ok(Ok(page)) = tokio::time::timeout(
                    crate::comms::topology::TOPOLOGY_RECEIPT_TTL,
                    ctx.db().connectivity_friends_page(&me, &after, true),
                )
                .await
                else {
                    return;
                };
                if page.is_empty() {
                    break;
                }
                after = page.last().unwrap().clone();
                let Ok(Ok(profiles)) = tokio::time::timeout(
                    crate::comms::topology::TOPOLOGY_RECEIPT_TTL,
                    ctx.profiles().connectivity_profiles(&page),
                )
                .await
                else {
                    return;
                };
                for friend in profiles {
                    if rx.len() > crate::rpc::pubsub::CHANNEL_CAP {
                        return;
                    }
                    let snapshot = ctx.topology().snapshot();
                    let online = snapshot.is_online(&friend.address);
                    let guard = ctx.friend_delivery_guard(
                        me.clone(),
                        friend.address.clone(),
                        online,
                        snapshot.generation,
                    );
                    let update = friend_replay_update(friend, online);
                    if !matches!(
                        yielder.yield_confirmed(update, guard).await,
                        Ok(crate::rpc::drpc::stream_protocol::DeliveryCompletion::Acknowledged)
                    ) {
                        // A coalesced transient may not produce another live event. Restart
                        // this replay rather than certifying an omitted current status.
                        return;
                    }
                }
                if page.len() < 256 {
                    break;
                }
            }
            loop {
                let event =
                    tokio::select! { _ = yielder.closed() => break, event = rx.recv() => event };
                match event {
                    Ok(ev) => {
                        if matches!(&*ev, SocialEvent::ConnectivityResync) {
                            break;
                        }
                        let update = match &*ev {
                            SocialEvent::FriendConnectivity(u) => Some(u.clone()),
                            SocialEvent::TopologyFriendConnectivity(u, generation) => u
                                .friend
                                .as_ref()
                                .filter(|friend| {
                                    ctx.topology().queued_status_is_current(
                                        *generation,
                                        &friend.address,
                                        u.status == ConnectivityStatus::Online as i32,
                                    )
                                })
                                .map(|_| u.clone()),
                            _ => None,
                        };
                        if let Some(update) = update {
                            let Some(friend) = update.friend.as_ref() else {
                                continue;
                            };
                            let generation = match &*ev {
                                SocialEvent::TopologyFriendConnectivity(_, generation) => {
                                    *generation
                                }
                                _ => ctx.topology().snapshot().generation,
                            };
                            let guard = ctx.friend_delivery_guard(
                                me.clone(),
                                friend.address.clone(),
                                update.status == ConnectivityStatus::Online as i32,
                                generation,
                            );
                            if !matches!(
                                tokio::time::timeout(
                                    crate::comms::topology::TOPOLOGY_RECEIPT_TTL,
                                    yielder.yield_guarded(update, guard)
                                )
                                .await,
                                Ok(Ok(_))
                            ) {
                                break;
                            }
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            "friend connectivity subscription lagged; events dropped"
                        );
                        break;
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        });
        Ok(generator)
    }

    pub(crate) async fn subscribe_to_block_updates(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<ServerStreamResponse<BlockUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::Block(u) => Some(u.clone()),
                _ => None,
            },
        ))
    }

    pub(crate) async fn subscribe_to_community_member_connectivity_updates(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<ServerStreamResponse<CommunityMemberConnectivityUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = context.server_context.clone();
        // Install live receiver before any SQL/profile await, including initialization.
        let mut rx = ctx.pubsub().subscribe(&me);
        let (generator, yielder) =
            Generator::bounded(16, crate::comms::topology::TOPOLOGY_RECEIPT_TTL);
        tokio::spawn(async move {
            let mut after = None;
            loop {
                if rx.len() > crate::rpc::pubsub::CHANNEL_CAP {
                    return;
                }
                let Ok(Ok(page)) = tokio::time::timeout(
                    crate::comms::topology::TOPOLOGY_RECEIPT_TTL,
                    ctx.db().connectivity_members_page(&me, after.take()),
                )
                .await
                else {
                    return;
                };
                if page.is_empty() {
                    break;
                }
                after = page.last().cloned();
                for (community, member) in &page {
                    if rx.len() > crate::rpc::pubsub::CHANNEL_CAP {
                        return;
                    }
                    let snapshot = ctx.topology().snapshot();
                    let online = snapshot.is_online(member);
                    let Some(update) = community_replay_update(&me, *community, member, online)
                    else {
                        continue;
                    };
                    let guard = ctx.community_delivery_guard(
                        me.clone(),
                        member.clone(),
                        online,
                        snapshot.generation,
                        *community,
                    );
                    if !matches!(
                        yielder.yield_confirmed(update, guard).await,
                        Ok(crate::rpc::drpc::stream_protocol::DeliveryCompletion::Acknowledged)
                    ) {
                        // A coalesced transient may not produce another live event. Restart
                        // this replay rather than certifying an omitted current status.
                        return;
                    }
                }
                if page.len() < 256 {
                    break;
                }
            }
            loop {
                let event =
                    tokio::select! { _ = yielder.closed() => break, event = rx.recv() => event };
                match event {
                    Ok(event) => {
                        if matches!(&*event, SocialEvent::ConnectivityResync) {
                            break;
                        }
                        let update = match &*event {
                            SocialEvent::TopologyCommunityMember(update, _)
                            | SocialEvent::CommunityMember(update) => update.clone(),
                            _ => continue,
                        };
                        let Some(guard) = ctx.event_delivery_guard(&me, &event) else {
                            continue;
                        };
                        if yielder.yield_guarded(update, guard).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) | Err(RecvError::Closed) => break,
                }
            }
        });
        Ok(generator)
    }

    pub(crate) async fn subscribe_to_private_voice_chat_updates(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<ServerStreamResponse<PrivateVoiceChatUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::PrivateVoice(u) => Some(u.clone()),
                _ => None,
            },
        ))
    }

    pub(crate) async fn subscribe_to_community_voice_chat_updates(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<ServerStreamResponse<CommunityVoiceChatUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::CommunityVoice(u) => Some(u.clone()),
                _ => None,
            },
        ))
    }
}

fn friend_replay_update(friend: FriendProfile, online: bool) -> FriendConnectivityUpdate {
    FriendConnectivityUpdate {
        friend: Some(friend),
        status: if online {
            ConnectivityStatus::Online
        } else {
            ConnectivityStatus::Offline
        } as i32,
    }
}
fn community_replay_update(
    me: &str,
    community: uuid::Uuid,
    member: &str,
    online: bool,
) -> Option<CommunityMemberConnectivityUpdate> {
    if member.eq_ignore_ascii_case(me) {
        return None;
    }
    Some(CommunityMemberConnectivityUpdate {
        community_id: community.to_string(),
        member: Some(User {
            address: member.to_string(),
        }),
        status: if online {
            ConnectivityStatus::Online
        } else {
            ConnectivityStatus::Offline
        } as i32,
    })
}
#[cfg(test)]
mod replay_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    const ME: &str = "0x1111111111111111111111111111111111111111";
    const A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[tokio::test(start_paused = true)]
    async fn encoded_drpc_replay_clears_retained_offline_and_clean_end_is_retryable() {
        use crate::rpc::drpc::{
            messages_handlers::StreamsHandler,
            rpc_protocol::{RpcMessageTypes, StreamMessage, parse::build_message_identifier},
            stream_protocol::Generator,
            transport::{Transport, memory::MemoryTransport},
        };
        use prost::Message;
        use std::sync::Arc;
        let (client, server) = MemoryTransport::create();
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let (source, yielder) =
            Generator::bounded(16, crate::comms::topology::TOPOLOGY_RECEIPT_TTL);
        let encoded = Generator::from_generator(source, |v: FriendConnectivityUpdate| {
            Some(v.encode_to_vec())
        });
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(Arc::new(server), encoded, 30, 30)
                .await
        });
        let producer = tokio::spawn(async move {
            let offline =
                friend_replay_update(crate::rpc::profiles::friend_profile(A, None), false);
            yielder
                .yield_confirmed(offline, Arc::new(|| Box::pin(async { Ok(true) })))
                .await
                .unwrap();
        });
        let mut retained_online = HashSet::from([A.to_string()]);
        let bytes = client.receive().await.unwrap();
        let frame = StreamMessage::decode(bytes.as_slice()).unwrap();
        let update = FriendConnectivityUpdate::decode(frame.payload.as_slice()).unwrap();
        assert_eq!(update.status, ConnectivityStatus::Offline as i32);
        retained_online.remove(&update.friend.unwrap().address);
        handler.clone().message_acknowledged_by_peer(
            30,
            StreamMessage {
                port_id: 30,
                sequence_id: frame.sequence_id,
                ack: true,
                message_identifier: build_message_identifier(RpcMessageTypes::StreamAck as u32, 30),
                ..Default::default()
            }
            .encode_to_vec(),
        );
        let terminal = StreamMessage::decode(client.receive().await.unwrap().as_slice()).unwrap();
        assert!(
            terminal.closed,
            "pinned KeepServerStreamOpen retries clean close without resetting tracker"
        );
        assert!(retained_online.is_empty());
        assert!(producer.await.is_ok());
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn encoded_drpc_community_initial_replay_repairs_retained_sets_and_preserves_self_membership()
     {
        use crate::rpc::drpc::{
            messages_handlers::StreamsHandler,
            rpc_protocol::{RpcMessageTypes, StreamMessage, parse::build_message_identifier},
            stream_protocol::Generator,
            transport::{Transport, memory::MemoryTransport},
        };
        use prost::Message;
        use std::sync::Arc;
        let first = uuid::Uuid::from_u128(1);
        let second = uuid::Uuid::from_u128(2);
        let mut joined = HashSet::from([first, second]);
        let mut members = HashMap::from([
            (first, HashSet::from([A.to_string()])),
            (second, HashSet::new()),
        ]);
        let (client, server) = MemoryTransport::create();
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let (source, yielder) =
            Generator::bounded(16, crate::comms::topology::TOPOLOGY_RECEIPT_TTL);
        let encoded = Generator::from_generator(source, |v: CommunityMemberConnectivityUpdate| {
            Some(v.encode_to_vec())
        });
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(Arc::new(server), encoded, 31, 31)
                .await
        });
        let producer = tokio::spawn(async move {
            for (community, member, online) in
                [(first, A, false), (second, A, true), (first, ME, false)]
            {
                if let Some(update) = community_replay_update(ME, community, member, online) {
                    yielder
                        .yield_confirmed(update, Arc::new(|| Box::pin(async { Ok(true) })))
                        .await
                        .unwrap();
                }
            }
        });
        let mut values = 0;
        loop {
            let frame = StreamMessage::decode(client.receive().await.unwrap().as_slice()).unwrap();
            if frame.closed {
                break;
            }
            let update =
                CommunityMemberConnectivityUpdate::decode(frame.payload.as_slice()).unwrap();
            let community: uuid::Uuid = update.community_id.parse().unwrap();
            let member = update.member.unwrap().address;
            if member == ME {
                if update.status == ConnectivityStatus::Offline as i32 {
                    joined.remove(&community);
                }
            } else if update.status == ConnectivityStatus::Online as i32 {
                members.entry(community).or_default().insert(member);
            } else {
                members.entry(community).or_default().remove(&member);
            }
            values += 1;
            handler.clone().message_acknowledged_by_peer(
                31,
                StreamMessage {
                    port_id: 31,
                    sequence_id: frame.sequence_id,
                    ack: true,
                    message_identifier: build_message_identifier(
                        RpcMessageTypes::StreamAck as u32,
                        31,
                    ),
                    ..Default::default()
                }
                .encode_to_vec(),
            );
        }
        assert_eq!(values, 2);
        assert_eq!(joined.len(), 2);
        assert!(!members[&first].contains(A));
        assert!(members[&second].contains(A));
        assert!(producer.await.is_ok());
        assert!(task.await.unwrap().is_ok());
    }

    // Source-pinned Unity399e709 models: friends retain status by address and
    // community other-member sets change only on explicit status updates.
    #[test]
    fn explicit_offline_replay_repairs_retained_friend_without_reset_or_reconnect() {
        let mut tracker = HashMap::from([(A.to_string(), ConnectivityStatus::Online as i32)]);
        // The only live OFFLINE was lost. A clean new stream does not clear tracker.
        let update = friend_replay_update(crate::rpc::profiles::friend_profile(A, None), false);
        assert_eq!(update.friend.as_ref().unwrap().address, A);
        tracker.insert(update.friend.unwrap().address, update.status);
        assert_eq!(tracker[A], ConnectivityStatus::Offline as i32);
    }

    #[test]
    fn community_replay_restores_other_members_but_never_synthesizes_self_membership_offline() {
        let first = uuid::Uuid::from_u128(1);
        let second = uuid::Uuid::from_u128(2);
        let mut joined = HashSet::from([first, second]);
        let mut online: HashMap<uuid::Uuid, HashSet<String>> = HashMap::from([
            (first, HashSet::from([A.to_string()])),
            (second, HashSet::new()),
        ]);
        for (community, member, status) in
            [(first, A, false), (second, A, true), (first, ME, false)]
        {
            if let Some(update) = community_replay_update(ME, community, member, status) {
                let member = update.member.unwrap().address;
                // Unity's same event also changes community membership for self.
                if member == ME {
                    if update.status == ConnectivityStatus::Offline as i32 {
                        joined.remove(&community);
                    }
                } else if update.status == ConnectivityStatus::Online as i32 {
                    online.entry(community).or_default().insert(member);
                } else {
                    online.entry(community).or_default().remove(&member);
                }
            }
        }
        assert!(joined.contains(&first) && joined.contains(&second));
        assert!(!online[&first].contains(A));
        assert!(online[&second].contains(A));
        assert!(community_replay_update(ME, first, &ME.to_uppercase(), false).is_none());
    }

    #[test]
    fn bounded_pages_replay_both_statuses_for_tail_and_multiple_communities() {
        let mut tracker = HashMap::new();
        let mut pages = Vec::new();
        let addresses: Vec<_> = (0..600).map(|n| format!("0x{n:040x}")).collect();
        for (page, addresses) in addresses.chunks(256).enumerate() {
            pages.push((page, addresses.len()));
            for (i, address) in addresses.iter().enumerate() {
                let update = friend_replay_update(
                    crate::rpc::profiles::friend_profile(address, None),
                    i % 2 == 0,
                );
                tracker.insert(update.friend.unwrap().address, update.status);
                for community in [uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2)] {
                    assert!(community_replay_update(ME, community, address, i % 2 == 0).is_some());
                }
            }
        }
        assert_eq!(pages, vec![(0, 256), (1, 256), (2, 88)]);
        assert_eq!(tracker.len(), 600);
        assert!(
            tracker
                .values()
                .any(|s| *s == ConnectivityStatus::Online as i32)
        );
        assert!(
            tracker
                .values()
                .any(|s| *s == ConnectivityStatus::Offline as i32)
        );
    }

    #[test]
    fn oversized_optional_profile_uses_verified_existing_unavailable_representation() {
        use prost::Message;
        let full = FriendProfile {
            address: A.to_string(),
            name: "n".repeat(32 * 1024),
            profile_picture_url: "u".repeat(32 * 1024),
            has_claimed_name: true,
            name_color: None,
        };
        let profile = crate::rpc::profiles::connectivity_profile(full);
        assert_eq!(profile, crate::rpc::profiles::friend_profile(A, None));
        let update = friend_replay_update(profile, false);
        assert_eq!(update.friend.as_ref().unwrap().address, A);
        assert_eq!(update.status, ConnectivityStatus::Offline as i32);
        assert!(
            update.encoded_len() < crate::rpc::drpc::transport::MAX_GUARDED_MESSAGE_BYTES - 256
        );
    }
}
