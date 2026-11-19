//! A scene's stand-in while it is in range but not live, built from the asset pipeline's
//! initial-scene-state descriptor, which places the scene's own glTF files. It has two parts with
//! their own rules, each a glTF container per model on the live container path
//! (`gltf_container`), which loads it under the scene's content path and processes it a slice a
//! frame:
//! - Colliders, for every scene within the start ring whatever the facing, so a player who
//!   reaches a loading scene, backing in or walking in, is stopped by it. Each model loads a
//!   second time under an asset path of its own with no textures (`GltfPart::Colliders`); its
//!   colliders block as soon as the model is processed, whether or not anything is drawn, and the
//!   glTF goes then, leaving the collider shapes in main-world memory. A colliders part is never
//!   interrupted: it goes with its scene.
//! - Visuals, for the scenes in view under "Max Near LODs" (`GltfPart::Visuals`): the same asset
//!   the live scene loads, drawn once every model is processed. When the cap is full, one behind
//!   the camera makes way for one in view; dropping it frees its textures and meshes once nothing
//!   else holds the glTF, and coming back reloads from the disk cache the colliders part filled.
//!
//! The stand-in goes, both parts with it, in the frame the live scene's content is first on
//! screen; the live scene's containers made before then name the same files and share the loaded
//! glTFs.

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
    structs::{AppConfig, PreviewMode, SceneMeta},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::sdk::components::PbGltfContainer,
    transform_and_parent::{DclQuat, DclTranslation},
    SceneEntityId,
};
use dcl_types::SceneId;
use ipfs::{EntityDefinition, IpfsAssetServer, IpfsResource};
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
        },
        mesh_collider::SceneColliderData,
        ComponentTracker,
    },
    vec3_to_parcel, PrimaryUser, SceneEntity,
};

/// A scene that is not live starts a near LOD when its nearest parcel is within this Chebyshev
/// parcel ring of the player's parcel (`Partition::parcel_distance`): about 48 m.
pub const NEAR_LOD_START_RING: u16 = 3;

/// A near LOD is kept until its scene's nearest parcel is past this ring (about 64 m), so a
/// player walking along the edge does not start and drop it every few steps.
pub const NEAR_LOD_KEEP_RING: u16 = 4;

/// The scene entity id of a stand-in's first container; the rest follow. A stand-in has no
/// scene, so any id is free: they only keep its containers' colliders apart.
const FIRST_CONTAINER_ID: u16 = 512;

pub struct SceneLodPlugin;

impl Plugin for SceneLodPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LodTint>()
            .init_asset::<Descriptor>()
            .register_asset_loader(DescriptorLoader)
            .add_console_command::<LodTintCommand, _>(set_lod_tint)
            .add_console_command::<LodStateCommand, _>(lod_state)
            .add_debug_section(DebugSection {
                title: "Scene stand-ins",
                query: "/scene_lod_state",
                fields: &[
                    ("Near LOD colliders", "near.colliders"),
                    ("Near LODs shown", "near.shown"),
                    ("Near LODs loading", "near.loading"),
                    (
                        "Near LODs not drawn (behind, out of range or over the limit)",
                        "near.waiting",
                    ),
                    ("Max Near LODs (setting)", "near.max"),
                    ("Scenes without a near LOD", "near.none"),
                ],
                actions: &[],
                inputs: &[DebugInput {
                    label: "Tint stand-ins (near LOD green)",
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

fn tinted(color: Color) -> Color {
    let color = color.to_linear();
    let tint = NEAR_LOD_TINT;
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
            tinted(base.base_color)
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
    for (_, lookup) in lods.iter().filter(|(lod, _)| lod.shown()) {
        apply_tint(tint.0, lookup, &base_materials, &mut materials);
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

/// The stand-ins by state, for the Scene stand-ins debug section.
fn lod_state(
    mut input: ConsoleCommand<LodStateCommand>,
    tint: Res<LodTint>,
    config: Res<AppConfig>,
    lods: Query<&SceneLod>,
) {
    if input.take().is_none() {
        return;
    }
    let (mut colliders, mut waiting, mut loading, mut shown, mut none) = (0, 0, 0, 0, 0);
    for lod in &lods {
        match (&lod.state, &lod.visuals) {
            (LodState::Ready(_), visuals) => {
                colliders += 1;
                match visuals {
                    Visuals::Off => waiting += 1,
                    Visuals::Loading => loading += 1,
                    Visuals::Shown => shown += 1,
                    Visuals::Nothing => none += 1,
                }
            }
            (LodState::Nothing, _) => none += 1,
            _ => waiting += 1,
        }
    }
    input.reply_ok(
        json!({
            "tint": tint.0,
            "near": {"colliders": colliders, "waiting": waiting, "loading": loading,
                     "shown": shown, "none": none, "max": config.max_near_lods},
        })
        .to_string(),
    );
}

/// The stand-in of one scene: the root its visual containers and its colliders root
/// ([`StandInColliders`]) hang from, at the scene's base parcel once its entity definition is
/// loaded. While it exists the scene's hash ranks its downloads in the LOD tier (the hooks), below
/// every live scene's; once the scene spawns, its own registration takes the hash to the scene
/// tier.
#[derive(Component)]
#[component(on_insert = register_lod_hash, on_replace = release_lod_hash)]
pub struct SceneLod {
    pub hash: String,
    /// the realm's urn for the scene, when it gave one: a city loader's carries the content base
    /// url
    urn: Option<String>,
    /// the scene's parcels: until its entity definition is loaded, those the realm's pointers
    /// name within the keep ring, which are all its partition needs; then all of them
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
    /// not started: no partition yet, or the scene past the start ring
    Pending,
    /// the scene's entity definition is loading, as the live scene loads it; the root holds the
    /// handle too (`SceneEntityDefinitionHandle`), which the containers' loads read
    Definition(Handle<EntityDefinition>),
    /// the descriptor is loading
    Descriptor {
        definition: Handle<EntityDefinition>,
        descriptor: Handle<Descriptor>,
    },
    /// the models are known and their colliders part is loading or built; the visuals follow
    /// their own rule ([`Visuals`])
    Ready(Vec<StandInModel>),
    /// nothing to stand in: no descriptor, a definition or descriptor that could not load, or
    /// no model the scene deployed. Not asked again while the scene stays within the keep ring
    Nothing,
}

/// A model the descriptor places, by the scene's file name for it.
#[derive(Clone)]
pub struct StandInModel {
    file: String,
    transform: Transform,
}

/// The visuals of a stand-in whose models are known ([`LodState::Ready`]).
pub enum Visuals {
    /// not drawn: not in view, or no free place
    Off,
    /// the visual containers are loading and processing; the root stays hidden until each is
    /// done
    Loading,
    Shown,
    /// no model loaded. Not tried again while the scene stays within the keep ring
    Nothing,
}

impl Visuals {
    /// Takes a place under "Max Near LODs".
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
    /// The same conversion a scene's `Transform` component gets.
    fn transform(&self) -> Transform {
        let rotation = DclQuat([
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
            self.rotation.w,
        ])
        .to_bevy_quat()
        .normalize();
        let scale = Vec3::new(self.scale.x, self.scale.y, self.scale.z);
        Transform {
            translation: DclTranslation([self.position.x, self.position.y, self.position.z])
                .to_bevy_translation(),
            rotation: if rotation.is_finite() {
                rotation
            } else {
                Quat::IDENTITY
            },
            scale: if scale.is_finite() { scale } else { Vec3::ONE },
        }
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

/// Which scenes have a stand-in: every scene with a parcel within the keep ring whose content
/// has not been on screen, in a realm (never in preview). Only one within the start ring starts
/// (`start_scene_lods`); a stand-in between the two rings is kept, or waits. A stand-in goes
/// when its scene's content is first shown, when the scene is past the keep ring, or when the
/// realm's pointers no longer name it.
fn update_scene_lods(
    mut commands: Commands,
    preview: Res<PreviewMode>,
    focus: Query<&GlobalTransform, With<PrimaryUser>>,
    pointers: Res<ScenePointers>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<(&RendererSceneContext, Has<ContentWasShown>)>,
    lods: Query<(Entity, &SceneLod)>,
) {
    let Ok(focus) = focus.single() else {
        return;
    };

    // each scene within the keep ring, with its urn and its parcels there
    let mut wanted: HashMap<&str, (Option<&str>, Vec<IVec2>)> = HashMap::new();
    if !preview.is_preview && pointers.min() != IVec2::MAX {
        // the parcels within the keep ring of the player's, the ring `scene_partition` measures
        let center = vec3_to_parcel(focus.translation());
        let ring = IVec2::splat(NEAR_LOD_KEEP_RING as i32);
        let min = (center - ring).max(pointers.min());
        let max = (center + ring).min(pointers.max());
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                let parcel = IVec2::new(x, y);
                if let Some(PointerResult::Exists { hash, urn }) = pointers.get(parcel) {
                    wanted
                        .entry(hash.as_str())
                        .or_insert_with(|| (urn.as_deref(), Vec::new()))
                        .1
                        .push(parcel);
                }
            }
        }
    }

    let shown = |hash: &str| {
        live_scenes
            .scenes
            .get(hash)
            .and_then(|entity| contexts.get(*entity).ok())
            .is_some_and(|(context, was_shown)| was_shown || context.content_shown())
    };

    let mut existing: HashSet<&str> = HashSet::new();
    for (entity, lod) in &lods {
        if wanted.contains_key(lod.hash.as_str()) && !shown(&lod.hash) {
            existing.insert(lod.hash.as_str());
            continue;
        }
        // its containers go with it, and release their models (`gltf_container`'s observers)
        commands.entity(entity).despawn();
    }

    for (hash, (urn, parcels)) in wanted {
        if existing.contains(hash) || shown(hash) {
            continue;
        }
        commands.spawn((
            SceneLod {
                hash: hash.to_owned(),
                urn: urn.map(ToOwned::to_owned),
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

type LodRow<'a> = (Entity, &'a SceneLod, Option<&'a Partition>);

/// Whether a stand-in that has not started may start: its scene within the start ring, whatever
/// the facing. The ring bounds how many start; each loads its colliders part and nothing drawn.
fn may_start(partition: Option<&Partition>) -> bool {
    partition.is_some_and(|partition| partition.0.parcel_distance <= NEAR_LOD_START_RING)
}

/// Whether a stand-in's visuals may start: its models known, its scene within the start ring and
/// not wholly behind the camera.
fn may_draw(lod: &SceneLod, partition: Option<&Partition>) -> bool {
    matches!(lod.state, LodState::Ready(_))
        && matches!(lod.visuals, Visuals::Off)
        && partition.is_some_and(|partition| {
            partition.0.parcel_distance <= NEAR_LOD_START_RING && !partition.0.is_behind
        })
}

/// The visuals to start and to stop this frame, keeping at most "Max Near LODs" started. Those
/// that may start (`may_draw`) do, nearest first (by the download key: ring, then distance),
/// while there is a free place. With none free, one started behind the camera makes way for each
/// in view, the farthest first; one in view never does. Once the setting is lowered, the
/// farthest over it stop, those behind the camera first. Visuals with no partition yet are kept.
fn visual_changes(lods: &[LodRow<'_>], max: usize) -> (Vec<Entity>, Vec<Entity>) {
    // started visuals, those behind the camera first, then the farthest first
    let mut started = lods
        .iter()
        .filter(|(_, lod, _)| lod.visuals.started())
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
        .filter(|(_, lod, partition)| may_draw(lod, *partition))
        .filter_map(|(entity, _, partition)| {
            partition.map(|partition| (partition.0.load_priority(Tier::Lod), *entity))
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
    for (_, entity) in candidates {
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

/// Starts a stand-in once its scene is within the start ring, whatever the facing: loads the
/// scene's entity definition as the live scene does, then the descriptor, then gives each model
/// the descriptor names among the scene's deployed files a colliders-part container under a
/// colliders root ([`StandInColliders`]). The visuals start apart (`update_lod_visuals`).
fn start_scene_lods(
    mut commands: Commands,
    mut lods: Query<(Entity, &mut SceneLod, Option<&Partition>)>,
    definitions: Res<Assets<EntityDefinition>>,
    descriptors: Res<Assets<Descriptor>>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<(), With<RendererSceneContext>>,
    ipfas: IpfsAssetServer,
    // absent without the render storage-buffer assets (tests), as for the live scene's bounds
    mut storage_buffers: Option<ResMut<Assets<ShaderStorageBuffer>>>,
) {
    for (root, lod, partition) in lods.iter_mut() {
        let lod = lod.into_inner();
        match &lod.state {
            LodState::Pending => {
                if !may_start(partition) {
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
                let regions = scene_regions(parcels.iter().copied())
                    .into_iter()
                    .map(|region| BoundRegion::new(region.min, region.max, region.count))
                    .collect::<Vec<_>>();
                let origin = base.as_vec2() * PARCEL_SIZE;
                commands.entity(root).insert((
                    Transform::from_translation(Vec3::new(origin.x, 0.0, -origin.y)),
                    StandInBounds(SceneBounds::new(&regions, storage_buffers.as_deref_mut())),
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
                let mut models = Vec::new();
                for asset in &descriptor.assets {
                    // the descriptor names the file by its content hash
                    let Some((file, _)) = definition
                        .content
                        .values()
                        .find(|(_, hash)| **hash == asset.hash)
                    else {
                        // a model the scene does not deploy, such as one it loads from a url
                        debug!(
                            "{}: near LOD skips {}: not a file the scene deployed",
                            lod.hash, asset.hash
                        );
                        continue;
                    };
                    models.push(StandInModel {
                        file: file.clone(),
                        transform: asset.transform(),
                    });
                }
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

/// Starts and stops stand-ins' visuals by `visual_changes`. Starting gives each model a visuals
/// container under the stand-in's root; stopping despawns them, hides the root and empties its
/// resource lookup, so the scene materials go and, once no live scene holds them, the glTFs with
/// their textures and meshes. The colliders part stays.
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
) {
    let (start, stop) = visual_changes(
        &lods
            .iter()
            .map(|(entity, lod, partition, ..)| (entity, lod, partition))
            .collect::<Vec<_>>(),
        config.max_near_lods,
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
        spawn_containers(&mut commands, root, models, GltfPart::Visuals);
        lod.visuals = Visuals::Loading;
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
/// some model loaded, or nothing to show if none did (the parcels then stay the loading walls'
/// and the imposters'; the colliders part stays).
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
            commands.entity(root).insert(Visibility::Inherited);
            if tint.0 {
                apply_tint(true, lookup, &base_materials, &mut materials);
            }
        } else {
            lod.visuals = Visuals::Nothing;
            despawn_visuals(&mut commands, Some(children), &parts);
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
            urn: None,
            parcels: Vec::new(),
            state,
            visuals,
        }
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

    /// A stand-in starts its colliders part whatever the facing, within the start ring only.
    #[test]
    fn colliders_start_by_distance_alone() {
        assert!(may_start(Some(&at_ring(1, true))));
        assert!(may_start(Some(&at_ring(NEAR_LOD_START_RING, false))));
        assert!(!may_start(Some(&at_ring(NEAR_LOD_KEEP_RING, false))));
        assert!(!may_start(None));
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
        assert_eq!(
            visual_changes(&as_rows(&rows), 4),
            (vec![rows[2].0, rows[1].0], vec![])
        );
        assert_eq!(
            visual_changes(&as_rows(&rows), 3),
            (vec![rows[2].0, rows[1].0], vec![rows[4].0])
        );
        assert_eq!(
            visual_changes(&as_rows(&rows), 2),
            (vec![rows[2].0], vec![rows[4].0])
        );
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
            visual_changes(&as_rows(&rows), 3),
            (vec![rows[4].0, rows[3].0], vec![rows[2].0, rows[1].0])
        );
    }

    /// Lowering "Max Near LODs" stops the farthest over it, those behind the camera first.
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
            visual_changes(&as_rows(&rows), 1),
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

    /// A world with the player in parcel (0,0) and one scene there, live but not yet shown.
    fn world_with_live_scene() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<PreviewMode>();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::splat(-10), IVec2::splat(10));
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
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
        assert_eq!(stand_ins(&mut world), 1);

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
        use std::sync::Arc;

        use bevy::{
            asset::io::memory::{Dir, MemoryAssetReader},
            diagnostic::FrameCount,
            gltf::{Gltf, GltfMesh},
        };
        use dcl_component::proto_components::sdk::components::ColliderLayer;
        use ipfs::{IpfsIo, IpfsResource};

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
            .insert_resource(IpfsResource {
                inner: Arc::new(IpfsIo::new(
                    false,
                    Box::new(MemoryAssetReader {
                        root: Dir::default(),
                    }),
                    None,
                    Default::default(),
                    1,
                    tokio::sync::mpsc::unbounded_channel().0,
                )),
            })
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
}
