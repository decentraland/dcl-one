//! `pulse-handshake-clock` against a Pulse server in this process, whose clock is this machine's.
//! Pulse accepts a handshake signed up to its window (`MAX_TIMESTAMP_SKEW_MS`) either side of its
//! own now, so the probe signs its first handshake at the far edge of that window: a server on
//! this clock accepts it, and the notes say so.

mod support;

use catalyst_comms::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;
use catalyst_comms_e2e::engine::{Outcome, RunOptions, run_scenarios};
use catalyst_comms_e2e::identity::now_ms;
use catalyst_comms_e2e::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot};
use catalyst_comms_e2e::scenarios::pulse_clock::{BEHIND_MS, HANDSHAKE_CLOCK, signed_at};

#[test]
fn the_ladder_starts_at_the_windows_edge_and_reaches_past_this_machines_now() {
    assert_eq!(BEHIND_MS[0], 0);
    assert!(
        BEHIND_MS.windows(2).all(|pair| pair[0] < pair[1]),
        "{BEHIND_MS:?}"
    );
    // one rung signs at this machine's now, and the ladder goes on behind it
    assert!(BEHIND_MS.contains(&MAX_TIMESTAMP_SKEW_MS), "{BEHIND_MS:?}");
    assert!(*BEHIND_MS.last().unwrap() > MAX_TIMESTAMP_SKEW_MS);
    assert_eq!(signed_at(1_000_000, 0), 1_000_000 + MAX_TIMESTAMP_SKEW_MS);
    assert_eq!(signed_at(1_000_000, MAX_TIMESTAMP_SKEW_MS), 1_000_000);
}

/// What the probe stands on: the server refuses a handshake signed past its window ahead, and
/// accepts one signed at the window's edge.
#[tokio::test(flavor = "multi_thread")]
async fn the_server_accepts_the_windows_edge_and_refuses_past_it() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "edge");
    for (index, past_the_edge) in [true, false].into_iter().enumerate() {
        let identity = ctx.identity(index as u32);
        let bot = PulseBot::dial(&ctx, &format!("edge-{index}"), ConnectOptions::default())
            .await
            .unwrap();
        let timestamp = if past_the_edge {
            signed_at(now_ms(), 0) + 5_000
        } else {
            signed_at(now_ms(), 0)
        };
        bot.send_handshake(
            &identity,
            &HandshakeOptions {
                timestamp_ms: Some(timestamp),
                ..Default::default()
            },
        );
        let outcome = bot.handshake_outcome(&ctx, None).await.unwrap();
        if past_the_edge {
            let HandshakeOutcome::Refused { error } = &outcome else {
                panic!("signed 5 s past the window: {outcome:?}");
            };
            assert!(
                error.as_deref().unwrap_or_default().contains("skew window"),
                "{error:?}"
            );
        } else {
            assert_eq!(outcome, HandshakeOutcome::Accepted);
        }
        bot.close();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_on_this_clock_accepts_the_first_rung() {
    let pulse = support::in_process_pulse().await;
    let options = RunOptions {
        repeat: Some(1),
        seed: Some(3),
        ..Default::default()
    };
    let mut report = run_scenarios(support::stack_at(pulse), &[&HANDSHAKE_CLOCK], &options).await;
    let result = report.results.remove(0);
    assert!(
        matches!(result.outcome, Outcome::Passed),
        "{:?}",
        result.outcome
    );
    let note = |key: &str| {
        result
            .notes
            .iter()
            .find(|note| note.key == key)
            .unwrap_or_else(|| panic!("no note {key}: {:?}", result.notes))
            .value
            .clone()
    };
    assert_eq!(note("window_ms").as_i64(), Some(MAX_TIMESTAMP_SKEW_MS));
    assert_eq!(note("accepted_behind_edge_ms").as_i64(), Some(0));
    let attempts = note("handshake_attempts");
    let attempts = attempts.as_array().unwrap();
    assert_eq!(attempts.len(), 1, "{attempts:?}");
    assert_eq!(attempts[0]["outcome"], "accepted");
    assert_eq!(
        attempts[0]["signed_ahead_ms"].as_i64(),
        Some(MAX_TIMESTAMP_SKEW_MS)
    );
    let round_trip = attempts[0]["round_trip_ms"].as_u64().unwrap();
    // nothing was refused, so nothing bounds the lag from below; the round trip bounds it above
    assert!(note("server_behind_more_than_ms").is_null());
    assert_eq!(note("server_behind_at_most_ms").as_u64(), Some(round_trip));
}
