//! `crowd-visit` against a real Pulse server in this process: a crowd goes where another client
//! already stands, every player catches that client's join, the client catches every player's,
//! and the crowd leaves the way clients do. A wallet the crowd never meets fails the run by name.

mod support;

use std::sync::Arc;
use std::time::Duration;

use catalyst_comms_e2e::engine::{
    CrowdSize, Outcome, RunOptions, ScenarioResult, Transport, VisitPlan, run_scenarios,
};
use catalyst_comms_e2e::scenarios::support::{enter_at, leave};
use catalyst_comms_e2e::scenarios::visit::{self, CROWD_VISIT};
use catalyst_comms_e2e::stack::Grid;

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
        ..Default::default()
    }
}

fn plan(expect: Vec<String>) -> VisitPlan {
    VisitPlan {
        realm: REALM.to_owned(),
        parcel: PARCEL,
        seconds: 2,
        expect,
    }
}

#[test]
fn a_visit_plan_comes_from_the_texts_of_its_variables() {
    assert_eq!(visit::plan_from(None, None, None, None).unwrap(), None);
    let made = visit::plan_from(Some("example-realm"), Some(" -12, 5 "), None, None)
        .unwrap()
        .unwrap();
    assert_eq!(made.realm, "example-realm");
    assert_eq!(made.parcel, (-12, 5));
    assert_eq!(made.seconds, visit::DEFAULT_SECONDS);
    assert!(made.expect.is_empty());

    let wallet = format!("0x{}", "Ab".repeat(20));
    let other = format!("0x{}", "cd".repeat(20));
    let made = visit::plan_from(
        Some("r"),
        Some("0,0"),
        Some("30"),
        Some(&format!("{wallet}, {other}")),
    )
    .unwrap()
    .unwrap();
    assert_eq!(made.seconds, 30);
    assert_eq!(made.expect, vec![wallet.to_lowercase(), other]);

    // each part that is there must be right, and a realm and a parcel come together
    for (realm, parcel, seconds, expect) in [
        (Some("r"), None, None, None),
        (None, Some("0,0"), None, None),
        (None, None, Some("30"), None),
        (Some(""), Some("0,0"), None, None),
        (Some("r"), Some("0;0"), None, None),
        (Some("r"), Some("0,0"), Some("0"), None),
        (Some("r"), Some("0,0"), Some("100000"), None),
        (Some("r"), Some("0,0"), None, Some("0x12")),
    ] {
        assert!(
            visit::plan_from(realm, parcel, seconds, expect).is_err(),
            "{realm:?} {parcel:?} {seconds:?} {expect:?}"
        );
    }
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
    let stack = Arc::new(catalyst_comms_e2e::stack::Stack::pulse_only(pulse));
    let mut report = run_scenarios(stack, &[&CROWD_VISIT], &options(None)).await;
    let result = report.results.remove(0);
    let Outcome::Skipped { reason } = &result.outcome else {
        panic!("not skipped: {:?}", result.outcome);
    };
    assert!(reason.contains("CE2E_VISIT_REALM"), "{reason}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_crowd_meets_a_client_already_there_and_the_client_meets_the_crowd() {
    let pulse = support::in_process_pulse().await;
    // the client that was there first, standing at the parcel's centre
    let client_ctx = support::ctx(pulse, "client");
    let client_identity = client_ctx.identity(0);
    let client_wallet = client_identity.address().to_lowercase();
    let centre = Grid::default().parcel_centre(PARCEL.0, PARCEL.1);
    let client = enter_at(&client_ctx, "client", &client_identity, REALM, centre)
        .await
        .unwrap();

    let stack = Arc::new(catalyst_comms_e2e::stack::Stack::pulse_only(pulse));
    let mut report = run_scenarios(
        stack,
        &[&CROWD_VISIT],
        &options(Some(plan(vec![client_wallet.clone()]))),
    )
    .await;
    let result = report.results.remove(0);
    if let Outcome::Failed { expectation, .. } = &result.outcome {
        panic!("crowd-visit failed: {expectation}");
    }
    assert!(matches!(result.outcome, Outcome::Passed));

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
    // and each other's
    let crowd = note(&result, "crowd_join_events_caught_by_player")
        .as_array()
        .unwrap();
    assert!(
        crowd
            .iter()
            .all(|count| count.as_u64() == Some(PLAYERS as u64 - 1)),
        "{crowd:?}"
    );
    // the crowd kept moving while it stayed
    assert!(note(&result, "walk_inputs_sent").as_u64().unwrap() > PLAYERS as u64 * 5);
    assert!(!note(&result, "held_samples").as_array().unwrap().is_empty());

    // the client, from its own connection: one join and one leave for every player of the crowd
    let wallets: Vec<String> = note(&result, "crowd_wallets")
        .as_array()
        .unwrap()
        .iter()
        .map(|wallet| wallet.as_str().unwrap().to_lowercase())
        .collect();
    assert_eq!(wallets.len(), PLAYERS);
    let seen = client.view();
    for wallet in &wallets {
        assert_eq!(seen.joins_of(wallet), 1, "{wallet}");
        assert_eq!(seen.leaves_of(wallet), 1, "{wallet}");
    }
    assert!(seen.subjects.is_empty(), "{:?}", seen.wallets());
    assert!(seen.deltas > 0, "the client saw no movement");
    leave(&client_ctx, client).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wallet_the_crowd_never_meets_fails_the_visit_by_name() {
    let pulse = support::in_process_pulse().await;
    let absent = format!("0x{}", "e5".repeat(20));
    let stack = Arc::new(catalyst_comms_e2e::stack::Stack::pulse_only(pulse));
    let mut report = run_scenarios(
        stack,
        &[&CROWD_VISIT],
        &options(Some(plan(vec![absent.clone()]))),
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
