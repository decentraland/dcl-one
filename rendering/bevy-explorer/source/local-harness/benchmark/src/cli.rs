//! The command line. Every option but four has an environment variable, so a workflow step needs
//! no shell-specific syntax (PowerShell on Windows, bash on macOS): `--keep-service-worker`,
//! `leg`'s `--change` and `--base` and `fetch`'s `--source` have none. An empty variable is not
//! set.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use crate::failure::Failure;
use crate::out;

/// One origin for every run on a laptop: the stored guest lives in this origin's storage inside
/// the Chrome profile, so the port must not change between runs, builds or jobs.
pub const DEFAULT_PORT: u16 = 18742;

pub const CHANGE_BUILD: &str = "change_build";
pub const BASE_BUILD: &str = "base_build";

/// The measured run: arriving in world is bounded by 300 s and the measuring window is 90 s,
/// plus boot.
const DEFAULT_TIMEOUT_SECONDS: u64 = 480;
/// A warm-up ends on `end` as a run does; its timeout (arriving in world is bounded by 300 s)
/// only backstops a page that never sends it.
const DEFAULT_WARMUP_TIMEOUT_SECONDS: u64 = 300;

#[derive(Parser)]
#[command(
    name = "benchmark",
    about = "Runs the web explorer's benchmark in Chrome and writes its report",
    long_about = "Runs the web explorer in Chrome on a test machine and stores the CSV and log the \
page streams back, as the desktop client writes them with --csv and --log-file; writes the report \
of the results. See rendering/bevy-explorer/docs/guidelines/benchmark.md."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// One browser run: serve a web app from localhost, open Chrome on it, store the streamed
    /// bytes as <build>.csv / <build>.log, close Chrome on `end` or on the timeout
    Run(RunArgs),
    /// `run` for a warm-up (no CSV is measured), exit 0 whatever the run's outcome
    Warmup(WarmupArgs),
    /// Download (URL) or unzip (zip) a web app into a folder
    Fetch(FetchArgs),
    /// The interleaved loop: change, base, change, base ... renamed to change_build_<i>.csv/.log
    /// and base_build_<i>.csv/.log, then runs_on.txt
    Measure(MeasureArgs),
    /// Everything for a by-hand check: fetch both builds, warm both up, measure, write runs_on.txt
    Leg(LegArgs),
    /// The Markdown report of the results under DIR: a section for each folder holding a
    /// runs_on.txt. Exits 2 when no section has data, after writing the report
    Report(ReportArgs),
}

/// The machine-level settings every run shares.
#[derive(Args)]
pub struct MachineArgs {
    /// Folder for the run files (the working directory by default)
    #[arg(long, env = "OUT_DIR")]
    out: Option<String>,
    /// The port on loopback; fixed, because the Chrome profile keeps the guest per origin
    #[arg(long, env = "PERF_PORT")]
    port: Option<String>,
    /// The Chrome profile, kept between runs
    #[arg(long, env = "PERF_CHROME_PROFILE")]
    profile: Option<String>,
    /// Chrome's executable
    #[arg(long, env = "CHROME_PATH")]
    chrome: Option<String>,
    /// The realm the run enters
    #[arg(long, env = "REALM")]
    realm: Option<String>,
    /// Extra page parameters, for example guest=1
    #[arg(long, env = "EXTRA_QUERY")]
    query: Option<String>,
    /// The Chrome window size instead of maximized, for example 1280x720
    #[arg(long, env = "PERF_WINDOW")]
    window: Option<String>,
    /// A flag for Chrome; repeat it for several (CHROME_EXTRA_ARGS holds them space-separated)
    #[arg(long = "chrome-arg")]
    chrome_arg: Vec<String>,
    /// Serve the app and print the address; open it in a browser by hand
    #[arg(long = "no-browser", env = "NO_BROWSER", num_args = 0..=1, default_missing_value = "true")]
    no_browser: Option<String>,
    /// Skip the profile scrub and the reset page (by hand only)
    #[arg(long = "keep-service-worker", num_args = 0..=1, default_missing_value = "true")]
    keep_service_worker: Option<String>,
    /// Seconds a measured run may take
    #[arg(long, env = "RUN_TIMEOUT_SECONDS")]
    timeout: Option<String>,
    /// Seconds a warm-up may take (a backstop)
    #[arg(long = "warmup-timeout", env = "WARMUP_TIMEOUT_SECONDS")]
    warmup_timeout: Option<String>,
}

#[derive(Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub machine: MachineArgs,
    /// The web app: a folder holding index.html, a .zip or the URL of one
    #[arg(long, env = "APP_DIR")]
    pub app: Option<String>,
    /// Names the files: <build>.csv, <build>.log
    #[arg(long, env = "BUILD_NAME")]
    pub build: Option<String>,
    /// A warm-up: the page streams the log only and the engine does not measure
    #[arg(long, env = "WARMUP", num_args = 0..=1, default_missing_value = "true")]
    pub warmup: Option<String>,
}

#[derive(Args)]
pub struct WarmupArgs {
    #[command(flatten)]
    pub machine: MachineArgs,
    /// The web app: a folder holding index.html, a .zip or the URL of one
    #[arg(long, env = "APP_DIR")]
    pub app: Option<String>,
    /// Names the files: <build>.log
    #[arg(long, env = "BUILD_NAME")]
    pub build: Option<String>,
}

#[derive(Args)]
pub struct FetchArgs {
    /// The web app: a folder or a .zip (outside Actions also the URL of a .zip)
    #[arg(long, env = "BUILD_URL")]
    pub url: Option<String>,
    /// As --url
    #[arg(long)]
    pub source: Option<String>,
    /// The folder the app is unpacked into; it is emptied first
    #[arg(long, env = "BUILD_DIR")]
    pub dir: Option<String>,
}

#[derive(Args)]
pub struct MeasureArgs {
    #[command(flatten)]
    pub machine: MachineArgs,
    /// The change build's web app folder
    #[arg(long = "change-dir", env = "CHANGE_BUILD_DIR")]
    pub change_dir: Option<String>,
    /// The baseline build's web app folder
    #[arg(long = "base-dir", env = "BASE_BUILD_DIR")]
    pub base_dir: Option<String>,
    /// The runner label that names the report section (runs_on.txt)
    #[arg(long, env = "RUNS_ON_LABEL")]
    pub label: Option<String>,
    /// Measured runs of each build
    #[arg(long, env = "RUNS")]
    pub runs: Option<String>,
    /// Seconds to wait after each launch
    #[arg(long, env = "SETTLE_SECONDS")]
    pub settle: Option<String>,
}

#[derive(Args)]
pub struct LegArgs {
    #[command(flatten)]
    pub machine: MachineArgs,
    /// The change build's web app: a folder, a .zip or the URL of one
    #[arg(long)]
    pub change: Option<String>,
    /// The baseline build's web app
    #[arg(long = "base")]
    pub baseline: Option<String>,
    /// The runner label that names the report section
    #[arg(long, env = "RUNS_ON_LABEL")]
    pub label: Option<String>,
    /// Measured runs of each build
    #[arg(long, env = "RUNS")]
    pub runs: Option<String>,
    /// Seconds to wait after each launch
    #[arg(long, env = "SETTLE_SECONDS")]
    pub settle: Option<String>,
}

#[derive(Args)]
pub struct ReportArgs {
    /// The folder the results are under (the working directory by default)
    pub dir: Option<String>,
    /// The Markdown file to write (summary.md by default)
    #[arg(long, env = "GITHUB_STEP_SUMMARY")]
    pub out: Option<String>,
    /// The report's title (Benchmark by default)
    #[arg(long, env = "RUN_NAME")]
    pub title: Option<String>,
    /// The pull request the change build is from
    #[arg(long, env = "PR_NUMBER")]
    pub pr: Option<String>,
    /// The change build, as the header names it
    #[arg(long, env = "CHANGE_DESCRIPTION")]
    pub change: Option<String>,
    /// The baseline build, as the header names it
    #[arg(long = "base", env = "BASE_DESCRIPTION")]
    pub baseline: Option<String>,
    /// The workflow run that measured
    #[arg(long = "run-url", env = "RUN_URL")]
    pub run_url: Option<String>,
    /// Measured runs of each build; fewer CSVs mean the missing runs crashed
    #[arg(long = "expected-runs", env = "EXPECTED_RUNS")]
    pub expected_runs: Option<String>,
    /// The labels that should report, comma-separated; one with no results gets a line
    #[arg(long = "expected-labels", env = "EXPECTED_LABELS")]
    pub expected_labels: Option<String>,
}

/// The option's text when it was given and is not empty.
pub fn given(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|text| !text.is_empty())
}

/// `true`, `1` or `yes` in any case; anything else, or nothing, is off.
pub fn flag(value: &Option<String>) -> bool {
    given(value).is_some_and(|text| {
        text.eq_ignore_ascii_case("true") || text == "1" || text.eq_ignore_ascii_case("yes")
    })
}

/// A whole number of digits only.
pub fn whole_number(option: &str, value: &Option<String>, fallback: u64) -> Result<u64, Failure> {
    let Some(text) = given(value) else {
        return Ok(fallback);
    };
    if text.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(number) = text.parse::<u32>() {
            return Ok(u64::from(number));
        }
    }
    Err(Failure::usage(format!(
        "--{option} must be a whole number, got '{}'",
        out::safe(text)
    )))
}

/// The machine-level settings every run shares.
#[derive(Debug, Clone)]
pub struct Settings {
    pub out_dir: PathBuf,
    pub port: u16,
    pub profile: PathBuf,
    pub chrome_path: Option<String>,
    pub realm: String,
    pub query: Option<String>,
    pub window: Option<String>,
    pub chrome_args: Vec<String>,
    pub no_browser: bool,
    pub keep_service_worker: bool,
    pub timeout: Duration,
    pub warmup_timeout: Duration,
}

impl Settings {
    pub fn from(args: &MachineArgs) -> Result<Self, Failure> {
        let profile = match given(&args.profile) {
            Some(profile) => PathBuf::from(profile),
            None => default_profile()?,
        };

        let mut chrome_args = args.chrome_arg.clone();
        chrome_args.extend(
            std::env::var("CHROME_EXTRA_ARGS")
                .unwrap_or_default()
                .split(' ')
                .filter(|arg| !arg.is_empty())
                .map(String::from),
        );

        let port = whole_number("port", &args.port, u64::from(DEFAULT_PORT))?;
        let timeout = whole_number("timeout", &args.timeout, 0)?;
        let warmup_timeout = whole_number("warmup-timeout", &args.warmup_timeout, 0)?;

        Ok(Self {
            out_dir: std::path::absolute(given(&args.out).unwrap_or("."))?,
            port: u16::try_from(port).map_err(|_| {
                Failure::usage(format!("--port must be at most 65535, got '{port}'"))
            })?,
            profile: std::path::absolute(profile)?,
            chrome_path: given(&args.chrome).map(String::from),
            realm: given(&args.realm)
                .unwrap_or("supportteam.dcl.eth")
                .to_string(),
            query: given(&args.query).map(String::from),
            window: given(&args.window).map(String::from),
            chrome_args,
            no_browser: flag(&args.no_browser),
            keep_service_worker: flag(&args.keep_service_worker),
            timeout: Duration::from_secs(if timeout > 0 {
                timeout
            } else {
                DEFAULT_TIMEOUT_SECONDS
            }),
            warmup_timeout: Duration::from_secs(match (warmup_timeout, timeout) {
                (0, 0) => DEFAULT_WARMUP_TIMEOUT_SECONDS,
                (0, run) => run,
                (warmup, _) => warmup,
            }),
        })
    }
}

/// `decentraland-benchmark/chrome-profile` under the user's local application data
/// folder.
fn default_profile() -> Result<PathBuf, Failure> {
    let dirs = directories::BaseDirs::new()
        .ok_or_else(|| Failure::cannot_run("this machine has no home folder; give --profile"))?;
    Ok(dirs
        .data_local_dir()
        .join("decentraland-benchmark")
        .join("chrome-profile"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("benchmark").chain(arguments.iter().copied()))
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn machine(arguments: &[&str]) -> MachineArgs {
        match parse(&[&["warmup"], arguments].concat()).command {
            Command::Warmup(args) => args.machine,
            _ => unreachable!(),
        }
    }

    #[test]
    fn a_bare_flag_is_on_and_a_value_decides() {
        assert!(flag(&machine(&["--no-browser"]).no_browser));
        assert!(flag(&machine(&["--no-browser", "yes"]).no_browser));
        assert!(!flag(&machine(&["--no-browser=false"]).no_browser));
        assert!(!flag(&machine(&[]).no_browser));
        assert!(!flag(&Some(String::new())));
        assert!(flag(&Some("TRUE".into())) && flag(&Some("1".into())));
    }

    #[test]
    fn options_take_a_value_either_way_and_chrome_args_repeat() {
        let args = machine(&[
            "--out",
            "a",
            "--port=1234",
            "--chrome-arg=--one",
            "--chrome-arg",
            "two",
        ]);
        assert_eq!(args.out.as_deref(), Some("a"));
        assert_eq!(args.port.as_deref(), Some("1234"));
        assert_eq!(args.chrome_arg, ["--one", "two"]);
    }

    #[test]
    fn numbers_are_digits_only_and_empty_is_unset() {
        assert_eq!(whole_number("runs", &None, 3).unwrap(), 3);
        assert_eq!(whole_number("runs", &Some(String::new()), 3).unwrap(), 3);
        assert_eq!(whole_number("runs", &Some("12".into()), 3).unwrap(), 12);
        for bad in ["-1", "+1", "1.5", "x", " 1", "99999999999"] {
            assert!(
                matches!(
                    whole_number("runs", &Some(bad.into()), 3),
                    Err(Failure::Usage(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_timeouts_default_as_the_workflow_expects() {
        let settings = |timeout: Option<&str>, warmup: Option<&str>| {
            let mut args = machine(&[]);
            args.timeout = timeout.map(String::from);
            args.warmup_timeout = warmup.map(String::from);
            Settings::from(&args).unwrap()
        };
        let s = settings(None, None);
        assert_eq!(
            (s.timeout.as_secs(), s.warmup_timeout.as_secs()),
            (480, 300)
        );
        let s = settings(Some("100"), None);
        assert_eq!(
            (s.timeout.as_secs(), s.warmup_timeout.as_secs()),
            (100, 100)
        );
        let s = settings(Some("0"), Some("7"));
        assert_eq!((s.timeout.as_secs(), s.warmup_timeout.as_secs()), (480, 7));
    }

    #[test]
    fn the_profile_defaults_under_the_local_application_data_folder() {
        let s = Settings::from(&machine(&[])).unwrap();
        assert!(s.profile.ends_with("decentraland-benchmark/chrome-profile"));
        assert_eq!(s.port, DEFAULT_PORT);
        assert_eq!(s.realm, "supportteam.dcl.eth");
    }

    #[test]
    fn a_port_that_is_no_port_is_refused() {
        let mut args = machine(&[]);
        args.port = Some("70000".into());
        assert!(matches!(Settings::from(&args), Err(Failure::Usage(_))));
    }
}
