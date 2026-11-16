// from: catalyrst/crates/catalyrst-pulse/tests/cluster_lookup.rs
#[path = "support/gate.rs"]
mod gate;
#[path = "support/nats_broker.rs"]
mod nats_broker;
use catalyst::comms::pulse::{
    cluster::nats::{NatsClusterFeed, NatsFeedOptions},
    cluster::{ClusterOptions, ClusterTracker},
    decentraland::{common::Vector3, pulse::PeerClusterSnapshot},
    interest::SPATIAL_GRID_CELL_SIZE,
    realm_grids::RealmSpatialGrids,
    snapshot::{IdentityBoard, PeerSnapshot, SnapshotBoard},
};
use futures::StreamExt;
use nats_broker::Broker;
use prost::Message;
use std::{sync::Arc, time::Duration};

const CAP: usize = 8;

async fn publish_control(
    client: &async_nats::Client,
    input: &catalyst::comms::protocol::bus::control_position::ControlPosition,
    origin: i32,
) {
    use catalyst::comms::protocol::bus::control_position::{LegacyRoomIntent, SUBJECT};
    let (subject, bytes) = if origin == 2 {
        (SUBJECT, input.encode().unwrap())
    } else {
        (
            LegacyRoomIntent::SUBJECT,
            LegacyRoomIntent {
                audience: input.audience.clone(),
                wallet: input.wallet.clone(),
                session: input.session.clone(),
                candidate_epoch: input.epoch,
                sequence: input.sequence,
                realm: input.realm.clone(),
                position: input.position,
            }
            .encode()
            .unwrap(),
        )
    };
    client.publish(subject, bytes.into()).await.unwrap();
}

#[tokio::test]
async fn real_versioned_control_ordering_expiry_and_reconnect_keep_exact_authority() {
    use catalyst::comms::protocol::bus::control_position::ControlPosition;
    use catalyst::comms::pulse::cluster::feed::ClusterFeedPublisher;
    let binary = gate::require_env("PULSE_NATS_SERVER_BIN").expect("required NATS executable");
    let broker = Broker::start(&binary, None);
    let port = broker.port;
    let client = async_nats::connect(broker.url()).await.unwrap();
    let mut world = World::new(broker.url());
    world
        .feed
        .enable_control_positions("ordered-readers")
        .unwrap();
    let mut inputs: Vec<_> = [(2, u64::MAX), (3, i64::MAX as u64)]
        .into_iter()
        .map(|(origin, epoch)| {
            (
                origin,
                ControlPosition {
                    audience: "ordered-readers".into(),
                    wallet: address(origin as usize),
                    session: address(origin as usize + 100),
                    epoch,
                    sequence: 1,
                    realm: "first".into(),
                    position: Some([1.0, 0.0, 1.0]),
                },
            )
        })
        .collect();
    for (origin, input) in &mut inputs {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                publish_control(&client, input, *origin).await;
                if world
                    .feed
                    .control_snapshot()
                    .members
                    .iter()
                    .any(|member| member.wallet == input.wallet)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("real version subscription receives initial sample");
        input.sequence = 2;
        input.realm = "ordered".into();
        publish_control(&client, input, *origin).await;
        let mut old = input.clone();
        old.sequence = 1;
        old.realm = "stale".into();
        publish_control(&client, &old, *origin).await;
        let mut wrong_session = input.clone();
        wrong_session.sequence = 3;
        wrong_session.session = address(999);
        publish_control(&client, &wrong_session, *origin).await;
        let mut marker = input.clone();
        marker.wallet = address(700 + *origin as usize);
        marker.epoch = 1;
        marker.sequence = 1;
        publish_control(&client, &marker, *origin).await;
        assert!(
            catalyst::shared::test_wait::until(async || world
                .feed
                .control_snapshot()
                .members
                .iter()
                .any(|member| member.wallet == marker.wallet))
            .await
        );
        let control = world.feed.control_snapshot();
        let observed = control
            .members
            .iter()
            .find(|member| member.wallet == input.wallet)
            .unwrap();
        assert_eq!(
            observed.realm, input.realm,
            "older samples cannot roll back a live head"
        );
        assert_eq!(
            observed.session, input.session,
            "a session stays fixed throughout its epoch"
        );
        let selected = current(&client, &mut world, &input.wallet, &input.session).await;
        assert_eq!(selected.realm, "ordered");
        assert_eq!(selected.selected_origin, *origin);
        assert_eq!(selected.selected_identity, Some(input.epoch));
    }
    // Duplicates arrive just before expiry, but may not renew the original live head.
    tokio::time::sleep(Duration::from_secs(9)).await;
    for (origin, input) in &inputs {
        publish_control(&client, input, *origin).await;
    }
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(
        world.feed.control_snapshot().members.is_empty(),
        "duplicates cannot extend either reader TTL"
    );
    world.pass();
    for (origin, input) in &mut inputs {
        assert!(
            lookup(&client, &input.wallet, &input.session)
                .await
                .is_none()
        );
        input.sequence = 3;
        publish_control(&client, input, *origin).await;
        assert_eq!(
            current(&client, &mut world, &input.wallet, &input.session)
                .await
                .selected_identity,
            Some(input.epoch),
            "live expiry permits a greater sequence in the same epoch"
        );
    }
    drop(broker);
    let broker = Broker::start(&binary, Some(port));
    let reader = async_nats::connect(broker.url()).await.unwrap();
    for (origin, input) in &mut inputs {
        input.sequence = 4;
        input.realm = "reconnected".into();
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                publish_control(&reader, input, *origin).await;
                if world
                    .feed
                    .control_snapshot()
                    .members
                    .iter()
                    .any(|member| member.wallet == input.wallet && member.realm == input.realm)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("both subscriptions recover after actual broker restart");
        let selected = current(&reader, &mut world, &input.wallet, &input.session).await;
        assert_eq!(selected.realm, "reconnected");
        assert_eq!(selected.selected_origin, *origin);
        let mut withdrawal = input.clone();
        withdrawal.sequence = 5;
        withdrawal.realm.clear();
        withdrawal.position = None;
        publish_control(&reader, &withdrawal, *origin).await;
        input.sequence = 6;
        publish_control(&reader, input, *origin).await;
        let mut marker = input.clone();
        marker.wallet = address(900 + *origin as usize);
        marker.epoch = 1;
        marker.sequence = 1;
        publish_control(&reader, &marker, *origin).await;
        assert!(
            catalyst::shared::test_wait::until(async || world
                .feed
                .control_snapshot()
                .members
                .iter()
                .any(|member| member.wallet == marker.wallet))
            .await
        );
        let control = world.feed.control_snapshot();
        assert!(
            control.owners[&input.wallet].closed,
            "terminal epoch rejects a later live sample"
        );
        assert!(
            !control
                .members
                .iter()
                .any(|member| member.wallet == input.wallet)
        );
        world.pass();
        assert!(
            lookup(&reader, &input.wallet, &input.session)
                .await
                .is_none()
        );
    }
}

#[tokio::test]
async fn real_versioned_control_heads_share_one_wallet_capacity() {
    use catalyst::comms::protocol::bus::control_position::ControlPosition;
    use catalyst::comms::pulse::cluster::feed::ClusterFeedPublisher;
    let binary = gate::require_env("PULSE_NATS_SERVER_BIN").expect("required NATS executable");
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let world = World::new(broker.url());
    world
        .feed
        .enable_control_positions("capacity-readers")
        .unwrap();
    let capacity = catalyst::comms::pulse::cluster::control::DEFAULT_CONTROL_POSITION_CAPACITY;
    let input = |wallet| ControlPosition {
        audience: "capacity-readers".into(),
        wallet: address(wallet),
        session: address(wallet + capacity + 1),
        epoch: 1,
        sequence: 1,
        realm: "r".into(),
        position: Some([1.0, 0.0, 1.0]),
    };
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            publish_control(&client, &input(1), 2).await;
            if !world.feed.control_snapshot().members.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("v1 subscription is installed before capacity fill");
    for wallet in 2..=capacity {
        publish_control(&client, &input(wallet), 2).await;
    }
    client.flush().await.unwrap();
    assert!(
        catalyst::shared::test_wait::until(
            async || world.feed.control_snapshot().members.len() == capacity
        )
        .await
    );
    // A second head fits an existing wallet even when no new wallet can fit.
    publish_control(&client, &input(1), 3).await;
    publish_control(&client, &input(capacity + 1), 3).await;
    publish_control(&client, &input(2), 3).await;
    assert!(
        catalyst::shared::test_wait::until(
            async || world.feed.control_snapshot().members.len() == capacity - 2
        )
        .await
    );
    let selected = world.feed.control_snapshot();
    assert!(
        !selected
            .members
            .iter()
            .any(|member| member.wallet == address(capacity + 1)),
        "v2 cannot allocate a separate wallet budget"
    );
    assert!(
        !selected
            .members
            .iter()
            .any(|member| member.wallet == address(1) || member.wallet == address(2)),
        "both retained heads make unproved mixed authority unavailable"
    );
}

#[tokio::test]
async fn real_versioned_control_readers_preserve_single_class_service_and_refuse_unproved_mixture()
{
    use catalyst::comms::protocol::bus::control_position::{
        ControlPosition, LegacyRoomIntent, SUBJECT,
    };
    let binary = gate::require_env("PULSE_NATS_SERVER_BIN").expect("required NATS executable");
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let mut world = World::new(broker.url());
    world.feed.enable_control_positions("reader-test").unwrap();
    let v1 = ControlPosition {
        audience: "reader-test".into(),
        wallet: address(1),
        session: address(101),
        epoch: u64::MAX,
        sequence: 1,
        realm: "r".into(),
        position: Some([1.0, 0.0, 1.0]),
    };
    let v2 = LegacyRoomIntent {
        audience: "reader-test".into(),
        wallet: address(2),
        session: address(102),
        candidate_epoch: i64::MAX as u64,
        sequence: 1,
        realm: "r".into(),
        position: Some([1.0, 0.0, 1.0]),
    };
    for (subject, bytes, wallet, session, origin, identity) in [
        (
            SUBJECT,
            v1.encode().unwrap(),
            &v1.wallet,
            &v1.session,
            2,
            u64::MAX,
        ),
        (
            LegacyRoomIntent::SUBJECT,
            v2.encode().unwrap(),
            &v2.wallet,
            &v2.session,
            3,
            i64::MAX as u64,
        ),
    ] {
        let selected = tokio::time::timeout(Duration::from_secs(12), async {
            loop {
                client.publish(subject, bytes.clone().into()).await.unwrap();
                client.flush().await.unwrap();
                world.pass();
                if let Some(snapshot) = lookup(&client, wallet, session).await {
                    break snapshot;
                }
            }
        })
        .await
        .expect("actual version subscription and pass must become ready");
        assert_eq!(selected.selected_origin, origin);
        assert_eq!(selected.selected_identity, Some(identity));
        assert_eq!(selected.assignment_state, 1);
    }
    let mixed = LegacyRoomIntent {
        wallet: v1.wallet.clone(),
        session: v1.session.clone(),
        candidate_epoch: 9,
        ..v2.clone()
    };
    client
        .publish(LegacyRoomIntent::SUBJECT, mixed.encode().unwrap().into())
        .await
        .unwrap();
    client.flush().await.unwrap();
    assert!(
        catalyst::shared::test_wait::until(async || {
            world.pass();
            lookup(&client, &v1.wallet, &v1.session).await.is_none()
        })
        .await
    );
    world
        .identity
        .set_with_session(0, v2.wallet.clone(), address(202));
    assert!(
        lookup(&client, &v2.wallet, &v2.session).await.is_none(),
        "unreadable native reservation blocks legacy before pass"
    );
    world.identity.remove(0);
    assert!(
        lookup(&client, &v2.wallet, &v2.session).await.is_none(),
        "departure cannot revive old legacy version"
    );
    let restored = current(&client, &mut world, &v2.wallet, &v2.session).await;
    assert_eq!(restored.selected_origin, 3);
}

#[tokio::test]
async fn complete_lookup_cap_keeps_pending_recovery_and_refuses_one_byte_over() {
    use catalyst::comms::protocol::bus::cluster::{ClusterLookup, valid_reply};
    use catalyst::comms::pulse::cluster::feed::{ClusterFeedPublisher, CurrentAssignment};
    let binary = gate::require_env("PULSE_NATS_SERVER_BIN").expect("required NATS executable");
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    for extra in [0, 1] {
        let mut world = World::new(broker.url());
        let wallet = address(1);
        let old = address(3);
        let session = address(2);
        world
            .identity
            .set_with_session(0, wallet.clone(), old.clone());
        // The frozen canonical cap vector uses a 13-byte source and one-byte
        // takeover id. Production UUIDs add 23 and 35 bytes respectively.
        let previous = CurrentAssignment {
            wallet: wallet.clone(),
            session: old,
            cluster_id: "x".repeat(65_356 - 23 - 35 + extra),
            realm: "r".into(),
            owner_epoch: world.identity.registration_by_peer(0),
            selected_origin: 1,
            owner_peer: 0,
            registration: Some(world.identity.registration_guard(0)),
        };
        world
            .feed
            .replace_assignments(7, Duration::from_secs(30), vec![previous]);
        let previous = lookup(&client, &wallet, &address(3)).await;
        if previous.is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        world
            .identity
            .set_with_session(0, wallet.clone(), session.clone());
        world.feed.replace_assignments(
            8,
            Duration::from_secs(30),
            vec![CurrentAssignment {
                wallet: wallet.clone(),
                session: session.clone(),
                cluster_id: "c1".into(),
                realm: "r".into(),
                owner_epoch: world.identity.registration_by_peer(0),
                selected_origin: 1,
                owner_peer: 0,
                registration: Some(world.identity.registration_guard(0)),
            }],
        );
        world
            .feed
            .replace_assignments(9, Duration::from_secs(30), vec![]);
        if extra == 0 {
            let snapshot = lookup(&client, &wallet, &session)
                .await
                .expect("exact cap reply");
            assert_eq!(snapshot.encode_to_vec().len(), 65_536);
            assert_eq!(snapshot.pending_takeovers.len(), 1);
            assert_eq!(snapshot.assignment_state, 2);
            assert!(valid_reply(
                &ClusterLookup {
                    wallet: wallet.clone(),
                    session: session.clone()
                },
                &snapshot
            ));
            assert!(catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp::from_snapshot(&snapshot, 0).is_none());
        } else {
            assert!(
                lookup(&client, &wallet, &session).await.is_none(),
                "complete over-cap recovery is withheld"
            );
        }
        drop(world);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn address(number: usize) -> String {
    format!("0x{number:040x}")
}

struct World {
    grids: RealmSpatialGrids,
    board: SnapshotBoard,
    identity: IdentityBoard,
    tracker: ClusterTracker,
    feed: Arc<NatsClusterFeed>,
}

impl World {
    fn new(url: String) -> Self {
        let feed = Arc::new(NatsClusterFeed::spawn(NatsFeedOptions {
            url,
            capacity: 1,
            discovery_interval_ms: 0,
            ..Default::default()
        }));
        let mut world = Self {
            grids: RealmSpatialGrids::new(SPATIAL_GRID_CELL_SIZE, CAP),
            board: SnapshotBoard::new(CAP, 4),
            identity: IdentityBoard::new(CAP),
            tracker: ClusterTracker::new(ClusterOptions::default(), CAP, feed.clone()),
            feed,
        };
        world.tracker.bind_identity(&mut world.identity);
        world
    }

    fn place(&mut self, peer: u32, wallet: &str, session: &str) {
        let position = Vector3 {
            x: 1.0,
            y: 0.0,
            z: 1.0,
        };
        self.board.set_active(peer);
        self.board.publish(
            peer,
            PeerSnapshot {
                global_position: position,
                realm: Some("test-realm".into()),
                ..Default::default()
            },
        );
        self.identity
            .set_with_session(peer, wallet.into(), session.into());
        self.grids.set(peer, "test-realm", position);
    }

    fn remove(&mut self, peer: u32) {
        self.grids.remove(peer);
        self.identity.remove(peer);
        self.board.clear_active(peer);
    }

    fn pass(&mut self) {
        self.tracker
            .run_pass(&self.grids, &self.board, &self.identity);
    }
}

async fn lookup(
    client: &async_nats::Client,
    wallet: &str,
    session: &str,
) -> Option<PeerClusterSnapshot> {
    let response = tokio::time::timeout(
        Duration::from_millis(200),
        client.request(
            format!("peer.{wallet}.cluster_lookup"),
            session.as_bytes().to_vec().into(),
        ),
    )
    .await
    .ok()?
    .ok()?;
    Some(PeerClusterSnapshot::decode(response.payload).expect("valid snapshot wire reply"))
}

async fn current(
    client: &async_nats::Client,
    world: &mut World,
    wallet: &str,
    session: &str,
) -> PeerClusterSnapshot {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            world.pass();
            if let Some(snapshot) = lookup(client, wallet, session).await {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("current authoritative assignment must become queryable")
}

async fn round_trip(client: &async_nats::Client) {
    let inbox = client.new_inbox();
    let mut replies = client.subscribe(inbox.clone()).await.unwrap();
    client.publish(inbox, "ready".into()).await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), replies.next())
            .await
            .unwrap()
            .unwrap()
            .payload
            .as_ref(),
        b"ready"
    );
}

#[tokio::test]
async fn current_lookup_survives_lost_edges_and_broker_restart_without_reemitting_changes() {
    let Some(binary) = gate::require_env("PULSE_NATS_SERVER_BIN") else {
        return;
    };
    let broker = Broker::start(&binary, None);
    let port = broker.port;
    let client = async_nats::connect(broker.url()).await.unwrap();
    let mut changes = client.subscribe("peer.*.cluster_change").await.unwrap();
    round_trip(&client).await;
    let mut world = World::new(broker.url());
    let mut other_source = World::new(broker.url());
    other_source.pass();
    for peer in 0..3 {
        world.place(
            peer,
            &address(peer as usize + 1),
            &address(peer as usize + 101),
        );
    }
    world.pass();
    assert_eq!(
        world.feed.pending_changes(),
        1,
        "the two older edges were evicted before the publisher could run"
    );
    let first = current(&client, &mut world, &address(1), &address(101)).await;
    assert_eq!(first.wallet, address(1));
    assert_eq!(first.session, address(101));
    assert_eq!(first.realm, "test-realm");
    assert!(!first.source_incarnation.is_empty());
    assert_eq!(
        first.cluster_id,
        world.tracker.board().cluster_id(0).unwrap()
    );
    while matches!(
        tokio::time::timeout(Duration::from_millis(50), changes.next()).await,
        Ok(Some(_))
    ) {}

    for peer in 0..3 {
        let snapshot = current(
            &client,
            &mut world,
            &address(peer + 1),
            &address(peer + 101),
        )
        .await;
        assert_eq!(snapshot.source_incarnation, first.source_incarnation);
        assert!(snapshot.pass >= first.pass);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(150), changes.next())
            .await
            .is_err(),
        "read-only recovery must not remint via cluster_change"
    );

    let mut reflected = client.subscribe("test.reflection").await.unwrap();
    round_trip(&client).await;
    client
        .publish_with_reply(
            format!("peer.{}.cluster_lookup", address(1)),
            "test.reflection",
            address(101).into(),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), reflected.next())
            .await
            .is_err(),
        "a lookup must not reflect into an arbitrary feed subject"
    );

    drop(broker);
    world.pass();
    let broker = Broker::start(&binary, Some(port));
    let reader = async_nats::connect(broker.url()).await.unwrap();
    let recovered = current(&reader, &mut world, &address(1), &address(101)).await;
    assert_eq!(
        recovered.source_incarnation, first.source_incarnation,
        "broker reconnect is not a source restart"
    );
    assert_eq!(recovered.cluster_id, first.cluster_id);

    world.remove(0);
    world.place(0, &address(1), &address(201));
    world.pass();
    assert!(lookup(&reader, &address(1), &address(101)).await.is_none());
    assert_eq!(
        current(&reader, &mut world, &address(1), &address(201))
            .await
            .session,
        address(201)
    );
    world.remove(0);
    world.pass();
    let inactive = lookup(&reader, &address(1), &address(201))
        .await
        .expect("exact retained session can finish departed cleanup");
    assert_eq!(inactive.pending_takeovers.len(), 1);
    assert!(inactive.token_not_before.is_some());
    reader
        .publish(
            format!("peer.{}.takeover_completed", address(1)),
            catalyst::comms::protocol::pulse::decentraland::pulse::TakeoverCompleted {
                takeover_id: inactive.pending_takeovers[0].takeover_id.clone(),
                revoke_before: chrono::Utc::now().timestamp() as u64 + 1,
            }
            .encode_to_vec()
            .into(),
        )
        .await
        .unwrap();
    reader.flush().await.unwrap();
    assert!(
        catalyst::shared::test_wait::until(async || lookup(&reader, &address(1), &address(201))
            .await
            .is_none())
        .await,
        "completed inactive recovery never authorizes mint"
    );
    for _ in 0..3 {
        assert!(
            lookup(&reader, &address(1), &address(201)).await.is_none(),
            "a completed takeover stays unauthorized"
        );
    }

    drop(world);
    let mut replacement = World::new(broker.url());
    replacement.place(0, &address(1), &address(201));
    let restarted = current(&reader, &mut replacement, &address(1), &address(201)).await;
    assert_ne!(
        restarted.source_incarnation, first.source_incarnation,
        "a new feed owner cannot reuse the old ordering scope"
    );
}

#[tokio::test]
async fn a_stopped_tracker_cannot_serve_an_indefinitely_fresh_assignment() {
    let Some(binary) = gate::require_env("PULSE_NATS_SERVER_BIN") else {
        return;
    };
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let mut world = World::new(broker.url());
    world.place(0, &address(1), &address(101));
    let before = current(&client, &mut world, &address(1), &address(101)).await;
    assert!(
        catalyst::shared::test_wait::until(async || lookup(&client, &address(1), &address(101))
            .await
            .is_none())
        .await
    );
    let after = current(&client, &mut world, &address(1), &address(101)).await;
    assert!(after.pass > before.pass);
    assert_eq!(after.source_incarnation, before.source_incarnation);
}
