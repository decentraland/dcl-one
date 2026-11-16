use super::*;

/// Asks at `now` and records the send when it is allowed.
fn ask(pacer: &mut MovementPacer, now: f64, change: Change) -> bool {
    let send = pacer.should_send(now, change);
    if send {
        pacer.record_send(now);
    }
    send
}

/// The most sends in any window of one second (open at its start) among `sends`.
fn busiest_second(sends: &[f64]) -> usize {
    sends
        .iter()
        .map(|&end| sends.iter().filter(|&&t| t > end - 1.0 && t <= end).count())
        .max()
        .unwrap_or(0)
}

#[test]
fn first_send_goes_out_at_once() {
    let mut pacer = MovementPacer::default();
    assert!(ask(&mut pacer, 0.0, Change::Idle));
}

#[test]
fn an_edge_inside_the_regular_slot_goes_out_at_once() {
    let mut pacer = MovementPacer::default();
    assert!(ask(&mut pacer, 5.0, Change::Moving));
    assert!(!ask(&mut pacer, 5.016, Change::Moving));
    assert!(ask(&mut pacer, 5.032, Change::Edge));
}

#[test]
fn a_burst_of_edges_never_exceeds_the_budget_in_any_second() {
    let mut pacer = MovementPacer::default();
    let mut sends = Vec::new();
    // An edge on every 60 Hz frame for five seconds.
    for frame in 0..300 {
        let now = 10.0 + frame as f64 / 60.0;
        if ask(&mut pacer, now, Change::Edge) {
            sends.push(now);
        }
    }
    assert!(busiest_second(&sends) <= MOVEMENT_MAX_SENDS_PER_SECOND as usize);
}

#[test]
fn a_storm_of_edges_settles_to_the_regular_cadence_without_gaps() {
    let mut pacer = MovementPacer::default();
    let mut sends = Vec::new();
    // An edge on every 60 Hz frame for ten seconds.
    for frame in 0..600 {
        let now = 10.0 + frame as f64 / 60.0;
        if ask(&mut pacer, now, Change::Edge) {
            sends.push(now);
        }
    }
    // The first sends spend the burst; after that no gap is much longer than one regular spacing.
    let settled = &sends[MOVEMENT_BURST as usize..];
    let longest = settled
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f64::max);
    assert!(
        longest <= REGULAR_SPACING_SECONDS + 1.0 / 60.0 + 1e-9,
        "longest gap {longest}"
    );
    // And it keeps sending: about ten a second, not a burst and a silence.
    assert!(sends.len() >= 95, "{} sends in ten seconds", sends.len());
}

#[test]
fn edges_mixed_with_regular_movement_stay_inside_the_budget() {
    let mut pacer = MovementPacer::default();
    let mut sends = Vec::new();
    for frame in 0..600 {
        let now = frame as f64 / 120.0;
        let change = if frame % 7 == 0 {
            Change::Edge
        } else {
            Change::Moving
        };
        if ask(&mut pacer, now, change) {
            sends.push(now);
        }
    }
    assert!(busiest_second(&sends) <= MOVEMENT_MAX_SENDS_PER_SECOND as usize);
}

#[test]
fn a_held_back_edge_goes_out_once_a_token_returns() {
    let mut pacer = MovementPacer::default();
    for step in 0..MOVEMENT_BURST as u32 {
        assert!(ask(&mut pacer, step as f64 * 0.01, Change::Edge));
    }
    assert!(!ask(&mut pacer, 0.05, Change::Edge));
    assert!(ask(&mut pacer, 0.13, Change::Edge));
}

#[test]
fn the_idle_keepalive_stays_at_one_hertz() {
    let mut pacer = MovementPacer::default();
    assert!(ask(&mut pacer, 0.0, Change::Idle));
    assert!(!ask(&mut pacer, 0.5, Change::Idle));
    assert!(!ask(&mut pacer, 0.99, Change::Idle));
    assert!(ask(&mut pacer, 1.0, Change::Idle));
    assert!(!ask(&mut pacer, 1.5, Change::Idle));
    assert!(ask(&mut pacer, 2.0, Change::Idle));
}

#[test]
fn regular_movement_keeps_its_hundred_millisecond_spacing() {
    let mut pacer = MovementPacer::default();
    let mut sends = Vec::new();
    for frame in 0..300 {
        let now = frame as f64 / 60.0;
        if ask(&mut pacer, now, Change::Moving) {
            sends.push(now);
        }
    }
    // Every send is at least 100 ms after the one before, and none waits more than a frame extra.
    for pair in sends.windows(2) {
        assert!(pair[1] - pair[0] >= REGULAR_SPACING_SECONDS);
        assert!(pair[1] - pair[0] < REGULAR_SPACING_SECONDS + 1.0 / 60.0 + 1e-9);
    }
    assert!(busiest_second(&sends) <= MOVEMENT_MAX_SENDS_PER_SECOND as usize);
}

#[test]
fn a_head_turn_under_one_wire_step_does_not_send_and_one_above_it_does() {
    assert!(!head_turned(10.0, 10.0 + HEAD_EPSILON_DEGREES - 0.1));
    assert!(!head_turned(10.0, 10.0 - HEAD_EPSILON_DEGREES + 0.1));
    assert!(head_turned(10.0, 10.0 + HEAD_EPSILON_DEGREES + 0.1));
    assert!(head_turned(10.0, 10.0 - HEAD_EPSILON_DEGREES - 0.1));
}
