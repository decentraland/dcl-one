//! The explorer actor without an explorer: what happens when the child is not one.

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use catalyst_comms_e2e::engine::{Ctx, expect_async, expect_within};
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

/// A stand-in explorer that is a script, so it needs no binary built for the test and runs the
/// same on Windows (a `.cmd` file) and elsewhere (a shell script): it writes a line to its
/// standard error, then beats once a second into `beat` until `cue` exists, then writes another
/// line and exits with status 3. It serves no console: the test decides whether one listens on
/// the port the explorer is given.
fn stand_in_script(
    dir: &std::path::Path,
    cue: &std::path::Path,
    beat: &std::path::Path,
) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    #[cfg(windows)]
    {
        let path = dir.join("explorer.cmd");
        let text = format!(
            "@echo off\r\necho stand-in explorer started 1>&2\r\n:loop\r\nif exist \"{cue}\" goto done\r\necho beat>>\"{beat}\"\r\nping -n 2 127.0.0.1 >nul\r\ngoto loop\r\n:done\r\necho stand-in explorer crashed after the cue 1>&2\r\nexit /b 3\r\n",
            cue = cue.display(),
            beat = beat.display()
        );
        std::fs::write(&path, text).unwrap();
        path
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("explorer.sh");
        let text = format!(
            "#!/bin/sh\necho stand-in explorer started >&2\nwhile [ ! -e '{cue}' ]; do echo beat >> '{beat}'; sleep 1; done\necho stand-in explorer crashed after the cue >&2\nexit 3\n",
            cue = cue.display(),
            beat = beat.display()
        );
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

fn stack_with_binary(binary: &std::path::Path) -> Arc<Stack> {
    let text = format!(
        "api = \"http://127.0.0.1:9\"\npulse_enet = \"127.0.0.1:9\"\n[explorer]\nbinary = '{}'\napi_front = \"127.0.0.1:0\"\npulse_front = \"127.0.0.1:0\"\n",
        binary.display()
    );
    Arc::new(Stack::parse(&text).unwrap())
}

/// A console served by the test: JSON-RPC over HTTP POST, one request per connection. Every
/// command is answered at once with `{"alive":true}`.
async fn serve_console(listener: tokio::net::TcpListener) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            let body = loop {
                let Ok(read) = stream.read(&mut chunk).await else {
                    return;
                };
                if read == 0 {
                    return;
                }
                buffer.extend_from_slice(&chunk[..read]);
                let Some(head_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
                else {
                    continue;
                };
                let head = String::from_utf8_lossy(&buffer[..head_end]).to_lowercase();
                let length: usize = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse().ok())
                    .unwrap_or(0);
                if buffer.len() >= head_end + 4 + length {
                    break buffer[head_end + 4..head_end + 4 + length].to_vec();
                }
            };
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let result = match request["method"].as_str() {
                Some("dcl/console") => serde_json::json!({ "id": 1 }),
                _ => serde_json::json!({ "done": true, "ok": true, "output": "{\"alive\":true}" }),
            };
            let reply =
                serde_json::json!({ "jsonrpc": "2.0", "id": request["id"], "result": result })
                    .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("comms-e2e-actor-{}-{name}", std::process::id()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dead_explorer_is_detected_after_startup() {
    let dir = scratch("dead");
    let (cue, beat) = (dir.join("cue"), dir.join("beat"));
    let binary = stand_in_script(&dir, &cue, &beat);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let console = tokio::spawn(serve_console(listener));

    let ctx = ctx(stack_with_binary(&binary), "dead-explorer");
    let explorer = Explorer::start(
        &ctx,
        "explorer",
        ExplorerOptions {
            automation_port: Some(port),
            ready_deadline: Duration::from_secs(30),
            ..ExplorerOptions::default()
        },
    )
    .await
    .expect("the console answers, so the explorer is ready");
    // while it lives, a probe is an answer
    assert_eq!(explorer.probe("state").await.unwrap()["alive"], true);

    // the explorer crashes, and with it the console
    std::fs::write(&cue, "now").unwrap();
    expect_within(&ctx, "the child exits", Duration::from_secs(20), || {
        (!explorer.running()).then_some(())
    })
    .await
    .unwrap();
    console.abort();
    let _ = console.await;

    // every way of reaching the console says so, at once, with the status and the log
    let (first, second, third) = tokio::join!(
        explorer.probe("state"),
        explorer.probe("state"),
        explorer.poll("state")
    );
    let reasons = [first, second, third];
    for reason in reasons {
        let reason = reason.expect_err("the console is gone");
        assert!(reason.starts_with("fatal:"), "{reason}");
        assert!(
            reason.contains("exited") && reason.contains('3'),
            "{reason}"
        );
        assert!(reason.contains("crashed after the cue"), "{reason}");
    }
    let command = explorer
        .command("state", Duration::from_secs(5))
        .await
        .expect_err("the console is gone");
    assert!(format!("{command:#}").starts_with("fatal:"), "{command:#}");

    // so a scenario's wait ends with the log, not at its deadline
    let started = Instant::now();
    let error = expect_async(
        &ctx,
        "the explorer answers",
        Duration::from_secs(60),
        Duration::from_millis(50),
        || explorer.probe("state"),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(started.elapsed() < Duration::from_secs(5), "{error}");
    assert!(error.contains("crashed after the cue"), "{error}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_child_that_never_opens_its_console_fails_at_the_deadline_and_is_gone() {
    let dir = scratch("silent");
    let (cue, beat) = (dir.join("cue"), dir.join("beat"));
    let binary = stand_in_script(&dir, &cue, &beat);
    // a port nothing listens on: bound and held to the end, so no other test is given it
    let held = tokio::net::TcpSocket::new_v4().unwrap();
    held.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let free = held.local_addr().unwrap().port();
    let ctx = ctx(stack_with_binary(&binary), "silent-explorer");
    let started = Instant::now();
    let error = Explorer::start(
        &ctx,
        "explorer",
        ExplorerOptions {
            automation_port: Some(free),
            ready_deadline: Duration::from_secs(3),
            ..ExplorerOptions::default()
        },
    )
    .await
    .err()
    .expect("no console, no explorer");
    let text = format!("{error:#}");
    assert!(started.elapsed() < Duration::from_secs(15), "{text}");
    assert!(text.contains("did not open its console"), "{text}");
    assert!(text.contains("stand-in explorer started"), "{text}");

    // nothing of it runs on: its beat stops
    let beats = || std::fs::read_to_string(&beat).map_or(0, |text| text.lines().count());
    let after_failure = beats();
    assert!(after_failure > 0, "the stand-in never ran");
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(
        beats(),
        after_failure,
        "the child outlived the failed start"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_explorer_scenario_is_registered_with_its_tags_guards_and_summary() {
    let all = catalyst_comms_e2e::scenarios::explorer::all();
    let names: std::collections::HashSet<&str> = all.iter().map(|scenario| scenario.name).collect();
    assert_eq!(names.len(), all.len(), "names are unique");
    assert_eq!(all.len(), 17);
    let uplink = [
        "explorer-uplink-cadence",
        "explorer-walk-800ms-round-trip",
        "explorer-walk-1200ms-round-trip",
        "explorer-walk-lossy-link",
        "explorer-queued-stall-1200ms-round-trip",
        "explorer-queued-stall-2000ms-round-trip",
    ];
    let registry = catalyst_comms_e2e::scenarios::registry();
    for scenario in &all {
        assert!(scenario.tags.contains(&"explorer"), "{}", scenario.name);
        assert!(scenario.needs.explorer, "{}", scenario.name);
        assert!(
            !scenario.guards.is_empty(),
            "{} names no guard",
            scenario.name
        );
        assert!(
            !scenario.summary.is_empty(),
            "{} has no summary",
            scenario.name
        );
        assert!(
            !scenario.hostile,
            "{} only disturbs the link",
            scenario.name
        );
        assert_eq!(
            scenario.tags.contains(&"uplink"),
            uplink.contains(&scenario.name),
            "{}",
            scenario.name
        );
        assert!(registry.iter().any(|known| known.name == scenario.name));
    }
    for name in uplink {
        assert!(names.contains(name), "{name} is missing");
    }
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
