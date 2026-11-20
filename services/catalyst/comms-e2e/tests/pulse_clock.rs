//! `pulse-handshake-clock` against a Pulse server in this process, whose clock is this machine's:
//! the first handshake, signed now, is accepted, and the notes say so.

mod support;

use catalyst_comms_e2e::engine::{Outcome, RunOptions, run_scenarios};
use catalyst_comms_e2e::scenarios::pulse_clock::{BEHIND_MS, HANDSHAKE_CLOCK};

#[test]
fn the_ladder_starts_at_now_and_only_goes_back() {
    assert_eq!(BEHIND_MS[0], 0);
    assert!(BEHIND_MS.windows(2).all(|pair| pair[0] < pair[1]), "{BEHIND_MS:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_on_this_clock_accepts_a_handshake_signed_now() {
    let pulse = support::in_process_pulse().await;
    let options = RunOptions {
        repeat: Some(1),
        seed: Some(3),
        ..Default::default()
    };
    let mut report = run_scenarios(support::stack_at(pulse), &[&HANDSHAKE_CLOCK], &options).await;
    let result = report.results.remove(0);
    assert!(matches!(result.outcome, Outcome::Passed), "{:?}", result.outcome);
    let note = |key: &str| {
        result
            .notes
            .iter()
            .find(|note| note.key == key)
            .unwrap_or_else(|| panic!("no note {key}: {:?}", result.notes))
            .value
            .clone()
    };
    assert_eq!(note("accepted_behind_ms").as_i64(), Some(0));
    let attempts = note("handshake_attempts");
    let attempts = attempts.as_array().unwrap();
    assert_eq!(attempts.len(), 1, "{attempts:?}");
    assert_eq!(attempts[0]["outcome"], "accepted");
    assert!(attempts[0]["round_trip_ms"].as_u64().is_some());
}
