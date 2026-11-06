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
    /// metres from the feet where the floor shadow has faded out
    pub shadow_reach: f32,
    /// exposure in stops above bevy's default (EV100 9.7): the toon shading ignores ambient
    /// light and scales with exposure, so this is what brightens or darkens the avatar
    pub exposure: f32,
    /// the floor glow around the feet, as a factor of the Unity renderer's (0 for none)
    pub glow: f32,
}

impl Default for StageLight {
    fn default() -> Self {
        Self {
            illuminance: 8000.0,
            ambient: 2000.0,
            // from the camera's right, fairly high: the shadow falls behind to the left, as in
            // wearable-preview's Unity renderer (compared with web/compare.html)
            yaw: 0.6,
            pitch: 0.65,
            shadow: 0.4,
            shadow_reach: 4.0,
            exposure: -0.4,
            glow: 1.0,
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
    /// xy: the world xz the shadow and glow centre on
    #[uniform(1)]
    pub centre: Vec4,
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
            .add_systems(Update, (apply_light, place_floors));
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
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
        MeshMaterial3d(catchers.add(ShadowCatcherMaterial {
            params: Vec4::new(light.shadow, light.shadow_reach, light.glow, 2.5),
            centre: Vec4::ZERO,
        })),
        Transform::from_xyz(0.0, 0.002, 0.0),
        NotShadowCaster,
        ShadowCatcher { item: false },
    ));
    // the item view's floor: its glow alone (nothing casts onto it), under the item
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(6.0, 6.0))),
        MeshMaterial3d(catchers.add(ShadowCatcherMaterial {
            params: Vec4::new(0.0, light.shadow_reach, light.glow, 2.5),
            centre: Vec4::ZERO,
        })),
        Transform::from_xyz(0.0, 0.002, 0.0),
        Visibility::Hidden,
        NotShadowCaster,
        ShadowCatcher { item: true },
    ));
}

/// A floor: the avatar's (its shadow and the glow around its feet), or the item view's (a glow
/// under the item). Each shows in its own view, as the Unity renderer swaps them.
#[derive(Component)]
pub struct ShadowCatcher {
    item: bool,
}

/// The key light stands fixed in the world, in front of the subject as the camera is; the
/// subject turns under it.
fn apply_light(
    light: Res<StageLight>,
    mut keys: Query<(&mut DirectionalLight, &mut Transform), With<KeyLight>>,
    mut cameras: Query<&mut bevy::render::camera::Exposure, With<crate::host::PreviewCamera>>,
    mut ambient: ResMut<AmbientLight>,
    catchers: Query<(&MeshMaterial3d<ShadowCatcherMaterial>, &ShadowCatcher)>,
    mut materials: ResMut<Assets<ShadowCatcherMaterial>>,
) {
    for (mut directional, mut transform) in keys.iter_mut() {
        directional.illuminance = light.illuminance;
        // the light shines along -Z and stays put, as the camera does: the subject turns under it,
        // as in the Unity renderer
        transform.rotation = Quat::from_rotation_y(light.yaw) * Quat::from_rotation_x(-light.pitch);
    }
    if light.is_changed() {
        ambient.brightness = light.ambient;
        for mut exposure in cameras.iter_mut() {
            exposure.ev100 = bevy::render::camera::Exposure::EV100_BLENDER - light.exposure;
        }
        for (handle, catcher) in catchers.iter() {
            if let Some(material) = materials.get_mut(&handle.0) {
                material.params.x = if catcher.item { 0.0 } else { light.shadow };
                material.params.y = light.shadow_reach.max(0.1);
                material.params.z = light.glow.max(0.0);
            }
        }
    }
}

/// Shows the floor of the view on screen, the item's under the item, as the Unity renderer swaps
/// its avatar and item floors with the view.
fn place_floors(
    orbit: Res<OrbitCamera>,
    mut catchers: Query<(
        &ShadowCatcher,
        &MeshMaterial3d<ShadowCatcherMaterial>,
        &mut Transform,
        &mut Visibility,
    )>,
    mut materials: ResMut<Assets<ShadowCatcherMaterial>>,
) {
    if !orbit.is_changed() {
        return;
    }
    for (catcher, handle, mut transform, mut visibility) in catchers.iter_mut() {
        visibility.set_if_neq(if catcher.item == orbit.item_view {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if catcher.item {
            let at = Vec3::new(
                orbit.item_target.x,
                orbit.item_floor + 0.002,
                orbit.item_target.z,
            );
            if transform.translation != at {
                transform.translation = at;
                if let Some(material) = materials.get_mut(&handle.0) {
                    material.centre = Vec4::new(at.x, at.z, 0.0, 0.0);
                }
            }
        }
    }
}
