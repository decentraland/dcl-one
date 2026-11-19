//! The doctor: what it says about a stack that is there, one that is not, and one that is only
//! partly described.

mod support;

use std::sync::Arc;

use catalyst_comms_e2e::doctor::{examine, render, usable};
use catalyst_comms_e2e::stack::Stack;

fn check<'a>(
    checks: &'a [catalyst_comms_e2e::doctor::Check],
    what: &str,
) -> &'a catalyst_comms_e2e::doctor::Check {
    checks
        .iter()
        .find(|check| check.what == what)
        .unwrap_or_else(|| panic!("no check named {what} in:\n{}", render(checks)))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pulse_only_stack_is_usable_and_the_rest_is_absent() {
    let pulse = support::in_process_pulse().await;
    let checks = examine(support::stack_at(pulse)).await;
    assert!(check(&checks, "Pulse over ENet").ok, "{}", render(&checks));
    // what the file leaves out is absent, not failed
    for what in [
        "Pulse over WebTransport",
        "API",
        "LiveKit",
        "Explorer",
        "Moderator token",
    ] {
        let absent = check(&checks, what);
        assert!(!absent.ok && absent.optional, "{what}: {absent:?}");
    }
    assert!(usable(&checks));
    let text = render(&checks);
    assert!(text.contains("ok       Pulse over ENet"), "{text}");
    assert!(text.contains("absent   API"), "{text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pulse_that_does_not_answer_fails_the_stack() {
    // a port something holds and never answers on: held to the end, so no other test is given it
    let silent = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let checks = examine(support::stack_at(silent.local_addr().unwrap())).await;
    let enet = check(&checks, "Pulse over ENet");
    assert!(!enet.ok && !enet.optional, "{enet:?}");
    assert!(!usable(&checks));
    assert!(render(&checks).contains("FAILED   Pulse over ENet"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_that_does_not_know_the_routes_fails_the_stack() {
    // something answers at the API address and it is not the API: every route is a 404
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, axum::Router::new()).await.unwrap();
    });
    let pulse = support::in_process_pulse().await;
    let text = format!("api = \"http://{address}\"\npulse_enet = \"{pulse}\"\n");
    let checks = examine(Arc::new(Stack::parse(&text).unwrap())).await;
    for what in ["API", "Gatekeeper", "Statistics"] {
        let failed = check(&checks, what);
        assert!(!failed.ok && !failed.optional, "{what}: {failed:?}");
        assert!(failed.detail.contains("404"), "{what}: {failed:?}");
    }
    assert!(!usable(&checks));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn what_the_file_names_and_is_not_there_is_a_failure_and_no_secret_is_shown() {
    let pulse = support::in_process_pulse().await;
    // an API nothing serves, a token variable that is not set, an explorer binary that is not there
    let text = format!(
        "api = \"http://127.0.0.1:9\"\npulse_enet = \"{pulse}\"\nmoderator_token_env = \"CE2E_DOCTOR_TEST_UNSET\"\nscene_listener_key_env = \"CE2E_DOCTOR_TEST_SET\"\n[explorer]\nbinary = \"no-such-explorer\"\napi_front = \"127.0.0.1:0\"\npulse_front = \"127.0.0.1:0\"\n"
    );
    // nothing sets the first variable; the second is given, so no test writes the environment
    let mut stack = Stack::parse(&text).unwrap();
    stack.give_secret(
        "CE2E_DOCTOR_TEST_SET",
        "a-secret-value-that-must-not-be-printed",
    );
    let checks = examine(Arc::new(stack)).await;
    for what in ["API", "Moderator token", "Explorer binary"] {
        let failed = check(&checks, what);
        assert!(!failed.ok && !failed.optional, "{what}: {failed:?}");
    }
    assert!(check(&checks, "Scene listener key").ok);
    assert!(
        check(&checks, "Explorer Pulse front").ok,
        "a free port is fine"
    );
    assert!(!usable(&checks));
    let shown = render(&checks);
    assert!(shown.contains("CE2E_DOCTOR_TEST_SET is set"), "{shown}");
    assert!(
        !shown.contains("a-secret-value"),
        "the doctor printed a secret:\n{shown}"
    );
}
