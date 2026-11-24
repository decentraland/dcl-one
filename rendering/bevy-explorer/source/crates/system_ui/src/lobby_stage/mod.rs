//! The system scene owns the preview avatar and its camera and names the look and camera layer;
//! the stage is built on that layer around the avatar's feet and refitted to the camera every
//! frame.

mod diagnostics;
mod dust;
mod materials;
pub mod presets;

use std::f32::consts::PI;

use bevy::{
    app::Propagate,
    asset::embedded_asset,
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    math::Affine2,
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::{
        camera::Exposure,
        view::{ColorGrading, NoFrustumCulling, RenderLayers},
    },
    scene::SceneInstanceReady,
};
use common::{
    sets::{PostUpdateSets, SceneSets},
    structs::{sun_cascades, AppConfig, SessionGraphics, StageCamera, StageLight, MIN_CASCADE},
};
use scene_runner::{initialize_scene::SuperUserScene, renderer_context::RendererSceneContext};
use system_bridge::{settings::NewCameraEvent, LobbyStage, SystemApi};
use texture_camera::TextureLayersCache;

use self::{
    materials::{
        order, BlobMaterial, DustMaterial, FloorExtension, FloorMaterial, FloorUniform,
        MistMaterial, MistUniform, RockExtension, RockMaterial, RockUniform, VignetteMaterial,
        VignetteUniform,
    },
    presets::{mirrored_rotation, StagePreset},
};

const ORIGIN_ABOVE_FEET: f32 = 1.0;
/// Heights in the stage's frame.
const FLOOR_Y: f32 = -1.116;
const PODIUM_Y: f32 = -1.132;
const BLOB_Y: f32 = -0.995;
const BLOB_SIZE: f32 = 1.4;
const FLOOR_SIZE: f32 = 60.0;
const FLOOR_TILING: f32 = 12.0;
const FLOOR_SPOT_RADIUS: f32 = 0.67;
const FLOOR_SPOT_SOFTNESS: f32 = 0.26;
/// How far from the camera the key light casts shadows, in metres.
const STAGE_SHADOW_DISTANCE: f32 = 20.0;
/// Its size and height are refitted to the camera every frame.
const BACKDROP_POSITION: Vec3 = Vec3::new(0.0, 1.0, -15.0);
const MIST_POSITION: Vec3 = Vec3::new(0.0, -1.87, -6.0);
const MIST_SIZE: Vec2 = Vec2::new(21.2, 1.25);
const MIST_SCALE: f32 = 2.5;
const MIST_SPEED: Vec4 = Vec4::new(0.2, 0.05, -0.016, 0.02);
const MIST_THRESHOLD: f32 = 0.307;
const MIST_SOFTNESS: f32 = 0.476;
const MIST_EDGE_SOFTNESS: f32 = 0.36;
const BLOB_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.65);
const DUST_TINT: Color = Color::srgba(1.0, 0.96, 0.9, 1.0);
const DUST_SOFTNESS: f32 = 1.0;
/// Moss tint and the moss mask (the higher, the more rock shows through), per rock model.
const ROCK_MOSS: &[(&str, Color, f32)] = &[
    (
        "rock01",
        Color::srgba(0.043137256, 0.42745098, 0.3019608, 1.0),
        0.9,
    ),
    (
        "rock02",
        Color::srgba(0.042052332, 0.4245283, 0.3003053, 1.0),
        0.734,
    ),
    (
        "rock03",
        Color::srgba(0.043137256, 0.42745098, 0.3019608, 1.0),
        0.886,
    ),
];

pub struct LobbyStagePlugin;

impl Plugin for LobbyStagePlugin {
    fn build(&self, app: &mut App) {
        add_materials(app);
        app.add_systems(
            Update,
            (take_requests, sync_stage)
                .chain()
                .in_set(SceneSets::RestrictedActions),
        )
        .add_systems(PostUpdate, fit_stage.in_set(PostUpdateSets::CameraFit))
        .add_observer(moss_rocks)
        .add_observer(layer_scene);
        diagnostics::register(app);
    }
}

fn add_materials(app: &mut App) {
    embedded_asset!(app, "lobby_floor.wgsl");
    embedded_asset!(app, "lobby_rock.wgsl");
    embedded_asset!(app, "lobby_mist.wgsl");
    embedded_asset!(app, "lobby_dust.wgsl");
    embedded_asset!(app, "lobby_blob.wgsl");
    embedded_asset!(app, "lobby_vignette.wgsl");
    app.add_plugins((
        MaterialPlugin::<FloorMaterial>::default(),
        MaterialPlugin::<RockMaterial>::default(),
        MaterialPlugin::<MistMaterial>::default(),
        MaterialPlugin::<DustMaterial>::default(),
        MaterialPlugin::<BlobMaterial>::default(),
        MaterialPlugin::<VignetteMaterial>::default(),
    ));
}

/// On the system scene's root: what the scene asked for, so it goes with the scene.
#[derive(Component)]
struct StageRequest(LobbyStage);

type SystemRoot = (With<SuperUserScene>, With<RendererSceneContext>);

#[derive(Component)]
struct Stage {
    preset: &'static StagePreset,
    layer: u32,
    render_layer: usize,
    feet: Vec3,
    camera: Option<Entity>,
}

#[derive(Component)]
struct Backdrop;

#[derive(Component)]
struct StageFloor;

#[derive(Component)]
struct Vignette;

#[derive(Component)]
struct Mossy {
    moss: LinearRgba,
    mask: f32,
}

fn take_requests(
    mut events: EventReader<SystemApi>,
    roots: Query<Entity, SystemRoot>,
    mut commands: Commands,
) {
    for event in events.read() {
        let SystemApi::SetLobbyStage(stage) = event else {
            continue;
        };
        let Some(root) = roots.iter().next() else {
            warn!("setLobbyStage: no system scene");
            continue;
        };
        match stage {
            Some(stage) if !stage.feet.world_vec_to_vec3().is_finite() => {
                warn!("setLobbyStage: ignoring non-finite feet");
            }
            Some(stage) => {
                commands
                    .entity(root)
                    .try_insert(StageRequest(stage.clone()));
            }
            None => {
                commands.entity(root).try_remove::<StageRequest>();
            }
        }
    }
}

fn render_layer(cache: &TextureLayersCache, root: Entity, layer: u32) -> Option<usize> {
    if layer == 0 {
        return None;
    }
    cache.layer(root, layer).map(|l| l as usize)
}

type CameraItem = (
    Entity,
    Ref<'static, GlobalTransform>,
    &'static RenderLayers,
    &'static Projection,
    Option<&'static Exposure>,
);

fn sync_stage(
    mut commands: Commands,
    mut stages: Query<(Entity, &mut Stage)>,
    cache: Res<TextureLayersCache>,
    roots: Query<(Entity, &StageRequest, &GlobalTransform), SystemRoot>,
    cameras: Query<CameraItem, With<Camera3d>>,
    mut new_cameras: EventWriter<NewCameraEvent>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut builder: StageAssets,
    (config, session): (Res<AppConfig>, Res<SessionGraphics>),
) {
    let wanted = roots.iter().next().and_then(|(root, request, transform)| {
        let render_layer = render_layer(&cache, root, request.0.layer)?;
        Some((&request.0, render_layer, root, transform))
    });

    // The stage faces the perspective camera that renders its layer. A texture camera is rebuilt
    // whenever its size changes (a window resize), so the camera is looked up again every frame
    // rather than kept from when the stage was built.
    let camera_for = |render_layer: usize| {
        let layers = RenderLayers::layer(render_layer);
        cameras
            .iter()
            .find(|(_, _, l, p, _)| **l == layers && matches!(p, Projection::Perspective(_)))
    };

    let wanted_camera = wanted
        .as_ref()
        .and_then(|(_, render_layer, ..)| camera_for(*render_layer))
        .map(|(c, ..)| c);
    let mut released = Vec::new();
    let mut built = false;
    for (entity, mut stage) in &mut stages {
        let keep = wanted.as_ref().is_some_and(|(want, render_layer, ..)| {
            presets::preset(&want.preset).id == stage.preset.id
                && want.layer == stage.layer
                && *render_layer == stage.render_layer
                && want.feet.world_vec_to_vec3() == stage.feet
        });
        if !keep {
            released.extend(stage.camera);
            commands.entity(entity).despawn();
            continue;
        }
        built = true;
        if stage.camera != wanted_camera {
            released.extend(stage.camera);
            stage.camera = wanted_camera;
            if let Some(camera) = wanted_camera {
                unprocessed(&mut commands, camera);
            }
        }
    }
    for camera in released {
        if Some(camera) != wanted_camera && cameras.contains(camera) {
            processed(&mut commands, &mut new_cameras, camera);
        }
    }

    let Some((want, render_layer, scene_root, root_transform)) = wanted else {
        return;
    };
    if built {
        return;
    }
    let preset = presets::preset(&want.preset);
    let layers = RenderLayers::layer(render_layer);
    let Some((camera, camera_transform, _, _, exposure)) = camera_for(render_layer) else {
        return;
    };
    // a camera spawned this frame is not placed until PostUpdate: build next frame
    if camera_transform.is_added() {
        return;
    }
    // The stage hangs from the scene's root, as the avatar and its camera do, so it moves with
    // them (a scene's root starts parked below the world and is raised once the scene runs).
    let feet = want.feet.world_vec_to_vec3();
    let origin = feet + Vec3::Y * ORIGIN_ABOVE_FEET;
    let camera_local = root_transform
        .affine()
        .inverse()
        .transform_point3(camera_transform.translation());
    let to_camera = (camera_local - origin) * Vec3::new(1.0, 0.0, 1.0);
    let facing = if to_camera.length_squared() > 1e-6 {
        Quat::from_rotation_arc(Vec3::Z, to_camera.normalize())
    } else {
        Quat::IDENTITY
    };
    let exposure = exposure.copied().unwrap_or_default().exposure();
    unprocessed(&mut commands, camera);

    let root = commands
        .spawn((
            Stage {
                preset,
                layer: want.layer,
                render_layer,
                feet,
                camera: Some(camera),
            },
            Transform::from_translation(origin).with_rotation(facing),
            Visibility::default(),
            Propagate(layers.clone()),
            ChildOf(scene_root),
        ))
        .id();
    // a change of the setting while the stage is up reaches its light through `apply_shadows`
    let shadows = sun_cascades(
        session.shadow_setting(&config),
        config.graphics.shadow_distance,
    )
    .is_some();
    builder.build(
        &mut commands,
        root,
        &layers,
        preset,
        exposure,
        shadows,
        &asset_server,
        &mut meshes,
    );
}

/// The stage is lit to come out as its source draws it, with no post-processing: no tonemapping,
/// grading or bloom on the camera that shows it (the engine's defaults crush its darkened rim).
fn unprocessed(commands: &mut Commands, camera: Entity) {
    commands
        .entity(camera)
        .try_insert((StageCamera, Tonemapping::None, ColorGrading::default()))
        .try_remove::<Bloom>();
}

/// A camera the stage no longer shows gets the engine's processing back, and the settings
/// re-applied over it.
fn processed(
    commands: &mut Commands,
    new_cameras: &mut EventWriter<NewCameraEvent>,
    camera: Entity,
) {
    commands
        .entity(camera)
        .try_remove::<StageCamera>()
        .try_insert(platform::default_camera_components());
    new_cameras.write(NewCameraEvent(camera));
}

#[derive(bevy::ecs::system::SystemParam)]
struct StageAssets<'w> {
    standard: ResMut<'w, Assets<StandardMaterial>>,
    floor: ResMut<'w, Assets<FloorMaterial>>,
    mist: ResMut<'w, Assets<MistMaterial>>,
    dust: ResMut<'w, Assets<DustMaterial>>,
    blob: ResMut<'w, Assets<BlobMaterial>>,
    vignette: ResMut<'w, Assets<VignetteMaterial>>,
}

/// The key light's illuminance for a source intensity: lit surfaces come out at the intensity
/// times their albedo after exposure, as the source's lighting does, where the PBR diffuse term
/// divides by pi.
pub fn key_light_illuminance(intensity: f32, exposure: f32) -> f32 {
    intensity * PI / exposure
}

impl StageAssets<'_> {
    fn build(
        &mut self,
        commands: &mut Commands,
        root: Entity,
        // set as each part spawns: propagation lands a frame later, on the default layer meanwhile
        layers: &RenderLayers,
        preset: &'static StagePreset,
        exposure: f32,
        shadows: bool,
        asset_server: &AssetServer,
        meshes: &mut Assets<Mesh>,
    ) {
        let unit_rect = meshes.add(Rectangle::new(1.0, 1.0));

        let backdrop = asset_server.load(preset.backdrop);
        commands.spawn((
            Backdrop,
            Mesh3d(unit_rect.clone()),
            MeshMaterial3d(self.standard.add(StandardMaterial {
                base_color_texture: Some(backdrop),
                unlit: true,
                cull_mode: None,
                ..Default::default()
            })),
            Transform::from_translation(BACKDROP_POSITION),
            NotShadowCaster,
            NotShadowReceiver,
            layers.clone(),
            ChildOf(root),
        ));

        let ground = asset_server.load_with_settings(
            "embedded://lobby_stage/ground.jpg",
            |settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
            },
        );
        commands.spawn((
            StageFloor,
            Mesh3d(meshes.add(Plane3d::default().mesh().size(FLOOR_SIZE, FLOOR_SIZE))),
            MeshMaterial3d(self.floor.add(FloorMaterial {
                base: StandardMaterial {
                    base_color: preset.floor_color,
                    base_color_texture: Some(ground),
                    uv_transform: Affine2::from_scale(Vec2::splat(FLOOR_TILING)),
                    perceptual_roughness: 1.0,
                    metallic: 0.0,
                    reflectance: 0.0,
                    alpha_mode: AlphaMode::Blend,
                    depth_bias: order::FLOOR,
                    ..Default::default()
                },
                extension: FloorExtension {
                    floor: FloorUniform {
                        spot_color: preset.spot_color.to_linear().to_vec4(),
                        blend_axis: Vec4::NEG_Z,
                        spot_radius: FLOOR_SPOT_RADIUS,
                        spot_softness: FLOOR_SPOT_SOFTNESS,
                        blend_start: f32::MAX,
                        blend_end: f32::MAX,
                        blend_hardness: preset.blend_hardness,
                    },
                },
            })),
            Transform::from_xyz(0.0, FLOOR_Y, 0.0),
            NotShadowCaster,
            layers.clone(),
            ChildOf(root),
        ));

        commands.spawn((
            SceneRoot(asset_server.load("embedded://lobby_stage/podium.glb#Scene0")),
            Transform::from_xyz(0.0, PODIUM_Y, 0.0),
            layers.clone(),
            ChildOf(root),
        ));

        let shape = asset_server.load_with_settings(
            "embedded://lobby_stage/blob_shadow.png",
            |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
        );
        commands.spawn((
            Mesh3d(unit_rect.clone()),
            MeshMaterial3d(self.blob.add(BlobMaterial {
                color: BLOB_COLOR.to_linear().to_vec4(),
                shape,
            })),
            Transform::from_xyz(0.0, BLOB_Y, 0.0)
                .with_rotation(Quat::from_rotation_x(-PI / 2.0))
                .with_scale(Vec3::new(BLOB_SIZE, BLOB_SIZE, 1.0)),
            NotShadowCaster,
            NotShadowReceiver,
            layers.clone(),
            ChildOf(root),
        ));

        if let Some(color) = preset.mist {
            commands.spawn((
                Mesh3d(unit_rect.clone()),
                MeshMaterial3d(self.mist.add(MistMaterial {
                    mist: MistUniform {
                        color: color.to_linear().to_vec4(),
                        speed: MIST_SPEED,
                        scale: MIST_SCALE,
                        threshold: MIST_THRESHOLD,
                        softness: MIST_SOFTNESS,
                        edge_softness: MIST_EDGE_SOFTNESS,
                    },
                })),
                Transform::from_translation(MIST_POSITION).with_scale(Vec3::new(
                    MIST_SIZE.x,
                    MIST_SIZE.y,
                    1.0,
                )),
                NotShadowCaster,
                NotShadowReceiver,
                layers.clone(),
                ChildOf(root),
            ));
        }

        if let Some(color) = preset.dust {
            commands.spawn((
                Mesh3d(meshes.add(dust::mesh())),
                MeshMaterial3d(self.dust.add(DustMaterial {
                    dust: dust::uniform(DUST_TINT.to_linear(), color.to_linear(), DUST_SOFTNESS),
                })),
                Transform::default(),
                NoFrustumCulling,
                NotShadowCaster,
                NotShadowReceiver,
                layers.clone(),
                ChildOf(root),
            ));
        }

        for prop in preset.props {
            let mut entity = commands.spawn((
                SceneRoot(
                    asset_server.load(format!("embedded://lobby_stage/{}.glb#Scene0", prop.model)),
                ),
                Transform::from_translation(prop.position + Vec3::Y * FLOOR_Y)
                    .with_rotation(mirrored_rotation(prop.rotation))
                    .with_scale(Vec3::splat(prop.scale)),
                layers.clone(),
                ChildOf(root),
            ));
            if let Some((_, moss, mask)) =
                ROCK_MOSS.iter().find(|(model, _, _)| *model == prop.model)
            {
                entity.insert(Mossy {
                    moss: moss.to_linear(),
                    mask: *mask,
                });
            }
        }

        // the source light shines along its +z, this one along its -z
        commands.spawn((
            DirectionalLight {
                color: preset.light_color,
                illuminance: key_light_illuminance(preset.light_intensity, exposure),
                shadows_enabled: shadows,
                ..Default::default()
            },
            CascadeShadowConfigBuilder {
                num_cascades: 1,
                minimum_distance: MIN_CASCADE,
                maximum_distance: STAGE_SHADOW_DISTANCE,
                ..Default::default()
            }
            .build(),
            StageLight,
            Transform::from_rotation(
                mirrored_rotation(preset.light_rotation) * Quat::from_rotation_y(PI),
            ),
            layers.clone(),
            ChildOf(root),
        ));

        if preset.vignette.intensity > 0.0 {
            let v = preset.vignette;
            commands.spawn((
                Vignette,
                Mesh3d(unit_rect),
                MeshMaterial3d(self.vignette.add(VignetteMaterial {
                    vignette: VignetteUniform {
                        color: v.color.to_linear().to_vec4(),
                        mask_radii: v.mask_radii.extend(0.0),
                        mask_center: v.mask_center,
                        intensity: v.intensity,
                        smoothness: v.smoothness,
                        mask_softness: v.mask_softness,
                    },
                })),
                Transform::default(),
                NoFrustumCulling,
                NotShadowCaster,
                NotShadowReceiver,
                layers.clone(),
                ChildOf(root),
            ));
        }
    }
}

/// A model's meshes spawn after the stage, so they take its layer here.
fn layer_scene(
    trigger: Trigger<SceneInstanceReady>,
    parents: Query<&ChildOf>,
    stages: Query<&Stage>,
    children: Query<&Children>,
    mut commands: Commands,
) {
    let Ok(stage) = parents
        .get(trigger.target())
        .and_then(|parent| stages.get(parent.parent()))
    else {
        return;
    };
    let layers = RenderLayers::layer(stage.render_layer);
    for entity in children.iter_descendants(trigger.target()) {
        commands.entity(entity).try_insert(layers.clone());
    }
}

fn moss_rocks(
    trigger: Trigger<SceneInstanceReady>,
    rocks: Query<&Mossy>,
    children: Query<&Children>,
    surfaces: Query<&MeshMaterial3d<StandardMaterial>>,
    standard: Res<Assets<StandardMaterial>>,
    mut rock_materials: ResMut<Assets<RockMaterial>>,
    mut commands: Commands,
) {
    let Ok(mossy) = rocks.get(trigger.target()) else {
        return;
    };
    for entity in children.iter_descendants(trigger.target()) {
        let Ok(material) = surfaces.get(entity) else {
            continue;
        };
        let Some(base) = standard.get(&material.0) else {
            continue;
        };
        let rock = rock_materials.add(RockMaterial {
            base: base.clone(),
            extension: RockExtension {
                rock: RockUniform {
                    moss: mossy.moss.to_vec4(),
                    moss_mask: mossy.mask,
                },
            },
        });
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(rock));
    }
}

/// `x` and `y` are 0..1 from the frame's bottom left.
fn frame_ray(camera: &GlobalTransform, half_height: f32, aspect: f32, x: f32, y: f32) -> Ray3d {
    let local = Vec3::new(
        (x * 2.0 - 1.0) * half_height * aspect,
        (y * 2.0 - 1.0) * half_height,
        -1.0,
    );
    Ray3d::new(
        camera.translation(),
        Dir3::new(camera.rotation() * local).unwrap_or(Dir3::NEG_Z),
    )
}

fn hit(ray: Ray3d, plane_point: Vec3, normal: Vec3) -> Option<Vec3> {
    let denominator = ray.direction.dot(normal);
    if denominator.abs() < 1e-6 {
        return None;
    }
    let t = (plane_point - ray.origin).dot(normal) / denominator;
    (t > 0.0).then(|| ray.get_point(t))
}

/// The smallest size with the texture's aspect whose part above the blend line covers the
/// width and the height needed.
pub fn cover_size(
    width: f32,
    height_above_blend: f32,
    texture_aspect: f32,
    below_blend: f32,
) -> Vec2 {
    let height = (height_above_blend / (1.0 - below_blend)).max(width / texture_aspect);
    Vec2::new(height * texture_aspect, height)
}

/// In world space.
#[derive(Debug, PartialEq)]
pub struct Fit {
    /// The dissolve's axis (away from the camera) and its start and end along it.
    pub blend_axis: Vec3,
    pub blend_start: f32,
    pub blend_end: f32,
    pub backdrop_centre: Vec3,
    pub backdrop_size: Vec2,
    pub backdrop_rotation: Quat,
}

pub fn fit(
    camera: &GlobalTransform,
    fov: f32,
    aspect: f32,
    floor_y: f32,
    backdrop_rest: Vec3,
    texture_aspect: Option<f32>,
    preset: &StagePreset,
) -> Option<Fit> {
    let forward = camera.forward().as_vec3() * Vec3::new(1.0, 0.0, 1.0);
    let flat_forward = forward.try_normalize()?;
    let half_height = (fov * 0.5).tan();
    let blend_ray = frame_ray(camera, half_height, aspect, 0.5, preset.blend_screen_height);

    let floor_hit = hit(blend_ray, Vec3::new(0.0, floor_y, 0.0), Vec3::Y)?;
    let blend_end = floor_hit.dot(flat_forward);

    let camera_position = camera.translation();
    let distance = (backdrop_rest - camera_position).dot(flat_forward);
    let wall_centre = camera_position + flat_forward * distance;
    let right = Vec3::Y.cross(flat_forward);

    let blend_y = hit(blend_ray, wall_centre, -flat_forward)
        .map(|p| p.y.max(floor_y))
        .unwrap_or(floor_y);
    let mut top = blend_y;
    let mut half_width = 0.0f32;
    for (x, y) in [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
        let ray = frame_ray(camera, half_height, aspect, x, y);
        if let Some(p) = hit(ray, wall_centre, -flat_forward) {
            top = top.max(p.y);
            half_width = half_width.max((p - wall_centre).dot(right).abs());
        }
    }

    let size = cover_size(
        half_width * 2.0,
        top - blend_y,
        texture_aspect.unwrap_or(aspect),
        preset.image_below_blend,
    );
    let mut centre = wall_centre;
    centre.y = blend_y - size.y * preset.image_below_blend + size.y * 0.5;

    Some(Fit {
        blend_axis: flat_forward,
        blend_start: blend_end - preset.blend_depth,
        blend_end,
        backdrop_centre: centre,
        backdrop_size: size,
        backdrop_rotation: Quat::from_rotation_arc(Vec3::Z, -flat_forward),
    })
}

fn fit_stage(
    stages: Query<(&Stage, &GlobalTransform, &Children)>,
    cameras: Query<(&GlobalTransform, &Projection)>,
    mut backdrops: Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        (With<Backdrop>, Without<StageFloor>, Without<Stage>),
    >,
    floors: Query<&MeshMaterial3d<FloorMaterial>, With<StageFloor>>,
    standard: Res<Assets<StandardMaterial>>,
    mut floor_materials: ResMut<Assets<FloorMaterial>>,
    images: Res<Assets<Image>>,
) {
    for (stage, stage_transform, children) in &stages {
        let Some((camera, Projection::Perspective(projection))) =
            stage.camera.and_then(|c| cameras.get(c).ok())
        else {
            continue;
        };
        let floor_y = stage_transform.transform_point(Vec3::Y * FLOOR_Y).y;
        let backdrop_rest = stage_transform.transform_point(BACKDROP_POSITION);
        let to_stage = stage_transform.affine().inverse();

        let Some(backdrop) = children.iter().find(|c| backdrops.contains(*c)) else {
            continue;
        };
        let Ok((mut transform, material)) = backdrops.get_mut(backdrop) else {
            continue;
        };
        let texture_aspect = standard
            .get(&material.0)
            .and_then(|m| m.base_color_texture.as_ref())
            .and_then(|t| images.get(t))
            .map(|image| image.width() as f32 / image.height() as f32);
        let Some(fit) = fit(
            camera,
            projection.fov,
            projection.aspect_ratio,
            floor_y,
            backdrop_rest,
            texture_aspect,
            stage.preset,
        ) else {
            continue;
        };
        let (_, stage_rotation, _) = stage_transform.to_scale_rotation_translation();
        let local = Transform {
            translation: to_stage.transform_point3(fit.backdrop_centre),
            rotation: stage_rotation.inverse() * fit.backdrop_rotation,
            scale: fit.backdrop_size.extend(1.0),
        };
        if *transform != local {
            *transform = local;
        }

        // `get_mut` marks the material modified, which re-prepares it
        let blend_axis = fit.blend_axis.extend(0.0);
        let Some(floor) = children.iter().find_map(|c| floors.get(c).ok()) else {
            continue;
        };
        let current = floor_materials.get(&floor.0).map(|m| &m.extension.floor);
        if current.is_some_and(|u| {
            u.blend_axis != blend_axis
                || u.blend_start != fit.blend_start
                || u.blend_end != fit.blend_end
        }) {
            if let Some(material) = floor_materials.get_mut(&floor.0) {
                let uniform = &mut material.extension.floor;
                uniform.blend_axis = blend_axis;
                uniform.blend_start = fit.blend_start;
                uniform.blend_end = fit.blend_end;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Bevy panics on conflicting query access when it initialises a system.
    #[test]
    fn stage_systems_have_disjoint_queries() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let _ = world.run_system_once(fit_stage);
        let _ = world.run_system_once(sync_stage);
    }

    /// The request lives on the system scene's root, so a respawned scene starts with none.
    #[test]
    fn a_stage_request_is_kept_on_the_system_scene_root() {
        use bevy::{ecs::system::RunSystemOnce, platform::collections::HashSet};
        use dcl_types::SceneId;

        let mut world = World::new();
        world.init_resource::<Events<SystemApi>>();
        let root = world
            .spawn((
                SuperUserScene,
                RendererSceneContext::new(
                    SceneId::DUMMY,
                    "hash".to_owned(),
                    "storage_root".to_owned(),
                    false,
                    0,
                    "title".to_owned(),
                    IVec2::ZERO,
                    HashSet::from_iter([IVec2::ZERO]),
                    vec![],
                    Default::default(),
                    vec![],
                    Entity::PLACEHOLDER,
                    0.,
                    false,
                    "sdk_version",
                    false,
                    false,
                ),
            ))
            .id();
        let stage = LobbyStage {
            preset: "halloween-2026".to_owned(),
            layer: 1,
            feet: Default::default(),
        };
        let send = |world: &mut World, stage: Option<LobbyStage>| {
            world.send_event(SystemApi::SetLobbyStage(stage));
            world.run_system_once(take_requests).unwrap();
        };

        send(&mut world, Some(stage.clone()));
        assert!(world.entity(root).contains::<StageRequest>());
        send(&mut world, None);
        assert!(!world.entity(root).contains::<StageRequest>());

        send(&mut world, Some(stage));
        world.despawn(root);
        assert_eq!(world.query::<&StageRequest>().iter(&world).count(), 0);
    }

    /// Whichever of the stage and the bloom setting reaches a new camera first, the stage's
    /// camera ends without bloom; once released it is processed again and follows the setting.
    #[test]
    fn the_stage_camera_keeps_no_bloom_and_gets_it_back_when_released() {
        use bevy::ecs::system::RunSystemOnce;
        use common::structs::BloomSetting;
        use system_bridge::settings::AppSetting;

        let bloom = |commands: &mut Commands, camera: Entity| {
            BloomSetting::High.apply_to_camera(&(), commands.reborrow(), camera)
        };
        for stage_first in [true, false] {
            let mut world = World::new();
            world.init_resource::<Events<NewCameraEvent>>();
            let camera = world.spawn(Camera3d::default()).id();
            world
                .run_system_once(move |mut commands: Commands| {
                    if stage_first {
                        unprocessed(&mut commands, camera);
                        bloom(&mut commands, camera);
                    } else {
                        bloom(&mut commands, camera);
                        unprocessed(&mut commands, camera);
                    }
                })
                .unwrap();
            let entity = world.entity(camera);
            assert!(entity.contains::<StageCamera>());
            assert!(!entity.contains::<Bloom>(), "stage first: {stage_first}");
            assert_eq!(entity.get::<Tonemapping>(), Some(&Tonemapping::None));

            world
                .run_system_once(
                    move |mut commands: Commands, mut new_cameras: EventWriter<NewCameraEvent>| {
                        processed(&mut commands, &mut new_cameras, camera);
                    },
                )
                .unwrap();
            let entity = world.entity(camera);
            assert!(!entity.contains::<StageCamera>());
            assert_ne!(entity.get::<Tonemapping>(), Some(&Tonemapping::None));
            assert_eq!(world.resource::<Events<NewCameraEvent>>().len(), 1);
            world
                .run_system_once(move |mut commands: Commands| bloom(&mut commands, camera))
                .unwrap();
            assert!(world.entity(camera).contains::<Bloom>());
        }
    }

    #[test]
    fn cover_size_keeps_the_aspect_and_covers_both_ways() {
        let size = cover_size(20.0, 4.0, 2.0, 0.5);
        assert_eq!(size, Vec2::new(20.0, 10.0));
        let size = cover_size(4.0, 6.0, 2.0, 0.5);
        assert_eq!(size, Vec2::new(24.0, 12.0));
    }

    #[test]
    fn fit_ends_the_dissolve_under_the_blend_line_and_covers_the_frame() {
        let preset = presets::preset("halloween-2026");
        let feet = Vec3::ZERO;
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.0, 1.8, 6.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        );
        let fov = 26.2f32.to_radians();
        let aspect = 16.0 / 9.0;
        let floor_y = feet.y + ORIGIN_ABOVE_FEET + FLOOR_Y;
        let rest = feet + Vec3::Y * ORIGIN_ABOVE_FEET + BACKDROP_POSITION;
        let fit = fit(
            &camera,
            fov,
            aspect,
            floor_y,
            rest,
            Some(16.0 / 9.0),
            preset,
        )
        .unwrap();

        assert!((fit.blend_axis - Vec3::NEG_Z).length() < 1e-5);
        assert!((fit.blend_end - fit.blend_start - preset.blend_depth).abs() < 1e-4);
        assert!(fit.blend_end > 0.0, "{fit:?}");
        assert!((fit.backdrop_rotation * Vec3::Z - Vec3::Z).length() < 1e-5);
        let distance = 6.0 - rest.z;
        let frame_width = 2.0 * distance * (fov * 0.5).tan() * aspect;
        assert!(fit.backdrop_size.x >= frame_width - 1e-3, "{fit:?}");
        let bottom = fit.backdrop_centre.y - fit.backdrop_size.y * 0.5;
        assert!(bottom < floor_y + 1.0);
    }

    /// With the lobby camera's target 55% down the frame, the dissolve starts behind every prop: one
    /// half sunk in the floor shows nothing through it.
    #[test]
    fn the_floor_is_solid_over_the_game_world_props() {
        let preset = presets::preset("game-world");
        let fov = 26.2f32.to_radians();
        let pitch = (1.0f32 / 6.0).atan() - (0.1 * (fov * 0.5).tan()).atan();
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.0, 1.8, 6.0).with_rotation(Quat::from_rotation_x(-pitch)),
        );
        let floor_y = ORIGIN_ABOVE_FEET + FLOOR_Y;
        let rest = Vec3::Y * ORIGIN_ABOVE_FEET + BACKDROP_POSITION;
        let fit = fit(&camera, fov, 16.0 / 9.0, floor_y, rest, None, preset).unwrap();

        // as lobby_floor.wgsl
        let opacity = |along: f32| {
            let t = ((along - fit.blend_start) / (fit.blend_end - fit.blend_start)).clamp(0.0, 1.0);
            let blend = t * t * (3.0 - 2.0 * t);
            let hardened = (blend - 0.5) / (1.0 - preset.blend_hardness).max(0.001) + 0.5;
            1.0 - hardened.clamp(0.0, 1.0)
        };
        assert!(!preset.props.is_empty());
        for prop in preset.props {
            let along = prop.position.dot(fit.blend_axis);
            assert_eq!(opacity(along), 1.0, "{} at {}", prop.model, prop.position);
        }
    }

    #[test]
    fn key_light_brightness_matches_the_source_after_exposure() {
        let exposure = Exposure::default().exposure();
        let e = key_light_illuminance(2.0, exposure);
        // a white surface facing the light: illuminance / pi * exposure
        assert!((e / PI * exposure - 2.0).abs() < 1e-4);
    }

    /// Every stage shader compiles on this machine's adapter, in a renderer with no window, with
    /// the prepasses and a shadowing light so those variants compile too. Needs a GPU, so
    /// `#[ignore]`d and skipped by CI:
    ///
    /// ```text
    /// cargo test -p system_ui lobby_stage_pipelines_compile -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn lobby_stage_pipelines_compile() {
        use bevy::{
            core_pipeline::prepass::{DepthPrepass, NormalPrepass},
            render::camera::RenderTarget,
        };
        use pipeline_test::{compile_pipelines, headless_app, render_target};

        let mut app = headless_app();
        add_materials(&mut app);
        app.finish();
        app.cleanup();

        let target = render_target(&mut app);
        let world = app.world_mut();
        world.spawn((
            Camera3d::default(),
            DepthPrepass,
            NormalPrepass,
            Camera {
                target: RenderTarget::Image(target.into()),
                ..default()
            },
            Transform::from_xyz(0.0, 1.8, 6.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        ));
        world.spawn((
            DirectionalLight {
                shadows_enabled: true,
                ..default()
            },
            Transform::from_rotation(mirrored_rotation(Vec3::new(45.0, 120.0, 0.0))),
        ));
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let rect = meshes.add(Rectangle::new(1.0, 1.0));
        let plane = meshes.add(Plane3d::default().mesh().size(4.0, 4.0));
        let sphere = meshes.add(
            Sphere::new(0.5)
                .mesh()
                .build()
                .with_generated_tangents()
                .unwrap(),
        );
        let dust = meshes.add(dust::mesh());
        let blob_shape = world.resource_mut::<Assets<Image>>().add(Image::default());

        let floor = world
            .resource_mut::<Assets<FloorMaterial>>()
            .add(FloorMaterial {
                base: StandardMaterial {
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                },
                extension: FloorExtension::default(),
            });
        world.spawn((Mesh3d(plane), MeshMaterial3d(floor)));
        let rock = world
            .resource_mut::<Assets<RockMaterial>>()
            .add(RockMaterial {
                base: StandardMaterial::default(),
                extension: RockExtension::default(),
            });
        world.spawn((Mesh3d(sphere), MeshMaterial3d(rock)));
        let mist = world
            .resource_mut::<Assets<MistMaterial>>()
            .add(MistMaterial {
                mist: MistUniform::default(),
            });
        world.spawn((Mesh3d(rect.clone()), MeshMaterial3d(mist)));
        let dust_material = world
            .resource_mut::<Assets<DustMaterial>>()
            .add(DustMaterial {
                dust: dust::uniform(LinearRgba::WHITE, LinearRgba::WHITE, 1.0),
            });
        world.spawn((
            Mesh3d(dust),
            MeshMaterial3d(dust_material),
            NoFrustumCulling,
        ));
        let blob = world
            .resource_mut::<Assets<BlobMaterial>>()
            .add(BlobMaterial {
                color: Vec4::ONE,
                shape: blob_shape,
            });
        world.spawn((Mesh3d(rect.clone()), MeshMaterial3d(blob)));
        let vignette = world
            .resource_mut::<Assets<VignetteMaterial>>()
            .add(VignetteMaterial {
                vignette: VignetteUniform::default(),
            });
        world.spawn((Mesh3d(rect), MeshMaterial3d(vignette), NoFrustumCulling));

        let compiled = compile_pipelines(&mut app);
        assert!(compiled >= 6, "only {compiled} pipelines compiled");
    }

    #[test]
    fn stage_assets_load() {
        use bevy::{
            asset::{AssetPlugin, LoadState},
            gltf::{Gltf, GltfPlugin},
            scene::ScenePlugin,
        };
        use std::time::{Duration, Instant};

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            assets::EmbedAssetsPlugin,
            ScenePlugin,
            GltfPlugin::default(),
            bevy::render::texture::ImagePlugin::default(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::render::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<AnimationClip>();
        app.finish();
        app.cleanup();

        let server = app.world().resource::<AssetServer>().clone();
        let mut models: Vec<String> = presets::PRESETS
            .iter()
            .flat_map(|p| p.props.iter().map(|prop| prop.model.to_owned()))
            .collect();
        models.push("podium".to_owned());
        models.sort();
        models.dedup();
        let gltfs: Vec<(String, Handle<Gltf>)> = models
            .iter()
            .map(|m| {
                (
                    m.clone(),
                    server.load(format!("embedded://lobby_stage/{m}.glb")),
                )
            })
            .collect();
        let mut images: Vec<Handle<Image>> = presets::PRESETS
            .iter()
            .map(|p| server.load(p.backdrop))
            .collect();
        images.push(server.load("embedded://lobby_stage/ground.jpg"));
        images.push(server.load("embedded://lobby_stage/blob_shadow.png"));

        let started = Instant::now();
        loop {
            app.update();
            let states: Vec<LoadState> = gltfs
                .iter()
                .map(|(_, h)| server.load_state(h))
                .chain(images.iter().map(|h| server.load_state(h)))
                .collect();
            if let Some(failed) = states.iter().find(|s| s.is_failed()) {
                panic!("{failed:?}");
            }
            if states.iter().all(LoadState::is_loaded) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "loading timed out"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let loaded = app.world().resource::<Assets<Gltf>>();
        for (model, handle) in &gltfs {
            let gltf = loaded.get(handle).unwrap();
            assert!(gltf.default_scene.is_some(), "{model}");
            assert_eq!(gltf.materials.len(), 1, "{model}");
        }
    }
}
