//! The crowd scenario against a real Pulse server in this process: it passes against a correct
//! server on both transports, and fails, naming how many players saw everyone, against a server
//! whose area of interest is smaller than the crowd.

mod support;

use std::sync::Arc;

use catalyst_comms::pulse::PulseServer;
use catalyst_comms::pulse::hardening::DisconnectReason;
use catalyst_comms::pulse::interest::{SpatialAreaOfInterest, SpatialAreaOfInterestOptions};
use catalyst_comms::pulse::ip_limiter::IpLimiterOptions;
use catalyst_comms_e2e::engine::{Outcome, RunOptions, ScenarioResult, Transport, run_scenarios};
use catalyst_comms_e2e::pulse::messages;
use catalyst_comms_e2e::scenarios::crowd::{self, CROWD, DEFAULT_PLAYERS, players_from};

fn options(transport: Transport) -> RunOptions {
    RunOptions {
        repeat: Some(1),
        seed: Some(7),
        transport,
        ..Default::default()
    }
}

async fn run_on_enet(server: PulseServer) -> ScenarioResult {
    let address = support::serve(server).await;
    let stack = Arc::new(catalyst_comms_e2e::stack::Stack::pulse_only(address));
    let mut report = run_scenarios(stack, &[&CROWD], &options(Transport::Enet)).await;
    report.results.remove(0)
}

fn note<'a>(result: &'a ScenarioResult, key: &str) -> &'a serde_json::Value {
    &result
        .notes
        .iter()
        .find(|note| note.key == key)
        .unwrap_or_else(|| panic!("{} has no note {key}: {:?}", result.name, result.notes))
        .value
}

fn assert_passed(result: &ScenarioResult) {
    match &result.outcome {
        Outcome::Passed => {}
        Outcome::Failed {
            expectation,
            seed,
            timeline,
            ..
        } => {
            let tail: Vec<String> = timeline
                .iter()
                .rev()
                .take(30)
                .rev()
                .map(|entry| format!("{entry:?}"))
                .collect();
            panic!(
                "{} failed (seed {seed}): {expectation}\n--- last entries\n{}",
                result.name,
                tail.join("\n")
            );
        }
        Outcome::Skipped { reason } => panic!("{} was skipped: {reason}", result.name),
    }
}

/// Every player of the crowd sees the other players, and the notes say so.
fn assert_everyone_saw_everyone(result: &ScenarioResult) {
    assert_passed(result);
    let players = DEFAULT_PLAYERS as u64;
    assert_eq!(note(result, "players").as_u64(), Some(players));
    assert_eq!(
        note(result, "players_seeing_everyone").as_u64(),
        Some(players)
    );
    let seen = note(result, "others_seen_by_each")
        .as_array()
        .expect("a count per player");
    assert_eq!(seen.len(), DEFAULT_PLAYERS);
    assert!(
        seen.iter().all(|count| count.as_u64() == Some(players - 1)),
        "{seen:?}"
    );
    assert!(note(result, "last_sighting_ms").as_u64().is_some());
}

#[test]
fn the_crowd_is_registered() {
    let registry = catalyst_comms_e2e::scenarios::registry();
    assert!(registry.iter().any(|known| known.name == "pulse-crowd"));
    assert!(crowd::all().iter().all(|scenario| !scenario.hostile));
    assert!(CROWD.tags.contains(&"pulse") && CROWD.tags.contains(&"crowd"));
}

#[test]
fn the_crowd_size_defaults_to_twenty_and_refuses_fewer_than_two() {
    assert_eq!(players_from(None).unwrap(), 20);
    assert_eq!(players_from(Some("2")).unwrap(), 2);
    assert_eq!(players_from(Some(" 12 ")).unwrap(), 12);
    for bad in ["0", "1", "-3", "many", ""] {
        assert!(players_from(Some(bad)).is_err(), "{bad:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn twenty_players_see_each_other_over_enet() {
    assert_everyone_saw_everyone(&run_on_enet(PulseServer::new()).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn twenty_players_see_each_other_over_webtransport() {
    let pulse = support::serve_both(PulseServer::new()).await;
    let mut report =
        run_scenarios(pulse.stack(), &[&CROWD], &options(Transport::WebTransport)).await;
    assert_everyone_saw_everyone(&report.results.remove(0));
}

/// A server that admits 10 players per source address refuses half the crowd, which all dials
/// from one address: the scenario fails at the entrance and names the refusal.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn the_crowd_fails_when_the_server_admits_fewer_players_per_address() {
    let mut server = PulseServer::new();
    server.ip_limiter_options = IpLimiterOptions {
        enabled: true,
        max_concurrency: 10,
        ..Default::default()
    };
    let result = run_on_enet(server).await;
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!(
            "the crowd should fail against this server, got {:?}",
            result.outcome
        );
    };
    assert!(
        expectation.contains("10 of 20 players could not enter"),
        "{expectation}"
    );
    assert!(
        expectation.contains(&messages::reason(
            DisconnectReason::IpConnectionLimitExceeded.code()
        )),
        "{expectation}"
    );
    assert_eq!(note(&result, "players_admitted").as_u64(), Some(10));
}

/// The crowd stands on a 5 by 4 grid, 2 m apart. With an area of interest of 3 m a player sees
/// only its near neighbours, so the scenario must fail and say how far the sightings got.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn the_crowd_fails_when_players_stand_outside_each_others_area_of_interest() {
    let mut server = PulseServer::new();
    server.aoi = SpatialAreaOfInterest::new(SpatialAreaOfInterestOptions {
        tier0_radius: 1.0,
        tier1_radius: 2.0,
        max_radius: 3.0,
    });
    let result = run_on_enet(server).await;
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!(
            "the crowd should fail against this server, got {:?}",
            result.outcome
        );
    };
    assert!(
        expectation.contains("of 20 players saw every other player"),
        "{expectation}"
    );
    assert_eq!(note(&result, "players_seeing_everyone").as_u64(), Some(0));
    let seen = note(&result, "others_seen_by_each")
        .as_array()
        .expect("a count per player");
    // a corner sees its two neighbours on the grid and the one diagonally next to it
    assert!(
        seen.iter().all(|count| count.as_u64().unwrap() < 19),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|count| count.as_u64().unwrap() >= 2),
        "{seen:?}"
    );
}
