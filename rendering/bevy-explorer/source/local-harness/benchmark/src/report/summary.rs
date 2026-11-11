//! The run's `perf_summary` lines, read from the run's log before its exit line. A log without
//! them gives no summary.
//!
//! Line 1 is `key=value` pairs separated by spaces; a value is a number or `n/a`. Line 2 is
//! `top_scopes=<name>:<self ms>,...`.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use super::exceptions::EXIT_MARKER;
use super::Lines;

/// The end of the entry line's prefix for the summary's target.
const TARGET_TAG: &str = "[perf_summary]</color>: ";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub cpu_p99_ms: Option<f32>,
    pub cpu_hiccups: Option<u64>,
    pub cpu_hiccup_threshold_ms: Option<f32>,
    pub gpu_p99_ms: Option<f32>,
    pub gpu_hiccups: Option<u64>,
    pub used_peak_mib: Option<u64>,
    pub js_heap_used_mib: Option<u64>,
    pub entity_count: Option<u64>,
    /// Self time in ms over the measured phase, highest first.
    pub top_scopes: Vec<(String, f32)>,
}

/// The last summary the log holds before its exit line; a log without one, or without a file,
/// gives `None`.
pub fn read(log: &Path) -> io::Result<Option<Summary>> {
    if !log.is_file() {
        return Ok(None);
    }
    let mut summary: Option<Summary> = None;
    for line in Lines::new(BufReader::new(File::open(log)?)) {
        let line = line?;
        if line.contains(EXIT_MARKER) {
            break;
        }
        let Some(index) = line.find(TARGET_TAG) else {
            continue;
        };
        let text = &line[index + TARGET_TAG.len()..];
        if let Some(scopes) = text.strip_prefix("top_scopes=") {
            if let Some(summary) = summary.as_mut() {
                summary.top_scopes = parse_scopes(scopes);
            }
        } else if text.starts_with("cpu_p99_ms=") {
            summary = Some(parse_values(text));
        }
    }
    Ok(summary)
}

fn parse_values(text: &str) -> Summary {
    let mut summary = Summary::default();
    for pair in text.split(' ') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let float = || value.parse::<f32>().ok();
        let int = || value.parse::<u64>().ok();
        match key {
            "cpu_p99_ms" => summary.cpu_p99_ms = float(),
            "cpu_frames_over_hiccup_threshold_count" => summary.cpu_hiccups = int(),
            "cpu_hiccup_threshold_ms" => summary.cpu_hiccup_threshold_ms = float(),
            "gpu_p99_ms" => summary.gpu_p99_ms = float(),
            "gpu_frames_over_hiccup_threshold_count" => summary.gpu_hiccups = int(),
            "used_peak_mib" => summary.used_peak_mib = int(),
            "js_heap_used_mib" => summary.js_heap_used_mib = int(),
            "entity_count" => summary.entity_count = int(),
            _ => {}
        }
    }
    summary
}

fn parse_scopes(text: &str) -> Vec<(String, f32)> {
    text.split(',')
        .filter_map(|entry| {
            let (name, ms) = entry.rsplit_once(':')?;
            Some((name.trim().to_owned(), ms.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_values_and_scopes_are_read_and_n_a_is_none() {
        let summary = parse_values(
            "cpu_p99_ms=80.00 cpu_frames_over_hiccup_threshold_count=1 cpu_hiccup_threshold_ms=50.00 gpu_p99_ms=n/a gpu_frames_over_hiccup_threshold_count=n/a used_peak_mib=300 js_heap_used_mib=64 entity_count=1234 sample_count=4 snapshot_duration_seconds=90.0",
        );
        assert_eq!(summary.cpu_p99_ms, Some(80.0));
        assert_eq!(summary.cpu_hiccups, Some(1));
        assert_eq!(summary.cpu_hiccup_threshold_ms, Some(50.0));
        assert_eq!(summary.gpu_p99_ms, None);
        assert_eq!(summary.gpu_hiccups, None);
        assert_eq!(summary.used_peak_mib, Some(300));
        assert_eq!(summary.js_heap_used_mib, Some(64));
        assert_eq!(summary.entity_count, Some(1234));
        assert_eq!(
            parse_scopes("a  b:9.00,system x:1.50"),
            [("a  b".to_owned(), 9.0), ("system x".to_owned(), 1.5)]
        );
    }
}
