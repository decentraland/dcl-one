//! The numbers and verdicts of `ProduceSummary.cs`, one to one: a run's frame-time metrics, the
//! median and range across a build's runs, the rules that give a metric a verdict, the outlier
//! rule and the headline's grouping. The arithmetic is `f32` where the C# is `float` and `f64`
//! where it is `double`, so every number and every threshold comparison comes out the same.

use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use super::{file_name, Lines};
use crate::out;

/// The smallest relative difference of the medians that gets a verdict.
pub const MIN_RELATIVE_MEDIAN_DELTA: f32 = 0.03;
/// A smaller median difference never gets a verdict: for small medians (a GPU time of a few ms) a
/// few tenths of a millisecond is noise, not signal.
pub const MIN_ABSOLUTE_FRAME_TIME_DELTA_MS: f32 = 0.5;
/// No real frame-time statistic is below a millisecond: a smaller median means the measurement
/// broke (GPU timing reporting zeros), so that metric gets no verdict.
pub const MIN_PLAUSIBLE_MS: f32 = 1.0;
/// The incidents-per-run verdict (exceptions, CPU hiccups) needs a difference of at least this
/// many incidents per run...
pub const MIN_INCIDENTS_PER_RUN_DELTA: f64 = 2.0;
/// ...and at least this many times the other build's: transient network errors flip tiny deltas.
pub const MIN_INCIDENTS_PER_RUN_RATIO: f64 = 1.5;
/// A run that logged at least this many exceptions, and [`OUTLIER_FACTOR`] times the median of
/// its build's runs, is excluded as untrustworthy (a service was down during it).
pub const OUTLIER_MIN_EXCEPTIONS: u64 = 10;
pub const OUTLIER_FACTOR: u64 = 5;
/// The headline names a metric that moved on one machine only when it moved by this much.
const HEADLINE_MAGNITUDE: f32 = 0.10;
/// Median sample counts further apart than this mean the builds did not run the same session.
const MAX_SAMPLE_RATIO: f32 = 1.5;

pub const FRAME_TIME_METRICS: [&str; 6] = [
    "CPU average",
    "CPU 1% worst",
    "CPU 0.1% worst",
    "GPU average",
    "GPU 1% worst",
    "GPU 0.1% worst",
];
/// The 0.1% worst metrics average only the worst two or three frames of a run, so one OS blip
/// moves them by tens of percent: shown for context, never given a verdict.
const FRAME_TIME_METRIC_HAS_VERDICT: [bool; 6] = [true, true, false, true, true, false];
/// "GPU average" in [`FRAME_TIME_METRICS`].
const GPU_AVERAGE: usize = 3;

pub const EXCEPTIONS_METRIC: &str = "Exceptions per run";
/// Frames over the explorer's hiccup threshold per run, from the run's `perf_summary` lines.
pub const CPU_HICCUPS_METRIC: &str = "Hiccups per run";
/// Peak engine memory per run, MiB, from the same lines.
pub const PEAK_MEMORY_METRIC: &str = "Peak memory";
/// A smaller peak-memory median difference never gets a verdict.
pub const MIN_ABSOLUTE_PEAK_MEMORY_DELTA_MIB: f32 = 32.0;

/// One run's sample count and its metrics in the order of [`FRAME_TIME_METRICS`], from its CSV. The first
/// line is the header; every other line is `frame,cpu,gpu`. A CSV with no rows, a row with fewer
/// than three fields or a time that is not a number is an error, as it is for the C#.
pub fn read_run_frame_times(csv: &Path) -> Result<(u64, [f32; 6]), String> {
    let name = file_name(csv);
    let file = File::open(csv).map_err(|error| format!("{name}: {error}"))?;
    let mut cpu = Vec::new();
    let mut gpu = Vec::new();
    for (index, line) in Lines::new(BufReader::new(file)).enumerate().skip(1) {
        let line = line.map_err(|error| format!("{name}: {error}"))?;
        let fields: Vec<&str> = line.split(',').collect();
        let (Some(cpu_ms), Some(gpu_ms)) = (fields.get(1), fields.get(2)) else {
            return Err(format!(
                "{name} line {}: fewer than three fields",
                index + 1
            ));
        };
        let number = |text: &str| {
            parse_float(text).ok_or_else(|| {
                format!(
                    "{name} line {}: '{}' is not a number",
                    index + 1,
                    out::safe(text)
                )
            })
        };
        cpu.push(number(cpu_ms)?);
        gpu.push(number(gpu_ms)?);
    }
    if cpu.is_empty() {
        return Err(format!("{name} contains zero samples"));
    }
    Ok((
        cpu.len() as u64,
        [
            average(&cpu),
            percent_worst(&cpu, 0.01),
            percent_worst(&cpu, 0.001),
            average(&gpu),
            percent_worst(&gpu, 0.01),
            percent_worst(&gpu, 0.001),
        ],
    ))
}

/// `float.Parse` under the invariant culture: the white space it allows around the number.
fn parse_float(text: &str) -> Option<f32> {
    text.trim_matches(|c| c == ' ' || ('\t'..='\r').contains(&c))
        .parse()
        .ok()
}

/// LINQ's `Average()` of floats: summed in `f64`, in order, and the mean narrowed to `f32`.
fn average(values: &[f32]) -> f32 {
    let sum: f64 = values.iter().map(|&v| f64::from(v)).sum();
    (sum / values.len() as f64) as f32
}

/// The mean of the worst `fraction` of the frames, at least one (as GamersNexus measures it).
fn percent_worst(values: &[f32], fraction: f32) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| float_compare_to(*b, *a));
    let take = ((values.len() as f32 * fraction) as usize).max(1);
    average(&sorted[..take.min(sorted.len())])
}

/// `float.CompareTo`, the order LINQ sorts by: NaN before every number.
fn float_compare_to(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b)
        .unwrap_or_else(|| a.is_nan().cmp(&b.is_nan()).reverse())
}

/// `Math.Min` of floats: NaN when either is.
fn min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.min(b)
    }
}

/// A metric across the runs of one build.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MedianAndRange {
    pub median: f32,
    pub min: f32,
    pub max: f32,
}

/// The median, the lowest and the highest. The median of an even count is the mean of the two
/// middle values: the upper one alone would be the maximum of a two-run build.
pub fn median_and_range(values: impl IntoIterator<Item = f32>) -> MedianAndRange {
    let mut sorted: Vec<f32> = values.into_iter().collect();
    sorted.sort_by(|a, b| float_compare_to(*a, *b));
    let n = sorted.len();
    let (Some(&min), Some(&max)) = (sorted.first(), sorted.last()) else {
        return MedianAndRange::default();
    };
    let median = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    };
    MedianAndRange { median, min, max }
}

/// The median of whole numbers; for an even count the mean of the two middle ones, rounded up.
pub fn median_int(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut sorted: Vec<u64> = values.into_iter().collect();
    sorted.sort_unstable();
    let n = sorted.len();
    match n {
        0 => 0,
        _ if n % 2 == 1 => sorted[n / 2],
        _ => (sorted[n / 2 - 1] + sorted[n / 2]).div_ceil(2),
    }
}

/// A run that logged many exceptions, and many times the median of its build's runs.
pub fn is_outlier(exceptions: u64, build: &[u64]) -> bool {
    exceptions >= OUTLIER_MIN_EXCEPTIONS
        && exceptions >= OUTLIER_FACTOR * median_int(build.iter().copied()).max(1)
}

/// What a frame-time metric's Result cell says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameTimeVerdict {
    /// Faster by this fraction of the baseline's median.
    Faster(f32),
    /// Slower by this fraction of the baseline's median.
    Slower(f32),
    WithinNoise,
    Informational,
    /// No verdict, for this reason.
    Withheld(&'static str),
}

/// Why the builds' sessions cannot be compared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionsNotComparable {
    /// A build has fewer than two usable runs: its noise cannot be estimated.
    TooFewRuns,
    /// The median sample counts are more than [`MAX_SAMPLE_RATIO`] apart: one build likely did
    /// not run the whole session.
    SampleCountsDiffer { base: u64, change: u64, ratio: f32 },
}

impl SessionsNotComparable {
    pub fn reason(self) -> &'static str {
        match self {
            SessionsNotComparable::TooFewRuns => "too few runs",
            SessionsNotComparable::SampleCountsDiffer { .. } => "sample counts differ",
        }
    }
}

pub fn sessions_not_comparable(
    change_runs: usize,
    base_runs: usize,
    change_samples: u64,
    base_samples: u64,
) -> Option<SessionsNotComparable> {
    let ratio = change_samples.max(base_samples) as f32 / change_samples.min(base_samples) as f32;
    if change_runs < 2 || base_runs < 2 {
        Some(SessionsNotComparable::TooFewRuns)
    } else if ratio > MAX_SAMPLE_RATIO {
        Some(SessionsNotComparable::SampleCountsDiffer {
            base: base_samples,
            change: change_samples,
            ratio,
        })
    } else {
        None
    }
}

/// Whether GPU timing worked in both builds: a sub-millisecond GPU average means it reported
/// zeros for most frames, which poisons every GPU metric.
pub fn gpu_timing_broken(change: &[MedianAndRange; 6], base: &[MedianAndRange; 6]) -> bool {
    min(change[GPU_AVERAGE].median, base[GPU_AVERAGE].median) < MIN_PLAUSIBLE_MS
}

/// The verdict of metric `m`. A change counts only when the builds' runs are fully separated
/// (every change run faster, or every one slower, than every baseline run: with three runs a
/// build, the most extreme outcome of a permutation test, one-sided p = 0.05) and the median
/// difference clears both the relative and the absolute floor.
pub fn frame_time_verdict(
    m: usize,
    change: &MedianAndRange,
    base: &MedianAndRange,
    incomparable: Option<SessionsNotComparable>,
    gpu_broken: bool,
) -> FrameTimeVerdict {
    if let Some(incomparable) = incomparable {
        return FrameTimeVerdict::Withheld(incomparable.reason());
    }
    if !FRAME_TIME_METRIC_HAS_VERDICT[m] {
        return FrameTimeVerdict::Informational;
    }
    if gpu_broken && FRAME_TIME_METRICS[m].starts_with("GPU") {
        return FrameTimeVerdict::Withheld("GPU timing not working");
    }
    if min(change.median, base.median) < MIN_PLAUSIBLE_MS {
        return FrameTimeVerdict::Withheld("implausibly small values");
    }

    let delta = change.median - base.median;
    let improvement = -delta / base.median;
    let separated = change.min > base.max || change.max < base.min;
    if !separated || delta.abs() < MIN_ABSOLUTE_FRAME_TIME_DELTA_MS {
        FrameTimeVerdict::WithinNoise
    } else if improvement >= MIN_RELATIVE_MEDIAN_DELTA {
        FrameTimeVerdict::Faster(improvement)
    } else if improvement <= -MIN_RELATIVE_MEDIAN_DELTA {
        FrameTimeVerdict::Slower(-improvement)
    } else {
        FrameTimeVerdict::WithinNoise
    }
}

/// What the Peak memory row's Result cell says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeakMemoryVerdict {
    /// Less memory by this fraction of the baseline's median.
    LessMemory(f32),
    /// More memory by this fraction of the baseline's median.
    MoreMemory(f32),
    WithinNoise,
    /// No verdict, for this reason.
    Withheld(&'static str),
}

/// The peak-memory verdict: the frame-time metrics' separation and relative floor, with an
/// absolute floor of [`MIN_ABSOLUTE_PEAK_MEMORY_DELTA_MIB`].
pub fn peak_memory_verdict(
    change: &MedianAndRange,
    base: &MedianAndRange,
    incomparable: Option<SessionsNotComparable>,
) -> PeakMemoryVerdict {
    if let Some(incomparable) = incomparable {
        return PeakMemoryVerdict::Withheld(incomparable.reason());
    }
    if base.median <= 0.0 {
        return PeakMemoryVerdict::Withheld("no baseline memory");
    }
    if change.median <= 0.0 {
        return PeakMemoryVerdict::Withheld("no change memory");
    }
    let delta = change.median - base.median;
    let improvement = -delta / base.median;
    let separated = change.min > base.max || change.max < base.min;
    if !separated || delta.abs() < MIN_ABSOLUTE_PEAK_MEMORY_DELTA_MIB {
        PeakMemoryVerdict::WithinNoise
    } else if improvement >= MIN_RELATIVE_MEDIAN_DELTA {
        PeakMemoryVerdict::LessMemory(improvement)
    } else if improvement <= -MIN_RELATIVE_MEDIAN_DELTA {
        PeakMemoryVerdict::MoreMemory(-improvement)
    } else {
        PeakMemoryVerdict::WithinNoise
    }
}

/// What the Exceptions per run and Hiccups per run rows' Result cell says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IncidentsPerRunVerdict {
    More,
    Fewer,
    NoSignificantChange,
    /// No verdict, for this reason.
    Withheld(&'static str),
}

pub fn incidents_per_run_verdict(
    change: f64,
    base: f64,
    incomparable: Option<SessionsNotComparable>,
) -> IncidentsPerRunVerdict {
    if let Some(incomparable) = incomparable {
        IncidentsPerRunVerdict::Withheld(incomparable.reason())
    } else if change >= base + MIN_INCIDENTS_PER_RUN_DELTA
        && change >= MIN_INCIDENTS_PER_RUN_RATIO * base.max(1.0)
    {
        IncidentsPerRunVerdict::More
    } else if base >= change + MIN_INCIDENTS_PER_RUN_DELTA
        && base >= MIN_INCIDENTS_PER_RUN_RATIO * change.max(1.0)
    {
        IncidentsPerRunVerdict::Fewer
    } else {
        IncidentsPerRunVerdict::NoSignificantChange
    }
}

/// The Hiccups per run verdict: the incidents rule on the medians, given fully separated run
/// ranges, and the same hiccup threshold in both builds (a build's threshold is the median of
/// its summaries').
pub fn hiccups_per_run_verdict(
    change: &MedianAndRange,
    base: &MedianAndRange,
    change_threshold_ms: Option<f32>,
    base_threshold_ms: Option<f32>,
    incomparable: Option<SessionsNotComparable>,
) -> IncidentsPerRunVerdict {
    if let Some(incomparable) = incomparable {
        return IncidentsPerRunVerdict::Withheld(incomparable.reason());
    }
    if change_threshold_ms
        .zip(base_threshold_ms)
        .is_some_and(|(change_ms, base_ms)| (change_ms - base_ms).abs() >= 0.5)
    {
        return IncidentsPerRunVerdict::Withheld("the builds' hiccup thresholds differ");
    }
    let verdict = incidents_per_run_verdict(f64::from(change.median), f64::from(base.median), None);
    let separated = change.min > base.max || change.max < base.min;
    if separated {
        verdict
    } else {
        IncidentsPerRunVerdict::NoSignificantChange
    }
}

/// A 🟢 or 🔴 verdict, kept for the headline.
#[derive(Debug, Clone, PartialEq)]
pub struct SignificantMetricChange {
    pub runner: String,
    pub metric: &'static str,
    pub improved: bool,
    /// The relative difference; 0 for incident counts.
    pub relative_change: f32,
}

/// The metrics the headline names for one direction, each with the runners it moved on, in the
/// order they were found. With about five verdict rows on each of up to five machines, one 🟢
/// or 🔴 cell turns up by chance now and then, so a metric is named only when it moved on two or
/// more machines, or by 10% or more on one.
pub fn headline_metrics(
    findings: &[SignificantMetricChange],
    improved: bool,
) -> Vec<(&'static str, Vec<&str>)> {
    let mut groups: Vec<(&'static str, Vec<&SignificantMetricChange>)> = Vec::new();
    for finding in findings.iter().filter(|f| f.improved == improved) {
        match groups
            .iter_mut()
            .find(|(metric, _)| *metric == finding.metric)
        {
            Some((_, group)) => group.push(finding),
            None => groups.push((finding.metric, vec![finding])),
        }
    }
    groups
        .into_iter()
        .filter_map(|(metric, group)| {
            let mut runners: Vec<&str> = Vec::new();
            for finding in &group {
                if !runners.contains(&finding.runner.as_str()) {
                    runners.push(&finding.runner);
                }
            }
            let large = group
                .iter()
                .any(|f| f.relative_change >= HEADLINE_MAGNITUDE);
            (runners.len() >= 2 || large).then_some((metric, runners))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agg(median: f32, min: f32, max: f32) -> MedianAndRange {
        MedianAndRange { median, min, max }
    }

    #[test]
    fn the_median_of_an_even_count_is_the_mean_of_the_middle_two() {
        assert_eq!(median_and_range([3.0, 1.0, 2.0]), agg(2.0, 1.0, 3.0));
        assert_eq!(median_and_range([4.0, 1.0]), agg(2.5, 1.0, 4.0));
        assert_eq!(median_int([1, 2]), 2, "rounded up");
        assert_eq!(median_int([1, 3]), 2);
        assert_eq!(median_int([5, 1, 9]), 5);
    }

    #[test]
    fn the_worst_frames_are_one_at_least_and_a_fraction_of_many() {
        let few: Vec<f32> = (1..=10).map(|v| v as f32).collect();
        assert_eq!(percent_worst(&few, 0.01), 10.0);
        let many: Vec<f32> = (1..=1000).map(|v| v as f32).collect();
        // 1% of 1000 frames: the worst ten, 991 to 1000
        assert_eq!(percent_worst(&many, 0.01), 995.5);
        assert_eq!(percent_worst(&many, 0.001), 1000.0);
    }

    #[test]
    fn a_float_mean_is_summed_wide_and_narrowed() {
        // in f32 the sum would lose the small values next to the large one
        let values = [16_777_216.0, 1.0, 1.0, 1.0, 1.0];
        assert_eq!(average(&values), 3_355_444.0);
    }

    #[test]
    fn numbers_take_white_space_around_them_and_nothing_else() {
        assert_eq!(parse_float(" 16.7\t"), Some(16.7));
        assert_eq!(parse_float("abc"), None);
        assert_eq!(parse_float(""), None);
    }

    #[test]
    fn the_hiccup_verdict_needs_separated_runs_and_equal_thresholds() {
        let runs = |median, min, max| MedianAndRange { median, min, max };
        // medians 0 and 2, ranges overlapping (base 0, 0, 3; change 2, 2, 0)
        assert_eq!(
            hiccups_per_run_verdict(
                &runs(2.0, 0.0, 2.0),
                &runs(0.0, 0.0, 3.0),
                Some(50.0),
                Some(50.0),
                None
            ),
            IncidentsPerRunVerdict::NoSignificantChange
        );
        assert_eq!(
            hiccups_per_run_verdict(
                &runs(4.0, 4.0, 5.0),
                &runs(0.0, 0.0, 1.0),
                Some(50.0),
                Some(50.0),
                None
            ),
            IncidentsPerRunVerdict::More
        );
        assert_eq!(
            hiccups_per_run_verdict(
                &runs(4.0, 4.0, 5.0),
                &runs(0.0, 0.0, 1.0),
                Some(66.7),
                Some(50.0),
                None
            ),
            IncidentsPerRunVerdict::Withheld("the builds' hiccup thresholds differ")
        );
    }

    #[test]
    fn a_verdict_needs_separated_runs_and_both_floors() {
        let base = agg(20.0, 19.5, 20.5);
        let v = |change| frame_time_verdict(0, &change, &base, None, false);
        assert_eq!(v(agg(18.0, 17.5, 18.5)), FrameTimeVerdict::Faster(0.1));
        assert_eq!(v(agg(22.0, 21.5, 22.5)), FrameTimeVerdict::Slower(0.1));
        assert_eq!(
            v(agg(18.0, 17.5, 19.6)),
            FrameTimeVerdict::WithinNoise,
            "overlap"
        );
        assert_eq!(
            v(agg(19.45, 19.45, 19.45)),
            FrameTimeVerdict::WithinNoise,
            "under 3%"
        );
        let small = agg(5.0, 5.0, 5.0);
        assert_eq!(
            frame_time_verdict(0, &agg(4.6, 4.6, 4.6), &small, None, false),
            FrameTimeVerdict::WithinNoise,
            "8% but under 0.5 ms"
        );
    }

    #[test]
    fn what_stops_a_verdict_in_the_c_sharp_order() {
        let (a, b) = (agg(10.0, 10.0, 10.0), agg(20.0, 20.0, 20.0));
        assert_eq!(
            frame_time_verdict(2, &a, &b, Some(SessionsNotComparable::TooFewRuns), true),
            FrameTimeVerdict::Withheld("too few runs")
        );
        assert_eq!(
            frame_time_verdict(5, &a, &b, None, true),
            FrameTimeVerdict::Informational
        );
        assert_eq!(
            frame_time_verdict(4, &a, &b, None, true),
            FrameTimeVerdict::Withheld("GPU timing not working")
        );
        assert_eq!(
            frame_time_verdict(1, &a, &b, None, true),
            FrameTimeVerdict::Faster(0.5)
        );
        assert_eq!(
            frame_time_verdict(1, &agg(0.9, 0.9, 0.9), &b, None, false),
            FrameTimeVerdict::Withheld("implausibly small values")
        );
    }

    #[test]
    fn sessions_are_incomparable_with_one_run_or_far_apart_sample_counts() {
        assert_eq!(
            sessions_not_comparable(1, 3, 100, 100),
            Some(SessionsNotComparable::TooFewRuns)
        );
        assert_eq!(
            sessions_not_comparable(3, 1, 100, 100),
            Some(SessionsNotComparable::TooFewRuns)
        );
        assert_eq!(
            sessions_not_comparable(2, 2, 150, 100),
            None,
            "1.5 is the limit"
        );
        assert_eq!(
            sessions_not_comparable(2, 2, 100, 151),
            Some(SessionsNotComparable::SampleCountsDiffer {
                base: 151,
                change: 100,
                ratio: 1.51
            })
        );
    }

    #[test]
    fn exceptions_count_on_two_more_a_run_and_half_as_many_again() {
        let v = |change, base| incidents_per_run_verdict(change, base, None);
        assert_eq!(v(3.0, 1.0), IncidentsPerRunVerdict::More);
        assert_eq!(v(2.0, 0.0), IncidentsPerRunVerdict::More);
        assert_eq!(v(1.0, 0.0), IncidentsPerRunVerdict::NoSignificantChange);
        assert_eq!(
            v(6.0, 4.0),
            IncidentsPerRunVerdict::More,
            "both limits are reached"
        );
        assert_eq!(
            v(7.0, 5.0),
            IncidentsPerRunVerdict::NoSignificantChange,
            "under 1.5x"
        );
        assert_eq!(v(0.0, 2.0), IncidentsPerRunVerdict::Fewer);
    }

    #[test]
    fn peak_memory_needs_a_reading_in_both_builds() {
        let reading = agg(900.0, 890.0, 910.0);
        let none = agg(0.0, 0.0, 0.0);
        assert_eq!(
            peak_memory_verdict(&none, &reading, None),
            PeakMemoryVerdict::Withheld("no change memory")
        );
        assert_eq!(
            peak_memory_verdict(&reading, &none, None),
            PeakMemoryVerdict::Withheld("no baseline memory")
        );
    }

    #[test]
    fn an_outlier_has_ten_exceptions_and_five_times_the_median() {
        assert!(is_outlier(40, &[2, 40, 3]));
        assert!(!is_outlier(9, &[0, 9, 0]), "under ten");
        assert!(!is_outlier(14, &[3, 14, 3]), "under five times");
        assert!(is_outlier(10, &[0, 10, 0]), "a median of 0 counts as 1");
    }

    #[test]
    fn the_headline_takes_two_machines_or_ten_percent() {
        let found = |runner: &str, metric, improved, relative_change| SignificantMetricChange {
            runner: runner.into(),
            metric,
            improved,
            relative_change,
        };
        let findings = [
            found("A", "CPU average", true, 0.05),
            found("B", "CPU average", true, 0.04),
            found("A", "GPU average", true, 0.05),
            found("A", "CPU 1% worst", true, 0.12),
            found("A", "GPU 1% worst", false, 0.05),
            found("A", EXCEPTIONS_METRIC, false, 0.0),
            found("A", EXCEPTIONS_METRIC, false, 0.0),
        ];
        assert_eq!(
            headline_metrics(&findings, true),
            [("CPU average", vec!["A", "B"]), ("CPU 1% worst", vec!["A"])]
        );
        assert!(
            headline_metrics(&findings, false).is_empty(),
            "one machine each"
        );
    }
}
