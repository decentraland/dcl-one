//! A scene's stand-in while it is in range but not live: the scene's own glTF files, placed from
//! the asset pipeline's initial-scene-state descriptor. They load under
//! the scene's content paths with the scene's loader settings, so the live scene's containers
//! find them in the asset cache and take them over. Nothing of it shows until every file has
//! loaded or failed; it goes in the frame the live scene's content is on screen. While shown, its
//! models' collider nodes are physics colliders, so a player who reaches the scene before it is
//! live walks on what they see.

use std::{f32::consts::PI, sync::Arc, time::Duration};

use bevy::{
    asset::{LoadState, RenderAssetTransferPriority},
    ecs::{component::HookContext, world::DeferredWorld},
    gltf::{Gltf, GltfLoaderSettings},
    pbr::NotShadowCaster,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::{
        mesh::skinning::SkinnedMesh, render_asset::RenderAssetUsages, storage::ShaderStorageBuffer,
    },
    scene::InstanceId,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::Service,
    bounds_calc::scene_regions,
    partition::{LoadPriority, Tier},
    sets::{PostUpdateSets, SceneSets},
    structs::{AppConfig, PreviewMode},
    util::{SceneSpawnerPlus, TaskCompat, TaskExt},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::sdk::components::ColliderLayer,
    transform_and_parent::{DclQuat, DclTranslation},
    SceneEntityId,
};
use ipfs::{
    ipfs_path::IpfsPath, ContentMap, EntityDefinition, IpfsAssetServer, IpfsIo, IpfsModifier,
    IpfsResource,
};
use rapier3d_f64::prelude::{ColliderBuilder, Group, InteractionGroups, InteractionTestMode};
use scene_material::{BoundRegion, SceneBound, SceneBounds, SceneMaterial};
use serde::Deserialize;
use serde_json::json;

use crate::{
    initialize_scene::{
        process_scene_lifecycle, LiveScenes, PointerResult, ScenePointers, PARCEL_SIZE,
    },
    renderer_context::RendererSceneContext,
    scene_partition::Partition,
    update_world::{
        gltf_container::{mesh_to_parry_shape, scene_gltf_loader_settings},
        mesh_collider::{ColliderId, SceneColliderData, SharedShape},
    },
    vec3_to_parcel, PrimaryUser,
};

/// A scene that is not live starts a near LOD when its nearest parcel is within this Chebyshev
/// parcel ring of the player's parcel (`Partition::parcel_distance`): about 48 m.
pub const NEAR_LOD_START_RING: u16 = 3;

/// A near LOD is kept until its scene's nearest parcel is past this ring (about 64 m), so a
/// player walking along the edge does not start and drop it every few steps.
pub const NEAR_LOD_KEEP_RING: u16 = 4;

pub struct SceneLodPlugin;

impl Plugin for SceneLodPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LodTint>()
            .add_console_command::<LodTintCommand, _>(set_lod_tint)
            .add_console_command::<LodStateCommand, _>(lod_state)
            .add_debug_section(DebugSection {
                title: "Scene stand-ins",
                query: "/scene_lod_state",
                fields: &[
                    ("Near LODs shown", "near.shown"),
                    ("Near LODs loading", "near.loading"),
                    ("Near LODs waiting (behind or out of range)", "near.waiting"),
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
                update_scene_lods,
                start_scene_lods,
                build_scene_lods,
                tint_scene_lods.run_if(resource_changed::<LodTint>),
            )
                .chain()
                .after(process_scene_lifecycle)
                .in_set(SceneSets::Init),
        );
        app.add_systems(
            PostUpdate,
            place_lod_colliders.in_set(PostUpdateSets::ColliderUpdate),
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

/// Applies or removes the debug tint on every stand-in already shown; ones built later are
/// dressed tinted from the start.
fn tint_scene_lods(
    tint: Res<LodTint>,
    lods: Query<&SceneLod>,
    mut materials: ResMut<Assets<SceneMaterial>>,
) {
    for lod in &lods {
        for (handle, original) in &lod.dressed {
            if let Some(material) = materials.get_mut(handle) {
                material.base.base_color = if tint.0 { tinted(*original) } else { *original };
            }
        }
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
    lods: Query<&SceneLod>,
) {
    if input.take().is_none() {
        return;
    }
    let (mut waiting, mut loading, mut shown, mut none) = (0, 0, 0, 0);
    for lod in &lods {
        match lod.state {
            LodState::Pending => waiting += 1,
            LodState::Fetching(_) | LodState::Loading(_) => loading += 1,
            LodState::Shown(_) => shown += 1,
            LodState::Nothing => none += 1,
        }
    }
    input.reply_ok(
        json!({
            "tint": tint.0,
            "near": {"waiting": waiting, "loading": loading, "shown": shown, "none": none},
        })
        .to_string(),
    );
}

/// What the realm's active-entities response said about each scene, by entity hash: what a
/// stand-in needs before the scene itself is fetched. The lifecycle keeps only the pointers.
#[derive(Resource, Default)]
pub struct SceneDefinitions(pub HashMap<String, SceneDefinition>);

pub struct SceneDefinition {
    pub content: ContentMap,
    /// the entity's metadata (`scene.json`) as the content collection records it
    pub metadata: Option<String>,
    /// the scene's urn when the realm gave one; a city loader's carries the content base url
    pub urn: Option<String>,
    pub base: IVec2,
    pub parcels: Vec<IVec2>,
}

/// The stand-in of one scene: the root its models hang from, at the scene's base parcel. While
/// it exists the scene's hash ranks its downloads in the LOD tier (the hooks), below every live
/// scene's; once the scene spawns, its own registration takes the hash to the scene tier.
#[derive(Component)]
#[component(on_insert = register_lod_hash, on_replace = release_lod_hash)]
pub struct SceneLod {
    pub hash: String,
    pub parcels: Vec<IVec2>,
    pub state: LodState,
    /// the materials made for the stand-in's models, each with its untinted base colour, so the
    /// debug tint can be turned on and off
    pub dressed: Vec<(Handle<SceneMaterial>, Color)>,
}

pub enum LodState {
    /// not started: no partition yet, the scene past the start ring, or wholly behind the
    /// camera
    Pending,
    Fetching(Task<Result<Option<Descriptor>, anyhow::Error>>),
    Loading(Vec<LodModel>),
    Shown(Vec<InstanceId>),
    /// no descriptor, or it could not be fetched: nothing to show, and no second ask
    Nothing,
}

pub struct LodModel {
    handle: Handle<Gltf>,
    transform: Transform,
    /// the anchor the glTF scene was spawned under, and the instance
    instance: Option<(Entity, InstanceId)>,
    failed: bool,
}

/// `{sceneId}_InitialSceneState.json`: the scene's models by content hash, each at its transform
/// relative to the base parcel, in SDK coordinates.
#[derive(Deserialize)]
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

/// Which scenes have a stand-in: every scene with a parcel within the keep ring whose content
/// is not on screen, in a realm (never in preview). Only one within the start ring starts
/// (`start_scene_lods`); a stand-in between the two rings is kept, or waits. A stand-in goes
/// when its scene's content is shown, when the scene is past the keep ring, or when the realm's
/// pointers no longer name it.
#[allow(clippy::too_many_arguments)]
fn update_scene_lods(
    mut commands: Commands,
    preview: Res<PreviewMode>,
    focus: Query<&GlobalTransform, With<PrimaryUser>>,
    pointers: Res<ScenePointers>,
    definitions: Res<SceneDefinitions>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<&RendererSceneContext>,
    lods: Query<(Entity, &SceneLod)>,
    mut scene_spawner: ResMut<SceneSpawner>,
) {
    let Ok(focus) = focus.single() else {
        return;
    };

    let mut wanted: HashSet<&str> = HashSet::new();
    if !preview.is_preview && pointers.min() != IVec2::MAX {
        // the parcels within the keep ring of the player's, the ring `scene_partition` measures
        let center = vec3_to_parcel(focus.translation());
        let ring = IVec2::splat(NEAR_LOD_KEEP_RING as i32);
        let min = (center - ring).max(pointers.min());
        let max = (center + ring).min(pointers.max());
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                if let Some(PointerResult::Exists { hash, .. }) = pointers.get(IVec2::new(x, y)) {
                    wanted.insert(hash.as_str());
                }
            }
        }
    }

    let shown = |hash: &str| {
        live_scenes
            .scenes
            .get(hash)
            .and_then(|entity| contexts.get(*entity).ok())
            .is_some_and(RendererSceneContext::content_shown)
    };

    let mut existing: HashSet<&str> = HashSet::new();
    for (entity, lod) in &lods {
        if wanted.contains(lod.hash.as_str()) && !shown(&lod.hash) {
            existing.insert(lod.hash.as_str());
            continue;
        }
        // the root takes the models with it; the spawner only needs to forget the instances
        match &lod.state {
            LodState::Loading(models) => {
                for (_, instance) in models.iter().filter_map(|model| model.instance) {
                    scene_spawner.unregister_instance(instance);
                }
            }
            LodState::Shown(instances) => {
                for instance in instances {
                    scene_spawner.unregister_instance(*instance);
                }
            }
            _ => (),
        }
        commands.entity(entity).despawn();
    }

    for hash in wanted {
        if existing.contains(hash) || shown(hash) {
            continue;
        }
        let Some(definition) = definitions.0.get(hash) else {
            continue;
        };
        let origin = definition.base.as_vec2() * PARCEL_SIZE;
        commands.spawn((
            SceneLod {
                hash: hash.to_owned(),
                parcels: definition.parcels.clone(),
                state: LodState::Pending,
                dressed: Vec::new(),
            },
            Transform::from_translation(Vec3::new(origin.x, 0.0, -origin.y)),
            Visibility::Hidden,
        ));
    }
}

/// Starts a stand-in once it has a partition within the start ring and the scene is not wholly
/// behind the camera, and turns the fetched descriptor into the scene's glTF loads, under the
/// scene's content paths and loader settings.
fn start_scene_lods(
    mut lods: Query<(&mut SceneLod, Option<&Partition>)>,
    definitions: Res<SceneDefinitions>,
    ipfas: IpfsAssetServer,
) {
    for (lod, partition) in lods.iter_mut() {
        let lod = lod.into_inner();
        match &mut lod.state {
            LodState::Pending => {
                let starts = partition.is_some_and(|partition| {
                    partition.0.parcel_distance <= NEAR_LOD_START_RING && !partition.0.is_behind
                });
                if !starts {
                    continue;
                }
                let Some(definition) = definitions.0.get(&lod.hash) else {
                    lod.state = LodState::Nothing;
                    continue;
                };
                // the scene's files resolve through its content collection, which the live
                // scene registers only when it spawns
                let modifier = definition
                    .urn
                    .as_deref()
                    .and_then(|urn| IpfsPath::new_from_urn::<EntityDefinition>(urn).ok())
                    .and_then(|path| {
                        path.base_url().map(|base_url| IpfsModifier {
                            base_url: Some(base_url.to_owned()),
                        })
                    });
                ipfas.ipfs().add_collection(
                    lod.hash.clone(),
                    definition.content.clone(),
                    modifier,
                    definition.metadata.clone(),
                );
                let url = format!(
                    "{}/LOD/lods-unity/manifests/{}_InitialSceneState.json",
                    common::base_domain::service(Service::AbgenCdn),
                    lod.hash.to_lowercase()
                );
                let task = IoTaskPool::get().spawn_compat(fetch_descriptor(
                    ipfas.ipfs().clone(),
                    url,
                    lod.hash.clone(),
                ));
                lod.state = LodState::Fetching(task);
            }
            LodState::Fetching(task) => {
                let Some(result) = task.complete() else {
                    continue;
                };
                let Some(definition) = definitions.0.get(&lod.hash) else {
                    lod.state = LodState::Nothing;
                    continue;
                };
                lod.state = match result {
                    Ok(Some(descriptor)) => {
                        let models = descriptor
                            .assets
                            .iter()
                            .filter_map(|asset| {
                                // the descriptor names the file by its content hash
                                let (file, _) = definition
                                    .content
                                    .values()
                                    .find(|(_, hash)| **hash == asset.hash)?;
                                let handle = ipfas
                                    .load_content_file_with_settings::<Gltf, GltfLoaderSettings>(
                                        file,
                                        &lod.hash,
                                        scene_gltf_loader_settings(
                                            RenderAssetTransferPriority::Priority(0),
                                            false,
                                        ),
                                    )
                                    .ok()?;
                                Some(LodModel {
                                    handle,
                                    transform: asset.transform(),
                                    instance: None,
                                    failed: false,
                                })
                            })
                            .collect::<Vec<_>>();
                        if models.is_empty() {
                            LodState::Nothing
                        } else {
                            LodState::Loading(models)
                        }
                    }
                    Ok(None) => LodState::Nothing,
                    Err(e) => {
                        warn!("{}: no near LOD: {e}", lod.hash);
                        LodState::Nothing
                    }
                };
            }
            _ => (),
        }
    }
}

/// The descriptor, `None` where the pipeline has none for the scene. The fetch takes a download
/// slot under the scene's hash, so it ranks with the stand-in's other files.
async fn fetch_descriptor(
    ipfs: Arc<IpfsIo>,
    url: String,
    owner: String,
) -> Result<Option<Descriptor>, anyhow::Error> {
    let client = ipfs.client();
    let request = client.get(&url).build()?;
    match ipfs
        .fetch_request(
            request,
            client,
            Duration::from_secs(30),
            Duration::from_secs(10),
            Some(owner),
        )
        .await
    {
        Ok(fetched) => Ok(Some(serde_json::from_slice(&fetched.body)?)),
        Err(platform::FetchError::Status(status, _))
            if status == reqwest::StatusCode::NOT_FOUND
                || status == reqwest::StatusCode::FORBIDDEN =>
        {
            Ok(None)
        }
        Err(platform::FetchError::Status(status, _)) => anyhow::bail!("{url}: {status}"),
        Err(platform::FetchError::Send(e)) => Err(e),
        Err(platform::FetchError::Headers) => anyhow::bail!("{url}: timed out awaiting headers"),
        Err(platform::FetchError::Stalled) => anyhow::bail!("{url}: stalled"),
        Err(platform::FetchError::Body(e)) => Err(anyhow::anyhow!(e)),
    }
}

/// Places each model as its glTF arrives and, once every model is placed or has failed, dresses
/// them as the live scene would (meshes uploaded, root nodes turned, collider nodes hidden,
/// lights off, scene-bound materials), turns the collider nodes into physics colliders on the
/// stand-in's root and shows the stand-in in one frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_scene_lods(
    mut commands: Commands,
    mut lods: Query<(Entity, &mut SceneLod)>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut scene_spawner: SceneSpawnerPlus,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut scene_materials: ResMut<Assets<SceneMaterial>>,
    // absent without the render storage-buffer assets (tests), as for the live scene's bounds
    mut storage_buffers: Option<ResMut<Assets<ShaderStorageBuffer>>>,
    config: Res<AppConfig>,
    tint: Res<LodTint>,
    nodes: Query<(
        &ChildOf,
        &Transform,
        Option<&Name>,
        Option<&Mesh3d>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Has<SkinnedMesh>,
    )>,
) {
    for (root, lod) in lods.iter_mut() {
        let lod = lod.into_inner();
        let LodState::Loading(models) = &mut lod.state else {
            continue;
        };

        let mut all_placed = true;
        for model in models.iter_mut().filter(|model| !model.failed) {
            match model.instance {
                Some((_, instance)) => {
                    all_placed &= scene_spawner.instance_is_really_ready(instance);
                }
                None => match asset_server.load_state(&model.handle) {
                    LoadState::Loaded => {
                        let scene = gltfs
                            .get(&model.handle)
                            .and_then(|gltf| gltf.default_scene.clone());
                        match scene {
                            Some(scene) => {
                                let anchor = commands
                                    .spawn((model.transform, Visibility::default(), ChildOf(root)))
                                    .id();
                                model.instance =
                                    Some((anchor, scene_spawner.spawn_as_child(scene, anchor)));
                                all_placed = false;
                            }
                            None => model.failed = true,
                        }
                    }
                    LoadState::Failed(_) => model.failed = true,
                    _ => all_placed = false,
                },
            }
        }
        if !all_placed {
            continue;
        }

        let regions = scene_regions(lod.parcels.iter().copied())
            .into_iter()
            .map(|region| BoundRegion::new(region.min, region.max, region.count))
            .collect::<Vec<_>>();
        let bounds = SceneBounds::new(&regions, storage_buffers.as_deref_mut());
        let mut dressed: HashMap<AssetId<StandardMaterial>, Handle<SceneMaterial>> = HashMap::new();
        let mut instances = Vec::new();
        let mut colliders = SceneColliderData::default();
        let mut collider_count = 0u32;
        let mut shapes: HashMap<AssetId<Mesh>, Option<SharedShape>> = HashMap::new();

        for model in models.iter() {
            let Some((anchor, instance)) = model.instance else {
                continue;
            };
            instances.push(instance);
            for entity in scene_spawner.iter_instance_entities(instance) {
                let Ok((child_of, transform, _, mesh, material, skinned)) = nodes.get(entity)
                else {
                    continue;
                };
                let mut node = commands.entity(entity);
                // a static stand-in: no lights
                node.remove::<(PointLight, SpotLight, DirectionalLight)>();
                // children of the glTF root turn, as the live scene's containers turn them
                if child_of.parent() == anchor {
                    let mut turned = *transform;
                    turned.rotate_around(Vec3::ZERO, Quat::from_rotation_y(PI));
                    node.insert(turned);
                }
                // collider nodes are not drawn (ADR-215); a collider node's mesh is a physics
                // collider, as the live scene's container makes it by default (skinned ones
                // default to none)
                if let Some(kind) = hidden_node(entity, anchor, &nodes) {
                    node.remove::<Mesh3d>();
                    if let (HiddenNode::Collider, Some(mesh), false) = (kind, mesh, skinned) {
                        let shape = shapes
                            .entry(mesh.id())
                            .or_insert_with(|| meshes.get(mesh.id()).and_then(mesh_to_parry_shape))
                            .clone();
                        if let Some(shape) = shape {
                            let physics =
                                Group::from_bits_truncate(ColliderLayer::ClPhysics as u32);
                            let collider = ColliderBuilder::new(shape)
                                .collision_groups(InteractionGroups::new(
                                    physics,
                                    physics,
                                    InteractionTestMode::And,
                                ))
                                .build();
                            let id = ColliderId::new(SceneEntityId::ROOT, None, collider_count);
                            collider_count += 1;
                            colliders.set_collider(&id, collider, Some(entity));
                            node.insert(LodCollider { root, id });
                        }
                    }
                    continue;
                }
                if let Some(mesh) = mesh {
                    upload_mesh(&mut meshes, mesh.id(), skinned);
                }
                if let Some(material) = material {
                    let Some(base) = materials.get(material.id()) else {
                        continue;
                    };
                    let handle = match dressed.get(&material.id()) {
                        Some(handle) => handle.clone(),
                        None => {
                            let mut dressed_base = base.clone();
                            if tint.0 {
                                dressed_base.base_color = tinted(base.base_color);
                            }
                            let handle = scene_materials.add(SceneMaterial {
                                base: dressed_base,
                                extension: SceneBound::new(&bounds, config.graphics.oob),
                            });
                            dressed.insert(material.id(), handle.clone());
                            lod.dressed.push((handle.clone(), base.base_color));
                            handle
                        }
                    };
                    node.remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(handle));
                    if base.unlit {
                        node.insert(NotShadowCaster);
                    }
                }
            }
        }

        lod.state = LodState::Shown(instances);
        commands.entity(root).insert(Visibility::Inherited);
        if collider_count > 0 {
            commands.entity(root).insert(colliders);
        }
    }
}

/// One physics collider of a shown near LOD: the root whose `SceneColliderData` holds it, and its
/// id there. Player movement casts against every `SceneColliderData`, so the stand-in's
/// colliders block the player like a live scene's; pointer events and triggers query live
/// scenes only.
#[derive(Component)]
pub struct LodCollider {
    root: Entity,
    id: ColliderId,
}

/// Moves each near-LOD collider to its node, as `update_collider_transforms` does for a live
/// scene's. The stand-in is static, so this runs once per collider after it is made.
fn place_lod_colliders(
    colliders: Query<
        (&LodCollider, &GlobalTransform),
        Or<(Added<LodCollider>, Changed<GlobalTransform>)>,
    >,
    mut roots: Query<&mut SceneColliderData, With<SceneLod>>,
) {
    for (collider, transform) in &colliders {
        if let Ok(mut data) = roots.get_mut(collider.root) {
            data.update_collider_transform(&collider.id, transform);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HiddenNode {
    Collider,
    Trigger,
}

/// A `*_collider` or `*_trigger` node, or one under such a node, up to the model's anchor.
fn hidden_node(
    entity: Entity,
    anchor: Entity,
    nodes: &Query<(
        &ChildOf,
        &Transform,
        Option<&Name>,
        Option<&Mesh3d>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Has<SkinnedMesh>,
    )>,
) -> Option<HiddenNode> {
    let mut current = entity;
    while let Ok((child_of, _, name, ..)) = nodes.get(current) {
        if let Some(name) = name.map(|name| name.to_lowercase()) {
            if name.ends_with("_collider") {
                return Some(HiddenNode::Collider);
            }
            if name.ends_with("_trigger") {
                return Some(HiddenNode::Trigger);
            }
        }
        if child_of.parent() == anchor {
            break;
        }
        current = child_of.parent();
    }
    None
}

/// A scene's glTF meshes load cpu-side only, for the live scene to fix up and upload; the
/// stand-in does the same, so the live scene finds them uploaded.
fn upload_mesh(meshes: &mut Assets<Mesh>, id: AssetId<Mesh>, skinned: bool) {
    let Some(mesh) = meshes.get_mut(id) else {
        return;
    };
    mesh.normalize_joint_weights();
    mesh.asset_usage |= RenderAssetUsages::RENDER_WORLD;
    if !skinned {
        // bevy rejects joints and weights on a mesh with no skin
        mesh.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
        mesh.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
    }
}
