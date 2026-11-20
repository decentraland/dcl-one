//! The engine's Memory and limits debug panel section. Its query builds its JSON only when asked.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    memory::{GpuMemoryBudget, MemoryStatus},
    structs::PrimaryUser,
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use scene_material::SceneMaterial;
use scene_runner::{
    visible_content_bytes::scene_visible_content_bytes, ContainerEntity, ContainingScene,
};
use serde_json::{json, Value};

use crate::perf::{frame_inputs::FrameInputs, populating_sample::PerformancePopulatingSample};

const BYTES_PER_MIB: u64 = 1 << 20;

pub(super) struct MemoryDebugPanelPlugin;

impl Plugin for MemoryDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<MemoryCommand, _>(debug_memory)
            .add_debug_section(DebugSection {
                title: "Memory and limits",
                query: "/debug_memory",
                fields: &[
                    ("Engine memory (MiB)", "used"),
                    (
                        "Peak engine memory in the sample (MiB)",
                        "usedPeakMiBInSample",
                    ),
                    ("Platform capacity (MiB)", "capacity"),
                    ("Effective cap (MiB)", "cap"),
                    ("Memory pressure", "pressure"),
                    ("Thresholds (MiB)", "thresholds"),
                    ("GPU memory pools", "gpu"),
                    (
                        "JS heaps (used MiB / limit MiB / external MiB / scenes)",
                        "jsHeap",
                    ),
                    ("Current scene textures (count / MiB)", "textures"),
                    ("Current scene meshes (count / MiB)", "meshes"),
                ],
                actions: &[
                    ("Restore platform limit", "/debug_memory 0"),
                    (
                        "Simulate Warning pressure",
                        "/debug_memory --simulate warning",
                    ),
                    ("Simulate Full pressure", "/debug_memory --simulate full"),
                    ("Stop simulating", "/debug_memory --simulate off"),
                ],
                inputs: &[DebugInput {
                    label: "Session memory limit (MiB)",
                    field: "limit",
                    command: "/debug_memory {value}",
                    kind: DebugInputKind::Number {
                        min: 0.0,
                        max: u32::MAX as f64,
                        step: 1.0,
                    },
                }],
            });
    }
}

/// Inspect the engine memory budget, set its session limit in MiB (zero restores platform
/// capacity), or simulate a pressure tier by setting the limit from the current use.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_memory")]
struct MemoryCommand {
    limit_mib: Option<u32>,
    #[arg(long, value_enum)]
    simulate: Option<SimulatedMemoryPressure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum SimulatedMemoryPressure {
    /// A cap that puts the current use at 72 %: the Warning tier.
    Warning,
    /// A cap that puts the current use at 80 %: the Full tier.
    Full,
    /// The platform's capacity again.
    Off,
}

fn debug_memory(
    mut cmd: ConsoleCommand<MemoryCommand>,
    mut memory: ResMut<MemoryStatus>,
    gpu: Option<Res<GpuMemoryBudget>>,
    sample: Res<PerformancePopulatingSample>,
    inputs: Res<FrameInputs>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    mesh_handles: Query<(&Mesh3d, &ContainerEntity, &Visibility)>,
    material_handles: Query<(
        &MeshMaterial3d<SceneMaterial>,
        &ContainerEntity,
        &Visibility,
    )>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<SceneMaterial>>,
    images: Res<Assets<Image>>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if let Some(simulate) = args.simulate {
        let used_mib = memory
            .reading()
            .map(|reading| reading.used_bytes / BYTES_PER_MIB);
        let limit = match (simulate, used_mib) {
            (SimulatedMemoryPressure::Off, _) => None,
            (_, None) => {
                cmd.reply_failed("No platform memory sample to simulate from");
                return;
            }
            (SimulatedMemoryPressure::Warning, Some(used)) => {
                Some((used as f64 / 0.72).ceil() as u64)
            }
            (SimulatedMemoryPressure::Full, Some(used)) => Some((used as f64 / 0.80).ceil() as u64),
        };
        // a limit at or above the capacity is refused: it is no lower cap
        if let Some((limit, reading)) = limit.zip(memory.reading()) {
            if limit >= reading.capacity_mib {
                cmd.reply_failed(format!(
                    "Current use ({} MiB) is already past the tier's share of the capacity ({} MiB): a limit of {limit} MiB would not lower the cap",
                    reading.used_bytes / BYTES_PER_MIB,
                    reading.capacity_mib,
                ));
                return;
            }
        }
        memory.limit_mib = limit.and_then(std::num::NonZeroU64::new);
        cmd.reply_ok(
            json!({
                "limit": memory.limit_mib.map_or(0, |n| n.get()),
                "pressure": memory.reading().map(|r| format!("{:?}", r.pressure())),
            })
            .to_string(),
        );
        return;
    }
    if let Some(limit) = args.limit_mib {
        memory.limit_mib = std::num::NonZeroU64::new(limit.into());
        cmd.reply_ok("Updated session memory limit");
        return;
    }
    let reading = memory.reading();
    let pools = match gpu.as_deref() {
        Some(GpuMemoryBudget::Available(pools)) => Some(pools.iter().map(|p| json!({
            "pool": format!("{:?}", p.kind), "usedMiB": p.used_bytes >> 20, "budgetMiB": p.budget_bytes.get() >> 20
        })).collect::<Vec<_>>()),
        _ => None,
    };
    let js_heap = inputs.js_heap;
    let current_scene_content = player
        .single()
        .ok()
        .and_then(|player| containing_scene.get_parcel(player))
        .map(|scene| {
            scene_visible_content_bytes(
                scene,
                mesh_handles.iter(),
                material_handles.iter(),
                &meshes,
                &materials,
                &images,
            )
        });
    let count_and_mib = |count: u32, bytes: u64| -> Value {
        json!([
            count,
            (bytes as f64 / BYTES_PER_MIB as f64 * 10.0).round() / 10.0
        ])
    };
    let ram_reason = "No platform memory sample.";
    cmd.reply_ok(json!({
        "used": reading.map(|r| r.used_bytes >> 20), "capacity": reading.map(|r| r.capacity_mib),
        "usedPeakMiBInSample": reading
            .and(sample.used_peak_bytes())
            .map(|bytes| bytes >> 20),
        "cap": reading.map(|r| r.cap_mib), "limit": memory.limit_mib.map_or(0, |n| n.get()),
        "pressure": reading.map(|r| format!("{:?}", r.pressure())), "gpu": pools,
        "thresholds": reading.map(|r| { let t = r.thresholds(); json!({
            "abundance": t.abundance_mib.round(), "warning": t.warning_mib.round(), "full": t.full_mib.round()
        }) }),
        "jsHeap": js_heap.map(|heap| json!([
            heap.used_bytes_sum >> 20, heap.limit_bytes_sum >> 20, heap.external_bytes_sum >> 20, heap.reporting_scene_count
        ])),
        "textures": current_scene_content.map(|c| count_and_mib(c.texture_count, c.texture_bytes_sum)),
        "meshes": current_scene_content.map(|c| count_and_mib(c.mesh_count, c.mesh_bytes_sum)),
        "unavailable": {
            "used":ram_reason, "capacity":ram_reason, "cap":ram_reason,
            "usedPeakMiBInSample": if reading.is_some() { "Waiting for the first sampled frame." } else { ram_reason },
            "pressure":ram_reason, "thresholds":ram_reason,
            "gpu":match gpu.as_deref() {
                Some(GpuMemoryBudget::NotApplicable) => "This platform does not expose a GPU memory budget.",
                _ => "The graphics driver has not provided a GPU memory budget.",
            },
            "jsHeap": if cfg!(target_arch = "wasm32") { "No running scene reports a heap: this browser has no memory API." } else { "No running scene has reported a heap yet." },
            "textures": "The player is not on a parcel scene.",
            "meshes": "The player is not on a parcel scene.",
        }
    }).to_string());
}
