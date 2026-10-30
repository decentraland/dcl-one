//! `benchmark report` on the fixtures in `tests/fixtures/report`: each case holds the result
//! artifacts of a run, as the leg jobs upload them. Every expected cell, note and headline is the
//! one the harness's `ProduceSummary.cs` prints for the same files; only the section titles (the
//! display names), the header and the note and headline for missing baseline runs are this
//! report's own.

mod support;

use std::path::PathBuf;
use std::process::Command;

use benchmark::report::{self, Options, Report};
use support::scratch;

fn fixture(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/report")
        .join(case)
}

fn options(case: &str) -> Options {
    Options {
        dir: fixture(case),
        title: "Benchmark".into(),
        pr: None,
        change: None,
        base: None,
        run_url: None,
        expected_runs: 3,
        expected_labels: Vec::new(),
    }
}

fn build(options: &Options) -> Report {
    report::build(options).unwrap_or_else(|failure| panic!("{failure}"))
}

fn report_of(case: &str) -> Report {
    build(&options(case))
}

/// The lines of the report, each checked to be there once.
fn has_line(report: &Report, line: &str) {
    let found = report.markdown.lines().filter(|l| *l == line).count();
    assert_eq!(found, 1, "{line}\n----- in -----\n{}", report.markdown);
}

/// The report's lines after the first one that starts with `title`, up to the next section.
fn section<'a>(report: &'a Report, title: &str) -> Vec<&'a str> {
    let heading = format!("### {title}");
    let lines: Vec<&str> = report.markdown.lines().collect();
    let start = lines
        .iter()
        .position(|l| *l == heading)
        .unwrap_or_else(|| panic!("no section {title}:\n{}", report.markdown));
    lines[start + 1..]
        .iter()
        .take_while(|l| !l.starts_with("### "))
        .copied()
        .collect()
}

#[test]
fn ok_two_sections_named_by_their_machine_and_chrome_the_change_faster_on_cpu() {
    let report = report_of("ok");
    assert!(report.has_data);
    has_line(
        &report,
        "**Overall: 🟢 CPU average improved on Apple M1 (Chrome), Intel Core i5 (Chrome)**",
    );

    let titles: Vec<&str> = report
        .markdown
        .lines()
        .filter(|l| l.starts_with("### "))
        .collect();
    assert_eq!(
        titles,
        ["### Apple M1 (Chrome)", "### Intel Core i5 (Chrome)"]
    );

    let apple = section(&report, "Apple M1 (Chrome)");
    for row in [
        "| Metric | Baseline | Change | Δ | Result |",
        "| Samples | 10 (×3) | 10 (×3) | | |",
        "| CPU average | 23.0 ms (22.9–23.1) | 21.0 ms (20.9–21.1) | -2.0 ms | 🟢 9% faster |",
        "| CPU 1% worst | 32.0 ms (31.4–32.6) | 30.0 ms (29.0–31.0) | -2.0 ms | 🟢 6% faster |",
        "| CPU 0.1% worst | 32.0 ms (31.4–32.6) | 30.0 ms (29.0–31.0) | -2.0 ms | — informational |",
        "| GPU average | 6.4 ms (6.3–6.5) | 6.4 ms (6.3–6.5) | 0.0 ms | ⚪ within noise |",
        "| GPU 1% worst | 10.0 ms (9.0–11.0) | 10.0 ms (9.0–11.0) | 0.0 ms | ⚪ within noise |",
        "| GPU 0.1% worst | 10.0 ms (9.0–11.0) | 10.0 ms (9.0–11.0) | 0.0 ms | — informational |",
        "| Exceptions per run | 0 | 0 | 0 | ⚪ no significant change |",
    ] {
        assert!(apple.contains(&row), "{row}\n{apple:#?}");
    }
    assert!(
        !apple.iter().any(|l| l.contains("Exception breakdown")),
        "no exception, no breakdown"
    );

    let intel = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| CPU average | 34.0 ms (33.8–34.4) | 31.0 ms (30.8–31.2) | -3.0 ms | 🟢 9% faster |",
        "| CPU 1% worst | 43.0 ms (41.0–47.0) | 40.0 ms (38.0–42.0) | -3.0 ms | ⚪ within noise |",
        "| GPU average | 8.3 ms (8.2–8.5) | 8.2 ms (8.1–8.4) | -0.1 ms | ⚪ within noise |",
        "| GPU 1% worst | 11.0 ms (10.0–13.0) | 10.0 ms (9.0–12.0) | -1.0 ms | ⚪ within noise |",
        "| Exceptions per run | 1 | 1 | 0 | ⚪ no significant change |",
        "<details><summary>Exception breakdown</summary>",
        "| Exception | Baseline (3 runs) | Change (3 runs) |",
        "| `[comms] Error` | 3 | 3 |",
    ] {
        assert!(intel.contains(&row), "{row}\n{intel:#?}");
    }
}

#[test]
fn ok_the_whole_report_with_its_header() {
    let mut options = options("ok");
    options.title = "Benchmark of #12".into();
    options.pr = Some(12);
    options.change = Some("`abc1234`".into());
    options.base = Some("`main` at `def5678`".into());
    options.run_url = Some("https://github.com/example/repo/actions/runs/7".into());
    let report = build(&options);

    let expected = std::fs::read_to_string(fixture("ok.md")).unwrap();
    assert_eq!(report.markdown, expected.replace("\r\n", "\n"));
}

#[test]
fn crash_a_run_without_a_csv_is_missing_and_its_log_is_not_read() {
    let report = report_of("crash");
    assert!(report.has_data);
    has_line(
        &report,
        "**Overall: 🔴 CPU average regressed on Intel Core i5 (Chrome); CPU 1% worst regressed on Intel Core i5 (Chrome)**",
    );
    let lines = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| Samples | 10 (×2) | 10 (×3) | | |",
        "| CPU average | 32.9 ms (32.8–33.0) | 37.0 ms (36.8–37.2) | 4.1 ms | 🔴 12% slower |",
        "| CPU 1% worst | 41.0 ms (40.0–42.0) | 46.0 ms (44.0–48.0) | 5.0 ms | 🔴 12% slower |",
        // the median of two runs is their mean: 8.25, which rounds to even
        "| GPU average | 8.2 ms (8.2–8.3) | 8.3 ms (8.2–8.4) | 0.1 ms | ⚪ within noise |",
        "| GPU 1% worst | 10.5 ms (10.0–11.0) | 11.0 ms (10.0–12.0) | 0.5 ms | ⚪ within noise |",
        "| Exceptions per run | 0 | 0 | 0 | ⚪ no significant change |",
        "> ⚠️ Only 2 of 3 Baseline runs produced data — the app may have crashed in the missing runs.",
    ] {
        assert!(lines.contains(&row), "{row}\n{lines:#?}");
    }
    assert!(
        !report.markdown.contains("PanicException"),
        "the crashed run's log is not counted"
    );
}

#[test]
fn partial_a_set_aside_csv_counts_as_a_crashed_run() {
    let report = report_of("partial");
    has_line(&report, "**Overall: ✅ no significant changes**");
    let lines = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| Samples | 10 (×3) | 10 (×2) | | |",
        "| CPU average | 34.1 ms (33.9–34.3) | 34.1 ms (34.0–34.2) | 0.0 ms | ⚪ within noise |",
        "| GPU 1% worst | 11.0 ms (10.0–12.0) | 10.5 ms (10.0–11.0) | -0.5 ms | ⚪ within noise |",
        "> ⚠️ Only 2 of 3 Change runs produced data — the app may have crashed in the missing runs.",
    ] {
        assert!(lines.contains(&row), "{row}\n{lines:#?}");
    }
}

#[test]
fn header_only_a_csv_without_rows_loses_its_section_with_a_note() {
    let report = report_of("header-only");
    assert!(!report.has_data);
    has_line(
        &report,
        "> ⚠️ Could not gather the results of a runner: change_build_2.csv contains zero samples",
    );
    assert!(
        !report.markdown.contains("**Overall:"),
        "no data, no headline"
    );
    assert!(!report.markdown.contains("### "));
}

#[test]
fn zero_gpu_an_all_zero_gpu_column_gets_no_gpu_verdict() {
    let report = report_of("zero-gpu");
    // 3% faster on one machine: a verdict, but not a headline
    has_line(&report, "**Overall: ✅ no significant changes**");
    let lines = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| CPU average | 32.0 ms (31.8–32.2) | 31.0 ms (30.8–31.2) | -1.0 ms | 🟢 3% faster |",
        "| GPU average | 8.3 ms (8.2–8.5) | 0.0 ms | -8.3 ms | ⚠️ no verdict — GPU timing not working |",
        "| GPU 1% worst | 11.0 ms (10.0–13.0) | 0.0 ms | -11.0 ms | ⚠️ no verdict — GPU timing not working |",
        "| GPU 0.1% worst | 11.0 ms (10.0–13.0) | 0.0 ms | -11.0 ms | — informational |",
    ] {
        assert!(lines.contains(&row), "{row}\n{lines:#?}");
    }
}

#[test]
fn outlier_a_run_with_40_exceptions_is_excluded_from_every_number() {
    let report = report_of("outlier");
    has_line(&report, "**Overall: ✅ no significant changes**");
    let lines = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| Samples | 10 (×3) | 10 (×2) | | |",
        "| CPU average | 34.1 ms (33.9–34.3) | 34.1 ms (34.0–34.2) | 0.0 ms | ⚪ within noise |",
        "| Exceptions per run | 2 | 2.5 | +0.5 | ⚪ no significant change |",
        "> ⚠️ One Change run was excluded as untrustworthy: it logged 40 exceptions, while the other runs logged 2, 3.",
        "> ⚠️ Exception kinds not seen in the baseline: `[asset] PanicException` (×2).",
        "| Exception | Baseline (3 runs) | Change (2 runs) |",
        "| `[scene] Error` | 6 | 3 |",
        "| `[asset] PanicException` | 0 | 2 |",
    ] {
        assert!(lines.contains(&row), "{row}\n{lines:#?}");
    }
    assert!(
        !report.markdown.contains("[comms]"),
        "the excluded run's exceptions are not in the breakdown"
    );
}

#[test]
fn teardown_exceptions_after_the_exit_marker_are_not_counted() {
    let report = report_of("teardown");
    let lines = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| Exceptions per run | 1 | 1 | 0 | ⚪ no significant change |",
        "| `[comms] Error` | 3 | 3 |",
    ] {
        assert!(lines.contains(&row), "{row}\n{lines:#?}");
    }
    assert!(!report.markdown.contains("scene_runner"));
}

#[test]
fn missing_label_an_expected_label_without_a_section_gets_a_line() {
    let mut options = options("missing-label");
    options.expected_labels = vec!["apple-m1-chrome".into(), "intel-i5-chrome".into()];
    let report = build(&options);
    assert!(report.has_data);
    has_line(
        &report,
        "> ⚠️ Apple M1 (Chrome) produced no results (its job failed, timed out or was skipped)",
    );
    assert!(!report.markdown.contains("Intel Core i5 (Chrome) produced"));
    // its one section has no baseline runs, so nothing was compared
    has_line(
        &report,
        "**Overall: ⚠️ nothing was compared — no Baseline run produced data**",
    );
    assert!(!report.markdown.contains("no significant changes"));

    // the change build alone: one column, no verdicts
    let lines = section(&report, "Intel Core i5 (Chrome)");
    for row in [
        "| Metric | |",
        "| Samples | 10 (×3) |",
        "| CPU average | 31.0 ms (30.8–31.2) |",
        "| GPU average | 8.2 ms (8.1–8.4) |",
        "| Exceptions per run | 0.33 |",
        "> ⚠️ No Baseline run produced data — the change is shown alone, with no verdicts.",
        "| Exception | Count |",
        "| `[comms] Error` | 1 |",
    ] {
        assert!(lines.contains(&row), "{row}\n{lines:#?}");
    }
}

#[test]
fn no_baseline_on_one_laptop_the_headline_comes_from_the_others() {
    let dir = scratch("report-one-without-baseline");
    for (case, folder) in [
        ("ok", "benchmark-result-apple-m1-chrome"),
        ("missing-label", "benchmark-result-intel-i5-chrome"),
    ] {
        let to = dir.join(folder);
        std::fs::create_dir_all(&to).unwrap();
        for entry in std::fs::read_dir(fixture(case).join(folder)).unwrap() {
            let from = entry.unwrap().path();
            std::fs::copy(&from, to.join(from.file_name().unwrap())).unwrap();
        }
    }
    let mut options = options("ok");
    options.dir = dir;
    let report = build(&options);
    assert!(report.has_data);
    // Apple M1's 9% and 6% on one laptop name nothing
    has_line(&report, "**Overall: ✅ no significant changes**");
    assert!(!report.markdown.contains("nothing was compared"));

    let note = "> ⚠️ No Baseline run produced data — the change is shown alone, with no verdicts.";
    assert!(section(&report, "Intel Core i5 (Chrome)").contains(&note));
    assert!(!section(&report, "Apple M1 (Chrome)").contains(&note));
}

#[test]
fn no_folder_no_section_and_every_expected_label_missing() {
    let mut options = options("no-such-case");
    options.expected_labels = vec!["intel-i5-chrome".into()];
    let report = build(&options);
    assert!(!report.has_data);
    has_line(
        &report,
        "> ⚠️ Intel Core i5 (Chrome) produced no results (its job failed, timed out or was skipped)",
    );
}

fn command(arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_benchmark"));
    command.arg("report").args(arguments);
    for variable in [
        "GITHUB_STEP_SUMMARY",
        "RUN_NAME",
        "PR_NUMBER",
        "CHANGE_DESCRIPTION",
        "BASE_DESCRIPTION",
        "RUN_URL",
        "EXPECTED_RUNS",
        "EXPECTED_LABELS",
    ] {
        command.env_remove(variable);
    }
    command
}

#[test]
fn the_command_writes_the_file_from_the_environment_and_exits_0_with_data() {
    let out = scratch("report-ok").join("summary.md");
    let output = command(&[fixture("ok").to_str().unwrap()])
        .env("GITHUB_STEP_SUMMARY", &out)
        .env("RUN_NAME", "Benchmark of #12")
        .env("PR_NUMBER", "12")
        .env("CHANGE_DESCRIPTION", "`abc1234`")
        .env("BASE_DESCRIPTION", "`main` at `def5678`")
        .env("RUN_URL", "https://github.com/example/repo/actions/runs/7")
        .env("EXPECTED_LABELS", "apple-m1-chrome, intel-i5-chrome")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("::warning::"));

    let written = std::fs::read_to_string(&out).unwrap();
    let expected = std::fs::read_to_string(fixture("ok.md")).unwrap();
    assert_eq!(written, expected.replace("\r\n", "\n"));
}

#[test]
fn the_command_writes_the_file_anyway_and_exits_2_without_data() {
    let out = scratch("report-header-only").join("report.md");
    let output = command(&[
        fixture("header-only").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--expected-runs",
        "3",
    ])
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(report::EXIT_NO_DATA));
    let log = String::from_utf8_lossy(&output.stdout);
    assert!(
        log.contains("::warning::[benchmark] could not gather the results of a runner: change_build_2.csv contains zero samples"),
        "{log}"
    );
    assert!(
        log.contains("::warning::[benchmark] no section has data"),
        "{log}"
    );

    let written = std::fs::read_to_string(&out).unwrap();
    assert!(written.starts_with("## Benchmark\n"), "{written}");
    assert!(written.contains("contains zero samples"));
}

#[test]
fn the_command_refuses_a_pr_that_is_not_a_number() {
    let out = scratch("report-bad-pr").join("report.md");
    let output = command(&[
        fixture("ok").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--pr",
        "12; rm",
    ])
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--pr must be a whole number"));
    assert!(!out.exists());
}

/// The two markers the log parser counts by are the ones the explorer writes
/// (`perf_output::format`, which takes the exit marker from `common::autopilot`). `benchmark`
/// cannot depend on `perf_output` without building the engine, so the sources are compared.
#[test]
fn the_log_markers_are_the_explorers() {
    let crates = PathBuf::from(env!("DCL_ONE_ROOT")).join("rendering/bevy-explorer/source/crates");
    let constant = |file: &str, name: &str| {
        let source = std::fs::read_to_string(crates.join(file)).unwrap();
        let line = source
            .lines()
            .find(|l| l.starts_with(&format!("pub const {name}: &str = \"")))
            .unwrap_or_else(|| panic!("{name} in {file}"));
        line.split('"').nth(1).unwrap().to_string()
    };
    assert_eq!(
        report::EXIT_MARKER,
        constant("common/src/autopilot.rs", "EXIT_MARKER")
    );
    assert_eq!(
        report::EXCEPTION_MARKER,
        constant("perf_output/src/format.rs", "EXCEPTION_MARKER")
    );
}
