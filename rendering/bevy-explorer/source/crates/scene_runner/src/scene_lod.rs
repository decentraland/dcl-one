//! A scene's stand-ins while it is in range but not live. Near: the scene's own glTF files,
//! placed from the asset pipeline's initial-scene-state descriptor. Far: the pipeline's one
//! simplified mesh for the scene, named by the Bevy variants client from the scene's hash and
//! placed at the base parcel the realm listed, with no entity definition. Each model is a glTF
//! container on the live container path (`gltf_container`), which loads it under the scene's
//! content path and processes it a slice a frame.
//!
//! A near LOD has two parts with their own rules:
//! - Colliders, for every scene within the start ring whatever the facing, so a player who
//!   reaches a loading scene, backing in or walking in, is stopped by it. Each model loads a
//!   second time under an asset path of its own with no textures (`GltfPart::Colliders`); its
//!   colliders block as soon as the model is processed, whether or not anything is drawn, and the
//!   glTF goes then, leaving the collider shapes in main-world memory. A colliders part is never
//!   interrupted: it goes with its scene.
//! - Visuals, for the scenes in view under the near LOD count (`AppConfig::near_lods_cap`, `GltfPart::Visuals`): the same asset
//!   the live scene loads, drawn once every model is processed. When the cap is full, one behind
//!   the camera makes way for one in view; dropping it frees its textures and meshes once nothing
//!   else holds the glTF, and coming back reloads from the disk cache the colliders part filled.
//!
//! A far LOD has visuals only, under the far LOD count (`AppConfig::far_lods_cap`) by the same rule, and no colliders. It goes
//! when the near one shows; any stand-in goes, all its parts with it, in the frame the live
//! scene's content is first on screen. The live scene's containers made before then name the
//! same files and share the loaded glTFs.

use std::ops::RangeInclusive;

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext, LoadState},
    ecs::{component::HookContext, world::DeferredWorld},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::storage::ShaderStorageBuffer,
};
use bevy_console::ConsoleCommand;
use common::{
    bounds_calc::scene_regions,
    partition::{LoadPriority, Tier},
    sets::SceneSets,
    structs::{AppConfig, PreviewMode, SceneMeta, FAR_LOD_RINGS, LIVE_SCENE_RINGS, NEAR_LOD_RINGS},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::sdk::components::PbGltfContainer,
    transform_and_parent::{DclQuat, DclTransformAndParent, DclTranslation},
    SceneEntityId,
};
use dcl_types::SceneId;
use ipfs::{bevy_variants::FarLodSource, EntityDefinition, IpfsAssetServer, IpfsResource};
use scene_material::{BoundRegion, SceneBounds, SceneMaterial};
use serde::Deserialize;
use serde_json::json;

use crate::{
    initialize_scene::{
        LiveScenes, PointerResult, SceneEntityDefinitionHandle, ScenePointers, PARCEL_SIZE,
    },
    renderer_context::RendererSceneContext,
    scene_partition::Partition,
    update_world::{
        gltf_container::{
            GltfDefinition, GltfPart, GltfProcessed, SceneResourceLookup, StandInBounds,
            StandInContent,
        },
        mesh_collider::SceneColliderData,
        ComponentTracker,
    },
    vec3_to_parcel, PrimaryUser, SceneEntity,
};

/// The streaming bands, in Chebyshev parcel rings of the player's parcel
/// (`Partition::parcel_distance`; one ring is 16 m), from the "Live Scene Ring", "Near LOD Ring"
/// and "Far LOD Ring" settings. Each keep ring is one past its start ring, so a player walking
/// along an edge does not start and drop a scene or stand-in every few steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rings {
    /// a scene with a parcel within this ring goes live whatever the load distance
    pub(crate) live: u16,
    /// a live scene is kept until it is past this ring (and past the load distance and margin)
    pub(crate) live_keep: u16,
    /// a scene that is not live starts a near LOD within this ring; at or inside the live ring
    /// that leaves near LODs only to live scenes still loading
    pub(crate) near_start: u16,
    /// a near LOD is kept until its scene is past this ring
    pub(crate) near_keep: u16,
    /// a far LOD, the pipeline's one mesh for the scene, starts from this ring: past the near
    /// keep ring, and past the live keep ring when the live ring reaches further
    pub(crate) far_start: u16,
    /// the far LOD's outer bound; the imposter tiles may end it sooner
    /// (`FAR_LOD_MAX_TILE_LEVEL`). Beyond either only the imposters remain
    pub(crate) far: u16,
}

impl Rings {
    /// The rings the settings give, each clamped to its setting's range, so a hand-edited config
    /// cannot widen the parcel scan past it.
    pub(crate) fn from_config(config: &AppConfig) -> Self {
        let clamp =
            |value: u16, range: RangeInclusive<u16>| value.clamp(*range.start(), *range.end());
        let live = clamp(config.live_scene_ring, LIVE_SCENE_RINGS);
        let near_start = clamp(config.near_lod_ring, NEAR_LOD_RINGS);
        Self {
            live,
            live_keep: live + 1,
            near_start,
            near_keep: near_start + 1,
            far_start: near_start.max(live) + 2,
            far: clamp(config.far_lod_ring, FAR_LOD_RINGS),
        }
    }

    /// The ring the stand-ins' parcel scan covers: both LOD bands.
    pub(crate) fn scan(&self) -> u16 {
        self.far.max(self.near_keep)
    }
}

/// The largest imposter tile level (tiles of 2^level parcels a side) a far LOD may sit in. The
/// imposters split any tile with a stand-in's parcel in it down to single parcels; keeping far
/// LODs out of larger tiles keeps that split to the small tiles near the player, so the edge
/// between far LODs and imposters falls on a tile boundary. At the default imposter distances
/// tiles of level 2 (4 parcels a side) start 200 m away, so far LODs end between about 200 m and
/// 290 m, depending on how the tiles align. The edge has no hysteresis: holding a far LOD past it
/// would split a level-2 tile, so a far LOD goes, and its tile is drawn, as soon as the focus
/// parcel moves its tile out of level 1.
pub(crate) const FAR_LOD_MAX_TILE_LEVEL: usize = 1;

/// The imposters' tile layout around the focus, which they write when it changes.
/// `SceneLodPlugin` registers it as `Off`, the fallback for an app without imposters; the
/// imposter plugin replaces it with `Pending` and owns it from then on.
#[derive(Resource, Default, PartialEq)]
pub enum ImposterTiling {
    /// no imposters: only the far ring bounds a far LOD
    #[default]
    Off,
    /// the imposters run but have not laid their tiles out yet: no far LOD fits
    Pending,
    Known(Tiling),
}

/// The inputs of the imposters' choice of tile level. The imposters lay their tiles out with
/// `tile_distance` and `level_allowed`, and far LODs keep to the same rule through `level`.
#[derive(Clone, PartialEq, Debug)]
pub struct Tiling {
    /// the parcel whose centre the tile distances are measured from
    pub origin_parcel: IVec2,
    pub min_distance: f32,
    pub distance_scale: f32,
    /// the imposter distances setting: tiles of level `n` are drawn from `distances[n - 1]` on
    pub distances: Vec<f32>,
}

impl Tiling {
    /// How far away the imposters take a tile of `level` (2^level parcels a side, starting at
    /// `tile_origin_parcel`) to be: from the focus parcel's centre to the tile's closest point,
    /// scaled and offset as the imposter focus asks.
    pub fn tile_distance(&self, tile_origin_parcel: IVec2, level: usize) -> f32 {
        let origin = (self.origin_parcel.as_vec2() + 0.5) * PARCEL_SIZE;
        let tile_min = tile_origin_parcel.as_vec2() * PARCEL_SIZE;
        let tile_max = tile_min + (1 << level) as f32 * PARCEL_SIZE;
        let closest = origin.clamp(tile_min, tile_max);
        (closest - origin).length() * self.distance_scale + self.min_distance
    }

    /// Whether a tile of `level` at `distance` (`tile_distance`) is far enough away to be drawn
    /// at that level rather than split; single parcels always are.
    pub fn level_allowed(&self, level: usize, distance: f32) -> bool {
        level == 0 || distance >= self.distances[level - 1]
    }

    /// The level of the imposter tile that covers `parcel` when no live scene or stand-in is near
    /// it: the largest level whose tile around the parcel is far enough away for that level.
    pub(crate) fn level(&self, parcel: IVec2) -> usize {
        (1..self.distances.len())
            .rev()
            .find(|&level| {
                let tile_origin_parcel = (parcel >> level as u32) * (1 << level);
                self.level_allowed(level, self.tile_distance(tile_origin_parcel, level))
            })
            .unwrap_or(0)
    }
}

/// Whether a far LOD may cover these parcels: each lies in an imposter tile no larger than
/// `FAR_LOD_MAX_TILE_LEVEL`, so no larger tile is split around it. Without imposters only the far
/// ring bounds it; before they publish a layout none fits.
fn far_lod_fits(tiling: &ImposterTiling, parcels: &[IVec2]) -> bool {
    match tiling {
        ImposterTiling::Off => true,
        ImposterTiling::Pending => false,
        ImposterTiling::Known(tiling) => parcels
            .iter()
            .all(|parcel| tiling.level(*parcel) <= FAR_LOD_MAX_TILE_LEVEL),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum LodLevel {
    /// the scene's own models, placed from the descriptor
    Near,
    /// the pipeline's one mesh for the scene
    Far,
}

/// The scene entity id of a stand-in's first container; the rest follow. A stand-in has no
/// scene, so any id is free: they only keep its containers' colliders apart.
const FIRST_CONTAINER_ID: u16 = 512;

pub struct SceneLodPlugin;

impl Plugin for SceneLodPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LodTint>()
            .init_resource::<ImposterTiling>()
            .init_asset::<Descriptor>()
            .register_asset_loader(DescriptorLoader)
            .add_console_command::<LodTintCommand, _>(set_lod_tint)
            .add_console_command::<LodStateCommand, _>(lod_state)
            .add_debug_section(DebugSection {
                title: "Scene stand-ins",
                query: "/scene_lod_state",
                fields: &[
                    ("Live scene ring", "rings.live"),
                    ("Live keep ring", "rings.live_keep"),
                    ("Near LOD ring", "rings.near"),
                    ("Near LOD keep ring", "rings.near_keep"),
                    ("Far LODs start at ring", "rings.far_start"),
                    ("Far LOD ring", "rings.far"),
                    ("Near LOD colliders", "near.colliders"),
                    ("Near LODs shown", "near.shown"),
                    ("Near LODs loading", "near.loading"),
                    (
                        "Near LODs not drawn (behind, out of range or over the limit)",
                        "near.waiting",
                    ),
                    ("Near LOD count (from the ring)", "near.max"),
                    ("Scenes without a near LOD", "near.none"),
                    ("Far LODs shown", "far.shown"),
                    ("Far LODs loading", "far.loading"),
                    (
                        "Far LODs not drawn (behind, out of range or over the limit)",
                        "far.waiting",
                    ),
                    ("Far LOD count (from the ring)", "far.max"),
                    ("Scenes without a far LOD", "far.none"),
                    ("Far LODs the server has none for", "far.missing"),
                    ("Far LOD requests failed in a row", "far.failures"),
                    ("Far LODs paused (seconds to the next try)", "far.paused_s"),
                ],
                actions: &[],
                inputs: &[DebugInput {
                    label: "Tint stand-ins (near LOD green, far LOD blue)",
                    field: "tint",
                    command: "/lod_tint {value}",
                    kind: DebugInputKind::Toggle,
                }],
            });
        app.add_systems(
            Update,
            (
                mark_shown_scenes,
                update_scene_lods,
                start_scene_lods,
                update_lod_visuals,
                show_scene_lods,
                tint_scene_lods.run_if(resource_changed::<LodTint>),
            )
                .chain()
                .in_set(SceneSets::Init),
        );
    }
}

/// Debug view: stand-in models are drawn tinted, so a near LOD can be told from the live scene
/// and from an imposter tile. Set from the Scene stand-ins debug section (`/lod_tint`).
#[derive(Resource, Default)]
pub struct LodTint(pub bool);

/// The near LOD's debug tint, multiplied into each model's base colour.
const NEAR_LOD_TINT: LinearRgba = LinearRgba::new(0.3, 1.0, 0.3, 1.0);

/// The far LOD's debug tint.
const FAR_LOD_TINT: LinearRgba = LinearRgba::new(0.3, 0.5, 1.0, 1.0);

fn tinted(color: Color, level: LodLevel) -> Color {
    let color = color.to_linear();
    let tint = match level {
        LodLevel::Near => NEAR_LOD_TINT,
        LodLevel::Far => FAR_LOD_TINT,
    };
    Color::LinearRgba(LinearRgba::new(
        color.red * tint.red,
        color.green * tint.green,
        color.blue * tint.blue,
        color.alpha,
    ))
}

/// Sets the debug tint on a stand-in's materials, or takes it off. They are the stand-in's own
/// (its `SceneResourceLookup`), each made from a glTF material that keeps the untinted colour.
fn apply_tint(
    tint: bool,
    level: LodLevel,
    lookup: &SceneResourceLookup,
    base_materials: &Assets<StandardMaterial>,
    materials: &mut Assets<SceneMaterial>,
) {
    for (base, material) in &lookup.materials {
        let (Some(base), Some(material)) = (base_materials.get(base), materials.get_mut(material))
        else {
            continue;
        };
        material.base.base_color = if tint {
            tinted(base.base_color, level)
        } else {
            base.base_color
        };
    }
}

/// Applies or removes the debug tint on every stand-in already shown; one shown later takes the
/// tint as it shows (`show_scene_lods`).
fn tint_scene_lods(
    tint: Res<LodTint>,
    lods: Query<(&SceneLod, &SceneResourceLookup)>,
    base_materials: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<SceneMaterial>>,
) {
    for (lod, lookup) in lods.iter().filter(|(lod, _)| lod.shown()) {
        apply_tint(tint.0, lod.level, lookup, &base_materials, &mut materials);
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lod_tint")]
struct LodTintCommand {
    #[arg(action = clap::ArgAction::Set)]
    enabled: bool,
}

fn set_lod_tint(mut input: ConsoleCommand<LodTintCommand>, mut tint: ResMut<LodTint>) {
    if let Some(Ok(command)) = input.take() {
        tint.0 = command.enabled;
        input.reply_ok(format!(
            "Stand-in tint {}",
            if tint.0 { "on" } else { "off" }
        ));
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_lod_state")]
struct LodStateCommand;

/// The active rings and the stand-ins by state, for the Scene stand-ins debug section, with the
/// far LODs' missing meshes and breaker (`IpfsIo::bevy_far_lod`): `paused_s` is null unless far LODs are paused.
/// `colliders` counts near LODs whose colliders part has started.
fn lod_state(
    mut input: ConsoleCommand<LodStateCommand>,
    tint: Res<LodTint>,
    config: Res<AppConfig>,
    ipfs: Res<IpfsResource>,
    lods: Query<&SceneLod>,
) {
    if input.take().is_none() {
        return;
    }
    // waiting, loading, shown, none, colliders; per level
    let mut counts = [[0u32; 5]; 2];
    for lod in &lods {
        let level = match lod.level {
            LodLevel::Near => 0,
            LodLevel::Far => 1,
        };
        let state = match (&lod.state, &lod.visuals) {
            (LodState::Ready(_), Visuals::Off) => 0,
            (LodState::Ready(_), Visuals::Loading) => 1,
            (LodState::Ready(_), Visuals::Shown) => 2,
            (LodState::Ready(_), Visuals::Nothing) | (LodState::Nothing, _) => 3,
            _ => 0,
        };
        counts[level][state] += 1;
        if matches!(lod.state, LodState::Ready(_)) {
            counts[level][4] += 1;
        }
    }
    let mut near = level_counts(counts[0]);
    near["max"] = json!(config.near_lods_cap());
    near["colliders"] = json!(counts[0][4]);
    let mut far = level_counts(counts[1]);
    far["max"] = json!(config.far_lods_cap());
    let health = ipfs.far_lod_status();
    far["missing"] = json!(health.missing);
    far["failures"] = json!(health.failures);
    far["paused_s"] = json!(health.paused_for.map(|left| left.as_secs()));
    let rings = Rings::from_config(&config);
    let rings = json!({
        "live": rings.live,
        "live_keep": rings.live_keep,
        "near": rings.near_start,
        "near_keep": rings.near_keep,
        "far_start": rings.far_start,
        "far": rings.far,
    });
    input.reply_ok(json!({"tint": tint.0, "rings": rings, "near": near, "far": far}).to_string());
}

fn level_counts([waiting, loading, shown, none, _]: [u32; 5]) -> serde_json::Value {
    json!({"waiting": waiting, "loading": loading, "shown": shown, "none": none})
}

/// One stand-in of one scene: the root its visual containers and, for a near LOD, its colliders
/// root ([`StandInColliders`]) hang from, at the scene's base parcel (a near LOD's once its
/// entity definition is loaded, a far LOD's as the realm listed it). While it exists the scene's
/// hash ranks its downloads in the LOD tier (the hooks), below every live scene's; once the scene
/// spawns, its own registration takes the hash to the scene tier. A scene may have a near and a
/// far one at once; the priority registrations are counted.
#[derive(Component)]
#[component(on_insert = register_lod_hash, on_replace = release_lod_hash)]
pub struct SceneLod {
    pub hash: String,
    pub(crate) level: LodLevel,
    /// the realm's urn for the scene, when it gave one: a city loader's carries the content base
    /// url. The near LOD loads the entity definition by it
    urn: Option<String>,
    /// the base parcel the realm listed for the scene (`ScenePointers::base`), which a far LOD is
    /// placed at: its mesh is relative to it
    base: Option<IVec2>,
    /// the scene's parcels: those the realm's pointers name within the level's ring (the near
    /// LOD's keep ring, the scan ring), which are all its partition needs; a near LOD's become all
    /// of them once its entity definition is loaded. A far LOD's stay those within the scan ring
    /// (`Rings::scan`): its bounds and the parcels the imposters leave to it
    pub parcels: Vec<IVec2>,
    pub state: LodState,
    pub visuals: Visuals,
}

impl SceneLod {
    /// Its visuals are drawn: they cover the scene's parcels.
    pub fn shown(&self) -> bool {
        matches!(self.visuals, Visuals::Shown)
    }
}

pub enum LodState {
    /// not started: no partition yet, (near) the scene past the start ring, or (far) no mesh
    /// offered yet (the GPU's BC support not reported)
    Pending,
    /// (near) the scene's entity definition is loading, as the live scene loads it; the root holds
    /// the handle too (`SceneEntityDefinitionHandle`), which the containers' loads read
    Definition(Handle<EntityDefinition>),
    /// (near) the descriptor is loading
    Descriptor {
        definition: Handle<EntityDefinition>,
        descriptor: Handle<Descriptor>,
    },
    /// the models are known: a near LOD's colliders part is loading or built, a far LOD's one
    /// model is its mesh. The visuals follow their own rule ([`Visuals`])
    Ready(Vec<StandInModel>),
    /// nothing to stand in: no descriptor, a definition or descriptor that could not load, no
    /// model the scene deployed, or (far) no mesh offered. Not asked again while the scene stays
    /// in the level's band
    Nothing,
}

/// A model the descriptor places, by the scene's file name for it; a far LOD's mesh, by its url.
#[derive(Clone)]
pub struct StandInModel {
    file: String,
    transform: Transform,
}

/// The visuals of a stand-in whose models are known ([`LodState::Ready`]).
pub enum Visuals {
    /// not drawn: not in view, outside the level's band, or no free place
    Off,
    /// the visual containers are loading and processing; the root stays hidden until each is
    /// done
    Loading,
    Shown,
    /// no model loaded. Not tried again while the scene stays within the keep ring
    Nothing,
}

impl Visuals {
    /// Takes a place under the near LOD count.
    fn started(&self) -> bool {
        matches!(self, Visuals::Loading | Visuals::Shown)
    }
}

/// The root of a stand-in's colliders part: a hidden child of the stand-in's root, with the
/// collider data its models' colliders join and a resource lookup of its own, so its meshes, held
/// in main-world memory only, are never handed to a visual container.
#[derive(Component)]
pub struct StandInColliders;

/// `{sceneId}_InitialSceneState.json`: the scene's models by content hash, each at its transform
/// relative to the base parcel, in SDK coordinates. It lists only glTFs the scene deployed: a
/// model the scene loads from a url, a primitive mesh, text and materials the scene's code sets
/// are not in it.
#[derive(Asset, TypePath, Deserialize)]
pub struct Descriptor {
    assets: Vec<DescriptorAsset>,
}

#[derive(Deserialize)]
struct DescriptorAsset {
    hash: String,
    position: Xyz,
    rotation: Xyzw,
    scale: Xyz,
}

#[derive(Deserialize, Clone, Copy)]
struct Xyz {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Deserialize, Clone, Copy)]
struct Xyzw {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl DescriptorAsset {
    /// The conversion a scene's `Transform` component gets, its guards included: a value that
    /// is not finite (an out-of-range number reads as infinity) falls back as it does there.
    fn transform(&self) -> Transform {
        DclTransformAndParent {
            translation: DclTranslation([self.position.x, self.position.y, self.position.z]),
            rotation: DclQuat([
                self.rotation.x,
                self.rotation.y,
                self.rotation.z,
                self.rotation.w,
            ]),
            scale: Vec3::new(self.scale.x, self.scale.y, self.scale.z),
            parent: SceneEntityId::ROOT,
        }
        .to_bevy_transform()
    }
}

/// Reads a descriptor; picked by the asset type, whatever the url's extension.
#[derive(Default)]
struct DescriptorLoader;

impl AssetLoader for DescriptorLoader {
    type Asset = Descriptor;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn register_lod_hash(world: DeferredWorld, context: HookContext) {
    let (Some(ipfs), Some(lod)) = (
        world.get_resource::<IpfsResource>(),
        world.get::<SceneLod>(context.entity),
    ) else {
        return;
    };
    let key = world
        .get::<Partition>(context.entity)
        .map_or(LoadPriority::LOD_PENDING, |partition| {
            partition.0.load_priority(Tier::Lod)
        });
    ipfs.register_download_priority(&lod.hash, key);
}

fn release_lod_hash(world: DeferredWorld, context: HookContext) {
    let (Some(ipfs), Some(lod)) = (
        world.get_resource::<IpfsResource>(),
        world.get::<SceneLod>(context.entity),
    ) else {
        return;
    };
    ipfs.release_download_priority(&lod.hash);
}

/// A live scene whose content has been on screen. Its stand-in is not wanted again even if the
/// content stops showing for a while (an in-client imposter bake, say): a stand-in would then
/// draw over the live content.
#[derive(Component)]
pub struct ContentWasShown;

fn mark_shown_scenes(
    mut commands: Commands,
    scenes: Query<(Entity, &RendererSceneContext), Without<ContentWasShown>>,
) {
    for (entity, context) in &scenes {
        if context.content_shown() {
            commands.entity(entity).try_insert(ContentWasShown);
        }
    }
}

/// The scenes within the scan ring of the player's parcel (`Rings::scan`), by hash, from the
/// realm's pointers: the urn, the base parcel, the ring of the nearest parcel (the ring
/// `scene_partition` measures), the parcels within the scan ring and whether a far LOD fits
/// them. Read again only when the player's parcel, the scan ring, the pointers, the preview state
/// or the imposters' tile layout changes.
#[derive(Default)]
struct ScenesInRange {
    /// what the scenes were read for: the player's parcel, the scan ring, the pointers'
    /// generation and preview
    read_for: Option<(IVec2, u16, u64, bool)>,
    scenes: HashMap<String, InRange>,
}

struct InRange {
    urn: Option<String>,
    base: Option<IVec2>,
    ring: u16,
    parcels: Vec<IVec2>,
    /// a far LOD may stand in: the base parcel is known, which places its mesh, and the parcels
    /// within the ring fit the imposter tiles (`far_lod_fits`)
    far_fits: bool,
}

impl ScenesInRange {
    fn read(
        &mut self,
        center: IVec2,
        scan: u16,
        is_preview: bool,
        pointers: &ScenePointers,
        tiling: &ImposterTiling,
    ) {
        self.read_for = Some((center, scan, pointers.generation(), is_preview));
        self.scenes.clear();
        if is_preview || pointers.min() == IVec2::MAX {
            return;
        }
        let ring = IVec2::splat(scan as i32);
        let min = (center - ring).max(pointers.min());
        let max = (center + ring).min(pointers.max());
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                let parcel = IVec2::new(x, y);
                if let Some(PointerResult::Exists { hash, urn }) = pointers.get(parcel) {
                    let distance = (parcel - center).abs().max_element() as u16;
                    let scene = self.scenes.entry(hash.clone()).or_insert_with(|| InRange {
                        urn: urn.clone(),
                        base: pointers.base(hash),
                        ring: u16::MAX,
                        parcels: Vec::new(),
                        far_fits: false,
                    });
                    scene.ring = scene.ring.min(distance);
                    scene.parcels.push(parcel);
                }
            }
        }
        for scene in self.scenes.values_mut() {
            scene.far_fits = scene.base.is_some() && far_lod_fits(tiling, &scene.parcels);
        }
    }
}

/// Which scenes have which stand-in, in a realm (never in preview), for a scene whose content
/// has not been on screen. A near LOD while a parcel of it is within the near keep ring; only one
/// within the start ring starts (`start_scene_lods`), and one between the two rings is kept, or
/// waits. A far LOD while its scene is within the far ring, the realm listed its base parcel and
/// its parcels within the scan ring fit the imposter tiles, whether or not a near LOD of the scene
/// shows: its visuals stop while the near LOD's show, and before the far start ring they start
/// only to stand in for a near LOD that is not drawn (`lod_visual_changes`). A stand-in goes when
/// its scene's content is first shown, when the scene leaves its band, or when the realm's
/// pointers no longer name it. A far LOD's parcels are those it was spawned with. The far LOD's
/// edge follows the imposter tiles with no hysteresis: see `FAR_LOD_MAX_TILE_LEVEL`.
fn update_scene_lods(
    mut commands: Commands,
    preview: Res<PreviewMode>,
    focus: Query<&GlobalTransform, With<PrimaryUser>>,
    pointers: Res<ScenePointers>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<(&RendererSceneContext, Has<ContentWasShown>)>,
    mut lods: Query<(Entity, &mut SceneLod)>,
    tiling: Res<ImposterTiling>,
    ipfs: Res<IpfsResource>,
    config: Res<AppConfig>,
    mut in_range: Local<ScenesInRange>,
) {
    let Ok(focus) = focus.single() else {
        return;
    };
    let rings = Rings::from_config(&config);
    let center = vec3_to_parcel(focus.translation());
    if in_range.read_for
        != Some((
            center,
            rings.scan(),
            pointers.generation(),
            preview.is_preview,
        ))
        || tiling.is_changed()
    {
        in_range.read(center, rings.scan(), preview.is_preview, &pointers, &tiling);
    }
    let scenes = &in_range.scenes;
    // no far LOD can be offered this session, or none may show (Distant Scene Rendering off or
    // a far LOD count of 0): none is kept or spawned
    let far_off = ipfs.far_lods_off() || config.far_lods_cap() == 0;

    let shown = |hash: &str| {
        live_scenes
            .scenes
            .get(hash)
            .and_then(|entity| contexts.get(*entity).ok())
            .is_some_and(|(context, was_shown)| was_shown || context.content_shown())
    };
    let mut existing: HashSet<(&str, LodLevel)> = HashSet::new();
    for (entity, mut lod) in &mut lods {
        let scene = scenes.get_key_value(lod.hash.as_str());
        let keep = !shown(&lod.hash)
            && match lod.level {
                LodLevel::Near => scene.is_some_and(|(_, scene)| scene.ring <= rings.near_keep),
                LodLevel::Far => {
                    !far_off
                        && scene.is_some_and(|(_, scene)| scene.ring <= rings.far && scene.far_fits)
                }
            };
        if let (true, Some((hash, scene))) = (keep, scene) {
            // until its entity definition gives all of them, a near LOD's parcels are those
            // within the keep ring now: its partition, and so whether it starts, reads them
            let unknown = matches!(lod.state, LodState::Pending | LodState::Definition(_));
            if lod.level == LodLevel::Near && unknown {
                let parcels = near_parcels(&scene.parcels, center, rings.near_keep);
                if lod.parcels != parcels {
                    lod.parcels = parcels;
                }
            }
            existing.insert((hash.as_str(), lod.level));
            continue;
        }
        // its containers go with it, and release their models (`gltf_container`'s observers)
        commands.entity(entity).despawn();
    }

    for (hash, scene) in scenes {
        if shown(hash) {
            continue;
        }
        let near = scene.ring <= rings.near_keep;
        let far = !far_off && scene.ring <= rings.far && scene.far_fits;
        for (level, wanted) in [(LodLevel::Near, near), (LodLevel::Far, far)] {
            if !wanted || existing.contains(&(hash.as_str(), level)) {
                continue;
            }
            let parcels = match level {
                LodLevel::Near => near_parcels(&scene.parcels, center, rings.near_keep),
                LodLevel::Far => scene.parcels.clone(),
            };
            commands.spawn((
                SceneLod {
                    hash: hash.clone(),
                    level,
                    urn: scene.urn.clone(),
                    base: scene.base,
                    parcels,
                    state: LodState::Pending,
                    visuals: Visuals::Off,
                },
                Transform::default(),
                Visibility::Hidden,
                // what the live container systems keep on a scene root
                SceneResourceLookup::default(),
                ComponentTracker::default(),
            ));
        }
    }
}

/// A scene's parcels within the near keep ring of `center`: a near LOD's until its entity
/// definition gives all of them.
fn near_parcels(parcels: &[IVec2], center: IVec2, near_keep: u16) -> Vec<IVec2> {
    parcels
        .iter()
        .copied()
        .filter(|parcel| (*parcel - center).abs().max_element() <= near_keep as i32)
        .collect()
}

type LodRow<'a> = (Entity, &'a SceneLod, Option<&'a Partition>);

/// Whether a stand-in that has not started may start. A near LOD within the start ring,
/// whatever the facing: the ring bounds how many start, and each loads its colliders part and
/// nothing drawn. A far LOD wherever it exists (`update_scene_lods` keeps it to its band): starting
/// only names its mesh.
fn may_start(level: LodLevel, partition: Option<&Partition>, rings: &Rings) -> bool {
    partition.is_some_and(|partition| match level {
        LodLevel::Near => partition.0.parcel_distance <= rings.near_start,
        LodLevel::Far => true,
    })
}

/// Whether a stand-in's visuals may start where its scene is, not wholly behind the camera: a
/// near LOD's within the near start ring; a far LOD's from the far start ring, or nearer when its
/// scene's near LOD is not drawn (`near_waiting`), which past the near start ring is whenever its
/// visuals are not already shown: they cannot start there. `update_scene_lods` keeps far LODs
/// within the far ring.
fn in_draw_band(level: LodLevel, partition: &Partition, near_waiting: bool, rings: &Rings) -> bool {
    !partition.0.is_behind
        && match level {
            LodLevel::Near => partition.0.parcel_distance <= rings.near_start,
            LodLevel::Far => partition.0.parcel_distance >= rings.far_start || near_waiting,
        }
}

/// The visuals of `level` to start and to stop this frame, keeping at most `max` started. Those
/// whose models are known and that are in the level's band (`in_draw_band`) start, nearest first
/// (by the download key: ring, then distance), while there is a free place. With none free, one
/// started behind the camera makes way for each in view, the farthest first; one in view never
/// does. Once the setting is lowered, the farthest over it stop, those behind the camera first.
/// Visuals with no partition yet are kept. At most `max_starts` start, and those of the scenes in
/// `blocked` never do.
fn visual_changes(
    lods: &[LodRow<'_>],
    level: LodLevel,
    max: usize,
    near_waiting: &HashSet<&str>,
    rings: &Rings,
    max_starts: usize,
    blocked: &HashSet<&str>,
) -> (Vec<Entity>, Vec<Entity>) {
    // started visuals, those behind the camera first, then the farthest first
    let mut started = lods
        .iter()
        .filter(|(_, lod, _)| lod.level == level && lod.visuals.started())
        .map(|(entity, _, partition)| {
            let key = partition.map(|partition| partition.0.load_priority(Tier::Lod));
            (key, *entity)
        })
        .collect::<Vec<_>>();
    let mut count = started.len();
    started.retain(|(key, _)| key.is_some());
    started.sort_by_key(|(key, _)| {
        (
            key.is_some_and(|key| !key.is_behind),
            std::cmp::Reverse(*key),
        )
    });
    let mut evictable = started.into_iter().peekable();

    let mut candidates = lods
        .iter()
        .filter(|(_, lod, _)| {
            lod.level == level
                && matches!(lod.state, LodState::Ready(_))
                && matches!(lod.visuals, Visuals::Off)
                && !blocked.contains(lod.hash.as_str())
        })
        .filter_map(|(entity, lod, partition)| {
            partition
                .filter(|partition| {
                    in_draw_band(
                        level,
                        partition,
                        near_waiting.contains(lod.hash.as_str()),
                        rings,
                    )
                })
                .map(|partition| (partition.0.load_priority(Tier::Lod), *entity))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(key, _)| *key);

    let mut stop = Vec::new();
    while count > max {
        let Some((_, entity)) = evictable.next() else {
            break;
        };
        stop.push(entity);
        count -= 1;
    }
    let mut start = Vec::new();
    for (_, entity) in candidates.into_iter().take(max_starts) {
        if count < max {
            start.push(entity);
            count += 1;
            continue;
        }
        // full: one behind the camera makes way, never one in view
        match evictable.next_if(|(key, _)| key.is_some_and(|key| key.is_behind)) {
            Some((_, behind)) => {
                stop.push(behind);
                start.push(entity);
            }
            None => break,
        }
    }
    (start, stop)
}

/// The visuals to start and to stop this frame: near LODs under the near LOD count, then far LODs
/// under the far LOD count, at most `far_max_starts` of them starting. A scene in the near band whose near
/// LOD is not drawn this frame (no free place, stopped, or nothing to show) may draw its far LOD
/// instead; while its near LOD is shown, the scene's far visuals stop and none start.
fn lod_visual_changes(
    lods: &[LodRow<'_>],
    config: &AppConfig,
    far_max_starts: usize,
) -> (Vec<Entity>, Vec<Entity>) {
    let rings = Rings::from_config(config);
    let none = HashSet::new();
    let (mut start, mut stop) = visual_changes(
        lods,
        LodLevel::Near,
        config.near_lods_cap(),
        &none,
        &rings,
        usize::MAX,
        &none,
    );
    let near_waiting: HashSet<&str> = lods
        .iter()
        .filter(|(entity, lod, partition)| {
            let Some(partition) =
                partition.filter(|p| lod.level == LodLevel::Near && !p.0.is_behind)
            else {
                return false;
            };
            if in_draw_band(LodLevel::Near, partition, false, &rings) {
                match (&lod.state, &lod.visuals) {
                    (LodState::Ready(_), Visuals::Off) => !start.contains(entity),
                    (LodState::Ready(_), Visuals::Loading | Visuals::Shown) => {
                        stop.contains(entity)
                    }
                    (LodState::Ready(_), Visuals::Nothing) | (LodState::Nothing, _) => true,
                    _ => false,
                }
            } else {
                // between the start and keep rings a near LOD's visuals cannot start: unless
                // they already show, it is not drawn, so the far LOD may stand in there too
                partition.0.parcel_distance <= rings.near_keep
                    && !(lod.visuals.started() && !stop.contains(entity))
            }
        })
        .map(|(_, lod, _)| lod.hash.as_str())
        .collect();
    let near_drawn: HashSet<&str> = lods
        .iter()
        .filter(|(entity, lod, _)| {
            lod.level == LodLevel::Near && lod.shown() && !stop.contains(entity)
        })
        .map(|(_, lod, _)| lod.hash.as_str())
        .collect();
    let (far_start, mut far_stop) = visual_changes(
        lods,
        LodLevel::Far,
        config.far_lods_cap(),
        &near_waiting,
        &rings,
        far_max_starts,
        &near_drawn,
    );
    for (entity, lod, _) in lods {
        if lod.level == LodLevel::Far
            && lod.visuals.started()
            && near_drawn.contains(lod.hash.as_str())
            && !far_stop.contains(entity)
        {
            far_stop.push(*entity);
        }
    }
    start.extend(far_start);
    stop.extend(far_stop);
    (start, stop)
}

/// Starts a stand-in once it may (`may_start`). A far LOD names the mesh the variants client
/// offers for the scene's hash and goes at the base parcel the realm listed, under the scene's
/// hash so its download ranks with the scene's: one request, with no entity definition. One the
/// client does not offer (none on the server this session, BC support or the content context not
/// known yet, ...) waits or has nothing to show; while one is not known yet, the others wait
/// for the next frame without asking. The download itself starts with the visuals
/// (`update_lod_visuals`). A near LOD loads the scene's entity definition as the live scene does, then
/// the descriptor, then gives each model the descriptor names among the scene's deployed files a
/// colliders-part container under a colliders root ([`StandInColliders`]). The visuals start
/// apart (`update_lod_visuals`).
fn start_scene_lods(
    mut commands: Commands,
    mut lods: Query<(Entity, &mut SceneLod, Option<&Partition>)>,
    definitions: Res<Assets<EntityDefinition>>,
    descriptors: Res<Assets<Descriptor>>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<(), With<RendererSceneContext>>,
    ipfas: IpfsAssetServer,
    config: Res<AppConfig>,
    // absent without the render storage-buffer assets (tests), as for the live scene's bounds
    mut storage_buffers: Option<ResMut<Assets<ShaderStorageBuffer>>>,
) {
    let rings = Rings::from_config(&config);
    // set once a far LOD's source is not known yet: the answer is the same for every scene
    let mut far_not_yet = false;
    for (root, lod, partition) in lods.iter_mut() {
        let lod = lod.into_inner();
        match &lod.state {
            LodState::Pending => {
                if !may_start(lod.level, partition, &rings) {
                    continue;
                }
                // a near LOD's descriptor is under the variants base, known once it settles
                if lod.level == LodLevel::Near && ipfas.ipfs().bevy_variants_pending() {
                    continue;
                }
                if lod.level == LodLevel::Far {
                    // `update_scene_lods` makes a far LOD only for a scene with a listed base
                    let Some(base) = lod.base else {
                        lod.state = LodState::Nothing;
                        continue;
                    };
                    if far_not_yet {
                        continue;
                    }
                    let url = match ipfas.ipfs().bevy_far_lod(&lod.hash) {
                        FarLodSource::Url(url) => url,
                        FarLodSource::NotYet => {
                            far_not_yet = true;
                            continue;
                        }
                        FarLodSource::Unavailable(reason) => {
                            debug!("{}: no far LOD: {reason}", lod.hash);
                            lod.state = LodState::Nothing;
                            continue;
                        }
                    };
                    // the one mesh, a url loaded under the scene's hash so its download ranks
                    // under it. Nothing walks on a far LOD: it has no colliders part
                    commands.entity(root).insert((
                        placement(base, &lod.parcels, storage_buffers.as_deref_mut()),
                        StandInContent(lod.hash.clone()),
                    ));
                    lod.state = LodState::Ready(vec![StandInModel {
                        file: url,
                        transform: Transform::IDENTITY,
                    }]);
                    continue;
                }
                // the live scene loads the same handle: the entity file is fetched once
                let definition = match &lod.urn {
                    Some(urn) => match ipfas.load_urn::<EntityDefinition>(urn) {
                        Ok(definition) => definition,
                        Err(e) => {
                            debug!("{}: no near LOD: {e}", lod.hash);
                            lod.state = LodState::Nothing;
                            continue;
                        }
                    },
                    None => ipfas.load_hash::<EntityDefinition>(&lod.hash),
                };
                // the scene's Bevy variants manifest, alongside its entity file: the stand-in
                // loads the same converted files the live scene will
                ipfas.ipfs().request_bevy_variants(&lod.hash);
                commands
                    .entity(root)
                    .insert(SceneEntityDefinitionHandle(definition.clone()));
                lod.state = LodState::Definition(definition);
            }
            LodState::Definition(handle) => {
                let handle = handle.clone();
                match ipfas.load_state(handle.id()) {
                    LoadState::Loaded => (),
                    LoadState::Failed(e) => {
                        debug!("{}: no near LOD: entity definition: {e}", lod.hash);
                        lod.state = LodState::Nothing;
                        continue;
                    }
                    _ => continue,
                }
                let Some((definition, (base, parcels))) = definitions
                    .get(handle.id())
                    .and_then(|definition| Some((definition, scene_placement(definition)?)))
                else {
                    debug!(
                        "{}: no near LOD: no scene in its entity definition",
                        lod.hash
                    );
                    lod.state = LodState::Nothing;
                    continue;
                };
                // the scene's files resolve through its content collection. A live scene has
                // registered it already (`load_scene_json`) and may have added files to it while
                // running, which registering it again would drop
                let live = live_scenes
                    .scenes
                    .get(&lod.hash)
                    .is_some_and(|scene| contexts.contains(*scene));
                if !live {
                    ipfas.ipfs().add_collection(
                        definition.id.clone(),
                        definition.content.clone(),
                        None,
                        definition.metadata.as_ref().map(ToString::to_string),
                    );
                }
                commands.entity(root).insert(placement(
                    base,
                    &parcels,
                    storage_buffers.as_deref_mut(),
                ));
                lod.parcels = parcels;
                // a url the scene does not deploy, loaded under the scene's content path so it
                // ranks with the stand-in's other downloads
                let Some(url) = ipfas.ipfs().bevy_lod_descriptor(&lod.hash) else {
                    debug!("{}: no near LOD: bevy variants off", lod.hash);
                    lod.state = LodState::Nothing;
                    continue;
                };
                lod.state = match ipfas.load_content_file::<Descriptor>(&url, &lod.hash) {
                    Ok(descriptor) => LodState::Descriptor {
                        definition: handle,
                        descriptor,
                    },
                    Err(e) => {
                        debug!("{}: no near LOD: {e}", lod.hash);
                        LodState::Nothing
                    }
                };
            }
            LodState::Descriptor {
                definition,
                descriptor,
            } => {
                match ipfas.load_state(descriptor.id()) {
                    LoadState::Loaded => (),
                    LoadState::Failed(e) => {
                        // most often the pipeline has none for the scene
                        debug!("{}: no near LOD: descriptor: {e}", lod.hash);
                        lod.state = LodState::Nothing;
                        continue;
                    }
                    _ => continue,
                }
                let definition_handle = definition.clone();
                let (Some(definition), Some(descriptor)) = (
                    definitions.get(definition.id()),
                    descriptors.get(descriptor.id()),
                ) else {
                    lod.state = LodState::Nothing;
                    continue;
                };
                let models = stand_in_models(&lod.hash, definition, descriptor);
                if models.is_empty() {
                    lod.state = LodState::Nothing;
                    continue;
                }
                // the colliders part: hidden, and blocking as each model is processed
                let colliders = commands
                    .spawn((
                        StandInColliders,
                        Transform::default(),
                        Visibility::Hidden,
                        SceneEntityDefinitionHandle(definition_handle),
                        SceneResourceLookup::default(),
                        ComponentTracker::default(),
                        SceneColliderData::default(),
                        ChildOf(root),
                    ))
                    .id();
                spawn_containers(&mut commands, colliders, &models, GltfPart::Colliders);
                lod.state = LodState::Ready(models);
            }
            _ => (),
        }
    }
}

/// The most models a stand-in places; past it the rest are skipped, with a warning. Each model
/// is a container in each part.
const MAX_STAND_IN_MODELS: usize = 2048;

/// The models a descriptor places among the files the scene deployed. The descriptor names each
/// by its content hash; a hash the scene deployed under several names is taken under the first in
/// name order. An entry the scene did not deploy is skipped (a model it loads from a url), as is
/// one repeated at the same transform.
fn stand_in_models(
    scene: &str,
    definition: &EntityDefinition,
    descriptor: &Descriptor,
) -> Vec<StandInModel> {
    let mut files: HashMap<&str, &str> = HashMap::new();
    for (file, hash) in definition.content.values() {
        files
            .entry(hash.as_str())
            .and_modify(|first| *first = (*first).min(file.as_str()))
            .or_insert(file.as_str());
    }
    let mut placed = HashSet::new();
    let mut models = Vec::new();
    for asset in &descriptor.assets {
        let Some(file) = files.get(asset.hash.as_str()) else {
            debug!(
                "{scene}: near LOD skips {}: not a file the scene deployed",
                asset.hash
            );
            continue;
        };
        let transform = asset.transform();
        let bits = [
            transform.translation.to_array(),
            transform.scale.to_array(),
            transform.rotation.xyz().to_array(),
        ]
        .map(|v| v.map(f32::to_bits));
        if !placed.insert((asset.hash.as_str(), bits, transform.rotation.w.to_bits())) {
            continue;
        }
        if models.len() == MAX_STAND_IN_MODELS {
            warn!(
                "{scene}: near LOD places its first {MAX_STAND_IN_MODELS} models and skips the rest"
            );
            break;
        }
        models.push(StandInModel {
            file: (*file).to_owned(),
            transform,
        });
    }
    models
}

/// One container per model under `root`, each taking `part` of its model.
fn spawn_containers(
    commands: &mut Commands,
    root: Entity,
    models: &[StandInModel],
    part: GltfPart,
) {
    for (model, id) in models.iter().zip(FIRST_CONTAINER_ID..=u16::MAX) {
        commands.spawn((
            SceneEntity {
                root,
                scene_id: SceneId(root),
                id: SceneEntityId::new(id, 0),
            },
            GltfDefinition::from(PbGltfContainer {
                src: model.file.clone(),
                ..Default::default()
            }),
            part,
            model.transform,
            Visibility::default(),
            ChildOf(root),
        ));
    }
}

/// Starts and stops stand-ins' visuals by `lod_visual_changes`. Starting gives each model a visuals
/// container under the stand-in's root; stopping despawns them, hides the root and empties its
/// resource lookup, so the scene materials go and, once no live scene holds them, the glTFs with
/// their textures and meshes. The colliders part stays. While far LODs are paused (the breaker of
/// `IpfsIo::far_lod_may_request`) none start, and none stops to make way, except for the probe once
/// per cool-down: then one far LOD may start, so at most one makes way for it. Those waiting keep
/// their imposters and those shown stay.
fn update_lod_visuals(
    mut commands: Commands,
    mut lods: Query<(
        Entity,
        &mut SceneLod,
        Option<&Partition>,
        Option<&Children>,
        &mut SceneResourceLookup,
    )>,
    parts: Query<&GltfPart>,
    config: Res<AppConfig>,
    ipfs: Res<IpfsResource>,
    mut seen_recoveries: Local<u32>,
) {
    let far_status = ipfs.far_lod_status();
    if far_status.recoveries != *seen_recoveries {
        *seen_recoveries = far_status.recoveries;
        retry_failed_far_lods(
            lods.iter_mut()
                .filter(|(_, lod, ..)| matches!(lod.visuals, Visuals::Nothing))
                .map(|(_, lod, ..)| lod.into_inner()),
            |hash| ipfs.far_lod_missing(hash),
        );
    }
    // while paused no far LOD starts; once the cool-down has run out, one: the probe
    let far_max_starts = match far_status.paused_for {
        None => usize::MAX,
        Some(left) if left.is_zero() => 1,
        Some(_) => 0,
    };
    let (start, stop) = lod_visual_changes(
        &lods
            .iter()
            .map(|(entity, lod, partition, ..)| (entity, lod, partition))
            .collect::<Vec<_>>(),
        &config,
        far_max_starts,
    );
    for root in stop {
        let Ok((_, mut lod, _, children, mut lookup)) = lods.get_mut(root) else {
            continue;
        };
        lod.visuals = Visuals::Off;
        despawn_visuals(&mut commands, children, &parts);
        *lookup = SceneResourceLookup::default();
        commands.entity(root).insert(Visibility::Hidden);
    }
    for root in start {
        let Ok((_, mut lod, ..)) = lods.get_mut(root) else {
            continue;
        };
        let LodState::Ready(models) = &lod.state else {
            continue;
        };
        // the download starts here: the breaker's probe is spent only by one that does
        if lod.level == LodLevel::Far && !ipfs.far_lod_may_request() {
            continue;
        }
        spawn_containers(&mut commands, root, models, GltfPart::Visuals);
        lod.visuals = Visuals::Loading;
    }
}

/// Once far LODs resume after an outage, the far LODs whose mesh did not load meanwhile
/// ([`Visuals::Nothing`]) wait to start again, so the scenes that failed while the LOD service was
/// down are asked for again where the player is, not only after leaving the band. A scene the
/// server has no mesh for stays as it is.
fn retry_failed_far_lods<'a>(
    lods: impl Iterator<Item = &'a mut SceneLod>,
    is_missing: impl Fn(&str) -> bool,
) {
    for lod in lods {
        if lod.level == LodLevel::Far
            && matches!(lod.state, LodState::Ready(_))
            && matches!(lod.visuals, Visuals::Nothing)
            && !is_missing(&lod.hash)
        {
            lod.visuals = Visuals::Off;
        }
    }
}

/// Despawns a stand-in root's visuals containers, leaving its colliders root.
fn despawn_visuals(commands: &mut Commands, children: Option<&Children>, parts: &Query<&GltfPart>) {
    for child in children.into_iter().flat_map(|children| children.iter()) {
        if matches!(parts.get(child), Ok(GltfPart::Visuals)) {
            commands.entity(child).despawn();
        }
    }
}

/// Where a stand-in's root goes: at the scene's base parcel, its models' materials clipped to
/// `parcels`.
fn placement(
    base: IVec2,
    parcels: &[IVec2],
    storage_buffers: Option<&mut Assets<ShaderStorageBuffer>>,
) -> (Transform, StandInBounds) {
    let regions = scene_regions(parcels.iter().copied())
        .into_iter()
        .map(|region| BoundRegion::new(region.min, region.max, region.count))
        .collect::<Vec<_>>();
    let origin = base.as_vec2() * PARCEL_SIZE;
    (
        Transform::from_translation(Vec3::new(origin.x, 0.0, -origin.y)),
        StandInBounds(SceneBounds::new(&regions, storage_buffers)),
    )
}

/// The scene's base parcel and parcels, read from its `scene.json` as the live scene reads them.
fn scene_placement(definition: &EntityDefinition) -> Option<(IVec2, Vec<IVec2>)> {
    let meta = serde_json::from_value::<SceneMeta>(definition.metadata.clone()?).ok()?;
    let parcel = |pointer: &str| {
        let (x, y) = pointer.split_once(',')?;
        Some(IVec2::new(x.parse().ok()?, y.parse().ok()?))
    };
    let base = parcel(&meta.scene.base)?;
    let parcels = meta
        .scene
        .parcels
        .iter()
        .map(|pointer| parcel(pointer))
        .collect::<Option<Vec<_>>>()?;
    Some((base, parcels))
}

/// Shows a stand-in's visuals in one frame once each of its visuals containers is processed:
/// some model loaded, or nothing to show if none did (the parcels then stay the imposters'; the
/// colliders part stays).
fn show_scene_lods(
    mut commands: Commands,
    mut lods: Query<(
        Entity,
        &mut SceneLod,
        Option<&Children>,
        &SceneResourceLookup,
    )>,
    containers: Query<(&GltfPart, Option<&GltfProcessed>)>,
    parts: Query<&GltfPart>,
    tint: Res<LodTint>,
    base_materials: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<SceneMaterial>>,
) {
    // scenes whose near LOD is drawn from this frame on
    let mut near_shown: Vec<String> = Vec::new();
    for (root, mut lod, children, lookup) in lods.iter_mut() {
        if !matches!(lod.visuals, Visuals::Loading) {
            continue;
        }
        let Some(children) = children else {
            continue;
        };
        let mut processed = true;
        let mut any_model = false;
        for (part, done) in containers.iter_many(children.collection()) {
            if *part != GltfPart::Visuals {
                continue;
            }
            match done {
                Some(done) => any_model |= done.instance_id.is_some(),
                None => processed = false,
            }
        }
        if !processed {
            continue;
        }
        if any_model {
            lod.visuals = Visuals::Shown;
            if lod.level == LodLevel::Near {
                near_shown.push(lod.hash.clone());
            }
            commands.entity(root).insert(Visibility::Inherited);
            if tint.0 {
                apply_tint(true, lod.level, lookup, &base_materials, &mut materials);
            }
        } else {
            lod.visuals = Visuals::Nothing;
            despawn_visuals(&mut commands, Some(children), &parts);
        }
    }
    // the far LOD of a scene whose near LOD just showed is hidden at once, not drawn under it
    // until `update_lod_visuals` stops it next frame
    if near_shown.is_empty() {
        return;
    }
    for (root, lod, ..) in lods.iter() {
        if lod.level == LodLevel::Far
            && matches!(lod.visuals, Visuals::Loading | Visuals::Shown)
            && near_shown.contains(&lod.hash)
        {
            commands.entity(root).insert(Visibility::Hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use common::partition::PartitionData;

    use super::*;

    fn at_ring(parcel_distance: u16, is_behind: bool) -> Partition {
        Partition(PartitionData {
            sqr_distance: (parcel_distance as f32 * PARCEL_SIZE).powi(2),
            parcel_distance,
            is_behind,
        })
    }

    fn lod(state: LodState, visuals: Visuals) -> SceneLod {
        SceneLod {
            hash: "scene".to_owned(),
            level: LodLevel::Near,
            urn: None,
            base: None,
            parcels: Vec::new(),
            state,
            visuals,
        }
    }

    /// An ipfs client with no realm and nothing to read.
    fn ipfs_resource() -> IpfsResource {
        use bevy::asset::io::memory::{Dir, MemoryAssetReader};
        use ipfs::IpfsIo;

        IpfsResource {
            inner: std::sync::Arc::new(IpfsIo::new(
                false,
                Box::new(MemoryAssetReader {
                    root: Dir::default(),
                }),
                None,
                Default::default(),
                1,
                tokio::sync::mpsc::unbounded_channel().0,
            )),
        }
    }

    /// An ipfs client with a variants base, so far LODs can be offered.
    fn ipfs_resource_with_base() -> IpfsResource {
        let ipfs = ipfs_resource();
        ipfs.set_bevy_variants_base(Some("https://abgen.example"), "test");
        ipfs
    }

    /// Stand-ins with their models known at the given rings, `true` behind the camera, their
    /// visuals `started` or not drawn; their entities in the same order.
    fn rows(
        world: &mut World,
        stand_ins: &[(u16, bool, bool)],
    ) -> Vec<(Entity, SceneLod, Partition)> {
        stand_ins
            .iter()
            .map(|&(ring, behind, started)| {
                let visuals = if started {
                    Visuals::Loading
                } else {
                    Visuals::Off
                };
                (
                    world.spawn_empty().id(),
                    lod(LodState::Ready(Vec::new()), visuals),
                    at_ring(ring, behind),
                )
            })
            .collect()
    }

    fn as_rows(rows: &[(Entity, SceneLod, Partition)]) -> Vec<LodRow<'_>> {
        rows.iter()
            .map(|(entity, lod, partition)| (*entity, lod, Some(partition)))
            .collect()
    }

    fn near_changes(
        rows: &[(Entity, SceneLod, Partition)],
        max: usize,
    ) -> (Vec<Entity>, Vec<Entity>) {
        let rings = Rings::from_config(&AppConfig::default());
        visual_changes(
            &as_rows(rows),
            LodLevel::Near,
            max,
            &HashSet::new(),
            &rings,
            usize::MAX,
            &HashSet::new(),
        )
    }

    /// A near LOD starts its colliders part whatever the facing, within the start ring only; a
    /// far LOD starts wherever it exists.
    #[test]
    fn colliders_start_by_distance_alone() {
        let rings = Rings::from_config(&AppConfig::default());
        let near = LodLevel::Near;
        assert!(may_start(near, Some(&at_ring(1, true)), &rings));
        assert!(may_start(
            near,
            Some(&at_ring(rings.near_start, false)),
            &rings
        ));
        assert!(!may_start(
            near,
            Some(&at_ring(rings.near_keep, false)),
            &rings
        ));
        assert!(!may_start(near, None, &rings));
        assert!(may_start(LodLevel::Far, Some(&at_ring(9, true)), &rings));
    }

    fn from_settings(live_scene_ring: u16, near_lod_ring: u16, far_lod_ring: u16) -> Rings {
        Rings::from_config(&AppConfig {
            live_scene_ring,
            near_lod_ring,
            far_lod_ring,
            ..Default::default()
        })
    }

    /// At the default rings near visuals draw within ring 3; far visuals past ring 4, or nearer
    /// only when the scene's near LOD is not drawn; neither when the scene is wholly behind the
    /// camera.
    #[test]
    fn visuals_draw_in_their_bands() {
        let rings = Rings::from_config(&AppConfig::default());
        assert_eq!(rings, rings_of(1, 2, 3, 4, 5, 20));
        let band =
            |level, ring, behind, over| in_draw_band(level, &at_ring(ring, behind), over, &rings);
        assert!(band(LodLevel::Near, 3, false, false));
        assert!(!band(LodLevel::Near, 4, false, false));
        assert!(!band(LodLevel::Near, 2, true, false));
        assert!(band(LodLevel::Far, 5, false, false));
        assert!(!band(LodLevel::Far, 3, false, false));
        assert!(band(LodLevel::Far, 3, false, true));
        assert!(!band(LodLevel::Far, 6, true, false));
    }

    fn rings_of(
        live: u16,
        live_keep: u16,
        near_start: u16,
        near_keep: u16,
        far_start: u16,
        far: u16,
    ) -> Rings {
        Rings {
            live,
            live_keep,
            near_start,
            near_keep,
            far_start,
            far,
        }
    }

    /// The bands follow the ring settings: a wider near ring pushes the far LODs out with it.
    #[test]
    fn stand_ins_follow_the_ring_settings() {
        let rings = from_settings(1, 6, 30);
        assert_eq!(rings, rings_of(1, 2, 6, 7, 8, 30));
        let band = |level, ring| in_draw_band(level, &at_ring(ring, false), false, &rings);
        assert!(band(LodLevel::Near, 6));
        assert!(!band(LodLevel::Near, 7));
        assert!(!band(LodLevel::Far, 7));
        assert!(band(LodLevel::Far, 8));
        assert_eq!(rings.scan(), 30);
    }

    /// A near ring at or inside the live ring leaves no near band of its own, and the far LODs
    /// start past the live keep ring; a far ring inside the near keep ring leaves no normal
    /// far-start band (far LODs may still stand in for near LODs not drawn), and the
    /// scan still covers the near one.
    #[test]
    fn overlapping_rings_stay_ordered() {
        let rings = from_settings(3, 1, 20);
        assert_eq!(rings, rings_of(3, 4, 1, 2, 5, 20));
        assert!(!in_draw_band(
            LodLevel::Far,
            &at_ring(4, false),
            false,
            &rings
        ));
        assert!(in_draw_band(
            LodLevel::Far,
            &at_ring(5, false),
            false,
            &rings
        ));

        let rings = from_settings(1, 8, 5);
        assert_eq!(rings, rings_of(1, 2, 8, 9, 10, 5));
        assert_eq!(rings.scan(), 9);
    }

    /// Values past a setting's range, as a hand-edited config could hold, are clamped to it.
    #[test]
    fn rings_are_clamped_to_their_settings() {
        assert_eq!(
            from_settings(u16::MAX, 0, u16::MAX),
            rings_of(3, 4, 1, 2, 5, 40)
        );
        assert_eq!(from_settings(0, u16::MAX, 0), rings_of(0, 1, 8, 9, 10, 5));
    }

    /// A scene whose near visuals find no place draws its far LOD in their stead, under the far
    /// cap; one whose near visuals start does not.
    #[test]
    fn a_far_lod_stands_in_for_near_visuals_without_a_place() {
        let mut world = World::new();
        let mut rows = rows(&mut world, &[(1, false, false), (2, false, false)]);
        rows[0].1.hash = "first".to_owned();
        rows[1].1.hash = "second".to_owned();
        let far = |world: &mut World, hash: &str| {
            let mut lod = lod(LodState::Ready(Vec::new()), Visuals::Off);
            lod.level = LodLevel::Far;
            lod.hash = hash.to_owned();
            (world.spawn_empty().id(), lod, at_ring(2, false))
        };
        rows.push(far(&mut world, "first"));
        rows.push(far(&mut world, "second"));
        let config = AppConfig {
            near_lods_budget: Some(1),
            far_lods_budget: Some(5),
            ..Default::default()
        };
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
            (vec![rows[0].0, rows[3].0], vec![])
        );
    }

    /// While far LODs are paused none start; near LODs are unaffected.
    #[test]
    fn paused_far_lods_do_not_start() {
        let mut world = World::new();
        let mut lod = lod(LodState::Ready(Vec::new()), Visuals::Off);
        lod.level = LodLevel::Far;
        let rows = [(world.spawn_empty().id(), lod, at_ring(8, false))];
        let config = AppConfig {
            near_lods_budget: Some(1),
            far_lods_budget: Some(5),
            ..Default::default()
        };
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, 0),
            (vec![], vec![])
        );
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
            (vec![rows[0].0], vec![])
        );
    }

    /// Between the near LOD's start and keep rings its visuals cannot start, so the far LOD stands
    /// in there unless the near LOD already shows.
    #[test]
    fn a_far_lod_stands_in_between_the_near_rings() {
        let mut world = World::new();
        let ring = Rings::from_config(&AppConfig::default()).near_keep;
        let near = |world: &mut World, state, visuals| {
            (
                world.spawn_empty().id(),
                lod(state, visuals),
                at_ring(ring, false),
            )
        };
        let far = |world: &mut World| {
            let mut lod = lod(LodState::Ready(Vec::new()), Visuals::Off);
            lod.level = LodLevel::Far;
            (world.spawn_empty().id(), lod, at_ring(ring, false))
        };
        let config = AppConfig {
            near_lods_budget: Some(1),
            far_lods_budget: Some(5),
            ..Default::default()
        };
        // a near LOD that never started (after a teleport) or whose visuals are off
        for (state, visuals) in [
            (LodState::Pending, Visuals::Off),
            (LodState::Ready(Vec::new()), Visuals::Off),
        ] {
            let rows = [near(&mut world, state, visuals), far(&mut world)];
            assert_eq!(
                lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
                (vec![rows[1].0], vec![])
            );
        }
        // one that already shows keeps the scene
        let rows = [
            near(&mut world, LodState::Ready(Vec::new()), Visuals::Shown),
            far(&mut world),
        ];
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
            (vec![], vec![])
        );
    }

    /// At the probe one far LOD starts, so with every place taken one behind the camera makes way
    /// for it, not one for each far LOD waiting in view.
    #[test]
    fn the_probe_starts_one_far_lod() {
        let mut world = World::new();
        let mut far = |visuals, ring, behind| {
            let mut lod = lod(LodState::Ready(Vec::new()), visuals);
            lod.level = LodLevel::Far;
            (world.spawn_empty().id(), lod, at_ring(ring, behind))
        };
        let rows = [
            far(Visuals::Shown, 8, true),
            far(Visuals::Shown, 9, true),
            far(Visuals::Off, 6, false),
            far(Visuals::Off, 7, false),
        ];
        let config = AppConfig {
            near_lods_budget: Some(1),
            far_lods_budget: Some(2),
            ..Default::default()
        };
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, 1),
            (vec![rows[2].0], vec![rows[1].0])
        );
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
            (vec![rows[2].0, rows[3].0], vec![rows[1].0, rows[0].0])
        );
    }

    /// A far LOD whose scene's near LOD is shown stops its visuals and does not start them.
    #[test]
    fn a_far_lod_steps_aside_for_the_shown_near_lod() {
        let mut world = World::new();
        let far = |world: &mut World, visuals| {
            let mut lod = lod(LodState::Ready(Vec::new()), visuals);
            lod.level = LodLevel::Far;
            (world.spawn_empty().id(), lod, at_ring(2, false))
        };
        let rows = [
            (
                world.spawn_empty().id(),
                lod(LodState::Ready(Vec::new()), Visuals::Shown),
                at_ring(2, false),
            ),
            far(&mut world, Visuals::Shown),
        ];
        let config = AppConfig {
            near_lods_budget: Some(1),
            far_lods_budget: Some(5),
            ..Default::default()
        };
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
            (vec![], vec![rows[1].0])
        );
        // stopped, it is not started again while the near LOD shows
        let rows = [
            rows.into_iter().next().unwrap(),
            far(&mut world, Visuals::Off),
        ];
        assert_eq!(
            lod_visual_changes(&as_rows(&rows), &config, usize::MAX),
            (vec![], vec![])
        );
    }

    /// Visuals start in view, nearest first, while a place is free; every started one takes a
    /// place, behind the camera or not, and none behind the camera starts. With one place left,
    /// the nearest in view takes it and the started one behind the camera makes way for the next.
    #[test]
    fn visuals_start_in_view_nearest_first_within_the_cap() {
        let mut world = World::new();
        let rows = rows(
            &mut world,
            &[
                (1, false, true),
                (3, false, false),
                (2, false, false),
                (1, true, false),
                (1, true, true),
            ],
        );
        assert_eq!(near_changes(&rows, 4), (vec![rows[2].0, rows[1].0], vec![]));
        assert_eq!(
            near_changes(&rows, 3),
            (vec![rows[2].0, rows[1].0], vec![rows[4].0])
        );
        assert_eq!(near_changes(&rows, 2), (vec![rows[2].0], vec![rows[4].0]));
    }

    /// With every place taken, one started behind the camera makes way for one in view, the
    /// farthest first; one in view never does.
    #[test]
    fn one_behind_makes_way_for_one_in_view() {
        let mut world = World::new();
        let rows = rows(
            &mut world,
            &[
                (3, false, true),
                (1, true, true),
                (3, true, true),
                (2, false, false),
                (1, false, false),
                (3, false, false),
            ],
        );
        assert_eq!(
            near_changes(&rows, 3),
            (vec![rows[4].0, rows[3].0], vec![rows[2].0, rows[1].0])
        );
    }

    /// Lowering the near LOD count stops the farthest over it, those behind the camera first.
    #[test]
    fn lowering_the_cap_stops_behind_then_in_view() {
        let mut world = World::new();
        let rows = rows(
            &mut world,
            &[
                (1, false, true),
                (3, false, true),
                (1, true, true),
                (3, true, true),
            ],
        );
        assert_eq!(
            near_changes(&rows, 1),
            (vec![], vec![rows[3].0, rows[2].0, rows[1].0])
        );
    }

    fn container(world: &mut World, root: Entity, processed: Option<GltfProcessed>) -> Entity {
        let mut container = world.spawn((
            GltfDefinition::from(PbGltfContainer {
                src: "model.glb".to_owned(),
                ..Default::default()
            }),
            GltfPart::Visuals,
            ChildOf(root),
        ));
        if let Some(processed) = processed {
            container.insert(processed);
        }
        container.id()
    }

    /// Once far LODs resume, those whose mesh did not load wait to start again; a scene the
    /// server has no mesh for, a near LOD and one drawn are left as they are.
    #[test]
    fn failed_far_lods_are_asked_for_again_on_recovery() {
        let far = |visuals, hash: &str| {
            let mut lod = lod(LodState::Ready(Vec::new()), visuals);
            lod.level = LodLevel::Far;
            lod.hash = hash.to_owned();
            lod
        };
        let mut failed = far(Visuals::Nothing, "failed");
        let mut missing = far(Visuals::Nothing, "missing");
        let mut shown = far(Visuals::Shown, "shown");
        let mut near = lod(LodState::Ready(Vec::new()), Visuals::Nothing);
        let mut no_mesh = far(Visuals::Off, "no-mesh");
        no_mesh.state = LodState::Nothing;
        retry_failed_far_lods(
            [
                &mut failed,
                &mut missing,
                &mut shown,
                &mut near,
                &mut no_mesh,
            ]
            .into_iter(),
            |hash| hash == "missing",
        );
        assert!(matches!(failed.visuals, Visuals::Off));
        assert!(matches!(missing.visuals, Visuals::Nothing));
        assert!(matches!(shown.visuals, Visuals::Shown));
        assert!(matches!(near.visuals, Visuals::Nothing));
        assert!(matches!(no_mesh.visuals, Visuals::Off));
    }

    /// Visuals whose every model failed show nothing: they wait for the last container, then
    /// become `Nothing`, drop their containers and stay hidden. The colliders root stays.
    #[test]
    fn visuals_whose_models_all_fail_show_nothing() {
        let mut world = World::new();
        world.init_resource::<LodTint>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<SceneMaterial>>();
        let root = world
            .spawn((
                lod(LodState::Ready(Vec::new()), Visuals::Loading),
                Visibility::Hidden,
                SceneResourceLookup::default(),
            ))
            .id();
        let colliders = world.spawn((StandInColliders, ChildOf(root))).id();
        container(&mut world, root, Some(GltfProcessed::default()));
        let last = container(&mut world, root, None);

        world.run_system_once(show_scene_lods).unwrap();
        assert!(matches!(
            world.get::<SceneLod>(root).unwrap().visuals,
            Visuals::Loading
        ));

        world.entity_mut(last).insert(GltfProcessed::default());
        world.run_system_once(show_scene_lods).unwrap();
        assert!(matches!(
            world.get::<SceneLod>(root).unwrap().visuals,
            Visuals::Nothing
        ));
        assert_eq!(
            world
                .get::<Children>(root)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![colliders]
        );
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
    }

    /// The player standing in `parcel`.
    fn player_at(parcel: IVec2) -> GlobalTransform {
        GlobalTransform::from_translation(Vec3::new(
            parcel.x as f32 * PARCEL_SIZE + 8.0,
            0.0,
            -(parcel.y as f32 * PARCEL_SIZE + 8.0),
        ))
    }

    /// A waiting stand-in follows the parcels within the keep ring as the player moves, so a
    /// long scene first seen at one end starts once its nearest parcel is in range, however the
    /// player came at it.
    #[test]
    fn a_waiting_stand_in_follows_its_parcels_in_range() {
        let mut world = World::new();
        world.init_resource::<PreviewMode>();
        world.init_resource::<LiveScenes>();
        world.init_resource::<ImposterTiling>();
        world.insert_resource(ipfs_resource_with_base());
        world.init_resource::<AppConfig>();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::splat(-20), IVec2::splat(20));
        for x in 0..=10 {
            pointers.insert(
                IVec2::new(x, 0),
                PointerResult::Exists {
                    hash: "long".to_owned(),
                    urn: None,
                },
            );
        }
        world.insert_resource(pointers);
        let player = world
            .spawn((PrimaryUser::default(), player_at(IVec2::new(-4, 0))))
            .id();
        let parcels = |world: &mut World| {
            let mut parcels = world
                .query::<&SceneLod>()
                .single(world)
                .unwrap()
                .parcels
                .clone();
            parcels.sort_by_key(|parcel| parcel.x);
            parcels
        };

        world.run_system_once(update_scene_lods).unwrap();
        assert_eq!(parcels(&mut world), vec![IVec2::ZERO]);

        world
            .entity_mut(player)
            .insert(player_at(IVec2::new(6, -3)));
        world.run_system_once(update_scene_lods).unwrap();
        assert_eq!(
            parcels(&mut world),
            (2..=10).map(|x| IVec2::new(x, 0)).collect::<Vec<_>>()
        );
    }

    fn asset(hash: &str, x: f32) -> DescriptorAsset {
        DescriptorAsset {
            hash: hash.to_owned(),
            position: Xyz { x, y: 0.0, z: 0.0 },
            rotation: Xyzw {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: Xyz {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        }
    }

    /// The descriptor's entries become models by the files the scene deployed: an entry the
    /// scene did not deploy and one repeated at the same transform are skipped, a file deployed
    /// under two names takes the first, and a non-finite translation falls back to the origin.
    #[test]
    fn descriptor_entries_become_the_scenes_models() {
        let definition = EntityDefinition {
            content: ipfs::ContentMap::from_content(
                [
                    ("models/b.glb", "tree"),
                    ("models/a.glb", "tree"),
                    ("models/rock.glb", "rock"),
                ]
                .map(|(file, hash)| (file.to_owned(), hash.to_owned())),
            ),
            ..Default::default()
        };
        let descriptor = Descriptor {
            assets: vec![
                asset("tree", 1.0),
                asset("tree", 1.0),
                asset("tree", 2.0),
                asset("url-model", 3.0),
                asset("rock", f32::INFINITY),
            ],
        };
        let models = stand_in_models("scene", &definition, &descriptor);
        let placed = models
            .iter()
            .map(|model| (model.file.as_str(), model.transform.translation))
            .collect::<Vec<_>>();
        assert_eq!(
            placed,
            vec![
                ("models/a.glb", Vec3::X),
                ("models/a.glb", Vec3::X * 2.0),
                ("models/rock.glb", Vec3::ZERO),
            ]
        );
    }

    /// A world with the player in parcel (0,0) and one scene there, based there, live but not
    /// yet shown.
    fn world_with_live_scene() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<PreviewMode>();
        world.init_resource::<ImposterTiling>();
        world.insert_resource(ipfs_resource_with_base());
        world.init_resource::<AppConfig>();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::splat(-10), IVec2::splat(10));
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
        pointers.set_base("scene", IVec2::ZERO);
        world.insert_resource(pointers);
        world.spawn((
            PrimaryUser::default(),
            GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
        ));
        let scene = world.spawn_empty().id();
        let context = RendererSceneContext::new(
            SceneId(scene),
            "scene".to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            [IVec2::ZERO].into_iter().collect(),
            Vec::new(),
            SceneBounds::default(),
            Vec::new(),
            scene,
            0.0,
            false,
            "",
            false,
            false,
        );
        world.entity_mut(scene).insert(context);
        let mut live_scenes = LiveScenes::default();
        live_scenes.scenes.insert("scene".to_owned(), scene);
        world.insert_resource(live_scenes);
        (world, scene)
    }

    fn stand_ins(world: &mut World) -> usize {
        world.query::<&SceneLod>().iter(world).count()
    }

    /// A scene whose content has been on screen gets no stand-in again, even when its content
    /// stops showing (its tick count is wound back here to stand for that).
    #[test]
    fn a_scene_once_shown_never_restarts_its_stand_in() {
        let (mut world, scene) = world_with_live_scene();
        world.run_system_once(update_scene_lods).unwrap();
        // a near LOD, and a far LOD, which without imposters fits anywhere in the far ring
        assert_eq!(stand_ins(&mut world), 2);

        world
            .get_mut::<RendererSceneContext>(scene)
            .unwrap()
            .tick_number = 6;
        world.run_system_once(mark_shown_scenes).unwrap();
        assert!(world.get::<ContentWasShown>(scene).is_some());
        world.run_system_once(update_scene_lods).unwrap();
        assert_eq!(stand_ins(&mut world), 0);

        world
            .get_mut::<RendererSceneContext>(scene)
            .unwrap()
            .tick_number = 0;
        assert!(!world
            .get::<RendererSceneContext>(scene)
            .unwrap()
            .content_shown());
        world.run_system_once(update_scene_lods).unwrap();
        assert_eq!(stand_ins(&mut world), 0);
    }

    /// A stand-in's colliders block as soon as its colliders part registers them, while nothing
    /// of the stand-in is drawn: a player backing into a loading scene is stopped by it.
    #[test]
    fn a_stand_in_collides_while_nothing_is_drawn() {
        use bevy::{
            diagnostic::FrameCount,
            gltf::{Gltf, GltfMesh},
        };
        use dcl_component::proto_components::sdk::components::ColliderLayer;

        use crate::{
            update_world::mesh_collider::{update_colliders, CtCollider, MeshCollider},
            ContainerEntity,
        };

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Gltf>()
            .init_asset::<GltfMesh>()
            .init_resource::<FrameCount>()
            .insert_resource(ipfs_resource())
            .add_systems(Update, update_colliders::<CtCollider>);

        let stand_in = app
            .world_mut()
            .spawn((
                lod(LodState::Ready(Vec::new()), Visuals::Off),
                Visibility::Hidden,
            ))
            .id();
        let root = app
            .world_mut()
            .spawn((
                StandInColliders,
                Visibility::Hidden,
                SceneColliderData::default(),
                ChildOf(stand_in),
            ))
            .id();
        let node = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(node).insert((
            ContainerEntity {
                container: node,
                root,
                container_id: SceneEntityId::new(FIRST_CONTAINER_ID, 0),
            },
            GlobalTransform::IDENTITY,
            MeshCollider::<CtCollider>::default(),
        ));
        let blocks = |app: &mut App| {
            app.world_mut()
                .get_mut::<SceneColliderData>(root)
                .unwrap()
                .cast_ray_nearest(
                    Vec3::new(0.0, 5.0, 0.0),
                    Vec3::NEG_Y,
                    10.0,
                    ColliderLayer::ClPhysics as u32,
                    true,
                    false,
                    None,
                )
                .is_some()
        };

        assert!(!blocks(&mut app));
        app.update();
        assert!(blocks(&mut app));
    }

    /// the default imposter distances, focused on parcel (0,0)
    fn default_tiling() -> Tiling {
        Tiling {
            origin_parcel: IVec2::ZERO,
            min_distance: 0.0,
            distance_scale: 1.0,
            distances: vec![100.0, 200.0, 400.0, 800.0, 1600.0, 99999.0],
        }
    }

    /// The imposters' distance to a tile: from the focus parcel's centre (8, 8) to the tile's
    /// closest point, here a 4x4 tile west of the focus, at negative coordinates.
    #[test]
    fn tile_distance_is_to_the_closest_point() {
        let tiling = default_tiling();
        assert_eq!(tiling.tile_distance(IVec2::new(-16, 0), 2), 200.0);
        assert!(tiling.level_allowed(2, 200.0));
        assert!(!tiling.level_allowed(2, 199.0));
        assert!(tiling.level_allowed(0, 0.0));
    }

    #[test]
    fn tile_level_follows_the_imposter_distances() {
        let tiling = default_tiling();
        // the 2x2 tile around the focus is too close for level 1
        assert_eq!(tiling.level(IVec2::new(1, 0)), 0);
        // 152 m: its 4x4 tile starts 120 m away, too close for level 2; its 2x2 tile is level 1
        assert_eq!(tiling.level(IVec2::new(10, 0)), 1);
        assert_eq!(tiling.level(IVec2::new(-10, 0)), 1);
        // 248 m: its 4x4 tile is far enough for level 2
        assert_eq!(tiling.level(IVec2::new(16, 0)), 2);
    }

    #[test]
    fn far_lod_stays_out_of_larger_tiles() {
        let tiling = ImposterTiling::Known(default_tiling());
        assert!(far_lod_fits(
            &tiling,
            &[IVec2::new(10, 0), IVec2::new(12, 0)]
        ));
        // one parcel in a level-2 tile keeps the whole scene to the imposters
        assert!(!far_lod_fits(
            &tiling,
            &[IVec2::new(12, 0), IVec2::new(16, 0)]
        ));
        // without imposters only the far ring bounds it
        assert!(far_lod_fits(&ImposterTiling::Off, &[IVec2::new(16, 0)]));
        // before they publish a layout nothing fits
        assert!(!far_lod_fits(&ImposterTiling::Pending, &[IVec2::new(1, 0)]));
        assert!(!far_lod_fits(&ImposterTiling::Pending, &[]));
    }

    /// A far LOD is placed from the base parcel the realm listed for its scene: a scene listed
    /// without one gets only its near LOD. The far LOD keeps the parcels the pointers name.
    #[test]
    fn a_far_lod_needs_the_listed_base() {
        let (mut world, _) = world_with_live_scene();
        let mut pointers = world.resource_mut::<ScenePointers>();
        pointers.insert(
            IVec2::new(1, 0),
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
        world.run_system_once(update_scene_lods).unwrap();
        let far = world
            .query::<&SceneLod>()
            .iter(&world)
            .find(|lod| lod.level == LodLevel::Far)
            .unwrap();
        assert_eq!(far.base, Some(IVec2::ZERO));
        let mut parcels = far.parcels.clone();
        parcels.sort_by_key(|parcel| (parcel.x, parcel.y));
        assert_eq!(parcels, vec![IVec2::ZERO, IVec2::new(1, 0)]);

        // a new realm forgets the bases with the pointers
        let mut pointers = world.resource_mut::<ScenePointers>();
        pointers.set_realm(IVec2::splat(-10), IVec2::splat(10));
        assert_eq!(pointers.base("scene"), None);
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
        world.run_system_once(update_scene_lods).unwrap();
        let levels = world
            .query::<&SceneLod>()
            .iter(&world)
            .map(|lod| lod.level)
            .collect::<Vec<_>>();
        assert_eq!(levels, vec![LodLevel::Near]);
    }

    /// A far LOD starts from its scene's hash alone and never loads the entity definition. With
    /// no mesh offered (variants off) it has nothing to show; while none can be offered yet (the
    /// GPU not reported) it waits.
    #[test]
    fn a_far_lod_never_loads_the_entity_definition() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<EntityDefinition>()
            .init_asset::<Descriptor>()
            .init_resource::<LiveScenes>()
            .init_resource::<AppConfig>()
            .insert_resource(ipfs_resource());
        let far = |app: &mut App| {
            app.world_mut()
                .spawn((
                    SceneLod {
                        level: LodLevel::Far,
                        base: Some(IVec2::new(2, 3)),
                        ..lod(LodState::Pending, Visuals::Off)
                    },
                    at_ring(6, false),
                ))
                .id()
        };
        let untouched = |app: &mut App, root: Entity| {
            let root = app.world().entity(root);
            !root.contains::<SceneEntityDefinitionHandle>()
                && !root.contains::<StandInContent>()
                && !root.contains::<Children>()
        };

        let off = far(&mut app);
        app.world_mut().run_system_once(start_scene_lods).unwrap();
        assert!(matches!(
            app.world().get::<SceneLod>(off).unwrap().state,
            LodState::Nothing
        ));
        assert!(untouched(&mut app, off));

        app.world()
            .resource::<IpfsResource>()
            .set_bevy_variants_base(Some("https://abgen.example"), "test");
        let waiting = far(&mut app);
        app.world_mut().run_system_once(start_scene_lods).unwrap();
        assert!(matches!(
            app.world().get::<SceneLod>(waiting).unwrap().state,
            LodState::Pending
        ));
        assert!(untouched(&mut app, waiting));
    }
}
