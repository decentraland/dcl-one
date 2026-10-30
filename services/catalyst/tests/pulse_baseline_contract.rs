//! Source-derived baseline vectors, deliberately separate from captured client evidence.
#[path = "support/pulse_contract.rs"]
mod pulse_contract;
use catalyst_comms::protocol::pulse::decentraland::pulse::*;
use catalyst_comms::pulse::{
    PulseServer,
    server::{Action, SERVER_FEATURES, channel},
    simulation::{PeerConnectionState, PeerState},
    snapshot::{PeerSnapshotPublisher, TickBoards},
};
use prost::Message;
use pulse_contract::*;
use serde_json::json;

#[test]
fn baseline_manifest_integrity() {
    let m = manifest();
    assert_eq!(m["version"], 1);
    assert_eq!(
        m["pins"]["protocol"],
        "694568db455b07e0eca2c119d9e916a4c5581c69"
    );
    let lanes = m["lanes"].as_array().unwrap();
    for required in [
        "0.189",
        "0.190",
        "0.191",
        "Godot `main`",
        "Godot `v0.32",
        "Bevy archived",
        "Bevy release",
        "Integrated Bevy",
        "SDK7",
        "sdk-multiplayer-server",
        "creator-hub",
        "web apps",
        "Places",
        "dependency wrappers",
    ] {
        assert!(
            lanes
                .iter()
                .any(|l| l["lane"].as_str().unwrap().contains(required)),
            "missing lane {required}"
        );
    }
    for lane in lanes {
        assert!(
            ["proved use", "no use found", "excluded", "unverified"]
                .contains(&lane["coverage"].as_str().unwrap())
        );
        assert!(
            ["compatible", "breaks", "unproven"].contains(&lane["compatibility"].as_str().unwrap())
        );
    }
    assert_eq!(m["release_blockers"].as_object().unwrap().len(), 7);
    let fixtures = cases();
    let fixtures = fixtures["cases"].as_array().unwrap();
    assert_eq!(
        fixtures.len(),
        36,
        "default/populated cases for both directions and all nine tags"
    );
    for (direction, arms) in [
        (
            "client",
            [
                "handshake",
                "input",
                "resync",
                "profile_announcement",
                "emote_start",
                "emote_stop",
                "teleport",
                "scene_listener_handshake",
                "scene_listener_update",
            ],
        ),
        (
            "server",
            [
                "handshake",
                "player_state_full",
                "player_state_delta",
                "player_joined",
                "player_left",
                "player_profile_version_announced",
                "emote_started",
                "emote_stopped",
                "teleported",
            ],
        ),
    ] {
        for (i, arm) in arms.iter().enumerate() {
            for suffix in ["", "_populated"] {
                let id = format!("{direction}_{arm}{suffix}");
                let matching: Vec<_> = fixtures
                    .iter()
                    .filter(|case| case["case_id"] == id)
                    .collect();
                assert_eq!(matching.len(), 1, "missing or duplicate {id}");
                assert_eq!(matching[0]["inputs"][0]["direction"], direction);
                assert_eq!(matching[0]["expected_events"][0]["arm"], *arm);
                assert_eq!(matching[0]["expected_events"][0]["tag"], i + 1);
            }
        }
    }
    let mut ids = std::collections::HashSet::new();
    for case in cases()["cases"].as_array().unwrap() {
        assert!(ids.insert(case["case_id"].as_str().unwrap()));
        assert_eq!(case["provenance"]["kind"], "source-derived");
        assert_eq!(case["provenance"]["encoder_executed"], false);
        assert_eq!(case["provenance"]["source_sha"], m["pins"]["protocol"]);
        assert_eq!(case["configuration"]["protocol_features"], 0);
        assert!(case["controlled_time_ms"].is_number());
        assert!(!bytes(case).is_empty());
    }
}

// Explicit projections of generated Rust fields make fixture expectations semantic,
// including absent messages/options, byte arrays and ordered repeated values.
fn handshake_request_payload(value: &HandshakeRequest) -> serde_json::Value {
    json!({
        "auth_chain": value.auth_chain,
        "profile_version": value.profile_version,
        "initial_state": value.initial_state.as_ref().map(player_initial_state_payload),
    })
}

fn player_initial_state_payload(value: &PlayerInitialState) -> serde_json::Value {
    json!({
        "state": value.state.as_ref().map(player_state_payload),
        "emote_id": value.emote_id,
        "emote_duration_ms": value.emote_duration_ms,
        "emote_start_offset_ms": value.emote_start_offset_ms,
        "realm": value.realm,
        "emote_mask": value.emote_mask,
    })
}

fn profile_version_announcement_payload(value: &ProfileVersionAnnouncement) -> serde_json::Value {
    json!({
        "version": value.version,
    })
}

fn player_state_input_payload(value: &PlayerStateInput) -> serde_json::Value {
    json!({
        "state": value.state.as_ref().map(player_state_payload),
    })
}

fn resync_request_payload(value: &ResyncRequest) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "known_seq": value.known_seq,
    })
}

fn emote_start_payload(value: &EmoteStart) -> serde_json::Value {
    json!({
        "emote_id": value.emote_id,
        "duration_ms": value.duration_ms,
        "player_state": value.player_state.as_ref().map(player_state_payload),
        "mask": value.mask,
    })
}

fn emote_stop_payload(_value: &EmoteStop) -> serde_json::Value {
    json!({})
}

fn teleport_request_payload(value: &TeleportRequest) -> serde_json::Value {
    json!({
        "parcel_index": value.parcel_index,
        "position_x": value.position_x,
        "position_y": value.position_y,
        "position_z": value.position_z,
        "realm": value.realm,
    })
}

fn parcel_rect_payload(value: &ParcelRect) -> serde_json::Value {
    json!({
        "min_x": value.min_x,
        "min_z": value.min_z,
        "max_x": value.max_x,
        "max_z": value.max_z,
    })
}

fn scene_listener_aoi_payload(value: &SceneListenerAoi) -> serde_json::Value {
    json!({
        "realm": value.realm,
        "parcel_rects": value.parcel_rects.iter().map(parcel_rect_payload).collect::<Vec<_>>(),
    })
}

fn scene_listener_handshake_request_payload(
    value: &SceneListenerHandshakeRequest,
) -> serde_json::Value {
    json!({
        "auth_chain": value.auth_chain,
        "aoi": value.aoi.iter().map(scene_listener_aoi_payload).collect::<Vec<_>>(),
    })
}

fn scene_listener_update_payload(value: &SceneListenerUpdate) -> serde_json::Value {
    json!({
        "aoi": value.aoi.iter().map(scene_listener_aoi_payload).collect::<Vec<_>>(),
    })
}

fn handshake_response_payload(value: &HandshakeResponse) -> serde_json::Value {
    json!({
        "success": value.success,
        "error": value.error,
    })
}

fn player_profile_versions_announced_payload(
    value: &PlayerProfileVersionsAnnounced,
) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "version": value.version,
    })
}

fn player_state_delta_tier0_payload(value: &PlayerStateDeltaTier0) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "baseline_seq": value.baseline_seq,
        "new_seq": value.new_seq,
        "server_tick": value.server_tick,
        "parcel_index": value.parcel_index,
        "position_x": value.position_x,
        "position_y": value.position_y,
        "position_z": value.position_z,
        "velocity_x": value.velocity_x,
        "velocity_y": value.velocity_y,
        "velocity_z": value.velocity_z,
        "rotation_y": value.rotation_y,
        "movement_blend": value.movement_blend,
        "slide_blend": value.slide_blend,
        "head_yaw": value.head_yaw,
        "head_pitch": value.head_pitch,
        "state_flags": value.state_flags,
        "glide_state": value.glide_state,
        "jump_count": value.jump_count,
        "point_at_x": value.point_at_x,
        "point_at_y": value.point_at_y,
        "point_at_z": value.point_at_z,
    })
}

fn player_state_full_payload(value: &PlayerStateFull) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "sequence": value.sequence,
        "server_tick": value.server_tick,
        "state": value.state.as_ref().map(player_state_payload),
    })
}

fn player_joined_payload(value: &PlayerJoined) -> serde_json::Value {
    json!({
        "user_id": value.user_id,
        "profile_version": value.profile_version,
        "state": value.state.as_ref().map(player_state_full_payload),
        "realm": value.realm,
    })
}

fn player_left_payload(value: &PlayerLeft) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
    })
}

fn emote_started_payload(value: &EmoteStarted) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "sequence": value.sequence,
        "server_tick": value.server_tick,
        "emote_id": value.emote_id,
        "player_state": value.player_state.as_ref().map(player_state_payload),
        "mask": value.mask,
    })
}

fn emote_stopped_payload(value: &EmoteStopped) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "server_tick": value.server_tick,
        "reason": value.reason,
        "sequence": value.sequence,
        "player_state": value.player_state.as_ref().map(player_state_payload),
    })
}

fn teleport_performed_payload(value: &TeleportPerformed) -> serde_json::Value {
    json!({
        "subject_id": value.subject_id,
        "sequence": value.sequence,
        "server_tick": value.server_tick,
        "state": value.state.as_ref().map(player_state_payload),
        "realm": value.realm,
    })
}

fn player_state_payload(value: &PlayerState) -> serde_json::Value {
    json!({
        "parcel_index": value.parcel_index,
        "position_x": value.position_x,
        "position_y": value.position_y,
        "position_z": value.position_z,
        "velocity_x": value.velocity_x,
        "velocity_y": value.velocity_y,
        "velocity_z": value.velocity_z,
        "rotation_y": value.rotation_y,
        "movement_blend": value.movement_blend,
        "slide_blend": value.slide_blend,
        "head_yaw": value.head_yaw,
        "head_pitch": value.head_pitch,
        "state_flags": value.state_flags,
        "glide_state": value.glide_state,
        "jump_count": value.jump_count,
        "point_at_x": value.point_at_x,
        "point_at_y": value.point_at_y,
        "point_at_z": value.point_at_z,
    })
}

// Compare every occupied field, including presence and oneof membership. Additional
// unoccupied tags are private allocations, not collisions with the pinned schema.
#[test]
fn baseline_occupied_schema_is_unchanged() {
    let m = manifest();
    let field =
        regex::Regex::new(r"(?m)^\s*(?:(optional|repeated)\s+)?(\w+)\s+(\w+)\s*=\s*(\d+)").unwrap();
    for (name, schema) in m["occupied_schema"].as_object().unwrap() {
        let path = std::path::Path::new(env!("DCL_ONE_ROOT"))
            .join("library/protocol/proto/decentraland/pulse")
            .join(schema["file"].as_str().unwrap());
        let source = std::fs::read_to_string(path).unwrap();
        let source = regex::Regex::new(r"//[^\n]*")
            .unwrap()
            .replace_all(&source, "");
        let start = source.find(&format!("message {name} {{")).unwrap();
        let body_start = start + source[start..].find('{').unwrap() + 1;
        let mut depth = 1;
        let end = source[body_start..]
            .char_indices()
            .find_map(|(i, c)| {
                match c {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                (depth == 0).then_some(body_start + i)
            })
            .unwrap();
        let body = &source[body_start..end];
        let current: Vec<_> = field.captures_iter(body).collect();
        for occupied in schema["fields"].as_array().unwrap() {
            let tag = occupied["tag"].as_u64().unwrap();
            let matches: Vec<_> = current
                .iter()
                .filter(|f| f[4].parse::<u64>().unwrap() == tag)
                .collect();
            assert_eq!(matches.len(), 1, "{name} tag {tag}");
            let f = matches[0];
            assert_eq!(&f[2], occupied["type"].as_str().unwrap(), "{name}.{tag}");
            assert_eq!(&f[3], occupied["name"].as_str().unwrap(), "{name}.{tag}");
            assert_eq!(
                f.get(1).map_or("implicit", |m| m.as_str()),
                occupied["presence"].as_str().unwrap(),
                "{name}.{tag}"
            );
            let in_oneof = body.find("oneof message {").is_some_and(|start| {
                let open = start + body[start..].find('{').unwrap() + 1;
                let mut depth = 1;
                let end = body[open..]
                    .char_indices()
                    .find_map(|(i, c)| {
                        match c {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        (depth == 0).then_some(open + i)
                    })
                    .unwrap();
                (open..end).contains(&f.get(2).unwrap().start())
            });
            assert_eq!(in_oneof, occupied["oneof"] == "message", "{name}.{tag}");
        }
    }
}

#[test]
fn baseline_all_legacy_arms_decode() {
    for direction in ["client", "server"] {
        for case in cases()["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["inputs"][0]["direction"] == direction)
        {
            let id = case["case_id"].as_str().unwrap();
            let raw = bytes(&load_case(id));
            let tag = case["expected_events"][0]["tag"].as_u64().unwrap();
            assert_eq!(raw[0] as u64, (tag << 3) | 2);
            let (actual_event, encoded) = if direction == "client" {
                let decoded = ClientMessage::decode(raw.as_slice()).unwrap();
                let event = match decoded.message.as_ref().unwrap() {
                    client_message::Message::Handshake(value) => {
                        json!({"arm":"handshake","tag":1,"payload":handshake_request_payload(value)})
                    }
                    client_message::Message::Input(value) => {
                        json!({"arm":"input","tag":2,"payload":player_state_input_payload(value)})
                    }
                    client_message::Message::Resync(value) => {
                        json!({"arm":"resync","tag":3,"payload":resync_request_payload(value)})
                    }
                    client_message::Message::ProfileAnnouncement(value) => {
                        json!({"arm":"profile_announcement","tag":4,"payload":profile_version_announcement_payload(value)})
                    }
                    client_message::Message::EmoteStart(value) => {
                        json!({"arm":"emote_start","tag":5,"payload":emote_start_payload(value)})
                    }
                    client_message::Message::EmoteStop(value) => {
                        json!({"arm":"emote_stop","tag":6,"payload":emote_stop_payload(value)})
                    }
                    client_message::Message::Teleport(value) => {
                        json!({"arm":"teleport","tag":7,"payload":teleport_request_payload(value)})
                    }
                    client_message::Message::SceneListenerHandshake(value) => {
                        json!({"arm":"scene_listener_handshake","tag":8,"payload":scene_listener_handshake_request_payload(value)})
                    }
                    client_message::Message::SceneListenerUpdate(value) => {
                        json!({"arm":"scene_listener_update","tag":9,"payload":scene_listener_update_payload(value)})
                    }
                    other => panic!("unexpected private client arm: {other:?}"),
                };
                (event, decoded.encode_to_vec())
            } else {
                let decoded = ServerMessage::decode(raw.as_slice()).unwrap();
                let event = match decoded.message.as_ref().unwrap() {
                    server_message::Message::Handshake(value) => {
                        json!({"arm":"handshake","tag":1,"payload":handshake_response_payload(value)})
                    }
                    server_message::Message::PlayerStateFull(value) => {
                        json!({"arm":"player_state_full","tag":2,"payload":player_state_full_payload(value)})
                    }
                    server_message::Message::PlayerStateDelta(value) => {
                        json!({"arm":"player_state_delta","tag":3,"payload":player_state_delta_tier0_payload(value)})
                    }
                    server_message::Message::PlayerJoined(value) => {
                        json!({"arm":"player_joined","tag":4,"payload":player_joined_payload(value)})
                    }
                    server_message::Message::PlayerLeft(value) => {
                        json!({"arm":"player_left","tag":5,"payload":player_left_payload(value)})
                    }
                    server_message::Message::PlayerProfileVersionAnnounced(value) => {
                        json!({"arm":"player_profile_version_announced","tag":6,"payload":player_profile_versions_announced_payload(value)})
                    }
                    server_message::Message::EmoteStarted(value) => {
                        json!({"arm":"emote_started","tag":7,"payload":emote_started_payload(value)})
                    }
                    server_message::Message::EmoteStopped(value) => {
                        json!({"arm":"emote_stopped","tag":8,"payload":emote_stopped_payload(value)})
                    }
                    server_message::Message::Teleported(value) => {
                        json!({"arm":"teleported","tag":9,"payload":teleport_performed_payload(value)})
                    }
                    other => panic!("unexpected private server arm: {other:?}"),
                };
                (event, decoded.encode_to_vec())
            };
            assert_trace(id, &case["expected_events"], &json!([actual_event]));
            assert_trace(id, &json!(raw), &json!(encoded));
            // Unknown top-level varint and length-delimited fields must not erase
            // a known arm. Prost deliberately drops unknown fields on re-encode.
            let mut extended = raw.clone();
            extended.extend_from_slice(&[0xf8, 0x07, 0x01, 0xf2, 0x07, 0x02, 0xaa, 0xbb]);
            let recovered = if direction == "client" {
                ClientMessage::decode(extended.as_slice())
                    .unwrap()
                    .encode_to_vec()
            } else {
                ServerMessage::decode(extended.as_slice())
                    .unwrap()
                    .encode_to_vec()
            };
            assert_trace(id, &json!(raw), &json!(recovered));
        }
    }
    let case = load_case("client_handshake_populated");
    let Some(client_message::Message::Handshake(h)) =
        ClientMessage::decode(bytes(&case).as_slice())
            .unwrap()
            .message
    else {
        panic!()
    };
    assert_eq!(h.protocol_features, 0);
    assert_eq!(h.auth_chain, b"synthetic");
    assert_eq!(h.profile_version, 1);
    let initial = h.initial_state.unwrap();
    assert_eq!(initial.emote_duration_ms, Some(0));
    assert_eq!(initial.emote_start_offset_ms, Some(0));
    assert_eq!(initial.emote_mask, Some(0));
    let case = load_case("server_player_state_delta_populated");
    let Some(server_message::Message::PlayerStateDelta(d)) =
        ServerMessage::decode(bytes(&case).as_slice())
            .unwrap()
            .message
    else {
        panic!()
    };
    assert_eq!(
        (d.subject_id, d.baseline_seq, d.new_seq, d.server_tick),
        (1, 1, 1, 1)
    );
    assert_eq!(d.parcel_index, Some(0));
    assert_eq!(d.position_x, Some(0));
    assert_eq!(d.jump_count, Some(0));
    assert!(
        ClientMessage::decode(&[0xf8, 0x07, 1][..])
            .unwrap()
            .message
            .is_none()
    );
    assert!(
        ServerMessage::decode(&[0xf8, 0x07, 1][..])
            .unwrap()
            .message
            .is_none()
    );
}

fn client_msg(message: client_message::Message) -> Vec<u8> {
    ClientMessage {
        message: Some(message),
    }
    .encode_to_vec()
}

async fn signed_handshake_request() -> (Vec<u8>, String, i64) {
    use alloy::signers::{Signer, local::PrivateKeySigner};
    use catalyst::auth::{AuthLink, AuthLinkType};
    use catalyst_comms::pulse::handshake::build_signed_fetch_payload;

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

    let eph_payload = format!(
        "Decentraland Login\nEphemeral address: {eph_addr}\nExpiration: 2099-01-01T00:00:00.000Z"
    );
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
        catalyst_comms::protocol::pulse::decentraland::pulse::HandshakeRequest {
            auth_chain: bag.into_bytes(),
            profile_version: 0,
            initial_state: None,
            protocol_features: 0,
        },
    ));
    (bytes, root_addr.to_lowercase(), now_ms)
}

fn pending() -> PulseServer {
    let mut server = PulseServer::new();
    server
        .peers
        .insert(1, PeerState::new(PeerConnectionState::PendingAuth, 0));
    server
}

#[tokio::test]
async fn baseline_handshake_does_not_require_v4() {
    let (raw, wallet, now) = signed_handshake_request().await;
    let mut server = pending();
    match server.dispatch(1, channel::RELIABLE, &raw, now, 0) {
        Action::Authenticated {
            wallet: actual,
            features,
            ..
        } => {
            assert_eq!(actual, wallet);
            assert_eq!(features, 0);
        }
        other => panic!("legacy admission: {other:?}"),
    }
}

#[tokio::test]
async fn baseline_listener_negotiation_is_independent() {
    let (raw, _, now) = signed_handshake_request().await;
    let Some(client_message::Message::Handshake(player)) =
        ClientMessage::decode(raw.as_slice()).unwrap().message
    else {
        panic!()
    };
    for offered in [0, u32::MAX] {
        let mut server = pending();
        let listener = SceneListenerHandshakeRequest {
            auth_chain: player.auth_chain.clone(),
            aoi: vec![SceneListenerAoi {
                realm: "realm-a".into(),
                parcel_rects: vec![ParcelRect::default()],
            }],
            protocol_features: offered,
        };
        let bytes = client_msg(client_message::Message::SceneListenerHandshake(listener));
        match server.dispatch(1, channel::RELIABLE, &bytes, now, 0) {
            Action::AuthenticatedListener {
                features, listener, ..
            } => {
                assert_eq!(features, offered & SERVER_FEATURES);
                assert_eq!(listener.parcel_count(), 1);
            }
            other => panic!("listener admission: {other:?}"),
        }
        let mut server = pending();
        let mut player = player.clone();
        player.protocol_features = offered;
        match server.dispatch(
            1,
            channel::RELIABLE,
            &client_msg(client_message::Message::Handshake(player)),
            now,
            0,
        ) {
            Action::Authenticated { features, .. } => {
                assert_eq!(features, offered & SERVER_FEATURES)
            }
            other => panic!("player admission: {other:?}"),
        }
    }
}

#[test]
fn baseline_feature_zero_receives_only_legacy_messages() {
    let mut server = PulseServer::new();
    for id in [0, 1] {
        let wallet = format!("synthetic-peer-{id}");
        let mut peer = PeerState::new(PeerConnectionState::Authenticated, 0);
        peer.wallet_id = Some(wallet.clone());
        assert_eq!(peer.features, 0);
        server.peers.insert(id, peer);
        server.identity.set(id, wallet);
        server.board.set_active(id);
        PeerSnapshotPublisher {
            board: &mut server.board,
            grids: &mut server.grids,
            encoder: &server.encoder,
        }
        .publish_teleport(
            id,
            10,
            TeleportRequest {
                position_x: 128 + id,
                position_z: 128,
                realm: "realm-a".into(),
                ..Default::default()
            },
        );
    }
    let mut saw_join = false;
    let mut saw_delta = false;
    for tick in 1..=3 {
        if tick > 1 {
            PeerSnapshotPublisher {
                board: &mut server.board,
                grids: &mut server.grids,
                encoder: &server.encoder,
            }
            .publish_from_player_state(
                1,
                tick * 50,
                &PlayerState {
                    position_x: 130 + tick,
                    position_z: 128,
                    ..Default::default()
                },
                None,
                None,
            );
        }
        server.simulation.outbox.clear();
        server.simulation.simulate_tick(
            &mut server.peers,
            TickBoards::new(&server.board, &server.identity, &server.profiles),
            &server.grids,
            &server.aoi,
            tick,
            0,
        );
        for output in &server.simulation.outbox {
            let bytes = output.message.encode_to_vec();
            assert!(!bytes.is_empty());
            assert!(
                (1..=9).contains(&(bytes[0] >> 3)),
                "private arm to baseline peer: {:?}",
                output.message
            );
            saw_join |= matches!(
                output.message.message,
                Some(server_message::Message::PlayerJoined(_))
            );
            saw_delta |= matches!(
                output.message.message,
                Some(server_message::Message::PlayerStateDelta(_))
            );
        }
    }
    assert!(saw_join, "must exercise visibility output");
    assert!(
        saw_delta,
        "must exercise delta output rather than pass vacuously"
    );
}
