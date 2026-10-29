// from: catalyrst/crates/catalyrst-pulse/tests/nats_live.rs
//! The cluster feed against a real broker. Set `PULSE_NATS_SERVER_BIN` for an isolated child,
//! or `PULSE_NATS_TEST_URL` for a scratch broker (never the deployment's own).
//!
//! What only a live broker can show is the wire: that a subscriber on `peer.{wallet}.cluster_change`
//! receives bytes that decode as the vendored `PeerClusterChange`, and that the topology and
//! discovery subjects carry archipelago's shapes. The tracker's publish decisions are asserted
//! against the in-process fake instead, which needs no broker at all.
#[path = "support/gate.rs"]
mod gate;
#[path = "support/nats_broker.rs"]
mod nats_broker;
use std::sync::Arc;
use std::time::Duration;

use catalyst::comms::protocol::bus::topology::{DISCOVERY_SUBJECT, ISLANDS_SUBJECT};
use catalyst::comms::pulse::PeerClusterChange;
use catalyst::comms::pulse::cluster::feed::{ClusterFeedPublisher, CurrentAssignment};
use catalyst::comms::pulse::cluster::nats::{NatsClusterFeed, NatsFeedOptions};
use catalyst::comms::pulse::cluster::{ClusterInfo, ClusterPass, ClusterPeerInfo};
use catalyst::comms::pulse::decentraland::common::Vector3;
use catalyst::comms::pulse::decentraland::kernel::comms::v3::{
    IslandStatusMessage, ServiceDiscoveryMessage,
};
use futures::StreamExt;
use prost::Message as _;

const WALLET: &str = "0xAbCdEf0000000000000000000000000000000001";
const SUBJECT: &str = "peer.0xabcdef0000000000000000000000000000000001.cluster_change";
const SESSION: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Long enough to absorb a loopback broker's scheduling, short enough that a genuinely missing
/// message fails the test rather than hanging the suite.
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(5);

fn pass() -> ClusterPass {
    let mut pass = ClusterPass::default();
    pass.clusters.push(ClusterInfo {
        id: "C1".into(),
        realm: "test-realm".into(),
        count: 1,
        centroid: Vector3 {
            x: 4.0,
            y: 0.0,
            z: 8.0,
        },
        radius: 2.5,
    });
    pass.peers.push(ClusterPeerInfo {
        peer: 0,
        wallet: WALLET.to_string(),
        cluster_id: "C1".into(),
        realm: "test-realm".into(),
        position: Vector3 {
            x: 4.0,
            y: 0.0,
            z: 8.0,
        },
        parcel: 0,
    });
    pass
}

async fn next_payload(sub: &mut async_nats::Subscriber) -> Vec<u8> {
    tokio::time::timeout(DELIVERY_TIMEOUT, sub.next())
        .await
        .expect("a message must arrive within the delivery timeout")
        .expect("the subscription must stay open")
        .payload
        .to_vec()
}

#[tokio::test]
async fn the_cluster_feed_reaches_a_live_broker_on_every_subject() {
    let broker = std::env::var("PULSE_NATS_SERVER_BIN")
        .ok()
        .map(|binary| nats_broker::Broker::start(&binary, None));
    let url = match &broker {
        Some(broker) => broker.url(),
        None => {
            let Some(url) = gate::require_env("PULSE_NATS_TEST_URL") else {
                return;
            };
            url
        }
    };

    let client = match async_nats::connect(&url).await {
        Ok(c) => c,
        Err(e) => panic!(
            "{}",
            format!("PULSE_NATS_TEST_URL is set but unusable: connect to {url} failed: {e}")
        ),
    };
    let mut changes = client.subscribe(SUBJECT).await.expect("subscribe changes");
    let mut islands = client
        .subscribe(ISLANDS_SUBJECT)
        .await
        .expect("subscribe islands");
    let mut discovery = client
        .subscribe(DISCOVERY_SUBJECT)
        .await
        .expect("subscribe discovery");
    let inbox = client.new_inbox();
    let mut ready = client.subscribe(inbox.clone()).await.unwrap();
    client.publish(inbox, "ready".into()).await.unwrap();
    assert_eq!(next_payload(&mut ready).await, b"ready");

    let feed = NatsClusterFeed::spawn(NatsFeedOptions {
        url: url.clone(),
        server_name: "pulse-test".into(),
        discovery_interval_ms: 200,
        ..Default::default()
    });

    let pass = pass();
    feed.publish_topology(&pass, 3);
    let assignment = |session: &str, cluster: &str| CurrentAssignment {
        wallet: WALLET.into(),
        session: session.into(),
        cluster_id: cluster.into(),
        realm: "test-realm".into(),
        owner_epoch: 1,
        owner_peer: 0,
        registration: None,
    };
    feed.replace_assignments(
        1,
        Duration::from_secs(2),
        vec![assignment("0xsession-one", "C0")],
    );
    feed.replace_assignments(
        2,
        Duration::from_secs(2),
        vec![assignment("0xsession-two", "C1")],
    );

    let change = PeerClusterChange::decode(next_payload(&mut changes).await.as_slice())
        .expect("the change decodes as PeerClusterChange");
    assert_eq!(change.cluster_id, "C1");
    assert_eq!(change.realm, "test-realm");
    assert_eq!(change.session, "0xsession-two");
    assert_eq!(change.displaced_session, "0xsession-one");
    assert_eq!(change.displaced_cluster_id, "C0");

    let topology = IslandStatusMessage::decode(next_payload(&mut islands).await.as_slice())
        .expect("the topology decodes as IslandStatusMessage");
    assert_eq!(topology.data.len(), 1);
    assert_eq!(topology.data[0].id, "C1");
    assert_eq!(topology.data[0].peers, vec![WALLET.to_string()]);
    assert_eq!(
        topology.data[0].max_peers, 0,
        "Pulse caps cluster size nowhere"
    );

    let heartbeat = ServiceDiscoveryMessage::decode(next_payload(&mut discovery).await.as_slice())
        .expect("the heartbeat decodes as ServiceDiscoveryMessage");
    assert_eq!(heartbeat.server_name, "pulse-test");
    let status = heartbeat.status.expect("the heartbeat carries a status");
    assert_eq!(
        status.user_count, 3,
        "the heartbeat advertises active peers, not clustered peers"
    );
    assert_eq!(
        status.commit_hash.as_deref(),
        Some(catalyst::comms::pulse::cluster::nats::DEFAULT_COMMIT_HASH),
        "an options set that never named a commit still announces upstream's fallback, never a \
         blank string a discovery consumer would have to special-case"
    );
    assert!(status.current_time > 0);
}

#[tokio::test]
async fn a_broker_that_is_down_costs_the_publisher_nothing() {
    let feed = NatsClusterFeed::spawn(NatsFeedOptions {
        url: "nats://127.0.0.1:1".into(),
        ..Default::default()
    });
    let publisher: Arc<dyn ClusterFeedPublisher> = Arc::new(feed);
    let pass = pass();

    let started = std::time::Instant::now();
    for n in 0..1000 {
        publisher.replace_assignments(
            n + 1,
            Duration::from_secs(2),
            vec![CurrentAssignment {
                wallet: WALLET.into(),
                session: SESSION.into(),
                cluster_id: format!("C{n}"),
                realm: "test-realm".into(),
                owner_epoch: 1,
                owner_peer: 0,
                registration: None,
            }],
        );
        publisher.publish_topology(&pass, 1);
    }
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "publishing must never wait on a broker, took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn private_control_positions_reach_the_audience_scoped_snapshot() {
    let Some(binary) = gate::require_env("PULSE_NATS_SERVER_BIN") else {
        return;
    };
    let broker = nats_broker::Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let feed = NatsClusterFeed::spawn(NatsFeedOptions {
        url: broker.url(),
        discovery_interval_ms: 0,
        ..Default::default()
    });
    feed.enable_control_positions("test-audience").unwrap();
    let mut update = catalyst::comms::protocol::bus::control_position::ControlPosition {
        audience: "test-audience".into(),
        wallet: WALLET.to_ascii_lowercase(),
        session: SESSION.into(),
        epoch: 4,
        sequence: 8,
        realm: "test-realm".into(),
        position: Some([4.0, 0.0, 8.0]),
    };

    tokio::time::timeout(DELIVERY_TIMEOUT, async {
        loop {
            client
                .publish(
                    catalyst::comms::protocol::bus::control_position::SUBJECT,
                    update.encode().unwrap().into(),
                )
                .await
                .unwrap();
            if feed.control_snapshot().members.len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("control position subscription becomes live");

    let assignment = |epoch| CurrentAssignment {
        wallet: WALLET.to_ascii_lowercase(),
        session: SESSION.into(),
        cluster_id: "control-C1".into(),
        realm: "test-realm".into(),
        owner_epoch: epoch,
        owner_peer: 100_000,
        registration: None,
    };
    feed.replace_assignments(1, Duration::from_secs(2), vec![assignment(4)]);
    let reply = client
        .request(
            format!("peer.{}.cluster_lookup", update.wallet),
            SESSION.as_bytes().to_vec().into(),
        )
        .await
        .unwrap();
    let before =
        catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot::decode(
            reply.payload,
        )
        .unwrap();
    update.epoch = 5;
    update.sequence += 1;
    client
        .publish(
            catalyst::comms::protocol::bus::control_position::SUBJECT,
            update.encode().unwrap().into(),
        )
        .await
        .unwrap();
    tokio::time::timeout(DELIVERY_TIMEOUT, async {
        while feed.control_snapshot().members[0].epoch != 5 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(150),
            client.request(
                format!("peer.{}.cluster_lookup", update.wallet),
                SESSION.as_bytes().to_vec().into()
            )
        )
        .await
        .is_err(),
        "new control epoch invalidates old authority before next pass"
    );
    feed.replace_assignments(
        2,
        Duration::from_secs(2),
        vec![CurrentAssignment {
            wallet: update.wallet.clone(),
            session: SESSION.into(),
            cluster_id: "control-C1".into(),
            realm: "test-realm".into(),
            owner_epoch: 5,
            owner_peer: 100_000,
            registration: None,
        }],
    );
    let reply = client
        .request(
            format!("peer.{}.cluster_lookup", update.wallet),
            SESSION.as_bytes().to_vec().into(),
        )
        .await
        .unwrap();
    let after = catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot::decode(
        reply.payload,
    )
    .unwrap();
    assert!(after.assignment_version > before.assignment_version);

    update.sequence += 1;
    update.realm.clear();
    update.position = None;
    client
        .publish(
            catalyst::comms::protocol::bus::control_position::SUBJECT,
            update.encode().unwrap().into(),
        )
        .await
        .unwrap();
    tokio::time::timeout(DELIVERY_TIMEOUT, async {
        loop {
            let snapshot = feed.control_snapshot();
            if snapshot.members.is_empty() && snapshot.owners.contains_key(&update.wallet) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("control tombstone reaches the bounded owner registry");
}

#[tokio::test]
async fn private_control_position_subscription_recovers_when_broker_starts_later() {
    let Some(binary) = gate::require_env("PULSE_NATS_SERVER_BIN") else {
        return;
    };
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("nats://127.0.0.1:{port}");
    let feed = NatsClusterFeed::spawn(NatsFeedOptions {
        url: url.clone(),
        discovery_interval_ms: 0,
        ..Default::default()
    });
    feed.enable_control_positions("test-audience").unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let _broker = nats_broker::Broker::start(&binary, Some(port));
    let client = async_nats::connect(url).await.unwrap();
    let update = catalyst::comms::protocol::bus::control_position::ControlPosition {
        audience: "test-audience".into(),
        wallet: WALLET.to_ascii_lowercase(),
        session: SESSION.into(),
        epoch: 4,
        sequence: 8,
        realm: "test-realm".into(),
        position: Some([4.0, 0.0, 8.0]),
    };

    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            client
                .publish(
                    catalyst::comms::protocol::bus::control_position::SUBJECT,
                    update.encode().unwrap().into(),
                )
                .await
                .unwrap();
            if feed.control_snapshot().members.len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("control position subscription connects after a broker appears");
}
