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
    /// Native reports the engine process's RSS, excluding the scene sidecar and CEF helpers.
    /// Web reports live engine heap allocations, excluding scene workers and GPU memory.
    pub used_bytes: u64,
    /// Physical RAM subject to the reported cgroup limit on native, linear memory limit on web.
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
