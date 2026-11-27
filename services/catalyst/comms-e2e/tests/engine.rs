//! The runner itself: repeats, seeds, skips, refusals, panics and deadlines.
#![allow(clippy::unwrap_used, reason = "test support")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use catalyst_comms_e2e::engine::{
    Ctx, Needs, Outcome, RunOptions, Scenario, ScenarioFuture, expect_or, expect_within, holds_for,
    run_scenarios,
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
            pulse_restart: false,
            visit: false,
            by_name: false,
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
    assert!(matches!(
        replay.results[0].outcome,
        Outcome::Failed { repeat: 0, .. }
    ));
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
    assert!(
        expectation.contains("refused before the first packet"),
        "{expectation}"
    );
    assert_eq!(RAN.load(Ordering::SeqCst), 0);

    // --allow-host alone admits ordinary scenarios, not a hostile one
    let allowed = RunOptions {
        allowed_hosts: vec!["8.8.8.8".to_owned()],
        ..options()
    };
    let report = run_scenarios(public.clone(), &[&hostile], &allowed).await;
    let Outcome::Failed { expectation, .. } = &report.results[0].outcome else {
        panic!("expected a refusal");
    };
    assert!(
        expectation.contains("--allow-hostile-host"),
        "{expectation}"
    );
    assert_eq!(RAN.load(Ordering::SeqCst), 0);

    let both = RunOptions {
        allowed_hosts: vec!["8.8.8.8".to_owned()],
        allowed_hostile_hosts: vec!["8.8.8.8".to_owned()],
        ..options()
    };
    let report = run_scenarios(public, &[&hostile], &both).await;
    assert!(matches!(report.results[0].outcome, Outcome::Passed));
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_ordinary_scenario_is_refused_against_a_host_that_is_not_named() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    fn counts(_ctx: Ctx) -> ScenarioFuture {
        Box::pin(async {
            RAN.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
    let ordinary = scenario("ordinary", counts);
    let public = Arc::new(Stack::pulse_only("203.0.113.5:9000".parse().unwrap()));

    let report = run_scenarios(public.clone(), &[&ordinary], &options()).await;
    let Outcome::Failed { expectation, .. } = &report.results[0].outcome else {
        panic!("expected a refusal");
    };
    assert!(
        expectation.contains("refused before the first packet"),
        "{expectation}"
    );
    assert!(expectation.contains("--allow-host"), "{expectation}");
    // the report names no host that is not loopback unless asked to
    assert!(!expectation.contains("203.0.113.5"), "{expectation}");
    assert!(!report.render(10).contains("203.0.113.5"));
    assert_eq!(RAN.load(Ordering::SeqCst), 0);

    let allowed = RunOptions {
        allowed_hosts: vec!["203.0.113.5".to_owned()],
        ..options()
    };
    let report = run_scenarios(public, &[&ordinary], &allowed).await;
    assert!(matches!(report.results[0].outcome, Outcome::Passed));
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn the_report_counts_hosts_that_are_not_loopback_unless_asked_to_name_them() {
    let passing = scenario("passing", passes);
    let mut stack = Stack::pulse_only("127.0.0.1:9000".parse().unwrap());
    stack.api = Some("http://203.0.113.5:5210".parse().unwrap());
    stack.serve_metrics = Some("http://198.51.100.7:5210/metrics".parse().unwrap());
    let stack = Arc::new(stack);
    let named = |report_hosts: bool| RunOptions {
        allowed_hosts: vec!["203.0.113.5".to_owned(), "198.51.100.7".to_owned()],
        report_hosts,
        ..options()
    };

    let report = run_scenarios(stack.clone(), &[&passing], &named(false)).await;
    assert!(matches!(report.results[0].outcome, Outcome::Passed));
    assert_eq!(report.hosts, vec!["127.0.0.1".to_owned()]);
    assert_eq!(report.other_hosts, 2);
    let json = serde_json::to_string(&report).unwrap();
    let rendered = report.render(10);
    for text in [&json, &rendered] {
        assert!(!text.contains("203.0.113.5"), "{text}");
        assert!(!text.contains("198.51.100.7"), "{text}");
    }
    assert!(rendered.contains("2 other hosts"), "{rendered}");

    let report = run_scenarios(stack, &[&passing], &named(true)).await;
    assert_eq!(report.other_hosts, 0);
    let rendered = report.render(10);
    for host in ["127.0.0.1", "203.0.113.5", "198.51.100.7"] {
        assert!(
            report.hosts.contains(&host.to_owned()),
            "{:?}",
            report.hosts
        );
        assert!(rendered.contains(host), "{rendered}");
    }
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
    let error = expect_within(&ctx, "the moon is full", Duration::from_millis(60), || {
        None::<()>
    })
    .await
    .unwrap_err()
    .to_string();
    assert!(started.elapsed() >= Duration::from_millis(60));
    assert!(
        error.contains("the moon is full") && error.contains("not met after"),
        "{error}"
    );

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
    assert!(
        error.contains("the session lives") && error.contains("it died"),
        "{error}"
    );
}

const SECRET_JWT: &str = "eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJ4In0.c2lnbmF0dXJl";

/// The text of a failure that names an address, a hash and three kinds of secret.
fn leaky_text(through: &str) -> String {
    format!(
        "{through}: connect to wss://host.example/rtc?access_token=SECRETCONNECTION&room=r1 \
         failed, Authorization: Bearer SECRETBEARER rejected for 0xabc123 \
         (hash deadbeef), token {SECRET_JWT} expired"
    )
}

fn assert_clean(text: &str, through: &str) {
    for secret in [
        "SECRETCONNECTION",
        "SECRETBEARER",
        SECRET_JWT,
        "eyJhbGciOiJFUzI1NiJ9",
    ] {
        assert!(!text.contains(secret), "{through} leaked {secret}: {text}");
    }
    // the rest of the message is intact
    for kept in [
        "wss://host.example/rtc?access_token=<redacted>&room=r1",
        "0xabc123",
        "hash deadbeef",
        "expired",
    ] {
        assert!(text.contains(kept), "{through} lost {kept}: {text}");
    }
}

#[tokio::test]
async fn secrets_in_an_error_do_not_reach_the_report() {
    fn returns_the_error(_ctx: Ctx) -> ScenarioFuture {
        Box::pin(async { Err(anyhow::anyhow!(leaky_text("the scenario error"))) })
    }
    fn panics_with_it(_ctx: Ctx) -> ScenarioFuture {
        Box::pin(async { panic!("{}", leaky_text("the panic")) })
    }
    fn fails_an_expectation(ctx: Ctx) -> ScenarioFuture {
        Box::pin(async move {
            expect_or(&ctx, "it connects", Duration::from_millis(20), || {
                Err::<(), _>(leaky_text("the reason"))
            })
            .await
        })
    }
    fn breaks_a_hold(ctx: Ctx) -> ScenarioFuture {
        Box::pin(async move {
            holds_for(&ctx, "it stays", Duration::from_millis(200), || {
                Err(leaky_text("the break"))
            })
            .await
        })
    }
    let scenarios = [
        scenario("error", returns_the_error),
        scenario("panic", panics_with_it),
        scenario("expectation", fails_an_expectation),
        scenario("hold", breaks_a_hold),
    ];
    let selection: Vec<&Scenario> = scenarios.iter().collect();
    let report = run_scenarios(stack(), &selection, &options()).await;

    let rendered = report.render(50);
    let file =
        std::env::temp_dir().join(format!("comms-e2e-redaction-{}.json", std::process::id()));
    report.write_json(&file).unwrap();
    let json = std::fs::read_to_string(&file).unwrap();
    let _ = std::fs::remove_file(&file);

    for result in &report.results {
        let Outcome::Failed { expectation, .. } = &result.outcome else {
            panic!("{} should have failed", result.name);
        };
        assert_clean(expectation, &result.name);
    }
    for secret in ["SECRETCONNECTION", "SECRETBEARER", SECRET_JWT] {
        assert!(!rendered.contains(secret), "render leaked {secret}");
        assert!(!json.contains(secret), "the JSON report leaked {secret}");
    }
    assert!(rendered.contains("0xabc123") && json.contains("0xabc123"));
}

#[tokio::test]
async fn a_secret_in_a_note_does_not_reach_the_report() {
    fn notes_a_connection_string(ctx: Ctx) -> ScenarioFuture {
        Box::pin(async move {
            ctx.note("connectivity", leaky_text("the note"));
            ctx.note(
                "nested",
                serde_json::json!({ "adapter": [leaky_text("the nested note")] }),
            );
            Ok(())
        })
    }
    let noting = scenario("leaky-notes", notes_a_connection_string);
    let report = run_scenarios(stack(), &[&noting], &options()).await;

    let rendered = report.render(50);
    let file = std::env::temp_dir().join(format!("comms-e2e-notes-{}.json", std::process::id()));
    report.write_json(&file).unwrap();
    let json = std::fs::read_to_string(&file).unwrap();
    let _ = std::fs::remove_file(&file);

    assert_eq!(report.results[0].notes.len(), 2);
    assert_clean(
        report.results[0].notes[0].value.as_str().unwrap(),
        "the note",
    );
    for secret in ["SECRETCONNECTION", "SECRETBEARER", SECRET_JWT] {
        assert!(!rendered.contains(secret), "render leaked {secret}");
        assert!(!json.contains(secret), "the JSON report leaked {secret}");
    }
    assert!(rendered.contains("0xabc123") && json.contains("0xabc123"));
}

#[test]
fn the_order_of_two_actors_comes_from_the_seed_alone() {
    let first = |repeat, seed| {
        Ctx::new(stack(), "run", "order", repeat, seed, std::env::temp_dir()).first_of_two()
    };
    // a failure at repeat 3 of a run seeded 100 is replayed as repeat 0 of a run seeded 103
    assert_eq!(first(3, 103), first(0, 103));
    assert_eq!(first(1, 101), first(0, 101));
    // one repeat after another still covers both orders
    assert_ne!(first(0, 100), first(1, 101));
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

#[tokio::test]
async fn each_scenario_and_repeat_dials_a_loopback_stack_from_its_own_address() {
    let work = std::env::temp_dir();
    let loopback = stack();
    let first = Ctx::new(loopback.clone(), "run", "one", 0, 1, work.clone());
    let again = Ctx::new(loopback.clone(), "run", "one", 0, 2, work.clone());
    let repeat = Ctx::new(loopback.clone(), "run", "one", 1, 1, work.clone());
    let other = Ctx::new(loopback.clone(), "run", "two", 0, 1, work.clone());
    if cfg!(target_os = "macos") {
        // one loopback address there: nothing to spread over
        assert_eq!(first.source_address(), None);
        return;
    }
    let address = first
        .source_address()
        .expect("a loopback stack gets an address");
    let std::net::IpAddr::V4(v4) = address else {
        panic!("a loopback address of the same family as the stack: {address}");
    };
    assert!(v4.is_loopback(), "{v4}");
    assert_ne!(
        v4.octets()[1],
        0,
        "never 127.0.x.x, where everything else on the machine is"
    );
    // every bot of one scenario and repeat shares the address, whatever the seed
    assert_eq!(first.source_address(), again.source_address());
    assert_ne!(first.source_address(), repeat.source_address());
    assert_ne!(first.source_address(), other.source_address());

    // a run may ask for the ordinary address
    first.use_one_source_address();
    assert_eq!(first.source_address(), None);

    // a stack that is not on loopback is dialled from wherever the machine routes it
    let remote = Arc::new(Stack::pulse_only("10.1.2.3:9000".parse().unwrap()));
    let far = Ctx::new(remote, "run", "one", 0, 1, work);
    assert_eq!(far.source_address(), None);
}
