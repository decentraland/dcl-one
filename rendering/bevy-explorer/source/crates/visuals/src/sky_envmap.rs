//! The environment maps attached as an `EnvironmentMapLight` to every camera that renders the
//! sky: the reflection (specular) map generated from the atmosphere skybox whenever the sky is
//! redrawn, and the diffuse map, which is not the sky at all but Unity's Trilight ambient
//! (`sky_preset::ambient_trilight`), written on the CPU as the three colours change.
//!
//! Only the upper hemisphere of the reflection map is generated. The pbr shaders' lookups
//! (`sky_envmap_lookup.wgsl`) mirror downward directions up, and level the reflections towards
//! the `AmbientLight`, which they use in place of the flat ambient.
//!
//! At the low sky reflections setting the reflection map is one texel per face instead: the sky,
//! the horizon and the ground as three colours, straight from the sky.

use bevy::{
    asset::{load_internal_asset, weak_handle},
    pbr::environment_map::EnvironmentMapLight,
    prelude::*,
    render::{
        camera::Exposure,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        graph::CameraDriverLabel,
        render_asset::{RenderAssetUsages, RenderAssets},
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{
            binding_types::{
                sampler, texture_2d_array, texture_cube, texture_storage_2d_array, uniform_buffer,
            },
            BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries,
            CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor,
            DynamicUniformBuffer, Extent3d, FilterMode, PipelineCache, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, StorageTextureAccess, Texture, TextureAspect,
            TextureDescriptor, TextureDimension, TextureFormat, TextureId, TextureSampleType,
            TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
        Render, RenderApp, RenderSet,
    },
};
use bevy_atmosphere::{
    pipeline::{AtmosphereImage, AtmosphereUpdateEvent, BevyAtmosphereLabel},
    prelude::AtmosphereCamera,
};
use bevy_console::ConsoleCommand;
use common::structs::{AppConfig, SkyReflectionSetting};
use console::DoAddConsoleCommand;
use scene_runner::update_world::lights::UNITY_LIGHT_LUX;
use std::f32::consts::PI;

const SKY_ENVMAP_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("6b2f4c7e-3a1d-4f8e-9c2b-7d5e1a0f3b64");

const SPECULAR_SIZE: u32 = 128;
const SPECULAR_MIPS: u32 = 8;
const SPECULAR_SAMPLES: u32 = 32;
// the low setting: one texel per face, straight from the (unfiltered) sky
const TRILIGHT_SAMPLES: u32 = 256;
const WORKGROUP_SIZE: u32 = 8;
// the filter passes generate every face but -y (see sky_envmap.wgsl)
const GENERATED_FACES: u32 = 5;
/// texels per face of the ambient map: enough for the gradient to read smoothly across a face
const AMBIENT_SIZE: u32 = 8;

pub struct SkyEnvmapPlugin;

/// The maps: the ambient (diffuse) map, and the reflection maps in the skybox's own units, the
/// filtered one and the one-texel one of the low setting.
#[derive(Resource, Clone, ExtractResource)]
pub struct SkyEnvmap {
    pub ambient: Handle<Image>,
    pub specular: Handle<Image>,
    pub trilight_specular: Handle<Image>,
}

impl SkyEnvmap {
    fn specular(&self, setting: SkyReflectionSetting) -> &Handle<Image> {
        match setting {
            SkyReflectionSetting::Low => &self.trilight_specular,
            SkyReflectionSetting::High => &self.specular,
        }
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
pub struct SkyEnvmapSettings {
    /// scale on the ambient (diffuse) map, applied when it is written
    pub diffuse: f32,
    /// scale on each band of the ambient map: sky, equator, ground
    pub trilight: Vec3,
    /// scale on the reflection (specular) map, applied when it is generated
    pub specular: f32,
    /// scale on the floor the lookups level the reflections towards, which comes from the ambient
    /// brightness setting. carried to the pbr shaders as the `AmbientLight`. unity: 0, the
    /// skybox reflects as drawn (`reflectionIntensity 1`, no floor); `/envmap` sets it live
    pub floor: f32,
    /// how far reflected light above the floor is compressed towards it, in log space (0 = not
    /// at all, 1 = down to the floor). carried with the floor; unity: 0
    pub compression: f32,
    /// which reflection map to generate, from the config
    pub quality: SkyReflectionSetting,
}

impl Default for SkyEnvmapSettings {
    fn default() -> Self {
        Self {
            diffuse: 1.0,
            trilight: Vec3::ONE,
            specular: 1.0,
            floor: 0.0,
            compression: 0.0,
            quality: SkyReflectionSetting::High,
        }
    }
}

/// unity: the Trilight ambient colours (`RenderSettings.ambientSkyColor`, `ambientEquatorColor`,
/// `ambientGroundColor`), linear, in Unity's light units: sky, equator, ground. Written by
/// `apply_global_light` from the preset's ramps at the time of day, a scene's ambient override
/// and the ambient brightness setting; read into the ambient map here.
#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub struct SkyAmbient {
    pub bands: [Vec3; 3],
}

impl Plugin for SkyEnvmapPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            SKY_ENVMAP_SHADER_HANDLE,
            "sky_envmap.wgsl",
            Shader::from_wgsl
        );

        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let specular = images.add(cube_image(
            "sky specular envmap",
            SPECULAR_SIZE,
            SPECULAR_MIPS,
        ));
        let trilight_specular = images.add(cube_image("sky trilight specular envmap", 1, 1));
        let ambient = images.add(ambient_image());

        app.insert_resource(SkyEnvmap {
            ambient,
            specular,
            trilight_specular,
        })
        .init_resource::<SkyEnvmapSettings>()
        .init_resource::<SkyAmbient>()
        .add_plugins((
            ExtractResourcePlugin::<SkyEnvmap>::default(),
            ExtractResourcePlugin::<SkyEnvmapSettings>::default(),
        ))
        .add_systems(
            Update,
            (
                attach_sky_envmap,
                write_sky_ambient.after(crate::apply_global_light),
            ),
        );

        app.add_console_command::<EnvmapConsoleCommand, _>(envmap_console_command);
        app.add_console_command::<AmbientConsoleCommand, _>(ambient_console_command);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<SkyEnvmapPipelines>()
            .add_systems(
                Render,
                prepare_sky_envmap.in_set(RenderSet::PrepareResources),
            );

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(SkyEnvmapLabel, SkyEnvmapNode);
        render_graph.add_node_edge(BevyAtmosphereLabel, SkyEnvmapLabel);
        render_graph.add_node_edge(SkyEnvmapLabel, CameraDriverLabel);
    }
}

fn cube_image(label: &'static str, size: u32, mips: u32) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.label = Some(label);
    image.texture_descriptor.mip_level_count = mips;
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        label: Some(label),
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// The ambient map, black until the first write. Kept in the main world so it can be rewritten.
fn ambient_image() -> Image {
    let label = "sky ambient envmap";
    let mut image = Image::new(
        Extent3d {
            width: AMBIENT_SIZE,
            height: AMBIENT_SIZE,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        ambient_cube_bytes([Vec3::ZERO; 3], 0.0),
        TextureFormat::Rgba16Float,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.label = Some(label);
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        label: Some(label),
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// unity: the Trilight gradient as a surface with normal `y` up receives it. The environment is
/// the sky colour straight up, the equator at the horizon and the ground straight down, linear
/// in between; Unity projects it onto the ambient SH probe, which URP evaluates at the normal as
/// irradiance (`SampleSH`), so a surface sees the cosine-weighted mean of the hemisphere it
/// faces, not the colour straight along its normal. An up-facing surface gets 2/3 sky and 1/3
/// equator, a wall a fifth of each of sky and ground. The L2 projection's slight rounding of this
/// is left out.
fn trilight_gradient(y: f32, [sky, equator, ground]: [Vec3; 3]) -> Vec3 {
    equator
        + (sky - equator) * clamped_cosine_overlap(y)
        + (ground - equator) * clamped_cosine_overlap(-y)
}

/// (1/pi) * the integral over the sphere of max(up . w, 0) * max(n . w, 0), for a normal `n`
/// with `y = n . up`: the share a surface receives of a light that rises linearly from the
/// horizon to 1 straight up. `(2 / 3pi) * ((pi - a) cos a + sin a)` with `a` the angle between
/// them: 2/3 facing up, 2/(3pi) on a wall, 0 facing down.
fn clamped_cosine_overlap(y: f32) -> f32 {
    let y = y.clamp(-1.0, 1.0);
    let angle = y.acos();
    2.0 / (3.0 * PI) * ((PI - angle) * y + angle.sin())
}

/// sky_envmap.wgsl `cube_texel_dir`: the direction a cube texel at `uv` of `face` covers, in the
/// wgpu face order +x, -x, +y, -y, +z, -z. Only `y` is read here, so the z flip bevy's lookups
/// need does not matter.
fn cube_texel_dir(uv: Vec2, face: u32) -> Vec3 {
    let uvc = 2.0 * uv - 1.0;
    match face {
        0 => Vec3::new(1.0, -uvc.y, -uvc.x),
        1 => Vec3::new(-1.0, -uvc.y, uvc.x),
        2 => Vec3::new(uvc.x, 1.0, uvc.y),
        3 => Vec3::new(uvc.x, -1.0, -uvc.y),
        4 => Vec3::new(uvc.x, -uvc.y, 1.0),
        _ => Vec3::new(-uvc.x, -uvc.y, -1.0),
    }
    .normalize()
}

/// The ambient map's texels: the gradient of `bands` (already in the map's units) times
/// `scale`, as rgba16float with alpha 1, faces in order, rows top down.
fn ambient_cube_bytes(bands: [Vec3; 3], scale: f32) -> Vec<u8> {
    let texels = (AMBIENT_SIZE * AMBIENT_SIZE * 6) as usize;
    let mut bytes = Vec::with_capacity(texels * 8);
    for face in 0..6 {
        for y in 0..AMBIENT_SIZE {
            for x in 0..AMBIENT_SIZE {
                let uv = (Vec2::new(x as f32, y as f32) + 0.5) / AMBIENT_SIZE as f32;
                let color = trilight_gradient(cube_texel_dir(uv, face).y, bands) * scale;
                for channel in [color.x, color.y, color.z, 1.0] {
                    bytes.extend_from_slice(&half::f16::from_f32(channel).to_le_bytes());
                }
            }
        }
    }
    bytes
}

// the skybox material draws the cube without the view exposure, while the pbr
// shader applies it to environment light. cancel it so a mirror reflects the
// sky exactly as drawn.
#[allow(clippy::type_complexity)]
fn attach_sky_envmap(
    mut commands: Commands,
    envmap: Res<SkyEnvmap>,
    config: Res<AppConfig>,
    mut settings: ResMut<SkyEnvmapSettings>,
    mut cameras: Query<
        (Entity, Option<&Exposure>, Option<&mut EnvironmentMapLight>),
        With<AtmosphereCamera>,
    >,
) {
    let quality = config.graphics.sky_reflections;
    if settings.quality != quality {
        settings.quality = quality;
    }
    let specular = envmap.specular(quality);
    for (camera, exposure, light) in cameras.iter_mut() {
        let intensity = 1.0 / exposure.copied().unwrap_or_default().exposure();
        match light {
            Some(mut light) => {
                if light.intensity != intensity {
                    light.intensity = intensity;
                }
                if light.specular_map != *specular {
                    light.specular_map = specular.clone();
                }
            }
            None => {
                commands.entity(camera).try_insert(EnvironmentMapLight {
                    diffuse_map: envmap.ambient.clone(),
                    specular_map: specular.clone(),
                    intensity,
                    ..default()
                });
            }
        }
    }
}

/// Writes the ambient map when the Trilight colours, the scales or the exposure change. The
/// map shares the `EnvironmentMapLight` intensity that cancels the view exposure for the
/// reflections, so its texels carry the exposure: a Unity light unit is `UNITY_LIGHT_LUX` lux,
/// and the pbr shader multiplies the diffuse colour by irradiance / pi.
fn write_sky_ambient(
    ambient: Res<SkyAmbient>,
    settings: Res<SkyEnvmapSettings>,
    envmap: Res<SkyEnvmap>,
    cameras: Query<Option<&Exposure>, With<AtmosphereCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut last_exposure: Local<Option<f32>>,
) {
    let Some(exposure) = cameras
        .iter()
        .next()
        .map(|exposure| exposure.copied().unwrap_or_default().exposure())
    else {
        return;
    };
    if !ambient.is_changed() && !settings.is_changed() && *last_exposure == Some(exposure) {
        return;
    }
    *last_exposure = Some(exposure);

    let mut bands = ambient.bands;
    for (band, scale) in bands.iter_mut().zip(settings.trilight.to_array()) {
        *band *= scale;
    }
    let bytes = ambient_cube_bytes(bands, settings.diffuse * UNITY_LIGHT_LUX / PI * exposure);
    // the clock moves the colours every frame by less than a half float resolves; only a
    // changed image is re-uploaded
    if let Some(image) = images.get(&envmap.ambient) {
        if image.data.as_deref() == Some(bytes.as_slice()) {
            return;
        }
    }
    if let Some(image) = images.get_mut(&envmap.ambient) {
        image.data = Some(bytes);
    }
}

/// scale the sky reflection map, the ambient map, and the floor the reflections are levelled
/// towards, and set the compression above the floor
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/envmap")]
struct EnvmapConsoleCommand {
    specular: Option<f32>,
    diffuse: Option<f32>,
    floor: Option<f32>,
    compression: Option<f32>,
}

fn envmap_console_command(
    mut input: ConsoleCommand<EnvmapConsoleCommand>,
    mut settings: ResMut<SkyEnvmapSettings>,
) {
    if let Some(Ok(command)) = input.take() {
        if let Some(specular) = command.specular {
            settings.specular = specular;
        }
        if let Some(diffuse) = command.diffuse {
            settings.diffuse = diffuse;
        }
        if let Some(floor) = command.floor {
            settings.floor = floor;
        }
        if let Some(compression) = command.compression {
            settings.compression = compression.clamp(0.0, 1.0);
        }
        input.reply_ok(format!(
            "envmap specular {} diffuse {} floor {} compression {}",
            settings.specular, settings.diffuse, settings.floor, settings.compression
        ));
    }
}

/// scale the three bands of the Trilight ambient: sky, equator, ground (1 = Unity's)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/ambient")]
struct AmbientConsoleCommand {
    sky: Option<f32>,
    equator: Option<f32>,
    ground: Option<f32>,
}

fn ambient_console_command(
    mut input: ConsoleCommand<AmbientConsoleCommand>,
    mut settings: ResMut<SkyEnvmapSettings>,
    ambient: Res<SkyAmbient>,
) {
    if let Some(Ok(command)) = input.take() {
        if let Some(sky) = command.sky {
            settings.trilight.x = sky;
        }
        if let Some(equator) = command.equator {
            settings.trilight.y = equator;
        }
        if let Some(ground) = command.ground {
            settings.trilight.z = ground;
        }
        let [sky, equator, ground] = ambient.bands;
        input.reply_ok(format!(
            "ambient scale sky {} equator {} ground {}; unity linear sky {sky:.3} equator {equator:.3} ground {ground:.3}",
            settings.trilight.x, settings.trilight.y, settings.trilight.z
        ));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct SkyEnvmapLabel;

mod decl {
    // temporary for ShaderType macro, remove in future
    #![allow(dead_code)]

    use bevy::render::render_resource::ShaderType;

    #[derive(ShaderType, Clone, Copy)]
    pub struct Constants {
        pub intensity: f32,
        pub roughness: f32,
        pub sample_count: u32,
        pub source_lod: f32,
    }
}
use decl::Constants;

#[derive(Resource)]
struct SkyEnvmapPipelines {
    downsample_layout: BindGroupLayout,
    filter_layout: BindGroupLayout,
    sampler: Sampler,
    downsample_sky: CachedComputePipelineId,
    downsample: CachedComputePipelineId,
    specular: CachedComputePipelineId,
    trilight: CachedComputePipelineId,
}

impl FromWorld for SkyEnvmapPipelines {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();

        let downsample_layout = device.create_bind_group_layout(
            "sky envmap downsample layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_storage_2d_array(
                        TextureFormat::Rgba16Float,
                        StorageTextureAccess::WriteOnly,
                    ),
                ),
            ),
        );
        let filter_layout = device.create_bind_group_layout(
            "sky envmap filter layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_cube(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_storage_2d_array(
                        TextureFormat::Rgba16Float,
                        StorageTextureAccess::WriteOnly,
                    ),
                    uniform_buffer::<Constants>(true),
                ),
            ),
        );
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("sky envmap sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..default()
        });

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = |label: &'static str,
                        layout: &BindGroupLayout,
                        entry: &'static str,
                        shader_defs: &[&str]| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(label.into()),
                layout: vec![layout.clone()],
                push_constant_ranges: Vec::new(),
                shader: SKY_ENVMAP_SHADER_HANDLE,
                shader_defs: shader_defs.iter().map(|def| (*def).into()).collect(),
                entry_point: entry.into(),
                zero_initialize_workgroup_memory: false,
            })
        };

        Self {
            downsample_sky: pipeline(
                "sky envmap downsample sky",
                &filter_layout,
                "downsample_sky",
                &[],
            ),
            downsample: pipeline(
                "sky envmap downsample",
                &downsample_layout,
                "downsample",
                &["DOWNSAMPLE"],
            ),
            specular: pipeline("sky envmap specular", &filter_layout, "specular", &[]),
            // the irradiance pass reading the sky itself: a cosine-weighted mean per face
            trilight: pipeline(
                "sky envmap trilight",
                &filter_layout,
                "irradiance",
                &["RAW_SKY"],
            ),
            downsample_layout,
            filter_layout,
            sampler,
        }
    }
}

/// Bind groups for one generation: box-downsample the sky into a mipped
/// intermediate cube, then filter each output mip from it. Rebuilt when the
/// sky or output textures are recreated; the uniforms are rewritten in place
/// when the scales change.
#[derive(Resource)]
struct SkyEnvmapResources {
    /// the sky and output textures the bind groups were built for
    textures: [TextureId; 3],
    downsample_sky: (BindGroup, u32),
    downsample: Vec<BindGroup>,
    specular: Vec<(BindGroup, u32)>,
    /// the low setting's reflection map
    trilight: (BindGroup, u32),
    _intermediate: Texture,
    uniforms: DynamicUniformBuffer<Constants>,
}

/// Pushes the constants for every filter pass: one per specular mip, then the trilight pass. The
/// order (and so the offsets) is the same every time, so rewriting keeps bind groups valid.
fn push_constants(
    uniforms: &mut DynamicUniformBuffer<Constants>,
    settings: &SkyEnvmapSettings,
) -> (Vec<u32>, u32) {
    uniforms.clear();
    let specular = (0..SPECULAR_MIPS)
        .map(|mip| {
            uniforms.push(&Constants {
                intensity: settings.specular,
                roughness: mip as f32 / (SPECULAR_MIPS - 1) as f32,
                sample_count: SPECULAR_SAMPLES,
                source_lod: 0.0,
            })
        })
        .collect();
    let trilight = uniforms.push(&Constants {
        intensity: settings.specular,
        roughness: 0.0,
        sample_count: TRILIGHT_SAMPLES,
        source_lod: 0.0,
    });
    (specular, trilight)
}

fn array_view(texture: &Texture, mip: u32) -> TextureView {
    texture.create_view(&TextureViewDescriptor {
        label: Some("sky envmap mip"),
        dimension: Some(TextureViewDimension::D2Array),
        aspect: TextureAspect::All,
        base_mip_level: mip,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(6),
        ..default()
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_sky_envmap(
    mut commands: Commands,
    existing: Option<ResMut<SkyEnvmapResources>>,
    pipelines: Res<SkyEnvmapPipelines>,
    settings: Res<SkyEnvmapSettings>,
    atmosphere_image: Res<AtmosphereImage>,
    envmap: Res<SkyEnvmap>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(source), Some(specular), Some(trilight_specular)) = (
        gpu_images.get(&atmosphere_image.handle),
        gpu_images.get(&envmap.specular),
        gpu_images.get(&envmap.trilight_specular),
    ) else {
        return;
    };
    let textures = [source, specular, trilight_specular].map(|image| image.texture.id());
    if let Some(mut existing) = existing {
        if existing.textures == textures {
            if settings.is_changed() {
                push_constants(&mut existing.uniforms, &settings);
                existing.uniforms.write_buffer(&device, &queue);
            }
            return;
        }
    }

    let intermediate = device.create_texture(&TextureDescriptor {
        label: Some("sky envmap intermediate"),
        size: Extent3d {
            width: SPECULAR_SIZE,
            height: SPECULAR_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: SPECULAR_MIPS,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let intermediate_cube = intermediate.create_view(&TextureViewDescriptor {
        label: Some("sky envmap intermediate cube"),
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    let mut uniforms = DynamicUniformBuffer::<Constants>::default();
    let (specular_offsets, trilight_offset) = push_constants(&mut uniforms, &settings);
    uniforms.write_buffer(&device, &queue);
    let uniform_binding = uniforms.binding().unwrap();

    // mip 0 comes from the sky cube itself (with the lower half mirrored), the rest from the
    // mip above
    let downsample = (1..SPECULAR_MIPS)
        .map(|mip| {
            device.create_bind_group(
                "sky envmap downsample",
                &pipelines.downsample_layout,
                &BindGroupEntries::sequential((
                    &array_view(&intermediate, mip - 1),
                    &pipelines.sampler,
                    &array_view(&intermediate, mip),
                )),
            )
        })
        .collect();

    let filter_bind_group = |input: &TextureView, output: &TextureView, offset: u32| {
        (
            device.create_bind_group(
                "sky envmap filter",
                &pipelines.filter_layout,
                &BindGroupEntries::sequential((
                    input,
                    &pipelines.sampler,
                    output,
                    uniform_binding.clone(),
                )),
            ),
            offset,
        )
    };
    // the first downsample reads the sky as a cube, so it shares the filter layout; it ignores
    // the constants
    let downsample_sky = filter_bind_group(
        &source.texture_view,
        &array_view(&intermediate, 0),
        specular_offsets[0],
    );
    let specular = (0..SPECULAR_MIPS)
        .map(|mip| {
            filter_bind_group(
                &intermediate_cube,
                &array_view(&specular.texture, mip),
                specular_offsets[mip as usize],
            )
        })
        .collect();
    let trilight = filter_bind_group(
        &source.texture_view,
        &array_view(&trilight_specular.texture, 0),
        trilight_offset,
    );

    commands.insert_resource(SkyEnvmapResources {
        textures,
        downsample_sky,
        downsample,
        specular,
        trilight,
        _intermediate: intermediate,
        uniforms,
    });
}

struct SkyEnvmapNode;

impl render_graph::Node for SkyEnvmapNode {
    fn run<'w>(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut bevy::render::renderer::RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), render_graph::NodeRunError> {
        // the sky only redraws when the atmosphere changes; follow it
        if world.resource::<Events<AtmosphereUpdateEvent>>().is_empty() {
            return Ok(());
        }
        let Some(resources) = world.get_resource::<SkyEnvmapResources>() else {
            return Ok(());
        };
        let pipelines = world.resource::<SkyEnvmapPipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let quality = world.resource::<SkyEnvmapSettings>().quality;
        let (Some(downsample_sky), Some(downsample), Some(specular), Some(trilight)) = (
            pipeline_cache.get_compute_pipeline(pipelines.downsample_sky),
            pipeline_cache.get_compute_pipeline(pipelines.downsample),
            pipeline_cache.get_compute_pipeline(pipelines.specular),
            pipeline_cache.get_compute_pipeline(pipelines.trilight),
        ) else {
            return Ok(());
        };

        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("sky envmap"),
                    timestamp_writes: None,
                });
        let groups = |size: u32| size.div_ceil(WORKGROUP_SIZE);

        if quality == SkyReflectionSetting::Low {
            pass.set_pipeline(trilight);
            let (bind_group, offset) = &resources.trilight;
            pass.set_bind_group(0, bind_group, &[*offset]);
            pass.dispatch_workgroups(1, 1, GENERATED_FACES);
            return Ok(());
        }

        pass.set_pipeline(downsample_sky);
        let (bind_group, offset) = &resources.downsample_sky;
        pass.set_bind_group(0, bind_group, &[*offset]);
        pass.dispatch_workgroups(groups(SPECULAR_SIZE), groups(SPECULAR_SIZE), 6);

        pass.set_pipeline(downsample);
        for (mip, bind_group) in resources.downsample.iter().enumerate() {
            let size = groups(SPECULAR_SIZE >> (mip + 1));
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(size, size, 6);
        }

        pass.set_pipeline(specular);
        for (mip, (bind_group, offset)) in resources.specular.iter().enumerate() {
            let size = groups(SPECULAR_SIZE >> mip);
            pass.set_bind_group(0, bind_group, &[*offset]);
            pass.dispatch_workgroups(size, size, GENERATED_FACES);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BANDS: [Vec3; 3] = [
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
    ];

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).abs().max_element() < 1e-5
    }

    #[test]
    fn gradient_is_the_cosine_weighted_hemisphere() {
        let wall = 2.0 / (3.0 * PI);
        // up: 2/3 sky, 1/3 equator, no ground
        assert!(close(
            trilight_gradient(1.0, BANDS),
            Vec3::new(0.0, 1.0 / 3.0, 2.0 / 3.0)
        ));
        // wall: equator, plus a share of sky and ground
        assert!(close(
            trilight_gradient(0.0, BANDS),
            Vec3::new(wall, 1.0 - 2.0 * wall, wall)
        ));
        // down: 2/3 ground, 1/3 equator, no sky
        assert!(close(
            trilight_gradient(-1.0, BANDS),
            Vec3::new(2.0 / 3.0, 1.0 / 3.0, 0.0)
        ));
        // one colour everywhere stays that colour at every normal
        for y in [-1.0, -0.4, 0.0, 0.3, 1.0] {
            assert!(close(trilight_gradient(y, [Vec3::ONE; 3]), Vec3::ONE));
        }
    }

    #[test]
    fn overlap_matches_a_numeric_integral() {
        // (1/pi) * integral of max(w.y, 0) * max(n . w, 0) over the sphere, n in the xy plane
        for y in [-0.8_f32, -0.2, 0.0, 0.45, 0.9] {
            let n = Vec3::new((1.0 - y * y).sqrt(), y, 0.0);
            let steps = 400;
            let mut sum = 0.0;
            for i in 0..steps {
                let theta = (i as f32 + 0.5) / steps as f32 * PI;
                for j in 0..steps {
                    let phi = (j as f32 + 0.5) / steps as f32 * 2.0 * PI;
                    let w = Vec3::new(
                        theta.sin() * phi.cos(),
                        theta.cos(),
                        theta.sin() * phi.sin(),
                    );
                    sum += w.y.max(0.0) * n.dot(w).max(0.0) * theta.sin();
                }
            }
            let integral = sum * (PI / steps as f32) * (2.0 * PI / steps as f32) / PI;
            assert!(
                (integral - clamped_cosine_overlap(y)).abs() < 2e-3,
                "y {y}: numeric {integral} vs {}",
                clamped_cosine_overlap(y)
            );
        }
    }

    #[test]
    fn cube_faces_point_up_and_down() {
        let centre = Vec2::splat(0.5);
        assert_eq!(cube_texel_dir(centre, 2).y, 1.0);
        assert_eq!(cube_texel_dir(centre, 3).y, -1.0);
        for face in [0, 1, 4, 5] {
            assert_eq!(cube_texel_dir(centre, face).y, 0.0);
            // v runs top down on the side faces
            assert!(cube_texel_dir(Vec2::new(0.5, 0.0), face).y > 0.7);
            assert!(cube_texel_dir(Vec2::new(0.5, 1.0), face).y < -0.7);
        }
    }

    #[test]
    fn ambient_bytes_hold_the_scaled_gradient() {
        let bytes = ambient_cube_bytes(BANDS, 2.0);
        assert_eq!(bytes.len(), (AMBIENT_SIZE * AMBIENT_SIZE * 6 * 8) as usize);
        let texel = |face: u32, x: u32, y: u32| {
            let at = ((face * AMBIENT_SIZE + y) * AMBIENT_SIZE + x) as usize * 8;
            [0, 2, 4, 6]
                .map(|c| half::f16::from_le_bytes([bytes[at + c], bytes[at + c + 1]]).to_f32())
        };
        // the +y face near its centre is about 2/3 sky and 1/3 equator, scaled, with alpha 1, and
        // no ground
        let [r, g, b, a] = texel(2, 3, 3);
        assert!(
            r < 0.01 && (g - 2.0 / 3.0).abs() < 0.1 && (b - 4.0 / 3.0).abs() < 0.1 && a == 1.0,
            "{:?}",
            [r, g, b, a]
        );
        // the -y face near its centre is about 2/3 ground and 1/3 equator, and no sky
        let [r, g, b, a] = texel(3, 3, 3);
        assert!(
            (r - 4.0 / 3.0).abs() < 0.1 && (g - 2.0 / 3.0).abs() < 0.1 && b < 0.01 && a == 1.0,
            "{:?}",
            [r, g, b, a]
        );
        // a side face's top row takes far more sky than ground, its bottom row the reverse
        let (top, bottom) = (texel(0, 4, 0), texel(0, 4, AMBIENT_SIZE - 1));
        assert!(top[2] > 0.5 && top[2] > 4.0 * top[0], "{top:?}");
        assert!(bottom[0] > 0.5 && bottom[0] > 4.0 * bottom[2], "{bottom:?}");
    }
}
