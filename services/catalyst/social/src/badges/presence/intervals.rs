//! Proven stretches of presence in epoch milliseconds, `[start, end)`: the
//! `intervals` of upstream's progress document (`[[start, end], ...]`, merged).

use serde_json::Value;

pub type Interval = (i64, i64);

/// The part of the interval inside the window, if any.
pub fn clamp_to_window((start, end): Interval, window: Interval) -> Option<Interval> {
    let (start, end) = (start.max(window.0), end.min(window.1));
    (end > start).then_some((start, end))
}

/// Sorted, with overlapping or touching intervals merged.
pub fn merge(mut intervals: Vec<Interval>) -> Vec<Interval> {
    intervals.sort_unstable();
    let mut merged: Vec<Interval> = Vec::with_capacity(intervals.len());
    for (start, end) in intervals {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Whole seconds the merged intervals cover.
pub fn whole_seconds(merged: &[Interval]) -> i64 {
    merged.iter().map(|(s, e)| e - s).sum::<i64>() / 1000
}

/// The instant the union reached `required_seconds`: a stay completes when
/// the threshold is crossed, not when the first visit began.
pub fn threshold_crossing(intervals: &[Interval], required_seconds: i64) -> Option<i64> {
    let required = required_seconds * 1000;
    let mut covered = 0;
    for (start, end) in merge(intervals.to_vec()) {
        if covered + (end - start) >= required {
            return Some(start + (required - covered));
        }
        covered += end - start;
    }
    None
}

/// The document's `intervals`; entries that are not two numbers are skipped.
pub fn from_document(value: Option<&Value>) -> Vec<Interval> {
    let number = |v: &Value| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64));
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|pair| match pair.as_array().map(Vec::as_slice) {
            Some([start, end]) => Some((number(start)?, number(end)?)),
            _ => None,
        })
        .collect()
}

pub fn to_document(intervals: &[Interval]) -> Value {
    Value::Array(
        intervals
            .iter()
            .map(|(s, e)| Value::Array(vec![(*s).into(), (*e).into()]))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clamping_keeps_only_the_part_inside_the_window() {
        assert_eq!(clamp_to_window((0, 100), (50, 200)), Some((50, 100)));
        assert_eq!(clamp_to_window((150, 300), (50, 200)), Some((150, 200)));
        assert_eq!(
            clamp_to_window((0, 50), (50, 200)),
            None,
            "ends at the start"
        );
        assert_eq!(
            clamp_to_window((200, 300), (50, 200)),
            None,
            "the end is excluded"
        );
    }

    #[test]
    fn merging_unions_overlapping_and_touching_intervals() {
        assert_eq!(
            merge(vec![
                (30, 40),
                (0, 10),
                (10, 20),
                (5, 8),
                (50, 60),
                (55, 70)
            ]),
            [(0, 20), (30, 40), (50, 70)]
        );
        assert_eq!(whole_seconds(&[(0, 1999), (3000, 4000)]), 2);
    }

    #[test]
    fn the_threshold_is_crossed_inside_the_interval_that_reaches_it() {
        let intervals = [(0, 120_000), (200_000, 500_000)];
        assert_eq!(threshold_crossing(&intervals, 300), Some(380_000));
        assert_eq!(threshold_crossing(&intervals, 420), Some(500_000));
        assert_eq!(threshold_crossing(&intervals, 421), None);
        // overlapping connections count once
        assert_eq!(threshold_crossing(&[(0, 100_000), (0, 100_000)], 150), None);
    }

    #[test]
    fn the_document_round_trips_upstream_numbers() {
        let doc = json!([
            [1_700_000_000_000_i64, 1_700_000_060_000_i64],
            [1.0, 2.0],
            "x",
            [1]
        ]);
        let parsed = from_document(Some(&doc));
        assert_eq!(parsed, [(1_700_000_000_000, 1_700_000_060_000), (1, 2)]);
        assert_eq!(
            to_document(&parsed[..1]),
            json!([[1_700_000_000_000_i64, 1_700_000_060_000_i64]])
        );
        assert!(from_document(None).is_empty());
    }
}
