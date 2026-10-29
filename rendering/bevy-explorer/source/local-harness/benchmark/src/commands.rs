//! The commands: `run`, `warmup`, `fetch`, `measure`, `leg` and `report`.
//!
//! Exit codes of `run`: 0 the page ended the run with exit code 0, 1 it ended with a failure code
//! or told `end` that a stream is incomplete, 2 the timeout closed Chrome, 3 Chrome exited by
//! itself before `end`, 130 interrupted. 64 is a usage error, 70 the machine cannot run the leg
//! (no Chrome, port taken, ...). `report` exits 0 when a section has data and 2 when none has.

use std::fs::{self, File};
use std::future::Future;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::app_source;
use crate::cli::{
    flag, given, whole_number, Command, FetchArgs, LegArgs, MeasureArgs, ReportArgs, RunArgs,
    Settings, WarmupArgs, BASE_BUILD, CHANGE_BUILD,
};
use crate::failure::Failure;
use crate::out;
use crate::report::EXIT_NO_DATA;
use crate::runner::{self, Outcome, RunResult};

/// What a command runs on: the async runtime the server lives on, and the interrupt.
pub struct Context {
    pub runtime: tokio::runtime::Runtime,
    pub cancel: CancellationToken,
}

impl Context {
    /// A runtime whose first task turns an interrupt (Ctrl+C, and SIGTERM where there is one)
    /// into the cancellation every wait honours.
    pub fn new() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let cancel = CancellationToken::new();
        let interrupted = cancel.clone();
        runtime.spawn(async move {
            wait_for_interrupt().await;
            interrupted.cancel();
        });
        Ok(Self { runtime, cancel })
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    fn run_once(
        &self,
        settings: &Settings,
        root: &Path,
        build: &str,
        warmup: bool,
    ) -> Result<RunResult, Failure> {
        self.block_on(runner::run_once(
            settings,
            root,
            build,
            warmup,
            &self.cancel,
        ))
    }

    fn pause(&self, length: Duration) -> Result<(), Failure> {
        self.block_on(async {
            tokio::select! {
                _ = tokio::time::sleep(length) => Ok(()),
                _ = self.cancel.cancelled() => Err(Failure::Interrupted),
            }
        })
    }

    fn check(&self) -> Result<(), Failure> {
        if self.cancel.is_cancelled() {
            Err(Failure::Interrupted)
        } else {
            Ok(())
        }
    }
}

#[cfg(unix)]
async fn wait_for_interrupt() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).ok();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = async {
            match terminate.as_mut() {
                Some(terminate) => { terminate.recv().await; }
                None => std::future::pending::<()>().await,
            }
        } => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_interrupt() {
    let _ = tokio::signal::ctrl_c().await;
}

/// The process exit code the command ends with.
pub fn dispatch(command: Command, ctx: &Context) -> Result<i32, Failure> {
    match command {
        Command::Run(args) => run(args, ctx),
        Command::Warmup(args) => warmup(args, ctx),
        Command::Fetch(args) => fetch(args, ctx),
        Command::Measure(args) => measure(args, ctx),
        Command::Leg(args) => leg(args, ctx),
        Command::Report(args) => write_report(args),
    }
}

fn run(args: RunArgs, ctx: &Context) -> Result<i32, Failure> {
    let settings = Settings::from(&args.machine)?;
    let build = build_name(given(&args.build).unwrap_or(CHANGE_BUILD))?;
    let app =
        given(&args.app).ok_or_else(|| Failure::usage("--app <folder|zip|url> is required"))?;

    let root = app_source::provision(app, &settings.out_dir.join(&build), &ctx.cancel)?;
    let result = ctx.run_once(&settings, &root, &build, flag(&args.warmup))?;
    report(&result);
    Ok(exit_code(&result))
}

fn warmup(args: WarmupArgs, ctx: &Context) -> Result<i32, Failure> {
    let settings = Settings::from(&args.machine)?;
    let build = build_name(given(&args.build).unwrap_or(CHANGE_BUILD))?;
    let app =
        given(&args.app).ok_or_else(|| Failure::usage("--app <folder|zip|url> is required"))?;

    let root = app_source::provision(app, &settings.out_dir.join(&build), &ctx.cancel)?;
    warm_up(ctx, &settings, &root, &build)?;
    Ok(0)
}

fn fetch(args: FetchArgs, ctx: &Context) -> Result<i32, Failure> {
    let source = given(&args.url)
        .or(given(&args.source))
        .ok_or_else(|| Failure::usage("--url <url|zip|folder> is required"))?;
    let dir = given(&args.dir).ok_or_else(|| Failure::usage("--dir <folder> is required"))?;

    // Under Actions the workflow downloads the build (a CI artifact), so the runner downloads
    // nothing itself.
    if app_source::under_actions() {
        app_source::require_local(source)?;
    }

    let root = app_source::provision(source, &std::path::absolute(dir)?, &ctx.cancel)?;
    out::log(format!("web app ready at {}", root.display()));
    Ok(0)
}

fn measure(args: MeasureArgs, ctx: &Context) -> Result<i32, Failure> {
    let settings = Settings::from(&args.machine)?;
    let change_dir = given(&args.change_dir).unwrap_or(CHANGE_BUILD);
    let label = given(&args.label).ok_or_else(|| {
        Failure::usage("--label <runner label> is required (it names the report section)")
    })?;
    let runs = whole_number("runs", &args.runs, 3)?;
    let settle = Duration::from_secs(whole_number("settle", &args.settle, 15)?);

    let change = app_source::find_app_root(&std::path::absolute(change_dir)?)?;
    let baseline = match given(&args.base_dir) {
        Some(dir) => Some(app_source::find_app_root(&std::path::absolute(dir)?)?),
        None => None,
    };

    measure_loop(
        ctx,
        &settings,
        &change,
        baseline.as_deref(),
        runs,
        settle,
        label,
    )?;
    Ok(0)
}

fn leg(args: LegArgs, ctx: &Context) -> Result<i32, Failure> {
    let settings = Settings::from(&args.machine)?;
    let change = given(&args.change)
        .ok_or_else(|| Failure::usage("--change <folder|zip|url> is required"))?;
    let label = given(&args.label).unwrap_or("local-chrome");
    let runs = whole_number("runs", &args.runs, 3)?;
    let settle = Duration::from_secs(whole_number("settle", &args.settle, 15)?);

    let change_root =
        app_source::provision(change, &settings.out_dir.join(CHANGE_BUILD), &ctx.cancel)?;
    let base_root = match given(&args.baseline) {
        Some(baseline) => Some(app_source::provision(
            baseline,
            &settings.out_dir.join(BASE_BUILD),
            &ctx.cancel,
        )?),
        None => None,
    };

    warm_up(ctx, &settings, &change_root, CHANGE_BUILD)?;
    if let Some(base_root) = &base_root {
        warm_up(ctx, &settings, base_root, BASE_BUILD)?;
    }

    measure_loop(
        ctx,
        &settings,
        &change_root,
        base_root.as_deref(),
        runs,
        settle,
        label,
    )?;
    Ok(0)
}

/// Writes the report of the results under DIR, whether or not a section has data: the workflow
/// posts it either way.
fn write_report(args: ReportArgs) -> Result<i32, Failure> {
    let pr = match given(&args.pr) {
        Some(_) => Some(whole_number("pr", &args.pr, 0)?),
        None => None,
    };
    let options = crate::report::Options {
        dir: PathBuf::from(given(&args.dir).unwrap_or(".")),
        title: given(&args.title).unwrap_or("Benchmark").to_string(),
        pr,
        change: given(&args.change).map(String::from),
        base: given(&args.baseline).map(String::from),
        run_url: given(&args.run_url).map(String::from),
        expected_runs: whole_number("expected-runs", &args.expected_runs, 3)?,
        expected_labels: given(&args.expected_labels)
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(String::from)
            .collect(),
    };
    let out_file = PathBuf::from(given(&args.out).unwrap_or("summary.md"));

    let report = crate::report::build(&options)?;
    for warning in &report.warnings {
        out::warn(out::safe(warning));
    }
    fs::write(&out_file, &report.markdown)?;
    out::log(format!(
        "wrote the report to {}",
        out::safe(&out_file.to_string_lossy())
    ));
    if report.has_data {
        Ok(0)
    } else {
        out::warn("no section has data");
        Ok(EXIT_NO_DATA)
    }
}

/// Warm-up: sign in (on a new profile, the stored guest is created here), arrive in world, end.
/// Its URL has `perfStream` but no `perfCsv`, so the log streams, `end` arrives and the engine
/// does not measure; the log is stored as <build>.log, as the desktop warm-up writes it, and
/// printed.
fn warm_up(ctx: &Context, settings: &Settings, root: &Path, build: &str) -> Result<(), Failure> {
    let result = ctx.run_once(settings, root, build, true)?;
    report(&result);
    out::print_file(&settings.out_dir.join(format!("{build}.log")))?;
    ctx.check()
}

/// The interleaved loop: change, base, change, base ..., a settle pause after each launch, every
/// run's files renamed to <build>_<i>.csv/.log; then the runner label the report keys on.
fn measure_loop(
    ctx: &Context,
    settings: &Settings,
    change_root: &Path,
    base_root: Option<&Path>,
    runs: u64,
    settle: Duration,
    label: &str,
) -> Result<(), Failure> {
    clear_earlier_session(settings)?;
    for i in 1..=runs {
        measure_one(ctx, settings, change_root, CHANGE_BUILD, i)?;
        ctx.pause(settle)?;

        if let Some(base_root) = base_root {
            measure_one(ctx, settings, base_root, BASE_BUILD, i)?;
            ctx.pause(settle)?;
        }
    }

    out::print_file(&settings.out_dir.join(format!("{CHANGE_BUILD}_1.log")))?;
    fs::write(settings.out_dir.join("runs_on.txt"), format!("{label}\n"))?;
    out::log(format!("wrote runs_on.txt ({})", out::safe(label)));
    Ok(())
}

fn measure_one(
    ctx: &Context,
    settings: &Settings,
    root: &Path,
    build: &str,
    i: u64,
) -> Result<(), Failure> {
    clear_run_files(settings, build, i)?;
    let result = ctx.run_once(settings, root, build, false)?;
    report(&result);
    keep_files(settings, build, i, &result)
}

/// Removes what an earlier `leg` or `measure` into the same `--out` kept for this run:
/// `<build>_<i>.csv`, `<build>_<i>.csv.partial` and `<build>_<i>.log`. A run that then leaves no
/// CSV (it crashed before measuring) has nothing for `keep_files` to move, and the old files
/// would be read by the report as this run's. Other runs' files stay.
pub fn clear_run_files(settings: &Settings, build: &str, i: u64) -> Result<(), Failure> {
    for name in [
        format!("{build}_{i}.csv"),
        format!("{build}_{i}.csv.partial"),
        format!("{build}_{i}.log"),
    ] {
        match fs::remove_file(settings.out_dir.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Removes what an earlier session into the same `--out` left for the report: `runs_on.txt` and
/// every run's `<build>_<n>.csv`, `.csv.partial` and `.log`, including runs this session does not
/// repeat (it measures fewer runs, or no base build). The warm-ups' `<build>.log` and the app
/// folders stay.
pub fn clear_earlier_session(settings: &Settings) -> Result<(), Failure> {
    let entries = match fs::read_dir(&settings.out_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name != "runs_on.txt" && !is_run_file(name) {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// `<build>_<n>.csv`, `<build>_<n>.csv.partial` or `<build>_<n>.log`, for either build.
fn is_run_file(name: &str) -> bool {
    [CHANGE_BUILD, BASE_BUILD].iter().any(|build| {
        name.strip_prefix(build)
            .and_then(|rest| rest.strip_prefix('_'))
            .and_then(|rest| rest.split_once('.'))
            .is_some_and(|(run, extension)| {
                !run.is_empty()
                    && run.bytes().all(|b| b.is_ascii_digit())
                    && matches!(extension, "csv" | "csv.partial" | "log")
            })
    })
}

/// A run that streamed no CSV or log leaves no file; there is then nothing to move. Whatever the
/// page streamed stays as the run's CSV, whether or not the run ended well (a panic, a timeout, a
/// closed Chrome), as the desktop steps keep whatever the client wrote. The one exception is a
/// CSV the harness's parser cannot read (a header and no rows): it throws on it and the whole
/// runner's section is lost, so that CSV is set aside, where the report does not look, and the
/// report counts the run as crashed, as it counts a run without a CSV. So is the CSV of a run
/// whose `end` says a stream is incomplete (the page gave one up, or the bytes it sent are not
/// the bytes stored): it would look complete and is not. The log of such a run stays.
pub fn keep_files(
    settings: &Settings,
    build: &str,
    i: u64,
    result: &RunResult,
) -> Result<(), Failure> {
    for extension in ["csv", "log"] {
        let from = settings.out_dir.join(format!("{build}.{extension}"));
        if !from.exists() {
            continue;
        }

        let mut name = format!("{build}_{i}.{extension}");
        if extension == "csv" {
            if let Some(problem) = &result.stream_problem {
                name += ".partial";
                out::warn(format!(
                    "{build} run {i}: {problem}; its CSV is kept as {name}, the log as it is, and the run counts as crashed"
                ));
            } else if !parser_can_read(&from)? {
                name += ".partial";
                out::warn(format!(
                    "{build} run {i}: its CSV has no rows the parser can read; kept as {name}, the run counts as crashed"
                ));
            }
        }

        // The other spelling of this run's CSV, left by an earlier run in the same folder, would
        // be read as this run's.
        if extension == "csv" {
            let other = if name.ends_with(".partial") {
                format!("{build}_{i}.csv")
            } else {
                format!("{build}_{i}.csv.partial")
            };
            match fs::remove_file(settings.out_dir.join(other)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }

        fs::rename(&from, settings.out_dir.join(name))?;
    }
    Ok(())
}

/// What the harness parser's statistics need of a CSV: after the header, at least one row, and
/// every row with a CPU and a GPU float in its second and third fields (it throws on none, and on
/// a row it cannot split).
pub fn parser_can_read(csv: &Path) -> io::Result<bool> {
    let mut lines = BufReader::new(File::open(csv)?).split(b'\n');
    lines.next().transpose()?; // the header

    let float = |field: &str| field.trim().parse::<f32>().is_ok();
    let mut rows = 0u64;
    for line in lines {
        let line = line?;
        let line = String::from_utf8_lossy(&line);
        let fields: Vec<&str> = line.trim_end_matches('\r').split(',').collect();

        if fields.len() < 3 || !float(fields[1]) || !float(fields[2]) {
            return Ok(false);
        }
        rows += 1;
    }
    Ok(rows > 0)
}

/// The name is a folder under --out that is emptied before a build is unpacked into it, so `.` and
/// `..` (and any name made only of dots) are not names.
pub fn build_name(name: &str) -> Result<String, Failure> {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    if !name.is_empty() && name.chars().all(plain) && name.chars().any(|c| c != '.') {
        Ok(name.to_string())
    } else {
        Err(Failure::usage(format!(
            "--build must be letters, digits, '_', '-' or '.', and not only dots, got '{}'",
            out::safe(name)
        )))
    }
}

pub fn report(result: &RunResult) {
    let how = match result.outcome {
        Outcome::Ended => format!("ended by the page (exit code {})", result.page_exit_code),
        Outcome::TimedOut => "closed by the timeout, the page never sent `end`".to_string(),
        Outcome::BrowserExited => "Chrome exited before the page sent `end`".to_string(),
        Outcome::Cancelled => "interrupted".to_string(),
    };
    let build = &result.build;
    let kind = if result.warmup { "warm-up" } else { "run" };
    let stored = |stream: &crate::byte_stream::StreamSummary| {
        if stream.created {
            format!("{} B", stream.length)
        } else {
            "none".to_string()
        }
    };

    let duplicates = result.csv.duplicates + result.log.duplicates;
    let refused = result.csv.refused + result.log.refused;
    out::log(format!(
        "{build} {kind}: {how} after {} s; csv {}, log {}{}{}",
        seconds(result.elapsed),
        stored(&result.csv),
        stored(&result.log),
        if duplicates > 0 {
            format!("; {duplicates} duplicate chunk(s) ignored")
        } else {
            String::new()
        },
        if refused > 0 {
            format!("; {refused} chunk(s) refused")
        } else {
            String::new()
        },
    ));

    if let Some(problem) = &result.stream_problem {
        out::warn(format!("{build} {kind}: {problem}"));
    }

    if result.outcome == Outcome::Ended && result.page_exit_code != 0 {
        out::warn(format!(
            "{build} {kind}: the page reported exit code {}",
            result.page_exit_code
        ));
    } else if result.outcome == Outcome::BrowserExited {
        out::warn(format!(
            "{build} {kind}: Chrome exited early; is another Chrome using the profile?"
        ));
    } else if result.outcome == Outcome::TimedOut {
        out::warn(format!("{build} {kind}: timed out"));
    }
}

/// Seconds with at most one decimal.
fn seconds(elapsed: Duration) -> String {
    let tenths = (elapsed.as_secs_f64() * 10.0).round() as u64;
    if tenths.is_multiple_of(10) {
        (tenths / 10).to_string()
    } else {
        format!("{}.{}", tenths / 10, tenths % 10)
    }
}

pub fn exit_code(result: &RunResult) -> i32 {
    match result.outcome {
        Outcome::Ended if result.page_exit_code == 0 && result.stream_problem.is_none() => 0,
        Outcome::Ended => 1,
        Outcome::TimedOut => 2,
        Outcome::BrowserExited => 3,
        Outcome::Cancelled => crate::failure::EXIT_INTERRUPTED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_build_name_is_a_plain_name_and_not_only_dots() {
        for name in ["change_build", "base-build", "a.b", "x1", "..a", ".hidden"] {
            assert_eq!(build_name(name).unwrap(), name);
        }
        for name in [
            "", ".", "..", "...", "a/b", "a\\b", "a b", "a:b", "..\\x", "é", "a\n",
        ] {
            assert!(
                matches!(build_name(name), Err(Failure::Usage(_))),
                "{name:?}"
            );
        }
    }

    #[test]
    fn seconds_have_at_most_one_decimal() {
        assert_eq!(seconds(Duration::from_millis(0)), "0");
        assert_eq!(seconds(Duration::from_millis(12_000)), "12");
        assert_eq!(seconds(Duration::from_millis(12_340)), "12.3");
        assert_eq!(seconds(Duration::from_millis(12_960)), "13");
        assert_eq!(seconds(Duration::from_millis(60)), "0.1");
    }

    fn scratch(name: &str) -> PathBuf {
        crate::scratch("commands", name)
    }

    #[test]
    fn the_parser_reads_a_csv_with_rows_and_nothing_else() {
        let dir = scratch("parser");
        let check = |text: &str| {
            let path = dir.join("a.csv");
            fs::write(&path, text).unwrap();
            parser_can_read(&path).unwrap()
        };
        let header = "\"Frame\",\"CPU Time\",\"GPU Time\"\r\n";
        assert!(check(&format!("{header}1,16.7,0\r\n2,16.9,0.5\r\n")));
        assert!(check(&format!("{header}1,16.7,0")));
        assert!(!check(header), "a header and no rows");
        assert!(!check(""));
        assert!(!check(&format!("{header}1,16.7\r\n")), "two fields");
        assert!(!check(&format!("{header}1,abc,0\r\n")), "no float");
        assert!(
            !check(&format!("{header}1,16.7,0\r\n\r\n2,1,1\r\n")),
            "a blank row"
        );
        assert!(
            !check(&format!("{header}1,16.7,0\r\ntruncated")),
            "a cut row"
        );
    }

    #[test]
    fn exit_codes() {
        let result = |outcome, page_exit_code, stream_problem: Option<&str>| RunResult {
            build: "b".into(),
            warmup: false,
            outcome,
            page_exit_code,
            elapsed: Duration::ZERO,
            csv: Default::default(),
            log: Default::default(),
            stream_problem: stream_problem.map(String::from),
        };
        assert_eq!(exit_code(&result(Outcome::Ended, 0, None)), 0);
        assert_eq!(exit_code(&result(Outcome::Ended, 101, None)), 1);
        assert_eq!(exit_code(&result(Outcome::Ended, 0, Some("short"))), 1);
        assert_eq!(exit_code(&result(Outcome::TimedOut, -1, None)), 2);
        assert_eq!(exit_code(&result(Outcome::BrowserExited, -1, None)), 3);
        assert_eq!(exit_code(&result(Outcome::Cancelled, -1, None)), 130);
    }
}
