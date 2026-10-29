//! The two pages that talk to the server, run under node: the page's own sender
//! (`deploy/web/engine/perf_stream.js`, unchanged) and the reset page the server serves. Skipped,
//! with the reason printed, where node is not installed.

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use benchmark::limits::Limits;
use benchmark::runner::stream_problem;
use benchmark::server::EndReport;
use support::{scratch, TestServer};

fn node_available() -> bool {
    let found = std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !found {
        println!(
            "skipped: node is not installed, and the page's own code can only be run under it"
        );
    }
    found
}

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn page_sender() -> PathBuf {
    here()
        .join("../../deploy/web/engine/perf_stream.js")
        .canonicalize()
        .expect("the page's sender")
}

async fn node(arguments: &[&str], dir: &Path) -> Output {
    tokio::process::Command::new("node")
        .args(arguments)
        .current_dir(dir)
        .output()
        .await
        .expect("node starts")
}

fn check_succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "node failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The sender's script and a copy of the sender as a module, in a folder of their own: the page
/// is an ES module and a `.js` file is only one under a package that says so.
fn sender_folder(name: &str) -> PathBuf {
    let dir = scratch(name);
    std::fs::copy(page_sender(), dir.join("perf_stream.mjs")).unwrap();
    std::fs::copy(here().join("tests/node/sender.mjs"), dir.join("sender.mjs")).unwrap();
    dir
}

async fn play_sender(name: &str, scenario: &str, limits: Limits) -> (TestServer, PathBuf) {
    let dir = sender_folder(name);
    let root = dir.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let server = TestServer::start_with(dir.clone(), root, limits).await;

    let output = node(
        &[
            "sender.mjs",
            &format!("http://127.0.0.1:{}", server.port),
            &server.shared.access.stream_path(),
            scenario,
            dir.to_str().unwrap(),
        ],
        &dir,
    )
    .await;
    check_succeeded(&output);
    (server, dir)
}

fn report(server: &TestServer) -> EndReport {
    server.shared.end.get().expect("the sender ended the run")
}

#[tokio::test]
async fn the_pages_sender_streams_a_run_the_server_stores_byte_for_byte() {
    if !node_available() {
        return;
    }
    let (server, dir) = play_sender("sender-ok", "ok", Limits::DEFAULT).await;

    let end = report(&server);
    let csv = std::fs::read(server.csv_path()).unwrap();
    let log = std::fs::read(server.log_path()).unwrap();
    assert_eq!(csv, std::fs::read(dir.join("expected.csv")).unwrap());
    assert_eq!(log, std::fs::read(dir.join("expected.log")).unwrap());
    assert!(csv.len() > 30_000 && String::from_utf8(log.clone()).unwrap().contains("日本語"));

    assert_eq!(end.exit_code, 0);
    assert!(!end.failed);
    assert_eq!(
        (end.csv_bytes, end.log_bytes),
        (Some(csv.len() as u64), Some(log.len() as u64))
    );
    let (csv_summary, log_summary) = (
        server.shared.csv.summary().await,
        server.shared.log.summary().await,
    );
    assert_eq!(stream_problem(Some(&end), &csv_summary, &log_summary), None);
    server.stop().await;
}

#[tokio::test]
async fn after_a_panic_the_sender_sends_what_the_engine_held_then_the_block_and_ends_with_101() {
    if !node_available() {
        return;
    }
    let (server, dir) = play_sender("sender-panic", "panic", Limits::DEFAULT).await;

    let end = report(&server);
    assert_eq!(end.exit_code, 101);
    assert!(!end.failed);
    let log = std::fs::read_to_string(server.log_path()).unwrap();
    assert_eq!(
        log,
        std::fs::read_to_string(dir.join("expected.log")).unwrap()
    );
    assert!(log.contains("PanicException: index out of bounds\n    second line\nDebugLogReportHandler:LogExceptionInternal\n"));
    assert_eq!(
        std::fs::read(server.csv_path()).unwrap(),
        std::fs::read(dir.join("expected.csv")).unwrap()
    );
    assert_eq!(
        end.csv_bytes,
        Some(std::fs::metadata(server.csv_path()).unwrap().len())
    );
    server.stop().await;
}

#[tokio::test]
async fn a_stream_the_sender_gives_up_is_reported_so_its_files_are_not_taken_for_whole() {
    if !node_available() {
        return;
    }
    // A file holds 5000 bytes: the log fits, the CSV does not, and the server refuses (413) what
    // passes the cap until the sender gives the stream up.
    let limits = Limits {
        file_bytes: 5000,
        ..Limits::DEFAULT
    };
    let (server, dir) = play_sender("sender-giveup", "giveup", limits).await;

    let end = report(&server);
    assert!(end.failed, "the page gave the CSV up and says so");
    assert_eq!(end.exit_code, 0);

    let stored = std::fs::read(server.csv_path()).unwrap();
    assert!(stored.len() <= 5000 && !stored.is_empty());
    let expected = std::fs::read(dir.join("expected.csv")).unwrap();
    assert!(
        expected.starts_with(&stored),
        "what is stored is a prefix of what the page wrote"
    );
    assert!(expected.len() > stored.len());
    assert_eq!(
        end.csv_bytes,
        Some(stored.len() as u64),
        "it reports the bytes the server acknowledged"
    );

    let (csv_summary, log_summary) = (
        server.shared.csv.summary().await,
        server.shared.log.summary().await,
    );
    assert_eq!(
        stream_problem(Some(&end), &csv_summary, &log_summary).as_deref(),
        Some("the page gave up a stream")
    );
    assert!(csv_summary.refused > 0);
    server.stop().await;
}

async fn reset_page(name: &str, search: &str, cookie_store: bool) -> serde_json::Value {
    let dir = scratch(name);
    let server = TestServer::start_with(dir.clone(), dir.join("root"), Limits::DEFAULT).await;
    let page = server.send("GET", &server.perf("reset"), &[], b"").await;
    assert_eq!(page.status, 200);
    std::fs::write(dir.join("reset.html"), &page.body).unwrap();
    std::fs::copy(
        here().join("tests/node/reset_page.mjs"),
        dir.join("reset_page.mjs"),
    )
    .unwrap();

    let case = serde_json::json!({ "search": search, "cookieStore": cookie_store }).to_string();
    let output = node(
        &[
            "reset_page.mjs",
            "reset.html",
            &format!("http://localhost:{}", server.port),
            server.token(),
            &case,
        ],
        &dir,
    )
    .await;
    check_succeeded(&output);
    server.stop().await;
    serde_json::from_slice(&output.stdout).expect("the stand-in browser's report")
}

fn query_to(target: &str) -> String {
    format!("?to={}", urlencoding::encode(target))
}

#[tokio::test]
async fn the_reset_page_continues_only_to_a_path_on_its_own_origin() {
    if !node_available() {
        return;
    }
    let app = "/?autopilot&realm=supportteam.dcl.eth&perfStream=/__perf/abc&perfCsv";

    for (target, continues_to) in [
        (app, app),
        ("/sub/page.html?x=1#frag", "/sub/page.html?x=1#frag"),
        ("/", "/"),
        // Another origin, however it is spelled: a browser reads `/\host` and `/<tab>/host`
        // as `//host`.
        ("/\\evil.example", "/"),
        ("//evil.example", "/"),
        ("/\t/evil.example", "/"),
        ("/\n/evil.example", "/"),
        ("/ evil", "/"),
        ("/a\\b", "/"),
        ("https://evil.example/", "/"),
        ("http://localhost:1/", "/"),
        ("javascript:alert(1)", "/"),
        ("evil.example", "/"),
        ("", "/"),
    ] {
        let search = if target.is_empty() {
            String::new()
        } else {
            query_to(target)
        };
        let seen = reset_page("reset-target", &search, false).await;
        assert_eq!(seen["replaced"], continues_to, "to={target:?}");
    }
    assert_eq!(
        reset_page("reset-target-none", "", false).await["replaced"],
        "/"
    );
}

#[tokio::test]
async fn the_reset_page_clears_what_a_build_leaves_and_keeps_the_guests_key() {
    if !node_available() {
        return;
    }
    for cookie_store in [false, true] {
        let seen = reset_page("reset-clears", &query_to("/"), cookie_store).await;

        assert_eq!(seen["unregistered"], 2, "service workers");
        assert_eq!(
            seen["localStorage"],
            serde_json::json!(["dcl-local-guest-root-key"])
        );
        assert_eq!(seen["sessionStorage"], 0);
        if cookie_store {
            assert_eq!(seen["cookiesDeleted"], serde_json::json!(["one", "two"]));
        }
        assert_eq!(seen["errors"], serde_json::json!([]));

        let posted = &seen["posted"];
        assert!(
            posted["url"].as_str().unwrap().starts_with("/__perf/"),
            "{posted}"
        );
        assert!(
            posted["url"].as_str().unwrap().ends_with("/reset-done"),
            "{posted}"
        );
        assert_eq!(posted["method"], "POST");
        assert_eq!(
            posted["body"],
            "serviceWorkers=2 localStorage=2 sessionStorage=3 cookies=2 guestKey=kept"
        );
    }
}

/// The text in quotes right after `after`: `const KEEP = 'name'` gives `name`.
fn quoted_after(text: &str, after: &str) -> Option<String> {
    let rest = &text[text.find(after)? + after.len()..];
    let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
    let inner = &rest[quote.len_utf8()..];
    Some(inner[..inner.find(quote)?].to_string())
}

/// The reset page keeps one local storage key, the local guest's root key, and the web app names it
/// in `sso.ts`; a rename there that the reset page does not follow would make it delete the guest in
/// every run. (The fake app holds the name too, to look for the guest.) Needs no node.
#[test]
fn the_reset_page_keeps_the_key_the_web_app_stores_the_guest_under() {
    let read = |path: &str| {
        std::fs::read_to_string(here().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
    };
    let app = quoted_after(
        &read("../../react-web/src/features/auth/sso.ts"),
        "const LOCAL_GUEST_ROOT_KEY = ",
    )
    .expect("sso.ts names the guest's root key in LOCAL_GUEST_ROOT_KEY");
    assert!(!app.is_empty());

    assert_eq!(
        quoted_after(&read("src/reset.html"), "const KEEP = ").as_deref(),
        Some(app.as_str()),
        "the reset page keeps another key than the web app's guest"
    );
    assert_eq!(
        quoted_after(&read("fake-app/index.html"), "const ROOT_KEY = ").as_deref(),
        Some(app.as_str()),
        "the fake app looks for another key than the web app's guest"
    );
}
