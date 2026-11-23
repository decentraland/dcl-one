//! `pulse-handshake-clock` against a Pulse server in this process, whose clock is this machine's.
//! Pulse accepts a handshake signed up to its window (`MAX_TIMESTAMP_SKEW_MS`) either side of its
//! own now: a server on this clock accepts the probe's handshake signed at this machine's now and
//! its first rung, at the far edge of that window, and the notes say so. A server on another
//! clock is scripted from that check, against the verdict alone.

mod support;

use catalyst_comms::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;
use catalyst_comms_e2e::engine::{Outcome, RunOptions, run_scenarios};
use catalyst_comms_e2e::identity::now_ms;
use catalyst_comms_e2e::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot};
use catalyst_comms_e2e::scenarios::pulse_clock::{
    Answer, Attempt, BEHIND_MS, HANDSHAKE_CLOCK, Reading, answer_of, reading, signed_at, verdict,
};

/// Whether a Pulse server whose clock is `offset_ms` off this machine's (ahead when positive)
/// accepts a handshake signed `signed_ahead_ms` ahead of this machine's now that took
/// `one_way_ms` to reach it: the server's own check, `now.abs_diff(ts) > window` refused.
fn accepts(offset_ms: i64, signed_ahead_ms: i64, one_way_ms: i64) -> bool {
    (signed_ahead_ms - (offset_ms + one_way_ms)).abs() <= MAX_TIMESTAMP_SKEW_MS
}

/// The answer of a server on another clock: accepted, or refused for the skew window.
fn answered(accepted: bool) -> Answer {
    if accepted {
        Answer::Accepted
    } else {
        Answer::SkewRefused
    }
}

/// The attempts the probe records against such a server: the handshake signed at this
/// machine's now, then the ladder up to its first accepted rung.
fn scripted(offset_ms: i64) -> (Attempt, Vec<Attempt>) {
    let (one_way_ms, round_trip_ms) = (10, 20);
    let at_now = Attempt {
        behind_edge_ms: MAX_TIMESTAMP_SKEW_MS,
        answer: answered(accepts(offset_ms, 0, one_way_ms)),
        round_trip_ms,
    };
    let mut ladder = Vec::new();
    for behind in BEHIND_MS {
        let accepted = accepts(offset_ms, MAX_TIMESTAMP_SKEW_MS - behind, one_way_ms);
        ladder.push(Attempt {
            behind_edge_ms: *behind,
            answer: answered(accepted),
            round_trip_ms,
        });
        if accepted {
            break;
        }
    }
    (at_now, ladder)
}

#[test]
fn a_server_70_s_behind_fails_as_behind_though_a_deeper_rung_is_accepted() {
    let (at_now, ladder) = scripted(-70_000);
    assert!(!at_now.accepted());
    assert_eq!(ladder.last().map(Attempt::accepted), Some(true));
    let failure = verdict(&at_now, &ladder).unwrap_err();
    assert!(failure.contains("behind"), "{failure}");
    assert!(!failure.contains("ahead of this machine's by"), "{failure}");
    // the last refused rung and the accepted one with its round trip bound it
    assert!(failure.contains("more than 60000 ms"), "{failure}");
    assert!(failure.contains("at most 92020 ms"), "{failure}");
}

#[test]
fn a_server_90_s_ahead_fails_as_ahead_though_the_first_rung_is_accepted() {
    let (at_now, ladder) = scripted(90_000);
    assert!(!at_now.accepted());
    assert_eq!(ladder.len(), 1);
    assert!(ladder[0].accepted());
    let failure = verdict(&at_now, &ladder).unwrap_err();
    assert!(failure.contains("ahead"), "{failure}");
    assert!(failure.contains("more than 59980 ms"), "{failure}");
    assert!(failure.contains("at most 120000 ms"), "{failure}");
}

#[test]
fn a_server_5_s_behind_or_ahead_or_on_time_passes() {
    for offset in [-5_000, 0, 5_000] {
        let (at_now, ladder) = scripted(offset);
        assert!(at_now.accepted(), "{offset}");
        assert_eq!(verdict(&at_now, &ladder), Ok(()), "{offset}");
    }
}

#[test]
fn a_server_out_of_the_ladders_reach_fails_naming_both_ways() {
    for offset in [-200_000, 200_000] {
        let (at_now, ladder) = scripted(offset);
        assert!(ladder.iter().all(|attempt| !attempt.accepted()), "{offset}");
        let failure = verdict(&at_now, &ladder).unwrap_err();
        assert!(failure.contains("behind"), "{failure}");
        assert!(failure.contains("ahead"), "{failure}");
    }
}

#[test]
fn the_notes_claim_only_what_the_verdict_claims() {
    // behind: the behind bounds alone
    let (at_now, ladder) = scripted(-70_000);
    assert_eq!(
        reading(&at_now, &ladder),
        Reading {
            behind_more_than_ms: Some(60_000),
            behind_at_most_ms: Some(92_020),
            ..Reading::default()
        }
    );
    // ahead: the ahead bounds alone
    let (at_now, ladder) = scripted(90_000);
    assert_eq!(
        reading(&at_now, &ladder),
        Reading {
            ahead_more_than_ms: Some(59_980),
            ahead_at_most_ms: Some(120_000),
            ..Reading::default()
        }
    );
    // out of the ladder's reach: one way or the other, neither alone
    for offset in [-200_000, 200_000] {
        let (at_now, ladder) = scripted(offset);
        assert_eq!(
            reading(&at_now, &ladder),
            Reading {
                behind_or_ahead_more_than_ms: Some((92_000, 119_980)),
                ..Reading::default()
            },
            "{offset}"
        );
    }
    // a refusal for another reason: nothing about the clock
    let (_, ladder) = scripted(0);
    let at_now = Attempt {
        behind_edge_ms: MAX_TIMESTAMP_SKEW_MS,
        answer: Answer::Other("refused: too many handshakes".to_owned()),
        round_trip_ms: 20,
    };
    assert_eq!(reading(&at_now, &ladder), Reading::default());
}

#[test]
fn a_refusal_for_another_reason_fails_with_it_and_no_word_on_the_clock() {
    let other = |reason: &str| Attempt {
        behind_edge_ms: MAX_TIMESTAMP_SKEW_MS,
        answer: Answer::Other(reason.to_owned()),
        round_trip_ms: 20,
    };
    // the handshake signed at this machine's now
    let (_, ladder) = scripted(0);
    let failure = verdict(
        &other("refused: too many handshakes from this address"),
        &ladder,
    )
    .unwrap_err();
    assert!(
        failure.contains("too many handshakes from this address"),
        "{failure}"
    );
    assert!(!failure.contains("clock is"), "{failure}");
    // a rung: the ladder stops there, and the probe fails with that rung's reason
    let (at_now, _) = scripted(0);
    let rung = Attempt {
        behind_edge_ms: 0,
        ..other("disconnected: auth failed")
    };
    let failure = verdict(&at_now, &[rung]).unwrap_err();
    assert!(failure.contains("disconnected: auth failed"), "{failure}");
    assert!(failure.contains("0 ms behind the far edge"), "{failure}");
    assert!(!failure.contains("clock is"), "{failure}");
}

#[test]
fn only_the_skew_window_refusal_reads_as_skew() {
    use catalyst_comms::pulse::handshake::HandshakeError;
    assert_eq!(answer_of(&HandshakeOutcome::Accepted), Answer::Accepted);
    assert_eq!(
        answer_of(&HandshakeOutcome::Refused {
            error: Some(HandshakeError::StaleTimestamp.message())
        }),
        Answer::SkewRefused
    );
    for outcome in [
        HandshakeOutcome::Refused {
            error: Some(HandshakeError::InvalidAuthChain(String::new()).message()),
        },
        HandshakeOutcome::Refused { error: None },
        HandshakeOutcome::Disconnected { code: 2 },
    ] {
        assert!(
            matches!(answer_of(&outcome), Answer::Other(_)),
            "{outcome:?}"
        );
    }
}

/// Checks that every `<n> s window` in `text` names the server's window in whole seconds, and
/// returns how many there are.
fn window_mentions(text: &str) -> usize {
    let window_s = (MAX_TIMESTAMP_SKEW_MS / 1000).to_string();
    let mut found = 0;
    for (at, _) in text.match_indices(" s window") {
        let mut digits: Vec<char> = text[..at]
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.reverse();
        assert_eq!(digits.into_iter().collect::<String>(), window_s, "{text}");
        found += 1;
    }
    found
}

#[test]
fn the_numbers_typed_in_the_summary_and_the_readme_are_the_servers() {
    assert!(window_mentions(HANDSHAKE_CLOCK.summary) > 0);
    assert!(
        HANDSHAKE_CLOCK.summary.contains(&format!(
            "this machine's clock plus {} s",
            MAX_TIMESTAMP_SKEW_MS / 1000
        )),
        "{}",
        HANDSHAKE_CLOCK.summary
    );
    let deepest_s = BEHIND_MS.last().unwrap() / 1000;
    assert!(
        HANDSHAKE_CLOCK
            .summary
            .contains(&format!(" {deepest_s} s behind")),
        "{}",
        HANDSHAKE_CLOCK.summary
    );
    let readme = include_str!("../README.md");
    let mentions: usize = readme
        .lines()
        .filter(|line| line.contains("`pulse-handshake-clock`"))
        .map(window_mentions)
        .sum();
    assert!(mentions > 0, "the README names no window for the probe");
}

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
            // the probe reads this refusal, and only it, as the clock's
            assert_eq!(answer_of(&outcome), Answer::SkewRefused);
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
    // and the handshake signed at this machine's now, the verdict's own
    let at_now = note("signed_at_now");
    assert_eq!(at_now["outcome"], "accepted");
    assert_eq!(at_now["signed_ahead_ms"].as_i64(), Some(0));
    assert_eq!(
        attempts[0]["signed_ahead_ms"].as_i64(),
        Some(MAX_TIMESTAMP_SKEW_MS)
    );
    let round_trip = attempts[0]["round_trip_ms"].as_u64().unwrap();
    // nothing was refused, so nothing bounds the lag from below; the round trip bounds it above
    assert!(note("server_behind_more_than_ms").is_null());
    assert_eq!(note("server_behind_at_most_ms").as_u64(), Some(round_trip));
}
