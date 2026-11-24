//! Client memory as observed by the engine. All consumers read the same sample.

use std::{num::NonZeroU64, time::Duration};

use bevy::prelude::{DetectChangesMut, Mut, Resource};

const MIB: u64 = 1 << 20;

/// Native GPU budget for the renderer's selected adapter, independent of engine RAM.
#[derive(Debug, PartialEq, Eq, Resource)]
pub enum GpuMemoryBudget {
    /// Browser WebGPU exposes no driver budget, or this app has no renderer.
    NotApplicable,
    /// Native renderer with no usable reading on this backend or device.
    Unavailable,
    /// Each pool has its own budget. Spare bytes in one pool cannot fund another.
    Available(Vec<GpuMemoryPool>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuMemoryPoolKind {
    DxgiLocal,
    VulkanDeviceLocal { heap_index: usize },
    MetalWorkingSet,
}

#[derive(Debug, PartialEq, Eq)]
pub struct GpuMemoryPool {
    pub kind: GpuMemoryPoolKind,
    /// Process usage on DXGI/Vulkan, Metal resource allocations on the selected device.
    pub used_bytes: u64,
    /// Driver budget or Metal's recommended working set. Usage can exceed this value.
    pub budget_bytes: NonZeroU64,
}

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
    /// Web counts live allocations in the shared engine heap, including workers and scene relays.
    /// The scene runtime's own WASM heap is separate. Neither platform measures GPU residency here.
    pub used_bytes: u64,
    /// Native physical RAM, capped on Linux by the limit visible at the cgroup mount root.
    /// Tighter limits in nested process cgroups may be missed. On web this is the WASM
    /// address-space ceiling. Fragmentation and device limits can cause allocations to fail sooner.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemorySample {
    pub used_bytes: u64,
    pub capacity_bytes: u64,
}

#[derive(Default, Resource)]
pub struct MemoryStatus {
    /// `None` uses the platform's capacity. Changes apply to the current reading.
    pub limit_mib: Option<NonZeroU64>,
    measurement: Option<MemorySample>,
}

impl MemoryStatus {
    /// A failed sample clears the reading. Identical samples leave Bevy change ticks alone.
    pub fn record(status: Mut<'_, Self>, measurement: Option<MemorySample>) {
        status
            .map_unchanged(|s| &mut s.measurement)
            .set_if_neq(measurement);
    }

    /// Returns `None` until the platform supplies both usage and a usable capacity.
    pub fn reading(&self) -> Option<MemoryReading> {
        let MemorySample {
            used_bytes,
            capacity_bytes,
        } = self.measurement?;
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

/// Whether new loads wait because engine memory is full. The scene lifecycle and the download
/// slots read it to admit only the player's loads; work already running continues.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct LoadRefusal {
    pub refusing: bool,
    /// When the reading was last `Full`, on the clock `next` is given.
    pub full_at: Duration,
}

impl LoadRefusal {
    /// How long a refusal outlasts the last `Full` reading while the reading stays in `Warning`.
    /// Freed memory can stay resident, so the reading may never fall below the warning threshold
    /// once it has risen past it; without a limit, loads would wait for the rest of the session.
    pub const WARNING_HOLD: Duration = Duration::from_secs(30);

    /// On at `Full`; held through `Warning` for up to [`Self::WARNING_HOLD`] after the last `Full`
    /// reading; off at `Normal`, `Abundance` or with no reading. The gap between the two
    /// thresholds keeps a reading at the full threshold from admitting and refusing on alternate
    /// samples.
    pub fn next(self, reading: Option<MemoryReading>, now: Duration) -> Self {
        match reading.map(|reading| reading.pressure()) {
            Some(MemoryPressure::Full) => Self {
                refusing: true,
                full_at: now,
            },
            Some(MemoryPressure::Warning)
                if now.saturating_sub(self.full_at) < Self::WARNING_HOLD =>
            {
                self
            }
            _ => Self {
                refusing: false,
                ..self
            },
        }
    }
}
