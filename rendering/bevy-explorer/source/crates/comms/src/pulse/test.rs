use std::collections::HashSet;

use alloy_core::primitives::Address;
use dcl_component::proto_components::pulse;

use super::*;

const SUBJECT: u32 = 42;
const WALLET: &str = "0x0000000000000000000000000000000000000001";

fn decoder_with_retry_gap(now: Instant) -> PulseDecoder {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle_at(server_msg(joined(5, (1.0, 2.0, 3.0), 0)), now);
    let events = decoder.handle_at(delta(SUBJECT, 7, 8, 1500, 5.0), now);
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");
    decoder
}

#[test]
fn unanswered_resyncs_retry_at_bounded_deadlines_without_inbound_packets() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    for seconds in [1, 3, 7, 12, 17, 22] {
        let deadline = now + Duration::from_secs(seconds);
        assert!(decoder
            .retry_resyncs(deadline - Duration::from_nanos(1))
            .is_empty());
        let events = decoder.retry_resyncs(deadline);
        assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");
        assert!(decoder.retry_resyncs(deadline).is_empty());
    }
}

#[test]
fn a_late_retry_poll_makes_one_attempt_and_schedules_from_the_current_time() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    let later = now + Duration::from_secs(3600);
    assert!(is_one_resync(&decoder.retry_resyncs(later), SUBJECT, 5));
    assert!(decoder.retry_resyncs(later).is_empty());
    assert!(decoder
        .retry_resyncs(later + Duration::from_millis(1999))
        .is_empty());
    assert!(is_one_resync(
        &decoder.retry_resyncs(later + Duration::from_secs(2)),
        SUBJECT,
        5
    ));
}

#[test]
fn stale_and_equal_snapshots_and_duplicate_gaps_preserve_retry_deadlines() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    for message in [
        player_state_full(4, 900, 0.0),
        player_state_full(5, 1000, 1.0),
        emote_started(5, 1100, "wave", 1.0),
        emote_stopped(5, 1200, 1.0),
        delta(SUBJECT, 7, 8, 1500, 5.0),
    ] {
        decoder.handle_at(message, now + Duration::from_millis(900));
    }
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_secs(1)),
        SUBJECT,
        5
    ));
    for message in [
        player_state_full(4, 900, 0.0),
        emote_started(5, 1100, "wave", 1.0),
        delta(SUBJECT, 7, 8, 1500, 5.0),
    ] {
        decoder.handle_at(message, now + Duration::from_secs(2));
    }
    assert!(decoder
        .retry_resyncs(now + Duration::from_millis(2999))
        .is_empty());
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_secs(3)),
        SUBJECT,
        5
    ));
}

#[test]
fn sequence_progress_resets_backoff_and_recovery_stops_all_retries() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    decoder.retry_resyncs(now + Duration::from_secs(1));
    let progress_at = now + Duration::from_millis(1500);
    let events = decoder.handle_at(player_state_full(6, 1400, 4.0), progress_at);
    assert!(events
        .iter()
        .any(|event| matches!(event, PulseEvent::Resync(request) if request.known_seq == 6)));
    assert!(decoder
        .retry_resyncs(progress_at + Duration::from_millis(999))
        .is_empty());
    assert!(is_one_resync(
        &decoder.retry_resyncs(progress_at + Duration::from_secs(1)),
        SUBJECT,
        6
    ));
    let recovered_at = progress_at + Duration::from_millis(1100);
    decoder.handle_at(player_state_full(7, 1450, 4.0), recovered_at);
    assert!(decoder.pending_resync.is_empty());
    assert!(decoder
        .retry_resyncs(recovered_at + Duration::from_secs(60))
        .is_empty());
}

#[test]
fn a_displaced_delta_destination_keeps_retrying_after_its_payload_is_gone() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    decoder.handle_at(delta(SUBJECT, 6, 7, 1400, 4.0), now);
    let progressed_at = now + Duration::from_millis(100);
    decoder.handle_at(player_state_full(6, 1300, 3.0), progressed_at);
    assert!(decoder.subjects[&SUBJECT].pending_delta.is_none());
    assert_eq!(decoder.subjects[&SUBJECT].last_seq, 7);
    assert!(is_one_resync(
        &decoder.retry_resyncs(progressed_at + Duration::from_secs(1)),
        SUBJECT,
        7
    ));
}

#[test]
fn retry_deadlines_end_with_the_subject_or_connection_lifetime() {
    let now = Instant::now();
    for ending in ["left", "reset", "rejoined", "id-reused", "wallet-replaced"] {
        let mut decoder = decoder_with_retry_gap(now);
        match ending {
            "left" => {
                decoder.handle_at(
                    server_msg(pulse::server_message::Message::PlayerLeft(
                        pulse::PlayerLeft {
                            subject_id: SUBJECT,
                        },
                    )),
                    now,
                );
            }
            "reset" => {
                decoder.reset();
            }
            "rejoined" => {
                decoder.handle_at(server_msg(joined(9, (1.0, 2.0, 3.0), 0)), now);
            }
            "id-reused" => {
                let pulse::server_message::Message::PlayerJoined(mut replacement) =
                    joined(0, (1.0, 2.0, 3.0), 0)
                else {
                    unreachable!()
                };
                replacement.user_id = "0x0000000000000000000000000000000000000002".to_owned();
                decoder.handle_at(
                    server_msg(pulse::server_message::Message::PlayerJoined(replacement)),
                    now,
                );
            }
            "wallet-replaced" => {
                let pulse::server_message::Message::PlayerJoined(mut replacement) =
                    joined(0, (1.0, 2.0, 3.0), 0)
                else {
                    unreachable!()
                };
                replacement.state.as_mut().unwrap().subject_id = SUBJECT + 1;
                decoder.handle_at(
                    server_msg(pulse::server_message::Message::PlayerJoined(replacement)),
                    now,
                );
            }
            _ => unreachable!(),
        }
        assert!(decoder.pending_resync.is_empty(), "{ending}");
        assert!(
            decoder
                .retry_resyncs(now + Duration::from_secs(60))
                .is_empty(),
            "{ending}"
        );
    }
}

#[test]
fn subjects_have_independent_retry_deadlines_and_recovery() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    let pulse::server_message::Message::PlayerJoined(mut other) = joined(5, (1.0, 2.0, 3.0), 0)
    else {
        unreachable!()
    };
    other.user_id = "0x0000000000000000000000000000000000000002".to_owned();
    other.state.as_mut().unwrap().subject_id = SUBJECT + 1;
    decoder.handle_at(
        server_msg(pulse::server_message::Message::PlayerJoined(other)),
        now,
    );
    decoder.handle_at(
        delta(SUBJECT + 1, 7, 8, 1500, 5.0),
        now + Duration::from_millis(500),
    );
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_secs(1)),
        SUBJECT,
        5
    ));
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_millis(1500)),
        SUBJECT + 1,
        5
    ));
    decoder.handle_at(
        player_state_full(7, 1400, 3.0),
        now + Duration::from_secs(2),
    );
    assert!(decoder
        .retry_resyncs(now + Duration::from_secs(3))
        .is_empty());
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_millis(3500)),
        SUBJECT + 1,
        5
    ));
}

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
    decoder.handle(joined_with_tick(WALLET, 5, TICKS[0] - 100, 1.0));

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
        server_tick: 1100,
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

/// A subject not held is about to be announced, with its full state, or has left, and the server
/// answers nothing for a subject out of view. Either way there is nothing to ask for.
#[test]
fn a_delta_for_an_unknown_subject_is_dropped() {
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
    assert!(events.is_empty(), "{events:?}");
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

    // After leaving, a delta that would follow on from the join applies to nothing, proving the
    // baseline is gone: it is an unknown subject's, and dropped.
    let delta = pulse::PlayerStateDeltaTier0 {
        subject_id: SUBJECT,
        baseline_seq: 5,
        new_seq: 6,
        ..Default::default()
    };
    let events = decoder.handle(server_msg(
        pulse::server_message::Message::PlayerStateDelta(delta),
    ));
    assert!(events.is_empty(), "{events:?}");
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
    // and a delta for it is now a stranger's: dropped rather than applied
    let events = decoder.handle(delta(SUBJECT, 1, 2, 1500, 5.0));
    assert!(events.is_empty(), "{events:?}");
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
/// leading on from a state we have already passed while advertising a newer destination.
#[test]
fn a_delta_behind_the_baseline_requests_its_newer_destination_without_applying_its_mask() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    // new_seq 6 is ahead of our last_seq 5, but baseline_seq 4 is behind it.
    let events = decoder.handle(delta(SUBJECT, 4, 6, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");
    assert_eq!(decoder.subjects[&SUBJECT].last_seq, 5);
    assert_eq!(decoder.subjects[&SUBJECT].latest_pending_seq, 6);
}

#[test]
fn a_late_resync_delta_with_an_old_base_recovers_after_a_full_state_closed_the_gap() {
    let now = Instant::now();
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle_at(server_msg(joined(5, (1.0, 2.0, 3.0), 0)), now);
    decoder.handle_at(delta(SUBJECT, 6, 7, 1500, 5.0), now);
    decoder.handle_at(player_state_full(8, 1800, 4.0), now);
    assert!(decoder.pending_resync.is_empty());

    let events = decoder.handle_at(delta(SUBJECT, 5, 9, 1900, 9.0), now);
    assert!(is_one_resync(&events, SUBJECT, 8), "{events:?}");
    assert_eq!(decoder.subjects[&SUBJECT].last_seq, 8);
    assert!(decoder.subjects[&SUBJECT].pending_delta.is_none());
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_secs(1)),
        SUBJECT,
        8
    ));
    let events = decoder.handle_at(
        delta(SUBJECT, 8, 9, 2000, 9.0),
        now + Duration::from_secs(1),
    );
    approx(only_movement(events).position_x, 169.0);
    assert!(decoder.pending_resync.is_empty());
    assert!(decoder
        .retry_resyncs(now + Duration::from_secs(60))
        .is_empty());
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
fn a_consecutive_delta_renews_the_remaining_gap_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    // the stream is back on track, whether this is the resync's reply or just the next in line
    let events = decoder.handle(delta(SUBJECT, 5, 6, 1600, 5.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { .. }, PulseEvent::Resync(request)] if request.known_seq == 6),
        "{events:?}"
    );

    // The remaining gap is already requested from the updated baseline.
    let events = decoder.handle(delta(SUBJECT, 8, 9, 1700, 5.0));
    assert!(events.is_empty(), "{events:?}");
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

/// An unknown subject's deltas leave nothing waiting on a resync. Once it is held, its gaps ask
/// as any held subject's do: once.
#[test]
fn an_unknown_subject_leaves_nothing_pending() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());

    let events: Vec<PulseEvent> = [(0, 1), (1, 2)]
        .into_iter()
        .flat_map(|(baseline, new)| decoder.handle(delta(99, baseline, new, 1500, 5.0)))
        .collect();
    assert!(events.is_empty(), "{events:?}");
    assert!(decoder.pending_resync.is_empty());

    decoder.handle(server_msg(joined_in_realm(
        99,
        WALLET,
        "home",
        (1.0, 2.0, 3.0),
    )));
    let events: Vec<PulseEvent> = [(3, 4), (4, 5)]
        .into_iter()
        .flat_map(|(baseline, new)| decoder.handle(delta(99, baseline, new, 1600, 5.0)))
        .collect();
    assert!(is_one_resync(&events, 99, 1), "{events:?}");
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

/// An old reply does not complete the request for a newer baseline.
#[test]
fn an_older_full_state_keeps_the_outstanding_gap_resync() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 5, 6, 1500, 5.0));

    let events = decoder.handle(delta(SUBJECT, 8, 9, 1600, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 6), "{events:?}");

    let events = decoder.handle(player_state_full(5, 1400, 1.0));
    assert!(events.is_empty(), "{events:?}");

    let events = decoder.handle(delta(SUBJECT, 8, 9, 1700, 5.0));
    assert!(events.is_empty(), "{events:?}");
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
    assert!(decoder.pending_resync.is_empty());

    // a delta the leave overtook is a stranger's: it asks for nothing, so nothing waits on an
    // answer the server will never send
    let events = decoder.handle(delta(SUBJECT, 8, 9, 1600, 5.0));
    assert!(events.is_empty(), "{events:?}");
    assert!(decoder.pending_resync.is_empty());
}

#[test]
fn reset_clears_the_pending_resyncs() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));

    let events = decoder.handle(delta(SUBJECT, 7, 8, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    // the request went with the connection that is gone; nothing waits on its answer
    decoder.reset();
    assert!(decoder.pending_resync.is_empty());

    // and the subject's deltas are a stranger's now, which ask for nothing
    let events = decoder.handle(delta(SUBJECT, 7, 8, 1600, 5.0));
    assert!(events.is_empty(), "{events:?}");
    assert!(decoder.pending_resync.is_empty());
}

#[test]
fn newer_gaps_preserve_the_pending_request_and_its_initial_retry_deadline() {
    let now = Instant::now();
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle_at(server_msg(joined(5, (1.0, 2.0, 3.0), 0)), now);

    let events = decoder.handle_at(delta(SUBJECT, 7, 8, 1500, 5.0), now);
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");

    // A later gap extends the recovery goal without another immediate request.
    assert!(decoder
        .handle_at(
            delta(SUBJECT, 8, 9, 1600, 5.0),
            now + Duration::from_millis(900),
        )
        .is_empty());
    assert!(decoder
        .retry_resyncs(now + Duration::from_millis(999))
        .is_empty());
    let events = decoder.retry_resyncs(now + Duration::from_secs(1));
    assert!(is_one_resync(&events, SUBJECT, 5), "{events:?}");
}

#[test]
fn an_older_baseline_reply_extends_a_pending_goal_without_duplicating_or_delaying_the_request() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    let events = decoder.handle_at(
        delta(SUBJECT, 4, 9, 1800, 6.0),
        now + Duration::from_millis(900),
    );
    assert!(events.is_empty(), "{events:?}");
    assert!(decoder
        .retry_resyncs(now + Duration::from_millis(999))
        .is_empty());
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_secs(1)),
        SUBJECT,
        5
    ));

    // The held 7 → 8 delta still replays; the incompatible 4 → 9 fields do not.
    let events = decoder.handle_at(
        player_state_full(7, 1400, 3.0),
        now + Duration::from_millis(1100),
    );
    assert!(
        events.iter().any(|event| matches!(event,
            PulseEvent::Resync(request) if request.known_seq == 8
        )),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 165.0);
    let events = decoder.handle_at(
        delta(SUBJECT, 8, 9, 1900, 7.0),
        now + Duration::from_millis(1200),
    );
    approx(only_movement(events).position_x, 167.0);
    assert!(decoder
        .retry_resyncs(now + Duration::from_secs(60))
        .is_empty());
}

#[test]
fn acknowledging_a_request_after_its_subject_left_does_not_restore_retry_state() {
    let now = Instant::now();
    let mut decoder = decoder_with_retry_gap(now);
    decoder.handle_at(left(SUBJECT), now);

    decoder.resync_sent(SUBJECT);

    assert!(decoder.pending_resync.is_empty());
    assert!(!decoder.subjects.contains_key(&SUBJECT));
    assert!(decoder
        .retry_resyncs(now + Duration::from_secs(60))
        .is_empty());
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

/// A peer that reconnects is announced under its new id, and the servers have already ended the old
/// session, whose messages can still arrive. They are a stranger's: the avatar stays with the
/// newcomer, and the old id's leave removes nothing.
#[test]
fn a_newer_join_replaces_the_older_subject() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));

    let events = decoder.handle(joined_home(SUBJECT + 1, WALLET));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Joined { subject_id, .. },
                PulseEvent::Movement { teleport: true, .. }
            ] if *subject_id == SUBJECT + 1
        ),
        "{events:?}"
    );

    for message in [
        delta(SUBJECT, 1, 2, 1500, 5.0),
        player_state_full(3, 1600, 5.0),
        emote_started(4, 1700, "wave", 5.0),
        teleported(5, 1800, "home"),
        left(SUBJECT),
    ] {
        let events = decoder.handle(message);
        assert!(events.is_empty(), "{events:?}");
    }

    let events = decoder.handle(delta(SUBJECT + 1, 1, 2, 1900, 5.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { address, .. }] if *address == wallet()),
        "{events:?}"
    );
}

#[test]
fn the_current_subjects_leave_removes_the_wallet() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, WALLET));

    let events = decoder.handle(left(SUBJECT + 1));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Left { address }] if *address == wallet()),
        "{events:?}"
    );
    // the subject it replaced is not held either
    assert!(decoder.reset().is_empty());
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
fn replay_emits_only_the_newer_session() {
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

/// The avatar stood in the replaced subject's realm, so a newcomer in another takes the wallet out
/// of it first. The emote goes with the `Left`: there is none to stop.
#[test]
fn a_subject_replaced_from_another_realm_leaves_its_realm() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(emote_started(2, 1500, "wave", 1.0));

    let events = decoder.handle(server_msg(joined_in_realm(
        SUBJECT + 1,
        WALLET,
        "away",
        (1.0, 2.0, 3.0),
    )));
    assert!(
        matches!(
            events.as_slice(),
            [
                PulseEvent::Left { address },
                PulseEvent::Joined { realm: joined, .. },
                PulseEvent::Movement { realm: moved, teleport: true, .. }
            ] if *address == wallet() && &**joined == "away" && &**moved == "away"
        ),
        "{events:?}"
    );
}

/// The replaced subject's session is gone, so nothing will answer the resync it was waiting on.
#[test]
fn a_replaced_subject_leaves_no_resync_pending() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    let events = decoder.handle(delta(SUBJECT, 5, 6, 1500, 5.0));
    assert!(is_one_resync(&events, SUBJECT, 1), "{events:?}");

    decoder.handle(joined_home(SUBJECT + 1, WALLET));
    assert!(decoder.pending_resync.is_empty());
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

#[test]
fn buffered_delta_applies_when_its_full_baseline_arrives() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 5, 6, 1500, 2.0));
    assert!(is_one_resync(
        &decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0)),
        SUBJECT,
        6
    ));

    let events = decoder.handle(player_state_full(7, 1600, 3.0));
    assert!(
        matches!(
            events.as_slice(),
            [PulseEvent::Movement { timestamp, teleport: false, .. }] if *timestamp == 1.7
        ),
        "{events:?}"
    );
    let movement = only_movement(events);
    approx(movement.position_x, 165.0);
    assert!(movement.position_precision.is_some());
    assert!(decoder.pending_resync.is_empty());
    approx(
        only_movement(decoder.handle(delta(SUBJECT, 8, 9, 1800, 6.0))).position_x,
        166.0,
    );
}

#[test]
fn buffered_delta_applies_when_a_delta_reaches_its_baseline() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));

    let events = decoder.handle(delta(SUBJECT, 5, 7, 1600, 3.0));
    assert!(
        matches!(
            events.as_slice(),
            [PulseEvent::Movement { timestamp, .. }] if *timestamp == 1.7
        ),
        "{events:?}"
    );
    let movement = only_movement(events);
    approx(movement.position_x, 165.0);
    assert!(decoder.pending_resync.is_empty());
    approx(
        only_movement(decoder.handle(bare_delta(8, 9, 1800))).position_x,
        165.0,
    );
}

#[test]
fn buffered_delta_keeps_the_newest_update_from_the_same_baseline() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 7, 9, 1700, 6.0));
    decoder.handle(delta(SUBJECT, 7, 8, 1600, 5.0));
    decoder.handle(delta(SUBJECT, 7, 10, 1800, 7.0));

    approx(
        only_movement(decoder.handle(player_state_full(7, 1500, 3.0))).position_x,
        167.0,
    );
    assert!(decoder.handle(bare_delta(9, 10, 1900)).is_empty());
    approx(
        only_movement(decoder.handle(bare_delta(10, 11, 2000))).position_x,
        167.0,
    );
}

#[test]
fn buffered_delta_prefers_the_nearest_missing_baseline_during_a_burst() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 8, 9, 1800, 6.0));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));
    for baseline in 9..100 {
        assert!(decoder
            .handle(delta(SUBJECT, baseline, baseline + 1, 1900, 7.0))
            .is_empty());
    }

    let events = decoder.handle(player_state_full(7, 1600, 3.0));
    assert!(
        events.iter().any(|event| matches!(event,
            PulseEvent::Resync(request) if request.known_seq == 8
        )),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 165.0);
    // Displaced later states remain a gap until the stream or a resync supplies them.
    approx(
        only_movement(decoder.handle(bare_delta(8, 9, 2000))).position_x,
        165.0,
    );
}

#[test]
fn buffered_delta_renews_resync_when_a_full_state_does_not_close_the_gap() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 8, 9, 1800, 6.0));

    let events = decoder.handle(player_state_full(7, 1600, 3.0));
    assert!(
        events.iter().any(|event| matches!(event,
            PulseEvent::Resync(request) if (request.subject_id, request.known_seq) == (SUBJECT, 7)
        )),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 163.0);
    for sequence in [7, 6, 7, 5] {
        let events = decoder.handle(player_state_full(sequence, 1600, 3.0));
        assert!(events.is_empty(), "{events:?}");
    }
    approx(
        only_movement(decoder.handle(player_state_full(8, 1700, 4.0))).position_x,
        166.0,
    );
}

#[test]
fn buffered_delta_renews_resync_when_a_delta_does_not_close_the_gap() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 8, 9, 1800, 6.0));

    let events = decoder.handle(delta(SUBJECT, 5, 7, 1600, 3.0));
    assert!(
        events.iter().any(|event| matches!(event,
            PulseEvent::Resync(request) if request.known_seq == 7
        )),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 163.0);
    approx(
        only_movement(decoder.handle(delta(SUBJECT, 7, 8, 1700, 4.0))).position_x,
        166.0,
    );
}

#[test]
fn buffered_delta_survives_a_stale_full_state_and_a_retry() {
    let now = Instant::now();
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle_at(server_msg(joined(5, (1.0, 2.0, 3.0), 0)), now);
    decoder.handle_at(delta(SUBJECT, 7, 8, 1700, 5.0), now);
    let stale = decoder.handle_at(player_state_full(4, 900, 0.0), now);
    assert!(stale.is_empty(), "{stale:?}");
    assert!(decoder
        .handle_at(delta(SUBJECT, 7, 8, 1700, 5.0), now)
        .is_empty());
    assert!(is_one_resync(
        &decoder.retry_resyncs(now + Duration::from_secs(1)),
        SUBJECT,
        5
    ));
    approx(
        only_movement(decoder.handle_at(
            player_state_full(7, 1600, 3.0),
            now + Duration::from_secs(1),
        ))
        .position_x,
        165.0,
    );
}

#[test]
fn buffered_delta_is_discarded_when_a_full_state_has_passed_it() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));
    let movement = only_movement(decoder.handle(player_state_full(9, 1800, 6.0)));
    approx(movement.position_x, 166.0);
    assert!(movement.position_precision.is_none());
    assert!(decoder.pending_resync.is_empty());
    assert!(decoder.handle(player_state_full(7, 1600, 3.0)).is_empty());
    approx(
        only_movement(decoder.handle(bare_delta(9, 10, 1900))).position_x,
        166.0,
    );
}

#[test]
fn buffered_delta_preserves_a_teleport_and_its_new_realm() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT,
        WALLET,
        "home",
        (1.0, 2.0, 3.0),
    )));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));

    let events = decoder.handle(teleported(7, 1600, "elsewhere"));
    assert!(
        matches!(events.as_slice(), [
            PulseEvent::Joined { realm: joined_realm, .. },
            PulseEvent::Movement { realm, teleport: true, timestamp, .. },
        ] if &**joined_realm == "elsewhere" && &**realm == "elsewhere" && *timestamp == 1.7
        ),
        "{events:?}"
    );
    let movement = only_movement(events);
    approx(movement.position_x, 165.0);
    let precision = movement
        .position_precision
        .expect("replayed delta precision");
    assert_eq!(precision.x, 16.0 / 255.0 * 0.5);
    assert_eq!(precision.y, 200.0 / 8191.0 * 0.5);
    assert_eq!(precision.z, 16.0 / 255.0 * 0.5);
}

#[test]
fn buffered_delta_is_discarded_when_a_teleport_passes_its_destination() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined_in_realm(
        SUBJECT,
        WALLET,
        "home",
        (1.0, 2.0, 3.0),
    )));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));

    let events = decoder.handle(teleported(9, 1800, "elsewhere"));
    assert!(
        matches!(events.as_slice(), [
            PulseEvent::Joined { realm: joined_realm, .. },
            PulseEvent::Movement { realm, teleport: true, timestamp, .. },
        ] if &**joined_realm == "elsewhere" && &**realm == "elsewhere" && *timestamp == 1.8
        ),
        "{events:?}"
    );
    let movement = only_movement(events);
    approx(movement.position_x, 164.0);
    assert!(movement.position_precision.is_none());
    assert!(decoder.pending_resync.is_empty());
    approx(
        only_movement(decoder.handle(bare_delta(9, 10, 1900))).position_x,
        164.0,
    );
}

#[test]
fn buffered_delta_is_applied_before_an_emote_start_or_stop() {
    for start in [true, false] {
        let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
        decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
        decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));
        let events = decoder.handle(if start {
            emote_started(7, 1600, "wave", 3.0)
        } else {
            emote_stopped(7, 1600, 3.0)
        });
        assert!(
            matches!(events.first(), Some(PulseEvent::Movement { timestamp, .. }) if *timestamp == 1.7),
            "{events:?}"
        );
        assert!(
            matches!(
                events.last(),
                Some(PulseEvent::EmoteStart { .. } | PulseEvent::EmoteStop { .. })
            ),
            "{events:?}"
        );
        approx(only_movement(events).position_x, 165.0);
    }
}

#[test]
fn buffered_delta_does_not_cross_a_subject_lifetime() {
    const OTHER: &str = "0x0000000000000000000000000000000000000002";
    for ending in ["leave", "reset", "rejoin", "wallet replacement", "id reuse"] {
        let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
        decoder.handle(joined_home(SUBJECT, WALLET));
        decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));
        match ending {
            "leave" => {
                decoder.handle(left(SUBJECT));
            }
            "reset" => {
                decoder.reset();
            }
            "rejoin" => {}
            "wallet replacement" => {
                decoder.handle(joined_home(SUBJECT + 1, WALLET));
            }
            "id reuse" => {
                decoder.handle(joined_home(SUBJECT, OTHER));
            }
            _ => unreachable!(),
        }
        decoder.handle(joined_home(SUBJECT, WALLET));
        let events = decoder.handle(player_state_full(7, 1600, 3.0));
        assert!(
            matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
            "{ending}: {events:?}"
        );
        approx(only_movement(events).position_x, 163.0);
        assert!(decoder.pending_resync.is_empty(), "{ending}");
    }
}

#[test]
fn buffered_delta_is_isolated_from_other_subjects() {
    const OTHER: &str = "0x0000000000000000000000000000000000000002";
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_home(SUBJECT, WALLET));
    decoder.handle(joined_home(SUBJECT + 1, OTHER));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));
    let events = decoder.handle(delta(SUBJECT + 1, 1, 7, 1600, 3.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { address, .. }] if *address == other()),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 163.0);
    approx(
        only_movement(decoder.handle(player_state_full(7, 1600, 3.0))).position_x,
        165.0,
    );
}

#[test]
fn buffered_delta_rejects_nonadvancing_sequences_without_losing_a_valid_link() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));
    for (baseline, new) in [(6, 6), (6, 5), (8, 7)] {
        assert!(decoder
            .handle(delta(SUBJECT, baseline, new, 1800, 6.0))
            .is_empty());
    }
    approx(
        only_movement(decoder.handle(player_state_full(7, 1600, 3.0))).position_x,
        165.0,
    );
}

#[test]
fn buffered_delta_is_not_retained_before_a_subject_joins() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    assert!(decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0)).is_empty());
    decoder.handle(joined_home(SUBJECT, WALLET));
    approx(
        only_movement(decoder.handle(player_state_full(7, 1600, 3.0))).position_x,
        163.0,
    );
}

#[test]
fn buffered_delta_renews_resync_when_progress_passes_its_baseline_but_not_its_end() {
    for full_state in [true, false] {
        let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
        decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
        decoder.handle(delta(SUBJECT, 7, 10, 1800, 6.0));

        let events = decoder.handle(if full_state {
            player_state_full(8, 1700, 4.0)
        } else {
            delta(SUBJECT, 5, 8, 1700, 4.0)
        });
        assert!(
            events.iter().any(|event| matches!(event,
                PulseEvent::Resync(request) if request.known_seq == 8
            )),
            "{events:?}"
        );
        // The old baseline cannot safely be overlaid on sequence 8.
        approx(only_movement(events).position_x, 164.0);
        approx(
            only_movement(decoder.handle(player_state_full(10, 1900, 6.0))).position_x,
            166.0,
        );
        assert!(decoder.pending_resync.is_empty());
    }
}

#[test]
fn buffered_delta_remembers_a_later_destination_after_keeping_a_nearer_link() {
    for nearest_first in [true, false] {
        let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
        decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
        let nearest = delta(SUBJECT, 7, 8, 1700, 5.0);
        let later = delta(SUBJECT, 9, 10, 1900, 7.0);
        if nearest_first {
            decoder.handle(nearest);
            decoder.handle(later);
        } else {
            decoder.handle(later);
            decoder.handle(nearest);
        }

        let events = decoder.handle(player_state_full(7, 1600, 3.0));
        assert!(
            events.iter().any(|event| matches!(event,
                PulseEvent::Resync(request) if request.known_seq == 8
            )),
            "{events:?}"
        );
        approx(only_movement(events).position_x, 165.0);
        approx(
            only_movement(decoder.handle(player_state_full(10, 2000, 7.0))).position_x,
            167.0,
        );
        assert!(decoder.pending_resync.is_empty());
    }
}

#[test]
fn buffered_delta_keeps_one_request_during_repeated_stale_and_equal_snapshots() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    assert!(is_one_resync(
        &decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0)),
        SUBJECT,
        5
    ));

    for _ in 0..4 {
        for message in [
            player_state_full(4, 900, 0.0),
            player_state_full(5, 1000, 1.0),
            teleported(4, 900, "old"),
            teleported(5, 1000, "old"),
        ] {
            let events = decoder.handle(message);
            assert!(events.is_empty(), "{events:?}");
            assert!(decoder.pending_resync.contains_key(&SUBJECT));
        }
    }
    approx(
        only_movement(decoder.handle(player_state_full(7, 1600, 3.0))).position_x,
        165.0,
    );
    assert!(decoder.pending_resync.is_empty());
}

#[test]
fn buffered_delta_keeps_its_request_while_equal_sequence_emotes_are_reported() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(server_msg(joined(5, (1.0, 2.0, 3.0), 0)));
    decoder.handle(delta(SUBJECT, 7, 8, 1700, 5.0));

    let events = decoder.handle(emote_started(5, 1100, "wave", 1.0));
    assert!(
        matches!(
            events.as_slice(),
            [PulseEvent::Movement { .. }, PulseEvent::EmoteStart { .. }]
        ),
        "{events:?}"
    );
    assert!(decoder.pending_resync.contains_key(&SUBJECT));

    for _ in 0..3 {
        let events = decoder.handle(emote_started(5, 1100, "wave", 1.0));
        assert!(
            matches!(
                events.as_slice(),
                [PulseEvent::Movement { .. }, PulseEvent::EmoteStart { .. }]
            ),
            "{events:?}"
        );
        assert!(decoder.pending_resync.contains_key(&SUBJECT));
    }

    let events = decoder.handle(emote_stopped(5, 1200, 1.0));
    assert!(
        matches!(
            events.as_slice(),
            [PulseEvent::Movement { .. }, PulseEvent::EmoteStop { .. }]
        ),
        "{events:?}"
    );
    assert!(decoder.pending_resync.contains_key(&SUBJECT));
    approx(
        only_movement(decoder.handle(player_state_full(7, 1600, 3.0))).position_x,
        165.0,
    );
    assert!(decoder.pending_resync.is_empty());
}

fn joined_with_tick(wallet: &str, sequence: u32, tick: u32, x: f32) -> pulse::ServerMessage {
    let pulse::server_message::Message::PlayerJoined(mut message) =
        joined_in_realm(SUBJECT, wallet, "home", (x, 2.0, 3.0))
    else {
        unreachable!()
    };
    let full = message.state.as_mut().unwrap();
    full.sequence = sequence;
    full.server_tick = tick;
    server_msg(pulse::server_message::Message::PlayerJoined(message))
}

#[test]
fn a_reused_subject_id_drops_deltas_from_before_its_join() {
    const OTHER: &str = "0x0000000000000000000000000000000000000002";
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_with_tick(WALLET, 2, 1000, 1.0));
    decoder.handle(left(SUBJECT));
    decoder.handle(joined_with_tick(OTHER, 0, 10000, 10.0));

    let events = decoder.handle(delta(SUBJECT, 2, 3, 1000, 2.0));
    assert!(events.is_empty(), "{events:?}");
    assert!(decoder.pending_resync.is_empty());
    let events = decoder.handle(delta(SUBJECT, 0, 2, 10100, 11.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { address, timestamp, .. }]
        if *address == other() && *timestamp == 10.1),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 171.0);
    approx(
        only_movement(decoder.handle(delta(SUBJECT, 2, 3, 10200, 12.0))).position_x,
        172.0,
    );
}

#[test]
fn a_subject_accepts_deltas_at_the_exact_join_tick() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_with_tick(WALLET, 0, 10000, 10.0));
    assert!(is_one_resync(
        &decoder.handle(delta(SUBJECT, 2, 3, 10000, 12.0)),
        SUBJECT,
        0
    ));
    let events = decoder.handle(delta(SUBJECT, 0, 2, 10000, 11.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { timestamp, .. }]
        if *timestamp == 10.0),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 172.0);
}

#[test]
fn a_subject_lifetime_floor_unwraps_the_server_tick_rollover() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_with_tick(WALLET, 0, u32::MAX - 50, 10.0));
    assert!(decoder
        .handle(delta(SUBJECT, 2, 3, u32::MAX - 100, 2.0))
        .is_empty());
    let events = decoder.handle(delta(SUBJECT, 0, 2, 25, 11.0));
    assert!(
        matches!(events.as_slice(), [PulseEvent::Movement { timestamp, .. }]
        if (*timestamp - ((1u64 << 32) + 25) as f64 / 1000.0).abs() < 1e-6),
        "{events:?}"
    );
    approx(only_movement(events).position_x, 171.0);
    approx(
        only_movement(decoder.handle(delta(SUBJECT, 2, 3, 30, 12.0))).position_x,
        172.0,
    );
}

#[test]
fn a_subject_can_resume_after_a_full_tick_rollover_while_other_peers_advance_the_clock() {
    const OTHER: &str = "0x0000000000000000000000000000000000000002";
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_with_tick(WALLET, 0, 1000, 10.0));
    let pulse::server_message::Message::PlayerJoined(mut peer) =
        joined_in_realm(SUBJECT + 1, OTHER, "home", (4.0, 2.0, 3.0))
    else {
        unreachable!()
    };
    peer.state.as_mut().unwrap().sequence = 0;
    decoder.handle(server_msg(pulse::server_message::Message::PlayerJoined(
        peer,
    )));
    for (sequence, tick) in [0x4000_0000, 0x8000_0000, 0xc000_0000, 0x100]
        .into_iter()
        .enumerate()
    {
        let sequence = sequence as u32;
        let events = decoder.handle(delta(SUBJECT + 1, sequence, sequence + 1, tick, 4.0));
        assert!(
            matches!(events.as_slice(), [PulseEvent::Movement { .. }]),
            "{events:?}"
        );
    }
    let events = decoder.handle(delta(SUBJECT, 0, 1, 0x101, 11.0));
    approx(only_movement(events).position_x, 171.0);
}

#[test]
fn an_emote_start_before_the_join_does_not_lower_the_delta_lifetime_floor() {
    let mut decoder = PulseDecoder::new(PulseParcelGrid::default());
    decoder.handle(joined_with_tick(WALLET, 0, 10000, 10.0));
    let events = decoder.handle(emote_started(0, 1000, "wave", 10.0));
    assert!(emits_emote_start(&events), "{events:?}");
    approx(only_movement(events).position_x, 170.0);
    assert!(decoder.handle(delta(SUBJECT, 2, 3, 2000, 2.0)).is_empty());
    approx(
        only_movement(decoder.handle(delta(SUBJECT, 0, 2, 10100, 11.0))).position_x,
        171.0,
    );
}
