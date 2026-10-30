//! `benchmark report`: the Markdown report of a benchmark run, from the result artifacts the leg
//! jobs upload. It is `scripts/ProduceSummary.cs` of `decentraland/performance-testing` ported
//! one to one, so its numbers, verdicts and notes are the harness's; the header, the section
//! names and the note and headline for missing baseline runs are dcl-one's. It reads files only:
//! the workflow posts what it writes.
//!
//! A section is a folder holding `runs_on.txt` (the leg's label) and its runs' files:
//! `change_build_<i>.csv/.log` and `base_build_<i>.csv/.log`. A run without a CSV crashed; a
//! `.csv.partial` is not read, so its run counts as crashed too. A section whose files cannot be
//! read becomes a note, as do the expected labels that left no section.

mod exceptions;
mod render;
mod stats;

use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use crate::cli::{BASE_BUILD, CHANGE_BUILD};
use crate::failure::Failure;
use crate::out;

use exceptions::Kinds;
pub use exceptions::{EXCEPTION_MARKER, EXIT_MARKER};
use stats::{Aggregate, ExceptionVerdict, Finding, Incomparable, Verdict};

/// The exit code when no section has data; the report is written anyway.
pub const EXIT_NO_DATA: i32 = 2;

const RUNS_ON: &str = "runs_on.txt";

pub struct Options {
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

pub struct Report {
    pub markdown: String,
    /// Whether any section has data.
    pub has_data: bool,
    /// What went wrong reading a section, for the job log.
    pub warnings: Vec<String>,
}

/// One measured run of a build: its frame-time metrics and the exceptions its log holds.
struct Run {
    samples: u64,
    metrics: [f32; 6],
    exceptions: u64,
    kinds: Kinds,
}

/// A build's runs in a section, outliers excluded.
struct Build {
    runs: usize,
    /// The median sample count.
    samples: u64,
    metrics: [Aggregate; 6],
    /// Per run, not in total: after a crash or an exclusion the builds have different run counts.
    exceptions: f64,
    kinds: Kinds,
}

struct Comparison {
    base: Build,
    verdicts: [Verdict; 6],
    exceptions: ExceptionVerdict,
}

/// A caveat shown under a section's table.
enum Note {
    MissingRuns {
        build: &'static str,
        runs: usize,
        expected: u64,
    },
    Outlier {
        build: &'static str,
        exceptions: u64,
        others: Vec<u64>,
    },
    Incomparable(Incomparable),
    /// Kinds the change threw at least twice and the baseline never did.
    NewKinds(Vec<(String, u64)>),
    /// No baseline run produced data: the table shows the change alone, with no verdicts.
    NoBaseline,
}

struct Section {
    title: String,
    change: Build,
    /// None when no baseline run produced data: the table shows the change alone.
    comparison: Option<Comparison>,
    notes: Vec<Note>,
}

impl Section {
    /// Its 🟢 and 🔴 verdicts, for the headline.
    fn findings(&self) -> Vec<Finding> {
        let Some(comparison) = &self.comparison else {
            return Vec::new();
        };
        let found = |metric, improved, magnitude| Finding {
            runner: self.title.clone(),
            metric,
            improved,
            magnitude,
        };
        let mut findings: Vec<Finding> = comparison
            .verdicts
            .iter()
            .zip(stats::METRICS)
            .filter_map(|(verdict, metric)| match *verdict {
                Verdict::Faster(by) => Some(found(metric, true, by)),
                Verdict::Slower(by) => Some(found(metric, false, by)),
                _ => None,
            })
            .collect();
        match comparison.exceptions {
            ExceptionVerdict::More => findings.push(found(stats::EXCEPTIONS_METRIC, false, 0.0)),
            ExceptionVerdict::Fewer => findings.push(found(stats::EXCEPTIONS_METRIC, true, 0.0)),
            _ => {}
        }
        findings
    }
}

/// The report of every section under `options.dir`. A folder that does not exist has none.
pub fn build(options: &Options) -> Result<Report, Failure> {
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
                compared |= section.comparison.is_some();
                findings.extend(section.findings());
                sections += &render::section(&section);
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
            &stats::headline(&findings, false),
            &stats::headline(&findings, true),
        );
    } else if has_data {
        markdown += render::NOTHING_COMPARED;
    }
    markdown += &render::legend(options.expected_runs);
    markdown += &sections;
    Ok(Report {
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

fn read_section(folder: &Path, label: String, expected_runs: u64) -> Result<Section, String> {
    let change = read_runs(folder, CHANGE_BUILD)?;
    if change.is_empty() {
        return Err(format!("no {CHANGE_BUILD} run CSVs were found"));
    }
    let base = read_runs(folder, BASE_BUILD)?;
    Ok(compare(label, change, base, expected_runs))
}

/// A build's runs (`<build>_<i>.csv` and its `.log`) in name order.
fn read_runs(folder: &Path, build: &str) -> Result<Vec<Run>, String> {
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
            let (samples, metrics) = stats::read_run(csv)?;
            let log = csv.with_extension("log");
            let (exceptions, kinds) = exceptions::count(&log).map_err(|error| {
                format!("{}: {}", file_name(&log), out::safe(&error.to_string()))
            })?;
            Ok(Run {
                samples,
                metrics,
                exceptions,
                kinds,
            })
        })
        .collect()
}

/// The section's numbers, verdicts and notes, as `ProduceSummary.cs` works them out.
fn compare(label: String, change: Vec<Run>, base: Vec<Run>, expected_runs: u64) -> Section {
    let mut notes = Vec::new();
    let change = summarize(&keep(change, "Change", expected_runs, &mut notes));
    let base = summarize(&keep(base, "Baseline", expected_runs, &mut notes));
    if base.runs == 0 {
        notes.push(Note::NoBaseline);
    }

    let comparison = (base.runs > 0).then(|| {
        let incomparable =
            stats::incomparable(change.runs, base.runs, change.samples, base.samples);
        if let Some(incomparable) = incomparable {
            notes.push(Note::Incomparable(incomparable));
        }

        // a kind of exception only the change throws is a stronger signal than a higher count
        let mut new_kinds: Vec<(String, u64)> = change
            .kinds
            .iter()
            .filter(|(kind, count)| *count >= 2 && !base.kinds.contains(kind))
            .map(|(kind, count)| (kind.to_string(), count))
            .collect();
        new_kinds.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        if !new_kinds.is_empty() {
            notes.push(Note::NewKinds(new_kinds));
        }

        let gpu_broken = stats::gpu_timing_broken(&change.metrics, &base.metrics);
        let verdicts = std::array::from_fn(|m| {
            stats::verdict(
                m,
                &change.metrics[m],
                &base.metrics[m],
                incomparable,
                gpu_broken,
            )
        });
        let exceptions = stats::exception_verdict(change.exceptions, base.exceptions, incomparable);
        Comparison {
            base,
            verdicts,
            exceptions,
        }
    });

    Section {
        title: render::display_name(&label),
        change,
        comparison,
        notes,
    }
}

/// Drops the outlier runs from a build; a build with fewer runs than expected, and every dropped
/// run, gets a note.
fn keep(
    runs: Vec<Run>,
    build: &'static str,
    expected_runs: u64,
    notes: &mut Vec<Note>,
) -> Vec<Run> {
    if !runs.is_empty() && (runs.len() as u64) < expected_runs {
        notes.push(Note::MissingRuns {
            build,
            runs: runs.len(),
            expected: expected_runs,
        });
    }
    let all: Vec<u64> = runs.iter().map(|run| run.exceptions).collect();
    let (dropped, kept): (Vec<Run>, Vec<Run>) = runs
        .into_iter()
        .partition(|run| stats::is_outlier(run.exceptions, &all));
    let others: Vec<u64> = kept.iter().map(|run| run.exceptions).collect();
    for run in dropped {
        notes.push(Note::Outlier {
            build,
            exceptions: run.exceptions,
            others: others.clone(),
        });
    }
    kept
}

fn summarize(runs: &[Run]) -> Build {
    let exceptions: u64 = runs.iter().map(|run| run.exceptions).sum();
    Build {
        runs: runs.len(),
        samples: stats::median_int(runs.iter().map(|run| run.samples)),
        metrics: std::array::from_fn(|m| stats::aggregate(runs.iter().map(|run| run.metrics[m]))),
        exceptions: if runs.is_empty() {
            0.0
        } else {
            exceptions as f64 / runs.len() as f64
        },
        kinds: Kinds::combine(runs.iter().map(|run| &run.kinds)),
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
