use super::*;
use crate::memory::{GpuMemoryPool, GpuMemoryPoolKind, MemorySample};
use std::num::NonZeroU64;

const MIB: u64 = 1 << 20;

fn ram(app: &mut App, used_mib: Option<u64>) {
    MemoryStatus::record(
        app.world_mut().resource_mut::<MemoryStatus>(),
        used_mib.map(|used| MemorySample {
            used_bytes: used * MIB,
            capacity_bytes: 1_000 * MIB,
        }),
    );
}

fn gpu(app: &mut App, used: &[u64]) {
    app.insert_resource(GpuMemoryBudget::Available(
        used.iter()
            .enumerate()
            .map(|(index, used)| GpuMemoryPool {
                kind: GpuMemoryPoolKind::VulkanDeviceLocal { heap_index: index },
                used_bytes: *used,
                budget_bytes: NonZeroU64::new(1_000).unwrap(),
            })
            .collect(),
    ));
}

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<MemoryStatus>()
        .insert_resource(GpuMemoryBudget::NotApplicable)
        .add_plugins(MemoryReclaimPlugin);
    app
}

fn ready(app: &mut App) {
    app.finish();
    app.cleanup();
}

#[test]
fn ram_reclamation_latches_through_normal_and_stops_at_abundance() {
    let mut app = app();
    ready(&mut app);
    for (used, active) in [
        (700, false),
        (701, true),
        (700, true),
        (650, true),
        (649, false),
        (750, true),
    ] {
        ram(&mut app, Some(used));
        app.update();
        assert_eq!(
            app.world().resource::<MemoryReclaimState>().ram_active,
            active
        );
    }
}

#[test]
fn gpu_pools_and_ram_recover_independently_without_rounding_bytes() {
    let mut app = app();
    ready(&mut app);
    ram(&mut app, Some(400));
    gpu(&mut app, &[700, 1]);
    app.update();
    assert!(!app.world().resource::<MemoryReclaimState>().gpu_active);
    gpu(&mut app, &[701, 1]);
    app.update();
    assert!(app.world().resource::<MemoryReclaimState>().gpu_active);
    ram(&mut app, Some(800));
    gpu(&mut app, &[649, 650]);
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert!(state.ram_active && state.gpu_active);
    gpu(&mut app, &[649, 649]);
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert!(state.ram_active && !state.gpu_active);
}

#[test]
fn missing_readings_clear_only_their_domain_and_simulation_leaves_samples_intact() {
    let mut app = app();
    ready(&mut app);
    ram(&mut app, Some(800));
    gpu(&mut app, &[800]);
    app.update();
    ram(&mut app, None);
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert!(!state.ram_active && state.gpu_active);
    assert_eq!(state.ram, None);
    gpu(&mut app, &[]);
    app.update();
    assert!(!app.world().resource::<MemoryReclaimState>().gpu_active);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Both;
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert!(state.ram_active && state.gpu_active);
    assert!(app.world().resource::<MemoryStatus>().reading().is_none());
    assert!(
        matches!(app.world().resource::<GpuMemoryBudget>(), GpuMemoryBudget::Available(pools) if pools.is_empty())
    );
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Off;
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert!(!state.ram_active && !state.gpu_active);
}

#[derive(Default, Resource)]
struct Visits(Vec<&'static str>);

#[test]
fn disabling_simulation_does_not_latch_normal_real_readings() {
    let mut app = app();
    ready(&mut app);
    ram(&mut app, Some(680));
    gpu(&mut app, &[680]);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Both;
    app.update();
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Off;
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert!(!state.ram_active && !state.gpu_active);
}

fn clean_a(mut budget: ResMut<ReclaimBudget>, mut visits: ResMut<Visits>) {
    if let Some(mut turn) = budget.begin("a", 10) {
        while turn.inspect() {
            visits.0.push("a");
            turn.released();
        }
        budget.finish_turn(turn, 8);
    }
}

fn clean_b(mut budget: ResMut<ReclaimBudget>, mut visits: ResMut<Visits>) {
    if let Some(mut turn) = budget.begin("b", 10) {
        while turn.inspect() {
            visits.0.push("b");
            turn.released();
        }
        budget.finish_turn(turn, 8);
    }
}

fn spec(name: &'static str, priority: u16, domains: ReclaimDomains) -> ReclaimSpec {
    ReclaimSpec {
        name,
        priority,
        domains,
        max_releases: 2,
        max_inspections: 4,
    }
}

#[test]
fn cache_registration_order_does_not_change_priority_or_release_caps() {
    let mut app = app();
    app.init_resource::<Visits>()
        .add_memory_reclaimer(spec("b", 20, ReclaimDomains::Ram), clean_b)
        .add_memory_reclaimer(spec("a", 10, ReclaimDomains::RamAndGpu), clean_a);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = Duration::from_secs(10);
    ready(&mut app);
    ram(&mut app, Some(800));
    app.update();
    assert_eq!(app.world().resource::<Visits>().0, ["a", "a", "b", "b"]);
    let budget = app.world().resource::<ReclaimBudget>();
    assert_eq!(budget.caches()[0].released, 2);
    assert_eq!(budget.caches()[1].released, 2);
}

#[test]
fn gpu_only_pressure_skips_ram_only_caches_and_no_render_resources_are_required() {
    let mut app = app();
    app.init_resource::<Visits>()
        .add_memory_reclaimer(spec("a", 10, ReclaimDomains::RamAndGpu), clean_a)
        .add_memory_reclaimer(spec("b", 20, ReclaimDomains::Ram), clean_b);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = Duration::from_secs(10);
    ready(&mut app);
    gpu(&mut app, &[800]);
    app.update();
    assert_eq!(app.world().resource::<Visits>().0, ["a", "a"]);
    app.world_mut().remove_resource::<GpuMemoryBudget>();
    app.update();
    assert_eq!(app.world().resource::<Visits>().0.len(), 2);
}

fn slow_a(mut budget: ResMut<ReclaimBudget>, mut visits: ResMut<Visits>) {
    if let Some(mut turn) = budget.begin("a", 10) {
        if turn.inspect() {
            visits.0.push("a");
            // Expire deterministically; scheduler/CI load must not decide this test.
            budget.deadline = Some(Instant::now());
        }
        budget.finish_turn(turn, 10);
    }
}

#[test]
fn exhausted_budget_resumes_at_later_cache_instead_of_starving_it() {
    let mut app = app();
    app.init_resource::<Visits>()
        .add_memory_reclaimer(spec("a", 10, ReclaimDomains::Ram), slow_a)
        .add_memory_reclaimer(spec("b", 20, ReclaimDomains::Ram), clean_b);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = Duration::from_secs(10);
    ready(&mut app);
    ram(&mut app, Some(800));
    app.update();
    assert_eq!(app.world().resource::<Visits>().0, ["a"]);
    assert!(app.world().resource::<ReclaimBudget>().budget_exhausted);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = Duration::from_secs(10);
    app.update();
    assert_eq!(app.world().resource::<Visits>().0, ["a", "b", "b"]);
}

#[test]
fn reclaimers_use_the_same_frame_count_as_update() {
    use bevy::diagnostic::{FrameCount, FrameCountPlugin};
    #[derive(Resource, Default)]
    struct ObservedFrames(Vec<u32>);
    fn record(frame: Res<FrameCount>, mut seen: ResMut<ObservedFrames>) {
        seen.0.push(frame.0);
    }
    for counter_first in [false, true] {
        let mut app = App::new();
        if counter_first {
            app.add_plugins(FrameCountPlugin);
        }
        app.add_plugins(MemoryReclaimPlugin)
            .init_resource::<ObservedFrames>()
            .add_systems(Update, record)
            .add_memory_reclaimer(spec("frame", 10, ReclaimDomains::Ram), record);
        if !counter_first {
            app.add_plugins(FrameCountPlugin);
        }
        ready(&mut app);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<ObservedFrames>().0,
            [0, 0, 1, 1, 2, 2, 3, 3, 4, 4]
        );
    }
}
