//! One browser run: the server, Chrome, and the wait for the page to end the run.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::access::Access;
use crate::byte_stream::{ByteStream, StreamSummary};
use crate::chrome;
use crate::cli::Settings;
use crate::failure::Failure;
use crate::limits::Limits;
use crate::out;
use crate::profile;
use crate::server::{self, EndReport, EndSignal, Shared};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ended,
    TimedOut,
    BrowserExited,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub build: String,
    pub warmup: bool,
    pub outcome: Outcome,
    /// What the page said in `end`; -1 when it never did.
    pub page_exit_code: i32,
    pub elapsed: Duration,
    pub csv: StreamSummary,
    pub log: StreamSummary,
    /// Why the files of a run the page ended are not what it sent, when they are not.
    pub stream_problem: Option<String>,
}

/// Where the run's page is opened.
pub struct PageSink(pub oneshot::Sender<String>);

pub async fn run_once(
    settings: &Settings,
    app_root: &Path,
    build: &str,
    warmup: bool,
    cancel: &CancellationToken,
) -> Result<RunResult, Failure> {
    run_with_limits(
        settings,
        app_root,
        build,
        warmup,
        cancel,
        Limits::DEFAULT,
        None,
    )
    .await
}

/// `run_once` with the limits and the page's opener given: with `page`, the address (token
/// included) is handed over instead of being opened in a browser, for a caller that plays the
/// page itself (`settings.no_browser` must be set).
pub async fn run_with_limits(
    settings: &Settings,
    app_root: &Path,
    build: &str,
    warmup: bool,
    cancel: &CancellationToken,
    limits: Limits,
    page: Option<PageSink>,
) -> Result<RunResult, Failure> {
    std::fs::create_dir_all(&settings.out_dir)?;
    if !settings.no_browser {
        std::fs::create_dir_all(&settings.profile)?;
    }

    // A file left from an earlier run must not be appended to, or taken for this run's.
    let csv_path = settings.out_dir.join(format!("{build}.csv"));
    let log_path = settings.out_dir.join(format!("{build}.log"));
    for path in [&csv_path, &log_path] {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let started = Instant::now();

    // Resolve Chrome before opening the port, so a missing browser stops here.
    let chrome_path = if settings.no_browser {
        None
    } else {
        Some(chrome::find(settings.chrome_path.as_deref())?)
    };

    // The profile outlives every run and build on the laptop: a run starts from a scrubbed one,
    // with no Chrome left from an earlier job running on it. (`--no-browser`: the profile is
    // whoever's browser opens the address.)
    if chrome_path.is_some() {
        let (profile, keep) = (settings.profile.clone(), settings.keep_service_worker);
        tokio::task::spawn_blocking(move || {
            profile::close_strays(&profile)?;
            if !keep {
                profile::scrub(&profile)?;
            }
            Ok::<_, Failure>(())
        })
        .await
        .map_err(|error| Failure::cannot_run(format!("the profile scrub stopped: {error}")))??;
    }

    let listeners = server::bind(settings.port).await.map_err(|error| {
        Failure::cannot_run(format!(
            "cannot listen on localhost:{} ({error}). Is another run still going? The port must \
             stay the same between runs (the Chrome profile keeps the stored guest per origin); \
             to change it, set PERF_PORT for every run on this machine.",
            settings.port
        ))
    })?;
    let addresses = listeners.addresses();
    if addresses.is_empty() || !addresses.iter().all(|address| address.ip().is_loopback()) {
        return Err(Failure::cannot_run(
            "the server would not listen on loopback only; stopping",
        ));
    }
    let port = listeners.port();

    let shared = Arc::new(Shared {
        access: Access::new(port),
        csv: ByteStream::new(csv_path, limits.file_bytes),
        log: ByteStream::new(log_path, limits.file_bytes),
        end: EndSignal::default(),
        missing: Default::default(),
        root: app_root.to_path_buf(),
        limits,
    });
    let server = listeners.serve(shared.clone());
    out::log(format!(
        "listening on {} (loopback only)",
        addresses
            .iter()
            .map(|address| format!("http://{address}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    let url = page_url(settings, &shared.access, port, warmup);
    // The token is a secret while the run lasts and the job log outlives it: a person who opens
    // the address by hand needs it, the log does not.
    let shown = if settings.no_browser {
        url.clone()
    } else {
        url.replace(shared.access.token(), "<token>")
    };

    let mut chrome = None;
    let waited = wait_for_end(
        settings,
        app_root,
        build,
        warmup,
        cancel,
        &shared,
        chrome_path.as_deref(),
        &url,
        &shown,
        page,
        &mut chrome,
    )
    .await;

    if let Some(chrome) = chrome.as_mut() {
        let patience = if matches!(waited, Ok(Outcome::Cancelled)) {
            chrome::Patience::INTERRUPTED
        } else {
            chrome::Patience::RUN
        };
        chrome::close(chrome, patience).await;
    }
    shared.csv.close().await;
    shared.log.close().await;
    server.stop().await;

    if shared.access.refused() > 0 {
        out::log(format!(
            "{build}: {} request(s) refused (not this run's Host or token)",
            shared.access.refused()
        ));
    }
    if shared.missing.total() > 0 {
        out::log(format!(
            "{build}: {} request(s) for a file the web app does not have (404)",
            shared.missing.total()
        ));
    }
    let outcome = waited?;

    if !settings.keep_service_worker && !shared.access.reset_reported() {
        out::warn(format!(
            "{build}: the profile reset page never reported, so local storage and cookies may hold an earlier build's"
        ));
    }

    let report = shared.end.get();
    let (csv, log) = (shared.csv.summary().await, shared.log.summary().await);

    Ok(RunResult {
        build: build.to_string(),
        warmup,
        outcome,
        page_exit_code: report.as_ref().map_or(-1, |report| report.exit_code),
        elapsed: started.elapsed(),
        csv,
        log,
        stream_problem: stream_problem(report.as_ref(), &csv, &log),
    })
}

/// The page address (the benchmark guideline, Launch options): every launch streams (the log, and
/// `end`); only a measured run adds `perfCsv`, the web twin of `--csv`, so the page sends CSV
/// bytes and the engine measures.
///
/// The stream path carries the run's token: the page takes the path from the address and sends
/// every request to it, and the server answers no other.
fn page_url(settings: &Settings, access: &Access, port: u16, warmup: bool) -> String {
    let target = format!(
        "/?autopilot&realm={}&perfStream={}{}{}",
        settings.realm,
        access.stream_path(),
        if warmup { "" } else { "&perfCsv" },
        settings
            .query
            .as_deref()
            .map_or(String::new(), |query| format!("&{query}")),
    );
    let origin = format!("http://localhost:{port}");

    if settings.keep_service_worker {
        format!("{origin}{target}")
    } else {
        format!(
            "{origin}{}/reset?to={}",
            access.stream_path(),
            urlencoding::encode(&target)
        )
    }
}

#[allow(clippy::too_many_arguments)]
async fn wait_for_end(
    settings: &Settings,
    app_root: &Path,
    build: &str,
    warmup: bool,
    cancel: &CancellationToken,
    shared: &Shared,
    chrome_path: Option<&Path>,
    url: &str,
    shown: &str,
    page: Option<PageSink>,
    chrome: &mut Option<tokio::process::Child>,
) -> Result<Outcome, Failure> {
    match (chrome_path, page) {
        (Some(path), _) => {
            out::log(format!(
                "{build}: serving {}; opening Chrome on {shown}",
                app_root.display()
            ));
            *chrome = Some(chrome::launch(path, settings, url)?);
        }
        (None, page) => {
            out::log(format!(
                "serving {} at {shown} (no browser; open it in Chrome)",
                app_root.display()
            ));
            if let Some(PageSink(sink)) = page {
                let _ = sink.send(url.to_string());
            }
        }
    }

    let limit = if warmup {
        settings.warmup_timeout
    } else {
        settings.timeout
    };
    let browser_exit = async {
        match chrome.as_mut() {
            Some(chrome) => {
                let _ = chrome.wait().await;
            }
            None => std::future::pending().await,
        }
    };

    tokio::select! {
        biased;
        _ = shared.end.wait() => Ok(Outcome::Ended),
        _ = browser_exit => Ok(Outcome::BrowserExited),
        _ = tokio::time::sleep(limit) => Ok(Outcome::TimedOut),
        _ = cancel.cancelled() => Ok(Outcome::Cancelled),
    }
}

/// A page that ended the run can still have given up a stream, and what it sent is then more than
/// what is stored: the files would look complete. `end` carries the totals; none when it says all
/// is well or says nothing.
pub fn stream_problem(
    report: Option<&EndReport>,
    csv: &StreamSummary,
    log: &StreamSummary,
) -> Option<String> {
    let report = report?;
    let mut problems = Vec::new();

    if report.failed {
        problems.push("the page gave up a stream".to_string());
    }
    if let Some(sent) = report.csv_bytes.filter(|sent| *sent != csv.length) {
        problems.push(format!(
            "the page sent {sent} B of CSV and {} B are stored",
            csv.length
        ));
    }
    if let Some(sent) = report.log_bytes.filter(|sent| *sent != log.length) {
        problems.push(format!(
            "the page sent {sent} B of log and {} B are stored",
            log.length
        ));
    }

    (!problems.is_empty()).then(|| problems.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(length: u64) -> StreamSummary {
        StreamSummary {
            created: length > 0,
            length,
            ..Default::default()
        }
    }

    fn report(csv: Option<u64>, log: Option<u64>, failed: bool) -> EndReport {
        EndReport {
            exit_code: 0,
            csv_bytes: csv,
            log_bytes: log,
            failed,
        }
    }

    #[test]
    fn a_whole_run_has_no_stream_problem() {
        let (csv, log) = (summary(8), summary(6));
        assert_eq!(stream_problem(None, &csv, &log), None);
        assert_eq!(
            stream_problem(Some(&report(None, None, false)), &csv, &log),
            None
        );
        assert_eq!(
            stream_problem(Some(&report(Some(8), Some(6), false)), &csv, &log),
            None
        );
    }

    #[test]
    fn a_page_that_gave_up_or_sent_more_than_is_stored_is_a_stream_problem() {
        let (csv, log) = (summary(8), summary(6));
        let problem = |r: EndReport| stream_problem(Some(&r), &csv, &log).unwrap();
        assert_eq!(
            problem(report(None, None, true)),
            "the page gave up a stream"
        );
        assert_eq!(
            problem(report(Some(108), None, false)),
            "the page sent 108 B of CSV and 8 B are stored"
        );
        assert_eq!(
            problem(report(Some(9), Some(7), true)),
            "the page gave up a stream; the page sent 9 B of CSV and 8 B are stored; the page sent 7 B of log and 6 B are stored"
        );
    }

    fn settings() -> Settings {
        Settings {
            out_dir: ".".into(),
            port: 18742,
            profile: "p".into(),
            chrome_path: None,
            realm: "supportteam.dcl.eth".into(),
            query: None,
            window: None,
            chrome_args: vec![],
            no_browser: false,
            keep_service_worker: false,
            timeout: Duration::from_secs(1),
            warmup_timeout: Duration::from_secs(1),
        }
    }

    #[test]
    fn a_measured_run_opens_the_reset_page_that_continues_to_the_app() {
        let access = Access::new(18742);
        let url = page_url(&settings(), &access, 18742, false);
        let stream = access.stream_path();
        let target = format!("/?autopilot&realm=supportteam.dcl.eth&perfStream={stream}&perfCsv");
        assert_eq!(
            url,
            format!(
                "http://localhost:18742{stream}/reset?to={}",
                urlencoding::encode(&target)
            )
        );
    }

    #[test]
    fn a_warm_up_streams_the_log_only_and_the_query_is_appended() {
        let access = Access::new(18742);
        let mut s = settings();
        s.keep_service_worker = true;
        s.query = Some("guest=1".into());
        let url = page_url(&s, &access, 18742, true);
        assert_eq!(
            url,
            format!(
                "http://localhost:18742/?autopilot&realm=supportteam.dcl.eth&perfStream={}&guest=1",
                access.stream_path()
            )
        );
    }
}
