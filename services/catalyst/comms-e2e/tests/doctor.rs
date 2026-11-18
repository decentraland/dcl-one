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
    // a port nothing serves
    let dead = {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.local_addr().unwrap()
    };
    let checks = examine(support::stack_at(dead)).await;
    let enet = check(&checks, "Pulse over ENet");
    assert!(!enet.ok && !enet.optional, "{enet:?}");
    assert!(!usable(&checks));
    assert!(render(&checks).contains("FAILED   Pulse over ENet"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn what_the_file_names_and_is_not_there_is_a_failure_and_no_secret_is_shown() {
    let pulse = support::in_process_pulse().await;
    // an API nothing serves, a token variable that is not set, an explorer binary that is not there
    let text = format!(
        "api = \"http://127.0.0.1:9\"\npulse_enet = \"{pulse}\"\nmoderator_token_env = \"CE2E_DOCTOR_TEST_UNSET\"\nscene_listener_key_env = \"CE2E_DOCTOR_TEST_SET\"\n[explorer]\nbinary = \"no-such-explorer\"\napi_front = \"127.0.0.1:0\"\npulse_front = \"127.0.0.1:0\"\n"
    );
    // SAFETY: this test alone reads and writes these two variables
    unsafe {
        std::env::remove_var("CE2E_DOCTOR_TEST_UNSET");
        std::env::set_var(
            "CE2E_DOCTOR_TEST_SET",
            "a-secret-value-that-must-not-be-printed",
        );
    }
    let checks = examine(Arc::new(Stack::parse(&text).unwrap())).await;
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
