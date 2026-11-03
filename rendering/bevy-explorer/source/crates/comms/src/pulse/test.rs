use alloy_core::primitives::Address;
use dcl_component::proto_components::pulse;

use super::*;

const SUBJECT: u32 = 42;
const WALLET: &str = "0x0000000000000000000000000000000000000001";

// Parcel (10, 20) under the default grid: min_x = min_z = -150 - 2 = -152, width = 318.
// index = (10 - -152) + (20 - -152) * 318 = 162 + 172*318 = 54858. World base = (160, _, 320).
const PARCEL_INDEX: i32 = 54858;

fn wallet() -> Address {
    WALLET.parse().unwrap()
}

fn server_msg(message: pulse::server_message::Message) -> pulse::ServerMessage {
    pulse::ServerMessage {
        message: Some(message),
    }
}

fn player_state(local: (f32, f32, f32), flags: u32) -> pulse::PlayerState {
    pulse::PlayerState {
        parcel_index: PARCEL_INDEX,
        position_x: pulse::PlayerState::position_x_quantized(local.0),
        position_y: pulse::PlayerState::position_y_quantized(local.1),
        position_z: pulse::PlayerState::position_z_quantized(local.2),
        state_flags: flags,
        ..Default::default()
    }
}

fn joined(sequence: u32, local: (f32, f32, f32), flags: u32) -> pulse::server_message::Message {
    pulse::server_message::Message::PlayerJoined(pulse::PlayerJoined {
        user_id: WALLET.to_string(),
        profile_version: 7,
        state: Some(pulse::PlayerStateFull {
            subject_id: SUBJECT,
            sequence,
            server_tick: 1000,
            state: Some(player_state(local, flags)),
        }),
        // announced by the server since protocol #31; the decoder does not read it yet
        realm: String::new(),
    })
}

fn only_movement(events: Vec<PulseEvent>) -> rfc4::Movement {
    let mut found = None;
    for event in events {
        if let PulseEvent::Movement { movement, .. } = event {
            assert!(found.is_none(), "expected exactly one Movement event");
            found = Some(*movement);
        }
    }
    found.expect("no Movement event")
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 0.05, "expected ~{b}, got {a}");
}

fn movement_timestamps(events: Vec<PulseEvent>) -> Vec<f64> {
    events
        .into_iter()
        .filter_map(|event| match event {
            PulseEvent::Movement { timestamp, .. } => Some(timestamp),
            _ => None,
        })
        .collect()
}

/// Real ticks captured from a live session, where a peer's stop went missing. They are milliseconds
/// and only ~100ms apart at ~2.35e9, which is past the point where an `f32` can tell them apart:
/// all three collapse onto 2350530.75. The receiver drops any update that is not strictly newer
/// than the last applied one, so the deltas that decelerated and then zeroed the peer's velocity
/// were discarded, and the avatar kept dead-reckoning at its last-known speed indefinitely.
#[test]
fn consecutive_ticks_stay_distinct_at_realistic_magnitudes() {
    const TICKS: [u32; 3] = [2350530722, 2350530821, 2350530940];

    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let stamps: Vec<f64> = TICKS
        .iter()
        .enumerate()
        .flat_map(|(i, tick)| {
            let delta = pulse::PlayerStateDeltaTier0 {
                subject_id: SUBJECT,
                baseline_seq: 5 + i as u32,
                new_seq: 6 + i as u32,
                server_tick: *tick,
                position_x: Some(128 + i as u32),
                ..Default::default()
            };
            movement_timestamps(decoder.handle(server_msg(
                pulse::server_message::Message::PlayerStateDelta(delta),
            )))
        })
        .collect();

    assert_eq!(
        stamps.len(),
        TICKS.len(),
        "every delta should emit movement"
    );
    for (a, b) in stamps.iter().zip(stamps.iter().skip(1)) {
        assert!(
            b > a,
            "ticks must stay strictly increasing after conversion, got {a} then {b}"
        );
    }
    // and the gaps must survive intact, not just be non-zero
    assert!((stamps[1] - stamps[0] - 0.099).abs() < 1e-6);
    assert!((stamps[2] - stamps[1] - 0.119).abs() < 1e-6);
}

#[test]
fn parcel_decode_matches_server_scheme() {
    let grid = PulseParcelGrid::default();
    let world = grid.decode_to_world(PARCEL_INDEX, Vec3::new(1.0, 2.0, 3.0));
    approx(world.x, 161.0); // 10*16 + 1
    approx(world.y, 2.0);
    approx(world.z, 323.0); // 20*16 + 3
}

#[test]
fn join_emits_alias_and_world_movement() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    let grounded = pulse::PlayerAnimationFlags::Grounded as u32;
    let events = decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), grounded)));

    let join = events
        .iter()
        .find_map(|e| match e {
            PulseEvent::Joined {
                subject_id,
                address,
                profile_version,
                ..
            } => Some((*subject_id, *address, *profile_version)),
            _ => None,
        })
        .expect("no Joined event");
    assert_eq!(join, (SUBJECT, wallet(), 7));

    let movement = only_movement(events);
    approx(movement.position_x, 161.0);
    approx(movement.position_z, 323.0);
    assert!(movement.is_grounded);
    assert!(movement.scene_driven_animation.is_none());
}

#[test]
fn in_sequence_delta_applies_and_dequantizes() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    // position_x is local, 8 bits over [0,16]; encoded 128 → 128/255*16 ≈ 8.031.
    let delta = pulse::PlayerStateDeltaTier0 {
        subject_id: SUBJECT,
        baseline_seq: 5,
        new_seq: 6,
        server_tick: 1033,
        position_x: Some(128),
        ..Default::default()
    };
    let movement = only_movement(decoder.handle(server_msg(
        pulse::server_message::Message::PlayerStateDelta(delta),
    )));

    // local x replaced (8.031), y/z carried forward from the join (2, 3); parcel base (160, 320).
    approx(movement.position_x, 168.031);
    approx(movement.position_y, 2.0);
    approx(movement.position_z, 323.0);
}

#[test]
fn head_angles_round_trip_signed() {
    // A left/up look: negative yaw and pitch. Head angles quantize over the unsigned [0, 360]
    // range, so without the sender's `rem_euclid` wrap + receiver's `signed_angle` unwrap these
    // would clamp to 0 — only right/down (positive) would survive.
    let grid = PulseParcelGrid::default();
    let movement = rfc4::Movement {
        position_x: 161.0,
        position_y: 2.0,
        position_z: 323.0,
        head_ik_yaw_enabled: true,
        head_ik_pitch_enabled: true,
        head_yaw: -30.0,
        head_pitch: -20.0,
        ..Default::default()
    };
    let state = from_movement(&movement, &grid);

    let mut decoder = PulseDecoder::new(grid);
    let out = only_movement(decoder.handle(server_msg(
        pulse::server_message::Message::PlayerJoined(pulse::PlayerJoined {
            user_id: WALLET.to_string(),
            profile_version: 1,
            state: Some(pulse::PlayerStateFull {
                subject_id: SUBJECT,
                sequence: 1,
                server_tick: 1000,
                state: Some(state),
            }),
            realm: String::new(),
        }),
    )));

    assert!(out.head_ik_yaw_enabled && out.head_ik_pitch_enabled);
    // 7 bits over 360° ≈ 2.83°/step.
    assert!((out.head_yaw - (-30.0)).abs() < 3.0, "yaw {}", out.head_yaw);
    assert!(
        (out.head_pitch - (-20.0)).abs() < 3.0,
        "pitch {}",
        out.head_pitch
    );
}

#[test]
fn sequence_gap_requests_resync_without_applying() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    // baseline_seq 6 is ahead of our last_seq 5 → we missed an intermediate delta.
    let delta = pulse::PlayerStateDeltaTier0 {
        subject_id: SUBJECT,
        baseline_seq: 6,
        new_seq: 7,
        position_x: Some(200),
        ..Default::default()
    };
    let events = decoder.handle(server_msg(
        pulse::server_message::Message::PlayerStateDelta(delta),
    ));

    assert_eq!(events.len(), 1);
    match &events[0] {
        PulseEvent::Resync(r) => {
            assert_eq!(r.subject_id, SUBJECT);
            assert_eq!(r.known_seq, 5);
        }
        other => panic!("expected Resync, got {other:?}"),
    }
}

#[test]
fn stale_delta_is_dropped() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    // new_seq 5 == last_seq 5 → already applied.
    let delta = pulse::PlayerStateDeltaTier0 {
        subject_id: SUBJECT,
        baseline_seq: 4,
        new_seq: 5,
        ..Default::default()
    };
    assert!(decoder
        .handle(server_msg(
            pulse::server_message::Message::PlayerStateDelta(delta)
        ))
        .is_empty());
}

#[test]
fn delta_for_unknown_subject_requests_full() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    let delta = pulse::PlayerStateDeltaTier0 {
        subject_id: 99,
        baseline_seq: 0,
        new_seq: 1,
        ..Default::default()
    };
    let events = decoder.handle(server_msg(
        pulse::server_message::Message::PlayerStateDelta(delta),
    ));
    match &events[0] {
        PulseEvent::Resync(r) => {
            assert_eq!(r.subject_id, 99);
            assert_eq!(r.known_seq, 0);
        }
        other => panic!("expected Resync, got {other:?}"),
    }
}

#[test]
fn left_drops_subject_and_emits_address() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(server_msg(pulse::server_message::Message::PlayerLeft(
        pulse::PlayerLeft {
            subject_id: SUBJECT,
        },
    )));
    match &events[0] {
        PulseEvent::Left { address } => assert_eq!(*address, wallet()),
        other => panic!("expected Left, got {other:?}"),
    }

    // After leaving, a delta should be treated as unknown (resync), proving the baseline is gone.
    let delta = pulse::PlayerStateDeltaTier0 {
        subject_id: SUBJECT,
        baseline_seq: 5,
        new_seq: 6,
        ..Default::default()
    };
    assert!(matches!(
        decoder
            .handle(server_msg(
                pulse::server_message::Message::PlayerStateDelta(delta)
            ))
            .as_slice(),
        [PulseEvent::Resync(_)]
    ));
}

/// Server ticks are `u32` milliseconds since the Pulse server started and roll over after ~49.7
/// days. The converted timestamps must keep increasing across the rollover, or the receiver's
/// strictly-newer gate would drop every update after it.
#[test]
fn ticks_stay_monotonic_across_u32_rollover() {
    const TICKS: [u32; 4] = [u32::MAX - 50, u32::MAX - 10, 30, 70];

    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(pulse::server_message::Message::PlayerJoined(
        pulse::PlayerJoined {
            user_id: WALLET.to_string(),
            profile_version: 7,
            state: Some(pulse::PlayerStateFull {
                subject_id: SUBJECT,
                sequence: 5,
                server_tick: u32::MAX - 100,
                state: Some(player_state((1.0, 2.0, 3.0), 0)),
            }),
            realm: String::new(),
        },
    )));

    let stamps: Vec<f64> = TICKS
        .iter()
        .enumerate()
        .flat_map(|(i, tick)| {
            let delta = pulse::PlayerStateDeltaTier0 {
                subject_id: SUBJECT,
                baseline_seq: 5 + i as u32,
                new_seq: 6 + i as u32,
                server_tick: *tick,
                position_x: Some(128 + i as u32),
                ..Default::default()
            };
            movement_timestamps(decoder.handle(server_msg(
                pulse::server_message::Message::PlayerStateDelta(delta),
            )))
        })
        .collect();

    assert_eq!(stamps.len(), TICKS.len());
    for (a, b) in stamps.iter().zip(stamps.iter().skip(1)) {
        assert!(
            b > a,
            "ticks must stay increasing across the rollover, got {a} then {b}"
        );
    }
    assert!((stamps[1] - stamps[0] - 0.040).abs() < 1e-6);
    assert!((stamps[2] - stamps[1] - 0.041).abs() < 1e-6);
    assert!((stamps[3] - stamps[2] - 0.040).abs() < 1e-6);
}

fn joined_in_realm(
    subject_id: u32,
    wallet: &str,
    realm: &str,
    local: (f32, f32, f32),
) -> pulse::server_message::Message {
    pulse::server_message::Message::PlayerJoined(pulse::PlayerJoined {
        user_id: wallet.to_string(),
        profile_version: 3,
        state: Some(pulse::PlayerStateFull {
            subject_id,
            sequence: 1,
            server_tick: 1000,
            state: Some(player_state(local, 0)),
        }),
        realm: realm.to_string(),
    })
}

fn delta(
    subject_id: u32,
    baseline_seq: u32,
    new_seq: u32,
    tick: u32,
    x: f32,
) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::PlayerStateDelta(
        pulse::PlayerStateDeltaTier0 {
            subject_id,
            baseline_seq,
            new_seq,
            server_tick: tick,
            position_x: Some(pulse::PlayerStateDeltaTier0::position_x_quantized(x)),
            ..Default::default()
        },
    ))
}

/// A realm change despawns every peer client-side while the server keeps its view of them for us,
/// so a quick return brings no re-announcement. The decoder replays what it still holds for that
/// realm — and only that realm — as a join plus a teleport-flagged snapshot of the latest state.
#[test]
fn replay_re_emits_held_subjects_of_that_realm_only() {
    const OTHER_WALLET: &str = "0x0000000000000000000000000000000000000002";
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT,
        WALLET,
        "home",
        (1.0, 2.0, 3.0),
    )));
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT + 1,
        OTHER_WALLET,
        "elsewhere",
        (1.0, 2.0, 3.0),
    )));
    // state moves on after the join: the replay must carry the latest, not the join snapshot
    decoder.handle(delta(SUBJECT, 1, 2, 1500, 5.0));
    decoder.handle(server_msg(
        pulse::server_message::Message::PlayerProfileVersionAnnounced(
            pulse::PlayerProfileVersionsAnnounced {
                subject_id: SUBJECT,
                version: 9,
            },
        ),
    ));

    let events = decoder.replay("home");
    assert_eq!(
        events.len(),
        2,
        "one join + one movement for the one home subject"
    );
    let PulseEvent::Joined {
        subject_id,
        address,
        profile_version,
        realm,
        ..
    } = &events[0]
    else {
        panic!("expected Joined first, got {:?}", events[0]);
    };
    assert_eq!(
        (*subject_id, *address, *profile_version),
        (SUBJECT, wallet(), 9)
    );
    assert_eq!(&**realm, "home");
    let PulseEvent::Movement {
        address,
        movement,
        teleport,
        timestamp,
        ..
    } = &events[1]
    else {
        panic!("expected Movement second, got {:?}", events[1]);
    };
    assert_eq!(*address, wallet());
    assert!(
        *teleport,
        "a replayed snapshot is a discontinuity, not travel"
    );
    approx(movement.position_x, 165.0); // 10*16 + 5 after the delta
    assert!(
        (*timestamp - 1.5).abs() < 1e-6,
        "stamped with the delta's tick"
    );

    assert!(decoder.replay("nowhere").is_empty());
    // replay is a read: the other realm's subject is still held for its own return
    assert_eq!(decoder.replay("elsewhere").len(), 2);
}

#[test]
fn reset_forgets_every_subject() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT,
        WALLET,
        "home",
        (1.0, 2.0, 3.0),
    )));
    assert_eq!(decoder.reset(), [wallet()]);
    assert!(decoder.replay("home").is_empty());
    // and a delta for it is now a stranger's: full state is requested rather than applied
    let events = decoder.handle(delta(SUBJECT, 1, 2, 1500, 5.0));
    assert!(matches!(events.as_slice(), [PulseEvent::Resync(r)] if r.known_seq == 0));
}

fn teleported(sequence: u32, tick: u32, realm: &str) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::Teleported(
        pulse::TeleportPerformed {
            subject_id: SUBJECT,
            sequence,
            server_tick: tick,
            state: Some(player_state((4.0, 0.0, 4.0), 0)),
            realm: realm.to_string(),
        },
    ))
}

/// An observer in the destination realm filtered out whatever it was sent while the subject was
/// elsewhere, and the server won't re-announce a subject it already has in view, so a teleport
/// into another realm is announced as a join (with the held profile version) ahead of its movement.
/// A teleport within the realm is just movement.
#[test]
fn teleport_into_another_realm_emits_join_first() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT,
        WALLET,
        "elsewhere",
        (1.0, 2.0, 3.0),
    )));
    decoder.handle(server_msg(
        pulse::server_message::Message::PlayerProfileVersionAnnounced(
            pulse::PlayerProfileVersionsAnnounced {
                subject_id: SUBJECT,
                version: 9,
            },
        ),
    ));

    let events = decoder.handle(teleported(2, 1500, "home"));
    let [PulseEvent::Joined {
        address,
        profile_version,
        realm: join_realm,
        ..
    }, PulseEvent::Movement {
        realm, teleport, ..
    }] = events.as_slice()
    else {
        panic!("expected Joined then Movement, got {events:?}");
    };
    assert_eq!((*address, *profile_version), (wallet(), 9));
    assert_eq!((&**join_realm, &**realm), ("home", "home"));
    assert!(*teleport);

    let events = decoder.handle(teleported(3, 1600, "home"));
    assert!(matches!(events.as_slice(), [PulseEvent::Movement { .. }]));
}

fn player_state_full(sequence: u32, tick: u32, x: f32) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::PlayerStateFull(
        pulse::PlayerStateFull {
            subject_id: SUBJECT,
            sequence,
            server_tick: tick,
            state: Some(player_state((x, 2.0, 3.0), 0)),
        },
    ))
}

fn emote_started(sequence: u32, tick: u32, urn: &str, x: f32) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::EmoteStarted(
        pulse::EmoteStarted {
            subject_id: SUBJECT,
            sequence,
            server_tick: tick,
            emote_id: urn.to_string(),
            player_state: Some(player_state((x, 2.0, 3.0), 0)),
            mask: None,
        },
    ))
}

fn emote_stopped(sequence: u32, tick: u32, x: f32) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::EmoteStopped(
        pulse::EmoteStopped {
            subject_id: SUBJECT,
            server_tick: tick,
            reason: pulse::EmoteStopReason::Completed as i32,
            sequence,
            player_state: Some(player_state((x, 2.0, 3.0), 0)),
        },
    ))
}

/// A delta that carries no field: the subject stays wherever its baseline has it.
fn bare_delta(baseline_seq: u32, new_seq: u32, tick: u32) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::PlayerStateDelta(
        pulse::PlayerStateDeltaTier0 {
            subject_id: SUBJECT,
            baseline_seq,
            new_seq,
            server_tick: tick,
            ..Default::default()
        },
    ))
}

/// Whether `events` is exactly one `Resync`, for `subject_id` from `known_seq`.
fn is_one_resync(events: &[PulseEvent], subject_id: u32, known_seq: u32) -> bool {
    matches!(
        events,
        [PulseEvent::Resync(request)]
            if (request.subject_id, request.known_seq) == (subject_id, known_seq)
    )
}

fn emits_emote_start(events: &[PulseEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, PulseEvent::EmoteStart { .. }))
}

/// The reply to a resync and the regular deltas travel on different channels, so one can arrive
/// leading on from a state we have already passed. That is nothing to ask the server about.
#[test]
fn a_delta_behind_the_baseline_is_dropped_without_a_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    // new_seq 6 is ahead of our last_seq 5, but baseline_seq 4 is behind it.
    let events = decoder.handle(delta(SUBJECT, 4, 6, 1500, 5.0));
    assert!(events.is_empty(), "{events:?}");
}

/// A gap shows in every delta after it, so one resync stays in flight per subject until something
/// answers it.
#[test]
fn a_burst_of_gaps_sends_one_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events: Vec<PulseEvent> = [(7, 8), (8, 9), (9, 10)]
        .into_iter()
        .flat_map(|(baseline, new)| decoder.handle(delta(SUBJECT, baseline, new, 1500, 5.0)))
        .collect();

    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");
}

#[test]
fn a_consecutive_delta_clears_the_pending_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    // the stream is back on track, whether this is the resync's reply or just the next in line
    let events = decoder.handle(delta(SUBJECT, 5, 6, 1600, 5.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
        "{events:?}"
    );

    // so a gap now is a new one, asked for from where we are now
    let events = decoder.handle(delta(SUBJECT, 8, 9, 1700, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 6), "{events:?}");
}

#[test]
fn a_full_state_clears_the_pending_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    let events = decoder.handle(player_state_full(9, 1600, 5.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
        "{events:?}"
    );

    let events = decoder.handle(delta(SUBJECT, 11, 12, 1700, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 9), "{events:?}");
}

#[test]
fn an_unknown_subject_is_asked_for_once() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());

    let events: Vec<PulseEvent> = [(0, 1), (1, 2)]
        .into_iter()
        .flat_map(|(baseline, new)| decoder.handle(delta(99, baseline, new, 1500, 5.0)))
        .collect();

    assert!(is_one_resync(&events, 99, 0), "{events:?}");
}

/// A full state travels reliably, so it can arrive behind a delta sent after it, and then describes
/// a place the subject has already left.
#[test]
fn an_older_full_state_does_not_move_the_peer_back() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 5, 6, 1500, 5.0));

    let events = decoder.handle(player_state_full(5, 1400, 1.0));
    assert!(events.is_empty(), "{events:?}");

    // the next delta applies without a resync, onto the position the first delta left
    let events = decoder.handle(bare_delta(6, 7, 1600));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 165.0); // 10*16 + 5 from the delta, not 161
}

#[test]
fn an_emote_stop_overtaken_by_a_delta_stops_without_moving_back() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(emote_started(6, 1500, "wave", 1.0));
    decoder.handle(delta(SUBJECT, 6, 7, 1600, 5.0));

    // the stop carries the state at sequence 6, which the delta has since moved on from
    let events = decoder.handle(emote_stopped(6, 1700, 1.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::EmoteStop { .. }]),
        "{events:?}"
    );

    let events = decoder.handle(bare_delta(7, 8, 1800));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 165.0);
}

/// A stop at a sequence is terminal for it: an emote needs a higher sequence to play again.
#[test]
fn a_stop_wins_over_a_start_at_the_same_sequence() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    decoder.handle(emote_stopped(8, 1500, 1.0));
    let events = decoder.handle(emote_started(8, 1600, "wave", 1.0));
    assert!(!emits_emote_start(&events), "{events:?}");

    let events = decoder.handle(emote_started(9, 1700, "wave", 1.0));
    assert!(emits_emote_start(&events), "{events:?}");
}

#[test]
fn a_stale_teleport_keeps_the_realm() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT,
        WALLET,
        "home",
        (1.0, 2.0, 3.0),
    )));
    decoder.handle(teleported(3, 1500, "home"));

    // an older teleport, into a realm they have since left, arriving behind the newer one
    let events = decoder.handle(teleported(2, 1400, "away"));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(delta(SUBJECT, 3, 4, 1600, 5.0));
    let [PulseEvent::Movement { realm, .. }] = events.as_slice() else {
        panic!("expected one Movement, got {events:?}");
    };
    assert_eq!(&**realm, "home");
}

/// Any full state completes a resync, whether or not it is newer than what is held.
#[test]
fn an_older_full_state_still_clears_the_pending_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 5, 6, 1500, 5.0));

    let events = decoder.handle(delta(SUBJECT, 8, 9, 1600, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 6), "{events:?}");

    let events = decoder.handle(player_state_full(5, 1400, 1.0));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(delta(SUBJECT, 8, 9, 1700, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 6), "{events:?}");
}

#[test]
fn a_join_clears_the_pending_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    // announced afresh: whatever the resync was for, this supersedes it
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1600, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");
}

#[test]
fn a_leave_clears_the_pending_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    decoder.handle(server_msg(pulse::server_message::Message::PlayerLeft(
        pulse::PlayerLeft {
            subject_id: SUBJECT,
        },
    )));

    // a stranger again, and asked for as one
    let events = decoder.handle(delta(SUBJECT, 7, 8, 1600, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 0), "{events:?}");
}

#[test]
fn reset_clears_the_pending_resyncs() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    // the request went with the connection that is gone; the next one must be able to ask
    decoder.reset();

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1600, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 0), "{events:?}");
}

#[test]
fn an_emote_event_older_than_the_last_is_ignored() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(emote_started(8, 1500, "wave", 1.0));
    assert!(emits_emote_start(&events), "{events:?}");

    // an earlier emote's start and stop, arriving behind it
    let events = decoder.handle(emote_started(7, 1400, "dance", 1.0));
    assert!(events.is_empty(), "{events:?}");
    let events = decoder.handle(emote_stopped(7, 1450, 1.0));
    assert!(events.is_empty(), "{events:?}");

    // the one playing still ends with its own stop
    let events = decoder.handle(emote_stopped(8, 1600, 1.0));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, PulseEvent::EmoteStop { .. })),
        "{events:?}"
    );
}

/// When the server has evicted the snapshot for a sequence a delta already carried, the emote event
/// reuses that sequence with a state of its own, and that one is authoritative. Nothing else may.
#[test]
fn only_an_emote_event_replaces_the_state_at_the_held_sequence() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 5, 6, 1500, 5.0));

    let events = decoder.handle(player_state_full(6, 1600, 1.0));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(emote_started(6, 1700, "wave", 1.0));
    let [PulseEvent::Movement { movement, .. }, PulseEvent::EmoteStart { .. }] = events.as_slice()
    else {
        panic!("expected Movement then EmoteStart, got {events:?}");
    };
    approx(movement.position_x, 161.0); // 10*16 + 1 from the emote event, not 165 from the delta
}

const OTHER: &str = "0x0000000000000000000000000000000000000002";

fn other() -> Address {
    OTHER.parse().unwrap()
}

/// `wallet` joining as `subject_id`, in "home".
fn joined_home(subject_id: u32, wallet: &str) -> pulse::ServerMessage {
    server_msg(joined_in_realm(subject_id, wallet, "home", (1.0, 2.0, 3.0)))
}

fn left(subject_id: u32) -> pulse::ServerMessage {
    server_msg(pulse::server_message::Message::PlayerLeft(
        pulse::PlayerLeft { subject_id },
    ))
}

/// A peer that reconnects is announced under its new id first; the old id leaves seconds later.
#[test]
fn a_newer_session_keeps_the_avatar_when_the_older_one_leaves() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));

    let events = decoder.handle(joined_home(SUBJECT + 1, WALLET));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Joined { .. },
                PulseEvent::Movement { teleport: true, .. }
            ]
        ),
        "{events:?}"
    );

    let events = decoder.handle(left(SUBJECT));
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn a_shadow_sends_nothing_to_the_engine() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));

    for message in [
        delta(SUBJECT, 1, 2, 1500, 5.0),
        emote_started(3, 1600, "wave", 5.0),
        server_msg(
            pulse::server_message::Message::PlayerProfileVersionAnnounced(
                pulse::PlayerProfileVersionsAnnounced {
                    subject_id: SUBJECT,
                    version: 9,
                },
            ),
        ),
    ] {
        let events = decoder.handle(message);
        assert!(events.is_empty(), "{events:?}");
    }
}

#[test]
fn the_newest_survivor_takes_over_when_the_selected_one_leaves() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    let events = decoder.handle(delta(SUBJECT, 1, 2, 1500, 5.0));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(left(SUBJECT + 1));
    let [PulseEvent::Movement {
        movement, teleport, ..
    }, PulseEvent::Resync(request)] = events.as_slice()
    else {
        panic!("expected Movement then Resync, got {events:?}");
    };
    assert!(*teleport);
    approx(movement.position_x, 165.0); // 10*16 + 5 from the delta it took in as a shadow
    assert_eq!((request.subject_id, request.known_seq), (SUBJECT, 2));
}

#[test]
fn the_last_session_of_a_wallet_leaving_removes_the_avatar() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    decoder.handle(left(SUBJECT + 1));

    let events = decoder.handle(left(SUBJECT));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Left { address }] if *address == wallet()),
        "{events:?}"
    );
}

#[test]
fn a_reused_id_retires_the_previous_wallet() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));

    let events = decoder.handle(joined_home(SUBJECT, OTHER));
    let [PulseEvent::Left { address: gone }, PulseEvent::Joined {
        address: arrived, ..
    }, PulseEvent::Movement { .. }] = events.as_slice()
    else {
        panic!("expected Left, Joined then Movement, got {events:?}");
    };
    assert_eq!((*gone, *arrived), (wallet(), other()));
}

#[test]
fn a_left_for_an_unknown_subject_is_ignored() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());

    let events = decoder.handle(left(99));
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn reset_returns_each_held_wallet_once() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    decoder.handle(joined_home(SUBJECT + 2, OTHER));

    let wallets = decoder.reset();
    assert_eq!(wallets.len(), 2, "{wallets:?}");
    assert_eq!(
        wallets.into_iter().collect::<HashSet<_>>(),
        HashSet::from([wallet(), other()])
    );
}

#[test]
fn replay_emits_only_the_selected_session() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));

    let events = decoder.replay("home");
    assert!(
        matches!(
            events.as_slice(),
            [PulseEvent::Joined { subject_id, .. }, PulseEvent::Movement { .. }]
                if *subject_id == SUBJECT + 1
        ),
        "{events:?}"
    );
}

/// The avatar may be playing the replaced session's emote, and the newcomer has none yet.
#[test]
fn a_newcomer_stops_the_emote_of_the_session_it_replaces() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(emote_started(2, 1500, "wave", 1.0));

    let events = decoder.handle(joined_home(SUBJECT + 1, WALLET));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Joined { .. },
                PulseEvent::Movement { teleport: true, .. },
                PulseEvent::EmoteStop {
                    completed: false,
                    ..
                }
            ]
        ),
        "{events:?}"
    );
}

/// The emote goes out stamped with the survivor's last tick, which a delta has moved past the
/// emote's own.
#[test]
fn a_promoted_shadow_brings_its_own_emote() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    for message in [
        server_msg(pulse::server_message::Message::EmoteStarted(
            pulse::EmoteStarted {
                subject_id: SUBJECT,
                sequence: 2,
                server_tick: 1500,
                emote_id: "wave".to_string(),
                player_state: Some(player_state((1.0, 2.0, 3.0), 0)),
                mask: Some(1), // upper body
            },
        )),
        delta(SUBJECT, 2, 3, 1600, 5.0),
    ] {
        let events = decoder.handle(message);
        assert!(events.is_empty(), "{events:?}");
    }

    let events = decoder.handle(left(SUBJECT + 1));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Movement { teleport: true, .. },
                PulseEvent::EmoteStart {
                    urn,
                    tick: 1600,
                    mask: Some(1),
                    ..
                },
                PulseEvent::Resync(request)
            ] if urn == "wave" && (request.subject_id, request.known_seq) == (SUBJECT, 3)
        ),
        "{events:?}"
    );
}

#[test]
fn a_promoted_shadow_stops_the_emote_of_the_session_that_left() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT - 1, WALLET));
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(emote_started(2, 1500, "wave", 1.0));

    let events = decoder.handle(left(SUBJECT));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Movement { teleport: true, .. },
                PulseEvent::EmoteStop {
                    completed: false,
                    ..
                },
                PulseEvent::Resync(_)
            ]
        ),
        "{events:?}"
    );
}

/// A shadow's state keeps up in silence, and a takeover brings it. A gap asks for nothing: the
/// takeover's resync covers it.
#[test]
fn a_shadow_keeps_up_in_silence() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));

    for message in [
        player_state_full(2, 1500, 1.0),
        teleported(3, 1600, "home"),
        emote_stopped(4, 1700, 5.0),
        delta(SUBJECT, 9, 10, 1800, 1.0),
    ] {
        let events = decoder.handle(message);
        assert!(events.is_empty(), "{events:?}");
    }

    let events = decoder.handle(left(SUBJECT + 1));
    let [PulseEvent::Movement { movement, .. }, PulseEvent::Resync(request)] = events.as_slice()
    else {
        panic!("expected Movement then Resync, got {events:?}");
    };
    approx(movement.position_x, 165.0); // 10*16 + 5 from the emote stop's state
    assert_eq!((request.subject_id, request.known_seq), (SUBJECT, 4));
}

/// The avatar stands in the leaver's realm, so a survivor there takes it over, even one older than
/// a survivor elsewhere (`PeerIdCache.RemovePeer`).
#[test]
fn the_newest_survivor_in_the_leavers_realm_takes_over() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT - 1, WALLET));
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    let events = decoder.handle(teleported(2, 1500, "away"));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(left(SUBJECT + 1));
    let [PulseEvent::Movement { realm, .. }, PulseEvent::Resync(request)] = events.as_slice()
    else {
        panic!("expected Movement then Resync, got {events:?}");
    };
    assert_eq!(&**realm, "home");
    assert_eq!((request.subject_id, request.known_seq), (SUBJECT - 1, 1));
}

/// With no subject left in the leaver's realm, the wallet leaves it. The newest survivor elsewhere
/// is announced in its own realm as a first sighting, with what it took in as a shadow.
#[test]
fn with_no_survivor_in_the_leavers_realm_the_wallet_leaves_it() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    for message in [
        teleported(2, 1500, "away"),
        emote_started(3, 1600, "wave", 5.0),
        server_msg(
            pulse::server_message::Message::PlayerProfileVersionAnnounced(
                pulse::PlayerProfileVersionsAnnounced {
                    subject_id: SUBJECT,
                    version: 9,
                },
            ),
        ),
    ] {
        let events = decoder.handle(message);
        assert!(events.is_empty(), "{events:?}");
    }

    let events = decoder.handle(left(SUBJECT + 1));
    let [gone, joined, movement, emote, resync] = events.as_slice() else {
        panic!("expected Left and a first sighting, got {events:?}");
    };
    assert!(
        matches!(gone, PulseEvent::Left { address } if *address == wallet()),
        "{gone:?}"
    );
    assert!(
        matches!(
            joined,
            PulseEvent::Joined {
                subject_id: SUBJECT,
                profile_version: 9,
                realm,
                ..
            } if &**realm == "away"
        ),
        "{joined:?}"
    );
    assert!(
        matches!(
            movement,
            PulseEvent::Movement { teleport: true, realm, .. } if &**realm == "away"
        ),
        "{movement:?}"
    );
    assert!(
        matches!(
            emote,
            PulseEvent::EmoteStart { urn, realm, .. } if urn == "wave" && &**realm == "away"
        ),
        "{emote:?}"
    );
    assert!(
        is_one_resync(std::slice::from_ref(resync), SUBJECT, 3),
        "{resync:?}"
    );

    // it drives the avatar now, so entering the leaver's realm brings the wallet back there
    let events = decoder.handle(teleported(4, 1700, "home"));
    assert!(
        matches!(
            events.as_slice(),
            [PulseEvent::Joined { realm, .. }, PulseEvent::Movement { .. }] if &**realm == "home"
        ),
        "{events:?}"
    );
}

/// A resync still waiting on its answer from before the subject was a shadow covers the takeover.
#[test]
fn a_takeover_asks_for_no_second_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    let events = decoder.handle(delta(SUBJECT, 5, 6, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 1), "{events:?}");
    decoder.handle(joined_home(SUBJECT + 1, WALLET));

    let events = decoder.handle(left(SUBJECT + 1));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
        "{events:?}"
    );
}

/// Announced again, a subject keeps its place among the wallet's joins (`PeerIdCache.Set`): it does
/// not take over by it, and when the selected one leaves, the newest of the others does.
#[test]
fn a_shadow_announced_again_keeps_its_place() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT - 1, WALLET));
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));

    let events = decoder.handle(joined_home(SUBJECT - 1, WALLET));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(left(SUBJECT + 1));
    assert!(
        matches!(events.last(), Some(PulseEvent::Resync(request)) if request.subject_id == SUBJECT),
        "{events:?}"
    );
}

/// Once its last subject has left, a wallet is no longer held: it comes back as a first sighting,
/// and a reset does not report it.
#[test]
fn a_wallet_that_left_is_no_longer_held() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, OTHER));
    decoder.handle(left(SUBJECT));

    let events = decoder.handle(joined_home(SUBJECT + 2, WALLET));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Joined { .. },
                PulseEvent::Movement {
                    teleport: false,
                    ..
                }
            ]
        ),
        "{events:?}"
    );
    decoder.handle(left(SUBJECT + 2));
    assert_eq!(decoder.reset(), [other()]);
    // nor, after the reset, is any other
    assert!(decoder.reset().is_empty());
}
