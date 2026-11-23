//! `crowd-visit` against a real Pulse server in this process and a stand-in content server: the
//! crowd makes sure of its profiles before it enters, goes where another client already stands,
//! every player catches that client's join, the client catches every player's, each announcing
//! the profile, and the crowd leaves the way clients do. A wallet the crowd never meets fails the
//! run by name; a refused deployment fails it before anyone enters; a run that does not name
//! it leaves it out.

mod support;

use std::sync::Arc;
use std::time::Duration;

use catalyst_comms_e2e::engine::{
    CrowdSize, Ctx, Outcome, RunOptions, ScenarioResult, Transport, VisitPlan, run_scenarios,
};
use catalyst_comms_e2e::identity::{Identity, now_ms};
use catalyst_comms_e2e::profile::{self, PROFILE_VERSION};
use catalyst_comms_e2e::pulse::PulseBot;
use catalyst_comms_e2e::scenarios::support::{enter_at, leave};
use catalyst_comms_e2e::scenarios::visit::{self, CROWD_VISIT};
use catalyst_comms_e2e::stack::{Grid, Stack};
use support::content::{self, ContentStandIn};

const PLAYERS: usize = 6;
const REALM: &str = "visit-test-realm";
const PARCEL: (i32, i32) = (3, -4);

fn note<'a>(result: &'a ScenarioResult, key: &str) -> &'a serde_json::Value {
    &result
        .notes
        .iter()
        .find(|note| note.key == key)
        .unwrap_or_else(|| panic!("{} has no note {key}: {:?}", result.name, result.notes))
        .value
}

fn options(plan: Option<VisitPlan>) -> RunOptions {
    RunOptions {
        repeat: Some(1),
        seed: Some(11),
        transport: Transport::Enet,
        crowd: CrowdSize {
            players: Some(PLAYERS),
            room_players: None,
        },
        visit: plan,
        named: Some(CROWD_VISIT.name.to_owned()),
        ..Default::default()
    }
}

fn plan(label: &str, expect: Vec<String>) -> VisitPlan {
    VisitPlan {
        realm: REALM.to_owned(),
        parcel: PARCEL,
        seconds: 2,
        expect,
        label: label.to_owned(),
    }
}

/// A Pulse-only stack whose `api` is the stand-in's.
fn stack(pulse: std::net::SocketAddr, stand_in: &ContentStandIn) -> Arc<Stack> {
    let mut stack = Stack::pulse_only(pulse);
    stack.api = Some(stand_in.api.clone());
    Arc::new(stack)
}

/// A client already standing at the parcel's centre, with its context.
async fn client_there(pulse: std::net::SocketAddr) -> (Ctx, PulseBot, String) {
    let client_ctx = support::ctx(pulse, "client");
    let client_identity = client_ctx.identity(0);
    let wallet = client_identity.address().to_lowercase();
    let centre = Grid::default().parcel_centre(PARCEL.0, PARCEL.1);
    let client = enter_at(&client_ctx, "client", &client_identity, REALM, centre)
        .await
        .unwrap();
    (client_ctx, client, wallet)
}

fn passed(result: &ScenarioResult) {
    if let Outcome::Failed { expectation, .. } = &result.outcome {
        panic!("crowd-visit failed: {expectation}");
    }
    assert!(
        matches!(result.outcome, Outcome::Passed),
        "{:?}",
        result.outcome
    );
}

#[test]
fn a_visit_plan_comes_from_the_texts_of_its_variables() {
    assert_eq!(
        visit::plan_from(None, None, None, None, None).unwrap(),
        None
    );
    let made = visit::plan_from(Some("example-realm"), Some(" -12, 5 "), None, None, None)
        .unwrap()
        .unwrap();
    assert_eq!(made.realm, "example-realm");
    assert_eq!(made.parcel, (-12, 5));
    assert_eq!(made.seconds, visit::DEFAULT_SECONDS);
    assert!(made.expect.is_empty());
    assert_eq!(made.label, visit::DEFAULT_LABEL);

    let wallet = format!("0x{}", "Ab".repeat(20));
    let other = format!("0x{}", "cd".repeat(20));
    let made = visit::plan_from(
        Some("r"),
        Some("0,0"),
        Some("30"),
        Some(&format!("{wallet}, {other}")),
        Some(" crowd-2 "),
    )
    .unwrap()
    .unwrap();
    assert_eq!(made.seconds, 30);
    assert_eq!(made.expect, vec![wallet.to_lowercase(), other]);
    assert_eq!(made.label, "crowd-2");

    // each part that is there must be right, and a realm and a parcel come together
    for (realm, parcel, seconds, expect, label) in [
        (Some("r"), None, None, None, None),
        (None, Some("0,0"), None, None, None),
        (None, None, Some("30"), None, None),
        (None, None, None, None, Some("crowd")),
        (Some(""), Some("0,0"), None, None, None),
        (Some("r"), Some("0;0"), None, None, None),
        (Some("r"), Some("0,0"), Some("0"), None, None),
        (Some("r"), Some("0,0"), Some("100000"), None, None),
        (Some("r"), Some("0,0"), None, Some("0x12"), None),
        (Some("r"), Some("0,0"), None, None, Some("")),
        (Some("r"), Some("0,0"), None, None, Some("Crowd")),
        (Some("r"), Some("0,0"), None, None, Some("a crowd")),
        (Some("r"), Some("0,0"), None, None, Some(&"a".repeat(33))),
    ] {
        assert!(
            visit::plan_from(realm, parcel, seconds, expect, label).is_err(),
            "{realm:?} {parcel:?} {seconds:?} {expect:?} {label:?}"
        );
    }
}

#[test]
fn a_visitors_wallet_comes_from_the_label_and_its_index() {
    let first = Identity::visitor("crowd-a", 0).address();
    assert_eq!(first, Identity::visitor("crowd-a", 0).address());
    assert_ne!(first, Identity::visitor("crowd-b", 0).address());
    assert_ne!(first, Identity::visitor("crowd-a", 1).address());
}

#[test]
fn spots_stay_inside_the_parcel_while_the_players_walk() {
    let grid = Grid::default();
    let centre = grid.parcel_centre(PARCEL.0, PARCEL.1);
    for index in 0..visit::MOST_PLAYERS_IN_A_PARCEL {
        let spot = visit::spot(centre, index, visit::MOST_PLAYERS_IN_A_PARCEL);
        for seconds in [0.0, 1.0, 2.5, 4.0, 5.9] {
            let (at, _, _) = visit::walking(spot, index, seconds);
            let (x, z) = ((at[0] / 16.0).floor() as i32, (at[2] / 16.0).floor() as i32);
            assert_eq!((x, z), PARCEL, "player {index} at {seconds} s: {at:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_plan_the_visit_is_skipped() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(None).await;
    let mut report = run_scenarios(stack(pulse, &stand_in), &[&CROWD_VISIT], &options(None)).await;
    let result = report.results.remove(0);
    let Outcome::Skipped { reason } = &result.outcome else {
        panic!("not skipped: {:?}", result.outcome);
    };
    assert!(reason.contains("CE2E_VISIT_REALM"), "{reason}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_that_does_not_name_the_visit_leaves_it_out() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(None).await;
    // as `--all` or `--tag` select it, or a run that names another scenario
    for named in [None, Some("pulse-smoke".to_owned())] {
        let mut unnamed = options(Some(plan("unnamed", Vec::new())));
        unnamed.named = named.clone();
        let mut report = run_scenarios(stack(pulse, &stand_in), &[&CROWD_VISIT], &unnamed).await;
        let result = report.results.remove(0);
        let Outcome::Skipped { reason } = &result.outcome else {
            panic!("not left out with {named:?}: {:?}", result.outcome);
        };
        assert!(reason.contains("comms-e2e run crowd-visit"), "{reason}");
        assert!(reason.contains("--all"), "{reason}");
        assert!(reason.contains("--tag"), "{reason}");
        // the run's output says so the way it says any skip
        report.results.push(result);
        let rendered = report.render(0);
        assert!(
            rendered.starts_with("SKIPPED  crowd-visit  (runs only when"),
            "{rendered}"
        );
    }
    assert_eq!(stand_in.posts(), 0);
    assert_eq!(stand_in.recorded.lock().reads, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_deployment_still_counts_what_was_deployed_and_already_served() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(None).await;
    let client = profile::client().unwrap();
    let endpoints = profile::endpoints(&client, &stand_in.api).await.unwrap();
    // players 0 and 1 have their profiles from an earlier visit
    for index in 0..2 {
        let identity = Identity::visitor("counted", index);
        profile::deploy(
            &client,
            &endpoints.entities,
            &profile::profile_deployment(&identity, index as usize, now_ms()),
        )
        .await
        .unwrap();
    }
    // of players 2 and 3, deployed together in the first wave, one is refused
    stand_in.answer_next(400, None);
    let mut report = run_scenarios(
        stack(pulse, &stand_in),
        &[&CROWD_VISIT],
        &options(Some(plan("counted", Vec::new()))),
    )
    .await;
    let result = report.results.remove(0);
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!("passed with a deployment refused: {:?}", result.outcome);
    };
    assert!(expectation.contains("HTTP 400"), "{expectation}");
    assert_eq!(note(&result, "profiles_already_served").as_u64(), Some(2));
    assert_eq!(note(&result, "profiles_deployed").as_u64(), Some(1));
    assert_eq!(stand_in.posts(), 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_an_api_the_visit_is_skipped() {
    let pulse = support::in_process_pulse().await;
    let mut report = run_scenarios(
        support::stack_at(pulse),
        &[&CROWD_VISIT],
        &options(Some(plan("no-api", Vec::new()))),
    )
    .await;
    let result = report.results.remove(0);
    let Outcome::Skipped { reason } = &result.outcome else {
        panic!("not skipped: {:?}", result.outcome);
    };
    assert!(reason.contains("api"), "{reason}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_crowd_deploys_its_profiles_meets_a_client_already_there_and_the_client_meets_it() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(None).await;
    let (client_ctx, client, client_wallet) = client_there(pulse).await;

    let mut report = run_scenarios(
        stack(pulse, &stand_in),
        &[&CROWD_VISIT],
        &options(Some(plan("meets", vec![client_wallet.clone()]))),
    )
    .await;
    let result = report.results.remove(0);
    passed(&result);

    // every player's profile was deployed, once, before anyone entered
    assert_eq!(
        note(&result, "profiles_deployed").as_u64(),
        Some(PLAYERS as u64)
    );
    assert_eq!(note(&result, "profiles_already_served").as_u64(), Some(0));
    assert_eq!(note(&result, "profiles_replaced").as_u64(), Some(0));
    assert_eq!(
        note(&result, "profiles_served_before_entering").as_u64(),
        Some(PLAYERS as u64)
    );
    assert_eq!(stand_in.posts(), PLAYERS);
    let wallets: Vec<String> = note(&result, "crowd_wallets")
        .as_array()
        .unwrap()
        .iter()
        .map(|wallet| wallet.as_str().unwrap().to_lowercase())
        .collect();
    assert_eq!(wallets.len(), PLAYERS);
    for (index, wallet) in wallets.iter().enumerate() {
        assert_eq!(*wallet, Identity::visitor("meets", index as u32).address());
        assert_eq!(
            stand_in.served_name(wallet),
            Some(format!("ce2e-visit-{index:02}"))
        );
    }
    // every player of the crowd caught the client's join, in the plan's realm
    let caught = note(&result, "expected_caught_by");
    assert_eq!(
        caught[&client_wallet].as_u64(),
        Some(PLAYERS as u64),
        "{caught}"
    );
    let visitors = note(&result, "visitor_joins_by_player").as_array().unwrap();
    assert_eq!(visitors.len(), PLAYERS);
    for joins in visitors {
        let joins = joins.as_array().unwrap();
        assert_eq!(joins.len(), 1, "{joins:?}");
        assert_eq!(joins[0]["wallet"], client_wallet.as_str());
        assert_eq!(joins[0]["realm"], REALM);
    }
    // and each other's, each announcing the profile
    let crowd = note(&result, "crowd_join_events_caught_by_player")
        .as_array()
        .unwrap();
    assert!(
        crowd
            .iter()
            .all(|count| count.as_u64() == Some(PLAYERS as u64 - 1)),
        "{crowd:?}"
    );
    assert_eq!(
        note(&result, "crowd_joins_without_the_profile_version").as_u64(),
        Some(0)
    );
    // the crowd kept moving while it stayed
    assert!(note(&result, "walk_inputs_sent").as_u64().unwrap() > PLAYERS as u64 * 5);
    assert!(!note(&result, "held_samples").as_array().unwrap().is_empty());

    // the client, from its own connection: one join and one leave for every player of the
    // crowd, each join announcing the profile the crowd deployed
    let seen = client.view();
    for wallet in &wallets {
        assert_eq!(seen.joins_of(wallet), 1, "{wallet}");
        assert_eq!(seen.leaves_of(wallet), 1, "{wallet}");
    }
    let announced: Vec<i64> = client_ctx
        .timeline()
        .snapshot()
        .into_iter()
        .filter(|entry| entry.actor == "client" && entry.kind == "joined")
        .map(|entry| entry.detail["profile_version"].as_i64().unwrap())
        .collect();
    assert_eq!(announced, vec![i64::from(PROFILE_VERSION); PLAYERS]);
    assert!(seen.subjects.is_empty(), "{:?}", seen.wallets());
    assert!(seen.deltas > 0, "the client saw no movement");
    leave(&client_ctx, client).await.unwrap();

    // a second visit under the same label deploys nothing
    let mut report = run_scenarios(
        stack(pulse, &stand_in),
        &[&CROWD_VISIT],
        &options(Some(plan("meets", Vec::new()))),
    )
    .await;
    let again = report.results.remove(0);
    passed(&again);
    assert_eq!(note(&again, "profiles_deployed").as_u64(), Some(0));
    assert_eq!(
        note(&again, "profiles_already_served").as_u64(),
        Some(PLAYERS as u64)
    );
    assert_eq!(stand_in.posts(), PLAYERS);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_deployment_fails_the_visit_before_anyone_enters() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(None).await;
    for _ in 0..PLAYERS {
        stand_in.answer_next(400, None);
    }
    let (client_ctx, client, _) = client_there(pulse).await;
    let mut report = run_scenarios(
        stack(pulse, &stand_in),
        &[&CROWD_VISIT],
        &options(Some(plan("refused", Vec::new()))),
    )
    .await;
    let result = report.results.remove(0);
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!("passed with every deployment refused: {:?}", result.outcome);
    };
    assert!(expectation.contains("HTTP 400"), "{expectation}");
    assert!(expectation.contains("player-0"), "{expectation}");
    // nobody entered: the client heard of no one
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(client.view().joins, 0, "{:?}", client.view().wallets());
    leave(&client_ctx, client).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_content_server_off_loopback_is_not_written_to_without_allow_host() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(Some("https://content.example/content")).await;
    let mut report = run_scenarios(
        stack(pulse, &stand_in),
        &[&CROWD_VISIT],
        &options(Some(plan("guarded", Vec::new()))),
    )
    .await;
    let result = report.results.remove(0);
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!("not refused: {:?}", result.outcome);
    };
    assert!(expectation.contains("--allow-host"), "{expectation}");
    assert!(!expectation.contains("content.example"), "{expectation}");
    assert_eq!(stand_in.posts(), 0);
    assert_eq!(stand_in.recorded.lock().reads, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wallet_the_crowd_never_meets_fails_the_visit_by_name() {
    let pulse = support::in_process_pulse().await;
    let stand_in = content::start(None).await;
    let absent = format!("0x{}", "e5".repeat(20));
    let mut report = run_scenarios(
        stack(pulse, &stand_in),
        &[&CROWD_VISIT],
        &options(Some(plan("never-meets", vec![absent.clone()]))),
    )
    .await;
    let result = report.results.remove(0);
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!("passed without meeting {absent}: {:?}", result.outcome);
    };
    assert!(expectation.contains(&absent), "{expectation}");
    assert_eq!(
        note(&result, "expected_caught_by")[&absent].as_u64(),
        Some(0)
    );
    // the crowd left all the same
    let probe_ctx = support::ctx(pulse, "late");
    let late_identity = probe_ctx.identity(0);
    let centre = Grid::default().parcel_centre(PARCEL.0, PARCEL.1);
    let late = enter_at(&probe_ctx, "late", &late_identity, REALM, centre)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        late.view().subjects.is_empty(),
        "{:?}",
        late.view().wallets()
    );
    leave(&probe_ctx, late).await.unwrap();
}
