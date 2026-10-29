use super::*;
use crate::protocol::pulse::decentraland::common::Vector3;
use crate::protocol::pulse::decentraland::pulse::TeleportRequest;
use crate::pulse::interest::{
    ParcelEncoder, ParcelEncoderOptions, SPATIAL_GRID_CELL_SIZE, SceneListenerCellMapper,
    SceneListenerState, SpatialAreaOfInterest, SpatialAreaOfInterestOptions,
};
use crate::pulse::messages::spec;
use crate::pulse::realm_grids::RealmSpatialGrids;
use crate::pulse::snapshot::{EmoteState, PeerSnapshotPublisher};

mod resync_metrics;
mod scene_listener;

fn v3(x: f32, z: f32) -> Vector3 {
    Vector3 { x, y: 0.0, z }
}

struct World {
    board: SnapshotBoard,
    grid: RealmSpatialGrids,
    encoder: ParcelEncoder,
    aoi: SpatialAreaOfInterest,
    identity: IdentityBoard,
    profiles: ProfileBoard,
    peers: HashMap<u32, PeerState>,
}

impl World {
    fn new() -> Self {
        Self::sized(16)
    }

    fn sized(cap: usize) -> Self {
        World {
            board: SnapshotBoard::new(cap, 16),
            grid: RealmSpatialGrids::new(SPATIAL_GRID_CELL_SIZE, cap),
            encoder: ParcelEncoder::new(ParcelEncoderOptions::default()),
            aoi: SpatialAreaOfInterest::new(SpatialAreaOfInterestOptions::default()),
            identity: IdentityBoard::new(cap),
            profiles: ProfileBoard::new(cap),
            peers: HashMap::new(),
        }
    }

    fn connect(&mut self, id: u32, wallet: &str) {
        self.board.set_active(id);
        self.identity.set(id, wallet.into());
        let mut st = PeerState::new(PeerConnectionState::Authenticated, 0);
        st.wallet_id = Some(wallet.into());
        self.peers.insert(id, st);
    }

    fn connect_listener(&mut self, id: u32, wallet: &str, realm: &str, parcels: &[i32]) {
        self.connect_listener_multi(id, wallet, &[(realm, parcels)]);
    }

    fn connect_listener_multi(&mut self, id: u32, wallet: &str, aoi: &[(&str, &[i32])]) {
        self.identity.set(id, wallet.into());
        let mut st = PeerState::new(PeerConnectionState::Authenticated, 0);
        st.wallet_id = Some(wallet.into());
        st.scene_listener = Some(Arc::new(listener_state(aoi)));
        self.peers.insert(id, st);
    }

    fn reassign_listener(&mut self, id: u32, aoi: &[(&str, &[i32])]) {
        self.peers.get_mut(&id).unwrap().scene_listener = Some(Arc::new(listener_state(aoi)));
    }

    fn teleport(&mut self, id: u32, parcel: i32, local: Vector3, realm: &str) {
        if self.board.try_read(id).and_then(|s| s.realm.as_deref()) != Some(realm) {
            self.peers.get_mut(&id).unwrap().observer_views_invalidated = true;
        }
        PeerSnapshotPublisher {
            board: &mut self.board,
            grids: &mut self.grid,
            encoder: &self.encoder,
        }
        .publish_teleport(
            id,
            10,
            TeleportRequest {
                parcel_index: parcel,
                position_x: spec::POSITION_X.encode(local.x),
                position_y: spec::POSITION_Y.encode(local.y),
                position_z: spec::POSITION_Z.encode(local.z),
                realm: realm.into(),
            },
        );
    }

    fn input(&mut self, id: u32, parcel: i32, local: Vector3) {
        let state = PlayerState {
            parcel_index: parcel,
            position_x: spec::POSITION_X.encode(local.x),
            position_y: spec::POSITION_Y.encode(local.y),
            position_z: spec::POSITION_Z.encode(local.z),
            ..Default::default()
        };
        self.publish_state(id, state);
    }

    fn publish_state(&mut self, id: u32, state: PlayerState) {
        PeerSnapshotPublisher {
            board: &mut self.board,
            grids: &mut self.grid,
            encoder: &self.encoder,
        }
        .publish_from_player_state(id, 20, &state, None, None);
    }
}

fn state_at(x: f32) -> PlayerState {
    PlayerState {
        parcel_index: 0,
        position_x: spec::POSITION_X.encode(x),
        position_z: spec::POSITION_Z.encode(8.0),
        ..Default::default()
    }
}

fn listener_state(aoi: &[(&str, &[i32])]) -> SceneListenerState {
    let encoder = ParcelEncoder::new(ParcelEncoderOptions::default());
    let mapper =
        SceneListenerCellMapper::new(&RealmSpatialGrids::new(SPATIAL_GRID_CELL_SIZE, 1), &encoder);
    SceneListenerState::from_parcels(
        aoi.iter()
            .map(|(realm, parcels)| (realm.to_string(), parcels.iter().copied().collect()))
            .collect(),
        &encoder,
        &mapper,
    )
}

fn first_sweep_tick_after(stamped_tick: u32) -> u32 {
    let earliest = stamped_tick + VIEW_STALE_TICKS + 1;
    let remainder = earliest % SWEEP_CHECK_INTERVAL;
    if remainder == 0 {
        earliest
    } else {
        earliest + (SWEEP_CHECK_INTERVAL - remainder)
    }
}

fn left_for(out: &[OutgoingMessage], target: u32, subject: u32) -> bool {
    out.iter().any(|m| {
        m.target == target
            && matches!(&m.message.message,
                Some(server_message::Message::PlayerLeft(l)) if l.subject_id == subject)
    })
}

fn tick(sim: &mut PeerSimulation, w: &mut World, tick_counter: u32) -> Vec<OutgoingMessage> {
    sim.outbox.clear();

    sim.simulate_tick(
        &mut w.peers,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &w.grid,
        &w.aoi,
        tick_counter,
        0,
    );
    std::mem::take(&mut sim.outbox)
}

#[test]
fn full_state_on_join_then_delta_after() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");

    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);

    let out = tick(&mut sim, &mut w, 1);
    let joined: Vec<_> = out
        .iter()
        .filter(|m| m.target == 0)
        .filter(|m| {
            matches!(
                &m.message.message,
                Some(server_message::Message::PlayerJoined(_))
            )
        })
        .collect();
    assert_eq!(
        joined.len(),
        1,
        "first visibility sends exactly one PlayerJoined"
    );
    match &joined[0].message.message {
        Some(server_message::Message::PlayerJoined(pj)) => {
            assert_eq!(pj.user_id, "0xsubject");
            assert_eq!(pj.realm, "realm-a", "PlayerJoined carries subject realm");
            let full = pj.state.as_ref().unwrap();
            assert_eq!(full.subject_id, 1);

            assert_eq!(full.server_tick, 10);
        }
        _ => unreachable!(),
    }
    assert_eq!(joined[0].mode, PacketMode::Reliable);

    w.input(1, 0, v3(10.0, 8.0));
    let out = tick(&mut sim, &mut w, 2);
    let to_obs: Vec<_> = out.iter().filter(|m| m.target == 0).collect();
    assert!(
        to_obs.iter().any(|m| matches!(&m.message.message, Some(server_message::Message::PlayerStateDelta(d)) if d.subject_id == 1)),
        "subsequent updates are deltas, got {to_obs:?}"
    );
    assert!(
        !to_obs.iter().any(|m| matches!(
            &m.message.message,
            Some(server_message::Message::PlayerJoined(_))
        )),
        "no second PlayerJoined"
    );

    let delta = to_obs
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerStateDelta(d)) => Some(d),
            _ => None,
        })
        .unwrap();
    assert_eq!(delta.new_seq, w.board.last_seq(1));
    assert!(delta.new_seq > delta.baseline_seq);
}

#[test]
fn full_state_carries_the_jump_count() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");
    w.publish_state(
        1,
        PlayerState {
            jump_count: 2,
            ..state_at(9.0)
        },
    );

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let out = tick(&mut sim, &mut w, 1);
    let joined = out
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerJoined(pj)) if m.target == 0 => {
                pj.state.as_ref().and_then(|full| full.state.as_ref())
            }
            _ => None,
        })
        .expect("join expected");
    assert_eq!(
        joined.jump_count, 2,
        "the full state relays the snapshot's jump count"
    );

    w.peers.get_mut(&0).unwrap().request_resync(1, 0);
    let out = tick(&mut sim, &mut w, 2);
    let full = out
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerStateFull(f)) if m.target == 0 => f.state.as_ref(),
            _ => None,
        })
        .expect("resync full state expected");
    assert_eq!(full.jump_count, 2, "a resync relays it too");
}

#[test]
fn out_of_interest_subject_is_invisible() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xfaraway");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");

    w.teleport(1, 0, v3(8.0, 8.0), "realm-a");
    w.input(1, 5000, v3(8.0, 8.0));

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let out = tick(&mut sim, &mut w, 1);
    assert!(
        !out.iter().any(|m| m.target == 0
            && matches!(
                &m.message.message,
                Some(server_message::Message::PlayerJoined(_))
            )),
        "a subject outside the interest radius never produces a join"
    );
}

#[test]
fn stale_view_survives_the_staleness_threshold_then_swept_within_one_check_interval() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let out = tick(&mut sim, &mut w, 0);
    assert!(joined_for(&out, 0, "0xsubject"));

    w.input(1, 5000, v3(8.0, 8.0));

    for t in 1..=VIEW_STALE_TICKS {
        let out = tick(&mut sim, &mut w, t);
        assert!(
            !left_for(&out, 0, 1),
            "a view exactly VIEW_STALE_TICKS ({VIEW_STALE_TICKS}) ticks old must not be swept yet (tick {t})"
        );
    }

    let mut swept_at = None;
    for t in VIEW_STALE_TICKS + 1..=VIEW_STALE_TICKS + SWEEP_CHECK_INTERVAL {
        if left_for(&tick(&mut sim, &mut w, t), 0, 1) {
            swept_at = Some(t);
            break;
        }
    }
    assert_eq!(
        swept_at,
        Some(first_sweep_tick_after(0)),
        "a stale view must be swept within SWEEP_CHECK_INTERVAL ticks of crossing VIEW_STALE_TICKS"
    );
}

#[test]
fn a_peer_reconnecting_into_its_freed_slot_is_joined_again() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    assert!(joined_for(&tick(&mut sim, &mut w, 1), 0, "0xsubject"));

    w.peers.remove(&1);
    w.board.clear_active(1);
    w.grid.remove(1);
    w.identity.remove(1);
    w.profiles.remove(1);
    sim.cleanup_observer_views(1);
    let _ = tick(&mut sim, &mut w, 2);

    w.connect(1, "0xsubject");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");
    assert!(
        joined_for(&tick(&mut sim, &mut w, 3), 0, "0xsubject"),
        "the same wallet back in the same slot is a new subject to its observers"
    );
}

#[test]
fn resync_request_recovers_with_full_state() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);

    let _ = tick(&mut sim, &mut w, 1);

    w.input(1, 0, v3(10.0, 8.0));

    w.peers.get_mut(&0).unwrap().request_resync(1, 0);
    let out = tick(&mut sim, &mut w, 2);

    let full: Vec<_> = out
        .iter()
        .filter(|m| m.target == 0)
        .filter_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerStateFull(f)) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(
        full.len(),
        1,
        "resync fallback delivers exactly one STATE_FULL"
    );
    assert_eq!(full[0].subject_id, 1);
    assert_eq!(full[0].sequence, w.board.last_seq(1));

    let msg = out
        .iter()
        .find(|m| {
            matches!(
                &m.message.message,
                Some(server_message::Message::PlayerStateFull(_))
            )
        })
        .unwrap();
    assert_eq!(msg.mode, PacketMode::Reliable);
}

#[test]
fn resync_request_recovers_with_targeted_delta_when_baseline_known() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], true);

    let _ = tick(&mut sim, &mut w, 1);

    w.input(1, 0, v3(10.0, 8.0));
    w.input(1, 0, v3(11.0, 8.0));
    let known_seq = 1;

    w.peers.get_mut(&0).unwrap().request_resync(1, known_seq);
    let out = tick(&mut sim, &mut w, 2);

    let delta = out.iter().find_map(|m| match &m.message.message {
        Some(server_message::Message::PlayerStateDelta(d)) if m.target == 0 => Some((d, m.mode)),
        _ => None,
    });
    let (delta, mode) = delta.expect("targeted resync delta expected");
    assert_eq!(
        delta.baseline_seq, known_seq,
        "delta is from the client's known baseline"
    );
    assert_eq!(delta.new_seq, w.board.last_seq(1));
    assert_eq!(mode, PacketMode::Reliable, "resync delta is reliable");

    assert!(
        !out.iter().any(|m| matches!(
            &m.message.message,
            Some(server_message::Message::PlayerStateFull(_))
        )),
        "known baseline avoids the STATE_FULL fallback"
    );
}

#[test]
fn teleport_is_broadcast_to_observer() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    w.teleport(1, 0, v3(12.0, 8.0), "realm-a");
    let out = tick(&mut sim, &mut w, 2);

    let tp = out.iter().find_map(|m| match &m.message.message {
        Some(server_message::Message::Teleported(t)) if m.target == 0 => Some(t),
        _ => None,
    });
    let tp = tp.expect("teleport broadcast expected");
    assert_eq!(tp.subject_id, 1);
    assert_eq!(tp.sequence, w.board.last_seq(1));
    assert_eq!(tp.realm, "realm-a", "TeleportPerformed carries peer realm");
}

#[test]
fn baseline_coteleport_reseeds_destination_identity() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    w.teleport(0, 0, v3(8.0, 8.0), "realm-b");
    w.teleport(1, 0, v3(12.0, 8.0), "realm-b");
    let out = tick(&mut sim, &mut w, 2);

    let tp = out
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerJoined(t)) if m.target == 0 => Some(t),
            _ => None,
        })
        .expect("destination identity expected");
    assert_eq!(tp.realm, "realm-b", "re-stamped with the destination realm");
}

#[test]
fn teleport_state_relays_raw_position_codes() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    w.teleport(1, 0, v3(12.0, 8.0), "realm-a");
    let out = tick(&mut sim, &mut w, 2);

    let tp = out
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::Teleported(t)) if m.target == 0 => Some(t),
            _ => None,
        })
        .expect("teleport broadcast expected");
    let state = tp.state.as_ref().unwrap();
    assert_eq!(state.position_x, spec::POSITION_X.encode(12.0));
    assert_eq!(state.position_z, spec::POSITION_Z.encode(8.0));
}

#[test]
fn tier_divisors_gate_far_subjects() {
    let sim = PeerSimulation::new(&[50, 100, 200], false);
    assert_eq!(sim.tier_divisors, vec![1, 2, 4]);
}

#[test]
fn delta_baseline_seq_detects_gap() {
    let from = PeerSnapshot {
        seq: 3,
        ..Default::default()
    };
    let to = PeerSnapshot {
        seq: 7,
        position_x: spec::POSITION_X.encode(1.0),
        ..Default::default()
    };
    let d = create_delta_message(1, &from, &to, PeerViewSimulationTier::TIER_0);
    assert_eq!(d.baseline_seq, 3);
    assert_eq!(d.new_seq, 7);
    assert_eq!(
        d.position_x,
        Some(to.position_x),
        "changed field carries the raw code"
    );
}

#[test]
fn delta_included_only_when_code_changes() {
    let from = PeerSnapshot {
        seq: 0,
        position_x: 100,
        rotation_y: 64,
        ..Default::default()
    };
    let to = PeerSnapshot {
        seq: 1,
        position_x: 100,
        rotation_y: 65,
        ..Default::default()
    };
    let d = create_delta_message(1, &from, &to, PeerViewSimulationTier::TIER_0);
    assert!(d.position_x.is_none(), "unchanged code omitted");
    assert_eq!(d.rotation_y, Some(65), "changed code included verbatim");
}

#[test]
fn tier2_omits_velocity_and_blend() {
    let from = PeerSnapshot {
        seq: 0,
        ..Default::default()
    };
    let to = PeerSnapshot {
        seq: 1,
        velocity_x: spec::VELOCITY_X.encode(5.0),
        movement_blend: spec::MOVEMENT_BLEND.encode(2.0),
        position_x: spec::POSITION_X.encode(1.0),
        ..Default::default()
    };
    let d2 = create_delta_message(1, &from, &to, PeerViewSimulationTier::TIER_2);
    assert!(d2.velocity_x.is_none(), "TIER_2 omits velocity");
    assert!(d2.movement_blend.is_none(), "TIER_2 omits movement blend");
    assert!(
        d2.position_x.is_some(),
        "TIER_2 still carries spatial position"
    );

    let d0 = create_delta_message(1, &from, &to, PeerViewSimulationTier::TIER_0);
    assert!(d0.velocity_x.is_some(), "TIER_0 includes velocity");
    assert!(d0.movement_blend.is_some());
}

#[test]
fn promotion_to_a_closer_tier_repairs_the_tier_gated_fields() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 5, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");
    w.publish_state(
        1,
        PlayerState {
            velocity_x: 100,
            movement_blend: 10,
            slide_blend: 3,
            head_yaw: Some(20),
            head_pitch: Some(30),
            ..state_at(9.0)
        },
    );

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    assert!(joined_for(&tick(&mut sim, &mut w, 4), 0, "0xsubject"));

    w.publish_state(
        1,
        PlayerState {
            velocity_x: 200,
            movement_blend: 20,
            slide_blend: 9,
            head_yaw: Some(40),
            head_pitch: Some(50),
            ..state_at(9.0)
        },
    );
    let out = tick(&mut sim, &mut w, 8);
    let far = out
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerStateDelta(d)) if m.target == 0 => Some(d),
            _ => None,
        })
        .expect("a TIER_2 delta expected");
    assert!(
        far.velocity_x.is_none() && far.movement_blend.is_none() && far.head_yaw.is_none(),
        "TIER_2 withholds the gated fields, got {far:?}"
    );

    w.input(0, 0, v3(8.0, 8.0));
    let out = tick(&mut sim, &mut w, 9);
    let (full, mode) = out
        .iter()
        .find_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerStateFull(f)) if m.target == 0 => Some((f, m.mode)),
            _ => None,
        })
        .expect("a promoted observer is sent the subject's full state");
    assert_eq!(mode, PacketMode::Reliable);
    assert_eq!(full.subject_id, 1);
    assert_eq!(full.sequence, w.board.last_seq(1));
    let state = full.state.as_ref().unwrap();
    assert_eq!(
        (
            state.velocity_x,
            state.movement_blend,
            state.slide_blend,
            state.head_yaw,
            state.head_pitch
        ),
        (200, 20, 9, Some(40), Some(50)),
        "the full state carries what the far tier withheld"
    );

    let out = tick(&mut sim, &mut w, 10);
    assert!(
        !out.iter().any(|m| m.target == 0
            && matches!(
                &m.message.message,
                Some(server_message::Message::PlayerStateFull(_))
            )),
        "the repair is sent once per promotion"
    );
}

#[test]
fn emote_start_then_stop_broadcast() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    let state = PlayerState {
        parcel_index: 0,
        position_x: spec::POSITION_X.encode(9.0),
        position_z: spec::POSITION_Z.encode(8.0),
        ..Default::default()
    };
    PeerSnapshotPublisher {
        board: &mut w.board,
        grids: &mut w.grid,
        encoder: &w.encoder,
    }
    .publish_from_player_state(
        1,
        30,
        &state,
        Some(crate::pulse::snapshot::EmoteInput {
            emote_id: "wave".into(),
            duration_ms: None,
            start_tick: None,
            mask: None,
        }),
        None,
    );
    let out = tick(&mut sim, &mut w, 2);
    let started = out.iter().find_map(|m| match &m.message.message {
        Some(server_message::Message::EmoteStarted(e)) if m.target == 0 => Some(e),
        _ => None,
    });
    assert_eq!(started.expect("emote started broadcast").emote_id, "wave");

    let active: EmoteState = w.board.try_read(1).unwrap().emote.clone().unwrap();
    let stop = PeerSnapshot {
        seq: w.board.last_seq(1) + 1,
        server_tick: 40,
        emote: Some(EmoteState {
            emote_id: None,
            start_seq: active.start_seq,
            start_tick: active.start_tick,
            duration_ms: None,
            mask: None,
            stop_reason: Some(EmoteStopReason::Cancelled),
        }),
        ..w.board.try_read(1).unwrap().clone()
    };
    w.board.publish(1, stop);
    let out = tick(&mut sim, &mut w, 3);
    let stopped = out.iter().find_map(|m| match &m.message.message {
        Some(server_message::Message::EmoteStopped(e)) if m.target == 0 => Some(e),
        _ => None,
    });
    assert_eq!(
        stopped.expect("emote stopped broadcast").reason,
        EmoteStopReason::Cancelled as i32
    );
}

#[test]
fn an_emote_stop_that_left_the_ring_is_still_delivered() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    PeerSnapshotPublisher {
        board: &mut w.board,
        grids: &mut w.grid,
        encoder: &w.encoder,
    }
    .publish_from_player_state(
        1,
        30,
        &state_at(9.0),
        Some(crate::pulse::snapshot::EmoteInput {
            emote_id: "dance".into(),
            duration_ms: None,
            start_tick: None,
            mask: None,
        }),
        None,
    );
    let out = tick(&mut sim, &mut w, 2);
    assert!(
        out.iter().any(|m| m.target == 0
            && matches!(
                &m.message.message,
                Some(server_message::Message::EmoteStarted(_))
            )),
        "the observer is shown the emote"
    );

    w.input(0, 5000, v3(8.0, 8.0));
    let _ = tick(&mut sim, &mut w, 3);

    let active: EmoteState = w.board.try_read(1).unwrap().emote.clone().unwrap();
    let stop = PeerSnapshot {
        seq: w.board.last_seq(1) + 1,
        server_tick: 40,
        emote: Some(EmoteState {
            emote_id: None,
            start_seq: active.start_seq,
            start_tick: active.start_tick,
            duration_ms: None,
            mask: None,
            stop_reason: Some(EmoteStopReason::Cancelled),
        }),
        ..w.board.try_read(1).unwrap().clone()
    };
    let stop_seq = stop.seq;
    w.board.publish(1, stop);
    for step in 0..20 {
        w.input(1, 0, v3(9.0 + (step % 2) as f32, 8.0));
    }
    assert!(
        w.board.try_read_seq(1, stop_seq).is_none(),
        "the stop snapshot has left the ring"
    );
    let _ = tick(&mut sim, &mut w, 4);

    w.input(0, 0, v3(8.0, 8.0));
    let mut stops = Vec::new();
    for t in 5..8 {
        for m in tick(&mut sim, &mut w, t) {
            if let Some(server_message::Message::EmoteStopped(e)) = m.message.message
                && m.target == 0
                && e.subject_id == 1
            {
                stops.push(e.sequence);
            }
        }
    }
    assert_eq!(
        stops,
        vec![w.board.last_seq(1)],
        "the returning observer is told once, from the subject's latest snapshot"
    );
}

#[test]
fn emote_mask_relays_verbatim_and_absence_stays_absent() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xmasked");
    w.connect(2, "0xplain");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");
    w.teleport(2, 0, v3(10.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    let state = |x: f32| PlayerState {
        parcel_index: 0,
        position_x: spec::POSITION_X.encode(x),
        position_z: spec::POSITION_Z.encode(8.0),
        ..Default::default()
    };
    for (id, x, mask) in [(1u32, 9.0f32, Some(5i32)), (2, 10.0, None)] {
        PeerSnapshotPublisher {
            board: &mut w.board,
            grids: &mut w.grid,
            encoder: &w.encoder,
        }
        .publish_from_player_state(
            id,
            30,
            &state(x),
            Some(crate::pulse::snapshot::EmoteInput {
                emote_id: "wave".into(),
                duration_ms: None,
                start_tick: None,
                mask,
            }),
            None,
        );
    }
    let out = tick(&mut sim, &mut w, 2);

    let started_mask = |subject: u32| {
        out.iter().find_map(|m| match &m.message.message {
            Some(server_message::Message::EmoteStarted(e))
                if m.target == 0 && e.subject_id == subject =>
            {
                Some(e.mask)
            }
            _ => None,
        })
    };
    assert_eq!(
        started_mask(1),
        Some(Some(5)),
        "the mask the client sent relays verbatim to the observer"
    );
    assert_eq!(
        started_mask(2),
        Some(None),
        "an absent mask stays absent, never coerced to 0"
    );
}

#[test]
fn profile_version_change_is_announced() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    w.profiles.set(1, 5);
    w.input(1, 0, v3(10.0, 8.0));
    let out = tick(&mut sim, &mut w, 2);
    let ann = out.iter().find_map(|m| match &m.message.message {
        Some(server_message::Message::PlayerProfileVersionAnnounced(a)) if m.target == 0 => Some(a),
        _ => None,
    });
    let ann = ann.expect("profile announcement expected");
    assert_eq!(ann.subject_id, 1);
    assert_eq!(ann.version, 5);
}

#[test]
fn v1_observer_gets_bit_packed_batch_v0_keeps_legacy_delta() {
    let mut w = World::new();
    w.connect(0, "0xv1obs");
    w.connect(1, "0xv0obs");
    w.connect(2, "0xsubject");
    w.peers.get_mut(&0).unwrap().features = crate::pulse::server::FEATURE_DELTA_BATCH;

    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(2, 0, v3(9.0, 8.0), "realm-a");

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    w.input(2, 0, v3(10.0, 8.0));
    let out = tick(&mut sim, &mut w, 2);

    let batch_msg = out
        .iter()
        .find(|m| {
            m.target == 0
                && matches!(
                    &m.message.message,
                    Some(server_message::Message::PlayerStateDeltaBatch(_))
                )
        })
        .expect("v1 observer receives a bit-packed batch");
    assert_eq!(batch_msg.mode, PacketMode::UnreliableSequenced);
    assert!(
        !out.iter().any(|m| m.target == 0
            && matches!(
                &m.message.message,
                Some(server_message::Message::PlayerStateDelta(_))
            )),
        "v1 observer gets no legacy per-message delta"
    );
    let Some(server_message::Message::PlayerStateDeltaBatch(batch)) = &batch_msg.message.message
    else {
        unreachable!()
    };
    assert_eq!(batch.subject_count, 1);
    let decoded =
        crate::pulse::batch::decode_batch(batch.subject_count, &batch.payload, |_| 0).unwrap();
    assert_eq!(decoded[0].subject_id, 2);
    assert_eq!(decoded[0].new_seq, w.board.last_seq(2));
    assert!(
        decoded[0].fields[1].is_some(),
        "position_x changed and is packed into the batch"
    );

    assert!(
        out.iter().any(|m| m.target == 1
            && matches!(
                &m.message.message,
                Some(server_message::Message::PlayerStateDelta(d)) if d.subject_id == 2
            )),
        "v0 observer keeps the legacy delta"
    );
    assert!(
        !out.iter().any(|m| m.target == 1
            && matches!(
                &m.message.message,
                Some(server_message::Message::PlayerStateDeltaBatch(_))
            )),
        "v0 observer never receives a batch"
    );
}

#[test]
fn pending_auth_peer_times_out() {
    let mut w = World::new();

    w.peers
        .insert(9, PeerState::new(PeerConnectionState::PendingAuth, 0));

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);

    sim.simulate_tick(
        &mut w.peers,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &w.grid,
        &w.aoi,
        1,
        DEFAULT_PENDING_AUTH_CLEAN_TIMEOUT_MS - 1,
    );
    assert!(
        sim.expired.is_empty(),
        "not expired before the auth budget elapses"
    );

    sim.simulate_tick(
        &mut w.peers,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &w.grid,
        &w.aoi,
        2,
        DEFAULT_PENDING_AUTH_CLEAN_TIMEOUT_MS,
    );
    assert_eq!(
        sim.expired,
        vec![ExpiredPeer {
            peer: 9,
            reason: ExpiredReason::AuthTimeout
        }]
    );
}

#[test]
fn baseline_batch_negotiation_is_per_observer_and_sparse_updates_stay_small() {
    for moving_subjects in [1, 8] {
        let mut world = World::sized(16);
        for id in 0..moving_subjects + 4 {
            world.connect(id, &format!("wallet-{id}"));
            world.teleport(id, 0, v3(8.0, 8.0), "realm");
        }
        world.peers.get_mut(&0).unwrap().features =
            crate::pulse::server::FEATURE_DELTA_BATCH_BASELINE;
        world.peers.get_mut(&2).unwrap().features = crate::pulse::server::FEATURE_DELTA_BATCH;
        world.peers.get_mut(&3).unwrap().features =
            crate::pulse::server::FEATURE_DELTA_BATCH_DICTIONARY;
        let mut simulation = PeerSimulation::new(&[50, 100, 200], false);
        tick(&mut simulation, &mut world, 1);
        for id in 4..moving_subjects + 4 {
            world.input(id, 0, v3(9.0, 8.0));
        }
        let messages = tick(&mut simulation, &mut world, 2);
        let selected: Vec<_> = messages
            .iter()
            .filter(|message| message.target == 0)
            .collect();
        let old: Vec<_> = messages
            .iter()
            .filter(|message| message.target == 1)
            .collect();
        assert!(!selected.is_empty());
        assert!(old.iter().all(|message| matches!(
            message.message.message,
            Some(server_message::Message::PlayerStateDelta(_))
        )));
        assert!(
            messages
                .iter()
                .filter(|message| message.target == 2)
                .all(|message| matches!(
                    message.message.message,
                    Some(server_message::Message::PlayerStateDeltaBatch(_))
                ))
        );
        if moving_subjects == 1 {
            assert!(selected.iter().all(|message| matches!(
                message.message.message,
                Some(server_message::Message::PlayerStateDelta(_))
            )));
        } else {
            assert!(selected.iter().all(|message| matches!(
                message.message.message,
                Some(server_message::Message::PlayerStateDeltaBatchBaseline(_))
            )));
        }
        let dictionary: Vec<_> = messages
            .iter()
            .filter(|message| message.target == 3)
            .collect();
        for message in &selected {
            if let Some(server_message::Message::PlayerStateDeltaBatchBaseline(batch)) =
                &message.message.message
            {
                assert_ne!(
                    batch.payload[0] & 128,
                    0,
                    "baseline-only observer never receives a dictionary"
                );
            }
        }
        if moving_subjects > 1 {
            assert!(dictionary.iter().any(|message| matches!(&message.message.message,
                Some(server_message::Message::PlayerStateDeltaBatchBaseline(batch)) if batch.payload[0] & 128 == 0)));
        }
        assert!(
            dictionary
                .iter()
                .map(|message| message.message.encoded_len())
                .sum::<usize>()
                <= selected
                    .iter()
                    .map(|message| message.message.encoded_len())
                    .sum::<usize>()
        );
        assert!(
            selected
                .iter()
                .map(|message| message.message.encoded_len())
                .sum::<usize>()
                <= old
                    .iter()
                    .map(|message| message.message.encoded_len())
                    .sum::<usize>()
        );
    }
}

#[test]
fn disconnecting_peer_cleaned_after_grace() {
    let mut w = World::new();
    let mut st = PeerState::new(PeerConnectionState::Disconnecting, 0);
    st.disconnection_time = 1_000;
    w.peers.insert(4, st);

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);

    sim.simulate_tick(
        &mut w.peers,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &w.grid,
        &w.aoi,
        1,
        1_000 + DEFAULT_DISCONNECTION_CLEAN_TIMEOUT_MS - 1,
    );
    assert!(sim.expired.is_empty());

    sim.simulate_tick(
        &mut w.peers,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &w.grid,
        &w.aoi,
        2,
        1_000 + DEFAULT_DISCONNECTION_CLEAN_TIMEOUT_MS,
    );
    assert_eq!(
        sim.expired,
        vec![ExpiredPeer {
            peer: 4,
            reason: ExpiredReason::DisconnectCleanTimeout
        }]
    );
}

const P_IN: i32 = 100;
const P_IN2: i32 = 101;
const P_OUT: i32 = 500;

fn joined_for(out: &[OutgoingMessage], target: u32, uid: &str) -> bool {
    out.iter().any(|m| {
        m.target == target
            && matches!(&m.message.message,
                Some(server_message::Message::PlayerJoined(pj)) if pj.user_id == uid)
    })
}

#[test]
fn shared_baseline_encodes_once() {
    const M: u32 = 50;

    let mut w = World::sized((M + 2) as usize);
    w.connect(0, "0xsubject");
    w.teleport(0, P_IN, v3(8.0, 8.0), "r");
    for id in 1..=M {
        w.connect_listener(id, &format!("0xlistener{id}"), "r", &[P_IN]);
    }

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);

    let _ = tick(&mut sim, &mut w, 1);
    w.input(0, P_IN, v3(9.0, 8.0));

    SCAN_CALLS.with(|c| c.set(0));
    ENCODE_CALLS.with(|c| c.set(0));
    let out = tick(&mut sim, &mut w, 2);

    assert_eq!(
        SCAN_CALLS.with(|c| c.get()),
        1,
        "scan_intermediate_events must be memoized to one call for the shared subject/baseline"
    );
    assert_eq!(
        ENCODE_CALLS.with(|c| c.get()),
        1,
        "create_delta_message must be memoized to one encode for the shared subject/baseline/tier"
    );

    let recipients = (1..=M)
        .filter(|&obs| {
            out.iter().any(|m| {
                m.target == obs
                    && matches!(
                        &m.message.message,
                        Some(server_message::Message::PlayerStateDelta(d)) if d.subject_id == 0
                    )
            })
        })
        .count();
    assert_eq!(
        recipients as u32, M,
        "all M observers must still receive their own delta for the subject"
    );
}

#[test]
fn a_realm_less_observer_is_not_mirrored_to_itself() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.board.publish(
        0,
        crate::pulse::snapshot::PeerSnapshot {
            global_position: v3(8.0, 8.0),
            ..Default::default()
        },
    );

    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    sim.self_mirror_enabled = true;

    let out = tick(&mut sim, &mut w, 1);
    assert!(
        out.iter().all(|m| m.target != 0),
        "a peer with no realm is in no grid, so it is not its own subject either"
    );

    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    let out = tick(&mut sim, &mut w, 2);
    assert!(
        out.iter().any(|m| m.target == 0),
        "once the observer has a realm the mirror is emitted"
    );
}

fn emote_then_teleport(w: &mut World) -> (u32, u32) {
    let started = PeerSnapshotPublisher {
        board: &mut w.board,
        grids: &mut w.grid,
        encoder: &w.encoder,
    }
    .publish_from_player_state(
        1,
        30,
        &state_at(9.0),
        Some(crate::pulse::snapshot::EmoteInput {
            emote_id: "wave".into(),
            duration_ms: None,
            start_tick: None,
            mask: None,
        }),
        None,
    );
    w.teleport(1, 0, v3(10.0, 8.0), "realm-a");
    (started.seq, w.board.last_seq(1))
}

fn discrete_events(out: &[OutgoingMessage], target: u32) -> Vec<(&'static str, u32)> {
    out.iter()
        .filter(|m| m.target == target)
        .filter_map(|m| match &m.message.message {
            Some(server_message::Message::Teleported(t)) => Some(("teleported", t.sequence)),
            Some(server_message::Message::EmoteStarted(e)) => Some(("emote-started", e.sequence)),
            Some(server_message::Message::EmoteStopped(e)) => Some(("emote-stopped", e.sequence)),
            _ => None,
        })
        .collect()
}

#[test]
fn an_emote_started_before_a_teleport_in_one_scan_follows_the_snap_and_stays_active() {
    let mut w = World::new();
    w.connect(0, "0xobserver");
    w.connect(1, "0xsubject");
    w.teleport(0, 0, v3(8.0, 8.0), "realm-a");
    w.teleport(1, 0, v3(9.0, 8.0), "realm-a");
    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    let _ = tick(&mut sim, &mut w, 1);

    let (started, teleported) = emote_then_teleport(&mut w);
    let out = tick(&mut sim, &mut w, 2);
    assert_eq!(
        discrete_events(&out, 0),
        vec![("teleported", teleported), ("emote-started", started)]
    );

    w.input(1, 0, v3(11.0, 8.0));
    let out = tick(&mut sim, &mut w, 3);
    assert_eq!(discrete_events(&out, 0), vec![]);
    let baselines: Vec<u32> = out
        .iter()
        .filter(|m| m.target == 0)
        .filter_map(|m| match &m.message.message {
            Some(server_message::Message::PlayerStateDelta(d)) if d.subject_id == 1 => {
                Some(d.baseline_seq)
            }
            _ => None,
        })
        .collect();
    assert_eq!(baselines, vec![teleported]);
}

#[test]
fn a_batched_delta_carries_the_tick_of_its_own_sample() {
    let mut world = World::sized(16);
    for id in 0..10 {
        world.connect(id, &format!("wallet-{id}"));
        world.teleport(id, 0, v3(8.0, 8.0), "realm");
    }
    world.peers.get_mut(&0).unwrap().features = crate::pulse::server::FEATURE_DELTA_BATCH_BASELINE
        | crate::pulse::server::FEATURE_DELTA_BATCH_DICTIONARY
        | crate::pulse::server::FEATURE_DELTA_BATCH_SAMPLE_TICK;
    let mut simulation = PeerSimulation::new(&[50, 100, 200], false);
    tick(&mut simulation, &mut world, 1);
    for id in 2..10 {
        PeerSnapshotPublisher {
            board: &mut world.board,
            grids: &mut world.grid,
            encoder: &world.encoder,
        }
        .publish_from_player_state(id, 20 + id, &state_at(9.0), None, None);
    }
    simulation.outbox.clear();
    simulation.simulate_tick(
        &mut world.peers,
        TickBoards::new(&world.board, &world.identity, &world.profiles),
        &world.grid,
        &world.aoi,
        2,
        60,
    );
    let out = std::mem::take(&mut simulation.outbox);
    let mut ticks = Vec::new();
    for message in out.iter().filter(|message| message.target == 0) {
        if let Some(server_message::Message::PlayerStateDeltaBatchBaseline(batch)) =
            &message.message.message
        {
            assert_eq!(batch.server_tick, 60);
            let subjects = crate::pulse::batch::decode_baseline_batch_with(
                batch.server_tick,
                batch.subject_count,
                &batch.payload,
                true,
            )
            .unwrap();
            for subject in subjects {
                ticks.push((
                    subject.subject_id,
                    subject.to_delta(subject.sample_tick).server_tick,
                ));
            }
        }
    }
    ticks.sort();
    let sampled: Vec<(u32, u32)> = (2..10)
        .map(|id| (id, world.board.try_read(id).unwrap().server_tick))
        .collect();
    assert_eq!(
        ticks, sampled,
        "every batched delta is stamped with the tick of the sample it carries"
    );
}

#[test]
fn a_sample_tick_bit_without_an_arm_11_codec_leaves_the_batch_untouched() {
    let mut world = World::new();
    for id in 0..3 {
        world.connect(id, &format!("wallet-{id}"));
        world.teleport(id, 0, v3(8.0, 8.0), "realm");
    }
    world.peers.get_mut(&0).unwrap().features = crate::pulse::server::FEATURE_DELTA_BATCH;
    world.peers.get_mut(&1).unwrap().features = crate::pulse::server::FEATURE_DELTA_BATCH
        | crate::pulse::server::FEATURE_DELTA_BATCH_SAMPLE_TICK;
    let mut simulation = PeerSimulation::new(&[50, 100, 200], false);
    tick(&mut simulation, &mut world, 1);
    world.input(2, 0, v3(9.0, 8.0));
    let out = tick(&mut simulation, &mut world, 2);
    let batch_for = |target: u32| {
        out.iter()
            .filter(|message| message.target == target)
            .find_map(|message| match &message.message.message {
                Some(server_message::Message::PlayerStateDeltaBatch(batch)) => Some(batch.clone()),
                _ => None,
            })
            .expect("an arm 10 batch")
    };
    assert_eq!(batch_for(1), batch_for(0));
    let batch = batch_for(1);
    let decoded =
        crate::pulse::batch::decode_batch(batch.subject_count, &batch.payload, |_| 0).unwrap();
    assert_eq!(decoded[0].new_seq, world.board.last_seq(2));
}

#[test]
fn baseline_retained_subject_realm_change_reseeds_current_identity() {
    let mut w = World::new();
    w.connect_listener_multi(0, "listener", &[("a", &[0]), ("b", &[0])]);
    w.connect(1, "subject");
    w.teleport(1, 0, v3(9.0, 8.0), "a");
    let mut sim = PeerSimulation::new(&[50, 100, 200], true);
    tick(&mut sim, &mut w, 1);
    w.profiles.set(1, 7);
    w.peers.get_mut(&0).unwrap().request_resync(1, 0);
    w.teleport(1, 0, v3(10.0, 8.0), "b");
    let out = tick(&mut sim, &mut w, 2);
    let messages: Vec<_> = out
        .iter()
        .filter(|m| m.target == 0)
        .map(|m| m.message.message.as_ref().unwrap())
        .collect();
    assert_eq!(messages.len(), 2);
    assert!(matches!(messages[0], server_message::Message::PlayerLeft(l) if l.subject_id == 1));
    assert!(
        matches!(messages[1], server_message::Message::PlayerJoined(j) if j.realm == "b" && j.profile_version == 7)
    );
    let baseline = w.board.last_seq(1);
    w.input(1, 0, v3(11.0, 8.0));
    assert!(tick(&mut sim, &mut w, 3).iter().any(|m| matches!(&m.message.message, Some(server_message::Message::PlayerStateDelta(d)) if m.target == 0 && d.baseline_seq == baseline && d.position_x == Some(spec::POSITION_X.encode(11.0)))));
}

#[test]
fn baseline_foreign_emote_events_use_destination_pose_and_following_delta() {
    for stop in [false, true] {
        let mut w = World::new();
        w.connect(0, "observer");
        w.connect(1, "subject");
        w.teleport(0, 0, v3(8.0, 8.0), "a");
        w.teleport(1, 0, v3(9.0, 8.0), "a");
        let mut sim = PeerSimulation::new(&[50, 100, 200], false);
        tick(&mut sim, &mut w, 1);
        if stop {
            emote_then_teleport(&mut w);
            w.teleport(1, 0, v3(9.0, 8.0), "a");
            tick(&mut sim, &mut w, 2);
        }
        w.teleport(1, 0, v3(1.0, 8.0), "b");
        let seq = w.board.last_seq(1) + 1;
        w.board.publish(
            1,
            PeerSnapshot {
                seq,
                server_tick: 77,
                position_x: spec::POSITION_X.encode(2.0),
                emote: Some(EmoteState {
                    emote_id: if stop { None } else { Some("wave".into()) },
                    start_seq: seq,
                    start_tick: 77,
                    duration_ms: None,
                    mask: Some(3),
                    stop_reason: if stop {
                        Some(EmoteStopReason::Cancelled)
                    } else {
                        None
                    },
                }),
                ..Default::default()
            },
        );
        w.teleport(1, 0, v3(10.0, 8.0), "a");
        let destination = w.board.last_seq(1);
        let out = tick(&mut sim, &mut w, 3);
        assert!(!left_for(&out, 0, 1), "subject round trip retains identity");
        let event = out
            .iter()
            .filter(|m| m.target == 0)
            .find_map(|m| match &m.message.message {
                Some(server_message::Message::EmoteStarted(e)) if !stop => {
                    assert_eq!(e.server_tick, 77);
                    assert_eq!(e.mask, Some(3));
                    Some((e.sequence, e.player_state.as_ref().unwrap()))
                }
                Some(server_message::Message::EmoteStopped(e)) if stop => {
                    Some((e.sequence, e.player_state.as_ref().unwrap()))
                }
                _ => None,
            })
            .expect("intermediate event is preserved");
        assert_eq!(event.0, destination);
        assert_eq!(event.1.position_x, spec::POSITION_X.encode(10.0));
        w.input(1, 0, v3(11.0, 8.0));
        assert!(tick(&mut sim, &mut w, 4).iter().any(|m| m.target == 0 && matches!(&m.message.message, Some(server_message::Message::PlayerStateDelta(d)) if d.baseline_seq == destination && d.position_x == Some(spec::POSITION_X.encode(11.0)))));
    }
}

#[test]
fn baseline_observer_realm_matrix_preserves_identity_and_self_mirror() {
    for (round_trip, mirror) in [(false, false), (true, false), (true, true)] {
        let mut w = World::new();
        w.connect(0, "observer");
        w.connect(1, "subject");
        w.teleport(0, 0, v3(8.0, 8.0), "a");
        w.teleport(1, 0, v3(9.0, 8.0), "a");
        let mut sim = PeerSimulation::new(&[50, 100, 200], true);
        sim.self_mirror_enabled = mirror;
        tick(&mut sim, &mut w, 1);
        if round_trip {
            w.teleport(0, 0, v3(8.0, 8.0), "b");
        }
        w.teleport(0, 0, v3(8.0, 8.0), "a");
        let out = tick(&mut sim, &mut w, 2);
        let target: Vec<_> = out.iter().filter(|m| m.target == 0).collect();
        let left = target
            .iter()
            .filter(|m| {
                matches!(
                    m.message.message,
                    Some(server_message::Message::PlayerLeft(_))
                )
            })
            .count();
        let joined = target
            .iter()
            .filter(|m| {
                matches!(
                    m.message.message,
                    Some(server_message::Message::PlayerJoined(_))
                )
            })
            .count();
        let expected = if round_trip {
            if mirror { 2 } else { 1 }
        } else {
            0
        };
        assert_eq!((left, joined), (expected, expected));
        if mirror {
            assert!(target.iter().any(|m| matches!(&m.message.message, Some(server_message::Message::PlayerJoined(j)) if j.user_id == SELF_MIRROR_WALLET_ID)));
            assert_eq!(sim.observer_views[&0][&0].last_seen_tick, 2);
        }
        let next = tick(&mut sim, &mut w, 3);
        assert!(!next.iter().any(|m| m.target == 0
            && matches!(
                m.message.message,
                Some(server_message::Message::PlayerLeft(_))
                    | Some(server_message::Message::PlayerJoined(_))
            )));
    }
}

#[test]
fn baseline_retained_realm_reseeds_active_emote_after_history_eviction() {
    let mut w = World::new();
    w.connect_listener_multi(0, "listener", &[("a", &[0]), ("b", &[0])]);
    w.connect(1, "subject");
    w.teleport(1, 0, v3(9.0, 8.0), "a");
    let mut sim = PeerSimulation::new(&[50, 100, 200], true);
    tick(&mut sim, &mut w, 1);
    emote_then_teleport(&mut w);
    w.teleport(1, 0, v3(10.0, 8.0), "b");
    for _ in 0..20 {
        w.input(1, 0, v3(10.0, 8.0));
    }
    w.profiles.set(1, 9);
    w.peers.get_mut(&0).unwrap().request_resync(1, 0);
    let out = tick(&mut sim, &mut w, 2);
    let messages: Vec<_> = out
        .iter()
        .filter(|m| m.target == 0)
        .map(|m| m.message.message.as_ref().unwrap())
        .collect();
    assert_eq!(messages.len(), 3);
    assert!(matches!(
        messages[0],
        server_message::Message::PlayerLeft(_)
    ));
    assert!(
        matches!(messages[1], server_message::Message::PlayerJoined(j) if j.profile_version == 9 && j.realm == "b")
    );
    assert!(
        matches!(messages[2], server_message::Message::EmoteStarted(e) if e.emote_id == "wave" && e.sequence == w.board.last_seq(1))
    );
    assert!(
        w.board.try_read(0).is_none(),
        "listener remains absent from player snapshot/grid population"
    );
}

#[test]
fn baseline_evicted_foreign_stop_reconciles_idle_and_new_start_supersedes_stop() {
    for (evict, restart) in [(true, false), (false, true)] {
        let mut w = World::new();
        w.connect(0, "observer");
        w.connect(1, "subject");
        w.teleport(0, 0, v3(8.0, 8.0), "a");
        w.teleport(1, 0, v3(9.0, 8.0), "a");
        let mut sim = PeerSimulation::new(&[50, 100, 200], false);
        tick(&mut sim, &mut w, 1);
        emote_then_teleport(&mut w);
        w.teleport(1, 0, v3(9.0, 8.0), "a");
        tick(&mut sim, &mut w, 2);
        w.teleport(1, 0, v3(1.0, 8.0), "b");
        let seq = w.board.last_seq(1) + 1;
        w.board.publish(
            1,
            PeerSnapshot {
                seq,
                emote: Some(EmoteState {
                    emote_id: None,
                    start_seq: seq,
                    start_tick: 30,
                    duration_ms: None,
                    mask: None,
                    stop_reason: Some(EmoteStopReason::Completed),
                }),
                ..Default::default()
            },
        );
        w.teleport(1, 0, v3(10.0, 8.0), "a");
        if evict {
            for _ in 0..20 {
                w.input(1, 0, v3(10.0, 8.0));
            }
        }
        if restart {
            emote_then_teleport(&mut w);
            w.teleport(1, 0, v3(10.0, 8.0), "a");
        }
        let out = tick(&mut sim, &mut w, 3);
        let events = discrete_events(&out, 0);
        assert_eq!(
            events
                .iter()
                .filter(|(kind, _)| *kind == "emote-started")
                .count(),
            usize::from(restart)
        );
        assert_eq!(
            events
                .iter()
                .filter(|(kind, _)| *kind == "emote-stopped")
                .count(),
            usize::from(!restart)
        );
        if evict {
            assert!(out.iter().any(|m| m.target == 0 && matches!(&m.message.message, Some(server_message::Message::EmoteStopped(e)) if e.reason == EmoteStopReason::Cancelled as i32 && e.sequence == w.board.last_seq(1))));
        }
        let baseline = w.board.last_seq(1);
        w.input(1, 0, v3(11.0, 8.0));
        assert!(tick(&mut sim, &mut w, 4).iter().any(|m| m.target == 0 && matches!(&m.message.message, Some(server_message::Message::PlayerStateDelta(d)) if d.baseline_seq == baseline)));
    }
}

#[test]
fn baseline_realm_retirement_precedes_tier_pacing() {
    let mut w = World::new();
    w.connect_listener_multi(0, "listener", &[("a", &[0]), ("b", &[0])]);
    w.connect(1, "subject");
    w.teleport(1, 0, v3(9.0, 8.0), "a");
    let mut sim = PeerSimulation::new(&[50, 100, 200], false);
    tick(&mut sim, &mut w, 4);
    w.teleport(1, 0, v3(10.0, 8.0), "b");
    let mut collector = InterestCollector::default();
    collector.add(1, PeerViewSimulationTier::TIER_2);
    sim.process_visible_subjects(
        0,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &collector,
        None,
        5,
        true,
    );
    assert!(left_for(&sim.outbox, 0, 1));
    assert!(!sim.outbox.iter().any(|m| matches!(
        m.message.message,
        Some(server_message::Message::PlayerJoined(_))
    )));
    sim.outbox.clear();
    sim.process_visible_subjects(
        0,
        TickBoards::new(&w.board, &w.identity, &w.profiles),
        &collector,
        None,
        8,
        true,
    );
    assert!(
        matches!(&sim.outbox[0].message.message, Some(server_message::Message::PlayerJoined(j)) if j.realm == "b")
    );
}
