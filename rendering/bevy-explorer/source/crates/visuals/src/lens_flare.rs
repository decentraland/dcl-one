//! The lens flare of the directional light, the sun by day and the moon by night, added onto the
//! frame before bloom. The flare data and its intensity by time of day are the sky preset's; the
//! element placement, the random copies and the shapes follow the data-driven lens flare Unity
//! draws.
//!
//! Each element copy is a quad placed here, on the CPU, once per view and frame, and drawn as one
//! instanced draw (`lens_flare.wgsl`). The vertex stage tests how much of the light is hidden:
//! samples in a small disc around it pass where nothing opaque was drawn (the depth is the far
//! plane) and, where the view has depth of field's transparent focus texture, by the share no
//! transparent surface covers. So imposters, which draw no depth, hide the light too.
//! The test runs per vertex, not per pixel.

use bevy::{
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        dof::TransparentFocusTexture,
    },
    ecs::{
        query::{Has, QueryItem},
        system::lifetimeless::Read,
    },
    image::BevyDefault as _,
    platform::collections::HashMap,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{
            NodeRunError, RenderGraph, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode,
            ViewNodeRunner,
        },
        render_resource::{
            binding_types::{
                texture_2d, texture_2d_multisampled, texture_depth_2d,
                texture_depth_2d_multisampled, uniform_buffer,
            },
            BindGroupEntries, BindGroupLayout, BlendComponent, BlendFactor, BlendOperation,
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            DynamicUniformBuffer, FragmentState, LoadOp, MultisampleState, Operations,
            PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderDefVal, ShaderStages, ShaderType,
            SpecializedRenderPipeline, SpecializedRenderPipelines, StoreOp, TextureFormat,
            TextureSampleType, TextureUsages, VertexState,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget},
        Render, RenderApp, RenderSet,
    },
};
use common::{
    sky_preset::{day_fraction, sample_keys, LENS_FLARE_INTENSITY_BY_TIME},
    structs::{LensFlare, SceneGlobalLight, TimeOfDay},
};
use scene_runner::update_world::lights::celestial_swap;

/// The shape of a flare element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlareKind {
    Circle,
    /// a regular polygon with `sides` corners, rounded by `roundness`
    Polygon {
        sides: u32,
        roundness: f32,
    },
    /// a thin band of colours, or with `partial` only a red and violet arc on one side, drawn
    /// procedurally in the colours of the ring images Unity draws
    Ring {
        partial: bool,
    },
}

/// One element of a flare.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlareElement {
    pub kind: FlareKind,
    pub intensity: f32,
    /// where along the ray from the light through the screen centre: 0 at the light, 0.5 at the
    /// centre, 1 at the light mirrored through it
    pub position: f32,
    /// scales `size_xy`
    pub scale: f32,
    pub size_xy: Vec2,
    /// the additive blend ignores its alpha
    pub tint: Vec4,
    /// 1 keeps the shape's edge soft over its whole radius, lower values sharpen it
    pub falloff: f32,
    /// where the shape starts fading, from its centre (0) to its rim (1)
    pub edge_offset: f32,
    /// copies of the element, spread along the ray
    pub count: u32,
    /// how far along the ray the copies spread
    pub spread: f32,
    /// degrees
    pub rotation: f32,
    /// turned to face along the ray
    pub auto_rotate: bool,
    /// tinted by the light's colour
    pub modulate_by_light: bool,
    /// how much the copies' sizes vary
    pub scale_variation: f32,
    /// how much the copies' rotations vary
    pub rotation_variation: f32,
    /// the size `size_xy` grows toward as the element moves from the screen's centre to its edge
    pub radial_distortion: Option<Vec2>,
}

/// The day flare: a warm glow on the sun, hexagonal ghosts, small streaks and two rings.
pub static DAY_FLARE: [FlareElement; 5] = [
    FlareElement {
        kind: FlareKind::Circle,
        intensity: 10.0,
        position: 0.0,
        scale: 5.0,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(1.0, 0.5914119, 0.1372549, 1.0),
        falloff: 0.5,
        edge_offset: 1.0,
        count: 1,
        spread: 1.0,
        rotation: 0.0,
        auto_rotate: false,
        modulate_by_light: true,
        scale_variation: 1.0,
        rotation_variation: 180.0,
        radial_distortion: None,
    },
    FlareElement {
        kind: FlareKind::Polygon {
            sides: 6,
            roundness: 0.174,
        },
        intensity: 1.0,
        position: 0.4,
        scale: 3.56,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(0.028301895, 0.028301895, 0.028301895, 1.0),
        falloff: 0.95,
        edge_offset: 0.535,
        count: 10,
        spread: 0.5,
        rotation: 0.0,
        auto_rotate: false,
        modulate_by_light: true,
        scale_variation: 0.9,
        rotation_variation: 360.0,
        radial_distortion: None,
    },
    FlareElement {
        kind: FlareKind::Circle,
        intensity: 1.0,
        position: 0.1,
        scale: 0.9,
        size_xy: Vec2::new(2.0, 1.0),
        tint: Vec4::new(0.0754717, 0.0754717, 0.0754717, 0.5),
        falloff: 0.799,
        edge_offset: 0.91,
        count: 12,
        spread: 0.2,
        rotation: 0.0,
        auto_rotate: true,
        modulate_by_light: true,
        scale_variation: 1.0,
        rotation_variation: 0.0,
        radial_distortion: None,
    },
    FlareElement {
        kind: FlareKind::Ring { partial: false },
        intensity: 0.44,
        position: 0.35,
        scale: 4.2,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(1.0, 1.0, 1.0, 0.5),
        falloff: 1.0,
        edge_offset: 0.1,
        count: 1,
        spread: 1.0,
        rotation: 0.0,
        auto_rotate: false,
        modulate_by_light: false,
        scale_variation: 1.0,
        rotation_variation: 180.0,
        radial_distortion: Some(Vec2::new(20.0, 20.0)),
    },
    FlareElement {
        kind: FlareKind::Ring { partial: true },
        intensity: 1.0,
        position: 0.33,
        scale: 3.93,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(1.0, 1.0, 1.0, 0.5),
        falloff: 1.0,
        edge_offset: 0.1,
        count: 1,
        spread: 1.0,
        rotation: 0.14,
        auto_rotate: true,
        modulate_by_light: false,
        scale_variation: 1.0,
        rotation_variation: 180.0,
        radial_distortion: Some(Vec2::new(1.0, 1.0)),
    },
];

/// The night flare: a blue glow on the moon, pentagonal ghosts, small streaks and a green arc.
pub static NIGHT_FLARE: [FlareElement; 4] = [
    FlareElement {
        kind: FlareKind::Circle,
        intensity: 2.72,
        position: 0.0,
        scale: 5.0,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(0.0, 0.22392893, 1.0, 1.0),
        falloff: 0.5,
        edge_offset: 1.0,
        count: 1,
        spread: 1.0,
        rotation: 0.0,
        auto_rotate: false,
        modulate_by_light: true,
        scale_variation: 1.0,
        rotation_variation: 180.0,
        radial_distortion: None,
    },
    FlareElement {
        kind: FlareKind::Polygon {
            sides: 5,
            roundness: 0.76,
        },
        intensity: 1.0,
        position: 0.4,
        scale: 3.36,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(0.028301895, 0.028301895, 0.028301895, 1.0),
        falloff: 0.941,
        edge_offset: 0.691,
        count: 10,
        spread: 0.5,
        rotation: 0.0,
        auto_rotate: false,
        modulate_by_light: true,
        scale_variation: 0.9,
        rotation_variation: 360.0,
        radial_distortion: None,
    },
    FlareElement {
        kind: FlareKind::Circle,
        intensity: 0.4,
        position: 0.1,
        scale: 0.9,
        size_xy: Vec2::new(2.0, 1.0),
        tint: Vec4::new(0.0754717, 0.0754717, 0.0754717, 0.5),
        falloff: 0.682,
        edge_offset: 0.507,
        count: 12,
        spread: 0.2,
        rotation: 0.0,
        auto_rotate: true,
        modulate_by_light: true,
        scale_variation: 1.0,
        rotation_variation: 0.0,
        radial_distortion: None,
    },
    FlareElement {
        kind: FlareKind::Ring { partial: true },
        intensity: 0.76,
        position: 0.33,
        scale: 3.93,
        size_xy: Vec2::new(1.0, 1.0),
        tint: Vec4::new(0.073584795, 1.0, 0.4108428, 0.5),
        falloff: 1.0,
        edge_offset: 0.1,
        count: 1,
        spread: 1.0,
        rotation: 0.14,
        auto_rotate: true,
        modulate_by_light: false,
        scale_variation: 1.0,
        rotation_variation: 180.0,
        radial_distortion: Some(Vec2::new(1.0, 1.0)),
    },
];

/// The day fractions at which the day flare and the night flare take over.
const DAY_FLARE_START: f32 = 0.2;
const NIGHT_FLARE_START: f32 = 0.8;

/// The most quads a flare draws, its elements' copies summed; the uniform holds this many.
const MAX_QUADS: usize = 32;
const fn quad_count(flare: &[FlareElement]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < flare.len() {
        count += flare[i].count as usize;
        i += 1;
    }
    count
}
const _: () = assert!(quad_count(&DAY_FLARE) <= MAX_QUADS && quad_count(&NIGHT_FLARE) <= MAX_QUADS);

/// How much the copies' intensities and their steps along the ray vary.
const INTENSITY_VARIATION: f32 = 0.75;
const POSITION_VARIATION: f32 = 1.0;

/// The copies' tint by a random value: (value, colour) keys, blended linearly.
const RANDOM_TINT: [(f32, Vec3); 3] = [
    (0.0, Vec3::new(1.0, 1.0, 1.0)),
    (52043.0 / 65535.0, Vec3::new(1.0, 0.6600066, 0.3537736)),
    (65342.0 / 65535.0, Vec3::new(1.0, 0.33182937, 0.1462264)),
];

/// An element's half size on screen, in half screen heights, is `size_xy * scale` times this.
const SCALE_SIZE: f32 = 0.1;

/// The occlusion test samples a disc around the light whose radius is this share of the
/// projected size of a body `CELESTIAL_ANGULAR_RADIUS_DEGREES` across.
const OCCLUSION_SAMPLES: u32 = 32;
const OCCLUSION_RADIUS: f32 = 0.1;
const CELESTIAL_ANGULAR_RADIUS_DEGREES: f32 = 3.3;

/// The flare in use at `day_fraction`.
pub fn flare_set(day_fraction: f32) -> &'static [FlareElement] {
    if (DAY_FLARE_START..NIGHT_FLARE_START).contains(&day_fraction) {
        &DAY_FLARE
    } else {
        &NIGHT_FLARE
    }
}

/// The flare's intensity at `day_fraction`, dimmed while the light swaps between the sun and the
/// moon (`swap_dip` 1 at the middle of the swap).
pub fn flare_intensity(day_fraction: f32, swap_dip: f32) -> f32 {
    sample_keys(&LENS_FLARE_INTENSITY_BY_TIME, day_fraction) * (1.0 - swap_dip)
}

pub struct LensFlarePlugin;

impl Plugin for LensFlarePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LensFlareSettings>()
            .add_plugins((
                ExtractResourcePlugin::<LensFlareSettings>::default(),
                ExtractComponentPlugin::<LensFlareView>::default(),
            ))
            .add_systems(Update, (update_lens_flare_settings, bind_depth_texture));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<SpecializedRenderPipelines<LensFlarePipeline>>()
            .init_resource::<LensFlareUniforms>()
            .add_systems(
                Render,
                (
                    prepare_lens_flare_pipelines.in_set(RenderSet::Prepare),
                    prepare_lens_flare_uniforms.in_set(RenderSet::PrepareResources),
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<LensFlareNode>>(Core3d, LensFlareLabel)
            // before bloom, so the flare blooms; bevy blooms before depth of field, so the flare
            // comes straight after the main pass
            .add_render_graph_edges(Core3d, (Node3d::EndMainPass, LensFlareLabel, Node3d::Bloom));
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<LensFlarePipeline>();
        // bevy orders the wireframe pass only between the main pass and post processing. Under
        // MSAA it draws into the multisampled frame and resolves it again, which overwrites
        // whatever was drawn onto the resolved frame before it, so it goes ahead of the flare
        let has_wireframe = render_app
            .world()
            .resource::<RenderGraph>()
            .get_sub_graph(Core3d)
            .is_some_and(|graph| graph.get_node_state(Node3d::Wireframe).is_ok());
        if has_wireframe {
            render_app.add_render_graph_edge(Core3d, Node3d::Wireframe, LensFlareLabel);
        }
    }
}

/// The flare as the directional light and the time of day set it.
#[derive(Resource, Clone, ExtractResource)]
pub(crate) struct LensFlareSettings {
    /// direction to the light's body
    pub(crate) toward_light: Vec3,
    /// the light's colour, for the elements it tints, as its sRGB components
    pub(crate) light_color: Vec3,
    pub(crate) intensity: f32,
    pub(crate) elements: &'static [FlareElement],
}

impl Default for LensFlareSettings {
    fn default() -> Self {
        Self {
            toward_light: Vec3::Y,
            light_color: Vec3::ONE,
            intensity: 0.0,
            elements: &DAY_FLARE,
        }
    }
}

fn update_lens_flare_settings(
    light: Res<SceneGlobalLight>,
    time: Res<TimeOfDay>,
    mut settings: ResMut<LensFlareSettings>,
) {
    let t = day_fraction(time.elapsed_secs() / 3600.0);
    let color = light.dir_color.to_srgba();
    *settings = LensFlareSettings {
        // the light travels away from its body: the sun by day, the moon by night
        toward_light: -light.dir_direction,
        light_color: Vec3::new(color.red, color.green, color.blue),
        // dimmed with the light while it swings between the sun and the moon; none while a scene
        // hides the sun and moon discs
        intensity: if light.sun_hidden {
            0.0
        } else {
            flare_intensity(t, celestial_swap(t).1)
        },
        elements: flare_set(t),
    };
}

/// The flare reads the view's depth texture, which bevy only makes bindable on request.
fn bind_depth_texture(mut cameras: Query<&mut Camera3d, With<LensFlare>>) {
    for mut camera in cameras.iter_mut() {
        let usages = TextureUsages::from(camera.depth_texture_usages);
        if !usages.contains(TextureUsages::TEXTURE_BINDING) {
            camera.depth_texture_usages = (usages | TextureUsages::TEXTURE_BINDING).into();
        }
    }
}

/// A view that draws the lens flare.
#[derive(Component, Clone)]
struct LensFlareView;

// Every camera is extracted, with or without `LensFlare`: returning `None` is what removes the
// marker from a render-world view whose camera lost `LensFlare`. Filtering on `With<LensFlare>`
// would skip that camera and leave the flare drawing after the setting is turned off.
impl ExtractComponent for LensFlareView {
    type QueryData = Has<LensFlare>;
    type QueryFilter = With<Camera3d>;
    type Out = Self;

    fn extract_component(flare: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        flare.then_some(Self)
    }
}

/// [`FlareKind`] as the shader tells them apart.
const KIND_CIRCLE: u32 = 0;
const KIND_POLYGON: u32 = 1;
const KIND_RING: u32 = 2;
const KIND_PARTIAL_RING: u32 = 3;

pub(crate) use gpu::{FlareQuad, LensFlareUniform};

mod gpu {
    // the ShaderType derive's layout check function is never called
    #![allow(dead_code)]

    use super::*;

    /// One element copy as `lens_flare.wgsl` draws it.
    #[derive(Clone, Copy, Default, Debug, ShaderType)]
    pub struct FlareQuad {
        /// the quad's centre on screen, x right and y down, -1 to 1
        pub centre: Vec2,
        /// half the quad's size, in half screen heights
        pub size: Vec2,
        /// cosine and sine of the angle that turns the quad
        pub turn: Vec2,
        /// where the shape starts fading, in the shape's own units
        pub edge: f32,
        /// the exponent of the fade
        pub falloff: f32,
        /// the light it adds, with the flare's intensity
        pub color: Vec3,
        pub roundness: f32,
        pub kind: u32,
        pub sides: u32,
    }

    /// The flare of one view.
    #[derive(Clone, ShaderType)]
    pub struct LensFlareUniform {
        /// the light's position on screen, x right and y down, -1 to 1
        pub sun: Vec2,
        /// the radius of the occlusion test's disc, in the same units
        pub occlusion_radius: f32,
        /// the view's width over its height
        pub aspect: f32,
        pub count: u32,
        pub quads: [FlareQuad; MAX_QUADS],
    }
}

/// A random value in [0, 1) for `seed`, the same for every view and frame.
fn hash(seed: u32) -> f32 {
    let mut s = seed ^ 2_747_636_419;
    s = s.wrapping_mul(2_654_435_769);
    s ^= s >> 16;
    s = s.wrapping_mul(2_654_435_769);
    s ^= s >> 16;
    s = s.wrapping_mul(2_654_435_769);
    s as f32 / 4_294_967_296.0
}

/// A copy's tint for the random value `t`.
fn random_tint(t: f32) -> Vec3 {
    let [(t0, c0), (t1, c1), (t2, c2)] = RANDOM_TINT;
    if t <= t0 {
        c0
    } else if t <= t1 {
        c0.lerp(c1, (t - t0) / (t1 - t0))
    } else if t <= t2 {
        c1.lerp(c2, (t - t1) / (t2 - t1))
    } else {
        c2
    }
}

/// The quads of `elements` for a light at `sun` (x right, y down) on a view `aspect` wide, each
/// with `intensity` folded into its colour.
fn flare_quads(
    elements: &[FlareElement],
    sun: Vec2,
    aspect: f32,
    light_color: Vec3,
    intensity: f32,
) -> Vec<FlareQuad> {
    let mut quads = Vec::with_capacity(MAX_QUADS);
    for element in elements {
        let (kind, sides, roundness) = match element.kind {
            FlareKind::Circle => (KIND_CIRCLE, 0, 0.0),
            FlareKind::Polygon { sides, roundness } => (KIND_POLYGON, sides, roundness),
            FlareKind::Ring { partial: false } => (KIND_RING, 0, 0.0),
            FlareKind::Ring { partial: true } => (KIND_PARTIAL_RING, 0, 0.0),
        };
        let mut color = element.tint.truncate() * element.intensity * intensity;
        if element.modulate_by_light {
            color *= light_color;
        }
        let falloff = (4.0 * (1.0 - element.falloff).clamp(0.0, 1.0)).exp();
        let mut edge = ((1.0 - element.edge_offset) - 1e-6).clamp(0.0, 1.0);
        if kind == KIND_POLYGON {
            edge = (edge + 1.0).powi(5);
        }
        let size = element.size_xy * SCALE_SIZE * element.scale;
        // toward its distortion target the further the element sits from the screen's centre
        let distorted = |position: f32| match element.radial_distortion {
            Some(target) => {
                let centre = sun * (1.0 - position);
                size.lerp(
                    target * SCALE_SIZE * element.scale,
                    centre.length().clamp(0.0, 1.0),
                )
            }
            None => size,
        };

        let mut push = |position: f32, size: Vec2, rotation: f32, color: Vec3| {
            if size.x <= 0.0 || size.y <= 0.0 {
                return;
            }
            let ray = -sun * position;
            let mut angle = rotation + 180.0;
            if element.auto_rotate && ray.length() > 0.0 {
                let along = ray.normalize() * Vec2::new(aspect, 1.0);
                angle -= along.y.atan2(along.x).to_degrees();
            }
            let (sin, cos) = (-angle.to_radians()).sin_cos();
            quads.push(FlareQuad {
                centre: sun + ray,
                size,
                turn: Vec2::new(cos, sin),
                edge,
                falloff,
                color,
                roundness,
                kind,
                sides,
            });
        };

        if element.count <= 1 {
            let position = 2.0 * element.position;
            push(position, distorted(position), element.rotation, color);
            continue;
        }
        let step = 2.0 * element.spread / (element.count - 1) as f32;
        let mut position = 2.0 * element.position;
        for i in 0..element.count {
            let k = i * 6;
            let local_intensity = (hash(k) * 2.0 - 1.0) * INTENSITY_VARIATION + 1.0;
            let copy_size =
                distorted(position) * (1.0 + element.scale_variation * (hash(k + 1) * 2.0 - 1.0));
            let tint = random_tint(hash(k + 2)) * local_intensity;
            // k + 3 would move the copy sideways, by a variation these flares leave at 0
            let rotation = element.rotation
                + (hash(k + 4) * 2.0 - 1.0) * std::f32::consts::PI * element.rotation_variation;
            push(position, copy_size, rotation, color * tint);
            position += step + 0.5 * step * (hash(k + 5) * 2.0 - 1.0) * POSITION_VARIATION;
        }
    }
    quads
}

/// Where the light shows on a view, or `None` where no flare is drawn: the light below the
/// horizon, behind the camera or off screen, or the flare at no intensity. Returns the light's
/// position (x right, y up, -1 to 1), the intensity after the light's angle from the view
/// direction, and the occlusion disc's radius.
pub(crate) fn light_on_screen(
    toward: Vec3,
    intensity: f32,
    clip_from_world: Mat4,
    camera_up: Vec3,
    camera_forward: Vec3,
) -> Option<(Vec2, f32, f32)> {
    let intensity = intensity * toward.dot(camera_forward).max(0.0);
    if intensity <= 0.0 || toward.y <= 0.0 {
        return None;
    }
    // the light is infinitely far: project its direction
    let project = |direction: Vec3| {
        let clip = clip_from_world * direction.extend(0.0);
        (clip.w > 0.0).then(|| clip.xy() / clip.w)
    };
    let ndc = project(toward)?;
    if ndc.abs().max_element() > 1.0 {
        return None;
    }
    let edge = project(
        toward + camera_up * OCCLUSION_RADIUS * CELESTIAL_ANGULAR_RADIUS_DEGREES.to_radians().tan(),
    )?;
    Some((ndc, intensity, 0.5 * (edge - ndc).length()))
}

/// The flare as a view sees it, or none where it draws none (see [`light_on_screen`]).
fn flare_uniform(
    settings: &LensFlareSettings,
    clip_from_world: Mat4,
    camera_up: Vec3,
    camera_forward: Vec3,
    aspect: f32,
) -> Option<LensFlareUniform> {
    let (ndc, intensity, occlusion_radius) = light_on_screen(
        settings.toward_light,
        settings.intensity,
        clip_from_world,
        camera_up,
        camera_forward,
    )?;
    let sun = Vec2::new(ndc.x, -ndc.y);
    let list = flare_quads(
        settings.elements,
        sun,
        aspect,
        settings.light_color,
        intensity,
    );
    let mut quads = [FlareQuad::default(); MAX_QUADS];
    quads[..list.len()].copy_from_slice(&list);
    Some(LensFlareUniform {
        sun,
        occlusion_radius,
        aspect,
        count: list.len() as u32,
        quads,
    })
}

#[derive(Resource, Default)]
struct LensFlareUniforms(DynamicUniformBuffer<LensFlareUniform>);

/// Where a view's flare sits in [`LensFlareUniforms`], and how many quads it draws; absent while
/// the view draws no flare.
#[derive(Component)]
struct LensFlareUniformOffset {
    offset: u32,
    count: u32,
}

fn prepare_lens_flare_uniforms(
    mut commands: Commands,
    settings: Res<LensFlareSettings>,
    views: Query<(Entity, &ExtractedView), With<LensFlareView>>,
    mut uniforms: ResMut<LensFlareUniforms>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    uniforms.0.clear();
    for (entity, view) in views.iter() {
        let clip_from_world = view.clip_from_world.unwrap_or_else(|| {
            view.clip_from_view * view.world_from_view.compute_matrix().inverse()
        });
        let size = view.viewport.zw().as_vec2();
        let flare = (size.y > 0.0)
            .then(|| {
                flare_uniform(
                    &settings,
                    clip_from_world,
                    *view.world_from_view.up(),
                    *view.world_from_view.forward(),
                    size.x / size.y,
                )
            })
            .flatten();
        match flare {
            Some(flare) => {
                let count = flare.count;
                let offset = uniforms.0.push(&flare);
                commands
                    .entity(entity)
                    .insert(LensFlareUniformOffset { offset, count });
            }
            None => {
                commands.entity(entity).remove::<LensFlareUniformOffset>();
            }
        }
    }
    uniforms.0.write_buffer(&device, &queue);
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
struct LensFlarePipelineKey {
    samples: u32,
    hdr: bool,
    /// the view has depth of field, and so a transparent focus texture to read coverage from
    focus: bool,
}

#[derive(Resource)]
struct LensFlarePipeline {
    shader: Handle<Shader>,
    /// by sample count and focus
    layouts: HashMap<(u32, bool), BindGroupLayout>,
}

impl FromWorld for LensFlarePipeline {
    fn from_world(world: &mut World) -> Self {
        let shader = world
            .resource::<AssetServer>()
            .load("embedded://shaders/lens_flare.wgsl");
        let device = world.resource::<RenderDevice>();

        let mut layouts = HashMap::default();
        for samples in [1, 2, 4, 8] {
            for focus in [false, true] {
                let depth = if samples > 1 {
                    texture_depth_2d_multisampled()
                } else {
                    texture_depth_2d()
                };
                let mut entries = vec![
                    depth.build(0, ShaderStages::VERTEX),
                    uniform_buffer::<LensFlareUniform>(true).build(1, ShaderStages::VERTEX),
                ];
                if focus {
                    let sample_type = TextureSampleType::Float { filterable: false };
                    let texture = if samples > 1 {
                        texture_2d_multisampled(sample_type)
                    } else {
                        texture_2d(sample_type)
                    };
                    entries.push(texture.build(2, ShaderStages::VERTEX));
                }
                let layout =
                    device.create_bind_group_layout(Some("lens flare bind group layout"), &entries);
                layouts.insert((samples, focus), layout);
            }
        }

        Self { shader, layouts }
    }
}

/// The numbers `lens_flare.wgsl` shares with this file.
fn shader_constants() -> Vec<ShaderDefVal> {
    vec![
        ShaderDefVal::UInt("MAX_QUADS".into(), MAX_QUADS as u32),
        ShaderDefVal::UInt("OCCLUSION_SAMPLES".into(), OCCLUSION_SAMPLES),
        ShaderDefVal::UInt("KIND_CIRCLE".into(), KIND_CIRCLE),
        ShaderDefVal::UInt("KIND_POLYGON".into(), KIND_POLYGON),
        ShaderDefVal::UInt("KIND_RING".into(), KIND_RING),
        ShaderDefVal::UInt("KIND_PARTIAL_RING".into(), KIND_PARTIAL_RING),
    ]
}

impl SpecializedRenderPipeline for LensFlarePipeline {
    type Key = LensFlarePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = shader_constants();
        if key.samples > 1 {
            shader_defs.push("MULTISAMPLED".into());
        }
        if key.focus {
            shader_defs.push("FOCUS".into());
        }
        RenderPipelineDescriptor {
            label: Some("lens flare pipeline".into()),
            layout: vec![self.layouts[&(key.samples, key.focus)].clone()],
            push_constant_ranges: Vec::new(),
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: "vertex".into(),
                buffers: Vec::new(),
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: if key.hdr {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    // the flare only adds light
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::Zero,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

#[derive(Component)]
struct LensFlarePipelineId {
    id: CachedRenderPipelineId,
    key: LensFlarePipelineKey,
}

fn prepare_lens_flare_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LensFlarePipeline>>,
    pipeline: Res<LensFlarePipeline>,
    views: Query<
        (Entity, &ExtractedView, &Msaa, Has<TransparentFocusTexture>),
        With<LensFlareView>,
    >,
) {
    for (entity, view, msaa, focus) in views.iter() {
        let key = LensFlarePipelineKey {
            samples: msaa.samples(),
            hdr: view.hdr,
            focus,
        };
        let id = pipelines.specialize(&pipeline_cache, &pipeline, key);
        commands
            .entity(entity)
            .insert(LensFlarePipelineId { id, key });
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct LensFlareLabel;

#[derive(Default)]
struct LensFlareNode;

impl ViewNode for LensFlareNode {
    type ViewQuery = (
        Read<ViewTarget>,
        Read<ViewDepthTexture>,
        Read<LensFlarePipelineId>,
        Read<LensFlareUniformOffset>,
        Option<Read<TransparentFocusTexture>>,
        Read<LensFlareView>,
    );

    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, depth, pipeline_id, offset, focus, _): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let (Some(render_pipeline), Some(flare_binding)) = (
            world
                .resource::<PipelineCache>()
                .get_render_pipeline(pipeline_id.id),
            world.resource::<LensFlareUniforms>().0.binding(),
        ) else {
            return Ok(());
        };
        // the depth texture is bindable once `bind_depth_texture` has run for the camera
        if !depth
            .texture
            .usage()
            .contains(TextureUsages::TEXTURE_BINDING)
        {
            return Ok(());
        }
        let key = pipeline_id.key;
        let layout = &world.resource::<LensFlarePipeline>().layouts[&(key.samples, key.focus)];
        let device = render_context.render_device();
        let bind_group = match (key.focus, focus) {
            (true, Some(focus)) => device.create_bind_group(
                Some("lens flare bind group"),
                layout,
                &BindGroupEntries::sequential((
                    depth.view(),
                    flare_binding,
                    &focus.texture.default_view,
                )),
            ),
            (false, _) => device.create_bind_group(
                Some("lens flare bind group"),
                layout,
                &BindGroupEntries::sequential((depth.view(), flare_binding)),
            ),
            // the pipeline was keyed for a focus texture the view no longer has
            (true, None) => return Ok(()),
        };

        let mut pass = render_context
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("lens flare"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    // drawn straight onto the frame, no copy
                    view: view_target.main_texture_view(),
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        pass.set_pipeline(render_pipeline);
        pass.set_bind_group(0, &bind_group, &[offset.offset]);
        pass.draw(0..6, 0..offset.count);

        Ok(())
    }
}

#[cfg(test)]
mod tests;
