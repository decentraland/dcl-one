//! The live half of `SubscribeToCommunityMemberConnectivityUpdates` over the
//! socket, against upstream's `subscribe-to-community-member-connectivity-updates`
//! spec (`decentraland/social-service-ea` at `fb3dcb8`). Catalyst passes the
//! subscriber's own update, as upstream does. Recorded difference (RPC.md,
//! friends services, "subscribe-to-community-member-connectivity-updates (7)"):
//! upstream passes every routed update through
//! (`src/controllers/handlers/rpc/subscribe-to-community-member-connectivity-updates.ts:41`,
//! `shouldHandleUpdate: () => true`); Catalyst drops at delivery an update whose
//! member is not a co-member or whose status is not the current presence.

use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_update_reaches_the_subscriber_only_when_current_and_co_member() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    // The server runs the private-voice expiry sweep against the scratch pool.
    let _turn = VOICE_TABLE.lock().await;
    let server = spawn_server(Some(pool.clone()), 5).await;
    let (me, _client, social) = explorer_as(&server, &PrivateKeySigner::random()).await;
    let community = seed_community(&pool, true, &me, "owner").await;
    let stranger = addr("cmc1");
    let mut updates = subscribed(
        &server,
        &me,
        social.subscribe_to_community_member_connectivity_updates(),
    )
    .await;
    let update = |member: &str, status: ConnectivityStatus| {
        SocialEvent::CommunityMember(CommunityMemberConnectivityUpdate {
            stream_closed: None,
            community_id: community.to_string(),
            member: Some(User {
                address: member.to_string(),
            }),
            status: status as i32,
        })
    };
    let pubsub = server.state.ctx.pubsub();
    // No presence was received, so ONLINE is not the current status: dropped.
    pubsub.publish(&me, update(&me, ConnectivityStatus::Online));
    // Not a member of the community: dropped.
    pubsub.publish(&me, update(&stranger, ConnectivityStatus::Offline));
    // The subscriber's own, current: delivered.
    pubsub.publish(&me, update(&me, ConnectivityStatus::Offline));
    let got = next_update(&mut updates).await;
    assert_eq!(got.member.map(|m| m.address), Some(me.clone()));
    assert_eq!(got.status, ConnectivityStatus::Offline as i32);
    drop_community(&pool, community).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's `updates.spec.ts` "online members across communities"
/// (`src/logic/updates.ts:165-183`): a member's new status reaches its
/// co-members in each of its communities, and no one outside them.
#[tokio::test]
async fn a_members_status_reaches_its_co_members_in_each_community() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let ctx = test_ctx(
        Db::new(pool.clone()),
        Arc::new(FakeVoiceRooms::new(pool.clone())),
    );
    let (x, first_peer, second_peer, stranger) =
        (addr("cmf1"), addr("cmf2"), addr("cmf3"), addr("cmf4"));
    let first = seed_community(&pool, true, &x, "member").await;
    let second = seed_community(&pool, true, &x, "member").await;
    for (community, peer) in [(first, &first_peer), (second, &second_peer)] {
        sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')")
            .bind(community)
            .bind(peer)
            .execute(&pool)
            .await
            .unwrap();
    }
    ctx.topology()
        .receive(&pulse_frame(vec![x.clone()]))
        .unwrap();
    let generation = ctx.topology().snapshot().generation;
    let mut receivers: Vec<_> = [&first_peer, &second_peer, &stranger]
        .iter()
        .map(|a| ctx.pubsub().subscribe(a))
        .collect();
    let mut cursor = crate::rpc::context::ConnectivityCursor::default();
    for _ in 0..4 {
        let (next, done) = ctx
            .fan_topology_page(&x, true, generation, cursor)
            .await
            .unwrap();
        cursor = next;
        if done {
            break;
        }
    }
    let heard = |rx: &mut tokio::sync::broadcast::Receiver<Arc<SocialEvent>>| {
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|e| match e.as_ref() {
                SocialEvent::TopologyCommunityMember(u, _) => Some((
                    u.community_id.clone(),
                    u.member.as_ref().map(|m| m.address.clone()),
                    u.status,
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let online = ConnectivityStatus::Online as i32;
    assert_eq!(
        heard(&mut receivers[0]),
        [(first.to_string(), Some(x.clone()), online)]
    );
    assert_eq!(
        heard(&mut receivers[1]),
        [(second.to_string(), Some(x.clone()), online)]
    );
    assert!(heard(&mut receivers[2]).is_empty());
    drop_community(&pool, first).await;
    drop_community(&pool, second).await;
}

/// Upstream's `updates.spec.ts` "queues live updates emitted during the
/// snapshot" (`src/logic/updates.ts:513-524`): an update published after
/// subscribing follows the replay. The replay is not held, so this does not
/// show that an update published during the snapshot is queued.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_live_update_published_after_subscribing_follows_the_snapshot() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let server = spawn_server(Some(pool.clone()), 5).await;
    let db = Db::new(pool.clone());
    let (alice, _alice_client, alice_social) =
        explorer_as(&server, &PrivateKeySigner::random()).await;
    let (bob, _bob_client, _bob_social) = explorer_as(&server, &PrivateKeySigner::random()).await;
    cleanup_pair(&db, &alice, &bob).await;
    db.apply_friendship_action(&alice, &bob, "accept", true, None, None)
        .await
        .unwrap();
    seed_profiles(&server.state.ctx, &[&bob]).await;
    server
        .state
        .ctx
        .topology()
        .receive(&pulse_frame(vec![alice.clone(), bob.clone()]))
        .unwrap();
    let mut updates = subscribed(
        &server,
        &alice,
        alice_social.subscribe_to_friend_connectivity_updates(),
    )
    .await;
    let first = next_update(&mut updates).await.friend.unwrap();
    assert_eq!(first.address, bob);
    // Source resolves the profile at RPC delivery, so publisher-provided display
    // fields cannot mark an event. Consume the initial item before publishing.
    let mut marked = crate::rpc::profiles::friend_profile(&bob, None);
    marked.name = "publisher placeholder".into();
    server.state.ctx.pubsub().publish(
        &alice,
        SocialEvent::FriendConnectivity(FriendConnectivityUpdate {
            stream_closed: None,
            friend: Some(marked),
            status: ConnectivityStatus::Online as i32,
        }),
    );
    let live = next_update(&mut updates).await.friend.unwrap();
    assert_eq!(live.address, bob);
    assert_ne!(live.name, "publisher placeholder");
    cleanup_pair(&db, &alice, &bob).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// A membership change reaches the stream as published (upstream
/// `communityMemberStatusHandler`, `src/logic/updates.ts:262-297` at
/// `b6b2cef`): the wallet that left hears its own OFFLINE though it is no
/// longer a member, and a co-member hears a join's ONLINE whatever the
/// joiner's presence; neither meets the co-member or presence check of a
/// connectivity update.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_membership_change_reaches_the_departed_wallet_and_the_members() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let server = spawn_server(Some(pool.clone()), 5).await;
    let (me, _client, social) = explorer_as(&server, &PrivateKeySigner::random()).await;
    let owner = addr("cms1");
    let joiner = addr("cms2");
    let community = seed_community(&pool, true, &owner, "owner").await;
    let mut updates = subscribed(
        &server,
        &me,
        social.subscribe_to_community_member_connectivity_updates(),
    )
    .await;
    let pubsub = server.state.ctx.pubsub();
    // `me` is not a member (it left): it still hears its own OFFLINE
    pubsub
        .community_member_status(&pool, community, &me, ConnectivityStatus::Offline)
        .await;
    let got = next_update(&mut updates).await;
    assert_eq!(
        (
            got.community_id.as_str(),
            got.member.map(|m| m.address),
            got.status
        ),
        (
            community.to_string().as_str(),
            Some(me.clone()),
            ConnectivityStatus::Offline as i32
        )
    );
    // now a member: a join's ONLINE arrives though the joiner is not online
    sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')")
        .bind(community)
        .bind(&me)
        .execute(&pool)
        .await
        .unwrap();
    pubsub
        .community_member_status(&pool, community, &joiner, ConnectivityStatus::Online)
        .await;
    let got = next_update(&mut updates).await;
    assert_eq!(
        (got.member.map(|m| m.address), got.status),
        (Some(joiner.clone()), ConnectivityStatus::Online as i32)
    );
    drop_community(&pool, community).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rpc_friend_connectivity_preserves_a_profile_larger_than_sixteen_kib() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let server = spawn_server(Some(pool.clone()), 5).await;
    let (me, _client, social) = explorer_as(&server, &PrivateKeySigner::random()).await;
    let friend = addr("large-connectivity");
    let picture = "https://profiles.example/".to_string() + &"u".repeat(70_000);
    let entity = seed_profile(&pool, &friend, "Large profile").await;
    let metadata = serde_json::json!({"v":{"avatars":[{"userId":friend,"name":"Large profile","hasClaimedName":true,"avatar":{"snapshots":{"face256":picture}}}]}});
    sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=$1")
        .bind(&entity)
        .bind(metadata)
        .execute(&pool)
        .await
        .unwrap();
    server
        .state
        .ctx
        .db()
        .apply_friendship_action(&me, &friend, "accept", true, None, None)
        .await
        .unwrap();
    server
        .state
        .ctx
        .topology()
        .receive(&pulse_frame(vec![me.clone(), friend.clone()]))
        .unwrap();
    let mut updates = subscribed(
        &server,
        &me,
        social.subscribe_to_friend_connectivity_updates(),
    )
    .await;
    let initial = next_update(&mut updates).await;
    assert_eq!(initial.friend.unwrap().profile_picture_url, picture);
    server.state.ctx.pubsub().publish(
        &me,
        SocialEvent::FriendConnectivity(FriendConnectivityUpdate {
            stream_closed: None,
            status: ConnectivityStatus::Online as i32,
            friend: Some(crate::rpc::profiles::friend_profile(&friend, None)),
        }),
    );
    let live = next_update(&mut updates).await;
    assert_eq!(live.friend.unwrap().profile_picture_url, picture);
    drop_profile(&pool, &friend, &entity).await;
    cleanup_pair(server.state.ctx.db(), &me, &friend).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_large_guarded_profile_retires_a_real_socket_that_stops_reading() {
    use std::sync::atomic::Ordering;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let server = spawn_server(Some(pool.clone()), 5).await;
    let key = PrivateKeySigner::random();
    let me = format!("{:#x}", key.address());
    let tcp = tokio::net::TcpSocket::new_v4().unwrap();
    tcp.set_recv_buffer_size(1024).unwrap();
    let tcp = tcp.connect(server.addr).await.unwrap();
    let (mut socket, _) = tokio_tungstenite::client_async(
        format!("ws://{}/social-rpc", server.addr),
        tokio_tungstenite::MaybeTlsStream::Plain(tcp),
    )
    .await
    .unwrap();
    let (_, frame) = crate::rpc::handshake::test_support::frame_for(
        &key,
        "/",
        chrono::Utc::now().timestamp_millis(),
    )
    .await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    let gate = Arc::new(SocketReadGate::default());
    let mut transport = TungsteniteTransport::new(socket);
    transport.gate = Some(gate.clone());
    let mut client = RpcClient::new(transport).await.unwrap();
    let social: SocialClient = client
        .create_port("slow-reader")
        .await
        .unwrap()
        .load_module(MODULE_NAME)
        .await
        .unwrap();
    let friend = addr("slow-large");
    let entity = seed_profile(&pool, &friend, "Large stalled profile").await;
    let picture = "https://profiles.example/".to_string() + &"u".repeat(8 * 1024 * 1024);
    let metadata = serde_json::json!({"v":{"avatars":[{"userId":friend,"name":"Large stalled profile","hasClaimedName":true,"avatar":{"snapshots":{"face256":picture}}}]}});
    sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=$1")
        .bind(&entity)
        .bind(metadata)
        .execute(&pool)
        .await
        .unwrap();
    server
        .state
        .ctx
        .db()
        .apply_friendship_action(&me, &friend, "accept", true, None, None)
        .await
        .unwrap();
    let _updates = subscribed(
        &server,
        &me,
        social.subscribe_to_friend_connectivity_updates(),
    )
    .await;
    gate.pause_after_response.store(true, Ordering::SeqCst);
    let _ = social.get_friendship_status(&friend).await;
    assert!(gate.paused.load(Ordering::SeqCst));
    server
        .state
        .ctx
        .topology()
        .receive(&pulse_frame(vec![me.clone(), friend.clone()]))
        .unwrap();
    server.state.ctx.pubsub().publish(
        &me,
        SocialEvent::FriendConnectivity(FriendConnectivityUpdate {
            stream_closed: None,
            status: ConnectivityStatus::Online as i32,
            friend: Some(crate::rpc::profiles::friend_profile(&friend, None)),
        }),
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while server.state.ctx.live_connections() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("stalled physical writer must retire the socket");
    gate.paused.store(false, Ordering::SeqCst);
    gate.resume.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !gate.closed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("peer must observe transport closure");
    drop_profile(&pool, &friend, &entity).await;
    cleanup_pair(server.state.ctx.db(), &me, &friend).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn rpc_connectivity_enriches_historical_aliases_but_guards_the_requested_actor() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let server = spawn_server(Some(pool.clone()), 5).await;
    let (me, _client, social) = explorer_as(&server, &PrivateKeySigner::random()).await;
    let actor = addr("alias-actor");
    let alias = addr("alias-public");
    let entity = seed_profile(&pool, &actor, "Historical alias").await;
    let metadata = serde_json::json!({"v":{"avatars":[{"userId":alias,"name":"Historical alias","hasClaimedName":true,"avatar":{"snapshots":{"face256":"https://profiles.example/alias.png"}}}]}});
    sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=$1")
        .bind(&entity)
        .bind(metadata)
        .execute(&pool)
        .await
        .unwrap();
    server
        .state
        .ctx
        .db()
        .apply_friendship_action(&me, &actor, "accept", true, None, None)
        .await
        .unwrap();
    server
        .state
        .ctx
        .topology()
        .receive(&pulse_frame(vec![me.clone(), actor.clone()]))
        .unwrap();
    assert!(!server.state.ctx.topology().snapshot().is_online(&alias));
    let mut updates = subscribed(
        &server,
        &me,
        social.subscribe_to_friend_connectivity_updates(),
    )
    .await;
    assert_eq!(
        next_update(&mut updates).await.friend.unwrap().address,
        alias
    );
    server.state.ctx.pubsub().publish(
        &me,
        SocialEvent::FriendConnectivity(FriendConnectivityUpdate {
            stream_closed: None,
            status: ConnectivityStatus::Online as i32,
            friend: Some(crate::rpc::profiles::friend_profile(&actor, None)),
        }),
    );
    assert_eq!(
        next_update(&mut updates).await.friend.unwrap().address,
        alias
    );
    drop_profile(&pool, &actor, &entity).await;
    cleanup_pair(server.state.ctx.db(), &me, &actor).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}
