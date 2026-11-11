//! A scene's stand-in while it is in range but not live: the scene's own glTF files, placed from
//! the asset pipeline's initial-scene-state descriptor. Each model is a glTF container of the
//! stand-in's root, which the live container path (`gltf_container`) loads under the scene's
//! content path, processes a slice a frame and gives its colliders, as it does a live scene's.
//! The live scene's containers made before it goes name the same files and share the loaded
//! glTFs. Nothing of
//! the stand-in shows until every model is processed or has failed; it goes in the frame the live
//! scene's content is first on screen. While shown, its models' collider nodes are colliders, so
//! a player who reaches the scene before it is live walks on what they see.

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
        gltf_container::{GltfDefinition, GltfProcessed, SceneResourceLookup, StandInBounds},
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
                    ("Near LODs shown", "near.shown"),
                    ("Near LODs loading", "near.loading"),
                    (
                        "Near LODs waiting (behind, out of range or over the limit)",
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
                trim_scene_lods,
                start_scene_lods,
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
    for (_, lookup) in lods
        .iter()
        .filter(|(lod, _)| matches!(lod.state, LodState::Shown))
    {
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
    let (mut waiting, mut loading, mut shown, mut none) = (0, 0, 0, 0);
    for lod in &lods {
        match lod.state {
            LodState::Pending => waiting += 1,
            LodState::Definition(_) | LodState::Descriptor { .. } | LodState::Loading => {
                loading += 1
            }
            LodState::Shown => shown += 1,
            LodState::Nothing => none += 1,
        }
    }
    input.reply_ok(
        json!({
            "tint": tint.0,
            "near": {"waiting": waiting, "loading": loading, "shown": shown, "none": none,
                     "max": config.max_near_lods},
        })
        .to_string(),
    );
}

/// The stand-in of one scene: the root its models' containers hang from, at the scene's base
/// parcel once its entity definition is loaded. While it exists the scene's hash ranks its
/// downloads in the LOD tier (the hooks), below every live scene's; once the scene spawns, its
/// own registration takes the hash to the scene tier.
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
}

pub enum LodState {
    /// not started: no partition yet, the scene past the start ring or wholly behind the
    /// camera, or no free place
    Pending,
    /// the scene's entity definition is loading, as the live scene loads it; the root holds the
    /// handle too (`SceneEntityDefinitionHandle`), which the containers' loads read
    Definition(Handle<EntityDefinition>),
    /// the descriptor is loading
    Descriptor {
        definition: Handle<EntityDefinition>,
        descriptor: Handle<Descriptor>,
    },
    /// the containers are loading and processing; the root stays hidden until each is done
    Loading,
    Shown,
    /// nothing to show: no descriptor, a definition or descriptor that could not load, no
    /// model the scene deployed, or no model that loaded. Not asked again while the scene
    /// stays within the keep ring
    Nothing,
}

impl LodState {
    fn started(&self) -> bool {
        matches!(
            self,
            LodState::Definition(_)
                | LodState::Descriptor { .. }
                | LodState::Loading
                | LodState::Shown
        )
    }
}

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
            },
            Transform::default(),
            Visibility::Hidden,
            // what the live container and collider systems keep on a scene root
            SceneResourceLookup::default(),
            ComponentTracker::default(),
            SceneColliderData::default(),
        ));
    }
}

/// Whether a stand-in counts toward "Max Near LODs": started, and not wholly behind the camera.
/// One behind is kept but takes no place from one ahead.
fn counts(lod: &SceneLod, partition: Option<&Partition>) -> bool {
    lod.state.started() && !partition.is_some_and(|partition| partition.0.is_behind)
}

/// The most stand-ins started at once, counted or behind the camera: twice "Max Near LODs", so
/// the setting still bounds memory when the player turns and new stand-ins start in view.
fn hard_total(max: usize) -> usize {
    max.saturating_mul(2)
}

type LodRow<'a> = (Entity, &'a SceneLod, Option<&'a Partition>);

/// Keeps the stand-ins within "Max Near LODs" and its hard total (`trimmed_lods`). A removed one
/// comes back as waiting (`update_scene_lods`) and starts again when a place frees.
fn trim_scene_lods(
    mut commands: Commands,
    lods: Query<(Entity, &SceneLod, Option<&Partition>)>,
    config: Res<AppConfig>,
    mut last_max: Local<Option<usize>>,
) {
    let max = config.max_near_lods;
    let lowered = last_max.is_some_and(|last| max < last);
    *last_max = Some(max);
    for entity in trimmed_lods(&lods.iter().collect::<Vec<_>>(), max, lowered) {
        commands.entity(entity).despawn();
    }
}

/// The stand-ins to remove, farthest first. Once "Max Near LODs" is lowered, the counted ones
/// over it. Then those past the hard total (`hard_total`): the ones behind the camera first, so
/// turning the camera alone never removes one; only stand-ins started in view after the turn push
/// the ones behind out. A stand-in with no partition yet is kept.
fn trimmed_lods(lods: &[LodRow<'_>], max: usize, lowered: bool) -> Vec<Entity> {
    // the started stand-ins in view (`behind` false) or behind the camera, farthest first
    let started = |behind: bool| {
        let mut rows = lods
            .iter()
            .filter(|(_, lod, partition)| lod.state.started() && counts(lod, *partition) != behind)
            .map(|(entity, _, partition)| {
                let key = partition.map(|partition| partition.0.load_priority(Tier::Lod));
                (key, *entity)
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|(key, _)| std::cmp::Reverse(*key));
        rows
    };
    let mut counted = started(false);
    let behind = started(true);
    let mut removed = Vec::new();
    if lowered {
        let excess = counted.len().saturating_sub(max);
        removed.extend(counted.drain(..excess).map(|(_, entity)| entity));
    }
    let excess = (counted.len() + behind.len()).saturating_sub(hard_total(max));
    removed.extend(
        behind
            .into_iter()
            .chain(counted)
            .filter(|(key, _)| key.is_some())
            .take(excess)
            .map(|(_, entity)| entity),
    );
    removed
}

/// Whether a stand-in that has not started may start: its scene within the start ring and not
/// wholly behind the camera.
fn may_start(partition: Option<&Partition>) -> bool {
    partition.is_some_and(|partition| {
        partition.0.parcel_distance <= NEAR_LOD_START_RING && !partition.0.is_behind
    })
}

/// The stand-ins allowed to start this frame: those that may, nearest first (by the download
/// key: ring, then in front of the camera, then distance), up to the "Max Near LODs" setting
/// less the stand-ins it counts. The rest wait for a free place. Started ones behind the camera
/// do not block one in view; `trim_scene_lods` then keeps the total within the hard total.
fn admitted_lods(lods: &[LodRow<'_>], max: usize) -> HashSet<Entity> {
    let counted = lods
        .iter()
        .filter(|(_, lod, partition)| counts(lod, *partition))
        .count();
    let mut candidates = lods
        .iter()
        .filter(|(_, lod, partition)| {
            matches!(lod.state, LodState::Pending) && may_start(*partition)
        })
        .filter_map(|(entity, _, partition)| {
            partition.map(|partition| (partition.0.load_priority(Tier::Lod), *entity))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(key, _)| *key);
    candidates
        .into_iter()
        .take(max.saturating_sub(counted))
        .map(|(_, entity)| entity)
        .collect()
}

/// Starts a stand-in once it has a partition within the start ring, the scene is not wholly
/// behind the camera and it fits the "Max Near LODs" count, nearest first: loads the scene's
/// entity definition as the live scene does, then the descriptor, then gives each model the
/// descriptor names among the scene's deployed files a glTF container under the root.
#[allow(clippy::too_many_arguments)]
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
    let admitted = admitted_lods(&lods.iter().collect::<Vec<_>>(), config.max_near_lods);
    for (root, lod, _) in lods.iter_mut() {
        let lod = lod.into_inner();
        match &lod.state {
            LodState::Pending => {
                if !admitted.contains(&root) {
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
                let (Some(definition), Some(descriptor)) = (
                    definitions.get(definition.id()),
                    descriptors.get(descriptor.id()),
                ) else {
                    lod.state = LodState::Nothing;
                    continue;
                };
                let mut spawned = 0u16;
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
                    let Some(id) = FIRST_CONTAINER_ID.checked_add(spawned) else {
                        break;
                    };
                    spawned += 1;
                    commands.spawn((
                        SceneEntity {
                            root,
                            scene_id: SceneId(root),
                            id: SceneEntityId::new(id, 0),
                        },
                        GltfDefinition::from(PbGltfContainer {
                            src: file.clone(),
                            ..Default::default()
                        }),
                        asset.transform(),
                        Visibility::default(),
                        ChildOf(root),
                    ));
                }
                lod.state = if spawned == 0 {
                    LodState::Nothing
                } else {
                    LodState::Loading
                };
            }
            _ => (),
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

/// Shows a stand-in in one frame once each of its containers is processed: some model loaded,
/// or nothing to show if none did (the parcels then stay the loading walls' and the
/// imposters').
pub(crate) fn show_scene_lods(
    mut commands: Commands,
    mut lods: Query<(
        Entity,
        &mut SceneLod,
        Option<&Children>,
        &SceneResourceLookup,
    )>,
    containers: Query<Option<&GltfProcessed>, With<GltfDefinition>>,
    tint: Res<LodTint>,
    base_materials: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<SceneMaterial>>,
) {
    for (root, mut lod, children, lookup) in lods.iter_mut() {
        if !matches!(lod.state, LodState::Loading) {
            continue;
        }
        let Some(children) = children else {
            continue;
        };
        let mut processed = true;
        let mut any_model = false;
        for done in containers.iter_many(children.collection()) {
            match done {
                Some(done) => any_model |= done.instance_id.is_some(),
                None => processed = false,
            }
        }
        if !processed {
            continue;
        }
        if any_model {
            lod.state = LodState::Shown;
            commands.entity(root).insert(Visibility::Inherited);
            if tint.0 {
                apply_tint(true, lookup, &base_materials, &mut materials);
            }
        } else {
            lod.state = LodState::Nothing;
            commands.entity(root).despawn_related::<Children>();
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

    fn lod(state: LodState) -> SceneLod {
        SceneLod {
            hash: "scene".to_owned(),
            urn: None,
            parcels: Vec::new(),
            state,
        }
    }

    /// Stand-ins at the given rings, `true` behind the camera, `started` or waiting; their
    /// entities in the same order.
    fn rows(
        world: &mut World,
        stand_ins: &[(u16, bool, bool)],
    ) -> Vec<(Entity, SceneLod, Partition)> {
        stand_ins
            .iter()
            .map(|&(ring, behind, started)| {
                let state = if started {
                    LodState::Loading
                } else {
                    LodState::Pending
                };
                (world.spawn_empty().id(), lod(state), at_ring(ring, behind))
            })
            .collect()
    }

    fn as_rows(rows: &[(Entity, SceneLod, Partition)]) -> Vec<LodRow<'_>> {
        rows.iter()
            .map(|(entity, lod, partition)| (*entity, lod, Some(partition)))
            .collect()
    }

    /// Only stand-ins in view take a place: one started behind the camera leaves the nearest
    /// waiting one in view free to start, and one waiting behind the camera never starts.
    #[test]
    fn admission_counts_stand_ins_in_view() {
        let mut world = World::new();
        let rows = rows(
            &mut world,
            &[
                (1, false, true),
                (1, true, true),
                (3, false, false),
                (2, false, false),
                (1, true, false),
            ],
        );
        let admitted = admitted_lods(&as_rows(&rows), 2);
        assert_eq!(admitted, HashSet::from([rows[3].0]));
        assert!(admitted_lods(&as_rows(&rows), 1).is_empty());
    }

    /// Past twice "Max Near LODs" the farthest stand-ins behind the camera go, never one in
    /// view; within it nothing goes until the setting is lowered.
    #[test]
    fn stand_ins_behind_the_camera_stay_within_the_hard_total() {
        let mut world = World::new();
        let rows = rows(
            &mut world,
            &[
                (1, false, true),
                (2, false, true),
                (1, true, true),
                (3, true, true),
                (2, true, true),
                (3, false, false),
            ],
        );
        assert_eq!(trimmed_lods(&as_rows(&rows), 2, false), vec![rows[3].0]);
        assert!(trimmed_lods(&as_rows(&rows), 3, false).is_empty());
    }

    /// Lowering "Max Near LODs" removes the farthest counted stand-ins over it, then the
    /// farthest behind the camera over the new hard total.
    #[test]
    fn lowering_the_count_trims_in_view_then_behind() {
        let mut world = World::new();
        let rows = rows(
            &mut world,
            &[
                (1, false, true),
                (3, false, true),
                (2, false, true),
                (1, true, true),
                (3, true, true),
            ],
        );
        assert_eq!(
            trimmed_lods(&as_rows(&rows), 1, true),
            vec![rows[1].0, rows[2].0, rows[4].0]
        );
    }

    /// The setting's change is read by the system: the hard total holds every frame, and
    /// lowering the count trims the stand-ins in view once.
    #[test]
    fn trim_follows_the_setting() {
        let mut world = World::new();
        world.insert_resource(AppConfig {
            max_near_lods: 2,
            ..Default::default()
        });
        let stand_ins =
            [(1, false), (2, false), (1, true), (2, true), (3, true)].map(|(ring, behind)| {
                world
                    .spawn((lod(LodState::Shown), at_ring(ring, behind)))
                    .id()
            });
        let mut trim = IntoSystem::into_system(trim_scene_lods);
        trim.initialize(&mut world);

        trim.run((), &mut world);
        trim.apply_deferred(&mut world);
        assert!(world.get_entity(stand_ins[4]).is_err());
        assert_eq!(world.query::<&SceneLod>().iter(&world).count(), 4);

        world.resource_mut::<AppConfig>().max_near_lods = 1;
        trim.run((), &mut world);
        trim.apply_deferred(&mut world);
        // the farther one in view goes for the count, the farther one behind for the hard total
        assert!(world.get_entity(stand_ins[1]).is_err());
        assert!(world.get_entity(stand_ins[3]).is_err());
        assert_eq!(world.query::<&SceneLod>().iter(&world).count(), 2);
    }

    fn container(world: &mut World, root: Entity, processed: Option<GltfProcessed>) -> Entity {
        let mut container = world.spawn((
            GltfDefinition::from(PbGltfContainer {
                src: "model.glb".to_owned(),
                ..Default::default()
            }),
            ChildOf(root),
        ));
        if let Some(processed) = processed {
            container.insert(processed);
        }
        container.id()
    }

    /// A stand-in whose every model failed shows nothing: it waits for the last container, then
    /// becomes `Nothing`, drops its containers and stays hidden.
    #[test]
    fn a_stand_in_whose_models_all_fail_shows_nothing() {
        let mut world = World::new();
        world.init_resource::<LodTint>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<SceneMaterial>>();
        let root = world
            .spawn((
                lod(LodState::Loading),
                Visibility::Hidden,
                SceneResourceLookup::default(),
            ))
            .id();
        container(&mut world, root, Some(GltfProcessed::default()));
        let last = container(&mut world, root, None);

        world.run_system_once(show_scene_lods).unwrap();
        assert!(matches!(
            world.get::<SceneLod>(root).unwrap().state,
            LodState::Loading
        ));

        world.entity_mut(last).insert(GltfProcessed::default());
        world.run_system_once(show_scene_lods).unwrap();
        assert!(matches!(
            world.get::<SceneLod>(root).unwrap().state,
            LodState::Nothing
        ));
        assert!(world.get::<Children>(root).is_none());
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

    /// A stand-in's colliders join the collider set only once it shows, so a hidden stand-in
    /// neither blocks nor carries the player: movement, collision resolution and foot IK all cast
    /// against the whole set.
    #[test]
    fn a_stand_in_collides_only_once_shown() {
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

        let root = app
            .world_mut()
            .spawn((lod(LodState::Loading), SceneColliderData::default()))
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

        // still loading: the processed model's collider waits
        app.update();
        app.update();
        assert!(!blocks(&mut app));

        // shown: the collider registers in the same update
        app.world_mut().get_mut::<SceneLod>(root).unwrap().state = LodState::Shown;
        app.update();
        assert!(blocks(&mut app));
    }
}
