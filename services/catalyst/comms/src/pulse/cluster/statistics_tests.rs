//! Real broker coverage for the shared connection, frame limits and recovery.
use super::*;
use crate::protocol::bus::statistics::{
    Assembler, Event, OBSERVATIONS_SUBJECT, Observation, PRODUCER_SUBJECT, ProducerGrant,
    ProducerReply, ProducerRequest, SNAPSHOTS_SUBJECT, Snapshot,
};
use std::io::BufRead;

#[tokio::test(start_paused = true)]
async fn statistics_publisher_recovers_during_single_item_refill() {
    continuous_publication(false).await;
}

#[tokio::test(start_paused = true)]
async fn statistics_publisher_preserves_checkpoint_prefix_during_continuous_backlog() {
    continuous_publication(true).await;
}

async fn continuous_publication(backlog: bool) {
    use crate::gatekeeper::bus::InProcessBus;
    use crate::protocol::bus::statistics::{Change, Update};
    use crate::stats::Reader;
    let shared = super::tests::feed_shared(1);
    let grant = {
        let mut stats = shared.statistics.lock().unwrap();
        let ProducerReply::Grant(grant) = stats.control(ProducerRequest::Begin {
            incarnation: shared.statistics_epoch,
            producer: uuid::Uuid::nil(),
        }) else {
            panic!("grant")
        };
        assert_eq!(
            stats.control(ProducerRequest::Confirm(grant.clone())),
            ProducerReply::Active(grant.clone())
        );
        grant
    };
    let healthy_bus = InProcessBus::new();
    let gap_bus = InProcessBus::new();
    let cold_bus = InProcessBus::new();
    let healthy = Arc::new(Reader::default());
    let gap = Arc::new(Reader::default());
    let cold = Arc::new(Reader::default());
    let _healthy = healthy.subscribe(&healthy_bus);
    let _gap = gap.subscribe(&gap_bus);
    let _cold = cold.subscribe(&cold_bus);
    let mut assembler = Assembler::default();
    let mut updates = Vec::new();
    let mut sequence = 0;
    let done = Notify::new();
    let mut recovered = 0;
    let mut last_change = 0;
    {
        let publish = publish_statistics_with(
            &shared,
            || Some(100),
            |parts| {
                let mut completed = None;
                for part in parts {
                    if let Some((_, update)) = assembler.receive_update(&part, Instant::now()) {
                        completed = Some(update);
                    }
                    healthy_bus.inject(SNAPSHOTS_SUBJECT, &part);
                    // Deliberately lose revision 1 for one established reader, and the
                    // initial snapshot for a cold reader. No reader implementation is mocked.
                    if updates.len() != 1 {
                        gap_bus.inject(SNAPSHOTS_SUBJECT, &part);
                    }
                    if !updates.is_empty() {
                        cold_bus.inject(SNAPSHOTS_SUBJECT, &part);
                    }
                }
                let update = completed.expect("publisher sends one complete transfer");
                if let Update::Change { revision, .. } = &update {
                    assert_eq!(
                        *revision,
                        last_change + 1,
                        "healthy changes stay contiguous"
                    );
                    last_change = *revision;
                }
                if let Update::Snapshot { revision, state } = &update {
                    assert_eq!(&*healthy.snapshot(), state);
                    if *revision > 0 {
                        recovered += 1;
                        assert_eq!(&*gap.snapshot(), state);
                        assert_eq!(&*cold.snapshot(), state);
                        if backlog && recovered == 1 {
                            assert_eq!(*revision, 4, "new arrivals cannot move the checkpoint");
                            assert_eq!(updates.len(), 5, "all four preceding changes must be sent");
                            assert_eq!(state.peers[0].address, "a");
                            assert_eq!(state.peers[1].address, "B");
                            assert_eq!(state.peers[1].time, -17);
                        } else if !backlog && recovered == 1 {
                            assert_eq!(*revision, 5);
                            assert_eq!(
                                updates.len(),
                                6,
                                "one-item refill cannot postpone recovery"
                            );
                        }
                    }
                }
                if matches!(
                    &update,
                    Update::Change {
                        change: Change::Remove(_),
                        ..
                    }
                ) {
                    assert_eq!(healthy.snapshot().peers.len(), 1);
                    assert_eq!(healthy.snapshot().peers[0].address, "a");
                }
                updates.push(update);
                if updates.len() == 24 {
                    done.notify_one();
                }
                // Refill while the actual publisher awaits its send. A one-item queue
                // empties on every pop; the other schedule always retains a backlog.
                let mut stats = shared.statistics.lock().unwrap();
                let count = if backlog && sequence == 0 { 4 } else { 1 };
                for _ in 0..count {
                    sequence += 1;
                    let event = if backlog && sequence == 3 {
                        Event::Disconnect {
                            address: "B".into(),
                        }
                    } else {
                        Event::Heartbeat {
                            address: if backlog && sequence == 2 { "a" } else { "B" }.into(),
                            position: [sequence as f64, -0.0, 16.0],
                        }
                    };
                    assert!(stats.observe(
                        Observation {
                            version: 2,
                            incarnation: grant.incarnation,
                            generation: grant.generation,
                            producer: uuid::Uuid::nil(),
                            sequence,
                            event,
                        },
                        if sequence == 4 { -17 } else { sequence as i64 }
                    ));
                }
                drop(stats);
                shared.statistics_pending.notify_one();
                let step = if backlog {
                    if updates.len() == 1 { 5 } else { 2 }
                } else {
                    1
                };
                async move {
                    tokio::time::advance(Duration::from_secs(step)).await;
                    true
                }
            },
        );
        tokio::pin!(publish);
        tokio::select! {
            _ = &mut publish => panic!("publisher unexpectedly stopped"),
            _ = done.notified() => {}
        }
    }
    assert!(
        recovered >= 2,
        "recovery must progress across multiple deadlines while changes continue"
    );
}

struct Broker {
    child: std::process::Child,
    reader: Option<std::thread::JoinHandle<()>>,
    config: std::path::PathBuf,
    port: u16,
}
impl Broker {
    fn start(binary: &str, port: Option<u16>) -> Self {
        let config =
            std::env::temp_dir().join(format!("statistics-nats-{}.conf", uuid::Uuid::new_v4()));
        std::fs::write(&config, "max_payload: 1024\n").unwrap();
        let mut command = std::process::Command::new(binary);
        command
            .args([
                "--addr",
                "127.0.0.1",
                "--port",
                &port.map(|p| p.to_string()).unwrap_or("-1".into()),
                "--config",
            ])
            .arg(&config)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if let Some((_, address)) = line.split_once("Listening for client connections on ")
                    && let Ok(address) = address.trim().parse::<std::net::SocketAddr>()
                {
                    let _ = send.try_send(address.port());
                }
            }
        });
        let mut broker = Self {
            child,
            reader: Some(reader),
            config,
            port: 0,
        };
        broker.port = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        broker
    }
    fn url(&self) -> String {
        format!("nats://127.0.0.1:{}", self.port)
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
        let _ = std::fs::remove_file(&self.config);
    }
}
async fn complete(
    sub: &mut async_nats::Subscriber,
    assembler: &mut Assembler,
    count: usize,
    source: Option<uuid::Uuid>,
) -> Snapshot {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let message = sub.next().await.unwrap();
            assert!(
                message.payload.len() <= 1024,
                "use actual advertised NATS limit"
            );
            if source.is_some_and(|source| {
                crate::protocol::bus::statistics::frame_producer(&message.payload) != Some(source)
            }) {
                continue;
            }
            if let Some(snapshot) = assembler.receive(&message.payload)
                && snapshot.peers.len() == count
            {
                return snapshot;
            }
        }
    })
    .await
    .expect("complete statistics snapshot")
}
async fn producer_control(client: &async_nats::Client, request: ProducerRequest) -> ProducerReply {
    let reply = tokio::time::timeout(
        crate::protocol::bus::REQUEST_DEADLINE,
        client.request(PRODUCER_SUBJECT, request.encode().unwrap().into()),
    )
    .await
    .unwrap()
    .unwrap();
    ProducerReply::decode(&reply.payload).unwrap()
}
async fn register_producer(client: &async_nats::Client, producer: uuid::Uuid) -> ProducerGrant {
    let ProducerReply::Status { incarnation } =
        producer_control(client, ProducerRequest::Status).await
    else {
        panic!("status")
    };
    let ProducerReply::Grant(grant) = producer_control(
        client,
        ProducerRequest::Begin {
            incarnation,
            producer,
        },
    )
    .await
    else {
        panic!("grant")
    };
    assert_eq!(
        producer_control(client, ProducerRequest::Confirm(grant.clone())).await,
        ProducerReply::Active(grant.clone())
    );
    grant
}
#[tokio::test]
#[ignore = "requires PULSE_NATS_SERVER_BIN; starts and restarts an isolated broker"]
async fn statistics_live_broker_late_join_and_reconnect_recover_complete_state() {
    let binary = std::env::var("PULSE_NATS_SERVER_BIN").unwrap();
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    assert_eq!(client.max_payload(), 1024);
    let mut sub = client.subscribe(SNAPSHOTS_SUBJECT).await.unwrap();
    client.flush().await.unwrap();
    let feed = NatsClusterFeed::spawn(NatsFeedOptions {
        url: broker.url(),
        discovery_interval_ms: 100,
        ..Default::default()
    });
    let mut assembler = Assembler::default();
    assert!(
        complete(&mut sub, &mut assembler, 0, None)
            .await
            .peers
            .is_empty()
    );
    let epoch = uuid::Uuid::new_v4();
    let grant = register_producer(&client, epoch).await;
    for sequence in 1..=100 {
        let observation = Observation {
            version: 2,
            incarnation: grant.incarnation,
            generation: grant.generation,
            producer: epoch,
            sequence,
            event: Event::Heartbeat {
                address: format!("MixedCase{sequence}"),
                position: [sequence as f64, -0.0, 0.1],
            },
        };
        client
            .publish(OBSERVATIONS_SUBJECT, observation.encode().unwrap().into())
            .await
            .unwrap();
    }
    client.flush().await.unwrap();
    let full = complete(&mut sub, &mut assembler, 100, None).await;
    assert_eq!(full.peers[0].address, "MixedCase1");
    assert_eq!(full.peers[99].address, "MixedCase100");
    assert!(full.peers[0].time > 0);
    assert!(!full.discovery.is_empty());
    // A fresh reader needs no new heartbeat or topology to obtain all retained peers.
    let mut late = client.subscribe(SNAPSHOTS_SUBJECT).await.unwrap();
    let mut late_assembler = Assembler::default();
    assert_eq!(
        complete(&mut late, &mut late_assembler, 100, None)
            .await
            .peers,
        full.peers
    );
    let port = broker.port;
    drop(broker);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let restarted_ms = chrono::Utc::now().timestamp_millis();
    let _restarted = Broker::start(&binary, Some(port));
    let recovered = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let view = complete(&mut sub, &mut assembler, 100, None).await;
            let beacon = crate::protocol::bus::topology::decode_discovery(&view.discovery).unwrap();
            if beacon.status.unwrap().current_time > restarted_ms as u64 {
                break view;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(recovered.peers, full.peers);
    // Disconnecting any socket removes the wallet; later heartbeat appends it.
    for (sequence, event) in [
        (
            101,
            Event::Disconnect {
                address: "MixedCase1".into(),
            },
        ),
        (
            102,
            Event::Heartbeat {
                address: "MixedCase1".into(),
                position: [0.0; 3],
            },
        ),
    ] {
        client
            .publish(
                OBSERVATIONS_SUBJECT,
                Observation {
                    version: 2,
                    incarnation: grant.incarnation,
                    generation: grant.generation,
                    producer: epoch,
                    sequence,
                    event,
                }
                .encode()
                .unwrap()
                .into(),
            )
            .await
            .unwrap();
    }
    client.flush().await.unwrap();
    let reordered = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let view = complete(&mut sub, &mut assembler, 100, None).await;
            if view.peers[99].address == "MixedCase1" {
                break view;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(reordered.peers[0].address, "MixedCase2");
    drop(feed);
    // Pulse restart fences old controls/events, independently of broker reconnect.
    let feed = NatsClusterFeed::spawn(NatsFeedOptions {
        url: _restarted.url(),
        discovery_interval_ms: 100,
        ..Default::default()
    });
    let mut fresh = client.subscribe(SNAPSHOTS_SUBJECT).await.unwrap();
    let mut fresh_assembler = Assembler::default();
    let source = Some(feed.shared.statistics_epoch);
    complete(&mut fresh, &mut fresh_assembler, 0, source).await;
    assert!(matches!(
        producer_control(&client, ProducerRequest::Confirm(grant.clone())).await,
        ProducerReply::Rejected { .. }
    ));
    let replacement = register_producer(&client, epoch).await;
    assert_ne!(replacement.incarnation, grant.incarnation);
    for (incarnation, generation, sequence, address) in [
        (grant.incarnation, grant.generation, 103, "stale"),
        (replacement.incarnation, replacement.generation, 1, "fresh"),
    ] {
        client
            .publish(
                OBSERVATIONS_SUBJECT,
                Observation {
                    version: 2,
                    incarnation,
                    generation,
                    producer: epoch,
                    sequence,
                    event: Event::Heartbeat {
                        address: address.into(),
                        position: [0.; 3],
                    },
                }
                .encode()
                .unwrap()
                .into(),
            )
            .await
            .unwrap();
    }
    client.flush().await.unwrap();
    assert_eq!(
        complete(&mut fresh, &mut fresh_assembler, 1, source)
            .await
            .peers[0]
            .address,
        "fresh"
    );
    drop(feed);
}

#[tokio::test]
async fn statistics_topology_is_retained_even_above_the_old_single_frame_cap() {
    use crate::protocol::pulse::decentraland::kernel::comms::v3::IslandStatusMessage;
    use crate::pulse::cluster::{ClusterInfo, ClusterPeerInfo};
    let shared = super::tests::feed_shared(1);
    let feed = NatsClusterFeed {
        shared: shared.clone(),
        drain: tokio::spawn(std::future::pending()),
    };
    let mut pass = ClusterPass::default();
    pass.clusters.push(ClusterInfo {
        id: "CustomID".into(),
        realm: "other-world".into(),
        count: 30_000,
        centroid: Default::default(),
        radius: 1.25,
    });
    for peer in 0..30_000 {
        pass.peers.push(ClusterPeerInfo {
            peer,
            wallet: format!("0x{peer:040x}"),
            cluster_id: "CustomID".into(),
            realm: "other-world".into(),
            position: Default::default(),
            parcel: 0,
        });
    }
    feed.publish_topology(&pass, 30_000);
    let view = shared.statistics.lock().unwrap().snapshot();
    assert!(view.islands.len() > crate::protocol::bus::topology::MAX_ISLANDS_BYTES);
    let islands = IslandStatusMessage::decode(view.islands.as_slice()).unwrap();
    assert_eq!(islands.data[0].peers.len(), 30_000);
    assert_eq!(islands.data[0].id, "CustomID");
    assert_eq!(islands.data[0].max_peers, 0);
    assert_eq!(islands.data[0].radius, 1.25);
    assert!(
        view.peers.is_empty(),
        "native topology never adds statistical membership"
    );
}
