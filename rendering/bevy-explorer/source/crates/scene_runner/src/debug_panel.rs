use std::collections::HashSet;

use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::PrimitiveTopology};
use bevy_console::ConsoleCommand;
use common::structs::PrimaryUser;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use scene_material::SceneMaterial;
use serde_json::{json, Value};

use crate::{
    asset_preload::plugin::{LoadingPreloadedAsset, PreloadedAsset},
    initialize_scene::{
        CurrentSceneLoading, NoParcelScene, SceneLoading, SuperUserScene, PARCEL_SIZE,
    },
    renderer_context::RendererSceneContext,
    scene_traffic::{Batch, SceneTraffic, Tick},
    stage::{state_of, Stage},
    update_world::{
        gltf_container::{GltfLoadingCount, SceneResourceLookup},
        mesh_collider::SceneColliderData,
    },
    util::settle::SceneWaits,
    vec3_to_parcel, ContainerEntity, ContainingScene,
};

pub(super) struct SceneDebugPanelPlugin;

impl Plugin for SceneDebugPanelPlugin {
    fn build(&self, app: &mut App) {
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
                    ("Collider height limit (m)", "colliderHeight"),
                    ("Scenes whose colliders reach the player", "scenesInReach"),
                    ("Global position (m) (X / Y / Z)", "world"),
                    ("Scene position (m) (X / Y / Z)", "relative"),
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
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Worker fetches started", "worker.fetch_started"),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Worker fetches completed", "worker.fetch_completed"),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Worker fetches failed", "worker.fetch_failed"),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Worker fetch bytes received", "worker.fetch_bytes_down"),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Current native V8 heap used (bytes)", "heap"),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Current native V8 heap limit (bytes)", "heapLimit"),
                    #[cfg(not(target_arch = "wasm32"))]
                    (
                        "Current native V8 external memory (bytes)",
                        "externalMemory",
                    ),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Sampled native JS engines", "engines.count"),
                    #[cfg(not(target_arch = "wasm32"))]
                    ("Sampled native JS heaps used (bytes)", "engines.used"),
                    #[cfg(not(target_arch = "wasm32"))]
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
                    ("Colliders off outside scene bounds", "collidersOutOfBounds"),
                ],
                actions: &[],
                inputs: &[],
            })
            .add_debug_section(DebugSection {
                title: "Scene Loading",
                query: "/scene_loading_state",
                fields: &[
                    ("Current parcel loading", "current"),
                    ("Player parcel", "playerParcel"),
                    ("Player out of world", "outOfWorld"),
                    ("Current parcel scene", "sceneHash"),
                    ("Current parcel stage", "stage"),
                    ("Current parcel state", "state"),
                    ("Current parcel scene ready", "isReady"),
                    ("Current parcel scene blocked by", "blockedBy"),
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
                    ("Commands waiting for a scene to settle", "settleWaits"),
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

fn traffic_report(history: &SceneTraffic) -> Value {
    let ticks = &history.ticks;
    if ticks.is_empty() {
        return Value::Null;
    }
    let seconds = ticks.iter().map(|tick| tick.dt).sum::<f64>();
    let direction = |batch: fn(&Tick) -> Batch| {
        let bytes: Vec<usize> = ticks.iter().map(|tick| batch(tick).bytes).collect();
        let updates: Vec<usize> = ticks.iter().map(|tick| batch(tick).updates).collect();
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
    let fps: Vec<f64> = ticks.iter().map(|tick| (1.0 / tick.dt).round()).collect();
    let recent = ticks.len().min(32);
    json!({"fps": (recent as f64 / ticks.iter().rev().take(recent).map(|t|t.dt).sum::<f64>()).round(),
        "minFps":fps.iter().copied().reduce(f64::min), "maxFps":fps.iter().copied().reduce(f64::max),
        "hiccups":ticks.iter().filter(|t|t.dt>0.05).count(), "tickFps":{"unit":"FPS", "samples":fps},
        "from":direction(|t|t.received), "to":direction(|t|t.sent)})
}

/// Runtime counters for the player's parcel scene.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/current_scene_state")]
struct SceneState;

/// The parcel may already be known to be empty, still resolving, waiting for its scene, or
/// holding one that refuses the player.
fn parcel_root(
    containing: &ContainingScene,
    bans: &comms::SceneBanGate,
    player: Option<Entity>,
) -> Result<Entity, &'static str> {
    let (transform, outside) = player
        .and_then(|id| containing.transforms.get(id).ok())
        .ok_or(NO_POSITION)?;
    if outside {
        return Err("The player has not entered the world.");
    }
    containing
        .admitted_scene(vec3_to_parcel(transform.translation()), bans)
        .map_err(NoParcelScene::reason)
}

const NO_POSITION: &str = "Waiting for the player position.";

fn scene_state(
    mut input: ConsoleCommand<SceneState>,
    player: Query<(Entity, &GlobalTransform), With<PrimaryUser>>,
    containing: ContainingScene,
    bans: comms::SceneBanGate,
    scenes: Query<(&RendererSceneContext, Has<SuperUserScene>)>,
    bounds: Res<ShowBounds>,
    histories: Query<&SceneTraffic>,
    colliders: Query<&SceneColliderData>,
    parcel_scenes: Query<(&RendererSceneContext, &SceneColliderData), Without<SuperUserScene>>,
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
    let player = player.single().ok();
    let position = player.map(|(_, t)| t.translation() * Vec3::new(1.0, 1.0, -1.0));
    let scene = parcel_root(&containing, &bans, player.map(|(id, _)| id)).and_then(|root| {
        let (scene, privileged) = scenes
            .get(root)
            .map_err(|_| "Waiting for the parcel scene to start.")?;
        if privileged || scene.is_portable {
            return Err("Scene inspection requires an ordinary parcel scene.");
        }
        Ok(scene)
    });
    let mut unavailable = json!({"engines": "No live native scene worker has reported a sample."});
    let scene = match scene {
        Ok(scene) => scene,
        Err(reason) => {
            unavailable[""] = reason.into();
            input.reply_ok(json!({"bounds": bounds.0, "engines": engines, "world": position.map(|p| p.to_array()), "unavailable": unavailable}).to_string());
            return;
        }
    };
    let base = Vec3::new(
        scene.base.x as f32 * PARCEL_SIZE,
        0.0,
        scene.base.y as f32 * PARCEL_SIZE,
    );
    let root = scene.scene_id.0;
    let history = histories.get(root).ok().map(traffic_report);
    // Browser scene workers do not publish V8 heap readings or the native fetch counters.
    unavailable["history"] = "Waiting for a complete tick in the current parcel scene.".into();
    unavailable["bytesPerSecond"] = "Waiting for two consecutive worker samples.".into();
    unavailable["height"] = "The scene has no parcel bounds.".into();
    unavailable["colliderHeight"] = "The scene's colliders have no bounds in this run.".into();
    unavailable[""] = "The parcel scene worker has not reported a sample.".into();
    let report = json!({
        "unavailable": unavailable,
        "bounds": bounds.0, "name": scene.title, "sdk": scene.sdk_version, "parcels": scene.parcels.len(),
        "height": scene.bounds.iter().map(|r| r.height).reduce(f32::max),
        "colliderHeight": colliders.get(root).ok().and_then(SceneColliderData::height_limit),
        "scenesInReach": player.map(|(_, at)| parcel_scenes.iter().filter(|(scene, colliders)| {
            !scene.is_portable && colliders.reaches_avatar(at.translation().as_dvec3())
        }).count()),
        "world": position.map(|p| p.to_array()), "relative": position.map(|p| (p-base).to_array()), "tick": scene.tick_number,
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
    bans: comms::SceneBanGate,
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
    let root = match parcel_root(&containing, &bans, player.single().ok()) {
        Ok(root) => root,
        Err(reason) => {
            input.reply_ok(json!({"unavailable": {"": reason}}).to_string());
            return;
        }
    };
    let Ok((scene, colliders, resources)) = scenes.get(root) else {
        input.reply_ok(
            json!({"unavailable": {"": "No ordinary parcel scene is ready for inspection."}})
                .to_string(),
        );
        return;
    };
    if scene.is_portable {
        input.reply_ok(
            json!({"unavailable": {"": "Scene inspection requires an ordinary parcel scene."}})
                .to_string(),
        );
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
        "colliders": colliders.map_or(0, SceneColliderData::collider_count),
        "collidersOutOfBounds": colliders.map_or(0, SceneColliderData::out_of_bounds_count)}).to_string());
}

/// How scene loading stands, as JSON: the parcel scene under the player (`playerParcel`,
/// `outOfWorld`, `sceneHash`, its `stage`, its lifecycle `state`, what it is `blockedBy` and
/// whether it `isReady`, the scene readiness `/teleport --wait` and `/reload --wait` wait for)
/// and counts of every scene by stage
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_loading_state")]
struct LoadingState;

fn loading_state(
    mut input: ConsoleCommand<LoadingState>,
    current: Res<CurrentSceneLoading>,
    containing: ContainingScene,
    bans: comms::SceneBanGate,
    player: Query<Entity, With<PrimaryUser>>,
    stages: Query<&SceneLoading>,
    scenes: Query<(&RendererSceneContext, Option<&GltfLoadingCount>)>,
    preloads: Query<&PreloadedAsset, With<LoadingPreloadedAsset>>,
    waits: Res<SceneWaits>,
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

    // the parcel scene under the player, out of world included: it loads before the player lands
    let player = player
        .single()
        .ok()
        .and_then(|id| containing.transforms.get(id).ok())
        .map(|(transform, outside)| (vec3_to_parcel(transform.translation()), outside));
    let hash = player.map(|(parcel, _)| containing.admitted_hash(parcel, &bans));
    let root = player.map(|(parcel, _)| containing.admitted_scene(parcel, &bans));
    let context = root
        .and_then(Result::ok)
        .and_then(|root| scenes.get(root).ok())
        .map(|(context, _)| context);
    let stage = root.map(|root| root.map(|root| Stage::of(stages.get(root).ok(), context)));
    let mut blocked_by: Vec<&str> = context
        .map(|context| context.blocked.iter().copied().collect())
        .unwrap_or_default();
    blocked_by.sort_unstable();
    let mut unavailable = serde_json::Map::new();
    match hash {
        None => {
            for field in ["playerParcel", "outOfWorld", "sceneHash"] {
                unavailable.insert(field.to_owned(), NO_POSITION.into());
            }
        }
        Some(Err(none)) => {
            unavailable.insert("sceneHash".to_owned(), none.reason().into());
        }
        Some(Ok(_)) => {}
    }

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
        if ext
            .and_then(bevy::image::ImageFormat::from_extension)
            .is_some()
        {
            textures += 1;
        }
    }
    input.reply_ok(json!({"current":current.0,
        "playerParcel": player.map(|(parcel, _)| [parcel.x, parcel.y]),
        "outOfWorld": player.map(|(_, outside)| outside),
        "sceneHash": hash.and_then(Result::ok),
        "stage": stage.map_or(NO_POSITION, |stage| stage.map_or_else(NoParcelScene::reason, Stage::name)),
        "state": stage.map_or(Stage::Unstarted.state(), state_of),
        "isReady": stage.is_some_and(|stage| stage.is_ok_and(Stage::is_ready)),
        "blockedBy": blocked_by, "unavailable": unavailable,
        "spawned":spawned,"definition":definition,"crdt":crdt,"javascript":javascript,"failed":failed,
        "running":running,"blocked":blocked,"broken":broken,"gltf":gltf,"preloads":preloads_count,"texturePreloads":textures,
        "settleWaits":waits.pending()}).to_string());
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
    use crate::{
        initialize_scene::{LiveScenes, PointerResult, PortableScenes, ScenePointers},
        renderer_context::{FROZEN_BLOCK, STARTUP_TICKS},
        update_world::gltf_container::GLTF_LOADING,
    };
    use bevy_console::{ConsoleCommandEntered, ConsoleResponder};
    use common::structs::OutOfWorld;
    use common::sync::MutexExt;
    use dcl_component::{DclReader, SceneComponentId, SceneCrdtTimestamp, SceneEntityId};
    use dcl_types::interface::{CrdtStore, CrdtType};
    use std::sync::{Arc, Mutex};

    fn scene_state_app() -> App {
        let mut app = App::new();
        app.add_plugins(console::ConsolePlugin {
            add_bevy_console: false,
        })
        .init_resource::<LiveScenes>()
        .init_resource::<PortableScenes>()
        .init_resource::<ScenePointers>()
        .init_resource::<ShowBounds>()
        // what the ban gate reads
        .init_resource::<comms::SceneAdmission>()
        .init_resource::<common::structs::CurrentRealm>()
        .init_resource::<common::structs::PreviewMode>()
        .init_resource::<wallet::Wallet>()
        .add_console_command::<SceneState, _>(scene_state);
        app.world_mut()
            .resource_mut::<ScenePointers>()
            .set_realm(IVec2::ZERO, IVec2::ONE);
        app
    }

    fn read_scene_state(app: &mut App) -> Value {
        read(app, "/current_scene_state")
    }

    fn read(app: &mut App, command: &str) -> Value {
        let result = Arc::new(Mutex::new(None));
        let responder: ConsoleResponder = {
            let result = result.clone();
            Arc::new(move |reply| *result.lock_ignore_poison() = Some(reply))
        };
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: command.into(),
            args: Vec::new(),
            responder: Some(responder),
        });
        app.update();
        let reply = result.lock_ignore_poison().take().unwrap().unwrap();
        serde_json::from_str(&reply).unwrap()
    }

    fn loading_state_app() -> App {
        let mut app = scene_state_app();
        app.init_resource::<CurrentSceneLoading>()
            .init_resource::<SceneWaits>()
            .add_console_command::<LoadingState, _>(loading_state);
        app
    }

    /// A scene "hash" on parcel (0, 0) at `tick`, its pointer resolved.
    fn loading_scene(app: &mut App, tick: u32) -> Entity {
        let world = app.world_mut();
        let root = world.spawn_empty().id();
        let mut context = RendererSceneContext::new(
            dcl_types::SceneId(root),
            "hash".to_owned(),
            String::new(),
            false,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            [IVec2::ZERO].into_iter().collect(),
            vec![],
            default(),
            vec![],
            root,
            0.0,
            false,
            "sdk7",
            false,
            false,
        );
        context.tick_number = tick;
        world.entity_mut(root).insert(context);
        world
            .resource_mut::<LiveScenes>()
            .scenes
            .insert("hash".to_owned(), root);
        resolve_pointer(app);
        root
    }

    fn resolve_pointer(app: &mut App) {
        app.world_mut().resource_mut::<ScenePointers>().insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "hash".to_owned(),
                urn: None,
            },
        );
    }

    #[test]
    fn loading_state_reports_the_readiness_of_the_scene_under_the_player() {
        let mut app = loading_state_app();
        app.world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY));

        // the parcel's pointer has not resolved yet
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["playerParcel"], json!([0, 0]));
        assert_eq!(report["outOfWorld"], false);
        assert_eq!(report["state"], "loading");
        assert_eq!(report["stage"], NoParcelScene::Unresolved.reason());
        assert_eq!(report["isReady"], false);
        assert!(report["sceneHash"].is_null());
        assert_eq!(
            report["unavailable"]["sceneHash"],
            NoParcelScene::Unresolved.reason()
        );

        // an empty parcel
        app.world_mut()
            .resource_mut::<ScenePointers>()
            .insert(IVec2::ZERO, PointerResult::Nothing);
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["state"], "none");
        assert_eq!(report["stage"], NoParcelScene::Empty.reason());
        assert_eq!(report["isReady"], false);

        // a scene in its startup ticks, then past them: one classification behind both names
        let root = loading_scene(&mut app, STARTUP_TICKS);
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["sceneHash"], "hash");
        assert_eq!(report["stage"], "Starting");
        assert_eq!(report["state"], "starting");
        assert_eq!(report["isReady"], false);
        assert_eq!(report["blockedBy"], json!([]));
        assert_eq!(report["unavailable"], json!({}));
        // the counts every caller already reads are still there
        assert_eq!(report["running"], 1);
        assert_eq!(report["settleWaits"], 0);

        app.world_mut()
            .get_mut::<RendererSceneContext>(root)
            .unwrap()
            .tick_number = STARTUP_TICKS + 1;
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["stage"], "Running");
        assert_eq!(report["state"], "running");
        assert_eq!(report["isReady"], true);
    }

    #[test]
    fn loading_state_without_a_player_names_what_it_waits_for() {
        let mut app = loading_state_app();
        loading_scene(&mut app, STARTUP_TICKS + 1);
        let report = read(&mut app, "/scene_loading_state");
        assert!(report["playerParcel"].is_null());
        assert!(report["outOfWorld"].is_null());
        assert!(report["sceneHash"].is_null());
        assert_eq!(report["stage"], NO_POSITION);
        assert_eq!(report["state"], "loading");
        assert_eq!(report["isReady"], false);
        assert_eq!(report["unavailable"]["playerParcel"], NO_POSITION);
        // the counts do not need the player
        assert_eq!(report["running"], 1);
    }

    #[test]
    fn loading_state_reads_the_scene_under_a_player_still_out_of_world() {
        let mut app = loading_state_app();
        app.world_mut().spawn((
            PrimaryUser::default(),
            GlobalTransform::IDENTITY,
            OutOfWorld::default(),
        ));
        loading_scene(&mut app, STARTUP_TICKS + 1);
        let report = read(&mut app, "/scene_loading_state");
        // the scene is ready before the player lands on it: `outOfWorld` tells the two apart
        assert_eq!(report["outOfWorld"], true);
        assert_eq!(report["isReady"], true);
        assert_eq!(report["sceneHash"], "hash");
    }

    #[test]
    fn loading_state_names_a_scene_that_has_not_spawned_by_its_pointer() {
        let mut app = loading_state_app();
        app.world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY));
        resolve_pointer(&mut app);
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["sceneHash"], "hash");
        assert_eq!(report["stage"], NoParcelScene::NotSpawned.reason());
        assert_eq!(report["state"], "loading");
        assert_eq!(report["isReady"], false);
    }

    #[test]
    fn loading_state_names_a_scene_that_refuses_the_player() {
        let mut app = loading_state_app();
        app.world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY));
        resolve_pointer(&mut app);
        crate::stage::refuse_player(app.world_mut(), "hash");
        let report = read(&mut app, "/scene_loading_state");
        // not a scene still to load: it is not loaded for this player
        assert_eq!(report["state"], "refused");
        assert_eq!(report["stage"], NoParcelScene::Refused.reason());
        assert_eq!(report["isReady"], false);
        assert!(report["sceneHash"].is_null());
        assert_eq!(
            report["unavailable"]["sceneHash"],
            NoParcelScene::Refused.reason()
        );
        // the scene readouts say the same
        let report = read_scene_state(&mut app);
        assert_eq!(report["unavailable"][""], NoParcelScene::Refused.reason());
    }

    #[test]
    fn loading_state_says_what_blocks_a_scene_and_when_it_is_broken() {
        let mut app = loading_state_app();
        app.world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY));
        let root = loading_scene(&mut app, STARTUP_TICKS + 1);
        let mut context = app
            .world_mut()
            .get_mut::<RendererSceneContext>(root)
            .unwrap();
        context.blocked.insert(GLTF_LOADING);
        context.blocked.insert(FROZEN_BLOCK);
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["stage"], "Blocked");
        assert_eq!(report["state"], "blocked");
        assert_eq!(report["isReady"], false);
        assert_eq!(report["blockedBy"], json!([FROZEN_BLOCK, GLTF_LOADING]));
        assert_eq!(report["blocked"], 1);

        let mut context = app
            .world_mut()
            .get_mut::<RendererSceneContext>(root)
            .unwrap();
        context.blocked.clear();
        context.state = crate::renderer_context::SceneState::Broken;
        let report = read(&mut app, "/scene_loading_state");
        assert_eq!(report["stage"], "Broken");
        assert_eq!(report["state"], "broken");
        assert_eq!(report["isReady"], false);
        assert_eq!(report["broken"], 1);
    }

    #[test]
    fn empty_parcel_keeps_player_coordinates_and_explains_missing_scene_readings() {
        let mut app = scene_state_app();
        app.world_mut()
            .resource_mut::<ScenePointers>()
            .insert(IVec2::ZERO, PointerResult::Nothing);
        app.world_mut().spawn((
            PrimaryUser::default(),
            GlobalTransform::from_translation(Vec3::new(8.0, 2.0, -8.0)),
        ));
        app.update();
        let report = read_scene_state(&mut app);
        assert_eq!(report["world"], json!([8.0, 2.0, 8.0]));
        assert_eq!(
            report["unavailable"][""],
            "There is no scene on this parcel."
        );
        assert!(report["parcels"].is_null());
    }

    #[test]
    fn first_scene_state_read_has_ticks_from_the_current_parcel_only() {
        let mut app = scene_state_app();
        app.add_systems(Update, crate::scene_traffic::track_parcel);
        let player = app
            .world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY))
            .id();
        let parcels = [IVec2::ZERO, IVec2::X].map(|parcel| {
            let world = app.world_mut();
            let root = world.spawn_empty().id();
            let hash = parcel.to_string();
            world.entity_mut(root).insert(RendererSceneContext::new(
                dcl_types::SceneId(root),
                hash.clone(),
                String::new(),
                false,
                0,
                hash.clone(),
                parcel,
                [parcel].into_iter().collect(),
                vec![],
                default(),
                vec![],
                root,
                0.0,
                false,
                "sdk7",
                false,
                false,
            ));
            world
                .resource_mut::<LiveScenes>()
                .scenes
                .insert(hash.clone(), root);
            world
                .resource_mut::<ScenePointers>()
                .insert(parcel, PointerResult::Exists { hash, urn: None });
            root
        });
        app.update();
        let empty = CrdtStore::default();
        let incoming = component_update();
        let mut history = app.world_mut().get_mut::<SceneTraffic>(parcels[0]).unwrap();
        history.record_sent(&empty);
        history.record_received(&incoming, 0.04);
        let first = read_scene_state(&mut app);
        assert_eq!(first["history"]["tickFps"]["samples"], json!([25.0]));
        assert_eq!(first["history"]["from"]["rate"], json!([750.0, 25.0]));
        assert!(app.world().get::<SceneTraffic>(parcels[1]).is_none());

        *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
            GlobalTransform::from_translation(Vec3::X * PARCEL_SIZE);
        app.update();
        assert!(app.world().get::<SceneTraffic>(parcels[0]).is_none());
        let mut next = app.world_mut().get_mut::<SceneTraffic>(parcels[1]).unwrap();
        // A reply already in flight on entry has no matching dispatch on this parcel.
        next.record_received(&incoming, 0.04);
        next.record_sent(&empty);
        next.record_received(&incoming, 0.02);
        assert_eq!(
            read_scene_state(&mut app)["history"]["tickFps"]["samples"],
            json!([50.0])
        );
    }

    #[test]
    fn scene_state_counts_the_scenes_whose_colliders_reach_the_player() {
        let mut app = scene_state_app();
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
            ))
            .id();
        // two scenes side by side: parcel (0, 0) spans world x 0 to 16, parcel (1, 0) x 16 to 32
        for parcel in [IVec2::ZERO, IVec2::X] {
            let world = app.world_mut();
            let root = world.spawn_empty().id();
            let hash = parcel.to_string();
            let regions = vec![scene_material::BoundRegion::new(parcel, parcel, 1)];
            world.entity_mut(root).insert((
                SceneColliderData::for_regions(&regions, 1),
                RendererSceneContext::new(
                    dcl_types::SceneId(root),
                    hash.clone(),
                    String::new(),
                    false,
                    0,
                    hash.clone(),
                    parcel,
                    [parcel].into_iter().collect(),
                    regions,
                    default(),
                    vec![],
                    root,
                    0.0,
                    false,
                    "sdk7",
                    false,
                    false,
                ),
            ));
            world
                .resource_mut::<LiveScenes>()
                .scenes
                .insert(hash.clone(), root);
            world
                .resource_mut::<ScenePointers>()
                .insert(parcel, PointerResult::Exists { hash, urn: None });
        }
        let mut scenes_in_reach = |at: Vec3| {
            *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
                GlobalTransform::from_translation(at);
            app.update();
            read_scene_state(&mut app)["scenesInReach"].clone()
        };

        assert_eq!(scenes_in_reach(Vec3::new(8.0, 0.0, -8.0)), json!(1));
        // the capsule reaches over the edge between the two
        assert_eq!(scenes_in_reach(Vec3::new(15.8, 0.0, -8.0)), json!(2));
        // above the 20 m a scene of one parcel is drawn to
        assert_eq!(scenes_in_reach(Vec3::new(8.0, 30.0, -8.0)), json!(0));
    }

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

    fn component_update() -> CrdtStore {
        let mut incoming = CrdtStore::default();
        incoming.try_update(
            SceneComponentId(42),
            CrdtType::LWW_ANY,
            SceneEntityId::ROOT,
            SceneCrdtTimestamp(1),
            Some(&mut DclReader::new(&[5; 30])),
        );
        incoming
    }

    #[test]
    fn charts_pair_actual_dispatched_and_received_component_batches() {
        let mut history = SceneTraffic::default();
        let incoming = component_update();
        let empty = CrdtStore::default();
        history.record_received(&incoming, 0.02);
        assert!(
            traffic_report(&history).is_null(),
            "entering mid-tick must not invent an outbound sample"
        );
        for _ in 0..260 {
            history.record_sent(&empty);
            history.record_received(&incoming, 0.02);
        }
        let report = traffic_report(&history);
        assert_eq!(report["tickFps"]["samples"].as_array().unwrap().len(), 256);
        assert_eq!(report["tickFps"]["samples"][0], 50.0);
        assert_eq!(report["from"]["bytes"]["samples"][255], 30);
        assert_eq!(report["from"]["updates"]["samples"][255], 1);
        assert_eq!(report["to"]["bytes"]["samples"][255], 0);
        assert_eq!(report["from"]["range"], json!([1, 1, 0]));
    }
}
