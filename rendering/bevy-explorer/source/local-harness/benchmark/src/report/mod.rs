//! `benchmark report`: the Markdown report of a benchmark run, from the result artifacts the leg
//! jobs upload. It is `scripts/ProduceSummary.cs` of `decentraland/performance-testing` ported
//! one to one, so its numbers, verdicts and caveats are the harness's; the header, the section
//! names and the note and headline for missing baseline runs are dcl-one's. It reads files only:
//! the workflow posts what it writes.
//!
//! A section is a folder holding `runs_on.txt` (the leg's label) and its runs' files:
//! `change_build_<i>.csv/.log` and `base_build_<i>.csv/.log`. A run without a CSV crashed; a
//! `.csv.partial` is not read, so its run counts as crashed too. A section whose files cannot be
//! read becomes a note, as do the expected labels that left no section.

mod exceptions;
mod perf_summary;
mod render;
mod stats;

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use crate::cli::{BASE_BUILD, CHANGE_BUILD};
use crate::failure::Failure;
use crate::out;

use exceptions::ExceptionKindCounts;
pub use exceptions::{EXCEPTION_MARKER, EXIT_MARKER};
use perf_summary::ExplorerPerfSummary;
use stats::{
    FrameTimeVerdict, IncidentsPerRunVerdict, MedianAndRange, PeakMemoryVerdict,
    SessionsNotComparable, SignificantMetricChange,
};

/// The exit code when no section has data; the report is written anyway.
pub const EXIT_NO_DATA: i32 = 2;

const RUNS_ON: &str = "runs_on.txt";

pub struct ReportOptions {
    /// Searched for sections, itself included.
    pub dir: PathBuf,
    pub title: String,
    pub pr: Option<u64>,
    /// The change build, as the header names it.
    pub change: Option<String>,
    /// The baseline build, as the header names it.
    pub base: Option<String>,
    /// The workflow run that measured.
    pub run_url: Option<String>,
    /// Measured runs of each build; fewer CSVs mean the app crashed in the missing runs.
    pub expected_runs: u64,
    /// The legs that should have reported.
    pub expected_labels: Vec<String>,
}

pub struct BenchmarkReport {
    pub markdown: String,
    /// Whether any section has data.
    pub has_data: bool,
    /// What went wrong reading a section, for the job log.
    pub warnings: Vec<String>,
}

/// One measured run of a build: its frame-time metrics and the exceptions its log holds.
struct BuildBenchmarkRun {
    sample_count: u64,
    frame_time_ms: [f32; 6],
    exception_count: u64,
    exception_kinds: ExceptionKindCounts,
    /// The explorer's `perf_summary` lines, when the run wrote them.
    perf_summary: Option<ExplorerPerfSummary>,
}

/// A build's runs in a section, outliers excluded.
struct BuildBenchmarkRunsStatistics {
    run_count: usize,
    /// The median sample count.
    median_sample_count: u64,
    frame_time_ms: [MedianAndRange; 6],
    /// Per run, not in total: after a crash or an exclusion the builds have different run counts.
    exceptions_per_run: f64,
    exception_kinds: ExceptionKindCounts,
    /// The runs that wrote a summary, which the rows below come from.
    perf_summary_run_count: usize,
    /// Frames over the hiccup threshold per run, across those runs.
    cpu_hiccups_per_run: MedianAndRange,
    /// The threshold in effect, the median across those runs.
    cpu_hiccup_threshold_ms: Option<f32>,
    /// Peak engine memory per run, MiB, across those runs.
    peak_memory_mib: MedianAndRange,
    /// Self time per run, ms, of the scopes the summaries name, over the runs that name each,
    /// highest first.
    scope_self_ms_per_run: Vec<(String, f32)>,
}

struct BaselineComparison {
    base: BuildBenchmarkRunsStatistics,
    frame_time_verdicts: [FrameTimeVerdict; 6],
    exceptions_per_run_verdict: IncidentsPerRunVerdict,
    cpu_hiccups_per_run_verdict: IncidentsPerRunVerdict,
    peak_memory_verdict: PeakMemoryVerdict,
}

/// A caveat shown under a section's table.
enum RunnerCaveat {
    MissingRuns {
        build: &'static str,
        runs: usize,
        expected: u64,
    },
    OutlierRunExcluded {
        build: &'static str,
        exceptions: u64,
        others: Vec<u64>,
    },
    SessionsNotComparable(SessionsNotComparable),
    /// Exception kinds the change threw at least twice and the baseline never did.
    NewExceptionKinds(Vec<(String, u64)>),
    /// No baseline run produced data: the table shows the change alone, with no verdicts.
    NoBaselineData,
}

struct RunnerSection {
    title: String,
    change: BuildBenchmarkRunsStatistics,
    /// None when no baseline run produced data: the table shows the change alone.
    baseline_comparison: Option<BaselineComparison>,
    caveats: Vec<RunnerCaveat>,
}

impl RunnerSection {
    /// Its 🟢 and 🔴 verdicts, for the headline.
    fn significant_metric_changes(&self) -> Vec<SignificantMetricChange> {
        let Some(comparison) = &self.baseline_comparison else {
            return Vec::new();
        };
        let found = |metric, improved, relative_change| SignificantMetricChange {
            runner: self.title.clone(),
            metric,
            improved,
            relative_change,
        };
        let mut findings: Vec<SignificantMetricChange> = comparison
            .frame_time_verdicts
            .iter()
            .zip(stats::FRAME_TIME_METRICS)
            .filter_map(|(verdict, metric)| match *verdict {
                FrameTimeVerdict::Faster(by) => Some(found(metric, true, by)),
                FrameTimeVerdict::Slower(by) => Some(found(metric, false, by)),
                _ => None,
            })
            .collect();
        match comparison.exceptions_per_run_verdict {
            IncidentsPerRunVerdict::More => {
                findings.push(found(stats::EXCEPTIONS_METRIC, false, 0.0))
            }
            IncidentsPerRunVerdict::Fewer => {
                findings.push(found(stats::EXCEPTIONS_METRIC, true, 0.0))
            }
            _ => {}
        }
        match comparison.cpu_hiccups_per_run_verdict {
            IncidentsPerRunVerdict::More => {
                findings.push(found(stats::CPU_HICCUPS_METRIC, false, 0.0))
            }
            IncidentsPerRunVerdict::Fewer => {
                findings.push(found(stats::CPU_HICCUPS_METRIC, true, 0.0))
            }
            _ => {}
        }
        match comparison.peak_memory_verdict {
            PeakMemoryVerdict::LessMemory(by) => {
                findings.push(found(stats::PEAK_MEMORY_METRIC, true, by))
            }
            PeakMemoryVerdict::MoreMemory(by) => {
                findings.push(found(stats::PEAK_MEMORY_METRIC, false, by))
            }
            _ => {}
        }
        findings
    }
}

/// The report of every section under `options.dir`. A folder that does not exist has none.
pub fn build_report(options: &ReportOptions) -> Result<BenchmarkReport, Failure> {
    let mut warnings = Vec::new();
    let mut folders = Vec::new();
    match fs::metadata(&options.dir) {
        Ok(_) => section_folders(&options.dir, &mut folders)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            warnings.push(format!("{} does not exist", options.dir.display()));
        }
        Err(error) => return Err(error.into()),
    }

    let mut sections = String::new();
    let mut findings = Vec::new();
    let mut labels = Vec::new();
    let mut has_data = false;
    // whether a section had baseline runs to compare with
    let mut compared = false;
    for folder in &folders {
        let result = read_label(folder).and_then(|label| {
            labels.push(label.clone());
            read_section(folder, label, options.expected_runs)
        });
        match result {
            Ok(section) => {
                compared |= section.baseline_comparison.is_some();
                findings.extend(section.significant_metric_changes());
                sections += &render::runner_section(&section);
                has_data = true;
            }
            Err(message) => {
                // the folder's own name: the artifact's (a single artifact is "a runner")
                let name = match folder.file_name() {
                    Some(name) if *folder != options.dir => name.to_string_lossy().into_owned(),
                    _ => "a runner".to_string(),
                };
                warnings.push(format!("could not gather the results of {name}: {message}"));
                sections += &render::failure(&name, &message);
            }
        }
    }

    let mut reported: Vec<&str> = Vec::new();
    for label in &options.expected_labels {
        if !labels.contains(label) && !reported.contains(&label.as_str()) {
            reported.push(label);
            sections += &render::missing(label);
        }
    }

    let mut markdown = render::header(options);
    if compared {
        markdown += &render::headline(
            &stats::headline_metrics(&findings, false),
            &stats::headline_metrics(&findings, true),
        );
    } else if has_data {
        markdown += render::NOTHING_COMPARED;
    }
    markdown += &render::legend(options.expected_runs);
    markdown += &sections;
    Ok(BenchmarkReport {
        markdown,
        has_data,
        warnings,
    })
}

/// Every folder under `dir`, itself included, that holds a `runs_on.txt`, in name order. Links to
/// folders are not followed.
fn section_folders(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    if dir.join(RUNS_ON).is_file() {
        found.push(dir.to_path_buf());
    }
    let mut children = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            children.push(entry.path());
        }
    }
    children.sort();
    for child in children {
        section_folders(&child, found)?;
    }
    Ok(())
}

fn read_label(folder: &Path) -> Result<String, String> {
    let bytes = fs::read(folder.join(RUNS_ON)).map_err(|error| format!("{RUNS_ON}: {error}"))?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.trim_start_matches('\u{FEFF}').trim_end().to_string())
}

fn read_section(folder: &Path, label: String, expected_runs: u64) -> Result<RunnerSection, String> {
    let change = read_runs(folder, CHANGE_BUILD)?;
    if change.is_empty() {
        return Err(format!("no {CHANGE_BUILD} run CSVs were found"));
    }
    let base = read_runs(folder, BASE_BUILD)?;
    Ok(compare_builds(label, change, base, expected_runs))
}

/// A build's runs (`<build>_<i>.csv` and its `.log`) in name order.
fn read_runs(folder: &Path, build: &str) -> Result<Vec<BuildBenchmarkRun>, String> {
    let mut csvs = Vec::new();
    let entries = fs::read_dir(folder).map_err(|error| error.to_string())?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        let is_run = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(build)?.strip_prefix('_'))
            .is_some_and(|rest| rest.ends_with(".csv"));
        if is_run && path.is_file() {
            csvs.push(path);
        }
    }
    csvs.sort();
    csvs.iter()
        .map(|csv| {
            let (sample_count, frame_time_ms) = stats::read_run_frame_times(csv)?;
            let log = csv.with_extension("log");
            let (exception_count, exception_kinds) = exceptions::count(&log).map_err(|error| {
                format!("{}: {}", file_name(&log), out::safe(&error.to_string()))
            })?;
            let perf_summary = perf_summary::read_last_perf_summary(&log).map_err(|error| {
                format!("{}: {}", file_name(&log), out::safe(&error.to_string()))
            })?;
            Ok(BuildBenchmarkRun {
                sample_count,
                frame_time_ms,
                exception_count,
                exception_kinds,
                perf_summary,
            })
        })
        .collect()
}

/// The section's numbers, verdicts and caveats, as `ProduceSummary.cs` works them out.
fn compare_builds(
    label: String,
    change: Vec<BuildBenchmarkRun>,
    base: Vec<BuildBenchmarkRun>,
    expected_runs: u64,
) -> RunnerSection {
    let mut caveats = Vec::new();
    let change = build_benchmark_runs_statistics(&exclude_outlier_runs(
        change,
        "Change",
        expected_runs,
        &mut caveats,
    ));
    let base = build_benchmark_runs_statistics(&exclude_outlier_runs(
        base,
        "Baseline",
        expected_runs,
        &mut caveats,
    ));
    if base.run_count == 0 {
        caveats.push(RunnerCaveat::NoBaselineData);
    }

    let baseline_comparison = (base.run_count > 0).then(|| {
        let incomparable = stats::sessions_not_comparable(
            change.run_count,
            base.run_count,
            change.median_sample_count,
            base.median_sample_count,
        );
        if let Some(incomparable) = incomparable {
            caveats.push(RunnerCaveat::SessionsNotComparable(incomparable));
        }

        // a kind of exception only the change throws is a stronger signal than a higher count
        let mut new_kinds: Vec<(String, u64)> = change
            .exception_kinds
            .iter()
            .filter(|(kind, count)| *count >= 2 && !base.exception_kinds.contains(kind))
            .map(|(kind, count)| (kind.to_string(), count))
            .collect();
        new_kinds.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        if !new_kinds.is_empty() {
            caveats.push(RunnerCaveat::NewExceptionKinds(new_kinds));
        }

        let gpu_broken = stats::gpu_timing_broken(&change.frame_time_ms, &base.frame_time_ms);
        let frame_time_verdicts = std::array::from_fn(|m| {
            stats::frame_time_verdict(
                m,
                &change.frame_time_ms[m],
                &base.frame_time_ms[m],
                incomparable,
                gpu_broken,
            )
        });
        let exceptions_per_run_verdict = stats::incidents_per_run_verdict(
            change.exceptions_per_run,
            base.exceptions_per_run,
            incomparable,
        );
        let both_have_perf_summaries =
            change.perf_summary_run_count > 0 && base.perf_summary_run_count > 0;
        let cpu_hiccups_per_run_verdict = if both_have_perf_summaries {
            stats::incidents_per_run_verdict(
                f64::from(change.cpu_hiccups_per_run.median),
                f64::from(base.cpu_hiccups_per_run.median),
                incomparable,
            )
        } else {
            IncidentsPerRunVerdict::Withheld("no summary")
        };
        let peak_memory_verdict = if both_have_perf_summaries {
            stats::peak_memory_verdict(&change.peak_memory_mib, &base.peak_memory_mib, incomparable)
        } else {
            PeakMemoryVerdict::Withheld("no summary")
        };
        BaselineComparison {
            base,
            frame_time_verdicts,
            exceptions_per_run_verdict,
            cpu_hiccups_per_run_verdict,
            peak_memory_verdict,
        }
    });

    RunnerSection {
        title: render::display_name(&label),
        change,
        baseline_comparison,
        caveats,
    }
}

/// Drops the outlier runs from a build; a build with fewer runs than expected, and every dropped
/// run, gets a note.
fn exclude_outlier_runs(
    runs: Vec<BuildBenchmarkRun>,
    build: &'static str,
    expected_runs: u64,
    caveats: &mut Vec<RunnerCaveat>,
) -> Vec<BuildBenchmarkRun> {
    if !runs.is_empty() && (runs.len() as u64) < expected_runs {
        caveats.push(RunnerCaveat::MissingRuns {
            build,
            runs: runs.len(),
            expected: expected_runs,
        });
    }
    let all: Vec<u64> = runs.iter().map(|run| run.exception_count).collect();
    let (dropped, kept): (Vec<BuildBenchmarkRun>, Vec<BuildBenchmarkRun>) = runs
        .into_iter()
        .partition(|run| stats::is_outlier(run.exception_count, &all));
    let others: Vec<u64> = kept.iter().map(|run| run.exception_count).collect();
    for run in dropped {
        caveats.push(RunnerCaveat::OutlierRunExcluded {
            build,
            exceptions: run.exception_count,
            others: others.clone(),
        });
    }
    kept
}

fn build_benchmark_runs_statistics(runs: &[BuildBenchmarkRun]) -> BuildBenchmarkRunsStatistics {
    let exceptions: u64 = runs.iter().map(|run| run.exception_count).sum();
    let summaries: Vec<&ExplorerPerfSummary> = runs
        .iter()
        .filter_map(|run| run.perf_summary.as_ref())
        .collect();
    // a scope's summed self time and the number of runs whose top scopes name it
    let mut scope_sums: BTreeMap<&str, (f32, usize)> = BTreeMap::new();
    for (name, ms) in summaries
        .iter()
        .flat_map(|summary| &summary.top_scope_self_ms)
    {
        let (sum, runs) = scope_sums.entry(name).or_default();
        *sum += ms;
        *runs += 1;
    }
    let mut scope_self_ms_per_run: Vec<(String, f32)> = scope_sums
        .into_iter()
        .map(|(name, (sum, runs))| (name.to_owned(), sum / runs as f32))
        .collect();
    scope_self_ms_per_run.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    BuildBenchmarkRunsStatistics {
        run_count: runs.len(),
        median_sample_count: stats::median_int(runs.iter().map(|run| run.sample_count)),
        frame_time_ms: std::array::from_fn(|m| {
            stats::median_and_range(runs.iter().map(|run| run.frame_time_ms[m]))
        }),
        exceptions_per_run: if runs.is_empty() {
            0.0
        } else {
            exceptions as f64 / runs.len() as f64
        },
        exception_kinds: ExceptionKindCounts::combine(runs.iter().map(|run| &run.exception_kinds)),
        perf_summary_run_count: summaries.len(),
        cpu_hiccups_per_run: stats::median_and_range(
            summaries
                .iter()
                .filter_map(|summary| summary.cpu_frames_over_hiccup_threshold_count)
                .map(|count| count as f32),
        ),
        cpu_hiccup_threshold_ms: {
            let thresholds: Vec<f32> = summaries
                .iter()
                .filter_map(|summary| summary.cpu_hiccup_threshold_ms)
                .collect();
            (!thresholds.is_empty()).then(|| stats::median_and_range(thresholds).median)
        },
        peak_memory_mib: stats::median_and_range(
            summaries
                .iter()
                .filter_map(|summary| summary.used_peak_mib)
                .map(|mib| mib as f32),
        ),
        scope_self_ms_per_run,
    }
}

/// A path's file name, for a message: never the folders above it.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The lines of a file as .NET's `StreamReader.ReadLine` splits them (`\n`, `\r\n` and a lone
/// `\r` each end one), as UTF-8 with invalid bytes replaced and a byte-order mark dropped: the
/// C# reads both files this way.
struct Lines<R> {
    reader: R,
    pending: std::collections::VecDeque<String>,
    first: bool,
}

impl<R: BufRead> Lines<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            pending: Default::default(),
            first: true,
        }
    }
}

impl<R: BufRead> Iterator for Lines<R> {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pending.is_empty() {
            let mut chunk = Vec::new();
            match self.reader.read_until(b'\n', &mut chunk) {
                Ok(0) => return None,
                Ok(_) => {}
                Err(error) => return Some(Err(error)),
            }
            if std::mem::take(&mut self.first) && chunk.starts_with(b"\xEF\xBB\xBF") {
                chunk.drain(..3);
            }
            let ended = chunk.last() == Some(&b'\n');
            if ended {
                chunk.pop();
                if chunk.last() == Some(&b'\r') {
                    chunk.pop();
                }
            }
            let mut parts: Vec<&[u8]> = chunk.split(|b| *b == b'\r').collect();
            // the end of the file after a lone `\r` is no line of its own
            if !ended && parts.last().is_some_and(|part| part.is_empty()) {
                parts.pop();
            }
            self.pending.extend(
                parts
                    .into_iter()
                    .map(|part| String::from_utf8_lossy(part).into_owned()),
            );
        }
        self.pending.pop_front().map(Ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(bytes: &[u8]) -> Vec<String> {
        Lines::new(bytes).map(Result::unwrap).collect()
    }

    #[test]
    fn lines_end_as_dotnet_ends_them() {
        assert_eq!(lines(b"a\r\nb\nc"), ["a", "b", "c"]);
        assert_eq!(lines(b"a\r\rb\n"), ["a", "", "b"]);
        assert_eq!(lines(b"a\r"), ["a"]);
        assert_eq!(lines(b"\n\n"), ["", ""]);
        assert_eq!(lines(b""), Vec::<String>::new());
        assert_eq!(lines(b"\xEF\xBB\xBFa\n\xEF\xBB\xBFb"), ["a", "\u{FEFF}b"]);
    }
}
