use super::*;

#[test]
fn connection_limiter_precedes_admission_and_rolls_back_refused_pre_auth() {
    let mut srv = PulseServer::new();
    srv.ip_limiter_options.enabled = true;
    srv.ip_limiter_options.max_concurrency = 1;
    assert_eq!(srv.connect_pending(1, "192.0.2.1".into(), 0), Ok(()));
    assert!(
        matches!(srv.connect_pending(4096, "::ffff:192.0.2.1".into(), 0), Err(reason) if reason.code() == 17)
    );
    assert!(!srv.peers.contains_key(&4096));
    srv.ip_limiter_options.whitelist =
        crate::pulse::hardening::IpWhitelist::parse("192.0.2.1").unwrap();
    srv.pre_auth_enet = PreAuthAdmission::new(1, 1);
    srv.pre_auth_enet.try_admit(1, "192.0.2.1");
    assert_eq!(
        srv.connect_pending(2, "192.0.2.1".into(), 0),
        Err(DisconnectReason::PreAuthIpLimitExhausted)
    );
    srv.ip_limiter_options.whitelist = crate::pulse::hardening::IpWhitelist::new();
    srv.ip_limiter.release(1);
    srv.pre_auth_enet.release_on_disconnect(1);
    assert_eq!(srv.connect_pending(3, "192.0.2.1".into(), 0), Ok(()));
}

#[test]
fn duplicate_connect_and_early_state_cleanup_cannot_replace_or_release_live_reservation() {
    let mut srv = PulseServer::new();
    srv.ip_limiter_options.enabled = true;
    srv.ip_limiter_options.max_concurrency = 1;
    assert_eq!(srv.connect_pending(1, "192.0.2.1".into(), 0), Ok(()));
    assert!(srv.connect_pending(1, "192.0.2.2".into(), 0).is_err());
    assert_eq!(srv.peers[&1].ip.as_deref(), Some("192.0.2.1"));
    srv.remove_peer_state(1);
    assert!(srv.connect_pending(2, "192.0.2.1".into(), 0).is_err());
}

#[tokio::test]
async fn actual_transport_disconnect_releases_after_early_cleanup_and_duplicate_callback_is_safe() {
    let mut srv = PulseServer::new();
    srv.ip_limiter_options.enabled = true;
    srv.ip_limiter_options.max_concurrency = 1;
    srv.connect_pending(1, "192.0.2.1".into(), 0).unwrap();
    srv.remove_peer_state(1);
    assert!(srv.connect_pending(2, "192.0.2.1".into(), 0).is_err());
    let host = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: 4,
        channel_limit: 8,
    })
    .await
    .unwrap();
    let mut transports = Transports::enet_only(host, 4);
    srv.on_disconnect(&mut transports, 1).await.unwrap();
    srv.on_disconnect(&mut transports, 1).await.unwrap();
    srv.connect_pending(2, "192.0.2.1".into(), 0).unwrap();
    assert!(srv.connect_pending(3, "192.0.2.1".into(), 0).is_err());
}

#[test]
fn corrupt_packets_exhaust_budget_then_disconnect() {
    let mut srv = PulseServer::new();

    let bad: &[u8] = &[0x0A, 0xFF];

    for _ in 0..5 {
        assert_eq!(
            srv.dispatch(7, channel::RELIABLE, bad, 1000, 1000),
            Action::Ignore
        );
    }

    assert_eq!(
        srv.dispatch(7, channel::RELIABLE, bad, 1000, 1000),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::PacketCorrupted
        }
    );
}

use crate::protocol::pulse::decentraland::pulse::{
    ClientMessage, PlayerInitialState, PlayerState, PlayerStateInput, ProfileVersionAnnouncement,
    ResyncRequest, TeleportRequest, client_message,
};

fn client_msg(inner: client_message::Message) -> Vec<u8> {
    ClientMessage {
        message: Some(inner),
    }
    .encode_to_vec()
}

fn valid_state(parcel: i32) -> PlayerState {
    let mut state = PlayerState {
        parcel_index: parcel,
        ..Default::default()
    };
    state.set_position_x_f(8.0);
    state.set_position_z_f(8.0);
    state
}

fn authed(srv: &mut PulseServer, peer: u32, wallet: &str) {
    let mut st = PeerState::new(PeerConnectionState::Authenticated, 0);
    st.wallet_id = Some(wallet.into());
    srv.peers.insert(peer, st);
    srv.identity.set(peer, wallet.into());
    srv.board.set_active(peer);
}

#[test]
fn gameplay_from_unauthenticated_peer_is_ignored() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(7, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
        state: Some(PlayerState::default()),
    }));
    assert_eq!(
        srv.dispatch(7, channel::UNRELIABLE_SEQUENCED, &bytes, 0, 0),
        Action::Ignore
    );
}

#[test]
fn early_reserved_and_oversized_application_rejection_is_terminal_for_queued_gameplay() {
    for (bad, reason) in [
        (
            vec![0xa2, 0x01, 0x00],
            DisconnectReason::InvalidHandshakeField,
        ),
        (
            vec![0xaa, 0x01, 0x00],
            DisconnectReason::InvalidHandshakeField,
        ),
        (
            {
                let mut bytes = vec![0x62, 0x80, 0x20];
                bytes.resize(4099, 0);
                bytes
            },
            DisconnectReason::PacketCorrupted,
        ),
        (
            {
                let mut bytes = vec![0x72, 0x80, 0x20];
                bytes.resize(4099, 0);
                bytes
            },
            DisconnectReason::PacketCorrupted,
        ),
    ] {
        let mut srv = PulseServer::new();
        authed(&mut srv, 7, "wallet");
        let input = client_msg(client_message::Message::Input(PlayerStateInput {
            state: Some(valid_state(5)),
        }));
        assert_eq!(srv.dispatch(7, 1, &input, 0, 100), Action::Applied);
        let before = srv.board.try_read(7).unwrap().clone();
        assert_eq!(
            srv.dispatch(7, 0, &bad, 0, 200),
            Action::Reject {
                reply: None,
                reason
            }
        );
        assert_eq!(
            srv.peers[&7].connection_state,
            PeerConnectionState::PendingDisconnect
        );
        let queued = client_msg(client_message::Message::Input(PlayerStateInput {
            state: Some(valid_state(6)),
        }));
        assert_eq!(srv.dispatch(7, 1, &queued, 0, 300), Action::Ignore);
        let teleport = client_msg(client_message::Message::Teleport(teleport_request(
            9,
            "next-realm",
        )));
        assert_eq!(srv.dispatch(7, 0, &teleport, 0, 400), Action::Ignore);
        assert_eq!(srv.board.try_read(7), Some(&before));
        assert_eq!(srv.board.last_seq(7), before.seq);
    }
}

#[test]
fn authenticated_input_publishes_snapshot_with_real_sequence() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 7, "0xabc");
    assert_eq!(srv.board.last_seq(7), crate::pulse::snapshot::NO_SEQ);
    let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
        state: Some(valid_state(5)),
    }));
    assert_eq!(
        srv.dispatch(7, channel::UNRELIABLE_SEQUENCED, &bytes, 0, 100),
        Action::Applied
    );

    assert_eq!(srv.board.last_seq(7), 0);
    let snap = srv.board.try_read(7).unwrap();
    assert_eq!(snap.seq, 0);
    assert_eq!(snap.server_tick, 100);
    assert_eq!(snap.parcel, 5);

    let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
        state: Some(valid_state(6)),
    }));
    srv.dispatch(7, channel::UNRELIABLE_SEQUENCED, &bytes, 0, 150);
    assert_eq!(srv.board.last_seq(7), 1);
}

fn teleport_request(parcel: i32, realm: &str) -> TeleportRequest {
    let mut req = TeleportRequest {
        parcel_index: parcel,
        realm: realm.into(),
        ..Default::default()
    };
    req.set_position_x_f(8.0);
    req.set_position_z_f(8.0);
    req
}

#[test]
fn teleport_seeds_realm_and_position() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 3, "0xabc");
    let req = teleport_request(0, "realm-a");
    let expected_x = req.position_x;
    let bytes = client_msg(client_message::Message::Teleport(req));
    assert_eq!(
        srv.dispatch(3, channel::RELIABLE, &bytes, 0, 50),
        Action::Applied
    );
    let snap = srv.board.try_read(3).unwrap();
    assert_eq!(snap.realm.as_deref(), Some("realm-a"));
    assert_eq!(snap.position_x, expected_x, "raw code stored verbatim");
    assert!(snap.is_teleport);
    assert_eq!(snap.last_teleport_seq, snap.seq);
}

#[test]
fn teleport_with_empty_realm_is_rejected() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 3, "0xabc");
    let bytes = client_msg(client_message::Message::Teleport(teleport_request(0, "")));
    assert_eq!(
        srv.dispatch(3, channel::RELIABLE, &bytes, 0, 50),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::InvalidTeleportField
        }
    );
    assert!(
        srv.board.try_read(3).is_none(),
        "rejected teleport publishes nothing"
    );
}

#[test]
fn teleport_with_overlong_realm_is_rejected() {
    let mut srv = PulseServer::new();
    srv.max_realm_length = 4;
    authed(&mut srv, 3, "0xabc");
    let bytes = client_msg(client_message::Message::Teleport(teleport_request(
        0, "abcd",
    )));
    assert_eq!(
        srv.dispatch(3, channel::RELIABLE, &bytes, 0, 50),
        Action::Applied
    );
    let snap = srv.board.try_read(3).unwrap();
    assert_eq!(snap.realm.as_deref(), Some("abcd"));

    let bytes = client_msg(client_message::Message::Teleport(teleport_request(
        0, "abcde",
    )));
    assert_eq!(
        srv.dispatch(3, channel::RELIABLE, &bytes, 0, 60),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::InvalidTeleportField
        }
    );
    let snap = srv.board.try_read(3).unwrap();
    assert_eq!(
        snap.realm.as_deref(),
        Some("abcd"),
        "rejected teleport publishes nothing"
    );
}

#[test]
fn teleport_with_out_of_range_code_is_rejected() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 3, "0xabc");
    let mut req = teleport_request(0, "realm-a");
    req.position_y = 8192;
    let bytes = client_msg(client_message::Message::Teleport(req));
    assert_eq!(
        srv.dispatch(3, channel::RELIABLE, &bytes, 0, 50),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::InvalidTeleportField
        }
    );
    assert!(srv.board.try_read(3).is_none());
}

#[test]
fn teleport_request_caps_boundaries() {
    let encoder = ParcelEncoder::new(ParcelEncoderOptions::default());
    type Set = fn(&mut TeleportRequest, u32);
    let cases: [(&str, u32, Set); 3] = [
        ("position_x", 255, |r, v| r.position_x = v),
        ("position_y", 8191, |r, v| r.position_y = v),
        ("position_z", 255, |r, v| r.position_z = v),
    ];
    for (name, cap, set) in cases {
        let mut req = teleport_request(0, "realm-a");
        set(&mut req, cap);
        assert!(validate::teleport(&req, &encoder), "{name} at cap accepted");
        set(&mut req, cap + 1);
        assert!(
            !validate::teleport(&req, &encoder),
            "{name} above cap rejected"
        );
    }
}

#[test]
fn player_state_caps_boundaries() {
    let encoder = ParcelEncoder::new(ParcelEncoderOptions::default());
    type Set = fn(&mut PlayerState, u32);
    let cases: [(&str, u32, Set); 14] = [
        ("position_x", 255, |s, v| s.position_x = v),
        ("position_y", 8191, |s, v| s.position_y = v),
        ("position_z", 255, |s, v| s.position_z = v),
        ("velocity_x", 255, |s, v| s.velocity_x = v),
        ("velocity_y", 255, |s, v| s.velocity_y = v),
        ("velocity_z", 255, |s, v| s.velocity_z = v),
        ("rotation_y", 127, |s, v| s.rotation_y = v),
        ("movement_blend", 31, |s, v| s.movement_blend = v),
        ("slide_blend", 15, |s, v| s.slide_blend = v),
        ("head_yaw", 127, |s, v| s.head_yaw = Some(v)),
        ("head_pitch", 127, |s, v| s.head_pitch = Some(v)),
        ("point_at_x", 131071, |s, v| s.point_at_x = Some(v)),
        ("point_at_y", 127, |s, v| s.point_at_y = Some(v)),
        ("point_at_z", 131071, |s, v| s.point_at_z = Some(v)),
    ];
    for (name, cap, set) in cases {
        let mut state = valid_state(0);
        set(&mut state, cap);
        assert!(
            validate::player_state(&state, &encoder),
            "{name} at cap accepted"
        );
        set(&mut state, cap + 1);
        assert!(
            !validate::player_state(&state, &encoder),
            "{name} above cap rejected"
        );
    }
}

#[test]
fn resync_request_is_recorded_on_peer_state() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "0xobs");
    let bytes = client_msg(client_message::Message::Resync(ResyncRequest {
        subject_id: 9,
        known_seq: 42,
    }));
    assert_eq!(
        srv.dispatch(1, channel::RELIABLE, &bytes, 0, 0),
        Action::Applied
    );
    let reqs = srv.peers[&1].resync_requests.as_ref().unwrap();
    assert_eq!(reqs.get(&9), Some(&42));
}

#[test]
fn profile_announcement_is_monotonic() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 2, "0xabc");
    let bytes = client_msg(client_message::Message::ProfileAnnouncement(
        ProfileVersionAnnouncement { version: 5 },
    ));
    srv.dispatch(2, channel::RELIABLE, &bytes, 0, 0);
    assert_eq!(srv.profiles.get(2), 5);

    let bytes = client_msg(client_message::Message::ProfileAnnouncement(
        ProfileVersionAnnouncement { version: 3 },
    ));
    srv.dispatch(2, channel::RELIABLE, &bytes, 0, 0);
    assert_eq!(srv.profiles.get(2), 5);
}

#[test]
fn malformed_packet_is_ignored() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "0xabc");
    assert_eq!(
        srv.dispatch(1, 0, &[0xFF, 0xFF, 0xFF], 0, 0),
        Action::Ignore
    );
}

#[test]
fn bad_handshake_replies_with_failure() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let bytes = client_msg(client_message::Message::Handshake(
        crate::protocol::pulse::decentraland::pulse::HandshakeRequest {
            auth_chain: b"not json".to_vec(),
            profile_version: 0,
            initial_state: None,
            protocol_features: 0,
        },
    ));
    match srv.dispatch(1, channel::RELIABLE, &bytes, 1000, 0) {
        Action::Reply(ServerMessage {
            message: Some(server_message::Message::Handshake(h)),
        }) => {
            assert!(!h.success);
            assert!(h.error.is_some());
        }
        other => panic!("unexpected: {other:?}"),
    }
}

async fn signed_handshake_request() -> (Vec<u8>, String, i64) {
    signed_handshake_request_with_expiration("2099-01-01T00:00:00.000Z").await
}

async fn signed_handshake_request_with_expiration(expiration: &str) -> (Vec<u8>, String, i64) {
    use crate::auth::{AuthLink, AuthLinkType};
    use crate::pulse::handshake::build_signed_fetch_payload;
    use alloy::signers::{Signer, local::PrivateKeySigner};

    let root: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        .parse()
        .unwrap();
    let root_addr = format!("{:#x}", root.address());
    let ephemeral: PrivateKeySigner =
        "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
            .parse()
            .unwrap();
    let eph_addr = format!("{:#x}", ephemeral.address());

    let ts = "1700000000000";
    let now_ms: i64 = ts.parse().unwrap();
    let metadata = "{\"signer\":\"dcl:explorer\"}";
    let connect_payload = build_signed_fetch_payload("connect", "/", ts, metadata);

    let eph_payload =
        format!("Decentraland Login\nEphemeral address: {eph_addr}\nExpiration: {expiration}");
    let eph_sig = root
        .sign_message(eph_payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let final_sig = ephemeral
        .sign_message(connect_payload.as_bytes())
        .await
        .unwrap()
        .to_string();

    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: root_addr.clone(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaEphemeral,
            payload: eph_payload,
            signature: Some(eph_sig),
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload: connect_payload,
            signature: Some(final_sig),
        },
    ];

    let mut map = serde_json::Map::new();
    for (i, link) in chain.iter().enumerate() {
        map.insert(
            format!("x-identity-auth-chain-{i}"),
            serde_json::Value::String(serde_json::to_string(link).unwrap()),
        );
    }
    map.insert(
        "x-identity-timestamp".into(),
        serde_json::Value::String(ts.into()),
    );
    map.insert(
        "x-identity-metadata".into(),
        serde_json::Value::String(metadata.into()),
    );
    let bag = serde_json::to_string(&serde_json::Value::Object(map)).unwrap();

    let bytes = client_msg(client_message::Message::Handshake(
        crate::protocol::pulse::decentraland::pulse::HandshakeRequest {
            auth_chain: bag.into_bytes(),
            profile_version: 0,
            initial_state: None,
            protocol_features: 0,
        },
    ));
    (bytes, root_addr.to_lowercase(), now_ms)
}

fn with_initial_state(bytes: &[u8], init: Option<PlayerInitialState>) -> Vec<u8> {
    let mut msg = ClientMessage::decode(bytes).unwrap();
    if let Some(client_message::Message::Handshake(h)) = msg.message.as_mut() {
        h.initial_state = init;
    }
    msg.encode_to_vec()
}

#[tokio::test]
async fn handshake_seeds_valid_initial_state() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (base, wallet, now_ms) = signed_handshake_request().await;

    let init = PlayerInitialState {
        state: Some(valid_state(7)),
        realm: "realm-a".into(),
        ..Default::default()
    };
    let bytes = with_initial_state(&base, Some(init));

    let action = srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0);
    match &action {
        Action::Authenticated {
            wallet: w,
            initial_state,
            ..
        } => {
            assert_eq!(*w, wallet);
            assert!(
                initial_state.is_some(),
                "validated initial state carried on the action"
            );
        }
        other => panic!("expected Authenticated, got {other:?}"),
    }

    srv.peers.get_mut(&1).unwrap().connection_state = PeerConnectionState::Authenticated;
    srv.identity.set(1, wallet.clone());
    srv.board.set_active(1);
    if let Action::Authenticated {
        initial_state: Some(init),
        ..
    } = action
    {
        srv.seed_initial_state(1, 500, &init);
    }
    let snap = srv.board.try_read(1).expect("seeded snapshot present");
    assert_eq!(snap.parcel, 7);
    assert_eq!(snap.server_tick, 500);
}

#[tokio::test]
async fn handshake_with_malformed_initial_state_is_rejected() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (base, _wallet, now_ms) = signed_handshake_request().await;

    let mut bad = valid_state(7);
    bad.position_y = 8192;
    let init = PlayerInitialState {
        state: Some(bad),
        realm: "realm-a".into(),
        ..Default::default()
    };
    let bytes = with_initial_state(&base, Some(init));

    match srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0) {
        Action::Reject { reply, reason } => {
            assert_eq!(reason, DisconnectReason::InvalidHandshakeField);
            match reply.and_then(|m| m.message) {
                Some(server_message::Message::Handshake(h)) => {
                    assert!(!h.success, "malformed initial state must not authenticate");
                    assert!(h.error.is_some());
                }
                other => panic!("expected handshake failure reply, got {other:?}"),
            }
        }
        other => panic!("expected Reject(InvalidHandshakeField), got {other:?}"),
    }
    assert!(!srv.is_authenticated(1));
}

#[test]
fn input_with_out_of_range_code_is_rejected() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 7, "0xabc");
    let mut state = valid_state(3);
    state.rotation_y = 128;
    let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
        state: Some(state),
    }));
    assert_eq!(
        srv.dispatch(7, channel::UNRELIABLE_SEQUENCED, &bytes, 0, 100),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::InvalidInputField
        }
    );
    assert!(
        srv.board.try_read(7).is_none(),
        "rejected input publishes nothing"
    );
}

#[tokio::test]
async fn baseline_legacy_auth_works_without_v4() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (bytes, wallet, now_ms) = signed_handshake_request().await;

    match srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0) {
        Action::Authenticated {
            wallet: w,
            session,
            duplicate_of,
            initial_state,
            features,
        } => {
            assert_ne!(
                session, w,
                "a delegated chain names its ephemeral as the session"
            );
            assert_eq!(w, wallet);
            assert_eq!(duplicate_of, None);
            assert_eq!(features, 0, "nothing offered negotiates the baseline");
            assert!(
                initial_state.is_none(),
                "no initial state in this handshake"
            );
        }
        other => panic!("expected Authenticated, got {other:?}"),
    }

    srv.peers.get_mut(&1).unwrap().wallet_id = Some(wallet.clone());
    srv.peers.get_mut(&1).unwrap().connection_state = PeerConnectionState::Authenticated;
    srv.identity.set(1, wallet.clone());
    srv.board.set_active(1);
    assert!(srv.is_authenticated(1));
    assert_eq!(srv.identity.peer_by_wallet(&wallet), Some(1));
}

#[tokio::test]
async fn baseline_v4_authentication_is_not_exposed() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (legacy, _, now_ms) = signed_handshake_request().await;
    for arm in [20_u8, 21] {
        let wire = [(arm << 3) | 2, 1, 0];
        for bytes in [wire.to_vec(), [legacy.as_slice(), wire.as_slice()].concat()] {
            assert!(matches!(
                srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0),
                Action::Reject {
                    reason: DisconnectReason::InvalidHandshakeField,
                    ..
                }
            ));
            assert!(!srv.is_authenticated(1));
            assert!(srv.peers[&1].handshake_fingerprint.is_none());
        }
    }
    assert_eq!(
        srv.peers[&1].connection_state,
        PeerConnectionState::PendingDisconnect
    );
    // Rejection is terminal for that connection; legacy admission remains
    // available to a fresh connection rather than reviving the rejected peer.
    srv.peers
        .insert(2, PeerState::new(PeerConnectionState::PendingAuth, 0));
    assert!(matches!(
        srv.dispatch(2, channel::RELIABLE, &legacy, now_ms, 0),
        Action::Authenticated { features: 0, .. }
    ));
    assert_eq!(SERVER_FEATURES & FEATURE_APPLICATION_RELAY, 0);
    assert!(srv.application_relay.is_none());
}

#[tokio::test]
async fn unknown_groups_preserve_top_level_auth_field_checks() {
    let (legacy, _, now_ms) = signed_handshake_request().await;
    let group = [0xa3, 0x06, 0xa4, 0x06];
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    for arm in [20_u8, 21] {
        let reserved = [(arm << 3) | 2, 1, 0];
        for bytes in [
            [group.as_slice(), reserved.as_slice(), legacy.as_slice()].concat(),
            [group.as_slice(), legacy.as_slice(), reserved.as_slice()].concat(),
        ] {
            assert!(ClientMessage::decode(bytes.as_slice()).is_ok());
            assert!(matches!(
                srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0),
                Action::Reject {
                    reason: DisconnectReason::InvalidHandshakeField,
                    ..
                }
            ));
            assert!(srv.peers[&1].handshake_fingerprint.is_none());
        }
    }
    for prefix in [
        group.to_vec(),
        vec![0xa3, 0x06, 0xa2, 0x01, 0, 0xa4, 0x06],
        vec![0xa3, 0x06, 0xaa, 0x01, 0, 0xa4, 0x06],
    ] {
        let bytes = [prefix, legacy.clone()].concat();
        let mut srv = PulseServer::new();
        srv.peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        assert!(
            matches!(
                srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0),
                Action::Authenticated { features: 0, .. }
            ),
            "unknown groups must not change ordinary legacy admission; nested field numbers are not top-level arms"
        );
    }
    for prefix in [
        vec![0xa3, 0x06],
        vec![0xa3, 0x06, 0xa4, 0x07],
        vec![0xa4, 0x06],
    ] {
        let bytes = [prefix, legacy.clone()].concat();
        let mut srv = PulseServer::new();
        srv.peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        assert!(ClientMessage::decode(bytes.as_slice()).is_err());
        assert_eq!(
            srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0),
            Action::Ignore,
            "malformed unknown groups stay on the ordinary decode-failure path"
        );
    }
}

#[tokio::test]
async fn handshake_negotiates_features_masking_unknown_bits() {
    use crate::pulse::server::{FEATURE_DELTA_BATCH, SERVER_FEATURES};

    async fn negotiate(offered: u32) -> u32 {
        let mut srv = PulseServer::new();
        srv.peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        let (base, _wallet, now_ms) = signed_handshake_request().await;
        let mut msg = ClientMessage::decode(&base[..]).unwrap();
        if let Some(client_message::Message::Handshake(h)) = msg.message.as_mut() {
            h.protocol_features = offered;
        }
        match srv.dispatch(1, channel::RELIABLE, &msg.encode_to_vec(), now_ms, 0) {
            Action::Authenticated { features, .. } => features,
            other => panic!("expected Authenticated, got {other:?}"),
        }
    }

    assert_eq!(negotiate(FEATURE_DELTA_BATCH).await, FEATURE_DELTA_BATCH);
    assert_eq!(
        negotiate(crate::pulse::server::FEATURE_DELTA_BATCH_BASELINE).await,
        crate::pulse::server::FEATURE_DELTA_BATCH_BASELINE
    );
    assert_eq!(
        negotiate(crate::pulse::server::FEATURE_DELTA_BATCH_DICTIONARY).await,
        crate::pulse::server::FEATURE_DELTA_BATCH_DICTIONARY
    );
    assert_eq!(negotiate(0).await, 0);
    assert_eq!(negotiate(0xFFFF_FFFF).await, SERVER_FEATURES);
    assert_eq!(
        negotiate(FEATURE_DELTA_BATCH | 1 << 31).await,
        FEATURE_DELTA_BATCH,
        "unknown bits are masked off, the peer still authenticates"
    );
    let Some(server_message::Message::Handshake(resp)) = handshake_response(true, None).message
    else {
        panic!("handshake_response must wrap a HandshakeResponse");
    };
    assert_eq!(resp.protocol_features, SERVER_FEATURES);
}

#[tokio::test]
async fn sample_ticks_are_granted_only_beside_an_arm_11_codec() {
    async fn negotiate(offered: u32) -> u32 {
        let mut srv = PulseServer::new();
        srv.peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        let (base, _wallet, now_ms) = signed_handshake_request().await;
        let mut msg = ClientMessage::decode(&base[..]).unwrap();
        if let Some(client_message::Message::Handshake(h)) = msg.message.as_mut() {
            h.protocol_features = offered;
        }
        match srv.dispatch(1, channel::RELIABLE, &msg.encode_to_vec(), now_ms, 0) {
            Action::Authenticated { features, .. } => features,
            other => panic!("expected Authenticated, got {other:?}"),
        }
    }

    assert_eq!(FEATURE_DELTA_BATCH_SAMPLE_TICK, 1 << 4);
    assert_eq!(negotiate(FEATURE_DELTA_BATCH_SAMPLE_TICK).await, 0);
    assert_eq!(
        negotiate(FEATURE_DELTA_BATCH | FEATURE_DELTA_BATCH_SAMPLE_TICK).await,
        FEATURE_DELTA_BATCH,
        "arm 10 has no sample ticks to carry"
    );
    for codec in [FEATURE_DELTA_BATCH_BASELINE, FEATURE_DELTA_BATCH_DICTIONARY] {
        assert_eq!(
            negotiate(codec | FEATURE_DELTA_BATCH_SAMPLE_TICK).await,
            codec | FEATURE_DELTA_BATCH_SAMPLE_TICK
        );
        assert_eq!(negotiate(codec).await, codec);
    }
}

#[tokio::test]
async fn tampered_handshake_replies_with_failure() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(2, PeerState::new(PeerConnectionState::PendingAuth, 0));

    let (bytes, _wallet, now_ms) = signed_handshake_request().await;
    let mut req = ClientMessage::decode(&bytes[..]).unwrap();
    if let Some(client_message::Message::Handshake(h)) = req.message.as_mut() {
        let mut bag: serde_json::Value = serde_json::from_slice(&h.auth_chain).unwrap();
        let link_json = bag["x-identity-auth-chain-2"].as_str().unwrap();
        let mut link: serde_json::Value = serde_json::from_str(link_json).unwrap();
        link["payload"] = serde_json::Value::String("connect:/:1700000000000:tampered".into());
        bag["x-identity-auth-chain-2"] =
            serde_json::Value::String(serde_json::to_string(&link).unwrap());
        h.auth_chain = serde_json::to_vec(&bag).unwrap();
    }
    let tampered = req.encode_to_vec();

    match srv.dispatch(2, channel::RELIABLE, &tampered, now_ms, 0) {
        Action::Reply(ServerMessage {
            message: Some(server_message::Message::Handshake(h)),
        }) => {
            assert!(!h.success, "tampered chain must not succeed");
            assert!(h.error.is_some());
        }
        other => panic!("expected failure Reply, got {other:?}"),
    }
    assert!(!srv.is_authenticated(2));
}

#[tokio::test]
async fn banned_wallet_is_rejected_at_handshake() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (bytes, wallet, now_ms) = signed_handshake_request().await;

    srv.ban_list.replace([wallet.clone()]);

    match srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0) {
        Action::Reject { reply, reason } => {
            assert_eq!(reason, DisconnectReason::Banned);

            match reply.and_then(|m| m.message) {
                Some(server_message::Message::Handshake(h)) => {
                    assert!(!h.success);
                    assert_eq!(h.error.as_deref(), Some("banned"));
                }
                other => panic!("expected banned handshake reply, got {other:?}"),
            }
        }
        other => panic!("expected Reject(Banned), got {other:?}"),
    }
    assert!(!srv.is_authenticated(1));

    assert_eq!(
        srv.peers[&1].connection_state,
        PeerConnectionState::PendingDisconnect
    );
}

#[tokio::test]
async fn replayed_handshake_pair_is_rejected() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    srv.peers
        .insert(2, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (bytes, wallet, now_ms) = signed_handshake_request().await;

    match srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 100) {
        Action::Authenticated { wallet: w, .. } => assert_eq!(w, wallet),
        other => panic!("first handshake should authenticate, got {other:?}"),
    }

    match srv.dispatch(2, channel::RELIABLE, &bytes, now_ms, 200) {
        Action::Reject { reply, reason } => {
            assert_eq!(reason, DisconnectReason::HandshakeReplayRejected);
            assert!(
                reply.is_none(),
                "replay rejection has no reply body (PeerDefense)"
            );
        }
        other => panic!("expected Reject(HandshakeReplayRejected), got {other:?}"),
    }
    assert!(!srv.is_authenticated(2));
}

#[tokio::test]
async fn handshake_attempts_are_throttled() {
    let mut srv = PulseServer::new();

    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let bad = client_msg(client_message::Message::Handshake(
        crate::protocol::pulse::decentraland::pulse::HandshakeRequest {
            auth_chain: b"not json".to_vec(),
            profile_version: 0,
            initial_state: None,
            protocol_features: 0,
        },
    ));

    for _ in 0..2 {
        assert!(matches!(
            srv.dispatch(1, channel::RELIABLE, &bad, 1000, 0),
            Action::Reply(_)
        ));
    }
    assert_eq!(srv.peers[&1].handshake_attempts, 2);

    match srv.dispatch(1, channel::RELIABLE, &bad, 1000, 0) {
        Action::Reject { reply, reason } => {
            assert_eq!(reason, DisconnectReason::AuthFailed);
            assert!(reply.is_none());
        }
        other => panic!("expected Reject(AuthFailed) on throttle, got {other:?}"),
    }
    assert_eq!(
        srv.peers[&1].connection_state,
        PeerConnectionState::PendingDisconnect
    );
}

#[tokio::test]
async fn duplicate_committed_handshake_replies_without_reapplying_admission() {
    let mut server = PulseServer::with_config(4, 2, &[50], false);
    server
        .peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (bytes, wallet, now_ms) = signed_handshake_request().await;
    let session = match server.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0) {
        Action::Authenticated { session, .. } => session,
        other => panic!("expected admission: {other:?}"),
    };
    server.peers.get_mut(&1).unwrap().connection_state = PeerConnectionState::Authenticated;
    server.peers.get_mut(&1).unwrap().wallet_id = Some(wallet.clone());
    server.identity.set_with_session(1, wallet.clone(), session);
    for delta in [7_000, 31_000, 120_001] {
        assert_eq!(
            server.dispatch(1, channel::RELIABLE, &bytes, now_ms + delta, delta as u32),
            Action::Reply(handshake_response(true, None))
        );
    }
    assert_eq!(server.peers[&1].handshake_attempts, 1);
    assert_eq!(server.identity.peer_by_wallet(&wallet), Some(1));
    let mut different = ClientMessage::decode(bytes.as_slice()).unwrap();
    if let Some(client_message::Message::Handshake(request)) = different.message.as_mut() {
        request.profile_version += 1;
    }
    assert_eq!(
        server.dispatch(1, channel::RELIABLE, &different.encode_to_vec(), now_ms, 0),
        Action::Ignore
    );
    server.peers.get_mut(&1).unwrap().connection_state = PeerConnectionState::PendingDisconnect;
    assert_eq!(
        server.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0),
        Action::Ignore
    );
}

#[tokio::test]
async fn future_dated_signature_is_consumed_before_reply_and_across_restart() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "catalyrst-pulse-server-future-restart-{}-{id}.tsv",
        std::process::id()
    ));
    let (bytes, _, timestamp) = signed_handshake_request().await;
    let earliest = timestamp - crate::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;

    {
        let mut server = PulseServer::with_config(4, 2, &[50], false);
        server.replay_policy = HandshakeReplayPolicy::durable(
            true,
            crate::pulse::handshake::MAX_TIMESTAMP_SKEW_MS,
            4,
            &path,
            earliest,
        )
        .unwrap();
        server
            .peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        assert!(matches!(
            server.dispatch(1, channel::RELIABLE, &bytes, earliest, 0),
            Action::Reply(ServerMessage {
                message: Some(server_message::Message::Handshake(HandshakeResponse {
                    success: false,
                    ..
                }))
            })
        ));
    }

    let mut restarted = PulseServer::with_config(4, 2, &[50], false);
    restarted.replay_policy = HandshakeReplayPolicy::durable_after_restart(
        true,
        crate::pulse::handshake::MAX_TIMESTAMP_SKEW_MS,
        4,
        &path,
        earliest + 1,
    )
    .unwrap();
    restarted.replay_policy.forget_before(earliest + 1);
    restarted
        .peers
        .insert(2, PeerState::new(PeerConnectionState::PendingAuth, 0));
    assert!(matches!(
        restarted.dispatch(2, channel::RELIABLE, &bytes, timestamp, 0),
        Action::Reject {
            reason: DisconnectReason::HandshakeReplayRejected,
            ..
        }
    ));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("replay-journal.tmp"));
}

#[tokio::test]
async fn a_stepped_back_wall_clock_cannot_reopen_a_used_handshake() {
    let (bytes, _, timestamp) = signed_handshake_request().await;
    let skew = crate::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;
    let mut server = PulseServer::with_config(4, 2, &[50], false);
    for peer in [1, 2] {
        server
            .peers
            .insert(peer, PeerState::new(PeerConnectionState::PendingAuth, 0));
    }
    assert!(matches!(
        server.dispatch(1, channel::RELIABLE, &bytes, timestamp + skew, 0),
        Action::Authenticated { .. }
    ));
    let elapsed = (2 * skew + 2) as u32;
    assert!(
        !matches!(
            server.dispatch(2, channel::RELIABLE, &bytes, timestamp, elapsed),
            Action::Authenticated { .. }
        ),
        "a used handshake was admitted again after the wall clock stepped back"
    );
    assert!(!server.is_authenticated(2));
}

#[tokio::test]
async fn a_handshake_signed_before_this_process_started_is_refused_with_a_retryable_reply() {
    let (bytes, _, timestamp) = signed_handshake_request().await;
    for (started_at, admitted) in [(timestamp + 1, false), (timestamp, true)] {
        let mut server = PulseServer::with_config(4, 2, &[50], false);
        server.replay_policy.forget_before(started_at);
        server
            .peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        match server.dispatch(1, channel::RELIABLE, &bytes, timestamp + 1_000, 0) {
            Action::Authenticated { .. } => assert!(admitted),
            Action::Reply(message) => {
                assert!(!admitted);
                match message.message {
                    Some(server_message::Message::Handshake(h)) => assert!(!h.success),
                    other => panic!("expected a failed handshake reply, got {other:?}"),
                }
                assert_eq!(
                    server.peers[&1].connection_state,
                    PeerConnectionState::PendingAuth,
                    "the peer must be able to sign a new handshake"
                );
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }
}

#[tokio::test]
async fn replay_memory_exhaustion_is_transient_capacity_without_eviction() {
    let (bytes, _, timestamp) = signed_handshake_request().await;
    let mut server = PulseServer::with_config(4, 2, &[50], false);
    server.replay_policy =
        HandshakeReplayPolicy::new(true, crate::pulse::handshake::MAX_TIMESTAMP_SKEW_MS, 1);
    let other = timestamp.to_string();
    assert_eq!(
        server
            .replay_policy
            .try_admit(timestamp, "other-wallet", &other),
        Ok(())
    );
    server
        .peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    assert!(matches!(
        server.dispatch(1, channel::RELIABLE, &bytes, timestamp, 1),
        Action::Reject {
            reason: DisconnectReason::ServerFull,
            ..
        }
    ));
    assert_eq!(
        server
            .replay_policy
            .try_admit(timestamp + 2, "other-wallet", &other),
        Err(ReplayRejection::Duplicate)
    );
}

#[test]
fn excessive_emote_duration_is_rejected() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 7, "0xabc");
    let bytes = client_msg(client_message::Message::EmoteStart(
        crate::protocol::pulse::decentraland::pulse::EmoteStart {
            emote_id: "wave".into(),
            duration_ms: Some(srv.max_emote_duration_ms + 1),
            player_state: Some(valid_state(3)),
            mask: None,
        },
    ));
    match srv.dispatch(7, channel::UNRELIABLE_UNSEQUENCED, &bytes, 0, 0) {
        Action::Reject { reason, .. } => {
            assert_eq!(reason, DisconnectReason::InvalidEmoteField)
        }
        other => panic!("expected Reject(InvalidEmoteField), got {other:?}"),
    }
}

#[test]
fn emote_within_caps_is_applied() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 7, "0xabc");
    let bytes = client_msg(client_message::Message::EmoteStart(
        crate::protocol::pulse::decentraland::pulse::EmoteStart {
            emote_id: "urn:decentraland:off-chain:base-emotes:wave".into(),
            duration_ms: Some(2000),
            player_state: Some(valid_state(3)),
            mask: None,
        },
    ));
    assert_eq!(
        srv.dispatch(7, channel::UNRELIABLE_UNSEQUENCED, &bytes, 0, 50),
        Action::Applied
    );
    assert!(srv.board.try_read(7).unwrap().is_emoting());
}

#[test]
fn a_one_shot_emote_completes_server_side() {
    let mut srv = PulseServer::new();
    let enter = |srv: &mut PulseServer, peer: u32, wallet: &str, now: u32| {
        authed(srv, peer, wallet);
        let bytes = client_msg(client_message::Message::Teleport(teleport_request(
            0, "realm-a",
        )));
        assert_eq!(
            srv.dispatch(peer, channel::RELIABLE, &bytes, 0, now),
            Action::Applied
        );
    };
    enter(&mut srv, 0, "0xobserver", 900);
    enter(&mut srv, 1, "0xsubject", 900);
    srv.simulate(950);

    let bytes = client_msg(client_message::Message::EmoteStart(
        crate::protocol::pulse::decentraland::pulse::EmoteStart {
            emote_id: "wave".into(),
            duration_ms: Some(2000),
            player_state: Some(valid_state(0)),
            mask: None,
        },
    ));
    assert_eq!(
        srv.dispatch(1, channel::UNRELIABLE_UNSEQUENCED, &bytes, 0, 1000),
        Action::Applied
    );

    let started = |out: &[OutgoingMessage], target: u32| {
        out.iter()
            .filter(|m| {
                m.target == target
                    && matches!(&m.message.message,
                        Some(server_message::Message::EmoteStarted(e)) if e.subject_id == 1)
            })
            .count()
    };
    let stopped = |out: &[OutgoingMessage], target: u32| -> Vec<i32> {
        out.iter()
            .filter(|m| m.target == target)
            .filter_map(|m| match &m.message.message {
                Some(server_message::Message::EmoteStopped(e)) if e.subject_id == 1 => {
                    Some(e.reason)
                }
                _ => None,
            })
            .collect()
    };

    assert_eq!(started(&srv.simulate(1050), 0), 1);
    assert!(
        stopped(&srv.simulate(2999), 0).is_empty(),
        "one millisecond short of its duration the emote still plays"
    );

    let mut reasons = Vec::new();
    for now in [3000, 3050, 3100] {
        reasons.extend(stopped(&srv.simulate(now), 0));
    }
    assert_eq!(
        reasons,
        vec![crate::protocol::pulse::decentraland::pulse::EmoteStopReason::Completed as i32],
        "an elapsed one-shot emote is stopped once, as completed"
    );
    assert!(!srv.board.is_emoting(1));

    enter(&mut srv, 2, "0xlate", 3120);
    let out = srv.simulate(3150);
    assert!(
        out.iter().any(|m| m.target == 2
            && matches!(&m.message.message,
                Some(server_message::Message::PlayerJoined(j)) if j.user_id == "0xsubject")),
        "the late observer does see the subject"
    );
    assert_eq!(
        started(&out, 2),
        0,
        "a late observer is not shown an emote that already finished"
    );
}

#[test]
fn a_looping_emote_is_never_completed_server_side() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "0xsubject");
    let bytes = client_msg(client_message::Message::EmoteStart(
        crate::protocol::pulse::decentraland::pulse::EmoteStart {
            emote_id: "dance".into(),
            duration_ms: None,
            player_state: Some(valid_state(0)),
            mask: None,
        },
    ));
    assert_eq!(
        srv.dispatch(1, channel::UNRELIABLE_UNSEQUENCED, &bytes, 0, 1000),
        Action::Applied
    );
    srv.simulate(u32::MAX);
    assert!(srv.board.is_emoting(1), "no duration means no server stop");
}

#[test]
fn emote_start_mask_is_published_and_absence_stays_absent() {
    let masked = |mask: Option<i32>| {
        let mut srv = PulseServer::new();
        authed(&mut srv, 7, "0xabc");
        let bytes = client_msg(client_message::Message::EmoteStart(
            crate::protocol::pulse::decentraland::pulse::EmoteStart {
                emote_id: "wave".into(),
                duration_ms: None,
                player_state: Some(valid_state(3)),
                mask,
            },
        ));
        assert_eq!(
            srv.dispatch(7, channel::UNRELIABLE_UNSEQUENCED, &bytes, 0, 50),
            Action::Applied
        );
        srv.board.try_read(7).unwrap().emote.clone().unwrap().mask
    };
    assert_eq!(masked(Some(5)), Some(5), "the wire mask reaches the ledger");
    assert_eq!(
        masked(None),
        None,
        "no mask on the wire means no mask stored"
    );
}

#[test]
fn handshake_initial_state_carries_the_emote_mask() {
    let seeded = |emote_mask: Option<i32>| {
        let mut srv = PulseServer::new();
        srv.board.set_active(1);
        let init = PlayerInitialState {
            state: Some(valid_state(7)),
            emote_id: Some("wave".into()),
            realm: "realm-a".into(),
            emote_mask,
            ..Default::default()
        };
        srv.seed_initial_state(1, 500, &init);
        srv.board.try_read(1).unwrap().emote.clone().unwrap().mask
    };
    assert_eq!(seeded(Some(5)), Some(5), "the seeded emote keeps its mask");
    assert_eq!(seeded(None), None, "a maskless seed stays maskless");
}

#[test]
fn handshake_initial_state_lands_the_peer_in_its_realm() {
    let mut srv = PulseServer::new();
    srv.board.set_active(1);
    let init = PlayerInitialState {
        state: Some(valid_state(7)),
        realm: "realm-a".into(),
        ..Default::default()
    };
    srv.seed_initial_state(1, 500, &init);
    assert_eq!(
        srv.board.try_read(1).unwrap().realm.as_deref(),
        Some("realm-a"),
        "the seeded snapshot carries the asserted realm"
    );
    assert_eq!(
        srv.grids.realm_of(1),
        Some("realm-a"),
        "so the peer is visible without a follow-up teleport"
    );
}

#[tokio::test]
async fn handshake_initial_state_rejects_an_empty_realm() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (base, _wallet, now_ms) = signed_handshake_request().await;

    let init = PlayerInitialState {
        state: Some(valid_state(7)),
        ..Default::default()
    };
    let bytes = with_initial_state(&base, Some(init));

    match srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0) {
        Action::Reject { reason, .. } => {
            assert_eq!(reason, DisconnectReason::InvalidHandshakeField);
        }
        other => panic!("expected Reject(InvalidHandshakeField), got {other:?}"),
    }
    assert!(!srv.is_authenticated(1));
}

#[tokio::test]
async fn handshake_initial_state_emote_duration_cap_is_enforced() {
    let mut srv = PulseServer::new();
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    let (base, _wallet, now_ms) = signed_handshake_request().await;

    let init = PlayerInitialState {
        state: Some(valid_state(7)),
        emote_id: Some("u".repeat(513)),
        emote_duration_ms: Some(60001),
        realm: "realm-a".into(),
        ..Default::default()
    };
    let bytes = with_initial_state(&base, Some(init));

    match srv.dispatch(1, channel::RELIABLE, &bytes, now_ms, 0) {
        Action::Reject { reason, .. } => {
            assert_eq!(reason, DisconnectReason::InvalidHandshakeField);
        }
        other => panic!("expected Reject(InvalidHandshakeField), got {other:?}"),
    }
    assert!(!srv.is_authenticated(1));
}

#[tokio::test]
async fn handshake_initial_state_rejects_overlong_realm() {
    let (base, _wallet, now_ms) = signed_handshake_request().await;
    let init_with_realm = |realm: &str| {
        with_initial_state(
            &base,
            Some(PlayerInitialState {
                state: Some(valid_state(7)),
                realm: realm.into(),
                ..Default::default()
            }),
        )
    };

    let mut srv = PulseServer::new();
    srv.max_realm_length = 4;
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    match srv.dispatch(1, channel::RELIABLE, &init_with_realm("abcd"), now_ms, 0) {
        Action::Authenticated { .. } => {}
        other => panic!("expected Authenticated, got {other:?}"),
    }

    let mut srv = PulseServer::new();
    srv.max_realm_length = 4;
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    match srv.dispatch(1, channel::RELIABLE, &init_with_realm("abcde"), now_ms, 0) {
        Action::Reject { reason, .. } => {
            assert_eq!(reason, DisconnectReason::InvalidHandshakeField);
        }
        other => panic!("expected Reject(InvalidHandshakeField), got {other:?}"),
    }
    assert!(!srv.is_authenticated(1));
}

#[test]
fn wt_flood_at_budget_does_not_starve_enet_admission() {
    use crate::pulse::hardening::AdmitResult;
    let mut srv = PulseServer::new();
    let wt_base = ENET_CAPACITY as u32;

    for i in 0..DEFAULT_PRE_AUTH_BUDGET_WT as u32 {
        let peer = wt_base + i;
        let ip = format!("10.{}.{}.1", i / 256, i % 256);
        assert_eq!(srv.pre_auth_for(peer).try_admit(peer, &ip), AdmitResult::Ok);
    }
    assert_eq!(srv.pre_auth_wt.in_flight(), DEFAULT_PRE_AUTH_BUDGET_WT);

    let extra = wt_base + DEFAULT_PRE_AUTH_BUDGET_WT as u32;
    assert_eq!(
        srv.pre_auth_for(extra).try_admit(extra, "192.0.2.254"),
        AdmitResult::BudgetExhausted
    );

    assert_eq!(srv.pre_auth_for(7).try_admit(7, "1.2.3.4"), AdmitResult::Ok);
    assert_eq!(srv.pre_auth_enet.in_flight(), 1);
}

#[test]
fn pre_auth_release_credits_the_admitting_transport() {
    use crate::pulse::hardening::AdmitResult;
    let mut srv = PulseServer::new();
    let enet = 3u32;
    let wt = ENET_CAPACITY as u32 + 1;

    assert_eq!(
        srv.pre_auth_for(enet).try_admit(enet, "1.1.1.1"),
        AdmitResult::Ok
    );
    assert_eq!(
        srv.pre_auth_for(wt).try_admit(wt, "2.2.2.2"),
        AdmitResult::Ok
    );
    assert_eq!(srv.pre_auth_enet.in_flight(), 1);
    assert_eq!(srv.pre_auth_wt.in_flight(), 1);

    srv.pre_auth_for(wt).release_on_disconnect(wt);
    assert_eq!(srv.pre_auth_wt.in_flight(), 0);
    assert_eq!(
        srv.pre_auth_enet.in_flight(),
        1,
        "enet budget untouched by a WT release"
    );

    srv.pre_auth_for(enet).release_on_promotion(enet);
    assert_eq!(srv.pre_auth_enet.in_flight(), 0);
}

mod scene_listener;

#[tokio::test]
async fn bans_snapshot_evicts_all_matching_peers_and_retains_other_players() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "0xABC");
    authed(&mut srv, 2, "0xabc");
    authed(&mut srv, 3, "0xdef");
    let host = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: 8,
        channel_limit: 8,
    })
    .await
    .unwrap();
    let mut transports = Transports::enet_only(host, 8);
    srv.apply_bans(&mut transports, vec!["abc".into()])
        .await
        .unwrap();
    assert!(!srv.peers.contains_key(&1));
    assert!(!srv.peers.contains_key(&2));
    assert!(srv.peers.contains_key(&3));
    assert_eq!(srv.identity.wallet_by_peer(1), None);
    assert_eq!(srv.identity.wallet_by_peer(2), None);
    assert!(srv.ban_list.is_banned("0xAbC"));
    srv.apply_bans(&mut transports, vec![]).await.unwrap();
    assert!(!srv.ban_list.is_banned("0xabc"));
    assert!(srv.peers.contains_key(&3));
}

#[test]
fn baseline_observer_round_trip_reseeds_once_after_history_eviction() {
    let mut srv = PulseServer::new();
    let teleport = |srv: &mut PulseServer, peer, realm, now| {
        let bytes = client_msg(client_message::Message::Teleport(teleport_request(
            0, realm,
        )));
        assert_eq!(
            srv.dispatch(peer, channel::RELIABLE, &bytes, 0, now),
            Action::Applied
        );
    };
    for (id, wallet) in [(0, "observer"), (1, "subject"), (2, "unrelated")] {
        authed(&mut srv, id, wallet);
        teleport(&mut srv, id, "a", 100);
    }
    srv.simulate(150);
    teleport(&mut srv, 0, "b", 200);
    teleport(&mut srv, 0, "a", 250);
    // Movement evicts every teleport without clearing the pending invalidation.
    for now in 300..330 {
        let mut state = valid_state(0);
        state.position_x = now - 300;
        let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
            state: Some(state),
        }));
        srv.dispatch(
            0,
            channel::UNRELIABLE_SEQUENCED,
            &bytes,
            0,
            (now - 300) * 100 + 300,
        );
    }
    teleport(&mut srv, 0, "a", 6100);
    let out = srv.simulate(6150);
    let events: Vec<_> = out
        .iter()
        .filter(|m| m.target == 0)
        .filter_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerLeft(l)) => Some(("left", l.subject_id)),
            Some(server_message::Message::PlayerJoined(j)) => {
                Some(("joined", j.state.as_ref().unwrap().subject_id))
            }
            _ => None,
        })
        .collect();
    assert_eq!(events.iter().filter(|(kind, _)| *kind == "left").count(), 2);
    assert_eq!(
        events.iter().filter(|(kind, _)| *kind == "joined").count(),
        2
    );
    assert!(events[..2].iter().all(|(kind, _)| *kind == "left"));
    assert!(!out.iter().any(|m| m.target == 2
        && matches!(
            m.message.message,
            Some(server_message::Message::PlayerLeft(_))
        )));
    assert!(!srv.simulate(6200).iter().any(|m| m.target == 0
        && matches!(
            m.message.message,
            Some(server_message::Message::PlayerLeft(_))
                | Some(server_message::Message::PlayerJoined(_))
        )));
}

#[test]
fn baseline_cleanup_and_slot_reuse_discards_pending_realm_invalidation() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 0, "old");
    authed(&mut srv, 1, "subject");
    for id in [0, 1] {
        let bytes = client_msg(client_message::Message::Teleport(teleport_request(0, "a")));
        srv.dispatch(id, channel::RELIABLE, &bytes, 0, 100);
    }
    srv.simulate(150);
    let bytes = client_msg(client_message::Message::Teleport(teleport_request(0, "b")));
    srv.dispatch(0, channel::RELIABLE, &bytes, 0, 200);
    assert!(srv.peers[&0].observer_views_invalidated);
    srv.remove_peer_state(0);
    authed(&mut srv, 0, "new");
    assert!(!srv.peers[&0].observer_views_invalidated);
    let bytes = client_msg(client_message::Message::Teleport(teleport_request(0, "a")));
    srv.dispatch(0, channel::RELIABLE, &bytes, 0, 250);
    let out = srv.simulate(300);
    assert!(!out.iter().any(|m| m.target == 0
        && matches!(
            m.message.message,
            Some(server_message::Message::PlayerLeft(_))
        )));
    assert!(out.iter().any(|m| m.target == 0 && matches!(&m.message.message, Some(server_message::Message::PlayerJoined(j)) if j.user_id == "subject")));
}

#[test]
fn baseline_input_boundaries_duplicate_history() {
    assert_eq!(DEFAULT_RING_CAPACITY, 20);
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "wallet");
    for seq in 0..21 {
        let mut state = valid_state(0);
        state.position_x = seq;
        let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
            state: Some(state),
        }));
        srv.dispatch(1, 0, &bytes, 0, seq * 100);
        let before = srv.board.try_read(1).unwrap().clone();
        srv.dispatch(1, 0, &bytes, 0, seq * 100 + 50);
        assert_eq!(srv.board.try_read(1), Some(&before));
        if seq == 19 {
            assert!(srv.board.try_read_seq(1, 0).is_some());
        }
    }
    assert!(srv.board.try_read_seq(1, 0).is_none());
    assert!(srv.board.try_read_seq(1, 1).is_some());
}

#[test]
fn baseline_input_boundaries_invalid_gameplay_is_terminal() {
    use crate::protocol::pulse::decentraland::pulse::EmoteStart;
    let mut invalid = valid_state(0);
    invalid.position_x = 256;
    for (message, reason) in [
        (
            client_message::Message::Input(PlayerStateInput { state: None }),
            DisconnectReason::InvalidInputField,
        ),
        (
            client_message::Message::Input(PlayerStateInput {
                state: Some(invalid.clone()),
            }),
            DisconnectReason::InvalidInputField,
        ),
        (
            client_message::Message::Teleport(teleport_request(0, "")),
            DisconnectReason::InvalidTeleportField,
        ),
        (
            client_message::Message::Teleport(TeleportRequest {
                position_x: 256,
                ..teleport_request(0, "realm")
            }),
            DisconnectReason::InvalidTeleportField,
        ),
        (
            client_message::Message::EmoteStart(EmoteStart {
                player_state: Some(invalid),
                ..Default::default()
            }),
            DisconnectReason::InvalidEmoteField,
        ),
        (
            client_message::Message::EmoteStart(EmoteStart {
                duration_ms: Some(60001),
                player_state: Some(valid_state(0)),
                ..Default::default()
            }),
            DisconnectReason::InvalidEmoteField,
        ),
    ] {
        let mut srv = PulseServer::new();
        authed(&mut srv, 1, "wallet");
        assert_eq!(
            srv.dispatch(1, 0, &client_msg(message), 0, 0),
            Action::Reject {
                reply: None,
                reason
            }
        );
        assert_eq!(
            srv.peers[&1].connection_state,
            PeerConnectionState::PendingDisconnect
        );
        let next = client_msg(client_message::Message::Input(PlayerStateInput {
            state: Some(valid_state(1)),
        }));
        assert_eq!(srv.dispatch(1, 0, &next, 0, 1000), Action::Ignore);
        assert!(srv.board.try_read(1).is_none());
    }
}

#[test]
fn baseline_input_boundaries_rate_rejections_are_terminal() {
    use crate::protocol::pulse::decentraland::pulse::EmoteStart;
    for (message, reason) in [
        (
            client_message::Message::Input(PlayerStateInput {
                state: Some(valid_state(0)),
            }),
            DisconnectReason::InputRateExceeded,
        ),
        (
            client_message::Message::Teleport(teleport_request(0, "realm")),
            DisconnectReason::DiscreteEventRateExceeded,
        ),
        (
            client_message::Message::EmoteStart(EmoteStart {
                player_state: Some(valid_state(0)),
                ..Default::default()
            }),
            DisconnectReason::DiscreteEventRateExceeded,
        ),
        (
            client_message::Message::EmoteStop(Default::default()),
            DisconnectReason::DiscreteEventRateExceeded,
        ),
    ] {
        let mut srv = PulseServer::new();
        srv.gameplay_limiter = GameplayRateLimiter::new(1, 1, 1, 1);
        authed(&mut srv, 1, "wallet");
        let bytes = client_msg(message);
        assert_eq!(srv.dispatch(1, 0, &bytes, 0, 1000), Action::Applied);
        let before = srv.board.try_read(1).cloned();
        assert_eq!(
            srv.dispatch(1, 0, &bytes, 0, 1000),
            Action::Reject {
                reply: None,
                reason
            }
        );
        assert_eq!(
            srv.peers[&1].connection_state,
            PeerConnectionState::PendingDisconnect
        );
        srv.dispatch(1, 0, &bytes, 0, 3000);
        assert_eq!(srv.board.try_read(1).cloned(), before);
    }
}

#[test]
fn baseline_input_boundaries_realms_count_utf16() {
    let srv = PulseServer::new();
    for (realm, accepted) in [
        ("a".repeat(255), true),
        ("a".repeat(256), false),
        (format!("{}a", "\u{1f600}".repeat(127)), true),
        ("\u{1f600}".repeat(128), false),
        (String::new(), false),
    ] {
        assert_eq!(
            srv.validate_handshake_initial_state(&PlayerInitialState {
                realm: realm.clone(),
                state: Some(valid_state(0)),
                ..Default::default()
            }),
            accepted
        );
        let mut server = PulseServer::new();
        authed(&mut server, 1, "wallet");
        assert_eq!(
            matches!(
                server.dispatch(
                    1,
                    0,
                    &client_msg(client_message::Message::Teleport(teleport_request(
                        0, &realm
                    ))),
                    0,
                    0
                ),
                Action::Applied
            ),
            accepted
        );
        assert_eq!(
            validate::realm_length(&realm, 255) && !realm.is_empty(),
            accepted
        );
    }
}

#[test]
fn baseline_input_boundaries_raw_fields_and_pointing_aggregate() {
    use crate::protocol::pulse::decentraland::pulse::PlayerAnimationFlags;
    type Change = fn(&mut PlayerState);
    let changes: &[Change] = &[
        |s| s.parcel_index += 1,
        |s| s.position_x += 1,
        |s| s.position_y += 1,
        |s| s.position_z += 1,
        |s| s.velocity_x += 1,
        |s| s.velocity_y += 1,
        |s| s.velocity_z += 1,
        |s| s.rotation_y += 1,
        |s| s.jump_count += 1,
        |s| s.movement_blend += 1,
        |s| s.slide_blend += 1,
        |s| s.head_yaw = Some(0),
        |s| s.head_pitch = Some(0),
        |s| s.state_flags += 1,
        |s| s.glide_state += 1,
    ];
    for change in changes {
        let mut srv = PulseServer::new();
        authed(&mut srv, 1, "wallet");
        let mut state = valid_state(0);
        let input = |state| {
            client_msg(client_message::Message::Input(PlayerStateInput {
                state: Some(state),
            }))
        };
        srv.dispatch(1, 0, &input(state.clone()), 0, 0);
        change(&mut state);
        srv.dispatch(1, 0, &input(state.clone()), 0, 100);
        assert_eq!(srv.board.last_seq(1), 1);
        srv.dispatch(1, 0, &input(state), 0, 200);
        assert_eq!(srv.board.last_seq(1), 1);
    }
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "wallet");
    let mut state = valid_state(0);
    let input = |state| {
        client_msg(client_message::Message::Input(PlayerStateInput {
            state: Some(state),
        }))
    };
    state.state_flags = PlayerAnimationFlags::PointingAt as u32;
    state.point_at_x = Some(1);
    srv.dispatch(1, 0, &input(state.clone()), 0, 0);
    assert_eq!(srv.board.try_read(1).unwrap().point_at_x, None);
    state.point_at_x = Some(2);
    srv.dispatch(1, 0, &input(state.clone()), 0, 100);
    assert_eq!(srv.board.last_seq(1), 0);
    state.point_at_y = Some(3);
    state.point_at_z = Some(4);
    srv.dispatch(1, 0, &input(state.clone()), 0, 200);
    assert_eq!(srv.board.last_seq(1), 1);
    for axis in 0..3 {
        match axis {
            0 => state.point_at_x = Some(3),
            1 => state.point_at_y = Some(4),
            _ => state.point_at_z = Some(5),
        }
        srv.dispatch(1, 0, &input(state.clone()), 0, 300 + axis * 100);
        assert_eq!(srv.board.last_seq(1), 2 + axis);
    }
    state.state_flags = 0;
    srv.dispatch(1, 0, &input(state.clone()), 0, 700);
    assert_eq!(srv.board.try_read(1).unwrap().point_at_x, None);
    state.point_at_x = Some(7);
    srv.dispatch(1, 0, &input(state.clone()), 0, 800);
    assert_eq!(srv.board.last_seq(1), 5);
    state.point_at_x = Some(1 << 17);
    assert_eq!(
        srv.dispatch(1, 0, &input(state), 0, 900),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::InvalidInputField
        }
    );
}

#[test]
fn baseline_input_boundaries_first_post_teleport_is_published() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 1, "wallet");
    srv.dispatch(
        1,
        0,
        &client_msg(client_message::Message::Teleport(teleport_request(
            0, "realm",
        ))),
        0,
        0,
    );
    let mut state = valid_state(0);
    state.state_flags =
        crate::protocol::pulse::decentraland::pulse::PlayerAnimationFlags::Grounded as u32;
    let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
        state: Some(state),
    }));
    srv.dispatch(1, 0, &bytes, 0, 100);
    assert_eq!(srv.board.last_seq(1), 1);
    assert!(!srv.board.try_read(1).unwrap().is_teleport);
    srv.dispatch(1, 0, &bytes, 0, 200);
    assert_eq!(srv.board.last_seq(1), 1);
}

#[tokio::test]
async fn baseline_input_boundaries_preserve_inclusive_pulse6_freshness() {
    let (bytes, _, timestamp) = signed_handshake_request().await;
    let request = match ClientMessage::decode(bytes.as_slice())
        .unwrap()
        .message
        .unwrap()
    {
        client_message::Message::Handshake(request) => request,
        _ => unreachable!(),
    };
    for (offset, accepted) in [
        (-60001, false),
        (-60000, true),
        (60000, true),
        (60001, false),
    ] {
        assert_eq!(
            verify_handshake_bytes(&request.auth_chain, timestamp + offset).is_ok(),
            accepted,
            "offset={offset}"
        );
        let mut srv = PulseServer::with_config(4, 2, &[50], false);
        srv.peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        // pulse-6 keeps inclusive signature freshness while consuming/refusing future signatures.
        assert_eq!(
            matches!(
                srv.dispatch(1, channel::RELIABLE, &bytes, timestamp + offset, 1000),
                Action::Authenticated { .. }
            ),
            accepted && offset >= 0
        );
    }
}

#[test]
fn baseline_input_boundaries_duplicate_has_no_outbound_and_resync_respects_history() {
    for (capacity, changes, retained) in
        [(20, 19, true), (20, 20, false), (2, 1, true), (2, 2, false)]
    {
        let mut srv = PulseServer::with_config(4, capacity, &[50], true);
        for peer in [0, 1] {
            authed(&mut srv, peer, &format!("wallet-{peer}"));
            srv.dispatch(
                peer,
                0,
                &client_msg(client_message::Message::Teleport(teleport_request(
                    0, "realm",
                ))),
                0,
                1000,
            );
        }
        srv.simulate(1050);
        let input = |state| {
            client_msg(client_message::Message::Input(PlayerStateInput {
                state: Some(state),
            }))
        };
        srv.dispatch(1, 0, &input(valid_state(0)), 0, 1100);
        srv.simulate(1150);
        let baseline = srv.board.last_seq(1);
        srv.dispatch(1, 0, &input(valid_state(0)), 0, 1200);
        assert!(
            !srv.simulate(1250).iter().any(|m| m.target == 0),
            "duplicate movement emitted observer traffic"
        );
        for n in 1..=changes {
            let mut state = valid_state(0);
            state.position_x += n;
            srv.dispatch(1, 0, &input(state), 0, 1300 + n * 100);
        }
        assert_eq!(srv.board.try_read_seq(1, baseline).is_some(), retained);
        srv.peers.get_mut(&0).unwrap().request_resync(1, baseline);
        let out = srv.simulate(5000);
        let full = out.iter().any(|m| m.target == 0 && matches!(&m.message.message, Some(server_message::Message::PlayerStateFull(f)) if f.subject_id == 1));
        let delta = out.iter().any(|m| m.target == 0 && matches!(&m.message.message, Some(server_message::Message::PlayerStateDelta(d)) if d.subject_id == 1 && d.baseline_seq == baseline));
        assert_eq!(delta, retained);
        assert_eq!(full, !retained);
    }
}

#[tokio::test]
async fn baseline_input_boundaries_expired_chain_and_duplicate_wallet() {
    let (bytes, _, timestamp) =
        signed_handshake_request_with_expiration("2000-01-01T00:00:00.000Z").await;
    let mut srv = PulseServer::with_config(4, 2, &[50], false);
    srv.peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    assert!(matches!(
        srv.dispatch(1, channel::RELIABLE, &bytes, timestamp, 1000),
        Action::Reply(_)
    ));
    assert!(!srv.is_authenticated(1));
    assert!(srv.board.try_read(1).is_none());
    let (bytes, wallet, timestamp) = signed_handshake_request().await;
    authed(&mut srv, 0, &wallet);
    assert!(matches!(
        srv.dispatch(1, channel::RELIABLE, &bytes, timestamp, 1100),
        Action::Authenticated {
            duplicate_of: Some(0),
            ..
        }
    ));
}

#[test]
fn baseline_input_boundaries_history_preserves_retained_stop_reason() {
    use crate::protocol::pulse::decentraland::pulse::EmoteStart;
    for (capacity, movements, retained) in
        [(20, 19, true), (20, 20, false), (2, 1, true), (2, 2, false)]
    {
        let mut srv = PulseServer::with_config(4, capacity, &[50], false);
        for peer in [0, 1] {
            authed(&mut srv, peer, &format!("wallet-{peer}"));
            srv.dispatch(
                peer,
                0,
                &client_msg(client_message::Message::Teleport(teleport_request(
                    0, "realm",
                ))),
                0,
                1000,
            );
        }
        srv.simulate(1050);
        let emote = client_msg(client_message::Message::EmoteStart(EmoteStart {
            emote_id: "wave".into(),
            player_state: Some(valid_state(0)),
            ..Default::default()
        }));
        srv.dispatch(1, 0, &emote, 0, 1100);
        srv.simulate(1150);
        srv.publish_emote_stop(1, 1200, EmoteStopReason::Completed);
        for n in 0..movements {
            let mut state = valid_state(0);
            state.position_x += n + 1;
            let bytes = client_msg(client_message::Message::Input(PlayerStateInput {
                state: Some(state),
            }));
            srv.dispatch(1, 0, &bytes, 0, 1300 + n * 100);
            let before = srv.board.last_seq(1);
            srv.dispatch(1, 0, &bytes, 0, 1350 + n * 100);
            assert_eq!(srv.board.last_seq(1), before);
        }
        let out = srv.simulate(5000);
        let reason = out
            .iter()
            .find_map(|m| match &m.message.message {
                Some(server_message::Message::EmoteStopped(e)) if m.target == 0 => Some(e.reason),
                _ => None,
            })
            .expect("known active emote must stop even after marker eviction");
        assert_eq!(
            reason,
            if retained {
                EmoteStopReason::Completed as i32
            } else {
                EmoteStopReason::Cancelled as i32
            }
        );
    }
}

#[test]
fn baseline_approved_policy_profiles_bypass_discrete_budget_and_keep_monotonicity() {
    let mut srv = PulseServer::new();
    srv.gameplay_limiter = GameplayRateLimiter::new(20, 16, 1, 1);
    authed(&mut srv, 1, "wallet");
    let teleport = client_msg(client_message::Message::Teleport(teleport_request(
        0, "realm",
    )));
    assert_eq!(srv.dispatch(1, 0, &teleport, 0, 1000), Action::Applied);
    for (version, expected) in [
        (5, 5),
        (3, 5),
        (-1, 5),
        (5, 5),
        (6, 6),
        (i32::MAX, i32::MAX),
    ] {
        let bytes = client_msg(client_message::Message::ProfileAnnouncement(
            ProfileVersionAnnouncement { version },
        ));
        assert_eq!(srv.dispatch(1, 0, &bytes, 0, 1000), Action::Applied);
        assert_eq!(srv.profiles.get(1), expected);
        assert_eq!(
            srv.peers[&1].connection_state,
            PeerConnectionState::Authenticated
        );
    }
    let mut srv = PulseServer::new();
    srv.gameplay_limiter = GameplayRateLimiter::new(20, 16, 1, 1);
    authed(&mut srv, 1, "wallet");
    for version in [-1, 0, 1, 2, 2, 1] {
        srv.dispatch(
            1,
            0,
            &client_msg(client_message::Message::ProfileAnnouncement(
                ProfileVersionAnnouncement { version },
            )),
            0,
            1000,
        );
    }
    assert_eq!(
        srv.dispatch(1, 0, &teleport, 0, 1000),
        Action::Applied,
        "profiles cannot debit discrete tokens"
    );
    assert_eq!(
        srv.dispatch(1, 0, &teleport, 0, 1000),
        Action::Reject {
            reply: None,
            reason: DisconnectReason::DiscreteEventRateExceeded
        }
    );
}

#[tokio::test]
async fn baseline_approved_policy_long_emote_ids_keep_duration_and_frame_bounds() {
    use crate::protocol::pulse::decentraland::pulse::EmoteStart;
    use crate::pulse::transport::webtransport::{
        config::DEFAULT_MAX_MESSAGE_BYTES,
        framing::{StreamFrameReader, stream_frame},
    };
    let (base, _, now_ms) = signed_handshake_request().await;
    for id in ["a".repeat(512), "a".repeat(513), "\u{1f600}".repeat(513)] {
        let mut srv = PulseServer::new();
        authed(&mut srv, 1, "wallet");
        let bytes = client_msg(client_message::Message::EmoteStart(EmoteStart {
            emote_id: id.clone(),
            duration_ms: Some(60000),
            player_state: Some(valid_state(0)),
            ..Default::default()
        }));
        assert!(bytes.len() < DEFAULT_MAX_MESSAGE_BYTES);
        assert_eq!(srv.dispatch(1, 0, &bytes, 0, 1000), Action::Applied);
        assert_eq!(
            srv.board
                .try_read(1)
                .unwrap()
                .emote
                .as_ref()
                .unwrap()
                .emote_id
                .as_deref(),
            Some(id.as_str())
        );
        let mut srv = PulseServer::new();
        srv.peers
            .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
        let bytes = with_initial_state(
            &base,
            Some(PlayerInitialState {
                state: Some(valid_state(0)),
                realm: "realm".into(),
                emote_id: Some(id.clone()),
                emote_duration_ms: Some(60000),
                ..Default::default()
            }),
        );
        assert!(matches!(
            srv.dispatch(1, 0, &bytes, now_ms, 1000),
            Action::Authenticated { .. }
        ));
        let mut srv = PulseServer::new();
        authed(&mut srv, 1, "wallet");
        let bytes = client_msg(client_message::Message::EmoteStart(EmoteStart {
            emote_id: id,
            duration_ms: Some(60001),
            player_state: Some(valid_state(0)),
            ..Default::default()
        }));
        assert_eq!(
            srv.dispatch(1, 0, &bytes, 0, 1000),
            Action::Reject {
                reply: None,
                reason: DisconnectReason::InvalidEmoteField
            }
        );
        assert!(srv.board.try_read(1).is_none());
    }
    let bytes = client_msg(client_message::Message::EmoteStart(EmoteStart {
        emote_id: "a".repeat(DEFAULT_MAX_MESSAGE_BYTES),
        player_state: Some(valid_state(0)),
        ..Default::default()
    }));
    let mut reader = StreamFrameReader::new(DEFAULT_MAX_MESSAGE_BYTES);
    reader.append(&stream_frame(&bytes));
    assert!(
        reader.try_read().is_err(),
        "transport frame cap still rejects an oversized encoded emote"
    );
}

async fn envelope_rig() -> (
    PulseServer,
    Transports,
    rusty_enet::Host<std::net::UdpSocket>,
    u32,
) {
    let host = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: 2,
        channel_limit: 3,
    })
    .await
    .unwrap();
    let addr = host.local_addr().unwrap();
    let mut transports = Transports::enet_only(host, 2);
    let mut client = rusty_enet::Host::new(
        std::net::UdpSocket::bind("127.0.0.1:0").unwrap(),
        rusty_enet::HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        },
    )
    .unwrap();
    client.connect(addr, 3, 0).unwrap();
    let mut server = PulseServer::new();
    let mut peer = None;
    let mut connected = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while peer.is_none() || !connected {
        assert!(std::time::Instant::now() < deadline);
        if let Some(Event::Connect { peer: id, .. }) = transports.service().await.unwrap() {
            server
                .connect_pending(id as u32, "127.0.0.1".into(), 0)
                .unwrap();
            authed(&mut server, id as u32, "0xabc");
            peer = Some(id as u32);
        }
        if let Some(rusty_enet::Event::Connect { .. }) = client.service().unwrap() {
            connected = true;
        }
    }
    (server, transports, client, peer.unwrap())
}

fn envelope_profile(size: usize, version: i32) -> Vec<u8> {
    let base = client_msg(client_message::Message::ProfileAnnouncement(
        ProfileVersionAnnouncement { version },
    ));
    for padding in size.saturating_sub(12)..size {
        let mut data = base.clone();
        data.extend_from_slice(&[0xa2, 0x06]);
        let mut length = padding;
        while length >= 128 {
            data.push((length as u8 & 127) | 128);
            length >>= 7;
        }
        data.push(length as u8);
        data.resize(data.len() + padding, 0);
        if data.len() == size {
            assert!(ClientMessage::decode(data.as_slice()).is_ok());
            return data;
        }
    }
    panic!("cannot construct exact packet size {size}");
}

async fn deliver_envelope(
    server: &mut PulseServer,
    transports: &mut Transports,
    client: &mut rusty_enet::Host<std::net::UdpSocket>,
    data: Vec<u8>,
) {
    client
        .peer_mut(rusty_enet::PeerID(0))
        .send(0, &rusty_enet::Packet::reliable(data))
        .unwrap();
    client.flush();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "reassembled ENet packet missing"
        );
        let _ = client.service().unwrap();
        match transports.service().await.unwrap() {
            Some(Event::Receive {
                peer,
                channel,
                packet,
            }) => {
                server
                    .receive(transports, peer as u32, channel, &packet.data, 1000, 1000)
                    .await
                    .unwrap();
                return;
            }
            Some(Event::ReceiveLimitExceeded { peer, kind, .. }) => {
                server
                    .receive_limit(transports, peer as u32, kind)
                    .await
                    .unwrap();
                return;
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn enet_application_envelope_boundaries_and_shared_corruption_budget() {
    let (mut server, mut transports, mut client, peer) = envelope_rig().await;
    deliver_envelope(
        &mut server,
        &mut transports,
        &mut client,
        envelope_profile(4096, 1),
    )
    .await;
    assert_eq!(server.profiles.get(peer), 1);
    for (n, size) in [4097, 8192, 4097, 8192, 4097].into_iter().enumerate() {
        deliver_envelope(
            &mut server,
            &mut transports,
            &mut client,
            envelope_profile(size, n as i32 + 2),
        )
        .await;
        assert_eq!(
            server.profiles.get(peer),
            1,
            "oversize packet must not mutate profile"
        );
    }
    deliver_envelope(&mut server, &mut transports, &mut client, vec![0x0a, 0xff]).await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut reason = None;
    while reason.is_none() {
        assert!(std::time::Instant::now() < deadline);
        if let Some(rusty_enet::Event::Disconnect { data, .. }) = client.service().unwrap() {
            reason = Some(data);
        }
        if let Some(Event::Disconnect { peer }) = transports.service().await.unwrap() {
            server
                .on_disconnect(&mut transports, peer as u32)
                .await
                .unwrap();
        }
    }
    assert_eq!(reason, Some(DisconnectReason::PacketCorrupted.code()));
}

#[tokio::test]
async fn enet_gross_oversize_immediately_tears_down() {
    let (mut server, mut transports, mut client, peer) = envelope_rig().await;
    client
        .peer_mut(rusty_enet::PeerID(0))
        .send(0, &rusty_enet::Packet::reliable(envelope_profile(8193, 9)))
        .unwrap();
    deliver_envelope(
        &mut server,
        &mut transports,
        &mut client,
        envelope_profile(4096, 10),
    )
    .await;
    assert!(
        !server.peers.contains_key(&peer),
        "gross oversize must remove application state inline"
    );
    assert!(!server.ip_limiter.contains_peer(peer));
    assert_eq!(server.profiles.get(peer), 0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let reason = loop {
        assert!(std::time::Instant::now() < deadline);
        if let Some(rusty_enet::Event::Disconnect { data, .. }) = client.service().unwrap() {
            break data;
        }
        if let Some(Event::Receive {
            peer: received_peer,
            ..
        }) = transports.service().await.unwrap()
        {
            assert_ne!(
                received_peer as u32, peer,
                "kicked peer retained queued application input"
            );
        }
    };
    assert_eq!(reason, DisconnectReason::PacketCorrupted.code());
    assert!(!server.peers.contains_key(&peer));
    assert_eq!(server.profiles.get(peer), 0);
}

#[tokio::test]
async fn enet_receive_limit_cleans_preauth_and_aggregate_preserves_admission() {
    let (mut server, mut transports, _client, peer) = envelope_rig().await;
    server.on_disconnect(&mut transports, peer).await.unwrap();
    server.connect_pending(peer, "127.0.0.1".into(), 0).unwrap();
    server
        .receive_limit(
            &mut transports,
            peer,
            rusty_enet::ReceiveLimitKind::QueuedBytes,
        )
        .await
        .unwrap();
    server
        .receive_limit(
            &mut transports,
            peer,
            rusty_enet::ReceiveLimitKind::QueuedCommands,
        )
        .await
        .unwrap();
    assert!(server.peers.contains_key(&peer));
    assert!(server.ip_limiter.contains_peer(peer));
    server
        .receive_limit(
            &mut transports,
            peer,
            rusty_enet::ReceiveLimitKind::PacketSize,
        )
        .await
        .unwrap();
    assert!(!server.peers.contains_key(&peer));
    assert!(!server.ip_limiter.contains_peer(peer));
    // Cleanup remains safe if an external disconnect is observed after the inline cleanup.
    server.on_disconnect(&mut transports, peer).await.unwrap();
    assert!(!server.peers.contains_key(&peer));
}

#[tokio::test]
async fn enet_receive_policy_preserves_large_outgoing_snapshot_to_default_client() {
    let (_server, mut transports, mut client, peer) = envelope_rig().await;
    let payload = vec![0x5a; 9000];
    transports
        .send(peer, Packet::reliable(0, payload.clone()))
        .await
        .unwrap();
    transports.flush();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "large outgoing snapshot missing"
        );
        if let Some(rusty_enet::Event::Receive { packet, .. }) = client.service().unwrap() {
            assert_eq!(packet.data(), payload);
            break;
        }
        let _ = transports.service().await.unwrap();
    }
}

fn assert_corruption_rejection_blocks_gameplay(srv: &mut PulseServer, peer: u32) {
    let before = srv.board.try_read(peer).unwrap().clone();
    let profile = srv.profiles.get(peer);
    let packets = [
        (1, client_message::Message::Input(PlayerStateInput { state: Some(valid_state(6)) })),
        (0, client_message::Message::Teleport(teleport_request(9, "next-realm"))),
        (0, client_message::Message::ProfileAnnouncement(ProfileVersionAnnouncement { version: 9 })),
        (2, client_message::Message::EmoteStart(crate::protocol::pulse::decentraland::pulse::EmoteStart {
            emote_id: "urn:decentraland:off-chain:base-emotes:wave".into(),
            duration_ms: Some(2000), player_state: Some(valid_state(3)), mask: None,
        })),
        (2, client_message::Message::EmoteStop(Default::default())),
    ];
    for (channel, message) in packets {
        assert_eq!(srv.dispatch(peer, channel, &client_msg(message), 1000, 1100), Action::Ignore);
        assert_eq!(srv.board.try_read(peer), Some(&before));
        assert_eq!(srv.board.last_seq(peer), before.seq);
        assert_eq!(srv.profiles.get(peer), profile);
    }
    assert_eq!(srv.peers[&peer].connection_state, PeerConnectionState::PendingDisconnect);
}

#[test]
fn decode_corruption_rejection_is_terminal_for_queued_gameplay() {
    let mut srv = PulseServer::new();
    authed(&mut srv, 7, "wallet");
    let input = client_msg(client_message::Message::Input(PlayerStateInput { state: Some(valid_state(5)) }));
    assert_eq!(srv.dispatch(7, 1, &input, 0, 100), Action::Applied);
    for _ in 0..5 {
        assert_eq!(srv.dispatch(7, 0, &[0x0a, 0xff], 1000, 1000), Action::Ignore);
        assert_eq!(srv.peers[&7].connection_state, PeerConnectionState::Authenticated);
    }
    assert_eq!(srv.dispatch(7, 0, &[0x0a, 0xff], 1000, 1000),
        Action::Reject { reply: None, reason: DisconnectReason::PacketCorrupted });
    assert_corruption_rejection_blocks_gameplay(&mut srv, 7);
}

#[tokio::test]
async fn native_corruption_rejection_is_terminal_before_transport_disconnect() {
    for size in [4097, 8192] {
        let (mut server, mut transports, mut client, peer) = envelope_rig().await;
        let input = client_msg(client_message::Message::Input(PlayerStateInput { state: Some(valid_state(5)) }));
        assert_eq!(server.dispatch(peer, 1, &input, 0, 100), Action::Applied);
        // Decode failures and oversize native envelopes share one corruption budget.
        for _ in 0..5 {
            assert_eq!(server.dispatch(peer, 0, &[0x0a, 0xff], 1000, 1000), Action::Ignore);
        }
        server.receive(&mut transports, peer, 0, &envelope_profile(size, 9), 1000, 1000).await.unwrap();
        assert_corruption_rejection_blocks_gameplay(&mut server, peer);
        assert!(server.ip_limiter.contains_peer(peer), "retain IP reservation until disconnect");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut wire_reason = None;
        let mut disconnected = false;
        while wire_reason.is_none() || !disconnected {
            assert!(std::time::Instant::now() < deadline);
            if let Some(rusty_enet::Event::Disconnect { data, .. }) = client.service().unwrap() {
                wire_reason = Some(data);
            }
            if let Some(Event::Disconnect { peer: id }) = transports.service().await.unwrap() {
                assert_eq!(id as u32, peer);
                server.on_disconnect(&mut transports, peer).await.unwrap();
                disconnected = true;
            }
        }
        assert_eq!(wire_reason, Some(16));
        assert!(!server.peers.contains_key(&peer));
        assert!(!server.ip_limiter.contains_peer(peer));
    }
}

#[tokio::test]
async fn native_aggregate_limit_disconnect_releases_preauth_and_reuses_slot() {
    struct RecordingSocket {
        udp: std::net::UdpSocket,
        header: Option<[u8; 2]>,
    }
    impl rusty_enet::Socket for RecordingSocket {
        type Address = std::net::SocketAddr;
        type Error = std::io::Error;
        fn init(&mut self, options: rusty_enet::SocketOptions) -> std::io::Result<()> {
            rusty_enet::Socket::init(&mut self.udp, options)
        }
        fn send(&mut self, address: Self::Address, buffer: &[u8]) -> std::io::Result<usize> {
            if buffer.len() >= 2 {
                self.header = Some([buffer[0], buffer[1]]);
            }
            rusty_enet::Socket::send(&mut self.udp, address, buffer)
        }
        fn receive(
            &mut self,
            buffer: &mut [u8; rusty_enet::MTU_MAX],
        ) -> std::io::Result<Option<(Self::Address, rusty_enet::PacketReceived)>> {
            rusty_enet::Socket::receive(&mut self.udp, buffer)
        }
    }

    let host = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: 1,
        channel_limit: 3,
    })
    .await
    .unwrap();
    let address = host.local_addr().unwrap();
    let mut transports = Transports::enet_only(host, 1);
    let mut client = rusty_enet::Host::new(
        RecordingSocket {
            udp: std::net::UdpSocket::bind("127.0.0.1:0").unwrap(),
            header: None,
        },
        rusty_enet::HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        },
    )
    .unwrap();
    let mut server = PulseServer::new();
    server.pre_auth_enet = PreAuthAdmission::new(1, 1);
    server.ip_limiter_options.enabled = true;
    server.ip_limiter_options.max_concurrency = 1;
    client.connect(address, 3, 0).unwrap();
    let mut connected = false;
    let mut admitted = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !connected || !admitted {
        assert!(
            std::time::Instant::now() < deadline,
            "initial native connection missing"
        );
        if let Some(Event::Connect { peer, .. }) = transports.service().await.unwrap() {
            assert_eq!(peer, 0);
            server.connect_pending(0, "127.0.0.1".into(), 0).unwrap();
            admitted = true;
        }
        if let Some(rusty_enet::Event::Connect { .. }) = client.service().unwrap() {
            connected = true;
        }
    }
    assert_eq!(server.pre_auth_enet.in_flight(), 1);
    assert!(server.ip_limiter.contains_peer(0));
    assert_eq!(
        server.peers[&0].connection_state,
        PeerConnectionState::PendingAuth
    );
    // Capture the negotiated peer/session header from the real connected sender.
    client.peer_mut(rusty_enet::PeerID(0)).ping();
    client.flush();
    let header = u16::from_be_bytes(client.socket().header.unwrap());
    let mut datagram = ((header & 0x3fff) | 0x4000).to_be_bytes().to_vec();
    datagram.extend_from_slice(&1u16.to_be_bytes());
    // Eight incomplete 8192-byte groups exceed Pulse's charged queued-byte cap.
    // Missing sequence 1 prevents any application Receive before the local report.
    for group in 0..8u16 {
        let sequence = 2 + group * 2;
        datagram.extend_from_slice(&[0x88, 0]);
        datagram.extend_from_slice(&sequence.to_be_bytes());
        datagram.extend_from_slice(&sequence.to_be_bytes());
        datagram.extend_from_slice(&1u16.to_be_bytes());
        datagram.extend_from_slice(&2u32.to_be_bytes());
        datagram.extend_from_slice(&0u32.to_be_bytes());
        datagram.extend_from_slice(&8192u32.to_be_bytes());
        datagram.extend_from_slice(&0u32.to_be_bytes());
        datagram.push(42);
    }
    client.socket().udp.send_to(&datagram, address).unwrap();
    // The refused command ends that datagram; the service receive pass must
    // process this separate lifecycle datagram before dispatching the report.
    let mut disconnect = datagram[..4].to_vec();
    disconnect.extend_from_slice(&[0x44, 0xff, 0, 0]);
    disconnect.extend_from_slice(&77u32.to_be_bytes());
    client.socket().udp.send_to(&disconnect, address).unwrap();
    let mut limited = false;
    let mut disconnected = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !disconnected {
        assert!(
            std::time::Instant::now() < deadline,
            "native Disconnect lost behind aggregate limit (report_seen={limited})"
        );
        match transports.service().await.unwrap() {
            Some(Event::ReceiveLimitExceeded { peer, kind, .. }) => {
                assert_eq!(peer, 0);
                assert_eq!(kind, rusty_enet::ReceiveLimitKind::QueuedBytes);
                server
                    .receive_limit(&mut transports, peer as u32, kind)
                    .await
                    .unwrap();
                assert_eq!(server.pre_auth_enet.in_flight(), 1);
                assert!(server.ip_limiter.contains_peer(0));
                limited = true;
            }
            Some(Event::Disconnect { peer }) => {
                assert!(limited, "native lifecycle must follow the aggregate report");
                assert_eq!(peer, 0);
                server
                    .on_disconnect(&mut transports, peer as u32)
                    .await
                    .unwrap();
                disconnected = true;
            }
            Some(_) => panic!("unexpected native application event"),
            None => {}
        }
    }
    assert_eq!(server.pre_auth_enet.in_flight(), 0);
    assert!(!server.ip_limiter.contains_peer(0));
    assert!(!server.peers.contains_key(&0));

    client.peer_mut(rusty_enet::PeerID(0)).reset();
    client.connect(address, 3, 0).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "native slot was not reusable"
        );
        if let Some(Event::Connect { peer, .. }) = transports.service().await.unwrap() {
            assert_eq!(peer, 0);
            server.connect_pending(0, "127.0.0.1".into(), 1).unwrap();
            break;
        }
        let _ = client.service().unwrap();
    }
    assert_eq!(server.pre_auth_enet.in_flight(), 1);
    assert!(server.ip_limiter.contains_peer(0));
}
