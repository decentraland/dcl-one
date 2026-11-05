use super::*;

/// A refusal and the wait it gets as the first failure of a fresh policy.
const REFUSALS: [(PulseDisconnect, f64); 5] = [
    (PulseDisconnect::ServerFull, 60.0),
    (PulseDisconnect::PreAuthIpLimit, 10.0),
    (PulseDisconnect::PreAuthBudget, 10.0),
    (PulseDisconnect::IpConnectionLimitExceeded, 10.0),
    (PulseDisconnect::SceneListenerIpLimitExceeded, 10.0),
];

/// A way for the failures to stop counting.
type Restart = fn(&mut ReconnectPolicy);

/// The two ways the failures stop counting, by name.
const RESTARTS: [(&str, Restart); 2] = [
    ("on_accepted", ReconnectPolicy::on_accepted),
    ("reset", ReconnectPolicy::reset),
];

/// The server's reason for a drop, on a connection whose handshake was `accepted` or not.
fn disconnected(reason: PulseDisconnect, accepted: bool) -> Failure {
    Failure::Disconnected { reason, accepted }
}

/// A scene listener turned away because its IP already holds as many as it may.
fn listener_refused() -> Failure {
    disconnected(PulseDisconnect::SceneListenerIpLimitExceeded, false)
}

#[test]
fn a_drop_after_acceptance_waits_three_seconds() {
    for reason in [
        PulseDisconnect::None,
        PulseDisconnect::Graceful,
        PulseDisconnect::AuthTimeout,
        PulseDisconnect::AuthFailed,
    ] {
        let mut policy = ReconnectPolicy::exact();

        let next = policy.next(disconnected(reason, true), 100.0);

        assert_eq!(next, Next::RetryAt(103.0), "{reason:?}");
    }
}

#[test]
fn failed_attempts_back_off_from_one_second_to_sixty() {
    let mut policy = ReconnectPolicy::exact();

    let waits: Vec<Next> = (0..8)
        .map(|_| policy.next(Failure::Unreached, 0.0))
        .collect();

    let expected = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0].map(Next::RetryAt);
    assert_eq!(waits, expected);
}

#[test]
fn the_backoff_follows_a_drop_after_acceptance() {
    let mut policy = ReconnectPolicy::exact();
    policy.on_accepted();

    assert_eq!(
        policy.next(disconnected(PulseDisconnect::Graceful, true), 0.0),
        Next::RetryAt(3.0)
    );
    assert_eq!(policy.next(Failure::Unreached, 0.0), Next::RetryAt(1.0));
    assert_eq!(policy.next(Failure::Unreached, 0.0), Next::RetryAt(2.0));
}

#[test]
fn acceptance_and_reset_restart_the_backoff() {
    for (name, restart) in RESTARTS {
        let mut policy = ReconnectPolicy::exact();
        assert_eq!(
            policy.next(Failure::Unreached, 0.0),
            Next::RetryAt(1.0),
            "{name}"
        );
        assert_eq!(
            policy.next(Failure::Unreached, 0.0),
            Next::RetryAt(2.0),
            "{name}"
        );

        restart(&mut policy);

        assert_eq!(
            policy.next(Failure::Unreached, 0.0),
            Next::RetryAt(1.0),
            "{name}"
        );
    }
}

#[test]
fn refusals_raise_the_wait_to_their_floor() {
    let mut fresh = ReconnectPolicy::exact();
    assert_eq!(
        fresh.next(disconnected(PulseDisconnect::ServerFull, false), 0.0),
        Next::RetryAt(60.0)
    );

    for reason in [
        PulseDisconnect::PreAuthIpLimit,
        PulseDisconnect::PreAuthBudget,
        PulseDisconnect::IpConnectionLimitExceeded,
    ] {
        let mut policy = ReconnectPolicy::exact();

        let next = policy.next(disconnected(reason, false), 0.0);

        assert_eq!(next, Next::RetryAt(10.0), "{reason:?}");
    }

    // Once the attempt backoff has outgrown a floor, the longer wait is the one that holds.
    let mut policy = ReconnectPolicy::exact();
    for wait in [1.0, 2.0, 4.0, 8.0, 16.0, 32.0] {
        assert_eq!(policy.next(Failure::Unreached, 0.0), Next::RetryAt(wait));
    }
    assert_eq!(
        policy.next(disconnected(PulseDisconnect::PreAuthBudget, false), 0.0),
        Next::RetryAt(60.0)
    );
}

#[test]
fn the_listener_limit_doubles_to_five_minutes() {
    let mut policy = ReconnectPolicy::exact();

    let waits: Vec<Next> = (0..7)
        .map(|_| policy.next(listener_refused(), 0.0))
        .collect();

    let expected = [10.0, 20.0, 40.0, 80.0, 160.0, 300.0, 300.0].map(Next::RetryAt);
    assert_eq!(waits, expected);
}

#[test]
fn handshake_failures_take_the_backoff() {
    let mut policy = ReconnectPolicy::exact();

    assert_eq!(
        policy.next(Failure::HandshakeFailed, 0.0),
        Next::RetryAt(1.0)
    );
    assert_eq!(
        policy.next(Failure::HandshakeFailed, 0.0),
        Next::RetryAt(2.0)
    );
}

#[test]
fn terminal_reasons_park_the_session() {
    let terminal = [
        PulseDisconnect::DuplicateSession,
        PulseDisconnect::Banned,
        PulseDisconnect::InputRateExceeded,
        PulseDisconnect::DiscreteEventRateExceeded,
        PulseDisconnect::InvalidInputField,
        PulseDisconnect::InvalidEmoteField,
        PulseDisconnect::InvalidTeleportField,
        PulseDisconnect::HandshakeReplayRejected,
        PulseDisconnect::InvalidHandshakeField,
        PulseDisconnect::PacketCorrupted,
        PulseDisconnect::InvalidSceneListenerField,
        PulseDisconnect::Unknown(42),
    ];

    for reason in terminal {
        for accepted in [false, true] {
            let mut policy = ReconnectPolicy::exact();

            let next = policy.next(disconnected(reason, accepted), 0.0);

            assert_eq!(next, Next::Dead, "{reason:?}, accepted: {accepted}");
        }
    }
}

#[test]
fn codes_17_and_18_are_named() {
    assert_eq!(
        PulseDisconnect::from_code(17),
        PulseDisconnect::IpConnectionLimitExceeded
    );
    assert_eq!(
        PulseDisconnect::from_code(18),
        PulseDisconnect::SceneListenerIpLimitExceeded
    );
    assert_eq!(PulseDisconnect::from_code(42), PulseDisconnect::Unknown(42));
}

// The corners the cases above leave: a drop before acceptance, a refusal after it, and how the
// listener count moves.

#[test]
fn a_drop_before_acceptance_takes_the_backoff() {
    for reason in [
        PulseDisconnect::None,
        PulseDisconnect::Graceful,
        PulseDisconnect::AuthTimeout,
        PulseDisconnect::AuthFailed,
    ] {
        let mut policy = ReconnectPolicy::exact();

        let first = policy.next(disconnected(reason, false), 0.0);
        let second = policy.next(disconnected(reason, false), 0.0);

        assert_eq!(first, Next::RetryAt(1.0), "{reason:?}");
        assert_eq!(second, Next::RetryAt(2.0), "{reason:?}");
    }
}

#[test]
fn a_refusal_takes_its_floor_and_counts_as_a_failed_attempt() {
    for (reason, floor) in REFUSALS {
        for accepted in [false, true] {
            let mut policy = ReconnectPolicy::exact();
            if accepted {
                policy.on_accepted();
            }

            let refused = policy.next(disconnected(reason, accepted), 0.0);
            let after = policy.next(Failure::Unreached, 0.0);

            let case = format!("{reason:?}, accepted: {accepted}");
            assert_eq!(refused, Next::RetryAt(floor), "{case}");
            assert_eq!(after, Next::RetryAt(2.0), "{case}");
        }
    }
}

#[test]
fn the_listener_limit_waits_at_least_as_long_as_the_attempt_backoff() {
    let mut policy = ReconnectPolicy::exact();
    for _ in 0..6 {
        policy.next(Failure::Unreached, 0.0);
    }

    // The seventh failed attempt's backoff, 60 s, outlasts the listener sequence's first 10 s.
    assert_eq!(policy.next(listener_refused(), 0.0), Next::RetryAt(60.0));
}

#[test]
fn other_failures_do_not_move_the_listener_limit() {
    let mut policy = ReconnectPolicy::exact();

    assert_eq!(policy.next(listener_refused(), 0.0), Next::RetryAt(10.0));
    assert_eq!(policy.next(Failure::Unreached, 0.0), Next::RetryAt(2.0));
    assert_eq!(policy.next(listener_refused(), 0.0), Next::RetryAt(20.0));
}

#[test]
fn acceptance_and_reset_restart_the_listener_limit() {
    for (name, restart) in RESTARTS {
        let mut policy = ReconnectPolicy::exact();
        assert_eq!(
            policy.next(listener_refused(), 0.0),
            Next::RetryAt(10.0),
            "{name}"
        );
        assert_eq!(
            policy.next(listener_refused(), 0.0),
            Next::RetryAt(20.0),
            "{name}"
        );

        restart(&mut policy);

        assert_eq!(
            policy.next(listener_refused(), 0.0),
            Next::RetryAt(10.0),
            "{name}"
        );
    }
}

/// The shortest factor the jitter can pick.
fn shortest() -> f64 {
    1.0 - JITTER
}

/// The longest factor the jitter can pick.
fn longest() -> f64 {
    1.0 + JITTER
}

/// How long after `now` the policy said to retry.
fn wait(next: Next, now: f64) -> f64 {
    match next {
        Next::RetryAt(at) => at - now,
        Next::Dead => panic!("no retry"),
    }
}

/// Each wait strays by up to a fifth either way: the drop after acceptance, the attempt backoff
/// and every refusal's floor alike.
#[test]
fn every_wait_strays_by_up_to_a_fifth() {
    let ends: [(fn() -> f64, f64); 2] = [(shortest, 1.0 - JITTER), (longest, 1.0 + JITTER)];
    for (jitter, factor) in ends {
        let mut cases = vec![
            (
                "a drop after acceptance",
                disconnected(PulseDisconnect::Graceful, true),
                DEFAULT_RETRY_DELAY,
            ),
            (
                "a failed attempt",
                Failure::Unreached,
                ATTEMPT_BACKOFF_START,
            ),
        ];
        for (reason, floor) in REFUSALS {
            cases.push(("a refusal", disconnected(reason, false), floor));
        }
        for (name, failure, length) in cases {
            let mut policy = ReconnectPolicy::with_jitter(jitter);

            let got = wait(policy.next(failure, 100.0), 100.0);

            assert!(
                (got - length * factor).abs() < 1e-9,
                "{name}: waited {got} s for {length} s at {factor}"
            );
        }
    }
}

/// The random factor stays within a fifth either way, and differs from one draw to the next.
#[test]
fn the_random_jitter_stays_within_a_fifth_and_varies() {
    let draws: Vec<f64> = (0..1000).map(|_| random_jitter()).collect();

    assert!(
        draws
            .iter()
            .all(|factor| (1.0 - JITTER..=1.0 + JITTER).contains(factor)),
        "{draws:?}"
    );
    let low = draws.iter().copied().fold(f64::MAX, f64::min);
    let high = draws.iter().copied().fold(f64::MIN, f64::max);
    assert!(low < 0.9 && high > 1.1, "{low}..{high}");
}

/// A restart forgets the failures, not how the waits are scaled.
#[test]
fn a_restart_keeps_the_jitter() {
    for (name, restart) in RESTARTS {
        let mut policy = ReconnectPolicy::with_jitter(longest);
        policy.next(Failure::Unreached, 0.0);

        restart(&mut policy);

        let got = wait(policy.next(Failure::Unreached, 0.0), 0.0);
        assert!(
            (got - ATTEMPT_BACKOFF_START * (1.0 + JITTER)).abs() < 1e-9,
            "{name}: waited {got} s"
        );
    }
}
