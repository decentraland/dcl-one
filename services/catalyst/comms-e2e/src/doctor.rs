//! `comms-e2e doctor`: is everything a stack file names really there? It dials each address
//! once, checks the files and the environment variables, and says what is missing, so a run does
//! not fail halfway for a reason that has nothing to do with the stack's behaviour.

use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use url::Url;

use crate::engine::Ctx;
use crate::pulse::{ConnectOptions, PulseBot};
use crate::stack::Stack;

/// One thing the doctor looked at.
#[derive(Debug, Clone)]
pub struct Check {
    pub what: String,
    pub ok: bool,
    pub detail: String,
    /// Absent from the stack file: the scenarios that need it are skipped, nothing is wrong.
    pub optional: bool,
}

impl Check {
    /// Something the stack file names: it is there or the stack is not what the file says.
    fn new(what: &str, result: Result<String, String>) -> Check {
        let (ok, detail) = match result {
            Ok(detail) => (true, detail),
            Err(detail) => (false, detail),
        };
        Check {
            what: what.to_owned(),
            ok,
            detail,
            optional: false,
        }
    }

    fn absent(what: &str, key: &str) -> Check {
        Check {
            what: what.to_owned(),
            ok: false,
            detail: format!(
                "not in the stack file (`{key}`): the scenarios that need it are skipped"
            ),
            optional: true,
        }
    }
}

const DIAL: Duration = Duration::from_secs(3);

/// Every route the doctor asks answers 200 on a stack that is what the file says. Anything else,
/// a 404 from some other server included, means the address is not that part of the stack.
async fn http(client: &reqwest::Client, url: Url) -> Result<String, String> {
    match client.get(url.clone()).send().await {
        Ok(response) if response.status().is_success() => {
            Ok(format!("{url} answered {}", response.status()))
        }
        Ok(response) => Err(format!("{url} answered {}", response.status())),
        Err(error) => Err(format!("{url} does not answer: {error}")),
    }
}

fn variable(stack: &Stack, name: &str) -> Result<String, String> {
    match stack.secret(name) {
        Ok(value) if !value.trim().is_empty() => Ok(format!("{name} is set")),
        _ => Err(format!("the environment variable {name} is not set")),
    }
}

fn file(path: &std::path::Path) -> Result<String, String> {
    if path.is_file() {
        Ok(format!("{} exists", path.display()))
    } else {
        Err(format!("{} is not a file", path.display()))
    }
}

fn free_tcp(address: SocketAddr) -> Result<String, String> {
    TcpListener::bind(address)
        .map(|_| format!("TCP {address} is free"))
        .map_err(|error| format!("TCP {address} cannot be bound: {error}"))
}

fn free_udp(address: SocketAddr) -> Result<String, String> {
    UdpSocket::bind(address)
        .map(|_| format!("UDP {address} is free"))
        .map_err(|error| format!("UDP {address} cannot be bound: {error}"))
}

/// Looks at everything `stack` names. Never prints or returns a secret: a variable is reported
/// as set or not set.
pub async fn examine(stack: Arc<Stack>) -> Vec<Check> {
    let ctx = Ctx::new(
        stack.clone(),
        "doctor",
        "doctor",
        0,
        0,
        std::env::temp_dir().join("comms-e2e").join("doctor"),
    );
    let client = reqwest::Client::builder()
        .timeout(DIAL)
        .build()
        .expect("a default HTTP client");
    let mut checks = Vec::new();

    let options = ConnectOptions {
        deadline: Some(DIAL),
        ..ConnectOptions::default()
    };
    let enet = PulseBot::connect(&ctx, "doctor", stack.pulse_enet, options.clone())
        .await
        .map(|bot| {
            bot.close();
            format!("{} accepts an ENet connection", stack.pulse_enet)
        })
        .map_err(|error| format!("{error:#}"));
    checks.push(Check::new("Pulse over ENet", enet));

    match &stack.pulse_webtransport {
        Some(target) => {
            let dialled = PulseBot::connect_webtransport(&ctx, "doctor", target, options)
                .await
                .map(|bot| {
                    bot.close();
                    format!("{} accepts a WebTransport session", target.url)
                })
                .map_err(|error| format!("{error:#}"));
            checks.push(Check::new("Pulse over WebTransport", dialled));
        }
        None => checks.push(Check::absent(
            "Pulse over WebTransport",
            "pulse_webtransport",
        )),
    }

    for (what, key, url) in [
        ("Pulse metrics", "pulse_metrics", &stack.pulse_metrics),
        ("Serve metrics", "serve_metrics", &stack.serve_metrics),
    ] {
        match url {
            Some(url) => checks.push(Check::new(what, http(&client, url.clone()).await)),
            None => checks.push(Check::absent(what, key)),
        }
    }

    match &stack.api {
        Some(api) => {
            for (what, path) in [
                ("API", "status"),
                ("Gatekeeper", "comms-gatekeeper/status"),
                ("Statistics", "peers"),
            ] {
                let result = match api.join(path) {
                    Ok(url) => http(&client, url).await,
                    Err(error) => Err(format!("{api} and {path} do not make a URL: {error}")),
                };
                checks.push(Check::new(what, result));
            }
        }
        None => checks.push(Check::absent("API", "api")),
    }

    match &stack.livekit {
        Some(target) => {
            checks.push(Check::new(
                "LiveKit",
                http(&client, target.url.clone()).await,
            ));
            checks.push(Check::new("LiveKit key", variable(&stack, &target.key_env)));
            checks.push(Check::new(
                "LiveKit secret",
                variable(&stack, &target.secret_env),
            ));
        }
        None => checks.push(Check::absent("LiveKit", "livekit")),
    }
    match stack.livekit_client() {
        Some(path) => checks.push(Check::new("LiveKit client", file(path))),
        None => checks.push(Check::absent(
            "LiveKit client",
            "livekit.client` or `livekit_client",
        )),
    }

    match &stack.moderator_token_env {
        Some(name) => checks.push(Check::new("Moderator token", variable(&stack, name))),
        None => checks.push(Check::absent("Moderator token", "moderator_token_env")),
    }
    match &stack.scene_listener_key_env {
        Some(name) => checks.push(Check::new("Scene listener key", variable(&stack, name))),
        None => checks.push(Check::absent(
            "Scene listener key",
            "scene_listener_key_env",
        )),
    }
    match &stack.restart_command {
        Some(command) if !command.is_empty() => checks.push(Check::new(
            "Restart command",
            Ok(format!("`{}` (not run)", command[0])),
        )),
        _ => checks.push(Check::absent("Restart command", "restart_command")),
    }

    match &stack.explorer {
        Some(target) => {
            checks.push(Check::new("Explorer binary", file(&target.binary)));
            checks.push(Check::new("Explorer API front", free_tcp(target.api_front)));
            checks.push(Check::new(
                "Explorer Pulse front",
                free_udp(target.pulse_front),
            ));
            if let Some(relay) = target.livekit_signal {
                checks.push(Check::new(
                    "Explorer LiveKit signalling front",
                    free_tcp(relay.front),
                ));
            }
            if let Some(relay) = target.livekit_media {
                checks.push(Check::new(
                    "Explorer LiveKit media front",
                    free_udp(relay.front),
                ));
            }
        }
        None => checks.push(Check::absent("Explorer", "explorer")),
    }
    checks
}

/// The checks as lines for a terminal.
pub fn render(checks: &[Check]) -> String {
    let mut out = String::new();
    for check in checks {
        let mark = match (check.ok, check.optional) {
            (true, _) => "ok     ",
            (false, true) => "absent ",
            (false, false) => "FAILED ",
        };
        out.push_str(&format!("{mark}  {:<34} {}\n", check.what, check.detail));
    }
    out
}

/// Whether everything the stack file names is there. What the file leaves out only skips
/// scenarios.
pub fn usable(checks: &[Check]) -> bool {
    checks.iter().all(|check| check.ok || check.optional)
}
