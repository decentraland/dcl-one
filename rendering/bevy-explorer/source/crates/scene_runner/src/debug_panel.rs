use std::collections::{HashSet, VecDeque};

use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::PrimitiveTopology};
use bevy_console::ConsoleCommand;
use common::structs::PrimaryUser;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_types::interface::CrdtStore;
use scene_material::SceneMaterial;
use serde_json::{json, Value};
use web_time::{Duration, Instant};

use crate::{
    asset_preload::plugin::{LoadingPreloadedAsset, PreloadedAsset},
    initialize_scene::{CurrentSceneLoading, SceneLoading, SuperUserScene, PARCEL_SIZE},
    renderer_context::RendererSceneContext,
    update_world::{
        gltf_container::{GltfLoadingCount, SceneResourceLookup},
        mesh_collider::SceneColliderData,
    },
    ContainerEntity, ContainingScene,
};

pub(super) fn register(app: &mut App) {
    app.init_resource::<ShowBounds>()
        .add_console_command::<BoundsCommand, _>(set_bounds)
        .add_systems(Update, draw_bounds.run_if(|show: Res<ShowBounds>| show.0))
        .add_console_command::<SceneState, _>(scene_state)
        .add_console_command::<SceneContent, _>(scene_content)
        .add_console_command::<LoadingState, _>(loading_state)
        .add_console_command::<TextureState, _>(texture_state)
        .add_debug_section(DebugSection {
            title: "Current Scene",
            query: "/current_scene_state",
            fields: &[
                ("Name", "name"),
                ("SDK", "sdk"),
                ("Parcels", "parcels"),
                ("Height (m)", "height"),
                ("Global position (m)", "world"),
                ("Scene position (m)", "relative"),
                ("Tick", "tick"),
                ("Scene tick FPS", "history.fps"),
                ("Minimum FPS (last 256 observed ticks)", "history.minFps"),
                ("Maximum FPS (last 256 observed ticks)", "history.maxFps"),
                (
                    "Hiccups over 50 ms (last 256 observed ticks)",
                    "history.hiccups",
                ),
                ("Tick FPS history", "history.tickFps"),
                (
                    "Component payload bytes/tick from scene",
                    "history.from.bytes",
                ),
                ("Component updates/tick from scene", "history.from.updates"),
                ("Component payload bytes/tick to scene", "history.to.bytes"),
                ("Component updates/tick to scene", "history.to.updates"),
                (
                    "From scene in chart window (bytes / updates)",
                    "history.from.total",
                ),
                (
                    "To scene in chart window (bytes / updates)",
                    "history.to.total",
                ),
                (
                    "From scene per second (bytes / updates)",
                    "history.from.rate",
                ),
                ("To scene per second (bytes / updates)", "history.to.rate"),
                (
                    "Updates/tick from scene (min / max / spikes)",
                    "history.from.range",
                ),
                (
                    "Updates/tick to scene (min / max / spikes)",
                    "history.to.range",
                ),
                ("CRDT bytes from scene", "worker.crdt_bytes"),
                ("CRDT bytes/s from scene", "bytesPerSecond"),
                ("IPC responses from scene", "worker.ipc_responses"),
                ("Worker fetches started", "worker.fetch_started"),
                ("Worker fetches completed", "worker.fetch_completed"),
                ("Worker fetches failed", "worker.fetch_failed"),
                ("Worker fetch bytes received", "worker.fetch_bytes_down"),
                ("Current native V8 heap used (bytes)", "heap"),
                ("Current native V8 heap limit (bytes)", "heapLimit"),
                (
                    "Current native V8 external memory (bytes)",
                    "externalMemory",
                ),
                ("Sampled native JS engines", "engines.count"),
                ("Sampled native JS heaps used (bytes)", "engines.used"),
                (
                    "Sampled native JS external memory (bytes)",
                    "engines.external",
                ),
            ],
            actions: &[],
            inputs: &[DebugInput {
                label: "Show scene bounds",
                field: "bounds",
                command: "/show_parcel_scene_bounds {value}",
                kind: DebugInputKind::Toggle,
            }],
        })
        .add_debug_section(DebugSection {
            title: "Scene Content",
            query: "/current_scene_content",
            fields: &[
                ("Entities known to renderer", "entities"),
                ("Mesh bodies (all)", "allBodies"),
                ("Triangles in mesh bodies (all)", "allTriangles"),
                ("Visible triangles", "triangles"),
                ("Visible meshes (bodies)", "bodies"),
                ("Unique geometries", "geometries"),
                ("Visible unique geometries", "visibleGeometries"),
                ("Unique materials", "materials"),
                ("Visible unique materials", "visibleMaterials"),
                ("Material textures", "textures"),
                ("Colliders", "colliders"),
            ],
            actions: &[],
            inputs: &[],
        })
        .add_debug_section(DebugSection {
            title: "Scene Loading",
            query: "/scene_loading_state",
            fields: &[
                ("Current parcel loading", "current"),
                ("Current parcel stage", "stage"),
                ("Pending glTF containers", "gltf"),
                ("Pending SDK asset preloads", "preloads"),
                ("Spawning", "spawned"),
                ("Reading definition", "definition"),
                ("Reading main.crdt", "crdt"),
                ("Starting JavaScript", "javascript"),
                ("Failed", "failed"),
                ("Running scenes", "running"),
                ("Blocked scenes", "blocked"),
                ("Broken scenes", "broken"),
            ],
            actions: &[("Reload current parcel scene", "/reload --current")],
            inputs: &[],
        })
        .add_debug_section(DebugSection {
            title: "Resource Promises",
            query: "/scene_loading_state",
            fields: &[
                ("Pending glTF containers", "gltf"),
                ("Pending SDK asset preloads", "preloads"),
                ("Pending texture preloads", "texturePreloads"),
            ],
            actions: &[],
            inputs: &[],
        })
        .add_debug_section(DebugSection {
            title: "Cache Textures",
            query: "/texture_cache_state",
            fields: &[
                ("Resident image assets", "count"),
                ("Retained image bytes (CPU)", "bytes"),
            ],
            actions: &[],
            inputs: &[],
        });
}

#[derive(Resource, Default)]
struct ShowBounds(bool);

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/show_parcel_scene_bounds")]
struct BoundsCommand {
    #[arg(action = clap::ArgAction::Set)]
    enabled: bool,
}

fn set_bounds(mut input: ConsoleCommand<BoundsCommand>, mut bounds: ResMut<ShowBounds>) {
    if let Some(Ok(command)) = input.take() {
        bounds.0 = command.enabled;
        input.reply_ok(format!(
            "Scene bounds {}",
            if bounds.0 { "on" } else { "off" }
        ));
    }
}

fn draw_bounds(
    containing: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    scenes: Query<&RendererSceneContext, Without<SuperUserScene>>,
    mut gizmos: Gizmos,
) {
    let Some(scene) = player
        .single()
        .ok()
        .and_then(|p| containing.get_parcel(p))
        .and_then(|e| scenes.get(e).ok())
        .filter(|s| !s.is_portable)
    else {
        return;
    };
    for region in &scene.bounds {
        let center = (region.world_min() + region.world_max()) * 0.5;
        gizmos.cuboid(
            Transform::from_translation(center).with_scale(region.world_size()),
            Color::WHITE,
        );
    }
}

#[derive(Clone, Copy, Default)]
struct Batch {
    bytes: usize,
    updates: usize,
}

impl Batch {
    fn measure(crdt: &CrdtStore) -> Self {
        let mut batch = Self::default();
        for state in crdt.lww.values() {
            batch.updates += state.last_write.len();
            batch.bytes += state
                .last_write
                .values()
                .filter(|v| v.is_some)
                .map(|v| v.data.len())
                .sum::<usize>();
        }
        for values in crdt.go.values().flat_map(|state| state.0.values()) {
            batch.updates += values.len();
            batch.bytes += values.iter().map(|v| v.data.len()).sum::<usize>();
        }
        batch
    }
}

struct Tick {
    dt: f64,
    sent: Batch,
    received: Batch,
}

/// Sampling is attached only to the queried parcel. Stop after the HUD stops reading it.
#[derive(Component)]
pub(super) struct SceneTraffic {
    last_read: Instant,
    sent: Option<Batch>,
    ticks: VecDeque<Tick>,
}

impl SceneTraffic {
    fn new() -> Self {
        Self {
            last_read: Instant::now(),
            sent: None,
            ticks: VecDeque::with_capacity(256),
        }
    }
    pub(super) fn expired(&self) -> bool {
        self.last_read.elapsed() > Duration::from_secs(4)
    }

    pub(super) fn record_sent(&mut self, crdt: &CrdtStore) {
        if !self.expired() {
            self.sent = Some(Batch::measure(crdt));
        }
    }
    pub(super) fn record_received(&mut self, crdt: &CrdtStore, dt: f64) {
        let Some(sent) = self.sent.take() else {
            return;
        };
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if self.ticks.len() == 256 {
            self.ticks.pop_front();
        }
        self.ticks.push_back(Tick {
            dt,
            sent,
            received: Batch::measure(crdt),
        });
    }
    fn report(&mut self) -> Value {
        if self.expired() {
            self.ticks.clear();
            self.sent = None;
        }
        self.last_read = Instant::now();
        if self.ticks.is_empty() {
            return Value::Null;
        }
        let seconds = self.ticks.iter().map(|tick| tick.dt).sum::<f64>();
        let direction = |batch: fn(&Tick) -> Batch| {
            let bytes: Vec<usize> = self.ticks.iter().map(|tick| batch(tick).bytes).collect();
            let updates: Vec<usize> = self.ticks.iter().map(|tick| batch(tick).updates).collect();
            let total_updates = updates.iter().map(|v| *v as u64).sum::<u64>();
            let total_bytes = bytes.iter().map(|v| *v as u64).sum::<u64>();
            let mean = total_updates as f64 / updates.len() as f64;
            let spikes = updates
                .iter()
                .filter(|count| **count as f64 > 2.0 * mean)
                .count();
            json!({
                "range": [updates.iter().min().copied(), updates.iter().max().copied(), Some(spikes)],
                "total": [total_bytes, total_updates],
                "rate": [(total_bytes as f64 / seconds).round(), (total_updates as f64 / seconds).round()],
                "bytes": {"unit":"bytes/tick","samples":bytes},
                "updates": {"unit":"updates/tick","samples":updates}
            })
        };
        let fps: Vec<f64> = self
            .ticks
            .iter()
            .map(|tick| (1.0 / tick.dt).round())
            .collect();
        let recent = self.ticks.len().min(32);
        json!({"fps": (recent as f64 / self.ticks.iter().rev().take(recent).map(|t|t.dt).sum::<f64>()).round(),
            "minFps":fps.iter().copied().reduce(f64::min), "maxFps":fps.iter().copied().reduce(f64::max),
            "hiccups":self.ticks.iter().filter(|t|t.dt>0.05).count(), "tickFps":{"unit":"FPS", "samples":fps},
            "from":direction(|t|t.received), "to":direction(|t|t.sent)})
    }
}

/// Runtime counters for the player's parcel scene.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/current_scene_state")]
struct SceneState;

fn scene_state(
    mut input: ConsoleCommand<SceneState>,
    player: Query<(Entity, &GlobalTransform), With<PrimaryUser>>,
    containing: ContainingScene,
    scenes: Query<(&RendererSceneContext, Has<SuperUserScene>)>,
    bounds: Res<ShowBounds>,
    mut histories: Query<&mut SceneTraffic>,
    mut commands: Commands,
) {
    if input.take().is_none() {
        return;
    }
    // Only Deno's native V8 isolates publish heap samples. A dead scene can retain its last
    // snapshot for inspection but is no longer a live engine.
    let engines = if cfg!(target_arch = "wasm32") {
        Value::Null
    } else {
        let (mut count, mut used, mut external) = (0usize, 0u64, 0u64);
        for c in scenes
            .iter()
            .filter(|(s, _)| s.sender().is_some())
            .filter_map(|(s, _)| s.resource_counters.as_ref())
        {
            count += 1;
            used += c.heap_used;
            external += c.external_memory;
        }
        json!({"count":count, "used":(count > 0).then_some(used), "external": (count > 0).then_some(external)})
    };
    let Some((transform, scene)) = player.single().ok().and_then(|(id, t)| {
        containing
            .get_parcel(id)
            .and_then(|e| scenes.get(e).ok())
            .filter(|(s, privileged)| !*privileged && !s.is_portable)
            .map(|(s, _)| (t, s))
    }) else {
        input.reply_ok(json!({"bounds": bounds.0, "engines": engines}).to_string());
        return;
    };
    let p = transform.translation() * Vec3::new(1.0, 1.0, -1.0);
    let base = Vec3::new(
        scene.base.x as f32 * PARCEL_SIZE,
        0.0,
        scene.base.y as f32 * PARCEL_SIZE,
    );
    let root = scene.scene_id.0;
    let history = match histories.get_mut(root) {
        Ok(mut history) => history.report(),
        Err(_) => {
            commands.entity(root).try_insert(SceneTraffic::new());
            Value::Null
        }
    };
    // Browser scene workers do not publish V8 heap readings or the native fetch counters.
    let report = json!({
        "bounds": bounds.0, "name": scene.title, "sdk": scene.sdk_version, "parcels": scene.parcels.len(),
        "height": scene.bounds.iter().map(|r| r.height).reduce(f32::max),
        "world": p.to_array(), "relative": (p-base).to_array(), "tick": scene.tick_number,
        "history": history, "engines": engines,
        "bytesPerSecond": scene.crdt_bytes_per_second.map(f64::round),
        "worker": scene.resource_counters.as_ref().map(|c| json!({"crdt_bytes": c.crdt_bytes, "ipc_responses": c.ipc_responses})),
    });
    #[cfg(not(target_arch = "wasm32"))]
    let report = {
        let mut report = report;
        if let Some(c) = &scene.resource_counters {
            report["worker"]["fetch_started"] = c.fetch_started.into();
            report["worker"]["fetch_completed"] = c.fetch_completed.into();
            report["worker"]["fetch_failed"] = c.fetch_failed.into();
            report["worker"]["fetch_bytes_down"] = c.fetch_bytes_down.into();
            report["heap"] = c.heap_used.into();
            report["heapLimit"] = c.heap_limit.into();
            report["externalMemory"] = c.external_memory.into();
        }
        report
    };
    input.reply_ok(report.to_string());
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/current_scene_content")]
struct SceneContent;

fn triangle_count(mesh: &Mesh) -> Option<usize> {
    let topology = mesh.primitive_topology();
    if !matches!(
        topology,
        PrimitiveTopology::TriangleList | PrimitiveTopology::TriangleStrip
    ) {
        return Some(0);
    }
    // The rendering fork can release vertex attributes after extraction. Do not retain or
    // access them to count a nonindexed GPU-only mesh.
    let n = mesh.indices().map(|indices| indices.len()).or_else(|| {
        mesh.asset_usage
            .contains(RenderAssetUsages::MAIN_WORLD)
            .then(|| mesh.count_vertices())
    })?;
    Some(match topology {
        PrimitiveTopology::TriangleList => n / 3,
        _ => n.saturating_sub(2),
    })
}

#[derive(Default)]
struct TriangleTotal {
    known: u64,
    missing: usize,
}

impl TriangleTotal {
    fn add(&mut self, count: Option<usize>) {
        if let Some(count) = count {
            self.known += count as u64;
        } else {
            self.missing += 1;
        }
    }
    fn display(&self) -> Value {
        match self.missing {
            0 => self.known.into(),
            n => format!(
                "At least {} (meshes with unknown triangle counts = {n})",
                self.known
            )
            .into(),
        }
    }
}

fn scene_content(
    mut input: ConsoleCommand<SceneContent>,
    containing: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    scenes: Query<
        (
            &RendererSceneContext,
            Option<&SceneColliderData>,
            Option<&SceneResourceLookup>,
        ),
        Without<SuperUserScene>,
    >,
    drawables: Query<(
        &ContainerEntity,
        &Mesh3d,
        Option<&MeshMaterial3d<SceneMaterial>>,
        &InheritedVisibility,
    )>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<SceneMaterial>>,
) {
    if input.take().is_none() {
        return;
    }
    let Some(root) = player.single().ok().and_then(|p| containing.get_parcel(p)) else {
        input.reply_ok("{}");
        return;
    };
    let Ok((scene, colliders, resources)) = scenes.get(root) else {
        input.reply_ok("{}");
        return;
    };
    if scene.is_portable {
        input.reply_ok("{}");
        return;
    }
    let mut geometry = HashSet::new();
    let mut material_ids = HashSet::new();
    if let Some(resources) = resources {
        geometry.extend(
            resources
                .meshes_by_hash
                .values()
                .map(|mesh| mesh.mesh_id)
                .filter(|id| meshes.contains(*id)),
        );
        material_ids.extend(
            resources
                .materials
                .values()
                .map(|h| h.id())
                .filter(|id| materials.contains(*id)),
        );
    }
    let mut visible_geometry = HashSet::new();
    let mut visible_materials = HashSet::new();
    let (mut all_bodies, mut visible_bodies) = (0, 0);
    let (mut all_triangles, mut visible_triangles) =
        (TriangleTotal::default(), TriangleTotal::default());
    for (container, mesh, material, visible) in &drawables {
        if container.root != root {
            continue;
        }
        all_bodies += 1;
        let loaded = meshes.get(mesh.id());
        let triangles = loaded.and_then(triangle_count);
        all_triangles.add(triangles);
        if loaded.is_some() {
            geometry.insert(mesh.id());
        }
        let material = material
            .map(|h| h.id())
            .filter(|id| materials.contains(*id));
        material_ids.extend(material);
        if visible.get() {
            visible_bodies += 1;
            visible_triangles.add(triangles);
            if loaded.is_some() {
                visible_geometry.insert(mesh.id());
            }
            visible_materials.extend(material);
        }
    }
    let mut textures = HashSet::new();
    for material in material_ids.iter().filter_map(|id| materials.get(*id)) {
        let base = &material.base;
        for texture in [
            &base.base_color_texture,
            &base.emissive_texture,
            &base.normal_map_texture,
            &base.metallic_roughness_texture,
            &base.occlusion_texture,
        ]
        .into_iter()
        .flatten()
        {
            textures.insert(texture.id());
        }
    }
    input.reply_ok(json!({"entities": scene.live_scene_entities().count(), "bodies": visible_bodies, "triangles": visible_triangles.display(),
        "allBodies": all_bodies, "allTriangles": all_triangles.display(),
        "geometries": geometry.len(), "visibleGeometries": visible_geometry.len(), "materials": material_ids.len(),
        "visibleMaterials": visible_materials.len(), "textures": textures.len(),
        "colliders": colliders.map_or(0, SceneColliderData::collider_count)}).to_string());
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_loading_state")]
struct LoadingState;

fn loading_state(
    mut input: ConsoleCommand<LoadingState>,
    current: Res<CurrentSceneLoading>,
    containing: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    stages: Query<&SceneLoading>,
    scenes: Query<(&RendererSceneContext, Option<&GltfLoadingCount>)>,
    preloads: Query<&PreloadedAsset, With<LoadingPreloadedAsset>>,
) {
    if input.take().is_none() {
        return;
    }
    let mut stages_by_kind = [0; 5];
    for stage in &stages {
        stages_by_kind[match stage {
            SceneLoading::SceneSpawned => 0,
            SceneLoading::SceneEntity => 1,
            SceneLoading::MainCrdt { .. } => 2,
            SceneLoading::Javascript { .. } => 3,
            SceneLoading::Failed => 4,
        }] += 1;
    }
    let [spawned, definition, crdt, javascript, failed] = stages_by_kind;
    let stage = player
        .single()
        .ok()
        .and_then(|p| containing.get_parcel_oow(p))
        .and_then(|root| {
            if let Ok(stage) = stages.get(root) {
                return Some(match stage {
                    SceneLoading::SceneSpawned => "Spawning",
                    SceneLoading::SceneEntity => "Reading definition",
                    SceneLoading::MainCrdt { .. } => "Reading main.crdt",
                    SceneLoading::Javascript { .. } => "Starting JavaScript",
                    SceneLoading::Failed => "Failed",
                });
            }
            scenes.get(root).ok().map(|(ctx, _)| {
                if ctx.broken() {
                    "Broken"
                } else if !ctx.blocked.is_empty() {
                    "Blocked"
                } else {
                    "Running"
                }
            })
        });
    let (mut running, mut blocked, mut broken, mut gltf) = (0, 0, 0, 0);
    for (scene, loading) in &scenes {
        if scene.broken() {
            broken += 1
        } else if scene.blocked.is_empty() {
            running += 1
        } else {
            blocked += 1
        }
        gltf += loading.map_or(0, |count| count.0);
    }
    let (mut preloads_count, mut textures) = (0, 0);
    for asset in &preloads {
        preloads_count += 1;
        let ext = std::path::Path::new(&asset.file_path)
            .extension()
            .and_then(|ext| ext.to_str());
        if ext.is_some_and(|ext| {
            ["ktx2", "png", "jpg", "jpeg", "webp"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        }) {
            textures += 1;
        }
    }
    input.reply_ok(json!({"current":current.0, "stage": stage, "spawned":spawned,"definition":definition,"crdt":crdt,"javascript":javascript,"failed":failed,
        "running":running,"blocked":blocked,"broken":broken,"gltf":gltf,"preloads":preloads_count,"texturePreloads":textures}).to_string());
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/texture_cache_state")]
struct TextureState;

fn texture_state(mut input: ConsoleCommand<TextureState>, images: Res<Assets<Image>>) {
    if input.take().is_some() {
        let bytes: usize = images
            .iter()
            .filter_map(|(_, image)| image.data.as_ref())
            .map(Vec::len)
            .sum();
        input.reply_ok(json!({"count":images.len(),"bytes":bytes}).to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::{DclReader, SceneComponentId, SceneCrdtTimestamp, SceneEntityId};
    use dcl_types::interface::CrdtType;

    #[test]
    fn gpu_only_nonindexed_triangles_remain_explicitly_partial() {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        let mut total = TriangleTotal::default();
        total.add(Some(12));
        assert_eq!(total.display(), json!(12));
        total.add(triangle_count(&mesh));
        assert_eq!(
            total.display(),
            json!("At least 12 (meshes with unknown triangle counts = 1)")
        );
    }

    #[test]
    fn charts_pair_actual_dispatched_and_received_component_batches() {
        let mut history = SceneTraffic::new();
        let mut incoming = CrdtStore::default();
        incoming.try_update(
            SceneComponentId(42),
            CrdtType::LWW_ANY,
            SceneEntityId::ROOT,
            SceneCrdtTimestamp(1),
            Some(&mut DclReader::new(&[5; 30])),
        );
        let empty = CrdtStore::default();
        history.record_received(&incoming, 0.02);
        assert!(
            history.report().is_null(),
            "opening mid-tick must not invent an outbound sample"
        );
        for _ in 0..260 {
            history.record_sent(&empty);
            history.record_received(&incoming, 0.02);
        }
        let report = history.report();
        assert_eq!(report["tickFps"]["samples"].as_array().unwrap().len(), 256);
        assert_eq!(report["tickFps"]["samples"][0], 50.0);
        assert_eq!(report["from"]["bytes"]["samples"][255], 30);
        assert_eq!(report["from"]["updates"]["samples"][255], 1);
        assert_eq!(report["to"]["bytes"]["samples"][255], 0);
        assert_eq!(report["from"]["range"], json!([1, 1, 0]));
    }
}
