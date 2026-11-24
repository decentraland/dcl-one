//! Deployment control metadata is independent of Pulse's in-memory assignment ledger.
#[cfg(unix)]
#[path = "support/auth.rs"]
mod auth;
#[path = "support/db.rs"]
mod db;
#[path = "support/gate.rs"]
mod gate;
use catalyst::comms::interfaces::PulseGeneration;

#[cfg(unix)]
#[tokio::test]
async fn pulse_process_disconnects_enet_peers_on_unix_signals() {
    use catalyst::comms::protocol::pulse::decentraland::pulse::{
        ClientMessage, HandshakeRequest, ServerMessage, client_message, server_message,
    };
    use catalyst::comms::pulse::{
        handshake::build_signed_fetch_payload,
        transport::{Event, Host, HostConfig, Packet},
    };
    use prost::Message as _;
    use std::{net::TcpListener, path::PathBuf, process::Stdio, time::Duration};
    use tokio::{
        io::{AsyncBufReadExt, BufReader},
        process::Command,
        time::{Instant, timeout, timeout_at},
    };

    struct ReplayFiles(PathBuf);
    impl Drop for ReplayFiles {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("replay-journal.lock"));
            let _ = std::fs::remove_file(self.0.with_extension("replay-journal.tmp"));
        }
    }

    gate::required_database_in_ci();
    let Some(url) = catalyst::db::scratch::url().await else {
        return;
    };
    let audience = format!("pulse-signal-test-{}", uuid::Uuid::new_v4());
    let replay = ReplayFiles(std::env::temp_dir().join(format!("{audience}.tsv")));
    let bans = TcpListener::bind("127.0.0.1:0").unwrap();
    let bans_url = format!("http://{}/bans", bans.local_addr().unwrap());
    for signal in ["-TERM", "-INT"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_catalyst"))
            .arg("pulse")
            .env_clear()
            .env("DATABASE_URL", &url)
            .env("ARCHIPELAGO_CONTROL_V4_AUDIENCE", &audience)
            .env("COMMS_MODERATOR_TOKEN", "pulse-test")
            .env("PULSE_BANS_URL", &bans_url)
            .env("PULSE_BIND", "127.0.0.1:0")
            .env("PULSE_METRICS_BIND", "127.0.0.1:0")
            .env("PULSE_REPLAY_STATE_PATH", &replay.0)
            .env("RUST_LOG", "info")
            .env("NO_COLOR", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let bind = timeout(Duration::from_secs(30), async {
            while let Some(line) = lines.next_line().await.unwrap() {
                if line.contains("listening (enet)") {
                    return line
                        .split_ascii_whitespace()
                        .find_map(|field| field.strip_prefix("local="))
                        .expect("Pulse did not log its bound ENet address")
                        .parse()
                        .unwrap();
                }
            }
            panic!("Pulse exited before starting its ENet listener");
        })
        .await
        .expect("Pulse startup timed out");
        let stdout_drain = tokio::spawn(async move {
            tokio::io::copy(&mut lines.into_inner(), &mut tokio::io::sink()).await
        });

        let mut client = Host::bind(HostConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            max_peers: 1,
            channel_limit: 3,
        })
        .await
        .unwrap();
        let peer = client.connect(bind, 3).unwrap();
        timeout(Duration::from_secs(10), async {
            while !matches!(client.service().await.unwrap(), Some(Event::Connect { .. })) {}
        })
        .await
        .expect("client did not connect to Pulse");

        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let metadata = r#"{"signer":"dcl:explorer"}"#;
        let payload = build_signed_fetch_payload("connect", "/", &timestamp, metadata);
        let chain = auth::signed_chain(&payload).await;
        let auth_chain = serde_json::to_vec(&serde_json::json!({
            "x-identity-auth-chain-0": serde_json::to_string(&chain[0]).unwrap(),
            "x-identity-auth-chain-1": serde_json::to_string(&chain[1]).unwrap(),
            "x-identity-timestamp": timestamp,
            "x-identity-metadata": metadata,
        }))
        .unwrap();
        let request = ClientMessage {
            message: Some(client_message::Message::Handshake(HandshakeRequest {
                auth_chain,
                ..Default::default()
            })),
        };
        client
            .send(peer, Packet::reliable(0, request.encode_to_vec()))
            .await
            .unwrap();
        client.flush();
        timeout(Duration::from_secs(10), async {
            loop {
                match client.service().await.unwrap() {
                    Some(Event::Receive { packet, .. }) => {
                        let response = ServerMessage::decode(packet.data).unwrap();
                        if let Some(server_message::Message::Handshake(handshake)) =
                            response.message
                        {
                            assert!(
                                handshake.success,
                                "Pulse refused the test login: {handshake:?}"
                            );
                            return;
                        }
                    }
                    Some(Event::Disconnect { .. }) => panic!("Pulse disconnected before login"),
                    _ => {}
                }
            }
        })
        .await
        .expect("Pulse did not complete the test login");
        let wire_deadline = Instant::now() + Duration::from_secs(2);
        assert!(
            Command::new("kill")
                .args([signal, &pid.to_string()])
                .status()
                .await
                .unwrap()
                .success()
        );
        let (received_notice, status, drained) = tokio::join!(
            async {
                timeout_at(wire_deadline, async {
                    while !matches!(
                        client.service().await.unwrap(),
                        Some(Event::Disconnect { .. })
                    ) {}
                })
                .await
                .is_ok()
                    && Instant::now() < wire_deadline
            },
            timeout(Duration::from_secs(10), child.wait()),
            timeout(Duration::from_secs(10), stdout_drain),
        );
        let status = status.expect("Pulse did not shut down").unwrap();
        drained
            .expect("Pulse stdout did not close")
            .expect("Pulse stdout reader task failed")
            .expect("could not drain Pulse stdout");
        assert!(
            status.success(),
            "Pulse exited after {signal} with {status}"
        );
        assert!(
            received_notice,
            "the ENet peer did not disconnect after {signal}"
        );
    }
}

#[tokio::test]
async fn generation_is_exclusive_monotonic_and_does_not_persist_assignments() {
    gate::required_database_in_ci();
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let first = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "generation-test".into(),
    )
    .await
    .unwrap();
    let binding = first.binding().clone();
    assert!(binding.valid());
    assert!(
        catalyst::pulse_generation::HeldPulseGeneration::acquire(
            db.pool.clone(),
            "generation-test".into()
        )
        .await
        .is_err()
    );
    let stored: (i64, String) = sqlx::query_as("SELECT epoch,source_incarnation::text FROM social.pulse_authority_generation WHERE deployment_audience='generation-test'").fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        stored,
        (binding.epoch as i64, binding.source_incarnation.clone())
    );
    let assignments: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.archipelago_v4_assignments")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(assignments, 0);
    first.close().await;
    assert!(first.lost().is_cancelled());
    let second = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "generation-test".into(),
    )
    .await
    .unwrap();
    assert!(second.binding().epoch > binding.epoch);
    assert_ne!(
        second.binding().source_incarnation,
        binding.source_incarnation
    );
    second.close().await;
    drop((first, second));
    db.teardown().await;
}

#[tokio::test]
async fn held_generation_connection_loss_revokes_local_capability_before_reuse() {
    gate::required_database_in_ci();
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let first = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "lost-generation".into(),
    )
    .await
    .unwrap();
    first.end_lease_backend_for_test(&db.pool).await;
    tokio::time::timeout(
        std::time::Duration::from_secs(8),
        first.observe_lease_loss_for_test(),
    )
    .await
    .unwrap();
    assert!(first.lost().is_cancelled());
    let second = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "lost-generation".into(),
    )
    .await
    .unwrap();
    assert!(second.binding().epoch > first.binding().epoch);
    second.close().await;
    first.close().await;
    drop((first, second));
    db.teardown().await;
}

#[path = "support/lost_commit.rs"]
mod lost_commit;

#[tokio::test]
async fn lost_generation_commit_ack_never_publishes_a_capability_and_fresh_acquisition_rotates() {
    gate::required_database_in_ci();
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let proxy = lost_commit::LostCommitProxy::start(&db.pool).await;
    proxy.lose_next_commit();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        catalyst::pulse_generation::HeldPulseGeneration::acquire(
            proxy.pool.clone(),
            "lost-generation-commit".into(),
        ),
    )
    .await
    .unwrap();
    assert!(
        result.is_err(),
        "unacknowledged generation commit grants no local capability"
    );
    let stored: (i64, String) = sqlx::query_as("SELECT epoch,source_incarnation::text FROM social.pulse_authority_generation WHERE deployment_audience='lost-generation-commit'").fetch_one(&db.pool).await.unwrap();
    let successor = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "lost-generation-commit".into(),
    )
    .await
    .unwrap();
    assert!(successor.binding().epoch > stored.0 as u64);
    assert_ne!(successor.binding().source_incarnation, stored.1);
    successor.close().await;
    drop(successor);
    proxy.pool.close().await;
    drop(proxy);
    db.teardown().await;
}

#[tokio::test]
async fn unregistered_sources_allocate_no_slots_and_complete_new_generation_reclaims_only_known_old_slots()
 {
    use catalyst::comms::gatekeeper::admission::cluster::fence::{
        AssignmentFence, PgAssignmentFence,
    };
    use catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterChange;
    gate::required_database_in_ci();
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let audience = "bounded-generation-recovery";
    let fence = PgAssignmentFence::new(db.pool.clone(), audience.into()).unwrap();
    let hint = |source: String| PeerClusterChange {
        session: format!("0x{:040x}", 2),
        source_incarnation: source,
        assignment_version: 1,
        ..Default::default()
    };
    let wallet = format!("0x{:040x}", 1);
    assert!(
        !fence
            .accept_selected_hint(&wallet, &hint(uuid::Uuid::new_v4().to_string()))
            .await
    );
    assert!(fence.selected_reconciliation_targets().is_empty());
    let first =
        catalyst::pulse_generation::HeldPulseGeneration::acquire(db.pool.clone(), audience.into())
            .await
            .unwrap();
    let old = first.binding().clone();
    assert!(
        fence
            .accept_selected_hint(&wallet, &hint(old.source_incarnation.clone()))
            .await
    );
    assert_eq!(fence.selected_reconciliation_targets().len(), 1);
    first.close().await;
    let successor =
        catalyst::pulse_generation::HeldPulseGeneration::acquire(db.pool.clone(), audience.into())
            .await
            .unwrap();
    let current = successor.binding().clone();
    assert!(
        !fence
            .accept_selected_hint(
                &format!("0x{:040x}", 3),
                &hint(old.source_incarnation.clone())
            )
            .await
    );
    // A generation comparison alone and the first incomplete transfer are insufficient.
    fence.retire_obsolete_generation(&current, 128, &[]).await;
    assert_eq!(fence.selected_reconciliation_targets().len(), 1);
    assert!(fence.complete_generation_view(&current).await);
    fence.retire_obsolete_generation(&current, 128, &[]).await;
    assert_eq!(fence.selected_reconciliation_targets().len(), 1);
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    // The cursor wrapped on the previous pass; ordinary bounded passes settle the target.
    fence.retire_obsolete_generation(&current, 128, &[]).await;
    fence.retire_obsolete_generation(&current, 128, &[]).await;
    assert!(fence.selected_reconciliation_targets().is_empty());
    let target = catalyst::comms::protocol::bus::selection_view::SelectionTarget {
        wallet: wallet.clone(),
        session: format!("0x{:040x}", 4),
        assignment_version: 1,
    };
    assert!(fence.recover_selected_target(&current, &target).await);
    assert_eq!(
        fence.selected_reconciliation_targets(),
        vec![(wallet.clone(), target.session.clone())]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM social.archipelago_v4_assignments")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        0
    );
    // A wallet that owned its row under one generation takes it back under the next,
    // inside the lease, after its old slot retired.
    let selected =
        |source: &str| catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
            selected_origin: 1,
            selected_identity: Some(1),
            assignment_state: 1,
            wallet: wallet.clone(),
            session: target.session.clone(),
            cluster_id: "c1".into(),
            realm: "realm".into(),
            source_incarnation: source.into(),
            assignment_version: 1,
            token_not_before: Some(0),
            pass: 1,
            ..Default::default()
        };
    let owned = fence
        .lock_selected(&selected(&current.source_incarnation))
        .await
        .unwrap()
        .token_fence();
    successor.close().await;
    let third =
        catalyst::pulse_generation::HeldPulseGeneration::acquire(db.pool.clone(), audience.into())
            .await
            .unwrap();
    let next = third.binding().clone();
    assert!(fence.complete_generation_view(&next).await);
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    fence.retire_obsolete_generation(&next, 128, &[]).await;
    fence.retire_obsolete_generation(&next, 128, &[]).await;
    assert!(fence.selected_reconciliation_targets().is_empty());
    let lease = fence
        .lock_selected(&selected(&next.source_incarnation))
        .await
        .unwrap();
    assert!(!lease.read_only() && lease.token_fence().owner_epoch > owned.owner_epoch);
    drop(lease);
    third.close().await;
    drop((first, successor, third, fence));
    db.teardown().await;
}

mod room_authority {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use catalyst::comms::{
        gatekeeper::admission::relay_authority::{self, RelayPolicy, proof::VerifiedRoom},
        protocol::room_authority::{self as wire, AUTHORITY_PATH, PROOF_DOMAIN},
    };
    use hmac::{Hmac, KeyInit, Mac};
    use prost::Message;
    use sha2::{Digest, Sha256};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tower::ServiceExt;
    #[derive(Default)]
    struct Policy {
        calls: AtomicUsize,
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl RelayPolicy for Policy {
        async fn authorize(&self, _: &VerifiedRoom) -> Result<(), wire::denied::Reason> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(())
        }
    }
    fn request(source: &str) -> Request<Body> {
        let now = chrono::Utc::now().timestamp() as u64;
        let claims = serde_json::json!({"iss":"key", "sub":format!("0x{}", "11".repeat(20)), "exp":now+60, "video":{"room":"scene-room","roomJoin":true,"canPublishData":true}});
        let hp = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let mut jwt = Hmac::<Sha256>::new_from_slice(&[7; 32]).unwrap();
        jwt.update(hp.as_bytes());
        let mut proof = Hmac::<Sha256>::new_from_slice(&jwt.finalize().into_bytes()).unwrap();
        proof.update(PROOF_DOMAIN);
        proof.update(&[3; 32]);
        proof.update(&[0x11; 20]);
        proof.update(&[0x22; 20]);
        proof.update(&Sha256::digest(hp.as_bytes()));
        let body = wire::AuthorityRequest {
            operation: Some(wire::authority_request::Operation::Join(wire::Join {
                pulse_instance: vec![1; 16],
                connection_generation: vec![2; 16],
                wallet: vec![0x11; 20],
                session: vec![0x22; 20],
                header_payload: hp,
                nonce: vec![3; 32],
                proof: proof.finalize().into_bytes().to_vec(),
            })),
        }
        .encode_to_vec();
        let nonce = URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes());
        let preimage = format!(
            "POST\n{AUTHORITY_PATH}\n{now}\n{nonce}\n{source}\n{}",
            hex::encode(Sha256::digest(&body))
        );
        let mut mac = Hmac::<Sha256>::new_from_slice(&[9; 32]).unwrap();
        mac.update(preimage.as_bytes());
        Request::builder()
            .method("POST")
            .uri(AUTHORITY_PATH)
            .header("content-type", "application/x-protobuf")
            .header("accept", "application/x-protobuf")
            .header("x-pulse-authority-timestamp", now.to_string())
            .header("x-pulse-authority-nonce", nonce)
            .header("x-pulse-authority-source", source)
            .header(
                "x-pulse-authority-signature",
                URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()),
            )
            .body(Body::from(body))
            .unwrap()
    }
    #[tokio::test]
    async fn source_cutover_rejects_old_requests_and_inflight_old_lease_issuance() {
        gate::required_database_in_ci();
        let Some(db) = db::scratch_database().await else {
            return;
        };
        let first = catalyst::pulse_generation::HeldPulseGeneration::acquire(
            db.pool.clone(),
            "room-generation".into(),
        )
        .await
        .unwrap();
        let old = first.binding().source_incarnation.clone();
        let policy = Arc::new(Policy::default());
        let authority = Arc::new(
            relay_authority::RelayAuthority::new_with_generation(
                policy.clone(),
                "key".into(),
                vec![7; 32],
                [9; 32],
                db.pool.clone(),
                "room-generation".into(),
            )
            .unwrap(),
        );
        let router: axum::Router = relay_authority::router(authority);
        let waiting = tokio::spawn(router.clone().oneshot(request(&old)));
        tokio::time::timeout(std::time::Duration::from_secs(8), policy.entered.notified())
            .await
            .expect("authenticated request reaches held room policy before cutover");
        first.close().await;
        let successor = catalyst::pulse_generation::HeldPulseGeneration::acquire(
            db.pool.clone(),
            "room-generation".into(),
        )
        .await
        .unwrap();
        assert!(
            !waiting.is_finished(),
            "issuance must still be held across the actual generation cutover"
        );
        policy.release.notify_one();
        let response = waiting.await.unwrap().unwrap();
        let bytes = to_bytes(response.into_body(), wire::MAX_BODY)
            .await
            .unwrap();
        let response = wire::AuthorityResponse::decode(bytes).unwrap();
        assert!(matches!(
            response.result,
            Some(wire::authority_response::Result::Denied(_))
        ));
        let denied = router.clone().oneshot(request(&old)).await.unwrap();
        assert!(!denied.status().is_success());
        assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
        let accepted = router
            .oneshot(request(&successor.binding().source_incarnation))
            .await
            .unwrap();
        let bytes = to_bytes(accepted.into_body(), wire::MAX_BODY)
            .await
            .unwrap();
        assert!(matches!(
            wire::AuthorityResponse::decode(bytes).unwrap().result,
            Some(wire::authority_response::Result::Lease(_))
        ));
        successor.close().await;
        drop((first, successor));
        db.teardown().await;
    }
}

mod complete_view_recovery {
    use super::*;
    use catalyst::comms::{
        gatekeeper::{
            admission::cluster::{
                ClusterConfig,
                fence::{AssignmentFence, PgAssignmentFence, PulseAdmissionStamp},
                peer_state::ClusterPeerState,
                subscriber::{ClusterGateway, ClusterSubscriber, GatewayError},
            },
            bus::{InProcessBus, RequestError},
            livekit::Removal,
        },
        protocol::bus::selection_view::{SelectionTarget, SelectionView, ViewRequest},
    };
    use std::{sync::Arc, time::Duration};
    struct DeniedGateway;
    #[async_trait::async_trait]
    impl ClusterGateway for DeniedGateway {
        async fn is_denied(&self, _: &str) -> Result<bool, GatewayError> {
            Ok(true)
        }
        async fn mint_connection_string(
            &self,
            _: &PulseAdmissionStamp,
            _: &str,
            _: Option<&str>,
            _: &str,
            _: u64,
            _: Option<u64>,
        ) -> Result<String, GatewayError> {
            panic!("discovery is never admission evidence")
        }
        async fn remove_participant(
            &self,
            _: &str,
            _: &str,
            _: Option<i64>,
        ) -> Result<Removal, GatewayError> {
            Err(GatewayError::Unknown)
        }
        async fn holds_participant(&self, _: &str, _: &str) -> Result<bool, GatewayError> {
            Ok(false)
        }
    }
    #[tokio::test]
    async fn started_strict_discovery_recovers_all_unchanged_targets_after_restart_without_hints() {
        gate::required_database_in_ci();
        let Some(db) = db::scratch_database().await else {
            return;
        };
        let authority = catalyst::pulse_generation::HeldPulseGeneration::acquire(
            db.pool.clone(),
            "complete-view-recovery".into(),
        )
        .await
        .unwrap();
        let binding = authority.binding().clone();
        let targets: Vec<_> = (1..=273)
            .map(|index| SelectionTarget {
                wallet: format!("0x{index:040x}"),
                session: format!("0x{:040x}", index + 1000),
                assignment_version: 1,
            })
            .collect();
        let view = Arc::new(SelectionView::new(binding.clone(), 1, targets.clone()).unwrap());
        let bus = Arc::new(InProcessBus::new());
        bus.respond_to_requests(Arc::new(move |subject, bytes| {
            let request = ViewRequest::decode(subject, bytes).ok_or(RequestError::NoResponders)?;
            view.page(&request)
                .and_then(|page| page.encode())
                .ok_or(RequestError::NoResponders)
        }));
        for _ in 0..2 {
            let fence = Arc::new(
                PgAssignmentFence::new(db.pool.clone(), binding.audience.clone()).unwrap(),
            );
            assert!(fence.selected_reconciliation_targets().is_empty());
            let config = ClusterConfig {
                control_v4_audience: Some(binding.audience.clone()),
                ..Default::default()
            };
            let peers = Arc::new(ClusterPeerState::new(
                config.peer_state_max,
                config.peer_state_ttl_ms,
            ));
            let subscriber = ClusterSubscriber::new_with_assignment_fence(
                bus.clone(),
                Arc::new(DeniedGateway),
                peers,
                config,
                Some(fence.clone()),
            );
            subscriber.start();
            let discovery = {
                let subscriber = subscriber.clone();
                tokio::spawn(async move { subscriber.recover_selection_views_for_test().await })
            };
            tokio::time::timeout(Duration::from_secs(8), async {
                loop {
                    if fence.selected_reconciliation_targets().len() == targets.len() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect(
                "started discovery recovers every unchanged target without cluster-change hints",
            );
            assert_eq!(
                fence.selected_reconciliation_targets(),
                targets
                    .iter()
                    .map(|target| (target.wallet.clone(), target.session.clone()))
                    .collect::<Vec<_>>()
            );
            discovery.abort();
            let _ = discovery.await;
            subscriber.stop().await;
            drop((subscriber, fence));
        }
        assert!(
            bus.published().is_empty(),
            "the fixture supplied no cluster-change events or reconnects"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM social.archipelago_v4_assignments")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            0
        );
        authority.close().await;
        drop(authority);
        db.teardown().await;
    }
}

#[tokio::test]
async fn mint_and_delivery_use_fresh_exact_source_permissions_after_cutover() {
    use catalyst::comms::gatekeeper::admission::cluster::{
        fence::{PulseAdmissionStamp, SelectedRoomOrigin},
        generation,
    };
    gate::required_database_in_ci();
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let first = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "operation-grants".into(),
    )
    .await
    .unwrap();
    let stamp = PulseAdmissionStamp {
        selected_origin: SelectedRoomOrigin::Native,
        selected_identity: 1,
        wallet: format!("0x{:040x}", 1),
        session: format!("0x{:040x}", 2),
        cluster_id: "room".into(),
        realm: "realm".into(),
        source_incarnation: first.binding().source_incarnation.clone(),
        assignment_version: 1,
        token_not_before: 0,
        credential_not_before: 0,
        safety_generation: 0,
        safety_token_floor: 0,
    };
    let pre_cutover_mint = generation::authorize(&db.pool, "operation-grants", &stamp)
        .await
        .unwrap();
    first.close().await;
    let successor = catalyst::pulse_generation::HeldPulseGeneration::acquire(
        db.pool.clone(),
        "operation-grants".into(),
    )
    .await
    .unwrap();
    assert!(
        pre_cutover_mint.consume(&stamp),
        "pre-cutover permission retains only its original finite completion window"
    );
    assert!(!pre_cutover_mint.consume(&stamp));
    assert!(
        generation::authorize(&db.pool, "operation-grants", &stamp)
            .await
            .is_err(),
        "old mint cannot obtain a fresh permission"
    );
    assert!(
        generation::authorize(&db.pool, "operation-grants", &stamp)
            .await
            .is_err(),
        "final delivery is a separate current-source check, not a deadline refresh"
    );
    let mut current = stamp.clone();
    current.source_incarnation = successor.binding().source_incarnation.clone();
    let grant = generation::authorize(&db.pool, "operation-grants", &current)
        .await
        .unwrap();
    assert!(!grant.consume(&stamp));
    assert!(grant.consume(&current));
    successor.close().await;
    drop((first, successor));
    db.teardown().await;
}

#[tokio::test]
async fn current_view_retains_exact_predecessor_until_reseed_and_reclaims_absent_slots_at_capacity()
{
    use catalyst::comms::{
        gatekeeper::admission::cluster::fence::{AssignmentFence, PgAssignmentFence},
        protocol::{
            bus::selection_view::SelectionTarget, pulse::decentraland::pulse::PeerClusterSnapshot,
        },
    };
    gate::required_database_in_ci();
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let audience = "current-view-predecessors";
    let first =
        catalyst::pulse_generation::HeldPulseGeneration::acquire(db.pool.clone(), audience.into())
            .await
            .unwrap();
    let fence =
        PgAssignmentFence::with_selected_limit_for_test(db.pool.clone(), audience.into(), 2)
            .unwrap();
    let address = |number| format!("0x{number:040x}");
    let snapshot =
        |wallet: String, session: String, source: String, identity| PeerClusterSnapshot {
            wallet,
            session,
            cluster_id: "current".into(),
            realm: "realm".into(),
            source_incarnation: source,
            assignment_version: identity,
            pass: 1,
            selected_origin: 1,
            selected_identity: Some(identity),
            assignment_state: 1,
            token_not_before: Some(0),
            pending_takeovers: vec![],
        };
    let previous = snapshot(
        address(1),
        address(11),
        first.binding().source_incarnation.clone(),
        1,
    );
    let absent = snapshot(
        address(2),
        address(12),
        first.binding().source_incarnation.clone(),
        1,
    );
    let previous_lease = fence.lock_selected(&previous).await.unwrap();
    let predecessor = previous_lease.token_fence();
    drop(previous_lease);
    let absent_lease = fence.lock_selected(&absent).await.unwrap();
    let absent_fence = absent_lease.token_fence();
    drop(absent_lease);
    first.close().await;
    let current =
        catalyst::pulse_generation::HeldPulseGeneration::acquire(db.pool.clone(), audience.into())
            .await
            .unwrap();
    let binding = current.binding().clone();
    let targets = vec![
        SelectionTarget {
            wallet: address(1),
            session: address(21),
            assignment_version: 2,
        },
        SelectionTarget {
            wallet: address(3),
            session: address(23),
            assignment_version: 1,
        },
    ];
    assert!(fence.complete_generation_view(&binding).await);
    assert!(
        !fence.recover_selected_target(&binding, &targets[1]).await,
        "the old two-slot map starts at capacity"
    );
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            fence
                .retire_obsolete_generation(&binding, 128, &targets)
                .await;
            if !fence
                .selected_reconciliation_targets()
                .iter()
                .any(|(wallet, _)| wallet == &absent.wallet)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("absent old wallets free capacity after the original grant barrier");
    assert_eq!(
        fence.selected_reconciliation_targets(),
        vec![(previous.wallet.clone(), previous.session.clone())],
        "a current-view wallet keeps its exact old predecessor until its later seeding batch"
    );
    assert!(
        fence.recover_selected_target(&binding, &targets[1]).await,
        "absent retirement frees capacity for a new current wallet"
    );
    assert!(fence.recover_selected_target(&binding, &targets[0]).await);
    let successor = snapshot(
        previous.wallet.clone(),
        targets[0].session.clone(),
        binding.source_incarnation.clone(),
        2,
    );
    let lease = fence.lock_selected(&successor).await.expect("the still-fresh exact owned predecessor allows changed-session takeover without a 90-second lapse");
    assert_eq!(lease.token_fence().owner_session, successor.session);
    assert!(lease.token_fence().owner_epoch > predecessor.owner_epoch);
    drop(lease);
    let absent_row: (String, i64) = sqlx::query_as("SELECT owner_session,owner_epoch FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(audience).bind(&absent.wallet).fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        absent_row,
        (absent.session, absent_fence.owner_epoch as i64),
        "retiring its slot does not mutate the existing SQL row"
    );
    current.close().await;
    drop((first, current, fence));
    db.teardown().await;
}
