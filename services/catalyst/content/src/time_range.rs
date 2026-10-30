//! The snapshot generation windows: the time since [`SNAPSHOTS_INIT_TIMESTAMP_MS`]
//! cut into whole years, months, weeks and days (a "month" is four weeks and
//! a "year" twelve such months, as upstream defines them), largest first, so
//! a window, once formed, only ever changes by being folded whole into the
//! next larger one. What is left after the last whole day is the remainder:
//! no snapshot covers it, a peer reads it from `/pointer-changes`.
//!
//! The old tree carried this division twice, over `f64` in the generator and
//! over `i64` here, with the same vectors; v2 keeps one, over the wire
//! [`TimeRange`] in milliseconds. The old file's `is_time_range_covered_by` and
//! `join_overlapped_time_ranges` had no caller in either tree and were not
//! copied.

use super::wire_types::{TimeRange, Timestamp};

/// 2020-01-01T00:00:00Z: the start of the first window (upstream's genesis
/// for time-windowed snapshots).
pub const SNAPSHOTS_INIT_TIMESTAMP_MS: Timestamp = 1_577_836_800_000;

pub const MS_PER_DAY: i64 = 86_400_000;
pub const MS_PER_WEEK: i64 = 7 * MS_PER_DAY;
pub const MS_PER_MONTH: i64 = 4 * MS_PER_WEEK;
pub const MS_PER_YEAR: i64 = 12 * MS_PER_MONTH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeRangeDivision {
    pub intervals: Vec<TimeRange>,
    pub remainder: TimeRange,
}

pub fn time_range_size_ms(time_range: &TimeRange) -> i64 {
    time_range.end_timestamp - time_range.init_timestamp
}

pub fn interval_size_label(time_range: &TimeRange) -> &'static str {
    match time_range_size_ms(time_range) {
        d if d == MS_PER_DAY => "day",
        d if d == MS_PER_WEEK => "week",
        d if d == MS_PER_MONTH => "month",
        d if d == MS_PER_YEAR => "year",
        _ => "unknown",
    }
}

/// Upstream's `divideTimeInYearsMonthsWeeksAndDays`: a size is used while
/// what remains still holds one of it plus one of each smaller size, so the
/// smaller windows keep a full set to fold into the next larger one.
pub fn divide_time_in_years_months_weeks_and_days(time_range: &TimeRange) -> TimeRangeDivision {
    let interval_sizes = [MS_PER_YEAR, MS_PER_MONTH, MS_PER_WEEK, MS_PER_DAY];
    let mut intervals = Vec::new();
    let mut remaining = time_range_size_ms(time_range);
    let mut init = time_range.init_timestamp;

    for (idx, &interval_size) in interval_sizes.iter().enumerate() {
        let next_size = interval_sizes
            .get(idx + 1)
            .copied()
            .unwrap_or(interval_size);
        let num_next_in_current = interval_size / next_size;
        let min_needed = num_next_in_current * next_size
            + interval_sizes.get(idx + 1).copied().unwrap_or(0)
            + interval_sizes.get(idx + 2).copied().unwrap_or(0)
            + interval_sizes.get(idx + 3).copied().unwrap_or(0);

        while remaining >= min_needed {
            let end = init + interval_size;
            intervals.push(TimeRange {
                init_timestamp: init,
                end_timestamp: end,
            });
            init = end;
            remaining -= interval_size;
        }
    }

    TimeRangeDivision {
        intervals,
        remainder: TimeRange {
            init_timestamp: init,
            end_timestamp: time_range.end_timestamp,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(init: i64, end: i64) -> TimeRange {
        TimeRange {
            init_timestamp: init,
            end_timestamp: end,
        }
    }

    fn time_range_of_days(start: i64, num_days: i64) -> TimeRange {
        range(start, start + MS_PER_DAY * num_days)
    }

    /// One letter per interval (I day, W week, M month, Y year), checking on
    /// the way that the intervals are contiguous from the start and the
    /// remainder closes the range.
    fn division_repr(time_range: &TimeRange) -> String {
        let div = divide_time_in_years_months_weeks_and_days(time_range);
        let mut repr = String::new();
        for interval in &div.intervals {
            repr.push(match time_range_size_ms(interval) {
                d if d == MS_PER_DAY => 'I',
                d if d == MS_PER_WEEK => 'W',
                d if d == MS_PER_MONTH => 'M',
                d if d == MS_PER_YEAR => 'Y',
                _ => '?',
            });
        }
        if let Some(first) = div.intervals.first() {
            assert_eq!(
                first.init_timestamp, time_range.init_timestamp,
                "first interval starts the range"
            );
            let mut prev_end = first.init_timestamp;
            for interval in &div.intervals {
                assert_eq!(
                    interval.init_timestamp, prev_end,
                    "intervals are contiguous"
                );
                prev_end = interval.end_timestamp;
            }
            assert_eq!(
                div.remainder.init_timestamp, prev_end,
                "the remainder starts where the last interval ends"
            );
        }
        assert_eq!(
            div.remainder.end_timestamp, time_range.end_timestamp,
            "the remainder ends the range"
        );
        repr
    }

    #[test]
    fn test_divide_simple() {
        let total = MS_PER_YEAR + MS_PER_MONTH + MS_PER_WEEK + MS_PER_DAY;
        let div = divide_time_in_years_months_weeks_and_days(&range(0, total));
        assert_eq!(div.intervals.len(), 4);
        assert_eq!(time_range_size_ms(&div.intervals[0]), MS_PER_YEAR);
        assert_eq!(time_range_size_ms(&div.intervals[1]), MS_PER_MONTH);
        assert_eq!(time_range_size_ms(&div.intervals[2]), MS_PER_WEEK);
        assert_eq!(time_range_size_ms(&div.intervals[3]), MS_PER_DAY);
    }

    #[test]
    fn test_interval_size_label_day() {
        assert_eq!(interval_size_label(&range(0, MS_PER_DAY)), "day");
        assert_eq!(interval_size_label(&range(0, MS_PER_WEEK)), "week");
        assert_eq!(interval_size_label(&range(0, MS_PER_MONTH)), "month");
        assert_eq!(interval_size_label(&range(0, MS_PER_YEAR)), "year");
    }

    #[test]
    fn test_interval_size_label_unknown() {
        assert_eq!(interval_size_label(&range(0, 12345)), "unknown");
    }

    /// The union of the two old vector tables (time_range.rs and the
    /// generator's `division_matches_reference_progression_vectors`).
    #[test]
    fn test_progression_key_boundaries() {
        let start = 1_640_995_200_000i64;
        let cases: &[(i64, &str)] = &[
            (1, "I"),
            (7, "IIIIIII"),
            (8, "WI"),
            (14, "WIIIIIII"),
            (15, "WWI"),
            (21, "WWIIIIIII"),
            (22, "WWWI"),
            (28, "WWWIIIIIII"),
            (29, "WWWWI"),
            (34, "WWWWIIIIII"),
            (35, "WWWWIIIIIII"),
            (36, "MWI"),
            (42, "MWIIIIIII"),
            (43, "MWWI"),
            (49, "MWWIIIIIII"),
            (50, "MWWWI"),
            (56, "MWWWIIIIIII"),
            (57, "MWWWWI"),
            (63, "MWWWWIIIIIII"),
            (64, "MMWI"),
            (364, "MMMMMMMMMMMMWWWIIIIIII"),
            (365, "MMMMMMMMMMMMWWWWI"),
            (370, "MMMMMMMMMMMMWWWWIIIIII"),
            (371, "MMMMMMMMMMMMWWWWIIIIIII"),
            (372, "YMWI"),
        ];
        for (days, expected) in cases {
            assert_eq!(
                division_repr(&time_range_of_days(start, *days)),
                *expected,
                "{days} days"
            );
        }
    }

    #[test]
    fn test_divide_two_days_plus_remainder() {
        let start = 1_640_995_200_000i64;
        let div = divide_time_in_years_months_weeks_and_days(&range(
            start,
            start + 2 * MS_PER_DAY + 50_000,
        ));
        assert_eq!(div.intervals.len(), 2);
        for interval in &div.intervals {
            assert_eq!(time_range_size_ms(interval), MS_PER_DAY);
        }
        assert_eq!(div.intervals[0], range(start, start + MS_PER_DAY));
        assert_eq!(
            div.intervals[1],
            range(start + MS_PER_DAY, start + 2 * MS_PER_DAY)
        );
        assert_eq!(time_range_size_ms(&div.remainder), 50_000);
    }

    #[test]
    fn division_matches_reference_shape() {
        let init = SNAPSHOTS_INIT_TIMESTAMP_MS;
        let whole = range(init, init + (6 * 365 + 142) * MS_PER_DAY);
        let repr = division_repr(&whole);
        assert!(
            repr.starts_with('Y'),
            "the first interval is a year: {repr}"
        );
        let div = divide_time_in_years_months_weeks_and_days(&whole);
        assert!(time_range_size_ms(&div.remainder) >= 0);
    }

    #[test]
    fn division_empty_when_too_small() {
        let init = SNAPSHOTS_INIT_TIMESTAMP_MS;
        let div = divide_time_in_years_months_weeks_and_days(&range(init, init + MS_PER_DAY / 2));
        assert!(div.intervals.is_empty());
        assert_eq!(div.remainder, range(init, init + MS_PER_DAY / 2));
    }

    #[test]
    fn division_genesis_to_fixed_end_exact_counts() {
        let init = SNAPSHOTS_INIT_TIMESTAMP_MS;
        let end = init + 372 * MS_PER_DAY;
        let div = divide_time_in_years_months_weeks_and_days(&range(init, end));
        let count_of = |size: i64| {
            div.intervals
                .iter()
                .filter(|iv| time_range_size_ms(iv) == size)
                .count()
        };
        assert_eq!(count_of(MS_PER_YEAR), 1);
        assert_eq!(count_of(MS_PER_MONTH), 1);
        assert_eq!(count_of(MS_PER_WEEK), 1);
        assert_eq!(count_of(MS_PER_DAY), 1);
        assert_eq!(div.intervals.len(), 4);
        assert_eq!(div.remainder, range(end, end), "zero remainder");
    }

    /// The windows share their boundaries and the generator's query is an
    /// inclusive BETWEEN, so an entity stamped exactly on a boundary is in
    /// both adjacent snapshots: intentional, for CID parity with upstream.
    #[test]
    fn entity_on_shared_boundary_is_counted_in_both_windows_inclusive() {
        let in_window = |ts: i64, w: &TimeRange| ts >= w.init_timestamp && ts <= w.end_timestamp;
        let init = SNAPSHOTS_INIT_TIMESTAMP_MS;
        let div = divide_time_in_years_months_weeks_and_days(&range(init, init + 8 * MS_PER_DAY));
        assert_eq!(div.intervals.len(), 2, "8 days divide into week + day");
        let boundary = div.intervals[0].end_timestamp;
        assert_eq!(boundary, div.intervals[1].init_timestamp);
        assert_eq!(
            div.intervals
                .iter()
                .filter(|w| in_window(boundary, w))
                .count(),
            2
        );
        let inside_first = div.intervals[0].init_timestamp + MS_PER_DAY;
        assert_eq!(
            div.intervals
                .iter()
                .filter(|w| in_window(inside_first, w))
                .count(),
            1
        );
    }

    /// Hierarchical: every window of an earlier division lies inside exactly
    /// one window of a later one, which is what lets a pass replace the
    /// smaller windows a larger one absorbed without leaving a gap.
    #[test]
    fn a_later_division_contains_every_earlier_window() {
        let init = SNAPSHOTS_INIT_TIMESTAMP_MS;
        for (earlier, later) in [(34, 36), (35, 36), (63, 64), (371, 372), (400, 700)] {
            let before =
                divide_time_in_years_months_weeks_and_days(&time_range_of_days(init, earlier))
                    .intervals;
            let after =
                divide_time_in_years_months_weeks_and_days(&time_range_of_days(init, later))
                    .intervals;
            for w in &before {
                let containers = after
                    .iter()
                    .filter(|c| {
                        c.init_timestamp <= w.init_timestamp && w.end_timestamp <= c.end_timestamp
                    })
                    .count();
                assert_eq!(containers, 1, "{earlier} -> {later} days: {w:?}");
            }
        }
    }
}
