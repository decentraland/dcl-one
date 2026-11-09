//! A scene's stand-ins while it is in range but not live. Near: the scene's own glTF files,
//! placed from the asset pipeline's initial-scene-state descriptor (unity-explorer's LOD 0).
//! They load under the scene's content paths with the scene's loader settings, so the live
//! scene's containers find them in the asset cache and take them over. Far: the pipeline's one
//! mesh for the scene (LOD 1). Nothing of a stand-in shows until every file has loaded or
//! failed; the far one goes when the near one shows, and any goes in the frame the live scene's
//! content is on screen.

use std::{f32::consts::PI, path::PathBuf, sync::Arc, time::Duration};

use bevy::{
    asset::{LoadState, RenderAssetTransferPriority},
    ecs::{component::HookContext, world::DeferredWorld},
    gltf::{Gltf, GltfLoaderSettings},
    pbr::NotShadowCaster,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::{
        mesh::skinning::SkinnedMesh, render_asset::RenderAssetUsages,
        storage::ShaderStorageBuffer,
    },
    scene::InstanceId,
    tasks::{IoTaskPool, Task},
};
use common::{
    bounds_calc::scene_regions,
    partition::{LoadPriority, Tier},
    sets::SceneSets,
    structs::{AppConfig, PreviewMode, SceneLoadDistance},
    util::{SceneSpawnerPlus, TaskCompat, TaskExt},
};
use dcl_component::transform_and_parent::{DclQuat, DclTranslation};
use ipfs::{
    ipfs_path::{IpfsKey, IpfsPath},
    ContentMap, EntityDefinition, IpfsAssetServer, IpfsIo, IpfsModifier, IpfsResource,
};
use scene_material::{BoundRegion, SceneBound, SceneBounds, SceneMaterial};
use serde::Deserialize;
use system_api_types::services::Service;

use crate::{
    initialize_scene::{
        parcels_in_range, process_scene_lifecycle, LiveScenes, PointerResult, ScenePointers,
        PARCEL_SIZE,
    },
    renderer_context::RendererSceneContext,
    scene_fade::{animate_scene_fades, FadedOut, SceneFade},
    scene_partition::Partition,
    update_world::gltf_container::scene_gltf_loader_settings,
    PrimaryUser,
};

/// How far from the player a scene that is not live gets a near LOD: unity-explorer's LOD 0
/// band. The live load distance lies inside it, so a loading scene has one too.
pub const NEAR_LOD_DISTANCE: f32 = 80.0;

/// How far a scene that is not live gets a far LOD, the pipeline's one mesh for it:
/// unity-explorer's LOD 1 band, out to 20 parcels. Beyond it only the imposters remain.
pub const FAR_LOD_DISTANCE: f32 = 320.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LodLevel {
    /// the scene's own models, placed from the descriptor
    Near,
    /// the pipeline's one mesh for the scene
    Far,
}

pub struct SceneLodPlugin;

impl Plugin for SceneLodPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                update_scene_lods,
                start_scene_lods,
                build_scene_lods,
                animate_scene_fades,
            )
                .chain()
                .after(process_scene_lifecycle)
                .in_set(SceneSets::Init),
        );
    }
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

/// One stand-in of one scene: the root its models hang from, at the scene's base parcel. While
/// it exists the scene's hash ranks its downloads in the LOD tier (the hooks), below every live
/// scene's; once the scene spawns, its own registration takes the hash to the scene tier.
#[derive(Component)]
#[component(on_insert = register_lod_hash, on_replace = release_lod_hash)]
pub struct SceneLod {
    pub hash: String,
    pub level: LodLevel,
    pub parcels: Vec<IVec2>,
    pub state: LodState,
}

pub enum LodState {
    /// not started: no partition yet, or the scene is wholly behind the camera
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

/// Which scenes have which stand-in, in a realm (never in preview): a scene whose content is not
/// on screen gets a near LOD while a parcel of it is within the near range, and a far LOD while
/// one is within the far range, started beyond the near range and kept inside it until the near
/// LOD shows. A stand-in goes when its scene's content is shown, when the scene leaves its
/// range, or when the realm's pointers no longer name it; a shown one that what replaces it has
/// covered dithers out first.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_scene_lods(
    mut commands: Commands,
    preview: Res<PreviewMode>,
    range: Res<SceneLoadDistance>,
    focus: Query<&GlobalTransform, With<PrimaryUser>>,
    pointers: Res<ScenePointers>,
    definitions: Res<SceneDefinitions>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<&RendererSceneContext>,
    lods: Query<(Entity, &SceneLod, Has<SceneFade>, Has<FadedOut>)>,
    mut scene_spawner: ResMut<SceneSpawner>,
) {
    let Ok(focus) = focus.single() else {
        return;
    };

    // the nearest parcel of every scene within the far range
    let mut nearest: HashMap<&str, f32> = HashMap::new();
    if !preview.is_preview && pointers.min() != IVec2::MAX {
        let in_range = parcels_in_range(focus, FAR_LOD_DISTANCE, pointers.min(), pointers.max());
        for (parcel, distance) in &in_range {
            if let Some(PointerResult::Exists { hash, .. }) = pointers.get(parcel) {
                let nearest = nearest.entry(hash.as_str()).or_insert(f32::MAX);
                *nearest = nearest.min(*distance);
            }
        }
    }
    let near_range = NEAR_LOD_DISTANCE.max(range.load);

    let shown = |hash: &str| {
        live_scenes
            .scenes
            .get(hash)
            .and_then(|entity| contexts.get(*entity).ok())
            .is_some_and(RendererSceneContext::content_shown)
    };
    let near_shown: HashSet<&str> = lods
        .iter()
        .filter(|(_, lod, ..)| {
            lod.level == LodLevel::Near && matches!(lod.state, LodState::Shown(_))
        })
        .map(|(_, lod, ..)| lod.hash.as_str())
        .collect();

    let mut existing: HashSet<(&str, LodLevel)> = HashSet::new();
    for (entity, lod, fading, faded_out) in &lods {
        let distance = nearest.get(lod.hash.as_str()).copied();
        let in_range = match lod.level {
            LodLevel::Near => distance.is_some_and(|distance| distance < near_range),
            LodLevel::Far => distance.is_some(),
        };
        // what replaces it is on screen: the live scene, or the near LOD over the far one
        let replaced = shown(&lod.hash)
            || (lod.level == LodLevel::Far && near_shown.contains(lod.hash.as_str()));
        if in_range && !replaced {
            existing.insert((lod.hash.as_str(), lod.level));
            continue;
        }
        if in_range && matches!(lod.state, LodState::Shown(_)) && !faded_out {
            // a shown stand-in dithers out across what replaced it, then goes
            if !fading {
                commands.entity(entity).insert(SceneFade::fade_out());
            }
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

    for (hash, distance) in nearest {
        if shown(hash) {
            continue;
        }
        let Some(definition) = definitions.0.get(hash) else {
            continue;
        };
        let level = if distance < near_range {
            LodLevel::Near
        } else {
            LodLevel::Far
        };
        if existing.contains(&(hash, level))
            || (level == LodLevel::Far && near_shown.contains(hash))
        {
            continue;
        }
        let origin = definition.base.as_vec2() * PARCEL_SIZE;
        commands.spawn((
            SceneLod {
                hash: hash.to_owned(),
                level,
                parcels: definition.parcels.clone(),
                state: LodState::Pending,
            },
            Transform::from_translation(Vec3::new(origin.x, 0.0, -origin.y)),
            Visibility::Hidden,
        ));
    }
}

/// Starts a stand-in once it has a partition and the scene is not wholly behind the camera
/// (unity-explorer fetches no LOD for one that is). A far LOD is one glTF by url, downloaded
/// under the scene's hash; a near LOD fetches the descriptor and turns it into the scene's glTF
/// loads, under the scene's content paths and loader settings.
fn start_scene_lods(
    mut lods: Query<(&mut SceneLod, Option<&Partition>)>,
    definitions: Res<SceneDefinitions>,
    ipfas: IpfsAssetServer,
) {
    for (lod, partition) in lods.iter_mut() {
        let lod = lod.into_inner();
        match &mut lod.state {
            LodState::Pending => {
                if !partition.is_some_and(|partition| !partition.0.is_behind) {
                    continue;
                }
                if lod.level == LodLevel::Far {
                    let url = format!(
                        "{}/LOD/lods-unity/lods/{}_1.glb",
                        common::base_domain::service(Service::AbgenCdn),
                        lod.hash.to_lowercase()
                    );
                    let path = IpfsPath::new_from_url(&url, "glb")
                        .with_keyvalue(IpfsKey::Owner, lod.hash.clone());
                    let handle = ipfas
                        .asset_server()
                        .load_with_settings::<Gltf, GltfLoaderSettings>(
                            PathBuf::from(&path),
                            scene_gltf_loader_settings(
                                RenderAssetTransferPriority::Priority(0),
                                false,
                            ),
                        );
                    lod.state = LodState::Loading(vec![LodModel {
                        handle,
                        transform: Transform::IDENTITY,
                        instance: None,
                        failed: false,
                    }]);
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
/// lights off, scene-bound materials) and shows the stand-in in one frame. A stand-in none of
/// whose files loaded shows nothing and leaves its parcels to the imposters.
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
    mut storage_buffers: ResMut<Assets<ShaderStorageBuffer>>,
    config: Res<AppConfig>,
    nodes: Query<(
        &ChildOf,
        &Transform,
        Option<&Name>,
        Option<&Mesh3d>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Has<SkinnedMesh>,
    )>,
) {
    // a near LOD coming up over a shown far one dithers in as the far one dithers out; over an
    // imposter, which cannot fade, it shows at once
    let far_shown: HashSet<String> = lods
        .iter()
        .filter(|(_, lod)| lod.level == LodLevel::Far && matches!(lod.state, LodState::Shown(_)))
        .map(|(_, lod)| lod.hash.clone())
        .collect();

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
        if models.iter().all(|model| model.instance.is_none()) {
            // every file failed (a mesh the loader refuses, a missing file): nothing to show,
            // and the parcels stay the imposters'
            lod.state = LodState::Nothing;
            continue;
        }

        let regions = scene_regions(lod.parcels.iter().copied())
            .into_iter()
            .map(|region| BoundRegion::new(region.min, region.max, region.count))
            .collect::<Vec<_>>();
        let bounds = SceneBounds::new(&regions, Some(storage_buffers.as_mut()));
        let mut dressed: HashMap<AssetId<StandardMaterial>, Handle<SceneMaterial>> =
            HashMap::new();
        let mut instances = Vec::new();

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
                // collider nodes are not drawn (ADR-215)
                if hidden_node(entity, anchor, &nodes) {
                    node.remove::<Mesh3d>();
                    continue;
                }
                if let Some(mesh) = mesh {
                    upload_mesh(&mut meshes, mesh.id(), skinned);
                }
                if let Some(material) = material {
                    let Some(base) = materials.get(material.id()) else {
                        continue;
                    };
                    let handle = dressed
                        .entry(material.id())
                        .or_insert_with(|| {
                            scene_materials.add(SceneMaterial {
                                base: base.clone(),
                                extension: SceneBound::new(&bounds, config.graphics.oob),
                            })
                        })
                        .clone();
                    node.remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(handle));
                    if base.unlit {
                        node.insert(NotShadowCaster);
                    }
                }
            }
        }

        let fade_in = lod.level == LodLevel::Near && far_shown.contains(&lod.hash);
        lod.state = LodState::Shown(instances);
        let mut root = commands.entity(root);
        root.insert(Visibility::Inherited);
        if fade_in {
            root.insert(SceneFade::fade_in());
        }
    }
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
) -> bool {
    let mut current = entity;
    while let Ok((child_of, _, name, ..)) = nodes.get(current) {
        if name.is_some_and(|name| {
            let name = name.to_lowercase();
            name.ends_with("_collider") || name.ends_with("_trigger")
        }) {
            return true;
        }
        if child_of.parent() == anchor {
            break;
        }
        current = child_of.parent();
    }
    false
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
