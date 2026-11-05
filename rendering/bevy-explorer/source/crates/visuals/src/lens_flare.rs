//! The lens flare of the directional light, the sun by day and the moon by night, drawn onto the
//! frame before bloom as URP draws its data-driven lens flare. Unity puts a
//! `LensFlareComponentSRP` with occlusion on the directional light and picks its data and
//! intensity by the time of day (unity:Explorer/Assets/DCL/SkyBox/SkyboxRenderController.cs,
//! `UpdateLensFlare`). The element placement, the occlusion and the shapes are URP's
//! (unity:Explorer/Library/PackageCache/com.unity.render-pipelines.core@a4c9a9793789/Runtime/PostProcessing/LensFlareCommonSRP.cs);
//! see `lens_flare.wgsl`.

use bevy::{
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::{query::QueryItem, system::lifetimeless::Read},
    image::BevyDefault as _,
    platform::collections::HashMap,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{texture_depth_2d, texture_depth_2d_multisampled, uniform_buffer},
            BindGroupEntries, BindGroupLayout, BlendComponent, BlendFactor, BlendOperation,
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            DynamicUniformBuffer, FragmentState, LoadOp, MultisampleState, Operations,
            PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderDefVal, ShaderStages, ShaderType,
            SpecializedRenderPipeline, SpecializedRenderPipelines, StoreOp, TextureFormat,
            TextureUsages,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget},
        Render, RenderApp, RenderSet,
    },
};
use common::structs::{LensFlare, SceneGlobalLight, TimeOfDay};
use scene_runner::update_world::lights::celestial_swap;

use crate::{
    day_night::day_fraction,
    sky_preset::{sample_keys, LENS_FLARE_INTENSITY_BY_TIME},
};

/// The shape of a flare element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlareKind {
    Circle,
    /// a regular polygon (`m_SideCount`) with its corners rounded by `m_SdfRoundness`
    Polygon {
        sides: u32,
        roundness: f32,
    },
    /// a ring, or with `partial` only its arc on one side. Unity draws these as images
    /// (`Ring_T_Flare.png`, `PartialRIng_T_Flare.png`) that were not carried, so they are drawn
    /// procedurally
    Ring {
        partial: bool,
    },
}

/// unity: Rendering/LensFlare/Day.asset and Night.asset, LensFlareDataSRP elements. Each field
/// is the element's field of the same meaning there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlareElement {
    /// `flareType`
    pub kind: FlareKind,
    /// `m_LocalIntensity`
    pub intensity: f32,
    /// `position`: 0 at the light, 0.5 at the screen centre, 1 at the light mirrored through it
    pub position: f32,
    /// `uniformScale`
    pub scale: f32,
    /// `sizeXY`
    pub size_xy: Vec2,
    /// `tint`; the additive blend ignores its alpha
    pub tint: Vec4,
    /// `m_FallOff`
    pub falloff: f32,
    /// `m_EdgeOffset`
    pub edge_offset: f32,
    /// `m_Count` where `allowMultipleElement` is on, else 1. The copies follow Unity's `Random`
    /// distribution, the only one these flares use
    pub count: u32,
    /// `lengthSpread`: how far along the ray the copies spread
    pub spread: f32,
    /// `rotation`, degrees
    pub rotation: f32,
    /// `autoRotate`: turned to face along the ray
    pub auto_rotate: bool,
    /// `modulateByLightColor`
    pub modulate_by_light: bool,
    /// `scaleVariation` of the copies
    pub scale_variation: f32,
    /// `rotationVariation` of the copies
    pub rotation_variation: f32,
}

/// unity:Explorer/Assets/DCL/Rendering/LensFlare/Day.asset, elements 1, 2, 3, 5 and 6; element
/// 4 (`Shimmer_T_Flare`) is not `visible`.
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
    },
];

/// unity:Explorer/Assets/DCL/Rendering/LensFlare/Night.asset
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
    },
];

/// The flare data's `StartTime`s in the sky preset's `lensFlareEntries`, as day fractions
/// (unity:Explorer/Assets/DCL/SkyBox/Presets/StylizedV1.asset).
const DAY_FLARE_START: f32 = 0.2;
const NIGHT_FLARE_START: f32 = 0.8;

/// The most elements a flare has; the uniform holds this many.
const MAX_FLARE_ELEMENTS: usize = 6;
const _: () =
    assert!(DAY_FLARE.len() <= MAX_FLARE_ELEMENTS && NIGHT_FLARE.len() <= MAX_FLARE_ELEMENTS);

/// Every element's `m_IntensityVariation` and `positionVariation.x` (its `.y` is 0).
const INTENSITY_VARIATION: f32 = 0.75;
const POSITION_VARIATION: f32 = 1.0;

/// The random copies' `colorGradient`, the same in every element that has copies: (time,
/// colour) keys, blended linearly.
const RANDOM_TINT: [(f32, Vec3); 3] = [
    (0.0, Vec3::new(1.0, 1.0, 1.0)),
    (52043.0 / 65535.0, Vec3::new(1.0, 0.6600066, 0.3537736)),
    (65342.0 / 65535.0, Vec3::new(1.0, 0.33182937, 0.1462264)),
];

/// The ring elements' `ringThickness`, for the procedural rings.
const RING_THICKNESS: f32 = 0.25;

/// `scaleSize` in `ProcessLensFlareSRPElementsSingle`: an element's size on screen is `sizeXY *
/// uniformScale` times this.
const SCALE_SIZE: f32 = 0.1;

/// `LensFlareComponentSRP` defaults: occlusion samples within `occlusionRadius` times the
/// projected size of a body `sCelestialAngularRadius` across, for a directional light. Its
/// `occlusionOffset` (0.05) moves the tested point toward the camera, which leaves a light at
/// infinity at infinity, so it has no counterpart here.
const OCCLUSION_SAMPLES: u32 = 32;
const OCCLUSION_RADIUS: f32 = 0.1;
const CELESTIAL_ANGULAR_RADIUS_DEGREES: f32 = 3.3;

/// The flare data in use at `day_fraction`: the entry with the latest start at or before it,
/// else the latest of the day, as `SkyboxLookPreset.ActiveLensFlareEntry`.
pub fn flare_set(day_fraction: f32) -> &'static [FlareElement] {
    if (DAY_FLARE_START..NIGHT_FLARE_START).contains(&day_fraction) {
        &DAY_FLARE
    } else {
        &NIGHT_FLARE
    }
}

/// The flare's intensity at `day_fraction`, dimmed while the light swaps between the sun and the
/// moon (`swap_dip` 1 at the middle of the swap), as `SkyboxRenderController.UpdateLensFlare`.
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
            .add_systems(Update, update_lens_flare_settings);

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
            // before bloom, as URP; bevy blooms before depth of field, so the flare comes
            // straight after the main pass
            .add_render_graph_edges(Core3d, (Node3d::EndMainPass, LensFlareLabel, Node3d::Bloom));
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<LensFlarePipeline>();
    }
}

/// The flare as the directional light and the time of day set it.
#[derive(Resource, Clone, ExtractResource)]
struct LensFlareSettings {
    /// direction to the light's body
    toward_light: Vec3,
    /// the light's colour, for the elements it tints: Unity multiplies by `light.color` as is,
    /// so these are its sRGB components
    light_color: Vec3,
    intensity: f32,
    elements: &'static [FlareElement],
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
        // dimmed with the light while it swings between the sun and the moon
        intensity: flare_intensity(t, celestial_swap(t).1),
        elements: flare_set(t),
    };
}

/// A view that draws the lens flare.
#[derive(Component, Clone)]
struct LensFlareView;

impl ExtractComponent for LensFlareView {
    type QueryData = ();
    type QueryFilter = With<LensFlare>;
    type Out = Self;

    fn extract_component(_: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(Self)
    }
}

/// [`FlareKind`] as the shader tells them apart.
const KIND_CIRCLE: u32 = 0;
const KIND_POLYGON: u32 = 1;
const KIND_RING: u32 = 2;
const KIND_PARTIAL_RING: u32 = 3;

use gpu::{FlareElementGpu, LensFlareUniform};

mod gpu {
    // the ShaderType derive's layout check function is never called
    #![allow(dead_code)]

    use super::*;

    /// A [`FlareElement`] as `lens_flare.wgsl` reads it.
    #[derive(Clone, Copy, Default, ShaderType)]
    pub struct FlareElementGpu {
        pub tint: Vec4,
        /// half the size of the element's quad, in half screen heights
        pub size: Vec2,
        pub position: f32,
        pub rotation: f32,
        pub intensity: f32,
        pub falloff: f32,
        pub edge_offset: f32,
        pub spread: f32,
        pub roundness: f32,
        pub scale_variation: f32,
        pub rotation_variation: f32,
        pub kind: u32,
        pub sides: u32,
        pub count: u32,
        pub auto_rotate: u32,
        pub modulate_by_light: u32,
    }

    /// The flare of one view.
    #[derive(Clone, ShaderType)]
    pub struct LensFlareUniform {
        pub light_color: Vec3,
        /// the flare's intensity, with the light's shape attenuation
        pub intensity: f32,
        /// the light's position on screen
        pub sun_ndc: Vec2,
        /// radius of the occlusion samples' disc, in Unity's units: viewport, used as NDC
        pub occlusion_radius: f32,
        pub count: u32,
        /// [`RANDOM_TINT`]: colour, then time
        pub random_tint: [Vec4; 3],
        pub elements: [FlareElementGpu; MAX_FLARE_ELEMENTS],
    }
}

impl From<&FlareElement> for FlareElementGpu {
    fn from(element: &FlareElement) -> Self {
        let (kind, sides, roundness) = match element.kind {
            FlareKind::Circle => (KIND_CIRCLE, 0, 0.0),
            FlareKind::Polygon { sides, roundness } => (KIND_POLYGON, sides, roundness),
            FlareKind::Ring { partial: false } => (KIND_RING, 0, 0.0),
            FlareKind::Ring { partial: true } => (KIND_PARTIAL_RING, 0, 0.0),
        };
        Self {
            tint: element.tint,
            size: element.size_xy * SCALE_SIZE * element.scale,
            position: element.position,
            rotation: element.rotation,
            intensity: element.intensity,
            falloff: element.falloff,
            edge_offset: element.edge_offset,
            spread: element.spread,
            roundness,
            scale_variation: element.scale_variation,
            rotation_variation: element.rotation_variation,
            kind,
            sides,
            count: element.count,
            auto_rotate: element.auto_rotate as u32,
            modulate_by_light: element.modulate_by_light as u32,
        }
    }
}

/// The flare as a view sees it, or none where Unity draws none: the light below the horizon,
/// behind the camera or off screen (`allowOffScreen` is off), or the flare at no intensity.
/// `LensFlareCommonSRP.DoComponent` and `ComputeOcclusion`.
fn flare_uniform(
    settings: &LensFlareSettings,
    clip_from_world: Mat4,
    camera_up: Vec3,
    camera_forward: Vec3,
) -> Option<LensFlareUniform> {
    let toward = settings.toward_light;
    // the light's shape attenuation, `ShapeAttenuationDirLight`
    let intensity = settings.intensity * toward.dot(camera_forward).max(0.0);
    if intensity <= 0.0 || toward.y <= 0.0 {
        return None;
    }
    // the light is infinitely far: project its direction
    let project = |direction: Vec3| {
        let clip = clip_from_world * direction.extend(0.0);
        (clip.w > 0.0).then(|| clip.xy() / clip.w)
    };
    let sun_ndc = project(toward)?;
    if sun_ndc.abs().max_element() > 1.0 {
        return None;
    }
    // `celestialProjectedOcclusionRadius` as far off as the light, measured as `DoComponent`
    // measures it: a distance in viewport units
    let edge = project(
        toward + camera_up * OCCLUSION_RADIUS * CELESTIAL_ANGULAR_RADIUS_DEGREES.to_radians().tan(),
    )?;
    let occlusion_radius = 0.5 * (edge - sun_ndc).length();

    let mut elements = [FlareElementGpu::default(); MAX_FLARE_ELEMENTS];
    for (gpu, element) in elements.iter_mut().zip(settings.elements) {
        *gpu = element.into();
    }
    Some(LensFlareUniform {
        light_color: settings.light_color,
        intensity,
        sun_ndc,
        occlusion_radius,
        count: settings.elements.len() as u32,
        random_tint: RANDOM_TINT.map(|(time, color)| color.extend(time)),
        elements,
    })
}

#[derive(Resource, Default)]
struct LensFlareUniforms(DynamicUniformBuffer<LensFlareUniform>);

/// Where a view's flare sits in [`LensFlareUniforms`]; absent while the view draws no flare.
#[derive(Component)]
struct LensFlareUniformOffset(u32);

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
        let flare = flare_uniform(
            &settings,
            clip_from_world,
            *view.world_from_view.up(),
            *view.world_from_view.forward(),
        );
        match flare {
            Some(flare) => {
                let offset = uniforms.0.push(&flare);
                commands
                    .entity(entity)
                    .insert(LensFlareUniformOffset(offset));
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
}

#[derive(Resource)]
struct LensFlarePipeline {
    shader: Handle<Shader>,
    /// by sample count
    layouts: HashMap<u32, BindGroupLayout>,
}

impl FromWorld for LensFlarePipeline {
    fn from_world(world: &mut World) -> Self {
        let shader = world
            .resource::<AssetServer>()
            .load("embedded://shaders/lens_flare.wgsl");
        let device = world.resource::<RenderDevice>();

        let layouts = [1, 2, 4, 8]
            .into_iter()
            .map(|samples| {
                let depth = if samples > 1 {
                    texture_depth_2d_multisampled()
                } else {
                    texture_depth_2d()
                };
                let entries = [
                    depth.build(0, ShaderStages::FRAGMENT),
                    uniform_buffer::<LensFlareUniform>(true).build(1, ShaderStages::FRAGMENT),
                ];
                let layout =
                    device.create_bind_group_layout(Some("lens flare bind group layout"), &entries);
                (samples, layout)
            })
            .collect();

        Self { shader, layouts }
    }
}

/// The numbers `lens_flare.wgsl` shares with this file; floats as their bit patterns, which it
/// reads back with `bitcast<f32>`.
fn shader_constants() -> Vec<ShaderDefVal> {
    let float = |name: &str, value: f32| ShaderDefVal::UInt(name.into(), value.to_bits());
    vec![
        ShaderDefVal::UInt("MAX_ELEMENTS".into(), MAX_FLARE_ELEMENTS as u32),
        ShaderDefVal::UInt("OCCLUSION_SAMPLES".into(), OCCLUSION_SAMPLES),
        ShaderDefVal::UInt("KIND_CIRCLE".into(), KIND_CIRCLE),
        ShaderDefVal::UInt("KIND_POLYGON".into(), KIND_POLYGON),
        ShaderDefVal::UInt("KIND_RING".into(), KIND_RING),
        ShaderDefVal::UInt("KIND_PARTIAL_RING".into(), KIND_PARTIAL_RING),
        float("INTENSITY_VARIATION", INTENSITY_VARIATION),
        float("POSITION_VARIATION", POSITION_VARIATION),
        float("RING_THICKNESS", RING_THICKNESS),
    ]
}

impl SpecializedRenderPipeline for LensFlarePipeline {
    type Key = LensFlarePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = shader_constants();
        if key.samples > 1 {
            shader_defs.push("MULTISAMPLED".into());
        }
        RenderPipelineDescriptor {
            label: Some("lens flare pipeline".into()),
            layout: vec![self.layouts[&key.samples].clone()],
            push_constant_ranges: Vec::new(),
            vertex: fullscreen_shader_vertex_state(),
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
                    // the flare only adds light, as URP's `LensFlareAdditive` pass
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
    samples: u32,
}

fn prepare_lens_flare_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LensFlarePipeline>>,
    pipeline: Res<LensFlarePipeline>,
    views: Query<(Entity, &ExtractedView, &Msaa), With<LensFlareView>>,
) {
    for (entity, view, msaa) in views.iter() {
        let key = LensFlarePipelineKey {
            samples: msaa.samples(),
            hdr: view.hdr,
        };
        let id = pipelines.specialize(&pipeline_cache, &pipeline, key);
        commands.entity(entity).insert(LensFlarePipelineId {
            id,
            samples: key.samples,
        });
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
        Read<LensFlareView>,
    );

    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, depth, pipeline_id, offset, _): QueryItem<'w, Self::ViewQuery>,
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
        let layout = &world.resource::<LensFlarePipeline>().layouts[&pipeline_id.samples];
        let bind_group = render_context.render_device().create_bind_group(
            Some("lens flare bind group"),
            layout,
            &BindGroupEntries::sequential((depth.view(), flare_binding)),
        );

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
        pass.set_bind_group(0, &bind_group, &[offset.0]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy::render::camera::CameraProjection;

    use super::*;

    #[test]
    fn flare_intensity_follows_the_stylized_keys() {
        assert_eq!(flare_intensity(0.5, 0.0), 1.0);
        assert_eq!(flare_intensity(0.8, 0.0), 0.0);
        assert!((flare_intensity(0.0, 0.0) - 0.6).abs() < 1e-6);
        assert_eq!(flare_intensity(0.5, 1.0), 0.0);
    }

    #[test]
    fn flare_set_switches_at_unity_start_times() {
        assert!(core::ptr::eq(flare_set(0.19), NIGHT_FLARE.as_slice()));
        assert!(core::ptr::eq(flare_set(0.2), DAY_FLARE.as_slice()));
        assert!(core::ptr::eq(flare_set(0.8), NIGHT_FLARE.as_slice()));
    }

    /// A light ahead of the camera shows its flare at its screen position; below the horizon,
    /// behind the camera or off screen it shows none.
    #[test]
    fn flare_shows_only_for_a_light_in_view_above_the_horizon() {
        let settings = |toward_light: Vec3| LensFlareSettings {
            toward_light,
            intensity: 1.0,
            ..default()
        };
        let camera = |forward: Vec3| {
            let view = Transform::default().looking_to(forward, Vec3::Y);
            let projection = PerspectiveProjection::default().get_clip_from_view();
            let clip_from_world = projection * view.compute_matrix().inverse();
            (clip_from_world, *view.up(), *view.forward())
        };
        let flare = |toward: Vec3, forward: Vec3| {
            let (clip_from_world, up, forward) = camera(forward);
            flare_uniform(&settings(toward), clip_from_world, up, forward)
        };

        let up_ahead = Vec3::new(0.0, 0.2, -1.0).normalize();
        let shown = flare(up_ahead, Vec3::NEG_Z).expect("a light ahead shows its flare");
        assert!(shown.sun_ndc.x.abs() < 1e-5 && shown.sun_ndc.y > 0.0);
        assert!(shown.occlusion_radius > 0.0 && shown.occlusion_radius < 0.01);
        assert_eq!(shown.count, DAY_FLARE.len() as u32);

        let below = Vec3::new(0.0, -0.2, -1.0).normalize();
        assert!(flare(below, Vec3::new(0.0, -0.2, -1.0)).is_none());
        assert!(flare(up_ahead, Vec3::Z).is_none());
        assert!(flare(up_ahead, Vec3::new(1.0, 0.0, -0.3)).is_none());
    }

    /// The flare pipelines compile on this machine's adapter, and the node draws with them, in a
    /// renderer with no window: a multisampled camera and an HDR one without MSAA, both with
    /// `LensFlare`, facing a directional light at noon. Needs a GPU, so `#[ignore]`d and skipped
    /// by CI:
    ///
    /// ```text
    /// cargo test -p visuals lens_flare_pipeline_compiles -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn lens_flare_pipeline_compiles() {
        use std::time::{Duration, Instant};

        use bevy::{
            asset::RenderAssetUsages,
            log::LogPlugin,
            render::{
                camera::RenderTarget,
                pipelined_rendering::PipelinedRenderingPlugin,
                render_resource::{
                    CachedPipelineState, Extent3d, PipelineDescriptor, TextureDimension,
                },
            },
            window::ExitCondition,
            winit::WinitPlugin,
        };

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
        app.add_plugins((assets::EmbedAssetsPlugin, LensFlarePlugin));
        // the light ahead of the cameras and up, at noon
        let toward_light = Vec3::new(0.0, 0.2, -1.0).normalize();
        app.insert_resource(SceneGlobalLight {
            dir_color: Color::WHITE,
            dir_direction: -toward_light,
            ..default()
        });
        app.insert_resource(TimeOfDay {
            time: 12.0 * 3600.0,
        });
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
        let mut images = world.resource_mut::<Assets<Image>>();
        let targets = [images.add(target.clone()), images.add(target)];
        world.spawn((
            DirectionalLight::default(),
            Transform::default().looking_to(-toward_light, Vec3::Y),
        ));
        for (target, hdr, msaa) in [
            (&targets[0], false, Msaa::Sample4),
            (&targets[1], true, Msaa::Off),
        ] {
            world.spawn((
                // the depth binding stars' `bind_depth_texture` adds to a `LensFlare` camera
                Camera3d {
                    depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                        | TextureUsages::TEXTURE_BINDING)
                        .into(),
                    ..default()
                },
                Camera {
                    target: RenderTarget::Image(target.clone().into()),
                    hdr,
                    ..default()
                },
                msaa,
                LensFlare,
                Transform::default().looking_to(Vec3::NEG_Z, Vec3::Y),
            ));
        }

        // a few frames queue every pipeline; then they compile in the background, and once they
        // are ready a few more frames draw the flare with them
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
        for _ in 0..5 {
            app.update();
        }
        println!("{ok} pipelines compiled");
        assert!(
            errors.is_empty(),
            "pipeline errors:\n{}",
            errors.join("\n\n")
        );

        let render_world = app.sub_app_mut(RenderApp).world_mut();
        let flares = render_world
            .resource::<PipelineCache>()
            .pipelines()
            .filter(|pipeline| {
                matches!(
                    &pipeline.descriptor,
                    PipelineDescriptor::RenderPipelineDescriptor(descriptor)
                        if descriptor.label.as_deref() == Some("lens flare pipeline")
                )
            })
            .count();
        assert_eq!(flares, 2, "one flare pipeline per camera");
        let drawing = render_world
            .query::<&LensFlareUniformOffset>()
            .iter(render_world)
            .count();
        assert_eq!(drawing, 2, "both cameras draw the flare");
    }
}
