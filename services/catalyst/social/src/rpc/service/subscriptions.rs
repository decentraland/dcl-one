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
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<ServerStreamResponse<FriendshipUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = context.server_context.clone();
        let mut rx = ctx.pubsub().subscribe(&me);
        let (generator, yielder) = Generator::create();
        let closed = yielder.clone();
        let updates = async move {
            loop {
                let event = match rx.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "friendship subscription lagged; events dropped");
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };
                let SocialEvent::Friendship(update) = &*event else {
                    continue;
                };
                let mut update = update.clone();
                let actor = match update.update.as_ref() {
                    Some(friendship_update::Update::Request(request)) => {
                        request.friend.as_ref().map(|friend| &friend.address)
                    }
                    Some(friendship_update::Update::Accept(response)) => {
                        response.user.as_ref().map(|user| &user.address)
                    }
                    Some(friendship_update::Update::Reject(response)) => {
                        response.user.as_ref().map(|user| &user.address)
                    }
                    Some(friendship_update::Update::Delete(response)) => {
                        response.user.as_ref().map(|user| &user.address)
                    }
                    Some(friendship_update::Update::Cancel(response)) => {
                        response.user.as_ref().map(|user| &user.address)
                    }
                    _ => None,
                };
                let Some(actor) = actor.cloned() else {
                    continue;
                };
                // The source RPC subscription retrieves a profile for every action.
                // The raw event remains available to the address-only JSON stream.
                let Ok(mut profiles) = ctx
                    .profiles()
                    .get_profiles_result(std::slice::from_ref(&actor))
                    .await
                else {
                    continue;
                };
                let Some(profile) = profiles.remove(&actor.to_lowercase()) else {
                    continue;
                };
                if let Some(friendship_update::Update::Request(request)) = update.update.as_mut() {
                    request.friend =
                        Some(crate::rpc::profiles::friend_profile(&actor, Some(&profile)));
                }
                if yielder.r#yield(update).await.is_err() {
                    break;
                }
            }
        };
        tokio::spawn(async move {
            tokio::select! {
                _ = cancellation.cancelled() => {},
                _ = closed.closed() => {},
                _ = updates => {},
            }
        });
        Ok(generator)
    }

    /// The source starts with currently online friends, then consumes live updates.
    pub(crate) async fn subscribe_to_friend_connectivity_updates(
        &self,
        context: ProcedureContext<Context>,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<ServerStreamResponse<FriendConnectivityUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = context.server_context.clone();
        let mut rx = ctx.pubsub().subscribe(&me);

        let (generator, yielder) =
            Generator::bounded(16, crate::comms::topology::TOPOLOGY_RECEIPT_TTL);
        let closed = yielder.clone();
        let updates = async move {
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
                    break;
                };
                if page.is_empty() {
                    break;
                }
                after = page.last().unwrap().clone();
                let Ok(Ok(profiles)) = tokio::time::timeout(
                    crate::comms::topology::TOPOLOGY_RECEIPT_TTL,
                    ctx.profiles().friend_profiles(&page),
                )
                .await
                else {
                    break;
                };
                for friend in profiles {
                    if rx.len() > crate::rpc::pubsub::CHANNEL_CAP {
                        return;
                    }
                    let snapshot = ctx.topology().snapshot();
                    let online = snapshot.is_online(&friend.address);
                    if !online {
                        continue;
                    }
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
                        if let Some(mut update) = update {
                            let Some(friend) = update.friend.as_ref() else {
                                continue;
                            };
                            let Ok(mut profiles) = ctx
                                .profiles()
                                .get_profiles_result(std::slice::from_ref(&friend.address))
                                .await
                            else {
                                continue;
                            };
                            let Some(profile) = profiles.remove(&friend.address.to_lowercase())
                            else {
                                continue;
                            };
                            update.friend = Some(crate::rpc::profiles::friend_profile(
                                &friend.address,
                                Some(&profile),
                            ));
                            let friend = update.friend.as_ref().unwrap();
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
        };
        tokio::spawn(async move {
            tokio::select! {
                _ = cancellation.cancelled() => {},
                _ = closed.closed() => {},
                _ = updates => {},
            }
        });
        Ok(generator)
    }

    pub(crate) async fn subscribe_to_block_updates(
        &self,
        context: ProcedureContext<Context>,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<ServerStreamResponse<BlockUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::Block(u) => Some(u.clone()),
                _ => None,
            },
            cancellation,
        ))
    }

    pub(crate) async fn subscribe_to_community_member_connectivity_updates(
        &self,
        context: ProcedureContext<Context>,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<ServerStreamResponse<CommunityMemberConnectivityUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = context.server_context.clone();
        // Install live receiver before any SQL/profile await, including initialization.
        let mut rx = ctx.pubsub().subscribe(&me);
        let (generator, yielder) =
            Generator::bounded(16, crate::comms::topology::TOPOLOGY_RECEIPT_TTL);
        let closed = yielder.clone();
        let updates = async move {
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
                            SocialEvent::CommunityMemberStatus(update) => {
                                // delivered as published: a bounded stream
                                // sends guarded items only, so its guard
                                // always admits it
                                let admit: crate::rpc::drpc::stream_protocol::DeliveryGuard =
                                    std::sync::Arc::new(|| Box::pin(async { Ok(true) }));
                                if yielder.yield_guarded(update.clone(), admit).await.is_err() {
                                    break;
                                }
                                continue;
                            }
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
        };
        tokio::spawn(async move {
            tokio::select! {
                _ = cancellation.cancelled() => {},
                _ = closed.closed() => {},
                _ = updates => {},
            }
        });
        Ok(generator)
    }

    pub(crate) async fn subscribe_to_private_voice_chat_updates(
        &self,
        context: ProcedureContext<Context>,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<ServerStreamResponse<PrivateVoiceChatUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        // Upstream `parseEmittedUpdateToPrivateVoiceChatUpdate`
        // (`subscribe-to-private-voice-chat-updates.ts:36-41`): the caller only
        // on REQUESTED, never the callee.
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::PrivateVoice(u) => Some(PrivateVoiceChatUpdate {
                    caller: u
                        .caller
                        .clone()
                        .filter(|_| u.status == PrivateVoiceChatStatus::VoiceChatRequested as i32),
                    callee: None,
                    ..u.clone()
                }),
                _ => None,
            },
            cancellation,
        ))
    }

    pub(crate) async fn subscribe_to_community_voice_chat_updates(
        &self,
        context: ProcedureContext<Context>,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> Result<ServerStreamResponse<CommunityVoiceChatUpdate>, SocialError> {
        let me = Self::caller(&context)?;
        Ok(stream_for(
            context.server_context.pubsub(),
            &me,
            |e| match e {
                SocialEvent::CommunityVoice(u) => Some(u.clone()),
                _ => None,
            },
            cancellation,
        ))
    }
}

fn friend_replay_update(friend: FriendProfile, online: bool) -> FriendConnectivityUpdate {
    FriendConnectivityUpdate {
        stream_closed: None,
        friend: Some(friend),
        status: if online {
            ConnectivityStatus::Online
        } else {
            ConnectivityStatus::Offline
        } as i32,
    }
}
