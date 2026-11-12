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

#[test]
fn leaving_preview_discards_overrides_before_the_same_frames_cleanup() {
    use crate::structs::PreviewMode;
    use bevy::ecs::schedule::common_conditions::run_once;

    for (measured_ram, measured_gpu) in [(false, false), (true, false), (false, true)] {
        let mut app = app();
        app.init_resource::<Visits>()
            .add_memory_reclaimer(spec("a", 10, ReclaimDomains::RamAndGpu), clean_a)
            .insert_resource(PreviewMode {
                is_preview: true,
                ..Default::default()
            })
            .insert_resource(MemoryReclaimSettings {
                frame_budget: Duration::from_millis(16),
                simulation: ReclaimSimulation::Both,
            });
        ready(&mut app);
        ram(&mut app, Some(if measured_ram { 800 } else { 400 }));
        gpu(&mut app, &[if measured_gpu { 800 } else { 400 }]);
        app.update();
        let settings = app.world().resource::<MemoryReclaimSettings>();
        assert_eq!(settings.simulation, ReclaimSimulation::Both);
        assert_eq!(settings.frame_budget, Duration::from_millis(16));

        // A real pressure latch must survive at Normal when the simulation ends.
        ram(&mut app, Some(if measured_ram { 680 } else { 400 }));
        gpu(&mut app, &[if measured_gpu { 680 } else { 400 }]);
        // Realm changes revoke preview in PostUpdate, after the First snapshot.
        app.add_systems(
            PostUpdate,
            (|mut preview: ResMut<PreviewMode>| preview.is_preview = false).run_if(run_once),
        );
        app.update();

        let settings = app.world().resource::<MemoryReclaimSettings>();
        assert_eq!(
            settings.simulation,
            ReclaimSimulation::Off,
            "preview overrides must expire in the realm-change frame"
        );
        assert_eq!(settings.frame_budget, Duration::from_millis(1));
        let state = app.world().resource::<MemoryReclaimState>();
        assert_eq!(
            (state.ram_active, state.gpu_active),
            (measured_ram, measured_gpu)
        );
        assert_eq!(
            state.ram,
            Some(if measured_ram {
                MemoryPressure::Normal
            } else {
                MemoryPressure::Abundance
            })
        );
        assert_eq!(
            state.gpu,
            Some(if measured_gpu {
                MemoryPressure::Normal
            } else {
                MemoryPressure::Abundance
            })
        );
        let budget = app.world().resource::<ReclaimBudget>();
        assert_eq!((budget.ram, budget.gpu), (measured_ram, measured_gpu));
        if measured_ram || measured_gpu {
            assert_eq!(
                budget.deadline.unwrap() - budget.started.unwrap(),
                Duration::from_millis(1),
                "the same frame's cleanup must use the restored allowance"
            );
        } else {
            assert!(budget.started.is_none() && budget.deadline.is_none());
            assert_eq!(budget.elapsed, Duration::ZERO);
            assert_eq!(budget.caches()[0].scan_status, ReclaimScanStatus::NotRun);
        }

        app.world_mut().resource_mut::<PreviewMode>().is_preview = true;
        app.update();
        let settings = app.world().resource::<MemoryReclaimSettings>();
        assert_eq!(settings.simulation, ReclaimSimulation::Off);
        assert_eq!(settings.frame_budget, Duration::from_millis(1));
        let state = app.world().resource::<MemoryReclaimState>();
        assert_eq!(
            (state.ram_active, state.gpu_active),
            (measured_ram, measured_gpu)
        );
    }
}

#[test]
fn restoring_platform_capacity_discards_the_preview_pressure_latch() {
    let mut app = app();
    app.insert_resource(PreviewMode {
        is_preview: true,
        ..default()
    });
    ready(&mut app);
    ram(&mut app, Some(680));
    app.world_mut().resource_mut::<MemoryStatus>().limit_mib = NonZeroU64::new(500);
    app.update();
    assert!(app.world().resource::<MemoryReclaimState>().ram_active);

    app.world_mut().resource_mut::<MemoryStatus>().limit_mib = None;
    app.update();
    let state = app.world().resource::<MemoryReclaimState>();
    assert_eq!(state.ram, Some(MemoryPressure::Normal));
    assert!(!state.ram_active);
    assert!(!app.world().resource::<ReclaimBudget>().ram);
}

#[test]
fn changing_preview_limits_discards_the_previous_pressure_latch() {
    let mut app = app();
    app.init_resource::<Visits>()
        .add_memory_reclaimer(spec("a", 10, ReclaimDomains::Ram), clean_a)
        .insert_resource(PreviewMode {
            is_preview: true,
            ..default()
        });
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = Duration::from_secs(10);
    ready(&mut app);

    for (limit, used, cap, pressure, active) in [
        (Some(500), 680, 500, MemoryPressure::Full, true),
        (Some(2_000), 680, 1_000, MemoryPressure::Normal, false),
        (Some(950), 680, 950, MemoryPressure::Warning, true),
        (Some(950), 640, 950, MemoryPressure::Normal, true),
        (Some(1_000), 680, 1_000, MemoryPressure::Normal, false),
        (Some(900), 680, 900, MemoryPressure::Full, true),
        (Some(2_000), 720, 1_000, MemoryPressure::Warning, true),
        (Some(500), 800, 500, MemoryPressure::Full, true),
        (Some(2_000), 800, 1_000, MemoryPressure::Full, true),
        (None, 680, 1_000, MemoryPressure::Normal, true),
    ] {
        ram(&mut app, Some(used));
        app.world_mut().resource_mut::<MemoryStatus>().limit_mib = limit.and_then(NonZeroU64::new);
        app.world_mut().resource_mut::<Visits>().0.clear();
        app.update();

        assert_eq!(
            app.world()
                .resource::<MemoryStatus>()
                .reading()
                .unwrap()
                .cap_mib,
            cap
        );
        let state = app.world().resource::<MemoryReclaimState>();
        assert_eq!(state.ram, Some(pressure), "limit {limit:?}, used {used}");
        assert_eq!(state.ram_active, active, "limit {limit:?}, used {used}");
        assert_eq!(app.world().resource::<ReclaimBudget>().ram, active);
        assert_eq!(app.world().resource::<Visits>().0.is_empty(), !active);
    }
}

#[test]
fn ineffective_preview_limits_preserve_real_pressure_until_abundance() {
    use bevy::ecs::schedule::common_conditions::run_once;

    let mut app = app();
    app.insert_resource(PreviewMode {
        is_preview: true,
        ..default()
    });
    ready(&mut app);
    ram(&mut app, Some(800));
    app.update();
    ram(&mut app, Some(680));
    app.update();
    assert!(app.world().resource::<MemoryReclaimState>().ram_active);

    for limit in [Some(2_000), Some(3_000), None] {
        app.world_mut().resource_mut::<MemoryStatus>().limit_mib = limit.and_then(NonZeroU64::new);
        app.update();
        let state = app.world().resource::<MemoryReclaimState>();
        assert_eq!(state.ram, Some(MemoryPressure::Normal));
        assert!(
            state.ram_active,
            "ineffective limit {limit:?} cleared real pressure"
        );
        assert!(app.world().resource::<ReclaimBudget>().ram);
    }

    app.world_mut().resource_mut::<MemoryStatus>().limit_mib = NonZeroU64::new(3_000);
    app.add_systems(
        PostUpdate,
        (|mut preview: ResMut<PreviewMode>| preview.is_preview = false).run_if(run_once),
    );
    app.update();
    assert!(app.world().resource::<MemoryStatus>().limit_mib.is_none());
    assert!(app.world().resource::<MemoryReclaimState>().ram_active);
    assert!(app.world().resource::<ReclaimBudget>().ram);

    ram(&mut app, Some(649));
    app.update();
    assert!(!app.world().resource::<MemoryReclaimState>().ram_active);
    assert!(!app.world().resource::<ReclaimBudget>().ram);
}

#[test]
fn leaving_preview_restores_platform_capacity_before_the_same_frames_cleanup() {
    use bevy::ecs::schedule::common_conditions::run_once;

    for (used, pressure, active) in [
        (400, MemoryPressure::Abundance, false),
        (680, MemoryPressure::Normal, false),
        (800, MemoryPressure::Full, true),
    ] {
        let mut app = app();
        app.init_resource::<Visits>()
            .add_memory_reclaimer(spec("a", 10, ReclaimDomains::Ram), clean_a)
            .insert_resource(PreviewMode {
                is_preview: true,
                ..default()
            });
        ready(&mut app);
        ram(&mut app, Some(used));
        app.update();
        app.world_mut().resource_mut::<MemoryStatus>().limit_mib = NonZeroU64::new(500);
        app.update();
        assert_eq!(
            app.world().resource::<MemoryReclaimState>().ram,
            Some(MemoryPressure::Full)
        );
        assert!(app.world().resource::<MemoryReclaimState>().ram_active);
        app.world_mut().resource_mut::<Visits>().0.clear();

        app.add_systems(
            PostUpdate,
            (|mut preview: ResMut<PreviewMode>| preview.is_preview = false).run_if(run_once),
        );
        app.update();

        let memory = app.world().resource::<MemoryStatus>();
        assert!(memory.limit_mib.is_none());
        assert_eq!(memory.reading().unwrap().cap_mib, 1_000);
        let state = app.world().resource::<MemoryReclaimState>();
        assert_eq!(state.ram, Some(pressure));
        assert_eq!(state.ram_active, active);
        assert_eq!(app.world().resource::<ReclaimBudget>().ram, active);
        assert_eq!(app.world().resource::<Visits>().0.is_empty(), !active);

        app.world_mut().resource_mut::<PreviewMode>().is_preview = true;
        app.update();
        assert!(app.world().resource::<MemoryStatus>().limit_mib.is_none());
        assert_eq!(
            app.world().resource::<MemoryReclaimState>().ram_active,
            active
        );
    }
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
#[should_panic(expected = "unregistered memory reclaimer: missing")]
fn an_unregistered_cache_name_is_reported_even_without_pressure() {
    let mut app = app();
    app.add_memory_reclaimer(spec("registered", 10, ReclaimDomains::Ram), || {});
    ready(&mut app);
    let mut budget = app.world_mut().resource_mut::<ReclaimBudget>();
    assert!(budget.begin("registered", 10).is_none());
    let _ = budget.begin("missing", 10);
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

#[test]
fn unrelated_last_work_cannot_exhaust_the_allowance_before_the_first_cache() {
    let mut app = app();
    app.init_resource::<Visits>()
        .add_memory_reclaimer(spec("a", 10, ReclaimDomains::Ram), clean_a)
        .add_memory_reclaimer(spec("b", 20, ReclaimDomains::Ram), clean_b)
        .add_systems(
            Last,
            (|mut budget: ResMut<ReclaimBudget>| {
                // Model unrelated Last work outlasting any already-started allowance,
                // without depending on wall-clock delays or scheduler speed.
                budget.deadline = budget.deadline.map(|_| Instant::now());
            })
            .after(begin_frame)
            .before(clean_a),
        );
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = Duration::from_secs(10);
    ready(&mut app);
    ram(&mut app, Some(800));
    app.update();

    assert_eq!(app.world().resource::<Visits>().0, ["a", "a", "b", "b"]);
    let budget = app.world().resource::<ReclaimBudget>();
    assert_eq!(budget.caches()[0].inspected, 2);
    assert_eq!(budget.caches()[1].inspected, 2);
    assert!(!budget.budget_exhausted);
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
fn gpu_pressure_wraps_past_an_ineligible_tail_without_an_idle_frame() {
    for initial_ram_pressure in [false, true] {
        let mut app = app();
        app.init_resource::<Visits>()
            .add_memory_reclaimer(spec("a", 10, ReclaimDomains::RamAndGpu), slow_a)
            .add_memory_reclaimer(
                spec("gpu-tail", 20, ReclaimDomains::RamAndGpu),
                |mut budget: ResMut<ReclaimBudget>, mut visits: ResMut<Visits>| {
                    if let Some(mut turn) = budget.begin("gpu-tail", 10) {
                        if turn.inspect() {
                            visits.0.push("gpu-tail");
                            budget.deadline = Some(Instant::now());
                        }
                        budget.finish_turn(turn, 10);
                    }
                },
            )
            .add_memory_reclaimer(spec("b", 30, ReclaimDomains::Ram), clean_b);
        app.world_mut()
            .resource_mut::<MemoryReclaimSettings>()
            .frame_budget = Duration::from_secs(10);
        ready(&mut app);
        ram(&mut app, Some(if initial_ram_pressure { 800 } else { 400 }));
        gpu(&mut app, &[800]);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Visits>().0, ["a", "gpu-tail"]);

        // The next cursor is the RAM-only tail, including when RAM pressure just ended.
        ram(&mut app, Some(400));
        app.update();
        assert_eq!(
            app.world().resource::<Visits>().0,
            ["a", "gpu-tail", "a"],
            "GPU cleanup must resume in the next frame without visiting the RAM-only tail"
        );
        app.update();
        assert_eq!(
            app.world().resource::<Visits>().0,
            ["a", "gpu-tail", "a", "gpu-tail"]
        );
    }
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
