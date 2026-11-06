//! The stage the avatar stands on: the light rig, matched to the Explorer's lobby preview (its
//! noon sun and the ambient its camera layer overrides to), and a shadow catcher floor that
//! shows only the avatar's shadow, so a transparent canvas still grounds it on the host page.

use bevy::{
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
};

use crate::host::OrbitCamera;

/// The light rig and the floor shadow; the page tunes it with the `light` command.
#[derive(Resource, Debug, Clone)]
pub struct StageLight {
    /// the key light, lux (the Explorer's noon sun is about 7000)
    pub illuminance: f32,
    /// ambient brightness (the lobby preview's layer override comes to 2000)
    pub ambient: f32,
    /// the key light's direction relative to the camera: yaw to the side, pitch down, radians
    pub yaw: f32,
    pub pitch: f32,
    /// how dark the floor shadow gets, 0 for none
    pub shadow: f32,
}

impl Default for StageLight {
    fn default() -> Self {
        Self {
            illuminance: 7000.0,
            ambient: 2000.0,
            // from the camera's right, fairly high: the shadow falls behind to the left, as in
            // wearable-preview's Unity renderer (compared with web/compare.html)
            yaw: 0.6,
            pitch: 0.65,
            shadow: 0.55,
        }
    }
}

#[derive(Component)]
pub struct KeyLight;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ShadowCatcherMaterial {
    /// x: the darkest the shadow gets (0..1), y: metres from the centre where the floor fades out
    #[uniform(0)]
    pub params: Vec4,
}

impl Material for ShadowCatcherMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://dcl_avatar_preview/shadow_catcher.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shadow_catcher.wgsl");
        app.add_plugins(MaterialPlugin::<ShadowCatcherMaterial>::default())
            .init_resource::<StageLight>()
            .insert_resource(bevy::pbr::DirectionalLightShadowMap { size: 2048 })
            .add_systems(Startup, spawn_stage)
            .add_systems(Update, apply_light);
    }
}

fn spawn_stage(
    mut commands: Commands,
    light: Res<StageLight>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut catchers: ResMut<Assets<ShadowCatcherMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: light.illuminance,
            shadows_enabled: true,
            ..default()
        },
        // one tight cascade around the avatar: its shadow at full map resolution
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.1,
            maximum_distance: 14.0,
            first_cascade_far_bound: 14.0,
            overlap_proportion: 0.0,
        }
        .build(),
        Transform::default(),
        KeyLight,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4.0, 4.0))),
        MeshMaterial3d(catchers.add(ShadowCatcherMaterial {
            params: Vec4::new(light.shadow, 2.0, 0.0, 0.0),
        })),
        Transform::from_xyz(0.0, 0.002, 0.0),
        NotShadowCaster,
        ShadowCatcher,
    ));
}

#[derive(Component)]
pub struct ShadowCatcher;

/// The key light keeps its place relative to the camera as it orbits, so the avatar stays lit
/// from the front whichever side the camera is on.
fn apply_light(
    light: Res<StageLight>,
    orbit: Res<OrbitCamera>,
    mut keys: Query<(&mut DirectionalLight, &mut Transform), With<KeyLight>>,
    mut ambient: ResMut<AmbientLight>,
    catchers: Query<&MeshMaterial3d<ShadowCatcherMaterial>, With<ShadowCatcher>>,
    mut materials: ResMut<Assets<ShadowCatcherMaterial>>,
) {
    for (mut directional, mut transform) in keys.iter_mut() {
        directional.illuminance = light.illuminance;
        // the light shines along -Z: turn it to face the avatar from the camera's side
        transform.rotation =
            Quat::from_rotation_y(orbit.alpha + light.yaw) * Quat::from_rotation_x(-light.pitch);
    }
    if light.is_changed() {
        ambient.brightness = light.ambient;
        for handle in catchers.iter() {
            if let Some(material) = materials.get_mut(&handle.0) {
                material.params.x = light.shadow;
            }
        }
    }
}
