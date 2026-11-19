//! The connection scenarios as the runner sees them: what each declares, and when each is skipped.

use catalyst_comms_e2e::scenarios::connections;
use catalyst_comms_e2e::stack::Stack;

const LOCAL: &str = "api = \"http://127.0.0.1:5210\"\npulse_enet = \"127.0.0.1:5217\"\n";

fn stack(extra: &str) -> Stack {
    Stack::parse(&format!("{LOCAL}{extra}")).unwrap()
}

#[test]
fn every_connection_scenario_is_tagged_named_and_needs_the_api() {
    let all = connections::all();
    let names: Vec<_> = all.iter().map(|scenario| scenario.name).collect();
    for expected in [
        "connect-full-stack",
        "second-device-takeover",
        "stale-socket-reconnect",
        "assignment-order",
        "mints-under-churn",
        "ban-eviction",
        "stationary-credentials",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} is not registered: {names:?}"
        );
    }
    for scenario in &all {
        assert!(scenario.tags.contains(&"connections"), "{}", scenario.name);
        assert!(
            scenario.tags.len() >= 2,
            "{} has no group tag",
            scenario.name
        );
        assert!(scenario.needs.api, "{}", scenario.name);
        assert!(!scenario.guards.is_empty(), "{}", scenario.name);
    }
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len());
}

#[test]
fn races_repeat_and_bans_are_hostile_and_slow() {
    let by_name = |name: &str| {
        *connections::all()
            .iter()
            .find(|scenario| scenario.name == name)
            .unwrap()
    };
    assert_eq!(by_name("second-device-takeover").default_repeat, 6);
    let ban = by_name("ban-eviction");
    assert!(ban.hostile && ban.needs.moderator && ban.tags.contains(&"slow"));
    assert!(!by_name("connect-full-stack").hostile);
}

#[test]
fn every_scenario_that_restarts_the_stack_is_hostile() {
    let restarting: Vec<_> = catalyst_comms_e2e::scenarios::registry()
        .into_iter()
        .filter(|scenario| scenario.needs.restart)
        .collect();
    assert!(
        restarting
            .iter()
            .any(|scenario| scenario.name == "stationary-credentials")
    );
    for scenario in restarting {
        assert!(scenario.hostile, "{} restarts the stack", scenario.name);
    }
}

#[test]
fn a_scenario_that_restarts_the_stack_is_skipped_without_a_command() {
    let restart = connections::all()
        .into_iter()
        .find(|scenario| scenario.name == "stationary-credentials")
        .unwrap();
    let missing = restart
        .needs
        .missing(&stack(""))
        .expect("skipped without a command");
    assert!(missing.contains("restart_command"), "{missing}");
    assert!(
        restart
            .needs
            .missing(&stack(
                "restart_command = [\"python\", \"stack.py\", \"restart\"]\n"
            ))
            .is_none()
    );
}

#[test]
fn the_moderation_scenario_is_skipped_without_a_moderator_token() {
    let ban = connections::all()
        .into_iter()
        .find(|scenario| scenario.name == "ban-eviction")
        .unwrap();
    assert!(
        ban.needs
            .missing(&stack(""))
            .unwrap()
            .contains("moderator_token_env")
    );
    assert!(
        ban.needs
            .missing(&stack("moderator_token_env = \"X\"\n"))
            .is_none()
    );
}
