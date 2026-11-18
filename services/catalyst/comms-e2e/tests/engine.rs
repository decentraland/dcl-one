//! The runner itself: repeats, seeds, skips, refusals, panics and deadlines.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use catalyst_comms_e2e::engine::{
    Ctx, Needs, Outcome, RunOptions, Scenario, ScenarioFuture, expect_or, expect_within,
    holds_for, run_scenarios,
};
use catalyst_comms_e2e::stack::Stack;

fn stack() -> Arc<Stack> {
    Arc::new(Stack::pulse_only("127.0.0.1:9".parse().unwrap()))
}

fn options() -> RunOptions {
    RunOptions {
        seed: Some(100),
        scenario_deadline: Duration::from_secs(5),
        ..RunOptions::default()
    }
}

const fn scenario(name: &'static str, run: fn(Ctx) -> ScenarioFuture) -> Scenario {
    Scenario {
        name,
        tags: &["test"],
        guards: &["test#1"],
        summary: "a test scenario",
        needs: Needs {
            api: false,
            webtransport: false,
            pulse_metrics: false,
            livekit: false,
            moderator: false,
            explorer: false,
            scene_listener: false,
            restart: false,
        },
        hostile: false,
        default_repeat: 1,
        run,
    }
}

fn passes(_ctx: Ctx) -> ScenarioFuture {
    Box::pin(async { Ok(()) })
}

fn fails_on_seed_103(ctx: Ctx) -> ScenarioFuture {
    Box::pin(async move {
        anyhow::ensure!(ctx.seed() != 103, "seed 103 breaks it");
        Ok(())
    })
}

#[tokio::test]
async fn a_failed_repeat_reports_its_seed() {
    let failing = scenario("fails-on-103", fails_on_seed_103);
    let report = run_scenarios(
        stack(),
        &[&failing],
        &RunOptions {
            repeat: Some(10),
            ..options()
        },
    )
    .await;
    let Outcome::Failed {
        seed,
        repeat,
        expectation,
        ..
    } = &report.results[0].outcome
    else {
        panic!("expected a failure, got {:?}", report.results[0].outcome);
    };
    assert_eq!((*seed, *repeat), (103, 3));
    assert!(expectation.contains("seed 103 breaks it"));
    // it stopped at the first failure
    assert_eq!(report.results[0].repeats_run, 4);
    assert!(report.failed(false));
    assert!(report.render(10).contains("--seed 103"));

    // the reported seed replays it on the first repeat
    let replay = run_scenarios(
        stack(),
        &[&failing],
        &RunOptions {
            seed: Some(103),
            repeat: Some(1),
            ..options()
        },
    )
    .await;
    assert!(matches!(replay.results[0].outcome, Outcome::Failed { repeat: 0, .. }));
}

#[tokio::test]
async fn skipped_is_not_passed() {
    let mut needs_explorer = scenario("needs-explorer", passes);
    needs_explorer.needs.explorer = true;
    let passing = scenario("passes", passes);
    let report = run_scenarios(stack(), &[&needs_explorer, &passing], &options()).await;
    let Outcome::Skipped { reason } = &report.results[0].outcome else {
        panic!("expected a skip, got {:?}", report.results[0].outcome);
    };
    assert!(reason.contains("explorer"), "{reason}");
    assert_eq!(report.results[0].repeats_run, 0);
    assert_eq!(report.count(), (1, 0, 1));
    assert!(!report.failed(false));
    assert!(report.failed(true), "under --strict a skip fails the run");
    assert!(report.render(10).contains("SKIPPED  needs-explorer"));
}

#[tokio::test]
async fn nothing_run_is_a_failed_run() {
    let report = run_scenarios(stack(), &[], &options()).await;
    assert!(report.failed(false));
}

#[tokio::test]
async fn a_hostile_scenario_is_refused_against_a_public_host_before_it_runs() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    fn counts(_ctx: Ctx) -> ScenarioFuture {
        Box::pin(async {
            RAN.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
    let mut hostile = scenario("hostile", counts);
    hostile.hostile = true;
    let public = Arc::new(Stack::pulse_only("8.8.8.8:9000".parse().unwrap()));

    let report = run_scenarios(public.clone(), &[&hostile], &options()).await;
    let Outcome::Failed { expectation, .. } = &report.results[0].outcome else {
        panic!("expected a refusal");
    };
    assert!(expectation.contains("refused before the first packet"), "{expectation}");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);

    let allowed = RunOptions {
        allowed_hosts: vec!["8.8.8.8".to_owned()],
        ..options()
    };
    let report = run_scenarios(public, &[&hostile], &allowed).await;
    assert!(matches!(report.results[0].outcome, Outcome::Passed));
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_panic_and_a_hang_both_end_as_failures() {
    fn panics(_ctx: Ctx) -> ScenarioFuture {
        Box::pin(async { panic!("boom") })
    }
    fn hangs(_ctx: Ctx) -> ScenarioFuture {
        Box::pin(async {
            futures::future::pending::<()>().await;
            Ok(())
        })
    }
    let panicking = scenario("panics", panics);
    let hanging = scenario("hangs", hangs);
    let report = run_scenarios(
        stack(),
        &[&panicking, &hanging],
        &RunOptions {
            scenario_deadline: Duration::from_millis(200),
            ..options()
        },
    )
    .await;
    let reasons: Vec<String> = report
        .results
        .iter()
        .map(|result| match &result.outcome {
            Outcome::Failed { expectation, .. } => expectation.clone(),
            other => panic!("expected failures, got {other:?}"),
        })
        .collect();
    assert!(reasons[0].contains("panicked: boom"), "{}", reasons[0]);
    assert!(reasons[1].contains("did not finish"), "{}", reasons[1]);
}

#[tokio::test]
async fn runs_do_not_share_realms_or_wallets() {
    let work = std::env::temp_dir();
    let first = Ctx::new(stack(), "run-a", "same-scenario", 0, 1, work.clone());
    let second = Ctx::new(stack(), "run-b", "same-scenario", 0, 1, work.clone());
    let repeat = Ctx::new(stack(), "run-a", "same-scenario", 1, 1, work);
    assert_ne!(first.realm("x"), second.realm("x"));
    assert_ne!(first.realm("x"), repeat.realm("x"));
    assert_ne!(first.identity(0).address(), second.identity(0).address());
    assert_ne!(first.identity(0).address(), repeat.identity(0).address());
    assert_eq!(first.identity(0).address(), first.identity(0).address());
    assert!(first.realm("x").len() < 255);
}

#[tokio::test]
async fn expect_within_reports_what_and_when() {
    let ctx = Ctx::new(stack(), "run", "expect", 0, 1, std::env::temp_dir());
    let started = Instant::now();
    let error = expect_within(&ctx, "the moon is full", Duration::from_millis(60), || None::<()>)
        .await
        .unwrap_err()
        .to_string();
    assert!(started.elapsed() >= Duration::from_millis(60));
    assert!(error.contains("the moon is full") && error.contains("not met after"), "{error}");

    // the last reason travels with the failure
    let error = expect_or(&ctx, "three peers", Duration::from_millis(30), || {
        Err::<(), _>("saw 2".to_owned())
    })
    .await
    .unwrap_err()
    .to_string();
    assert!(error.ends_with("saw 2"), "{error}");

    // a fatal reason ends the wait at once
    let started = Instant::now();
    let error = expect_or(&ctx, "still connected", Duration::from_secs(30), || {
        Err::<(), _>("fatal: disconnected with code 11".to_owned())
    })
    .await
    .unwrap_err()
    .to_string();
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(error.ends_with("disconnected with code 11"), "{error}");

    // a met expectation returns its value
    let mut calls = 0;
    let value = expect_within(&ctx, "third look", Duration::from_secs(1), || {
        calls += 1;
        (calls == 3).then_some(calls)
    })
    .await
    .unwrap();
    assert_eq!(value, 3);
}

#[tokio::test]
async fn holds_for_fails_at_the_first_break() {
    let ctx = Ctx::new(stack(), "run", "holds", 0, 1, std::env::temp_dir());
    holds_for(&ctx, "nothing breaks", Duration::from_millis(40), || Ok(()))
        .await
        .unwrap();
    let started = Instant::now();
    let error = holds_for(&ctx, "the session lives", Duration::from_secs(30), || {
        if started.elapsed() > Duration::from_millis(30) {
            Err("it died".to_owned())
        } else {
            Ok(())
        }
    })
    .await
    .unwrap_err()
    .to_string();
    assert!(error.contains("the session lives") && error.contains("it died"), "{error}");
}

#[tokio::test]
async fn notes_survive_a_pass() {
    fn notes(ctx: Ctx) -> ScenarioFuture {
        Box::pin(async move {
            ctx.note("resyncs", 3);
            Ok(())
        })
    }
    let noting = scenario("notes", notes);
    let report = run_scenarios(stack(), &[&noting], &options()).await;
    assert_eq!(report.results[0].notes.len(), 1);
    assert_eq!(report.results[0].notes[0].value, 3);
    assert!(report.render(10).contains("resyncs = 3"));
}
