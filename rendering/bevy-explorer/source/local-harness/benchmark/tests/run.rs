//! A whole run in this process: the runner serves a web app, a test plays the page, and the files
//! and the result are what `measure` would keep.

mod support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use benchmark::cli::Settings;
use benchmark::commands::{clear_earlier_session, clear_run_files, keep_files};
use benchmark::failure::Failure;
use benchmark::limits::Limits;
use benchmark::runner::{run_with_limits, Outcome, PageSink, RunResult};
use support::{fake_app, scratch, Request};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

const HEADER: &str = "\"Frame\",\"CPU Time\",\"GPU Time\"\r\n";

fn settings(out: &Path, timeout: Duration) -> Settings {
    Settings {
        out_dir: out.to_path_buf(),
        port: 0,
        profile: out.join("profile"),
        chrome_path: None,
        realm: "supportteam.dcl.eth".into(),
        query: None,
        window: None,
        chrome_args: vec![],
        no_browser: true,
        keep_service_worker: true,
        timeout,
        warmup_timeout: timeout,
    }
}

/// The page of a run, as the runner opened it for a browser.
struct Page {
    port: u16,
    stream: String,
    measuring: bool,
}

impl Page {
    fn open(url: &str) -> Self {
        let rest = url
            .strip_prefix("http://localhost:")
            .expect("a localhost address");
        let (port, query) = rest.split_once("/?").expect("the app page");
        let stream = query
            .split('&')
            .find_map(|part| part.strip_prefix("perfStream="))
            .expect("a perfStream");
        assert!(query.contains("autopilot"), "{query}");
        assert!(query.contains("realm=supportteam.dcl.eth"), "{query}");
        Self {
            port: port.parse().unwrap(),
            stream: stream.to_string(),
            measuring: query.split('&').any(|part| part == "perfCsv"),
        }
    }

    async fn post(&self, what: &str, offset: usize, body: &[u8]) -> u16 {
        Request::new("POST", &format!("{}/{what}", self.stream))
            .header("Host", &format!("localhost:{}", self.port))
            .header("X-Perf-Offset", &offset.to_string())
            .body(body)
            .send(self.port)
            .await
            .status
    }

    async fn end(&self, json: &str) -> u16 {
        Request::new("POST", &format!("{}/end", self.stream))
            .header("Host", &format!("localhost:{}", self.port))
            .header("Content-Type", "application/json")
            .body(json.as_bytes())
            .send(self.port)
            .await
            .status
    }

    async fn get(&self, path: &str) -> u16 {
        Request::new("GET", path)
            .header("Host", &format!("localhost:{}", self.port))
            .send(self.port)
            .await
            .status
    }
}

fn csv_rows(rows: usize) -> String {
    let mut text = HEADER.to_string();
    for frame in 1..=rows {
        text += &format!("{frame},16.{},0\r\n", frame % 10);
    }
    text
}

/// What a page does in a run, once it has its address.
type Script =
    Box<dyn FnOnce(Page) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send>;

fn script<F, Fut>(play: F) -> Script
where
    F: FnOnce(Page) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    Box::new(move |page| Box::pin(play(page)))
}

async fn run(
    out: &Path,
    timeout: Duration,
    warmup: bool,
    limits: Limits,
    cancel: CancellationToken,
    play: Script,
) -> Result<RunResult, Failure> {
    let (sink, address) = oneshot::channel();
    let settings = settings(out, timeout);
    let root = fake_app();
    let runner = tokio::spawn({
        let cancel = cancel.clone();
        async move {
            run_with_limits(
                &settings,
                &root,
                "change_build",
                warmup,
                &cancel,
                limits,
                Some(PageSink(sink)),
            )
            .await
        }
    });
    let page = Page::open(&address.await.expect("the runner gave the page's address"));
    play(page).await;
    runner.await.unwrap()
}

async fn plain_run(out: &Path, play: Script) -> RunResult {
    run(
        out,
        Duration::from_secs(20),
        false,
        Limits::DEFAULT,
        CancellationToken::new(),
        play,
    )
    .await
    .expect("the run")
}

fn kept(out: &Path, name: &str) -> Option<Vec<u8>> {
    std::fs::read(out.join(name)).ok()
}

#[tokio::test]
async fn a_run_the_page_ends_leaves_both_files_as_the_desktop_client_would() {
    let out = scratch("run-ok");
    let csv = csv_rows(300);
    let log = "12:00:00.000 - <color=#8CC8FF>[autopilot]</color>: in world\n";

    let result = plain_run(
        &out,
        script({
            let (csv, log) = (csv.clone(), log.to_string());
            move |page| async move {
                assert!(page.measuring);
                assert_eq!(
                    page.get("/index.html").await,
                    200,
                    "the app is served from the runner"
                );
                assert_eq!(page.post("log", 0, log.as_bytes()).await, 204);
                assert_eq!(page.post("csv", 0, &csv.as_bytes()[..100]).await, 204);
                assert_eq!(page.post("csv", 100, &csv.as_bytes()[100..]).await, 204);
                assert_eq!(
                    page.post("csv", 100, &csv.as_bytes()[100..]).await,
                    204,
                    "a repeat"
                );
                let end = format!(
                    "{{\"exitCode\": 0, \"csvBytes\": {}, \"logBytes\": {}, \"failed\": false}}",
                    csv.len(),
                    log.len()
                );
                assert_eq!(page.end(&end).await, 204);
            }
        }),
    )
    .await;

    assert_eq!(result.outcome, Outcome::Ended);
    assert_eq!(result.page_exit_code, 0);
    assert_eq!(result.stream_problem, None);
    assert_eq!(
        (result.csv.length, result.log.length),
        (csv.len() as u64, log.len() as u64)
    );
    assert_eq!(result.csv.duplicates, 1);
    assert_eq!(benchmark::commands::exit_code(&result), 0);

    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        1,
        &result,
    )
    .unwrap();
    assert_eq!(kept(&out, "change_build_1.csv").unwrap(), csv.as_bytes());
    assert_eq!(kept(&out, "change_build_1.log").unwrap(), log.as_bytes());
    assert!(kept(&out, "change_build_1.csv.partial").is_none());
    assert!(
        kept(&out, "change_build.csv").is_none(),
        "the run's files were renamed"
    );
}

#[tokio::test]
async fn a_warm_up_streams_the_log_and_ends() {
    let out = scratch("run-warmup");
    let result = run(
        &out,
        Duration::from_secs(20),
        true,
        Limits::DEFAULT,
        CancellationToken::new(),
        script(|page| async move {
            assert!(!page.measuring, "a warm-up has no perfCsv");
            assert_eq!(page.post("log", 0, b"line\n").await, 204);
            assert_eq!(
                page.end(r#"{"exitCode": 0, "csvBytes": 0, "logBytes": 5, "failed": false}"#)
                    .await,
                204
            );
        }),
    )
    .await
    .unwrap();

    assert_eq!(result.outcome, Outcome::Ended);
    assert_eq!(result.stream_problem, None);
    assert!(!result.csv.created);
    assert_eq!(kept(&out, "change_build.log").unwrap(), b"line\n");
    assert!(kept(&out, "change_build.csv").is_none());
}

#[tokio::test]
async fn a_stream_the_page_says_is_short_sets_the_csv_aside_and_keeps_the_log() {
    let out = scratch("run-short");
    let csv = csv_rows(50);

    let result = plain_run(
        &out,
        script({
            let csv = csv.clone();
            move |page| async move {
                assert_eq!(page.post("csv", 0, csv.as_bytes()).await, 204);
                assert_eq!(page.post("log", 0, b"log\n").await, 204);
                // The page handed over more CSV than the runner holds.
                let end = format!(
                    "{{\"exitCode\": 0, \"csvBytes\": {}, \"logBytes\": 4}}",
                    csv.len() + 100
                );
                assert_eq!(page.end(&end).await, 204);
            }
        }),
    )
    .await;

    let problem = result.stream_problem.clone().expect("a stream problem");
    assert!(
        problem.contains("sent") && problem.contains("stored"),
        "{problem}"
    );
    assert_eq!(benchmark::commands::exit_code(&result), 1);

    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        2,
        &result,
    )
    .unwrap();
    assert_eq!(
        kept(&out, "change_build_2.csv.partial").unwrap(),
        csv.as_bytes()
    );
    assert!(kept(&out, "change_build_2.csv").is_none());
    assert_eq!(kept(&out, "change_build_2.log").unwrap(), b"log\n");
}

#[tokio::test]
async fn a_page_that_gave_a_stream_up_is_a_partial_run_however_whole_the_files_look() {
    let out = scratch("run-failed");
    let csv = csv_rows(20);

    let result = plain_run(
        &out,
        script({
            let csv = csv.clone();
            move |page| async move {
                assert_eq!(page.post("csv", 0, csv.as_bytes()).await, 204);
                let end = format!(
                    "{{\"exitCode\": 0, \"csvBytes\": {}, \"logBytes\": 0, \"failed\": true}}",
                    csv.len()
                );
                assert_eq!(page.end(&end).await, 204);
            }
        }),
    )
    .await;

    assert_eq!(
        result.stream_problem.as_deref(),
        Some("the page gave up a stream")
    );
    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        1,
        &result,
    )
    .unwrap();
    assert!(kept(&out, "change_build_1.csv.partial").is_some());
    assert!(kept(&out, "change_build_1.csv").is_none());
}

#[tokio::test]
async fn a_csv_with_a_header_and_no_rows_is_set_aside_for_the_parser() {
    let out = scratch("run-header");
    let result = plain_run(
        &out,
        script(|page| async move {
            assert_eq!(page.post("csv", 0, HEADER.as_bytes()).await, 204);
            let end = format!(
                "{{\"exitCode\": 0, \"csvBytes\": {}, \"logBytes\": 0}}",
                HEADER.len()
            );
            assert_eq!(page.end(&end).await, 204);
        }),
    )
    .await;

    assert_eq!(result.stream_problem, None);
    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        1,
        &result,
    )
    .unwrap();
    assert_eq!(
        kept(&out, "change_build_1.csv.partial").unwrap(),
        HEADER.as_bytes()
    );
    assert!(kept(&out, "change_build_1.csv").is_none());
}

#[tokio::test]
async fn a_panic_the_page_reports_is_exit_code_101_and_its_files_stay() {
    let out = scratch("run-panic");
    let csv = csv_rows(10);
    let result = plain_run(
        &out,
        script({
            let csv = csv.clone();
            move |page| async move {
                assert_eq!(page.post("csv", 0, csv.as_bytes()).await, 204);
                let end = format!(
                    "{{\"exitCode\": 101, \"csvBytes\": {}, \"logBytes\": 0}}",
                    csv.len()
                );
                assert_eq!(page.end(&end).await, 204);
            }
        }),
    )
    .await;

    assert_eq!(
        (result.outcome, result.page_exit_code),
        (Outcome::Ended, 101)
    );
    assert_eq!(benchmark::commands::exit_code(&result), 1);
    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        1,
        &result,
    )
    .unwrap();
    assert_eq!(
        kept(&out, "change_build_1.csv").unwrap(),
        csv.as_bytes(),
        "what was streamed stays"
    );
}

#[tokio::test]
async fn a_page_that_never_ends_is_cut_off_by_the_timeout_and_what_it_sent_stays() {
    let out = scratch("run-timeout");
    let csv = csv_rows(30);
    let result = run(
        &out,
        Duration::from_secs(1),
        false,
        Limits::DEFAULT,
        CancellationToken::new(),
        script({
            let csv = csv.clone();
            move |page| async move {
                assert_eq!(page.post("csv", 0, csv.as_bytes()).await, 204);
            }
        }),
    )
    .await
    .unwrap();

    assert_eq!(result.outcome, Outcome::TimedOut);
    assert_eq!(result.page_exit_code, -1);
    assert_eq!(benchmark::commands::exit_code(&result), 2);
    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        1,
        &result,
    )
    .unwrap();
    assert_eq!(kept(&out, "change_build_1.csv").unwrap(), csv.as_bytes());
}

#[tokio::test]
async fn an_interrupt_ends_the_wait() {
    let out = scratch("run-cancel");
    let cancel = CancellationToken::new();
    let result = run(
        &out,
        Duration::from_secs(30),
        false,
        Limits::DEFAULT,
        cancel.clone(),
        script({
            let cancel = cancel.clone();
            move |_| async move { cancel.cancel() }
        }),
    )
    .await
    .unwrap();

    assert_eq!(result.outcome, Outcome::Cancelled);
    assert_eq!(benchmark::commands::exit_code(&result), 130);
}

#[tokio::test]
async fn files_left_by_an_earlier_run_are_not_appended_to() {
    let out = scratch("run-stale");
    std::fs::write(out.join("change_build.csv"), "old csv from an earlier run").unwrap();
    std::fs::write(out.join("change_build.log"), "old log").unwrap();

    let csv = csv_rows(5);
    let result = plain_run(
        &out,
        script({
            let csv = csv.clone();
            move |page| async move {
                assert_eq!(page.post("csv", 0, csv.as_bytes()).await, 204);
                assert_eq!(
                    page.end(&format!(
                        "{{\"exitCode\": 0, \"csvBytes\": {}, \"logBytes\": 0}}",
                        csv.len()
                    ))
                    .await,
                    204
                );
            }
        }),
    )
    .await;

    assert_eq!(result.stream_problem, None);
    assert_eq!(kept(&out, "change_build.csv").unwrap(), csv.as_bytes());
    assert!(
        kept(&out, "change_build.log").is_none(),
        "a run that streamed no log leaves none"
    );
}

#[tokio::test]
async fn a_csv_an_earlier_run_kept_is_not_read_next_to_this_runs_partial() {
    let out = scratch("run-twice");
    let settings = settings(&out, Duration::from_secs(1));
    let result = |problem: Option<&str>| RunResult {
        build: "change_build".into(),
        warmup: false,
        outcome: Outcome::Ended,
        page_exit_code: 0,
        elapsed: Duration::ZERO,
        csv: Default::default(),
        log: Default::default(),
        stream_problem: problem.map(String::from),
    };

    std::fs::write(out.join("change_build.csv"), csv_rows(3)).unwrap();
    keep_files(&settings, "change_build", 1, &result(None)).unwrap();
    assert!(kept(&out, "change_build_1.csv").is_some());

    std::fs::write(out.join("change_build.csv"), csv_rows(4)).unwrap();
    keep_files(
        &settings,
        "change_build",
        1,
        &result(Some("the page gave up a stream")),
    )
    .unwrap();
    assert!(
        kept(&out, "change_build_1.csv").is_none(),
        "the report would read it as this run's"
    );
    assert!(kept(&out, "change_build_1.csv.partial").is_some());

    std::fs::write(out.join("change_build.csv"), csv_rows(5)).unwrap();
    keep_files(&settings, "change_build", 1, &result(None)).unwrap();
    assert!(kept(&out, "change_build_1.csv.partial").is_none());
    assert_eq!(
        kept(&out, "change_build_1.csv").unwrap(),
        csv_rows(5).as_bytes()
    );
}

#[tokio::test]
async fn a_run_that_leaves_no_csv_does_not_inherit_what_an_earlier_session_kept_for_it() {
    let out = scratch("run-inherit");
    let settings = settings(&out, Duration::from_secs(1));
    for name in [
        "change_build_1.csv",
        "change_build_1.csv.partial",
        "change_build_1.log",
        "change_build_2.csv",
        "base_build_1.csv",
    ] {
        std::fs::write(out.join(name), csv_rows(3)).unwrap();
    }

    // What `measure` does for run 1 of the change build: clear its files, run, keep what is left.
    clear_run_files(&settings, "change_build", 1).unwrap();
    let result = plain_run(
        &out,
        script(|page| async move {
            assert_eq!(page.end(r#"{"exitCode": 1}"#).await, 204);
        }),
    )
    .await;
    keep_files(&settings, "change_build", 1, &result).unwrap();

    for gone in [
        "change_build_1.csv",
        "change_build_1.csv.partial",
        "change_build_1.log",
    ] {
        assert!(
            kept(&out, gone).is_none(),
            "{gone} would be read as this run's"
        );
    }
    for stays in ["change_build_2.csv", "base_build_1.csv"] {
        assert!(kept(&out, stays).is_some(), "{stays} is another run's");
    }
    clear_run_files(&settings, "change_build", 1).expect("clearing what is not there is fine");
}

#[test]
fn a_new_session_clears_every_run_an_earlier_session_left_and_nothing_else() {
    let out = scratch("run-earlier-session");
    let settings = settings(&out, Duration::from_secs(1));
    let gone = [
        "change_build_1.csv",
        "change_build_4.csv.partial",
        "change_build_12.log",
        "base_build_3.csv",
        "runs_on.txt",
    ];
    let stays = [
        "change_build.log",
        "base_build.csv",
        "change_build_x.csv",
        "change_build_1.txt",
        "notes.txt",
    ];
    for name in gone.iter().chain(&stays) {
        std::fs::write(out.join(name), "x").unwrap();
    }
    std::fs::create_dir_all(out.join("change_build").join("app")).unwrap();

    clear_earlier_session(&settings).unwrap();

    for name in gone {
        assert!(kept(&out, name).is_none(), "{name} is an earlier session's");
    }
    for name in stays {
        assert!(kept(&out, name).is_some(), "{name} is not a run's output");
    }
    assert!(
        out.join("change_build").join("app").is_dir(),
        "the app folder stays"
    );
    clear_earlier_session(&settings).expect("clearing what is not there is fine");
}

#[tokio::test]
async fn nothing_is_moved_when_a_run_streamed_nothing() {
    let out = scratch("run-nothing");
    let result = plain_run(
        &out,
        script(|page| async move {
            assert_eq!(page.end(r#"{"exitCode": 1}"#).await, 204);
        }),
    )
    .await;
    keep_files(
        &settings(&out, Duration::from_secs(1)),
        "change_build",
        1,
        &result,
    )
    .unwrap();
    assert!(!PathBuf::from(&out).join("change_build_1.csv").exists());
    assert!(!PathBuf::from(&out).join("change_build_1.log").exists());
}

#[tokio::test]
async fn a_port_that_is_taken_stops_the_run_with_a_message() {
    let out = scratch("run-port");
    let taken = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let mut busy = settings(&out, Duration::from_secs(1));
    busy.port = taken.local_addr().unwrap().port();

    let result = run_with_limits(
        &busy,
        &fake_app(),
        "change_build",
        false,
        &CancellationToken::new(),
        Limits::DEFAULT,
        None,
    )
    .await;
    match result {
        Err(Failure::CannotRun(message)) => assert!(message.contains("cannot listen"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_missing_chrome_stops_the_run_before_the_port_is_opened() {
    let out = scratch("run-no-chrome");
    let mut no_browser = settings(&out, Duration::from_secs(1));
    no_browser.no_browser = false;
    no_browser.chrome_path = Some(out.join("no-such-chrome").to_string_lossy().into_owned());

    let result = run_with_limits(
        &no_browser,
        &fake_app(),
        "change_build",
        false,
        &CancellationToken::new(),
        Limits::DEFAULT,
        None,
    )
    .await;
    match result {
        Err(Failure::CannotRun(message)) => assert!(message.contains("no Chrome"), "{message}"),
        other => panic!("{other:?}"),
    }
}
