//! The crowd scenario against a real Pulse server in this process: it passes against a correct
//! server on both transports, and fails, naming how many players saw everyone, against a server
//! whose area of interest is smaller than the crowd.

mod support;

use std::sync::Arc;

use catalyst_comms::pulse::PulseServer;
use catalyst_comms::pulse::hardening::DisconnectReason;
use catalyst_comms::pulse::interest::{SpatialAreaOfInterest, SpatialAreaOfInterestOptions};
use catalyst_comms::pulse::ip_limiter::IpLimiterOptions;
use catalyst_comms_e2e::engine::{
    Entry, Outcome, RunOptions, ScenarioResult, Transport, run_scenarios,
};
use catalyst_comms_e2e::pulse::messages;
use catalyst_comms_e2e::scenarios::crowd::{
    self, CROWD, DEFAULT_PLAYERS, join_evidence, join_verdict, players_from,
};

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
    // the join events each client caught: one for every other player, none from outside
    let caught = note(result, "join_events_caught_by_player")
        .as_array()
        .expect("a count per player");
    assert_eq!(caught.len(), DEFAULT_PLAYERS);
    assert!(
        caught
            .iter()
            .all(|count| count.as_u64() == Some(players - 1)),
        "{caught:?}"
    );
    let by_player = note(result, "join_events_by_player")
        .as_array()
        .expect("events per player");
    for (index, events) in by_player.iter().enumerate() {
        let mut from: Vec<u64> = events
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event[0].as_u64().unwrap())
            .collect();
        from.sort_unstable();
        let expected: Vec<u64> = (0..players)
            .filter(|other| *other != index as u64)
            .collect();
        assert_eq!(from, expected, "player {index}");
    }
    assert_eq!(
        note(result, "join_events_outside_the_crowd").as_u64(),
        Some(0)
    );
}

fn entry(actor: &str, at_ms: u64, wallet: &str, realm: &str) -> Entry {
    Entry {
        at_ms,
        actor: actor.into(),
        kind: "joined".into(),
        detail: serde_json::json!({ "subject": 1, "wallet": wallet, "realm": realm }),
    }
}

#[test]
fn join_events_are_read_per_player_by_index_and_judged() {
    let names: Vec<String> = (0..3).map(|index| format!("player-{index}")).collect();
    let wallets: Vec<String> = ["0xAa", "0xbb", "0xcc"].map(str::to_owned).to_vec();
    let mut entries = vec![
        entry("player-0", 5, "0xbb", "r"),
        entry("player-0", 7, "0xcc", "r"),
        entry("player-1", 6, "0xaa", "r"),
        entry("player-1", 8, "0xcc", "r"),
        entry("player-2", 9, "0xaa", "r"),
        entry("player-2", 9, "0xbb", "r"),
        // another actor's entries and other kinds are not join events of the crowd
        entry("observer", 3, "0xaa", "r"),
        Entry {
            at_ms: 1,
            actor: "player-0".into(),
            kind: "left".into(),
            detail: serde_json::json!({ "wallet": "0xbb" }),
        },
    ];
    let evidence = join_evidence(&entries, &names, &wallets, "r");
    assert_eq!(evidence[0].from, vec![(1, 5), (2, 7)]);
    assert_eq!(
        (evidence[0].first_ms(), evidence[0].last_ms()),
        (Some(5), Some(7))
    );
    assert_eq!(evidence[2].distinct(), vec![0, 1]);
    join_verdict(&evidence).unwrap();

    // a stranger, a crowd wallet in another realm, a self join and a second join each fail it
    entries.push(entry("player-2", 10, "0xfeed", "r"));
    entries.push(entry("player-1", 11, "0xaa", "elsewhere"));
    entries.push(entry("player-0", 12, "0xaa", "r"));
    entries.push(entry("player-0", 13, "0xbb", "r"));
    let failure = join_verdict(&join_evidence(&entries, &names, &wallets, "r")).unwrap_err();
    for expected in [
        "player-2 caught joins of players outside the crowd",
        "0xfeed",
        "elsewhere",
        "player-0 was told 1 times that it joined",
        "player-0 caught 3 join events for 2 players",
    ] {
        assert!(failure.contains(expected), "{expected}: {failure}");
    }

    // a missing join names the player it misses
    let short = join_evidence(&entries[1..6], &names, &wallets, "r");
    let failure = join_verdict(&short).unwrap_err();
    assert!(
        failure.contains("player-0 caught joins of 1 of 2 players, missing [1]"),
        "{failure}"
    );
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
    // the join events fail it first: no client was told of every other player
    assert!(
        expectation.starts_with("join events: player-0 caught joins of"),
        "{expectation}"
    );
    let caught = note(&result, "join_events_caught_by_player")
        .as_array()
        .expect("a count per player");
    assert!(
        caught.iter().all(|count| count.as_u64().unwrap() < 19),
        "{caught:?}"
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
