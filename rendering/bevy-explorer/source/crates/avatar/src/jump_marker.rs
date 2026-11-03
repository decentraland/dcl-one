//! The jump landing marker: a soft dark blob on the ground under the player while airborne,
//! showing where they will land. It follows
//! `unity:Explorer/Assets/DCL/JumpIndicator/JumpIndicatorSystem.cs`, which enables a decal
//! projector (`JumpIndicator.prefab`, 2 x 1.5 m, 10 m deep) under the avatar and feeds its
//! material the distance to the ground; `BlobShadow.mat` selects the black blob variant of
//! `unity:Explorer/Assets/DCL/JumpIndicator/Jump Indicator.shadergraph`, whose ramps are in
//! jump_marker.wgsl. Divergence: a ground-aligned quad instead of a decal projector, because
//! bevy's forward decal needs the depth prepass the default camera no longer has. Only the
//! primary player gets one.

use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderDefVal, ShaderRef,
            SpecializedMeshPipelineError,
        },
    },
};
use common::{
    sets::{SceneSets, SetupSets},
    structs::{AvatarDynamicState, PrimaryUser},
};

/// the decal's footprint on the ground, in metres
/// (unity:Explorer/Assets/DCL/JumpIndicator/JumpIndicator.prefab, `m_Size`)
pub const MARKER_SIZE: Vec2 = Vec2::new(2.0, 1.5);
/// the blob is opaque inside the inner radius and fades out to the outer one, as fractions of
/// the footprint's half extent (unity:Explorer/Assets/DCL/JumpIndicator/BlobShadow.mat)
pub const INNER_RADIUS: f32 = 0.35;
pub const OUTER_RADIUS: f32 = 1.0;
/// the opacity range and the ground distances it follows: it rises over the first pair of
/// distances and falls over the second (BlobShadow.mat `_Opacity_Range`,
/// `_Opacity_Distance_Ranges`)
pub const OPACITY_RANGE: Vec2 = Vec2::new(0.0, 0.9);
pub const OPACITY_DISTANCES: Vec4 = Vec4::new(0.0, 1.0, 6.0, 10.0);
/// the blob's scale range and the distances it follows (BlobShadow.mat `_Height_Scaling_Range`,
/// `_Height_Scaling_Distance_Ranges`). The material's "small to large" scaling inverts the
/// shadergraph's ramp, which is one over the first pair of distances and falls over the second,
/// so the blob is small near the ground and reaches full size at the last distance
pub const HEIGHT_SCALE_RANGE: Vec2 = Vec2::new(0.4, 1.0);
pub const HEIGHT_SCALE_DISTANCES: Vec4 = Vec4::new(0.0, 0.0, 4.0, 8.0);
/// the decal's depth, and the ground probe's range: farther from the ground there is no marker
pub const MAX_GROUND_DISTANCE: f32 = 10.0;
/// below this distance to the ground the player is standing, not airborne
const GROUNDED_EPSILON: f32 = 0.05;
/// the quad's lift above the ground, so it does not z-fight with it
const GROUND_LIFT: f32 = 0.02;

/// smoothstep over `[from, to]`; a degenerate range is a step at `from`
fn ramp(from: f32, to: f32, x: f32) -> f32 {
    if to <= from {
        return (x >= from) as u32 as f32;
    }
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// the shadergraph's two ramps over `distances`: one over the first pair, back to zero over the
/// second
fn rise_and_fall(distances: Vec4, ground_distance: f32) -> f32 {
    ramp(distances.x, distances.y, ground_distance)
        - ramp(distances.z, distances.w, ground_distance)
}

/// Opacity of the blob for a ground distance in metres (the shadergraph's two four-point ramps:
/// rises over the first pair, falls over the second).
pub fn marker_opacity(ground_distance: f32) -> f32 {
    OPACITY_RANGE.x
        + (OPACITY_RANGE.y - OPACITY_RANGE.x) * rise_and_fall(OPACITY_DISTANCES, ground_distance)
}

/// Scale of the blob for a ground distance in metres: small near the ground, growing over the
/// second pair of distances.
pub fn marker_scale(ground_distance: f32) -> f32 {
    // the shadergraph's "small to large" branch: one minus the ramp
    HEIGHT_SCALE_RANGE.x
        + (HEIGHT_SCALE_RANGE.y - HEIGHT_SCALE_RANGE.x)
            * (1.0 - rise_and_fall(HEIGHT_SCALE_DISTANCES, ground_distance))
}

/// the quad under the player; a child of it
#[derive(Component)]
pub struct JumpMarker;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct JumpMarkerMaterial {
    /// the distance from the player down to the ground, in metres
    #[uniform(0)]
    pub ground_distance: f32,
}

impl Material for JumpMarkerMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://shaders/jump_marker.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        1.0
    }

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(shader_defs());
        }
        Ok(())
    }
}

/// the constants jump_marker.wgsl reads, as `bitcast<f32>(#{NAME}u)`: a vector is `NAME_0` to
/// `NAME_3`
fn shader_defs() -> Vec<ShaderDefVal> {
    fn float(name: &str, value: f32) -> ShaderDefVal {
        ShaderDefVal::UInt(name.to_owned(), value.to_bits())
    }
    let mut defs = vec![
        float("INNER_RADIUS", INNER_RADIUS),
        float("OUTER_RADIUS", OUTER_RADIUS),
    ];
    for (name, values) in [
        ("OPACITY_RANGE", OPACITY_RANGE.to_array().to_vec()),
        ("OPACITY_DISTANCES", OPACITY_DISTANCES.to_array().to_vec()),
        ("HEIGHT_SCALE_RANGE", HEIGHT_SCALE_RANGE.to_array().to_vec()),
        (
            "HEIGHT_SCALE_DISTANCES",
            HEIGHT_SCALE_DISTANCES.to_array().to_vec(),
        ),
    ] {
        for (i, value) in values.into_iter().enumerate() {
            defs.push(float(&format!("{name}_{i}"), value));
        }
    }
    defs
}

pub struct JumpMarkerPlugin;

impl Plugin for JumpMarkerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<JumpMarkerMaterial>::default());
        app.add_systems(Startup, spawn_jump_marker.in_set(SetupSets::Main));
        app.add_systems(Update, update_jump_marker.in_set(SceneSets::PostLoop));
    }
}

fn spawn_jump_marker(
    mut commands: Commands,
    player: Single<Entity, With<PrimaryUser>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<JumpMarkerMaterial>>,
) {
    commands.spawn((
        JumpMarker,
        Mesh3d(meshes.add(Rectangle::new(MARKER_SIZE.x, MARKER_SIZE.y))),
        MeshMaterial3d(materials.add(JumpMarkerMaterial {
            ground_distance: 0.0,
        })),
        // lies flat, facing up
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        ChildOf(*player),
    ));
}

/// Places the quad on the ground under the player and shows it while the player is airborne.
/// `ground_height` is the world height of the ground under the player, so the distance to it is
/// the player's height above it; standing, they are level.
#[allow(clippy::type_complexity)]
fn update_jump_marker(
    player: Single<(&Transform, &AvatarDynamicState), With<PrimaryUser>>,
    marker: Single<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<JumpMarkerMaterial>,
        ),
        (With<JumpMarker>, Without<PrimaryUser>),
    >,
    mut materials: ResMut<Assets<JumpMarkerMaterial>>,
) {
    let (player_transform, state) = player.into_inner();
    let (mut transform, mut visibility, material) = marker.into_inner();

    let ground_distance = player_transform.translation.y - state.ground_height;
    let airborne = ground_distance > GROUNDED_EPSILON && ground_distance < MAX_GROUND_DISTANCE;
    visibility.set_if_neq(if airborne {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });
    if !airborne {
        return;
    }

    transform.translation.y = GROUND_LIFT - ground_distance;
    if materials
        .get(&material.0)
        .is_some_and(|m| m.ground_distance != ground_distance)
    {
        if let Some(m) = materials.get_mut(&material.0) {
            m.ground_distance = ground_distance;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_ramps_follow_the_material_ranges() {
        // opacity rises 0 to 0.9 over 0..1 m and falls to 0 over 6..10 m
        assert_eq!(marker_opacity(0.0), 0.0);
        assert!((marker_opacity(1.0) - 0.9).abs() < 1e-6);
        assert!((marker_opacity(3.0) - 0.9).abs() < 1e-6);
        assert_eq!(marker_opacity(10.0), 0.0);
        // the blob is small near the ground and grows to full size over 4..8 m
        assert!((marker_scale(0.5) - 0.4).abs() < 1e-6);
        assert!((marker_scale(4.0) - 0.4).abs() < 1e-6);
        assert!((marker_scale(8.0) - 1.0).abs() < 1e-6);
        assert!(marker_scale(6.0) > 0.4 && marker_scale(6.0) < 1.0);
    }

    fn test_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<JumpMarkerMaterial>();
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                Transform::from_xyz(0.0, 5.0, 0.0),
                AvatarDynamicState::default(),
            ))
            .id();
        app.add_systems(Startup, spawn_jump_marker);
        app.add_systems(Update, update_jump_marker);
        app.update();
        (app, player)
    }

    /// the marker's visibility, local height, material ground distance and parent
    fn marker_state(app: &mut App) -> (Visibility, f32, f32, Entity) {
        let world = app.world_mut();
        let (visibility, y, material, parent) = {
            let (visibility, transform, material, child_of) = world
                .query_filtered::<(
                    &Visibility,
                    &Transform,
                    &MeshMaterial3d<JumpMarkerMaterial>,
                    &ChildOf,
                ), With<JumpMarker>>()
                .single(world)
                .unwrap();
            (
                *visibility,
                transform.translation.y,
                material.0.clone(),
                child_of.parent(),
            )
        };
        let distance = world
            .resource::<Assets<JumpMarkerMaterial>>()
            .get(&material)
            .unwrap()
            .ground_distance;
        (visibility, y, distance, parent)
    }

    #[test]
    fn marker_shows_under_an_airborne_player_only() {
        let (mut app, player) = test_app();

        // standing: the ground is under the player's feet
        app.world_mut()
            .get_mut::<AvatarDynamicState>(player)
            .unwrap()
            .ground_height = 5.0;
        app.update();
        let (visibility, _, _, parent) = marker_state(&mut app);
        assert_eq!(visibility, Visibility::Hidden);
        assert_eq!(parent, player, "the marker is a child of the player");

        // airborne, the ground 3 m below
        app.world_mut()
            .get_mut::<AvatarDynamicState>(player)
            .unwrap()
            .ground_height = 2.0;
        app.update();
        let (visibility, y, distance, _) = marker_state(&mut app);
        assert_eq!(visibility, Visibility::Visible);
        assert_eq!(distance, 3.0);
        // local to the player: 3 m down, lifted off the ground by 2 cm
        assert!((y - (-3.0 + GROUND_LIFT)).abs() < 1e-6, "{y}");

        // landed
        app.world_mut()
            .get_mut::<AvatarDynamicState>(player)
            .unwrap()
            .ground_height = 5.0;
        app.update();
        assert_eq!(marker_state(&mut app).0, Visibility::Hidden);
    }

    /// The marker's shader compiles on this machine's adapter, in a renderer with no window: the
    /// quad in front of a camera. Needs a GPU, so `#[ignore]`d and skipped by CI:
    ///
    /// ```text
    /// cargo test -p avatar jump_marker_pipeline_compiles -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn jump_marker_pipeline_compiles() {
        use bevy::{
            asset::RenderAssetUsages,
            image::Image,
            log::LogPlugin,
            render::{
                camera::RenderTarget,
                pipelined_rendering::PipelinedRenderingPlugin,
                render_resource::{
                    CachedPipelineState, Extent3d, PipelineCache, TextureDimension, TextureFormat,
                    TextureUsages,
                },
                RenderApp,
            },
            window::ExitCondition,
            winit::WinitPlugin,
        };
        use std::time::{Duration, Instant};

        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<LogPlugin>()
                .disable::<WinitPlugin>()
                // keeps the render app in this app, so the pipeline cache is readable
                .disable::<PipelinedRenderingPlugin>()
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    close_when_requested: false,
                }),
        );
        app.add_plugins((assets::EmbedAssetsPlugin, JumpMarkerPlugin));
        app.finish();
        app.cleanup();

        let world = app.world_mut();
        let mut target = Image::new_fill(
            Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Bgra8UnormSrgb,
            RenderAssetUsages::all(),
        );
        target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        let target = world.resource_mut::<Assets<Image>>().add(target);
        world.spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(target.into()),
                ..default()
            },
            Transform::from_xyz(0.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));
        // a visible marker, as update_jump_marker leaves it while airborne
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(MARKER_SIZE.x, MARKER_SIZE.y));
        let material = world
            .resource_mut::<Assets<JumpMarkerMaterial>>()
            .add(JumpMarkerMaterial {
                ground_distance: 3.0,
            });
        world.spawn((
            JumpMarker,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::Visible,
        ));

        // a few frames queue the pipeline; then it compiles in the background
        for _ in 0..10 {
            app.update();
        }
        let started = Instant::now();
        let mut ok;
        let mut errors = Vec::new();
        loop {
            app.update();
            let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
            ok = 0;
            errors.clear();
            let mut pending = 0;
            for pipeline in cache.pipelines() {
                match &pipeline.state {
                    CachedPipelineState::Ok(_) => ok += 1,
                    CachedPipelineState::Err(error) => errors.push(error.to_string()),
                    _ => pending += 1,
                }
            }
            if pending == 0 {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "{pending} pipelines still compiling"
            );
        }
        println!("{ok} pipelines compiled");
        assert!(
            errors.is_empty(),
            "pipeline errors:
{}",
            errors.join(
                "

"
            )
        );
        assert!(ok >= 1, "no pipeline compiled");
    }
}
