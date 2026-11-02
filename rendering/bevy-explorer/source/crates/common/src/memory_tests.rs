use std::num::NonZeroU64;

use crate::memory::{MemoryPressure, MemoryStatus, MemoryThresholds};

const MIB: u64 = 1 << 20;

#[test]
fn classifies_whole_mib_at_unity_boundaries() {
    use MemoryPressure::*;
    let mut status = MemoryStatus::default();
    for (used_bytes, expected) in [
        (650 * MIB - 1, Abundance),
        (650 * MIB, Normal),
        (701 * MIB - 1, Normal),
        (701 * MIB, Warning),
        (751 * MIB - 1, Warning),
        (751 * MIB, Full),
        (2_000 * MIB, Full),
        (500 * MIB, Abundance),
    ] {
        status.record(Some((used_bytes, 1_000 * MIB)));
        let reading = status.reading().unwrap();
        assert_eq!(reading.used_bytes, used_bytes);
        assert_eq!(reading.pressure(), expected);
    }
}

#[test]
fn thresholds_preserve_fractional_mib_on_a_four_gib_heap() {
    let mut status = MemoryStatus::default();
    status.record(Some((2662 * MIB, 1_u64 << 32)));
    let reading = status.reading().unwrap();
    assert_eq!(reading.pressure(), MemoryPressure::Abundance);
    assert_eq!(
        reading.thresholds(),
        MemoryThresholds {
            abundance_mib: 2662.4,
            warning_mib: 2867.2,
            full_mib: 3072.0,
        }
    );
}

#[test]
fn changing_the_limit_reclassifies_the_last_measurement() {
    let mut status = MemoryStatus::default();
    status.record(Some((600 * MIB, 1_000 * MIB)));
    for (limit, cap, pressure) in [
        (Some(500), 500, MemoryPressure::Full),
        (Some(900), 900, MemoryPressure::Normal),
        (Some(2_000), 1_000, MemoryPressure::Abundance),
        (None, 1_000, MemoryPressure::Abundance),
    ] {
        status.limit_mib = limit.and_then(NonZeroU64::new);
        let reading = status.reading().unwrap();
        assert_eq!(reading.capacity_mib, 1_000);
        assert_eq!(reading.cap_mib, cap);
        assert_eq!(reading.pressure(), pressure);
    }
}

#[test]
fn unavailable_measurements_do_not_report_a_pressure_tier() {
    let mut status = MemoryStatus::default();
    assert!(status.reading().is_none());
    status.record(Some((0, 0)));
    assert!(status.reading().is_none());
    status.record(Some((0, 4096 * MIB)));
    assert_eq!(
        status.reading().unwrap().pressure(),
        MemoryPressure::Abundance
    );
    status.record(None);
    assert!(status.reading().is_none());
}
