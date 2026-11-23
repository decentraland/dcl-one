//! The Pulse scenarios against a real Pulse server in this process: each passes against a
//! correct server, and the ones that guard a limit fail against a server that lacks it.

mod support;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use catalyst_comms::pulse::PulseServer;
use catalyst_comms::pulse::hardening::{DisconnectReason, GameplayRateLimiter, PreAuthAdmission};
use catalyst_comms_e2e::engine::{Outcome, RunOptions, Scenario, ScenarioResult, run_scenarios};
use catalyst_comms_e2e::identity::Identity;
use catalyst_comms_e2e::pulse::{HandshakeOutcome, PulseBot};
use catalyst_comms_e2e::scenarios::pulse::{self, *};
use catalyst_comms_e2e::scenarios::pulse_enet;
use catalyst_comms_e2e::stack::Stack;
use sha2::{Digest, Sha256};

/// The variable the scene listener scenario reads its wallet key from, in these tests.
const LISTENER_KEY_VARIABLE: &str = "COMMS_E2E_TEST_SCENE_LISTENER_KEY";

fn options(repeat: u32) -> RunOptions {
    RunOptions {
        repeat: Some(repeat),
        seed: Some(7),
        ..Default::default()
    }
}

async fn run_on(
    server: PulseServer,
    stack: impl FnOnce(&mut Stack),
    scenario: &'static Scenario,
    options: RunOptions,
) -> ScenarioResult {
    let address = support::serve(server).await;
    let mut stack_file = Stack::pulse_only(address);
    stack(&mut stack_file);
    let mut report = run_scenarios(Arc::new(stack_file), &[scenario], &options).await;
    report.results.remove(0)
}

async fn run(server: PulseServer, scenario: &'static Scenario, repeat: u32) -> ScenarioResult {
    run_on(server, |_| {}, scenario, options(repeat)).await
}

fn assert_passed(result: &ScenarioResult) {
    match &result.outcome {
        Outcome::Passed => {}
        Outcome::Failed {
            expectation,
            seed,
            repeat,
            timeline,
        } => {
            let tail: Vec<String> = timeline
                .iter()
                .filter(|entry| entry.actor == "scenario")
                .chain(timeline.iter().rev().take(25).rev())
                .map(|entry| format!("{entry:?}"))
                .collect();
            panic!(
                "{} failed on repeat {repeat} (seed {seed}): {expectation}\n--- last entries\n{}",
                result.name,
                tail.join("\n")
            );
        }
        Outcome::Skipped { reason } => panic!("{} was skipped: {reason}", result.name),
    }
}

fn failure_of(result: &ScenarioResult) -> String {
    match &result.outcome {
        Outcome::Failed { expectation, .. } => expectation.clone(),
        other => panic!(
            "{} should have failed against this server, got {other:?}",
            result.name
        ),
    }
}

/// A command that does nothing to Pulse and exits with `code`.
fn exits_with(code: u8) -> Vec<String> {
    let (shell, flag) = if cfg!(windows) {
        ("cmd", "/c")
    } else {
        ("sh", "-c")
    };
    vec![shell.to_owned(), flag.to_owned(), format!("exit {code}")]
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_command_that_fails_is_reported_as_the_commands_failure() {
    let started = Instant::now();
    let result = run_on(
        PulseServer::new(),
        |stack| stack.pulse_restart_command = Some(exits_with(3)),
        &pulse_enet::GRACEFUL_STOP,
        // above what the scenario asks of its deadline, whatever the run's default is
        RunOptions {
            scenario_deadline: Duration::from_secs(400),
            ..options(1)
        },
    )
    .await;
    let failure = failure_of(&result);
    assert!(
        failure.contains("the Pulse restart command failed"),
        "{failure}"
    );
    assert!(!failure.contains("must be told"), "{failure}");
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_is_refused_when_the_deadline_cannot_hold_the_restart() {
    let started = Instant::now();
    let result = run_on(
        PulseServer::new(),
        |stack| stack.pulse_restart_command = Some(exits_with(0)),
        &pulse_enet::GRACEFUL_STOP,
        RunOptions {
            scenario_deadline: Duration::from_secs(60),
            ..options(1)
        },
    )
    .await;
    let failure = failure_of(&result);
    assert!(
        failure.contains("the Pulse restart command may take"),
        "{failure}"
    );
    assert!(failure.contains("--deadline 295 or more"), "{failure}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn every_pulse_scenario_is_registered_with_its_tags_guards_and_summary() {
    let all = pulse::all();
    let names: HashSet<&str> = all.iter().map(|scenario| scenario.name).collect();
    assert_eq!(names.len(), all.len(), "names are unique");
    for expected in [
        "pulse-smoke",
        "pulse-refusal-codes",
        "pulse-auth-timeout",
        "pulse-rate-limits",
        "pulse-same-wallet-race",
        "pulse-handshake-swarm",
        "pulse-churn",
        "pulse-realm-round-trip",
        "pulse-emote-lifecycle",
        "pulse-resync-requests",
        "pulse-stall-and-resume",
        "pulse-scene-listener",
    ] {
        assert!(names.contains(expected), "{expected} is missing");
    }
    for scenario in &all {
        assert!(
            scenario.tags.contains(&"pulse"),
            "{} lacks the pulse tag",
            scenario.name
        );
        assert!(
            scenario.tags.len() >= 2,
            "{} lacks a group tag",
            scenario.name
        );
        assert!(
            !scenario.guards.is_empty(),
            "{} names no guard",
            scenario.name
        );
        assert!(
            !scenario.summary.is_empty(),
            "{} has no summary",
            scenario.name
        );
    }
    assert_eq!(SAME_WALLET_RACE.default_repeat, 20);
    assert!(REFUSAL_CODES.hostile && RATE_LIMITS.hostile && HANDSHAKE_SWARM.hostile);
    assert!(AUTH_TIMEOUT.tags.contains(&"slow"));
    // the registry the command line uses carries them all
    let registry = catalyst_comms_e2e::scenarios::registry();
    for scenario in &all {
        assert!(registry.iter().any(|known| known.name == scenario.name));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn smoke_passes() {
    assert_passed(&run(PulseServer::new(), &SMOKE, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn refusal_codes_pass() {
    assert_passed(&run(PulseServer::new(), &REFUSAL_CODES, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn refusal_codes_fail_against_a_server_without_the_emote_limit() {
    let mut server = PulseServer::new();
    server.max_emote_duration_ms = u32::MAX;
    let failure = failure_of(&run(server, &REFUSAL_CODES, 1).await);
    assert!(failure.contains("emote-too-long"), "{failure}");
    assert!(failure.contains("12 (InvalidEmoteField)"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn refusal_codes_fail_against_a_server_without_the_handshake_attempt_limit() {
    let mut server = PulseServer::new();
    server.attempt_policy = catalyst_comms::pulse::hardening::HandshakeAttemptPolicy::new(0);
    let failure = failure_of(&run(server, &REFUSAL_CODES, 1).await);
    assert!(failure.contains("3 (AuthFailed)"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn rate_limits_pass() {
    assert_passed(&run(PulseServer::new(), &RATE_LIMITS, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn rate_limits_fail_against_a_server_with_a_loosened_input_limiter() {
    let mut server = PulseServer::new();
    // a burst of 255 and 1000 inputs a second: the scenario's flood of 52 never reaches it
    server.gameplay_limiter = GameplayRateLimiter::new(1000, 255, 20, 16);
    let failure = failure_of(&run(server, &RATE_LIMITS, 1).await);
    assert!(failure.contains("input-over"), "{failure}");
    assert!(failure.contains("over the budget"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn rate_limits_fail_against_a_server_that_limits_too_hard() {
    let mut server = PulseServer::new();
    // a steady 20 a second is the budget; a server that allows 5 ends the bot sending it
    server.gameplay_limiter = GameplayRateLimiter::new(5, 2, 20, 16);
    let failure = failure_of(&run(server, &RATE_LIMITS, 1).await);
    assert!(failure.contains("InputRateExceeded"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn same_wallet_race_passes_in_both_orders() {
    // six repeats: both orders (`first_of_two` alternates) and a spread of gaps from the seed
    let result = run(PulseServer::new(), &SAME_WALLET_RACE, 6).await;
    assert_passed(&result);
    assert_eq!(result.repeats_run, 6);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn handshake_swarm_passes() {
    assert_passed(&run(PulseServer::new(), &HANDSHAKE_SWARM, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn handshake_swarm_fails_against_a_server_without_a_per_address_budget() {
    let mut server = PulseServer::new();
    server.pre_auth = PreAuthAdmission::new(0, 0);
    let failure = failure_of(&run(server, &HANDSHAKE_SWARM, 1).await);
    assert!(failure.contains("allowed"), "{failure}");
    assert!(failure.contains("accepted"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn handshake_swarm_fails_against_a_server_that_refuses_within_the_budget() {
    let mut server = PulseServer::new();
    server.pre_auth = PreAuthAdmission::new(4, 512);
    let failure = failure_of(&run(server, &HANDSHAKE_SWARM, 1).await);
    assert!(failure.contains("within the budget"), "{failure}");
    assert!(failure.contains("PreAuthIpLimitExhausted"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn churn_passes() {
    assert_passed(&run(PulseServer::new(), &CHURN, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn realm_round_trip_passes() {
    assert_passed(&run(PulseServer::new(), &REALM_ROUND_TRIP, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn emote_lifecycle_passes() {
    assert_passed(&run(PulseServer::new(), &EMOTE_LIFECYCLE, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn resync_requests_pass() {
    let result = run(PulseServer::new(), &RESYNC_REQUESTS, 2).await;
    assert_passed(&result);
    // this stack names no metrics address: the report says the server's count was not checked
    let unchecked: Vec<_> = result
        .notes
        .iter()
        .filter(|note| note.key == "resync_metric")
        .collect();
    assert_eq!(
        unchecked.len(),
        2,
        "one for each repeat: {:?}",
        result.notes
    );
    let said = unchecked[0].value.as_str().unwrap_or_default();
    assert!(said.contains("not checked"), "{said}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn resync_requests_fail_when_the_metrics_the_stack_file_names_cannot_be_read() {
    let nothing_serves_it = |stack: &mut Stack| {
        stack.pulse_metrics = Some("http://127.0.0.1:9/metrics".parse().unwrap());
    };
    let result = run_on(
        PulseServer::new(),
        nothing_serves_it,
        &RESYNC_REQUESTS,
        options(1),
    )
    .await;
    let failure = failure_of(&result);
    assert!(failure.contains("pulse_metrics"), "{failure}");
}

/// A key for the scene listener wallet, derived from text so that no key is written down.
fn listener_key() -> String {
    hex::encode(Sha256::digest(b"comms-e2e test scene listener wallet"))
}

/// Names the variable of the scene listener key and gives its value, so no test writes the
/// environment.
fn name_listener_key(stack: &mut Stack) {
    stack.scene_listener_key_env = Some(LISTENER_KEY_VARIABLE.to_owned());
    stack.give_secret(LISTENER_KEY_VARIABLE, &listener_key());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn scene_listener_passes_when_the_server_admits_its_wallet() {
    let key = listener_key();
    let wallet = Identity::from_wallet_key(&key, "test").unwrap().address();
    let mut server = PulseServer::new();
    server.scene_listener_wallets = Some(HashSet::from([wallet]));
    assert_passed(&run_on(server, name_listener_key, &SCENE_LISTENER, options(1)).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn scene_listener_is_skipped_without_a_wallet_key_in_the_stack_file() {
    let result = run(PulseServer::new(), &SCENE_LISTENER, 1).await;
    match result.outcome {
        Outcome::Skipped { reason } => {
            assert!(reason.contains("scene_listener_key_env"), "{reason}")
        }
        other => panic!("expected a skip, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn scene_listener_fails_when_the_server_does_not_admit_the_wallet() {
    let mut server = PulseServer::new();
    server.scene_listener_wallets = Some(HashSet::from([
        "0x0000000000000000000000000000000000000001".to_owned(),
    ]));
    let failure = failure_of(&run_on(server, name_listener_key, &SCENE_LISTENER, options(1)).await);
    assert!(failure.contains("scene listener"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn a_wallet_the_server_does_not_admit_gets_no_listener() {
    // what the scenario relies on: the allow-list is the wallet configured, nobody else
    let key = listener_key();
    let allowed = Identity::from_wallet_key(&key, "test").unwrap();
    let stranger = Identity::derive("a stranger", 0);
    let mut server = PulseServer::new();
    server.scene_listener_wallets = Some(HashSet::from([allowed.address()]));
    let pulse = support::serve(server).await;
    let ctx = support::ctx(pulse, "listener-allow-list");
    let area = vec![catalyst_comms_e2e::pulse::messages::area_of_interest(
        "realm",
        (0, 0),
        (1, 1),
    )];

    let refused = PulseBot::dial(&ctx, "stranger", Default::default())
        .await
        .unwrap();
    refused.send_listener_handshake(&stranger, area.clone());
    assert_eq!(
        refused
            .handshake_outcome(&ctx, Some(Duration::from_secs(5)))
            .await
            .unwrap(),
        HandshakeOutcome::Disconnected {
            code: DisconnectReason::AuthFailed.code()
        }
    );
    let admitted = PulseBot::dial(&ctx, "allowed", Default::default())
        .await
        .unwrap();
    admitted.send_listener_handshake(&allowed, area);
    assert_eq!(
        admitted
            .handshake_outcome(&ctx, Some(Duration::from_secs(5)))
            .await
            .unwrap(),
        HandshakeOutcome::Accepted
    );
}

/// A bot the server ends unexpectedly mid-scenario fails the scenario at once, with the code in
/// the message, not at the deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn a_scenario_fails_fast_when_the_server_ends_a_bot_unexpectedly() {
    let mut server = PulseServer::new();
    // one input a second: the walker of the smoke scenario is ended on its second input
    server.gameplay_limiter = GameplayRateLimiter::new(1, 1, 1, 1);
    let started = Instant::now();
    let result = run_on(
        server,
        |_| {},
        &SMOKE,
        RunOptions {
            repeat: Some(1),
            seed: Some(7),
            scenario_deadline: Duration::from_secs(120),
            ..Default::default()
        },
    )
    .await;
    let failure = failure_of(&result);
    // at once, not at the 120 s deadline: half of it is a margin a loaded machine keeps
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "the scenario took {:?} to fail: {failure}",
        started.elapsed()
    );
    assert!(
        failure.contains("InputRateExceeded") || failure.contains("DiscreteEventRateExceeded"),
        "{failure}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn emote_lifecycle_fails_against_a_server_that_refuses_the_emote() {
    let mut server = PulseServer::new();
    // an emote limit shorter than the emote the scenario plays
    server.max_emote_duration_ms = 100;
    let failure = failure_of(&run(server, &EMOTE_LIFECYCLE, 1).await);
    assert!(failure.contains("InvalidEmoteField"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn realm_round_trip_fails_against_a_server_that_limits_teleports_too_hard() {
    let mut server = PulseServer::new();
    // one discrete event at a time: the second teleport ends the traveller
    server.gameplay_limiter = GameplayRateLimiter::new(20, 16, 1, 1);
    let failure = failure_of(&run(server, &REALM_ROUND_TRIP, 1).await);
    assert!(failure.contains("DiscreteEventRateExceeded"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "waits for the server's 30 s authentication timeout"]
async fn auth_timeout_passes() {
    assert_passed(&run(PulseServer::new(), &AUTH_TIMEOUT, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "the stall past the transport timeout takes about 15 s"]
async fn stall_and_resume_passes() {
    let result = run(PulseServer::new(), &STALL_AND_RESUME, 1).await;
    assert_passed(&result);
    println!("{:?}", result.notes);
}

// ---- the same scenarios over WebTransport, against the same server listening on both ----

async fn run_webtransport_on(
    server: PulseServer,
    stack: impl FnOnce(&mut Stack),
    scenario: &'static Scenario,
    options: RunOptions,
) -> ScenarioResult {
    let pulse = support::serve_both(server).await;
    let mut stack_file = Stack::pulse_only(pulse.enet);
    stack_file.pulse_webtransport = Some(pulse.webtransport.clone());
    stack(&mut stack_file);
    let options = RunOptions {
        transport: catalyst_comms_e2e::engine::Transport::WebTransport,
        ..options
    };
    let mut report = run_scenarios(Arc::new(stack_file), &[scenario], &options).await;
    report.results.remove(0)
}

async fn run_webtransport(
    server: PulseServer,
    scenario: &'static Scenario,
    repeat: u32,
) -> ScenarioResult {
    run_webtransport_on(server, |_| {}, scenario, options(repeat)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_smoke_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &SMOKE, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_refusal_codes_pass() {
    assert_passed(&run_webtransport(PulseServer::new(), &REFUSAL_CODES, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_rate_limits_pass() {
    assert_passed(&run_webtransport(PulseServer::new(), &RATE_LIMITS, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_same_wallet_race_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &SAME_WALLET_RACE, 6).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_handshake_swarm_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &HANDSHAKE_SWARM, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_handshake_swarm_fails_against_a_server_without_a_per_address_budget() {
    let mut server = PulseServer::new();
    server.pre_auth = PreAuthAdmission::new(0, 0);
    let failure =
        failure_of(&run_webtransport_on(server, |_| {}, &HANDSHAKE_SWARM, options(1)).await);
    assert!(failure.contains("allowed"), "{failure}");
    assert!(failure.contains("accepted"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_handshake_swarm_fails_against_a_server_that_refuses_within_the_budget() {
    let mut server = PulseServer::new();
    server.pre_auth = PreAuthAdmission::new(4, 512);
    let failure =
        failure_of(&run_webtransport_on(server, |_| {}, &HANDSHAKE_SWARM, options(1)).await);
    assert!(failure.contains("within the budget"), "{failure}");
    assert!(failure.contains("PreAuthIpLimitExhausted"), "{failure}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_churn_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &CHURN, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_realm_round_trip_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &REALM_ROUND_TRIP, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_emote_lifecycle_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &EMOTE_LIFECYCLE, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_resync_requests_pass() {
    assert_passed(&run_webtransport(PulseServer::new(), &RESYNC_REQUESTS, 2).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn webtransport_scene_listener_passes_when_the_server_admits_its_wallet() {
    let key = listener_key();
    let wallet = Identity::from_wallet_key(&key, "test").unwrap().address();
    let mut server = PulseServer::new();
    server.scene_listener_wallets = Some(HashSet::from([wallet]));
    assert_passed(
        &run_webtransport_on(server, name_listener_key, &SCENE_LISTENER, options(1)).await,
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "waits for the server's 30 s authentication timeout"]
async fn webtransport_auth_timeout_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &AUTH_TIMEOUT, 1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "the stall past the transport timeout takes about 15 s"]
async fn webtransport_stall_and_resume_passes() {
    assert_passed(&run_webtransport(PulseServer::new(), &STALL_AND_RESUME, 1).await);
}

/// Other people come and go on a shared stack, and the server tells every connected player when
/// any player leaves: the scenario counts what it saw of its own traveller, nothing else.
async fn amid_strangers(
    scenario: &'static Scenario,
    transport: catalyst_comms_e2e::engine::Transport,
) {
    let pulse = support::serve_both(PulseServer::new()).await;
    let strangers = pulse.ctx_on(transport, "strangers");
    let churn = tokio::spawn(async move {
        for number in 0_u32..200 {
            let identity = Identity::derive("a stranger passing by", number);
            if let Ok(bot) = PulseBot::enter(&strangers, "stranger", &identity).await {
                bot.close();
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    });
    let mut stack_file = Stack::pulse_only(pulse.enet);
    stack_file.pulse_webtransport = Some(pulse.webtransport.clone());
    let options = RunOptions {
        transport,
        ..options(1)
    };
    let mut report = run_scenarios(Arc::new(stack_file), &[scenario], &options).await;
    churn.abort();
    assert_passed(&report.results.remove(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn realm_round_trip_counts_only_its_own_traveller_on_enet() {
    amid_strangers(
        &REALM_ROUND_TRIP,
        catalyst_comms_e2e::engine::Transport::Enet,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn realm_round_trip_counts_only_its_own_traveller_on_webtransport() {
    amid_strangers(
        &REALM_ROUND_TRIP,
        catalyst_comms_e2e::engine::Transport::WebTransport,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn smoke_counts_only_its_own_traveller_on_enet() {
    amid_strangers(&SMOKE, catalyst_comms_e2e::engine::Transport::Enet).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn smoke_counts_only_its_own_traveller_on_webtransport() {
    amid_strangers(&SMOKE, catalyst_comms_e2e::engine::Transport::WebTransport).await;
}
