//! Cooperative cache reclamation. Registration is consumed at plugin finalization;
//! the running app uses ordinary, statically scheduled Bevy systems.
use crate::{
    memory::{GpuMemoryBudget, MemoryPressure, MemoryStatus},
    structs::PreviewMode,
};
use bevy::{
    ecs::{
        schedule::{IntoScheduleConfigs, ScheduleConfigs},
        system::ScheduleSystem,
    },
    platform::time::Instant,
    prelude::*,
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReclaimSimulation {
    #[default]
    Off,
    Ram,
    Gpu,
    Both,
}

#[derive(Resource)]
pub struct MemoryReclaimSettings {
    pub frame_budget: Duration,
    /// Session-only override; never changes the platform samples.
    pub simulation: ReclaimSimulation,
}

impl Default for MemoryReclaimSettings {
    fn default() -> Self {
        Self {
            frame_budget: Duration::from_millis(1),
            simulation: ReclaimSimulation::Off,
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct MemoryReclaimState {
    pub ram: Option<MemoryPressure>,
    pub gpu: Option<MemoryPressure>,
    pub ram_active: bool,
    pub gpu_active: bool,
    ram_latched: bool,
    gpu_latched: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum ReclaimDomains {
    Ram,
    RamAndGpu,
}

#[derive(Clone, Copy, Debug)]
pub struct ReclaimSpec {
    pub name: &'static str,
    pub priority: u16,
    pub domains: ReclaimDomains,
    pub max_releases: usize,
    pub max_inspections: usize,
}

impl ReclaimSpec {
    pub fn new(name: &'static str, priority: u16, domains: ReclaimDomains) -> Self {
        Self {
            name,
            priority,
            domains,
            max_releases: 10,
            max_inspections: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReclaimScanStatus {
    #[default]
    NotRun,
    InProgress,
    NoEligible,
    Released,
}

impl ReclaimScanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::InProgress => "in_progress",
            Self::NoEligible => "no_eligible",
            Self::Released => "released",
        }
    }
}

#[derive(Debug)]
pub struct ReclaimStats {
    pub spec: ReclaimSpec,
    pub entries: usize,
    /// Cumulative pins released, not a claim about bytes freed by the renderer.
    pub released: u64,
    pub inspected: u64,
    pub last_released: usize,
    pub last_inspected: usize,
    pub scan_status: ReclaimScanStatus,
    pub completed_scans: u64,
    /// Releases in the last complete candidate scan, including earlier bounded turns.
    pub last_scan_released: Option<u64>,
}

#[derive(Resource, Default)]
pub struct ReclaimBudget {
    caches: Vec<ReclaimStats>,
    started: Option<Instant>,
    deadline: Option<Instant>,
    start_cache: usize,
    next_cache: usize,
    ram: bool,
    gpu: bool,
    pub elapsed: Duration,
    pub budget_exhausted: bool,
}

/// A cache's bounded work allowance. Checking before each candidate bounds scans
/// as well as releases. A single destructor may still exceed the time allowance.
pub struct ReclaimTurn {
    cache: Option<usize>,
    deadline: Option<Instant>,
    max_releases: usize,
    max_inspections: usize,
    released: usize,
    inspected: usize,
    completed_scan: Option<u64>,
}

impl ReclaimTurn {
    /// Also used by ordinary idle expiry, outside memory-pressure passes.
    pub fn bounded(max_releases: usize, max_inspections: usize) -> Self {
        Self {
            cache: None,
            deadline: None,
            max_releases,
            max_inspections,
            released: 0,
            inspected: 0,
            completed_scan: None,
        }
    }

    pub fn inspect(&mut self) -> bool {
        if self.released >= self.max_releases
            || self.inspected >= self.max_inspections
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return false;
        }
        self.inspected += 1;
        true
    }

    pub fn released(&mut self) {
        self.released += 1;
    }

    /// The cache reached the end of its scan. The count spans all resumed turns,
    /// including ordinary idle expiry when it shares the same cache cursor.
    pub fn complete_scan(&mut self, released: u64) {
        self.completed_scan = Some(released);
    }
}

impl ReclaimBudget {
    pub fn caches(&self) -> &[ReclaimStats] {
        &self.caches
    }

    pub fn begin(&mut self, name: &str, entries: usize) -> Option<ReclaimTurn> {
        let index = self
            .caches
            .iter()
            .position(|cache| cache.spec.name == name)?;
        let cache = &mut self.caches[index];
        cache.entries = entries;
        if index < self.start_cache
            || !(self.ram || (self.gpu && matches!(cache.spec.domains, ReclaimDomains::RamAndGpu)))
        {
            return None;
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.budget_exhausted = true;
            return None;
        }
        let mut turn = ReclaimTurn::bounded(cache.spec.max_releases, cache.spec.max_inspections);
        turn.cache = Some(index);
        turn.deadline = self.deadline;
        Some(turn)
    }

    pub fn finish_turn(&mut self, turn: ReclaimTurn, entries: usize) {
        let Some(index) = turn.cache else {
            return;
        };
        let cache = &mut self.caches[index];
        cache.entries = entries;
        cache.released += turn.released as u64;
        cache.inspected += turn.inspected as u64;
        cache.last_released = turn.released;
        cache.last_inspected = turn.inspected;
        cache.scan_status = match turn.completed_scan {
            Some(released) => {
                cache.completed_scans += 1;
                cache.last_scan_released = Some(released);
                if released == 0 {
                    ReclaimScanStatus::NoEligible
                } else {
                    ReclaimScanStatus::Released
                }
            }
            None => ReclaimScanStatus::InProgress,
        };
        self.next_cache = (index + 1) % self.caches.len();
    }
}

#[derive(Default)]
struct ReclaimerRegistration(Vec<(ReclaimSpec, ScheduleConfigs<ScheduleSystem>)>);

pub trait AddMemoryReclaimer {
    fn add_memory_reclaimer<M>(
        &mut self,
        spec: ReclaimSpec,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;
}

impl AddMemoryReclaimer for App {
    fn add_memory_reclaimer<M>(
        &mut self,
        spec: ReclaimSpec,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.init_non_send_resource::<ReclaimerRegistration>();
        let mut registry = self
            .world_mut()
            .non_send_resource_mut::<ReclaimerRegistration>();
        assert!(
            !registry
                .0
                .iter()
                .any(|(existing, _)| existing.name == spec.name),
            "duplicate memory reclaimer: {}",
            spec.name
        );
        registry.0.push((spec, system.into_configs()));
        self
    }
}

pub struct MemoryReclaimPlugin;
impl Plugin for MemoryReclaimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MemoryReclaimSettings>()
            .init_resource::<MemoryReclaimState>()
            .init_non_send_resource::<ReclaimerRegistration>()
            .add_systems(First, snapshot_pressure);
    }

    fn finish(&self, app: &mut App) {
        let mut registered = app
            .world_mut()
            .remove_non_send_resource::<ReclaimerRegistration>()
            .unwrap()
            .0;
        registered.sort_by_key(|(spec, _)| (spec.priority, spec.name));
        let mut budget = ReclaimBudget::default();
        let mut configs = vec![begin_frame.into_configs()];
        for (spec, system) in registered {
            budget.caches.push(ReclaimStats {
                spec,
                entries: 0,
                released: 0,
                inspected: 0,
                last_released: 0,
                last_inspected: 0,
                scan_status: ReclaimScanStatus::NotRun,
                completed_scans: 0,
                last_scan_released: None,
            });
            configs.push(system);
        }
        configs.push(end_frame.into_configs());
        app.insert_resource(budget).add_systems(
            Last,
            ScheduleConfigs::Configs {
                configs,
                collective_conditions: vec![],
                metadata: Default::default(),
            }
            .chain()
            .before(bevy::diagnostic::update_frame_count),
        );
    }
}

fn latch(active: bool, pressure: Option<MemoryPressure>) -> bool {
    match pressure {
        Some(MemoryPressure::Warning | MemoryPressure::Full) => true,
        Some(MemoryPressure::Normal) => active,
        _ => false,
    }
}

fn gpu_pressure(budget: &GpuMemoryBudget) -> Option<MemoryPressure> {
    let GpuMemoryBudget::Available(pools) = budget else {
        return None;
    };
    pools
        .iter()
        .map(|pool| {
            // Integer cross multiplication avoids rounding and u64 overflow.
            let used = u128::from(pool.used_bytes) * 100;
            let budget = u128::from(pool.budget_bytes.get());
            if used > budget * 75 {
                3
            } else if used > budget * 70 {
                2
            } else if used < budget * 65 {
                0
            } else {
                1
            }
        })
        .max()
        .map(|tier| match tier {
            0 => MemoryPressure::Abundance,
            1 => MemoryPressure::Normal,
            2 => MemoryPressure::Warning,
            _ => MemoryPressure::Full,
        })
}

fn snapshot_pressure(
    ram: Option<Res<MemoryStatus>>,
    gpu: Option<Res<GpuMemoryBudget>>,
    settings: Res<MemoryReclaimSettings>,
    mut state: ResMut<MemoryReclaimState>,
) {
    state.ram = ram
        .and_then(|ram| ram.reading())
        .map(|reading| reading.pressure());
    state.gpu = gpu.as_deref().and_then(gpu_pressure);
    state.ram_latched = latch(state.ram_latched, state.ram);
    state.gpu_latched = latch(state.gpu_latched, state.gpu);
    state.ram_active = state.ram_latched;
    state.gpu_active = state.gpu_latched;
    if matches!(
        settings.simulation,
        ReclaimSimulation::Ram | ReclaimSimulation::Both
    ) {
        state.ram_active = true;
    }
    if matches!(
        settings.simulation,
        ReclaimSimulation::Gpu | ReclaimSimulation::Both
    ) {
        state.gpu_active = true;
    }
}

fn begin_frame(
    mut settings: ResMut<MemoryReclaimSettings>,
    mut state: ResMut<MemoryReclaimState>,
    mut budget: ResMut<ReclaimBudget>,
    preview: Option<Res<PreviewMode>>,
) {
    // Preview can end in PostUpdate, after the pressure snapshot.
    if preview.is_some_and(|preview| !preview.is_preview) {
        *settings = MemoryReclaimSettings::default();
        state.ram_active = state.ram_latched;
        state.gpu_active = state.gpu_latched;
    }
    let now = Instant::now();
    budget.started = Some(now);
    budget.deadline = Some(now + settings.frame_budget);
    budget.start_cache = budget.next_cache;
    budget.ram = state.ram_active;
    budget.gpu = state.gpu_active;
    budget.budget_exhausted = false;
    for cache in &mut budget.caches {
        cache.last_released = 0;
        cache.last_inspected = 0;
        cache.scan_status = ReclaimScanStatus::NotRun;
    }
}

fn end_frame(mut budget: ResMut<ReclaimBudget>) {
    budget.elapsed = budget
        .started
        .map_or(Duration::ZERO, |started| started.elapsed());
    budget.budget_exhausted |= (budget.ram || budget.gpu)
        && budget
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline);
    if !budget.budget_exhausted {
        budget.next_cache = 0;
    }
}

#[cfg(test)]
#[path = "memory_reclaim_tests.rs"]
mod tests;
