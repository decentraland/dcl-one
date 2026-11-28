use std::{num::NonZeroU64, time::Duration};

use crate::memory::{
    EngineMemoryPressureMode, MemoryPressure, MemorySample, MemoryStatus, MemoryThresholds,
};
use bevy::prelude::*;

const MIB: u64 = 1 << 20;

fn sample(used_bytes: u64, capacity_bytes: u64) -> Option<MemorySample> {
    Some(MemorySample {
        used_bytes,
        capacity_bytes,
    })
}

fn record(world: &mut World, sample: Option<MemorySample>) -> &MemoryStatus {
    MemoryStatus::record(world.get_resource_or_init::<MemoryStatus>(), sample);
    world.resource()
}

#[test]
fn publishing_an_identical_measurement_preserves_bevy_change_ticks() {
    let mut world = World::new();
    world.init_resource::<MemoryStatus>();
    for (sample, changed) in [
        (None, false),
        (sample(123, MIB), true),
        (sample(123, MIB), false),
        (None, true),
        (None, false),
    ] {
        world.clear_trackers();
        let mut status = world.resource_mut::<MemoryStatus>();
        MemoryStatus::record(status.reborrow(), sample);
        assert_eq!(status.is_changed(), changed);
    }
}

#[test]
fn classifies_whole_mib_at_unity_boundaries() {
    use MemoryPressure::*;
    let mut world = World::new();
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
        let reading = record(&mut world, sample(used_bytes, 1_000 * MIB))
            .reading()
            .unwrap();
        assert_eq!(reading.used_bytes, used_bytes);
        assert_eq!(reading.pressure(), expected);
    }
}

#[test]
fn thresholds_preserve_fractional_mib_on_a_four_gib_heap() {
    let mut world = World::new();
    let reading = record(&mut world, sample(2662 * MIB, 1_u64 << 32))
        .reading()
        .unwrap();
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
    let mut world = World::new();
    record(&mut world, sample(600 * MIB, 1_000 * MIB));
    let mut status = world.resource_mut::<MemoryStatus>();
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
    let mut world = World::new();
    assert!(record(&mut world, None).reading().is_none());
    assert!(record(&mut world, sample(0, 0)).reading().is_none());
    let status = record(&mut world, sample(0, 4096 * MIB));
    assert_eq!(
        status.reading().unwrap().pressure(),
        MemoryPressure::Abundance
    );
    assert!(record(&mut world, None).reading().is_none());
}

#[test]
fn load_refusal_starts_at_full_and_ends_below_warning() {
    let mut world = World::new();
    let mut mode = EngineMemoryPressureMode::Normal;
    let now = Duration::from_secs(100);
    // cap 1000 MiB: warning above 700, full above 750
    for (used_mib, refusing) in [
        (720, false),
        (760, true),
        (720, true),
        (700, false),
        (720, false),
    ] {
        let reading = record(&mut world, sample(used_mib * MIB, 1_000 * MIB)).reading();
        mode = mode.next(reading, now);
        assert_eq!(mode.is_refusing(), refusing, "at {used_mib} MiB");
    }
    mode = mode.next(
        record(&mut world, sample(760 * MIB, 1_000 * MIB)).reading(),
        now,
    );
    assert!(mode.is_refusing());
    assert!(!mode.next(None, now).is_refusing());
}

#[test]
fn load_refusal_ends_after_warning_outlasts_the_hold() {
    let mut world = World::new();
    let full = record(&mut world, sample(760 * MIB, 1_000 * MIB)).reading();
    let mode = EngineMemoryPressureMode::Normal.next(full, Duration::from_secs(10));
    let warning = record(&mut world, sample(720 * MIB, 1_000 * MIB)).reading();

    let held =
        Duration::from_secs(10) + EngineMemoryPressureMode::WARNING_HOLD - Duration::from_millis(1);
    assert!(mode.next(warning, held).is_refusing());
    let past = Duration::from_secs(10) + EngineMemoryPressureMode::WARNING_HOLD;
    let ended = mode.next(warning, past);
    assert!(!ended.is_refusing());
    // a later full reading starts it again
    assert!(ended.next(full, past).is_refusing());
}
