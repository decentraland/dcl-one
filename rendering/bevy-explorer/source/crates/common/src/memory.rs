//! Client memory as observed by the engine. All consumers read the same sample.

use std::num::NonZeroU64;

use bevy::prelude::Resource;

const MIB: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryPressure {
    Abundance,
    Normal,
    Warning,
    Full,
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryReading {
    /// Engine process resident memory on desktop, live engine heap allocations on web.
    /// Scene workers' separate heaps and GPU memory are outside the web figure.
    pub used_bytes: u64,
    /// Physical RAM on desktop, linear memory limit on web.
    pub capacity_mib: u64,
    /// Capacity after applying the optional client limit.
    pub cap_mib: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemoryThresholds {
    pub abundance_mib: f64,
    pub warning_mib: f64,
    pub full_mib: f64,
}

impl MemoryReading {
    pub fn thresholds(&self) -> MemoryThresholds {
        let cap = self.cap_mib as f64;
        MemoryThresholds {
            abundance_mib: cap * 65.0 / 100.0,
            warning_mib: cap * 70.0 / 100.0,
            full_mib: cap * 75.0 / 100.0,
        }
    }

    pub fn pressure(&self) -> MemoryPressure {
        // Unity compares whole MiB and stays in the lower tier at warning/full equality.
        let used_mib = (self.used_bytes / MIB) as f64;
        let thresholds = self.thresholds();
        if used_mib > thresholds.full_mib {
            MemoryPressure::Full
        } else if used_mib > thresholds.warning_mib {
            MemoryPressure::Warning
        } else if used_mib < thresholds.abundance_mib {
            MemoryPressure::Abundance
        } else {
            MemoryPressure::Normal
        }
    }
}

#[derive(Default, Resource)]
pub struct MemoryStatus {
    /// `None` uses the platform's capacity. Changes apply to the current reading.
    pub limit_mib: Option<NonZeroU64>,
    measurement: Option<(u64, u64)>,
}

impl MemoryStatus {
    /// Publish (engine usage, platform capacity), both in bytes. A failed sample clears the reading.
    pub fn record(&mut self, measurement: Option<(u64, u64)>) {
        self.measurement = measurement;
    }

    /// Returns `None` until the platform supplies both usage and a usable capacity.
    pub fn reading(&self) -> Option<MemoryReading> {
        let (used_bytes, capacity_bytes) = self.measurement?;
        let capacity_mib = NonZeroU64::new(capacity_bytes / MIB)?.get();
        Some(MemoryReading {
            used_bytes,
            capacity_mib,
            cap_mib: self
                .limit_mib
                .map_or(capacity_mib, |limit| capacity_mib.min(limit.get())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
