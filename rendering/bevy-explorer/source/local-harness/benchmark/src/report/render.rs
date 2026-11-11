//! The report's Markdown: the header, the headline, the legend, a section per runner and a line
//! for each runner without results. The headline, the legend, the tables and the notes are
//! `ProduceSummary.cs`'s words, with every number formatted as .NET formats it, but for the note
//! and headline for missing baseline runs.

use std::fmt::Write;

use super::exceptions::ExceptionKindCounts;
use super::stats::{
    self, FrameTimeVerdict, IncidentsPerRunVerdict, MedianAndRange, PeakMemoryVerdict,
    SessionsNotComparable, CPU_HICCUPS_METRIC, EXCEPTIONS_METRIC, FRAME_TIME_METRICS,
    PEAK_MEMORY_METRIC,
};
use super::{BuildBenchmarkRunsStatistics, ReportOptions, RunnerCaveat, RunnerSection};

/// The fleet's runner labels and the machines they name (`ProduceSummary.cs`'s
/// `labelDisplayNames`).
const MACHINES: [(&str, &str); 5] = [
    ("apple-m1", "Apple M1"),
    ("amd-ryzen-3", "AMD Ryzen 3"),
    ("intel-i3", "Intel Core i3"),
    ("intel-i5", "Intel Core i5"),
    ("framework-13-i7", "Framework 13 i7"),
];

/// A leg in Chrome on a machine is labelled `<runner label>-chrome`.
const CHROME: &str = "-chrome";

/// `intel-i5-chrome` is "Intel Core i5 (Chrome)": the parentheses keep the headline's ", " list
/// readable. Any other label is shown as it is.
pub fn display_name(label: &str) -> String {
    let machine = label.strip_suffix(CHROME).and_then(|runner| {
        MACHINES
            .iter()
            .find(|(known, _)| *known == runner)
            .map(|(_, name)| *name)
    });
    match machine {
        Some(name) => format!("{name} (Chrome)"),
        None => label.to_string(),
    }
}

/// The title, and what was compared: the PR, the two builds and the run.
pub fn header(options: &ReportOptions) -> String {
    let mut out = format!("## {}\n\n", options.title);
    let mut parts = Vec::new();
    if let Some(pr) = options.pr {
        parts.push(format!("PR #{pr}"));
    }
    if let Some(change) = &options.change {
        parts.push(format!("change: {change}"));
    }
    if let Some(base) = &options.base {
        parts.push(format!("base: {base}"));
    }
    if let Some(url) = &options.run_url {
        parts.push(format!("[run]({url})"));
    }
    if !parts.is_empty() {
        let _ = write!(out, "{}\n\n", parts.join(" · "));
    }
    out
}

/// The **Overall** line: the regressions when there are any, else the improvements.
pub fn headline(regressions: &[(&str, Vec<&str>)], improvements: &[(&str, Vec<&str>)]) -> String {
    let list = |groups: &[(&str, Vec<&str>)], moved: &str| {
        groups
            .iter()
            .map(|(metric, runners)| format!("{metric} {moved} on {}", runners.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    };
    if !regressions.is_empty() {
        format!("**Overall: 🔴 {}**\n\n", list(regressions, "regressed"))
    } else if !improvements.is_empty() {
        format!("**Overall: 🟢 {}**\n\n", list(improvements, "improved"))
    } else {
        "**Overall: ✅ no significant changes**\n\n".to_string()
    }
}

/// The **Overall** line when sections have data but none had baseline runs to compare with.
pub const NOTHING_COMPARED: &str =
    "**Overall: ⚠️ nothing was compared — no Baseline run produced data**\n\n";

/// How to read the tables, for someone with no context on the method.
pub fn legend(expected_runs: u64) -> String {
    format!(
        "<details><summary>How to read this table</summary>\n\
         \n\
         - Each build is measured **{expected_runs} times**, interleaved with the other build (change, baseline, change, baseline, ...) in the same session, so both see the same world content and machine state. The values are the **median**, and **(min–max)** is the lowest and highest of those runs.\n\
         - **Δ** is Change minus Baseline (a negative Δ means Change is faster).\n\
         - **🟢 faster / 🔴 slower** — a difference that passed every check: the runs are fully separated (every run of one build faster than every run of the other), and the median difference is at least **{lambda:.0}%** and at least **{min_ms} ms**.\n\
         - **⚪ within noise** — the builds' runs overlap, or the difference is tiny; it cannot be told apart from random variation. Treat it as no change.\n\
         - **— informational** — the 0.1% worst metrics average only the few worst frames of a run, so a single OS hiccup swings them by a lot; they are shown for context and never earn a verdict.\n\
         - **⚠️ no verdict** — the two builds' sessions were not comparable (very different sample counts, or too few usable runs), so no conclusion is drawn from them.\n\
         - **Exceptions per run** — the average number of exceptions in a run's log, not counting teardown ones logged while the app quits. Flagged only on a difference of at least {delta_min} per run and {ratio_min}× the other build; exception kinds the baseline never threw are called out under the table. The *Exception breakdown* groups all of them by the explorer's report category and exception type (as totals across the runs).\n\
         - **Hiccups per run / Peak memory** — from the `perf_summary` lines an explorer writes when its measured phase ends: frames over its hiccup threshold (the larger of 50 ms and two target frame times) and the peak engine memory. Hiccups follow the exceptions rule; memory the frame-time rule with a floor of {min_mib} MiB. The *Top scopes* list is the self time per run of the scopes the explorer's profiler recorded. Shown only when a run wrote them.\n\
         - A run that logged **unusually many exceptions** (at least {outlier_min} and {outlier_factor}× the median of its build's runs — e.g. a service was down during it) is excluded from all numbers and called out under the table.\n\
         - The **Overall** line at the top only reacts to a metric that moved on **two or more machines**, or by **10% or more** on one — a single modest 🟢/🔴 cell can still be a statistical fluke.\n\
         </details>\n\
         \n",
        lambda = stats::MIN_RELATIVE_MEDIAN_DELTA * 100.0,
        min_ms = stats::MIN_ABSOLUTE_FRAME_TIME_DELTA_MS,
        delta_min = stats::MIN_INCIDENTS_PER_RUN_DELTA,
        ratio_min = stats::MIN_INCIDENTS_PER_RUN_RATIO,
        outlier_min = stats::OUTLIER_MIN_EXCEPTIONS,
        outlier_factor = stats::OUTLIER_FACTOR,
        min_mib = stats::MIN_ABSOLUTE_PEAK_MEMORY_DELTA_MIB,
    )
}

/// A runner's table, its notes and its exception breakdown.
pub fn runner_section(section: &RunnerSection) -> String {
    let change = &section.change;
    let mut out = format!("### {}\n\n", section.title);

    match &section.baseline_comparison {
        Some(comparison) => {
            let base = &comparison.base;
            out += "| Metric | Baseline | Change | Δ | Result |\n";
            out += "| --- | ---: | ---: | ---: | :--- |\n";
            let _ = writeln!(
                out,
                "| Samples | {} | {} | | |",
                samples(base),
                samples(change)
            );
            for (m, name) in FRAME_TIME_METRICS.iter().enumerate() {
                let delta = change.frame_time_ms[m].median - base.frame_time_ms[m].median;
                let _ = writeln!(
                    out,
                    "| {name} | {} | {} | {delta:.1} ms | {} |",
                    frame_time_cell(&base.frame_time_ms[m]),
                    frame_time_cell(&change.frame_time_ms[m]),
                    frame_time_verdict_text(comparison.frame_time_verdicts[m])
                );
            }
            let _ = writeln!(
                out,
                "| {EXCEPTIONS_METRIC} | {} | {} | {} | {} |",
                hundredths(base.exceptions_per_run),
                hundredths(change.exceptions_per_run),
                signed_hundredths(change.exceptions_per_run - base.exceptions_per_run),
                exceptions_verdict_text(comparison.exceptions_per_run_verdict)
            );
            if change.perf_summary_run_count > 0 || base.perf_summary_run_count > 0 {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    CPU_HICCUPS_METRIC,
                    cpu_hiccups_cell(base),
                    cpu_hiccups_cell(change),
                    if change.perf_summary_run_count > 0 && base.perf_summary_run_count > 0 {
                        signed_hundredths(f64::from(
                            change.cpu_hiccups_per_run.median - base.cpu_hiccups_per_run.median,
                        ))
                    } else {
                        "-".to_owned()
                    },
                    hiccups_verdict_text(comparison.cpu_hiccups_per_run_verdict)
                );
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    PEAK_MEMORY_METRIC,
                    peak_memory_cell(base),
                    peak_memory_cell(change),
                    if change.perf_summary_run_count > 0 && base.perf_summary_run_count > 0 {
                        signed_mib(change.peak_memory_mib.median - base.peak_memory_mib.median)
                    } else {
                        "-".to_owned()
                    },
                    peak_memory_verdict_text(comparison.peak_memory_verdict)
                );
            }
        }
        None => {
            out += "| Metric | |\n";
            out += "| --- | ---: |\n";
            let _ = writeln!(out, "| Samples | {} |", samples(change));
            for (m, name) in FRAME_TIME_METRICS.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "| {name} | {} |",
                    frame_time_cell(&change.frame_time_ms[m])
                );
            }
            let _ = writeln!(
                out,
                "| {EXCEPTIONS_METRIC} | {} |",
                hundredths(change.exceptions_per_run)
            );
            if change.perf_summary_run_count > 0 {
                let _ = writeln!(
                    out,
                    "| {CPU_HICCUPS_METRIC} | {} |",
                    cpu_hiccups_cell(change)
                );
                let _ = writeln!(
                    out,
                    "| {PEAK_MEMORY_METRIC} | {} |",
                    peak_memory_cell(change)
                );
            }
        }
    }

    let base_scopes = section
        .baseline_comparison
        .as_ref()
        .map(|comparison| comparison.base.scope_self_ms_per_run.as_slice())
        .unwrap_or_default();
    if !change.scope_self_ms_per_run.is_empty() || !base_scopes.is_empty() {
        out += "\n<details><summary>Top scopes (self time per run, ms)</summary>\n\n";
        // the change's scopes first, by its self time; then the baseline's it lacks
        let mut names: Vec<&str> = change
            .scope_self_ms_per_run
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        for (name, _) in base_scopes {
            if !names.contains(&name.as_str()) {
                names.push(name);
            }
        }
        names.truncate(TOP_SCOPES_SHOWN);
        let self_ms = |scopes: &[(String, f32)], name: &str| {
            scopes
                .iter()
                .find(|(known, _)| known == name)
                .map_or("-".to_owned(), |(_, ms)| format!("{ms:.1}"))
        };
        match &section.baseline_comparison {
            Some(comparison) => {
                out += "| Scope | Baseline | Change |\n| --- | ---: | ---: |\n";
                for name in names {
                    let _ = writeln!(
                        out,
                        "| `{name}` | {} | {} |",
                        self_ms(&comparison.base.scope_self_ms_per_run, name),
                        self_ms(&change.scope_self_ms_per_run, name)
                    );
                }
            }
            None => {
                out += "| Scope | Self time |\n| --- | ---: |\n";
                for name in names {
                    let _ = writeln!(
                        out,
                        "| `{name}` | {} |",
                        self_ms(&change.scope_self_ms_per_run, name)
                    );
                }
            }
        }
        out += "</details>\n";
    }

    for note in &section.caveats {
        let _ = write!(out, "\n> {}\n", caveat_text(note));
    }

    let no_kinds = ExceptionKindCounts::default();
    let base_kinds = section
        .baseline_comparison
        .as_ref()
        .map_or(&no_kinds, |comparison| &comparison.base.exception_kinds);
    if !change.exception_kinds.is_empty() || !base_kinds.is_empty() {
        out += "\n<details><summary>Exception breakdown</summary>\n\n";
        // the change's kinds first, then the baseline's; the most frequent in the change first
        let mut kinds: Vec<&str> = change
            .exception_kinds
            .iter()
            .map(|(kind, _)| kind)
            .collect();
        for (kind, _) in base_kinds.iter() {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        kinds.sort_by(|a, b| {
            (change.exception_kinds.get(b), base_kinds.get(b))
                .cmp(&(change.exception_kinds.get(a), base_kinds.get(a)))
        });

        match &section.baseline_comparison {
            Some(comparison) => {
                let _ = writeln!(
                    out,
                    "| Exception | Baseline ({} runs) | Change ({} runs) |",
                    comparison.base.run_count, change.run_count
                );
                out += "| --- | ---: | ---: |\n";
                for kind in kinds {
                    let _ = writeln!(
                        out,
                        "| `{kind}` | {} | {} |",
                        base_kinds.get(kind),
                        change.exception_kinds.get(kind)
                    );
                }
            }
            None => {
                out += "| Exception | Count |\n";
                out += "| --- | ---: |\n";
                for kind in kinds {
                    let _ = writeln!(out, "| `{kind}` | {} |", change.exception_kinds.get(kind));
                }
            }
        }
        out += "</details>\n";
    }

    out.push('\n');
    out
}

/// A section whose files could not be read.
pub fn failure(name: &str, message: &str) -> String {
    format!("> ⚠️ Could not gather the results of {name}: {message}\n\n")
}

/// An expected runner that left no `runs_on.txt`.
pub fn missing(label: &str) -> String {
    format!(
        "> ⚠️ {} produced no results (its job failed, timed out or was skipped)\n\n",
        display_name(label)
    )
}

fn caveat_text(note: &RunnerCaveat) -> String {
    match note {
        RunnerCaveat::MissingRuns {
            build,
            runs,
            expected,
        } => format!(
            "⚠️ Only {runs} of {expected} {build} runs produced data — the app may have crashed in the missing runs."
        ),
        RunnerCaveat::OutlierRunExcluded {
            build,
            exceptions,
            others,
        } => format!(
            "⚠️ One {build} run was excluded as untrustworthy: it logged {exceptions} exceptions, while the other runs logged {}.",
            others
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        RunnerCaveat::SessionsNotComparable(SessionsNotComparable::TooFewRuns) => {
            "⚠️ No verdicts: fewer than 2 usable runs of a build — the run-to-run noise cannot be estimated from a single run."
                .to_string()
        }
        RunnerCaveat::SessionsNotComparable(SessionsNotComparable::SampleCountsDiffer {
            base,
            change,
            ratio,
        }) => format!(
            "⚠️ No verdicts: the builds produced wildly different sample counts per run ({base} baseline vs {change} change, {:.0}% apart) — far beyond any plausible performance difference, so one of them likely did not run the full session and their frame times cannot be compared.",
            (ratio - 1.0) * 100.0
        ),
        RunnerCaveat::NoBaselineData => {
            "⚠️ No Baseline run produced data — the change is shown alone, with no verdicts."
                .to_string()
        }
        RunnerCaveat::NewExceptionKinds(kinds) => format!(
            "⚠️ Exception kinds not seen in the baseline: {}.",
            kinds
                .iter()
                .map(|(kind, count)| format!("`{kind}` (×{count})"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The median, and the (min–max) range when the runs differ, so the noise shows at a glance.
fn frame_time_cell(aggregate: &MedianAndRange) -> String {
    let MedianAndRange { median, min, max } = *aggregate;
    if max - min < 0.05 {
        format!("{median:.1} ms")
    } else {
        format!("{median:.1} ms ({min:.1}–{max:.1})")
    }
}

/// Scopes the breakdown lists.
const TOP_SCOPES_SHOWN: usize = 15;

/// The hiccup count's median and range, with the threshold in effect.
fn cpu_hiccups_cell(build: &BuildBenchmarkRunsStatistics) -> String {
    if build.perf_summary_run_count == 0 {
        return "-".to_owned();
    }
    let MedianAndRange { median, min, max } = build.cpu_hiccups_per_run;
    let threshold = build
        .cpu_hiccup_threshold_ms
        .map_or(String::new(), |ms| format!(" over {ms:.0} ms"));
    if max - min < 0.5 {
        format!("{median:.0}{threshold}")
    } else {
        format!("{median:.0} ({min:.0}–{max:.0}){threshold}")
    }
}

fn peak_memory_cell(build: &BuildBenchmarkRunsStatistics) -> String {
    if build.perf_summary_run_count == 0 {
        return "-".to_owned();
    }
    let MedianAndRange { median, min, max } = build.peak_memory_mib;
    if max - min < 0.5 {
        format!("{median:.0} MiB")
    } else {
        format!("{median:.0} MiB ({min:.0}–{max:.0})")
    }
}

fn signed_mib(delta: f32) -> String {
    if delta.round() == 0.0 {
        "0".to_owned()
    } else {
        format!("{delta:+.0} MiB")
    }
}

fn hiccups_verdict_text(verdict: IncidentsPerRunVerdict) -> String {
    match verdict {
        IncidentsPerRunVerdict::More => "🔴 more hiccups".to_string(),
        IncidentsPerRunVerdict::Fewer => "🟢 fewer hiccups".to_string(),
        IncidentsPerRunVerdict::NoSignificantChange => "⚪ no significant change".to_string(),
        IncidentsPerRunVerdict::Withheld(reason) => format!("⚠️ no verdict — {reason}"),
    }
}

fn peak_memory_verdict_text(verdict: PeakMemoryVerdict) -> String {
    match verdict {
        PeakMemoryVerdict::LessMemory(by) => format!("🟢 {:.0}% less memory", by * 100.0),
        PeakMemoryVerdict::MoreMemory(by) => format!("🔴 {:.0}% more memory", by * 100.0),
        PeakMemoryVerdict::WithinNoise => "⚪ within noise".to_string(),
        PeakMemoryVerdict::Withheld(reason) => format!("⚠️ no verdict — {reason}"),
    }
}

fn samples(build: &BuildBenchmarkRunsStatistics) -> String {
    if build.run_count > 1 {
        format!("{} (×{})", build.median_sample_count, build.run_count)
    } else {
        build.median_sample_count.to_string()
    }
}

fn frame_time_verdict_text(verdict: FrameTimeVerdict) -> String {
    match verdict {
        FrameTimeVerdict::Faster(by) => format!("🟢 {:.0}% faster", by * 100.0),
        FrameTimeVerdict::Slower(by) => format!("🔴 {:.0}% slower", by * 100.0),
        FrameTimeVerdict::WithinNoise => "⚪ within noise".to_string(),
        FrameTimeVerdict::Informational => "— informational".to_string(),
        FrameTimeVerdict::Withheld(reason) => format!("⚠️ no verdict — {reason}"),
    }
}

fn exceptions_verdict_text(verdict: IncidentsPerRunVerdict) -> String {
    match verdict {
        IncidentsPerRunVerdict::More => "🔴 more errors".to_string(),
        IncidentsPerRunVerdict::Fewer => "🟢 fewer errors".to_string(),
        IncidentsPerRunVerdict::NoSignificantChange => "⚪ no significant change".to_string(),
        IncidentsPerRunVerdict::Withheld(reason) => format!("⚠️ no verdict — {reason}"),
    }
}

/// .NET's custom format `0.##` of a double: at most two decimals, no trailing zeros, and `-0` for
/// a negative value that rounds to zero.
fn hundredths(value: f64) -> String {
    let (negative, n) = in_hundredths(value);
    format!("{}{}", if negative { "-" } else { "" }, decimal(n))
}

/// .NET's `+0.##;-0.##;0`: the sign always, and a plain `0` for anything that rounds to zero.
fn signed_hundredths(value: f64) -> String {
    match in_hundredths(value) {
        (_, 0) => "0".to_string(),
        (true, n) => format!("-{}", decimal(n)),
        (false, n) => format!("+{}", decimal(n)),
    }
}

/// The sign and the whole hundredths of `value`, rounded as .NET rounds a custom format: first
/// to 15 significant digits, then half away from zero (so 1.005 is 1.01, where Rust's `{:.2}`
/// says 1.00).
fn in_hundredths(value: f64) -> (bool, u128) {
    let negative = value.is_sign_negative();
    if value == 0.0 || !value.is_finite() {
        return (negative, 0);
    }
    let text = format!("{:.14e}", value.abs());
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return (negative, 0);
    };
    let (Ok(digits), Ok(exponent)) = (
        mantissa.replace('.', "").parse::<u128>(),
        exponent.parse::<i32>(),
    ) else {
        return (negative, 0);
    };
    // |value| = digits × 10^(exponent − 14), so in hundredths it is digits × 10^(exponent − 12)
    let shift = exponent - 12;
    let n = if shift >= 0 {
        digits.saturating_mul(10u128.saturating_pow(shift.unsigned_abs()))
    } else if shift < -15 {
        0
    } else {
        let unit = 10u128.pow(shift.unsigned_abs());
        (digits + unit / 2) / unit
    };
    (negative, n)
}

fn decimal(hundredths: u128) -> String {
    let (whole, cents) = (hundredths / 100, hundredths % 100);
    match cents {
        0 => whole.to_string(),
        c if c % 10 == 0 => format!("{whole}.{}", c / 10),
        c => format!("{whole}.{c:02}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_name_the_machine_and_chrome() {
        assert_eq!(display_name("intel-i5-chrome"), "Intel Core i5 (Chrome)");
        assert_eq!(display_name("apple-m1-chrome"), "Apple M1 (Chrome)");
        assert_eq!(display_name("apple-m1"), "apple-m1");
        assert_eq!(display_name("local-chrome"), "local-chrome");
        assert_eq!(display_name("intel-i5-firefox"), "intel-i5-firefox");
    }

    /// The values .NET prints for these formats (checked with dotnet 10).
    #[test]
    fn hundredths_round_as_dotnet_custom_formats_do() {
        for (value, plain, signed) in [
            (0.0, "0", "0"),
            (-0.0, "-0", "0"),
            (0.125, "0.13", "+0.13"),
            (-0.125, "-0.13", "-0.13"),
            (1.0 / 3.0, "0.33", "+0.33"),
            (2.0 / 3.0, "0.67", "+0.67"),
            (0.005, "0.01", "+0.01"),
            (1.005, "1.01", "+1.01"),
            (2.675, "2.68", "+2.68"),
            (0.004999999999999999, "0.01", "+0.01"),
            (0.001, "0", "0"),
            (-0.001, "-0", "0"),
            (1.5, "1.5", "+1.5"),
            (0.995, "1", "+1"),
            (0.994999, "0.99", "+0.99"),
            (2.5, "2.5", "+2.5"),
            (40.0, "40", "+40"),
        ] {
            assert_eq!(hundredths(value), plain, "{value}");
            assert_eq!(signed_hundredths(value), signed, "{value}");
        }
    }

    /// `{:.1}` and `{:.0}` of an f32 are .NET's `F1` and `F0`: the exact value, ties to even.
    #[test]
    fn a_cell_shows_the_range_only_when_the_runs_differ() {
        let a = |median, min, max| MedianAndRange { median, min, max };
        assert_eq!(frame_time_cell(&a(16.25, 16.25, 16.25)), "16.2 ms");
        assert_eq!(frame_time_cell(&a(16.75, 16.7, 16.74)), "16.8 ms");
        assert_eq!(
            frame_time_cell(&a(33.0, 32.8, 33.35)),
            "33.0 ms (32.8–33.3)"
        );
        assert_eq!(
            frame_time_verdict_text(FrameTimeVerdict::Faster(0.125)),
            "🟢 12% faster"
        );
        assert_eq!(
            frame_time_verdict_text(FrameTimeVerdict::Slower(0.135)),
            "🔴 14% slower"
        );
    }

    #[test]
    fn the_headline_names_regressions_first() {
        let up = [("CPU average", vec!["A", "B"])];
        let down = [("GPU average", vec!["A"]), ("CPU 1% worst", vec!["B"])];
        assert_eq!(
            headline(&down, &up),
            "**Overall: 🔴 GPU average regressed on A; CPU 1% worst regressed on B**\n\n"
        );
        assert_eq!(
            headline(&[], &up),
            "**Overall: 🟢 CPU average improved on A, B**\n\n"
        );
        assert_eq!(
            headline(&[], &[]),
            "**Overall: ✅ no significant changes**\n\n"
        );
    }

    #[test]
    fn the_sample_count_note_rounds_the_gap() {
        let note = RunnerCaveat::SessionsNotComparable(SessionsNotComparable::SampleCountsDiffer {
            base: 2000,
            change: 1000,
            ratio: 2.0,
        });
        assert!(caveat_text(&note).contains("(2000 baseline vs 1000 change, 100% apart)"));
    }
}
