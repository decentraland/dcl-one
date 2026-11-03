//! Cross-replica signalling over two links to an owned core-NATS process.
use super::*;
use crate::comms::gatekeeper::bus::{BrokerBus, InProcessBus, NatsBus};
use crate::rpc::fanout::{Event, Scope};
use tokio::sync::broadcast;

struct Broker(std::process::Child);
impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Broker {
    fn start() -> (Self, String) {
        let binary = std::env::var("PULSE_NATS_SERVER_BIN")
            .expect("owned broker test requires PULSE_NATS_SERVER_BIN");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let child = std::process::Command::new(binary)
            .args(["-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        (Self(child), format!("nats://127.0.0.1:{port}"))
    }
}
fn replica(pool: &PgPool) -> Arc<State> {
    Arc::new(State::with_context(
        Settings::default(),
        test_ctx(
            Db::new(pool.clone()),
            Arc::new(FakeVoiceRooms::new(pool.clone())),
        ),
        None,
    ))
}
async fn receive(rx: &mut broadcast::Receiver<Arc<SocialEvent>>) -> Arc<SocialEvent> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn silent(rx: &mut broadcast::Receiver<Arc<SocialEvent>>) {
    assert!(
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err()
    );
}
fn started(community: Uuid, name: &str, image: Option<String>) -> CommunityVoiceChatUpdate {
    CommunityVoiceChatUpdate {
        community_id: community.to_string(),
        community_name: name.into(),
        community_image: image,
        status: CommunityVoiceChatStatus::CommunityVoiceChatStarted as i32,
        created_at: 123,
        ..Default::default()
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "owns a real NATS server; requires PULSE_NATS_SERVER_BIN and CATALYST_TEST_DATABASE_URL"]
async fn two_replicas_route_mutations_and_preserve_large_community_snapshots() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let (_broker, url) = Broker::start();
    let left = replica(&pool);
    let right = replica(&pool);
    let a = addr("bus-a");
    let b = addr("bus-b");
    let outsider = addr("bus-out");
    let community = seed_community(&pool, true, &a, "owner").await;
    sqlx::query("INSERT INTO social.community_members(community_id,member_address,role) VALUES($1,$2,'member')")
        .bind(community).bind(&b).execute(&pool).await.unwrap();
    let mut local = left.ctx.pubsub().subscribe(&b);
    let mut remote = right.ctx.pubsub().subscribe(&b);
    let mut public = right.ctx.pubsub().subscribe(&outsider);
    let mut excluded = right.ctx.pubsub().subscribe(&a);
    let jobs = Jobs::new();
    let one = Arc::new(BrokerBus::new(Some(url.clone()), "social-test-one"));
    let two = Arc::new(BrokerBus::new(Some(url), "social-test-two"));
    left.start_fanout(&jobs, one.clone());
    right.start_fanout(&jobs, two.clone());
    one.connect();
    two.connect();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !one.is_ready() || !two.is_ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    left.ctx.pubsub().publish(
        &b,
        SocialEvent::Block(BlockUpdate {
            address: a.clone(),
            is_blocked: true,
            stream_closed: None,
        }),
    );
    assert!(matches!(&*receive(&mut local).await,SocialEvent::Block(u) if u.address==a));
    assert!(matches!(&*receive(&mut remote).await,SocialEvent::Block(u) if u.address==a));
    silent(&mut local).await;
    silent(&mut public).await;
    // A long legitimate profile is fetched at RPC delivery, outside the bus
    // envelope, while the raw address event also remains available for JSON.
    let profile = seed_profile(&pool, &a, "Current remote profile").await;
    let picture = "https://profiles.example/".to_string() + &"u".repeat(140_000);
    let metadata = serde_json::json!({"v":{"avatars":[{"userId":a,"name":"Current remote profile","hasClaimedName":true,"avatar":{"snapshots":{"face256":picture}}}]}});
    sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=$1")
        .bind(&profile)
        .bind(metadata)
        .execute(&pool)
        .await
        .unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    let mut rpc = SocialServiceImpl
        .subscribe_to_friendship_updates(procedure_ctx(&right.ctx, &b, 8001), cancel.clone())
        .await
        .unwrap();
    let request = FriendshipUpdate {
        stream_closed: None,
        update: Some(friendship_update::Update::Request(
            friendship_update::RequestResponse {
                id: Uuid::new_v4().to_string(),
                created_at: 123,
                message: Some("🦊".repeat(256)),
                friend: Some(FriendProfile {
                    address: a.clone(),
                    profile_picture_url: picture.clone(),
                    ..Default::default()
                }),
            },
        )),
    };
    left.ctx
        .pubsub()
        .publish(&b, SocialEvent::Friendship(request));
    let _ = receive(&mut local).await;
    assert!(matches!(
        &*receive(&mut remote).await,
        SocialEvent::Friendship(_)
    ));
    let delivered = next_update(&mut rpc).await;
    let Some(friendship_update::Update::Request(request)) = delivered.update else {
        panic!("request expected")
    };
    assert_eq!(request.friend.unwrap().profile_picture_url, picture);
    assert_eq!(request.message.unwrap().chars().count(), 256);
    cancel.cancel();
    drop_profile(&pool, &a, &profile).await;
    // Topology is already derived independently in every serve: it must not be rebroadcast.
    left.ctx.pubsub().publish(
        &b,
        SocialEvent::FriendConnectivity(FriendConnectivityUpdate::default()),
    );
    assert!(matches!(
        &*receive(&mut local).await,
        SocialEvent::FriendConnectivity(_)
    ));
    silent(&mut remote).await;
    let first = started(community, "first", Some("u".repeat(140_000)));
    left.ctx
        .pubsub()
        .community_voice(&pool, community, first.clone(), true, Some(&a))
        .await;
    assert!(
        matches!(&*receive(&mut local).await,SocialEvent::CommunityVoice(u) if u.community_image==first.community_image && u.is_member)
    );
    assert!(
        matches!(&*receive(&mut remote).await,SocialEvent::CommunityVoice(u) if u.community_image==first.community_image && u.is_member)
    );
    assert!(
        matches!(&*receive(&mut public).await,SocialEvent::CommunityVoice(u) if u.community_image==first.community_image && !u.is_member)
    );
    silent(&mut excluded).await;
    // The immutable cache holds both queued START snapshots across END/new START.
    let second = started(community, "second", Some("v".repeat(140_000)));
    let event_one = Event::community_voice(Scope::All, None, &first).unwrap();
    let event_two = Event::community_voice(Scope::All, None, &second).unwrap();
    let id_one = left
        .ctx
        .db()
        .store_social_event_payload(event_one.encode_stored().unwrap())
        .await
        .unwrap();
    left.ctx
        .db()
        .record_voice_notification(community, Scope::All)
        .await
        .unwrap();
    let entry = left
        .ctx
        .db()
        .voice_notification(community)
        .await
        .unwrap()
        .unwrap();
    left.ctx
        .db()
        .clear_voice_notification(&entry)
        .await
        .unwrap();
    left.ctx
        .db()
        .record_voice_notification(community, Scope::Members)
        .await
        .unwrap();
    let id_two = left
        .ctx
        .db()
        .store_social_event_payload(event_two.encode_stored().unwrap())
        .await
        .unwrap();
    Event::Reference { event_id: id_one }
        .deliver(&right.ctx)
        .await;
    Event::Reference { event_id: id_two }
        .deliver(&right.ctx)
        .await;
    for expected in [&first, &second] {
        assert!(
            matches!(&*receive(&mut remote).await,SocialEvent::CommunityVoice(u) if u.community_name==expected.community_name && u.community_image==expected.community_image)
        );
        let _ = receive(&mut public).await;
        let _ = receive(&mut excluded).await;
    }
    // A public hint never overrides a privacy change at delivery.
    sqlx::query("UPDATE social.communities SET private=true WHERE id=$1")
        .bind(community)
        .execute(&pool)
        .await
        .unwrap();
    Event::Reference { event_id: id_one }
        .deliver(&right.ctx)
        .await;
    assert!(matches!(&*receive(&mut remote).await,SocialEvent::CommunityVoice(u) if u.is_member));
    let _ = receive(&mut excluded).await;
    silent(&mut public).await;
    jobs.shutdown(Duration::from_secs(2)).await;
    drop_community(&pool, community).await;
}

#[tokio::test]
async fn notification_generation_survives_context_restart_and_expires_like_source_cache() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let a = addr("scope");
    let public = addr("scope-out");
    let community = seed_community(&pool, true, &a, "owner").await;
    let db = Db::new(pool.clone());
    let old = db
        .record_voice_notification(community, Scope::All)
        .await
        .unwrap();
    let old_entry = db.voice_notification(community).await.unwrap().unwrap();
    let current = db
        .record_voice_notification(community, Scope::Members)
        .await
        .unwrap();
    assert_ne!(old, current);
    assert!(!db.clear_voice_notification(&old_entry).await.unwrap());
    assert!(!db.refresh_voice_notification(&old_entry).await.unwrap());
    let current_entry = db.voice_notification(community).await.unwrap().unwrap();
    assert!(db.clear_voice_notification(&current_entry).await.unwrap());
    db.record_voice_notification(community, Scope::All)
        .await
        .unwrap();
    // A new process sees the persisted public scope and emits ENDED once.
    let restarted = replica(&pool);
    let mut rx = restarted.ctx.pubsub().subscribe(&public);
    restarted.ctx.poll_community_voice_notifications().await;
    assert!(
        matches!(&*receive(&mut rx).await,SocialEvent::CommunityVoice(u) if u.status==CommunityVoiceChatStatus::CommunityVoiceChatEnded as i32)
    );
    restarted.ctx.poll_community_voice_notifications().await;
    silent(&mut rx).await;
    db.record_voice_notification(community, Scope::All)
        .await
        .unwrap();
    sqlx::query("UPDATE social.communities SET voice_notification_expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(community).execute(&pool).await.unwrap();
    restarted.ctx.poll_community_voice_notifications().await;
    silent(&mut rx).await;
    assert!(db.voice_notification(community).await.unwrap().is_none());
    drop_community(&pool, community).await;
}

#[tokio::test]
async fn outbound_queue_and_disconnected_bus_are_bounded_best_effort() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let state = replica(&pool);
    let me = addr("bound");
    let actor = addr("actor");
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    state.ctx.pubsub().attach_outbound(tx);
    for _ in 0..1000 {
        state.ctx.pubsub().publish(
            &me,
            SocialEvent::Block(BlockUpdate {
                address: actor.clone(),
                is_blocked: true,
                stream_closed: None,
            }),
        );
    }
    assert!(rx.try_recv().is_ok());
    assert!(rx.try_recv().is_err());
    let left = replica(&pool);
    let right = replica(&pool);
    let jobs = Jobs::new();
    let bus = Arc::new(InProcessBus::new());
    left.start_fanout(&jobs, bus.clone());
    right.start_fanout(&jobs, bus.clone());
    let mut remote = right.ctx.pubsub().subscribe(&me);
    bus.set_connected(false);
    left.ctx.pubsub().publish(
        &me,
        SocialEvent::Block(BlockUpdate {
            address: actor.clone(),
            is_blocked: true,
            stream_closed: None,
        }),
    );
    silent(&mut remote).await;
    bus.set_connected(true);
    left.ctx.pubsub().publish(
        &me,
        SocialEvent::Block(BlockUpdate {
            address: actor,
            is_blocked: false,
            stream_closed: None,
        }),
    );
    assert!(matches!(&*receive(&mut remote).await,SocialEvent::Block(u) if !u.is_blocked));
    silent(&mut remote).await;
    jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
#[ignore = "isolated EXPLAIN workload; run explicitly with PostgreSQL"]
async fn notification_and_spill_queries_use_bounded_index_paths() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    let owner = addr("plans");
    sqlx::query("INSERT INTO social.communities(id,name,owner_address) SELECT gen_random_uuid(),'plan fixture',$1 FROM generate_series(1,5000)")
        .bind(&owner).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE social.communities SET voice_notification_scope='all',voice_notification_generation=gen_random_uuid(),voice_notification_expires_at=now()+interval '24 hours' WHERE id IN(SELECT id FROM social.communities WHERE owner_address=$1 ORDER BY id LIMIT 10)")
        .bind(&owner).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO social.event_payloads(id,payload,expires_at) SELECT gen_random_uuid(),decode('01','hex'),now()+CASE WHEN n<=10 THEN interval '-1 hour' ELSE interval '1 hour' END FROM generate_series(1,5000) AS n")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("ANALYZE social.communities")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ANALYZE social.event_payloads")
        .execute(&mut *tx)
        .await
        .unwrap();
    let rooms:Vec<String>=sqlx::query_scalar("EXPLAIN(ANALYZE,BUFFERS) SELECT id,voice_notification_generation FROM social.communities WHERE id>'00000000-0000-0000-0000-000000000000'::uuid AND voice_notification_generation IS NOT NULL ORDER BY id LIMIT 64")
        .fetch_all(&mut *tx).await.unwrap();
    let cache:Vec<String>=sqlx::query_scalar("EXPLAIN(ANALYZE,BUFFERS) SELECT id FROM social.event_payloads WHERE expires_at<=timezone('UTC',now()) ORDER BY expires_at,id LIMIT 64 FOR UPDATE SKIP LOCKED")
        .fetch_all(&mut *tx).await.unwrap();
    println!(
        "notification pages (5000 communities,10 tracked):\n{}",
        rooms.join("\n")
    );
    println!(
        "spill cleanup (5000 snapshots,10 expired):\n{}",
        cache.join("\n")
    );
    assert!(
        rooms
            .join("\n")
            .contains("communities_voice_notifications_idx")
    );
    assert!(cache.join("\n").contains("event_payloads_expiry_idx"));
    tx.rollback().await.unwrap();
}
