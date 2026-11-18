//! The explorer actor without an explorer: what happens when the child is not one.

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use catalyst_comms_e2e::engine::Ctx;
use catalyst_comms_e2e::explorer::{Explorer, ExplorerOptions, Fronts};
use catalyst_comms_e2e::stack::Stack;

/// A stack whose "explorer" is this test binary: given the explorer's arguments it prints an
/// error about them and exits at once.
fn stack_with_a_stand_in(pulse: std::net::SocketAddr) -> Arc<Stack> {
    let binary = std::env::current_exe().unwrap();
    let text = format!(
        "api = \"http://127.0.0.1:9\"\npulse_enet = \"{pulse}\"\n[explorer]\nbinary = '{}'\napi_front = \"127.0.0.1:0\"\npulse_front = \"127.0.0.1:0\"\n",
        binary.display()
    );
    Arc::new(Stack::parse(&text).unwrap())
}

fn ctx(stack: Arc<Stack>, name: &str) -> Ctx {
    Ctx::new(
        stack,
        &format!("t{}", std::process::id()),
        name,
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_child_that_exits_fails_with_the_end_of_its_log() {
    let pulse = support::in_process_pulse().await;
    let ctx = ctx(stack_with_a_stand_in(pulse), "stand-in");
    let started = Instant::now();
    let error = Explorer::start(
        &ctx,
        "explorer",
        ExplorerOptions {
            ready_deadline: Duration::from_secs(60),
            ..ExplorerOptions::default()
        },
    )
    .await
    .err()
    .expect("a program that is not an explorer cannot open a console");
    let text = format!("{error:#}");
    // it failed when the child exited, not at the deadline
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "waited {:?}",
        started.elapsed()
    );
    assert!(
        text.contains("exited") && text.contains("before its console opened"),
        "{text}"
    );
    // the child's own words are in the error: the test harness complains about the first flag
    assert!(text.contains("realm"), "the log tail is missing: {text}");
    // and the failure is on the timeline for the report
    assert_eq!(ctx.timeline().count("explorer", "explorer starting"), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_binary_is_an_error_that_names_it() {
    let pulse = support::in_process_pulse().await;
    let stack = Stack::parse(&format!(
        "api = \"http://127.0.0.1:9\"\npulse_enet = \"{pulse}\"\n[explorer]\nbinary = \"no-such-explorer-binary\"\napi_front = \"127.0.0.1:0\"\npulse_front = \"127.0.0.1:0\"\n"
    ))
    .unwrap();
    let ctx = ctx(Arc::new(stack), "missing");
    let error = Explorer::start(&ctx, "explorer", ExplorerOptions::default())
        .await
        .err()
        .expect("no binary, no explorer");
    assert!(
        format!("{error:#}").contains("no-such-explorer-binary"),
        "{error:#}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_fronts_relay_what_the_stack_file_names() {
    let pulse = support::in_process_pulse().await;
    // an API address nothing serves is fine: the front only has to listen
    let ctx = ctx(stack_with_a_stand_in(pulse), "fronts");
    let fronts = Fronts::start(&ctx).await.unwrap();
    assert_eq!(fronts.pulse.upstream(), pulse);
    assert_ne!(fronts.pulse.address().port(), 0);
    assert_ne!(fronts.api.address().port(), 0);
    assert!(fronts.livekit_signal.is_none() && fronts.livekit_media.is_none());
    // faults on all fronts at once do not panic with fronts that are absent
    fronts.half_open_all();
    fronts.restore_all();
}

#[tokio::test]
async fn without_an_explorer_in_the_stack_file_the_actor_says_so() {
    let ctx = ctx(support::stack_at("127.0.0.1:9".parse().unwrap()), "none");
    let error = Fronts::start(&ctx)
        .await
        .err()
        .expect("no explorer section");
    assert!(error.to_string().contains("explorer"), "{error}");
}
