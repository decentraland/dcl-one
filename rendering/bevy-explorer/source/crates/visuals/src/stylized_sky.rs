//! The sky: the stylized skybox preset, drawn into the atmosphere cube by
//! `shaders/stylized_sky.wgsl`. The sky colour is a lookup by day phase and elevation, baked
//! from the preset's four gradients (`sky_preset::bake_sky_lut`); a scene's own sky colours
//! blend over it. The sun or the moon is a disc in a halo on that sky, and three cloud strips
//! wrap the dome over them; the cube's alpha is how much the clouds hide.
//!
//! The authored colours go to the shader in the space the preset was tuned in: the lookup and
//! the cloud colours as authored (sRGB values), the disc colour linearised as a material colour
//! (`gamma_to_linear`). The shader composites on those values and decodes its result from sRGB to
//! linear as its last step.
//!
//! The screen shows the sky per pixel through `StylizedSkyMaterial`, which takes the place of
//! bevy_atmosphere's skybox material: the cube's texels are too coarse for the cloud strips'
//! detail. The compute model draws the same sky into the atmosphere cube for the reflections,
//! the ambient and the star pass. Both are written unscaled, in the units the preset's light and
//! exposure values are tuned for.

use bevy::{
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_asset::{RenderAssetUsages, RenderAssets},
        render_resource::{
            binding_types::{sampler, texture_2d, texture_2d_array, uniform_buffer},
            encase, AsBindGroup, BindGroup, BindGroupEntries, BindGroupLayout,
            BindGroupLayoutEntries, BufferInitDescriptor, BufferUsages, CompareFunction,
            ComputePipelineDescriptor, Extent3d, PipelineCache, RenderPipelineDescriptor,
            SamplerBindingType, ShaderRef, ShaderStages, SpecializedMeshPipelineError,
            TextureDimension, TextureFormat, TextureSampleType,
        },
        renderer::RenderDevice,
        texture::{FallbackImage, GpuImage},
        RenderApp,
    },
};
use bevy_atmosphere::{
    model::AtmosphereModelMetadata, pipeline::AtmosphereImageBindGroupLayout,
    plugin::AtmosphereSkyBox, prelude::AtmosphereModel, skybox::SkyBoxMaterial,
    system_param::AtmosphereMut,
};

use scene_runner::update_world::lights::{celestial_swap, moon_progress};

use crate::sky_preset::{
    bake_sky_lut, evaluate_by_phase, sample_keys, sample_ramp, time_to_phase, CLOUD_FADES,
    CLOUD_LAYERS, CLOUD_LIT_RAMP, CLOUD_PARAMS, CLOUD_SHADOW_RAMP, HORIZON_NOISE, MOON_COLOR_RAMP,
    MOON_DISC_SIZE, MOON_MASK_OFFSET, MOON_MASK_SIZE, SKY_LUT_ROWS, SKY_LUT_WIDTH, SUN_COLOR_RAMP,
    SUN_RADIANCE_CURVE, SUN_RADIANCE_INTENSITY_CURVE, SUN_SIZE_CURVE,
};

const SHADER_PATH: &str = "embedded://shaders/stylized_sky.wgsl";
const HORIZON_NOISE_PATH: &str = "embedded://sky/horizon_noise.png";
/// One strip a cloud layer, in the order of `CLOUD_LAYERS`.
const CLOUD_STRIP_PATHS: [&str; 3] = [
    "embedded://sky/clouds_overhead.png",
    "embedded://sky/clouds_low.png",
    "embedded://sky/clouds_cumulus.png",
];
/// Turns a second a strip of speed 1 scrolls.
const CLOUD_SCROLL_TURNS: f32 = 0.0004;

/// The stylized sky model. `apply_global_light` writes it every frame.
#[derive(Reflect, Debug, Clone)]
pub struct StylizedSky {
    /// Frame counter: each frame redraws one texel in 64 of the cube, in a dither pattern.
    pub tick: u32,
    /// Seconds, driving the scrolls.
    pub time: f32,
    /// The day phase: 0 and 1 night, 0.25 sunrise, 0.5 day, 0.75 sunset.
    pub phase: f32,
    /// World direction toward the drawn disc, the sun or the moon.
    pub body_direction: Vec3,
    pub disc_opacity: f32,
    pub disc_size: f32,
    /// The size of the circle that cuts the moon's disc to a crescent; 0 leaves a disc whole.
    pub mask_size: f32,
    pub disc_color: Vec3,
    /// The halo around the disc: radius, intensity.
    pub halo: Vec2,
    pub cloud_lit: Vec3,
    pub cloud_shadow: Vec3,
    /// Scales every strip's opacity.
    pub cloud_opacity: f32,
    /// How far each cloud strip has scrolled, in turns.
    pub cloud_scroll: Vec3,
    /// How much the body behind a cloud lights it through.
    pub backlight_weight: f32,
    /// Scene-authored linear sky colours; alpha flags whether each band is overridden.
    pub scene_zenith: Vec4,
    pub scene_horizon: Vec4,
    pub scene_nadir: Vec4,
    pub scene_rim: Vec4,
    pub scene_sun_visible: f32,
    /// The sky lookup (`sky_lut_image`).
    pub lut: Handle<Image>,
    /// The horizon noise (`load_horizon_noise`).
    pub noise: Handle<Image>,
    /// The cloud strips, one layer each (`stack_cloud_strips`). No cloud is drawn without it.
    pub clouds: Handle<Image>,
}

impl Default for StylizedSky {
    fn default() -> Self {
        Self {
            tick: 0,
            time: 0.0,
            phase: 0.5,
            body_direction: Vec3::Y,
            disc_opacity: 0.0,
            disc_size: 0.0,
            mask_size: 0.0,
            disc_color: Vec3::ONE,
            halo: Vec2::ZERO,
            cloud_lit: Vec3::ONE,
            cloud_shadow: Vec3::ONE,
            cloud_opacity: 1.0,
            cloud_scroll: Vec3::ZERO,
            backlight_weight: 0.0,
            scene_zenith: Vec4::ZERO,
            scene_horizon: Vec4::ZERO,
            scene_nadir: Vec4::ZERO,
            scene_rim: Vec4::ZERO,
            scene_sun_visible: 1.0,
            lut: Handle::default(),
            noise: Handle::default(),
            clouds: Handle::default(),
        }
    }
}

mod uniform {
    #![expect(
        dead_code,
        reason = "the ShaderType derive emits a never-called check fn per field"
    )]

    use bevy::{prelude::*, render::render_resource::ShaderType};

    /// `StylizedSky` as the shader reads it, plus the preset's constants.
    #[derive(ShaderType, Clone)]
    pub struct StylizedSkyUniform {
        pub tick: u32,
        pub time: f32,
        pub phase: f32,
        pub body_direction: Vec3,
        pub disc_opacity: f32,
        pub disc_color: Vec3,
        pub disc_size: f32,
        pub halo: Vec2,
        /// Where the crescent's mask sits off the disc's centre, in the body's own frame.
        pub mask_offset: Vec2,
        pub cloud_opacity: f32,
        pub backlight_weight: f32,
        pub cloud_lit: Vec3,
        pub scene_sun_visible: f32,
        pub cloud_shadow: Vec3,
        pub mask_size: f32,
        pub horizon_noise: Vec4,
        pub scene_zenith: Vec4,
        pub scene_horizon: Vec4,
        pub scene_nadir: Vec4,
        pub scene_rim: Vec4,
        /// Ramp knee, highlight strength, highlight threshold, highlight falloff.
        pub cloud_params: Vec4,
        /// Zenith fade start and end, occlusion start and end.
        pub cloud_fades: Vec4,
        /// Per strip: stretch and offset down the dome, repeats and offset around it.
        pub cloud_layers: [Vec4; 3],
        /// Per strip: strength, opacity, flow at the phase, scroll in turns.
        pub cloud_looks: [Vec4; 3],
    }
}
use uniform::StylizedSkyUniform;

impl From<&StylizedSky> for StylizedSkyUniform {
    fn from(sky: &StylizedSky) -> Self {
        Self {
            tick: sky.tick,
            time: sky.time,
            phase: sky.phase,
            body_direction: sky.body_direction,
            disc_opacity: sky.disc_opacity,
            disc_color: sky.disc_color,
            disc_size: sky.disc_size,
            halo: sky.halo,
            mask_offset: MOON_MASK_OFFSET,
            cloud_opacity: sky.cloud_opacity,
            backlight_weight: sky.backlight_weight,
            cloud_lit: sky.cloud_lit,
            scene_sun_visible: sky.scene_sun_visible,
            cloud_shadow: sky.cloud_shadow,
            mask_size: sky.mask_size,
            horizon_noise: HORIZON_NOISE,
            scene_zenith: sky.scene_zenith,
            scene_horizon: sky.scene_horizon,
            scene_nadir: sky.scene_nadir,
            scene_rim: sky.scene_rim,
            cloud_params: CLOUD_PARAMS,
            cloud_fades: CLOUD_FADES,
            cloud_layers: CLOUD_LAYERS.map(|layer| {
                Vec4::new(
                    layer.stretch_v,
                    layer.offset_v,
                    layer.tiling_u,
                    layer.offset_u,
                )
            }),
            cloud_looks: std::array::from_fn(|index| {
                let layer = &CLOUD_LAYERS[index];
                Vec4::new(
                    layer.strength,
                    layer.opacity,
                    evaluate_by_phase(layer.flow_by_phase, sky.phase),
                    sky.cloud_scroll[index],
                )
            }),
        }
    }
}

/// Where a cloud strip is read for a direction, with U in repeats of the strip and V up the
/// image. `v_dome` runs from 0 at the zenith to 1 at the dome's rim,
/// 113.6 degrees below it. The shader does the same; this copy is what the tests check.
#[cfg(test)]
pub fn cloud_uv(
    layer: &crate::sky_preset::CloudLayer,
    azimuth_turns: f32,
    v_dome: f32,
    scroll_turns: f32,
) -> Vec2 {
    let v_tex = (v_dome + layer.offset_v - 0.5) / layer.stretch_v.max(1e-3) + 0.5;
    let u = azimuth_turns * layer.tiling_u + layer.offset_u - scroll_turns;
    Vec2::new(u, (1.0 - v_tex).clamp(0.0, 1.0))
}

/// The clouds' lit and shadowed colours at `phase`. A scene's colour is the lit one, and its
/// shadow is the same hue, darker by as much as the preset's shadow is darker than its lit
/// colour (Rec. 709 luminance) and never brighter.
pub fn cloud_colors(phase: f32, scene: Option<Vec3>) -> (Vec3, Vec3) {
    let lit = sample_ramp(&CLOUD_LIT_RAMP, phase);
    let shadow = sample_ramp(&CLOUD_SHADOW_RAMP, phase);
    match scene {
        Some(color) => {
            let luminance = |colour: Vec3| colour.dot(Vec3::new(0.2126, 0.7152, 0.0722));
            let contrast = (luminance(shadow) / luminance(lit).max(1e-4)).min(1.0);
            (color, color * contrast)
        }
        None => (lit, shadow),
    }
}

/// Turns a second each strip scrolls: the preset's speeds, or a scene's radians a second for
/// all three.
pub fn cloud_scroll_rates(scene_speed: Option<f32>) -> Vec3 {
    match scene_speed {
        Some(speed) => Vec3::splat(speed / std::f32::consts::TAU),
        None => Vec3::from_array(CLOUD_LAYERS.map(|layer| layer.speed * CLOUD_SCROLL_TURNS)),
    }
}

/// The disc the sky draws at an hour: which body, and its shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscParams {
    pub moon_active: bool,
    pub opacity: f32,
    pub size: f32,
    /// The size of the circle that cuts the disc to a crescent; 0 leaves it whole.
    pub mask_size: f32,
    /// The halo's radius and intensity.
    pub halo: Vec2,
}

/// The disc at `day_fraction`, following the directional light along its path. The moon is the
/// body once the light has swung past halfway onto its arc, and the disc is hidden
/// mid-swing. A direction a scene gives the sun (`scene_direction`) keeps the sun's whole disc
/// there all night.
pub fn disc_params(day_fraction: f32, scene_direction: bool) -> DiscParams {
    let (moon_weight, dip) = celestial_swap(day_fraction);
    let moon_active = moon_weight > 0.5 && !scene_direction;
    DiscParams {
        moon_active,
        opacity: 1.0 - dip,
        // the moon keeps one size, so its crescent keeps one shape
        size: if moon_active {
            MOON_DISC_SIZE
        } else {
            sample_keys(&SUN_SIZE_CURVE, day_fraction)
        },
        mask_size: if moon_active { MOON_MASK_SIZE } else { 0.0 },
        halo: Vec2::new(
            sample_keys(&SUN_RADIANCE_CURVE, day_fraction),
            sample_keys(&SUN_RADIANCE_INTENSITY_CURVE, day_fraction),
        ),
    }
}

/// A material colour to linear, as the preset's disc ramps are authored for: the sRGB curve below
/// 1, a 2.2 power from 1 up, where the ramps' bright keys sit.
pub fn gamma_to_linear(colour: Vec3) -> Vec3 {
    let channel = |value: f32| {
        if value <= 0.04045 {
            value / 12.92
        } else if value < 1.0 {
            ((value + 0.055) / 1.055).powf(2.4)
        } else {
            value.powf(2.2)
        }
    };
    Vec3::new(channel(colour.x), channel(colour.y), channel(colour.z))
}

/// The sky shader's last step, sRGB to linear: the sRGB curve, continued past 1 on its power
/// branch.
#[cfg(test)]
pub fn sky_rgb_to_linear(colour: Vec3) -> Vec3 {
    let channel = |value: f32| {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055)
                .abs()
                .max(1.192_092_9e-7)
                .powf(2.4)
        }
    };
    Vec3::new(channel(colour.x), channel(colour.y), channel(colour.z))
}

/// The colour of the disc and its halo at `day_fraction`: the moon's ramp along its rise to set
/// while it is the body, or the sun's ramp at the phase, linearised as a material colour
/// (`gamma_to_linear`) before the sky shader decodes it again. A scene's sun
/// colour tints it: its hue at that preset's brightness, times its own brightness above 1.
pub fn disc_color(disc: &DiscParams, day_fraction: f32, scene: Option<Vec3>) -> Vec3 {
    let preset = gamma_to_linear(if disc.moon_active {
        sample_ramp(
            &MOON_COLOR_RAMP,
            moon_progress(day_fraction).clamp(0.0, 1.0),
        )
    } else {
        sample_ramp(&SUN_COLOR_RAMP, time_to_phase(day_fraction))
    });
    match scene {
        None => preset,
        Some(authored) => {
            let brightest = authored.max_element();
            authored / brightest.max(1e-4) * preset.max_element() * brightest.max(1.0)
        }
    }
}

impl StylizedSky {
    pub fn set_disc(&mut self, disc: &DiscParams) {
        self.disc_opacity = disc.opacity;
        self.disc_size = disc.size;
        self.mask_size = disc.mask_size;
        self.halo = disc.halo;
    }
}

impl bevy_atmosphere::model::Atmospheric for StylizedSky {
    fn as_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        images: &RenderAssets<GpuImage>,
        fallback_image: &FallbackImage,
    ) -> BindGroup {
        let lut = images.get(&self.lut).unwrap_or(&fallback_image.d2);
        let noise = images.get(&self.noise).unwrap_or(&fallback_image.d2);
        // an unset handle is the default image, which is no array
        let clouds = images
            .get(&self.clouds)
            .filter(|image| image.size.depth_or_array_layers > 1);
        let mut values = StylizedSkyUniform::from(self);
        if clouds.is_none() {
            // the stand-in is opaque white, which would be cloud all over
            values.cloud_opacity = 0.0;
        }
        let clouds = clouds.unwrap_or(&fallback_image.d2_array);
        let mut uniform = encase::UniformBuffer::new(Vec::new());
        uniform.write(&values).unwrap();
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("stylized_sky_uniform"),
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            contents: uniform.as_ref(),
        });
        render_device.create_bind_group(
            "stylized_sky_bind_group",
            layout,
            &BindGroupEntries::sequential((
                buffer.as_entire_binding(),
                &lut.texture_view,
                &lut.sampler,
                &noise.texture_view,
                &noise.sampler,
                &clouds.texture_view,
                &clouds.sampler,
            )),
        )
    }

    fn clone_dynamic(&self) -> Box<dyn bevy_atmosphere::model::Atmospheric> {
        Box::new(self.clone())
    }

    fn as_reflect(&self) -> &dyn Reflect {
        self
    }

    fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
        self
    }
}

impl bevy_atmosphere::model::RegisterAtmosphereModel for StylizedSky {
    fn register(app: &mut App) {
        use std::any::TypeId;

        app.register_type::<Self>();
        let shader = app.world().resource::<AssetServer>().load(SHADER_PATH);

        let render_app = app.sub_app_mut(RenderApp);
        let render_device = render_app.world().resource::<RenderDevice>();
        let AtmosphereImageBindGroupLayout(image_bind_group_layout) = render_app
            .world()
            .resource::<AtmosphereImageBindGroupLayout>()
            .clone();
        let bind_group_layout = Self::bind_group_layout(render_device);
        let pipeline = render_app
            .world_mut()
            .resource_mut::<PipelineCache>()
            .queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("stylized_sky_compute_pipeline".into()),
                layout: vec![bind_group_layout.clone(), image_bind_group_layout],
                push_constant_ranges: vec![],
                shader,
                shader_defs: vec![],
                entry_point: "main".into(),
                zero_initialize_workgroup_memory: false,
            });
        app.world_mut()
            .resource_mut::<AppTypeRegistry>()
            .write()
            .get_mut(TypeId::of::<Self>())
            .expect("Type not registered")
            .insert(AtmosphereModelMetadata {
                id: TypeId::of::<Self>(),
                bind_group_layout,
                pipeline,
            });
    }

    fn bind_group_layout(render_device: &RenderDevice) -> BindGroupLayout {
        let filtered = TextureSampleType::Float { filterable: true };
        render_device.create_bind_group_layout(
            "stylized_sky_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    uniform_buffer::<StylizedSkyUniform>(false),
                    // the sky lookup
                    texture_2d(filtered),
                    sampler(SamplerBindingType::Filtering),
                    // the horizon noise
                    texture_2d(filtered),
                    sampler(SamplerBindingType::Filtering),
                    // the cloud strips
                    texture_2d_array(filtered),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        )
    }
}

/// The sky on the screen: `stylized_sky.wgsl`'s fragment entry, per pixel, from the same model
/// as the cube. A scene's panorama replaces it.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
#[bind_group_data(StylizedSkyMaterialKey)]
pub struct StylizedSkyMaterial {
    #[uniform(0)]
    sky: StylizedSkyUniform,
    #[texture(1)]
    #[sampler(2)]
    lut: Handle<Image>,
    /// Until it loads, the stand-in: no lift at the horizon.
    #[texture(3)]
    #[sampler(4)]
    noise: Option<Handle<Image>>,
    /// Until the strips are stacked, the stand-in, with the clouds' opacity at 0.
    #[texture(5, dimension = "2d_array")]
    #[sampler(6)]
    clouds: Option<Handle<Image>>,
    /// An equirectangular image shown instead of the sky. Its centre column faces -Z, its top
    /// row is the zenith.
    #[texture(7)]
    #[sampler(8)]
    pub panorama: Option<Handle<Image>>,
}

impl Default for StylizedSkyMaterial {
    fn default() -> Self {
        Self {
            sky: StylizedSkyUniform::from(&StylizedSky::default()),
            lut: Handle::default(),
            noise: None,
            clouds: None,
            panorama: None,
        }
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct StylizedSkyMaterialKey {
    panorama: bool,
}

impl From<&StylizedSkyMaterial> for StylizedSkyMaterialKey {
    fn from(material: &StylizedSkyMaterial) -> Self {
        Self {
            panorama: material.panorama.is_some(),
        }
    }
}

impl Material for StylizedSkyMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // as bevy_atmosphere's skybox: the sky's pixels keep the cleared depth, so the stars can
        // tell the sky from geometry, while the main pass still tests it against the geometry
        if let Some(depth_stencil) = &mut descriptor.depth_stencil {
            depth_stencil.depth_write_enabled = false;
            depth_stencil.depth_compare = CompareFunction::GreaterEqual;
        }
        if let Some(fragment) = &mut descriptor.fragment {
            fragment.shader_defs.push("SKY_MATERIAL".into());
            if key.bind_group_data.panorama {
                fragment.shader_defs.push("PANORAMA".into());
            }
        }
        Ok(())
    }
}

/// The one `StylizedSkyMaterial` every skybox shows.
#[derive(Resource, Clone)]
pub struct StylizedSkyMaterialHandle(pub Handle<StylizedSkyMaterial>);

/// Draws the sky on the screen per pixel: the material, on every skybox bevy_atmosphere spawns,
/// kept in step with the model.
pub struct StylizedSkyMaterialPlugin;

impl Plugin for StylizedSkyMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StylizedSkyMaterial> {
            prepass_enabled: false,
            ..default()
        });
        let handle = app
            .world_mut()
            .resource_mut::<Assets<StylizedSkyMaterial>>()
            .add(StylizedSkyMaterial::default());
        app.insert_resource(StylizedSkyMaterialHandle(handle))
            .add_observer(use_stylized_skybox)
            .add_systems(PostUpdate, update_sky_material);
    }
}

/// Gives each skybox bevy_atmosphere spawns, which shows the cube, the per-pixel sky instead.
/// An observer, so the swap lands in the command flush that spawns the skybox and no frame
/// renders the cube on the screen.
fn use_stylized_skybox(
    trigger: Trigger<OnAdd, AtmosphereSkyBox>,
    mut commands: Commands,
    material: Res<StylizedSkyMaterialHandle>,
) {
    commands
        .entity(trigger.target())
        .remove::<MeshMaterial3d<SkyBoxMaterial>>()
        .insert(MeshMaterial3d(material.0.clone()));
}

/// Copies the model into the material: what the compute model reads, so the screen and the cube
/// show one sky. Every frame, on purpose: the sky's time moves every frame, so the material is
/// re-prepared (a new uniform buffer and bind group) every frame anyway. It is one small uniform
/// on one material.
fn update_sky_material(
    atmosphere: Res<AtmosphereModel>,
    images: Res<Assets<Image>>,
    handle: Res<StylizedSkyMaterialHandle>,
    mut materials: ResMut<Assets<StylizedSkyMaterial>>,
) {
    let Some(sky) = atmosphere.to_ref::<StylizedSky>() else {
        return;
    };
    let Some(material) = materials.get_mut(&handle.0) else {
        return;
    };
    material.lut = sky.lut.clone();
    material.noise = images.contains(&sky.noise).then(|| sky.noise.clone());
    // the stacked strips live on the GPU only, so they are known by their handle
    material.clouds = (sky.clouds != Handle::default()).then(|| sky.clouds.clone());
    material.sky = StylizedSkyUniform::from(sky);
    if material.clouds.is_none() {
        // the stand-in is opaque white, which would be cloud all over
        material.sky.cloud_opacity = 0.0;
    }
}

/// The sky lookup as the shader samples it: `SKY_LUT_WIDTH` by `SKY_LUT_ROWS`, blending
/// between elevations and between neighbouring phases, and holding its edges. Half floats,
/// which every device filters.
pub fn sky_lut_image() -> Image {
    let mut image = Image::new(
        Extent3d {
            width: SKY_LUT_WIDTH as u32,
            height: SKY_LUT_ROWS as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bake_sky_lut()
            .into_iter()
            .flatten()
            .flat_map(|channel| half::f16::from_f32(channel).to_le_bytes())
            .collect(),
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        label: Some("sky_lut".to_owned()),
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// The noise that lifts the lookup near the horizon: data, not colour, tiling both ways.
pub fn load_horizon_noise(asset_server: &AssetServer) -> Handle<Image> {
    asset_server.load_with_settings(HORIZON_NOISE_PATH, |settings: &mut ImageLoaderSettings| {
        settings.is_srgb = false;
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            label: Some("horizon_noise".to_owned()),
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    })
}

/// The cloud strips while they load, in the order of `CLOUD_LAYERS`.
#[derive(Resource)]
pub struct CloudStrips([Handle<Image>; 3]);

/// The strips as authored: data, not colour, each wide image one turn of the sky. They
/// stay off the GPU; `stack_cloud_strips` makes the texture the shader reads.
pub fn load_cloud_strips(asset_server: &AssetServer) -> CloudStrips {
    CloudStrips(CLOUD_STRIP_PATHS.map(|path| {
        asset_server.load_with_settings(path, |settings: &mut ImageLoaderSettings| {
            settings.is_srgb = false;
            settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
        })
    }))
}

/// Once the three strips have loaded, gives the sky one texture with a layer for each:
/// repeating around the sky, held at the top and bottom rows, which are clear.
pub fn stack_cloud_strips(
    mut commands: Commands,
    strips: Res<CloudStrips>,
    mut images: ResMut<Assets<Image>>,
    mut sky: AtmosphereMut<StylizedSky>,
) {
    let Some(layers) = strips
        .0
        .iter()
        .map(|strip| images.get(strip))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let mut stacked = Image::new(
        Extent3d {
            width: layers[0].width(),
            height: layers[0].height(),
            depth_or_array_layers: layers.len() as u32,
        },
        TextureDimension::D2,
        layers
            .iter()
            .flat_map(|layer| layer.data.iter().flatten().copied())
            .collect(),
        layers[0].texture_descriptor.format,
        RenderAssetUsages::RENDER_WORLD,
    );
    stacked.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        label: Some("cloud_strips".to_owned()),
        address_mode_u: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    sky.clouds = images.add(stacked);
    // the last handles to the three sources
    commands.remove_resource::<CloudStrips>();
}

/// What the GPU tests of the sky share: an app that draws the model into a small cube, and the
/// cube read back.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use bevy::render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::TextureUsages,
    };
    use bevy_atmosphere::{model::AddAtmosphereModel, pipeline::AtmosphereImage, prelude::*};
    use pipeline_test::{compile_pipelines, headless_app_without_embedded_assets};

    #[derive(Resource, Default)]
    struct ReadBack(Option<Vec<u8>>);

    fn tick(mut sky: AtmosphereMut<StylizedSky>) {
        sky.tick = sky.tick.wrapping_add(1);
    }

    /// A headless app with the sky model and a cube of `resolution` texels a side, a dither
    /// step further every update. Add the other plugins under test, then `start_sky`.
    pub(crate) fn sky_test_app(resolution: u32) -> App {
        let mut app = headless_app_without_embedded_assets();
        app.init_resource::<ReadBack>()
            .insert_resource(AtmosphereSettings {
                resolution,
                ..default()
            })
            .insert_resource(AtmosphereModel::new(StylizedSky::default()))
            .add_plugins(AtmospherePlugin)
            .add_systems(Update, tick);
        app
    }

    /// Finishes the app as the client does, gives the model its lookup and noise and returns
    /// the sky cube, readable. The sky stays clear of cloud until `start_clouds`.
    pub(crate) fn start_sky(app: &mut App) -> Handle<Image> {
        app.add_plugins(assets::EmbedAssetsPlugin);
        app.finish();
        app.cleanup();
        app.sub_app_mut(RenderApp)
            .init_resource::<AtmosphereImageBindGroupLayout>();
        app.add_atmosphere_model::<StylizedSky>();
        let cube = app.world().resource::<AtmosphereImage>().handle.clone();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        images.get_mut(&cube).unwrap().texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let lut = images.add(sky_lut_image());
        let noise = load_horizon_noise(app.world().resource::<AssetServer>());
        let mut model = app.world_mut().resource_mut::<AtmosphereModel>();
        let sky = model.to_mut::<StylizedSky>().unwrap();
        sky.lut = lut;
        sky.noise = noise;
        cube
    }

    /// Loads the cloud strips, as the client does at startup.
    pub(crate) fn start_clouds(app: &mut App) {
        let strips = load_cloud_strips(app.world().resource::<AssetServer>());
        app.insert_resource(strips).add_systems(
            Update,
            stack_cloud_strips.run_if(resource_exists::<CloudStrips>),
        );
    }

    /// Changes the model and updates until the whole cube is drawn from it: the pipeline
    /// compiled, the noise and any cloud strips loaded, then every dither step.
    pub(crate) fn draw_sky(app: &mut App, change: impl FnOnce(&mut StylizedSky)) {
        let mut model = app.world_mut().resource_mut::<AtmosphereModel>();
        let sky = model.to_mut::<StylizedSky>().unwrap();
        change(sky);
        let noise = sky.noise.clone();
        compile_pipelines(app);
        let start = std::time::Instant::now();
        while !app.world().resource::<AssetServer>().is_loaded(&noise)
            || app.world().contains_resource::<CloudStrips>()
        {
            app.update();
            assert!(
                start.elapsed().as_secs() < 30,
                "the sky's textures never loaded"
            );
        }
        for _ in 0..80 {
            app.update();
        }
    }

    /// Every texel of a cube of `size` texels a side: faces in order, rows top down. Panics on
    /// a channel that is not finite.
    pub(crate) fn read_cube(
        app: &mut App,
        handle: &Handle<Image>,
        size: usize,
        label: &str,
    ) -> Vec<[f32; 4]> {
        app.world_mut().resource_mut::<ReadBack>().0 = None;
        let entity = app
            .world_mut()
            .spawn(Readback::texture(handle.clone()))
            .observe(
                |event: Trigger<ReadbackComplete>, mut read: ResMut<ReadBack>| {
                    read.0 = Some(event.event().0.clone());
                },
            )
            .id();
        let start = std::time::Instant::now();
        let bytes = loop {
            app.update();
            if let Some(bytes) = app.world_mut().resource_mut::<ReadBack>().0.take() {
                break bytes;
            }
            assert!(
                start.elapsed().as_secs() < 30,
                "{label}: GPU readback timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        app.world_mut().despawn(entity);
        // readback rows are aligned to 256 bytes
        let stride = (size * 8).div_ceil(256) * 256;
        assert_eq!(bytes.len(), stride * size * 6, "{label}: six cubemap faces");
        let mut texels = Vec::with_capacity(size * size * 6);
        for face in 0..6 {
            for y in 0..size {
                for x in 0..size {
                    let offset = (face * size + y) * stride + x * 8;
                    let channels: [f32; 4] = std::array::from_fn(|channel| {
                        let i = offset + channel * 2;
                        half::f16::from_le_bytes([bytes[i], bytes[i + 1]]).to_f32()
                    });
                    assert!(
                        channels.iter().all(|channel| channel.is_finite()),
                        "{label}: nonfinite texel face={face} x={x} y={y}: {channels:?}"
                    );
                    texels.push(channels);
                }
            }
        }
        texels
    }

    pub(crate) fn texel(
        cube: &[[f32; 4]],
        size: usize,
        face: usize,
        x: usize,
        y: usize,
    ) -> [f32; 4] {
        cube[(face * size + y) * size + x]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        test_support::{draw_sky, read_cube, sky_test_app, start_clouds, start_sky, texel},
        *,
    };
    use crate::sky_preset::{
        day_fraction, sample_keys, sample_ramp, time_to_phase, CLOUD_LAYERS, CLOUD_LIT_RAMP,
        CLOUD_SHADOW_RAMP, MOON_COLOR_RAMP, MOON_DISC_SIZE, MOON_MASK_SIZE, SUN_COLOR_RAMP,
        SUN_RADIANCE_CURVE, SUN_RADIANCE_INTENSITY_CURVE, SUN_SIZE_CURVE,
    };
    use scene_runner::update_world::lights::{moon_progress, MOONRISE, MOON_SWAP_DURATION};
    use std::f32::consts::TAU;

    const SIZE: usize = 128;
    // cube faces, and the texel whose ray runs along the face's axis
    const UP: usize = 2;
    const NORTH: usize = 4;
    const CENTRE: usize = SIZE / 2;

    #[test]
    fn colour_conversions() {
        let close = |a: Vec3, b: Vec3| a.abs_diff_eq(b, 1e-5);
        // the sky's last step: the sRGB curve, continued past 1 on its power branch
        assert!(close(
            sky_rgb_to_linear(Vec3::new(0.02, 0.5, 2.0)),
            Vec3::new(
                0.02 / 12.92,
                ((0.5f32 + 0.055) / 1.055).powf(2.4),
                ((2.0f32 + 0.055) / 1.055).powf(2.4)
            )
        ));
        // a material colour: a 2.2 power from 1 up
        assert!(close(
            gamma_to_linear(Vec3::new(0.02, 0.5, 2.0)),
            Vec3::new(
                0.02 / 12.92,
                ((0.5f32 + 0.055) / 1.055).powf(2.4),
                2.0f32.powf(2.2)
            )
        ));
    }

    #[test]
    fn the_uniform_carries_the_preset_horizon_noise() {
        let uniform = StylizedSkyUniform::from(&StylizedSky::default());
        assert_eq!(uniform.horizon_noise, Vec4::new(0.1, 5.0, 10.0, 0.002));
    }

    #[test]
    fn the_lookup_image_is_the_baked_rows() {
        let image = sky_lut_image();
        assert_eq!((image.width(), image.height()), (256, 5));
        let data = image.data.as_ref().unwrap();
        assert_eq!(data.len(), 256 * 5 * 8);
        // the last texel of the day row, the zenith at noon, to a half float's precision
        let at = (2 * 256 + 255) * 8;
        let blue = half::f16::from_le_bytes([data[at + 4], data[at + 5]]).to_f32();
        assert!((blue - bake_sky_lut()[2 * 256 + 255][2]).abs() < 1e-3);
    }

    #[test]
    fn cloud_uv_wraps_at_the_seam() {
        for layer in &CLOUD_LAYERS {
            for scroll in [0.0, 0.37] {
                let east = cloud_uv(layer, 0.5 - 1e-4, 0.6, scroll).x.rem_euclid(1.0);
                let west = cloud_uv(layer, -0.5 + 1e-4, 0.6, scroll).x.rem_euclid(1.0);
                // around the strip: 0.9999 and 0.0001 are neighbours
                let apart = (east - west).abs();
                assert!(
                    apart.min(1.0 - apart) < 1e-3,
                    "{layer:?} scrolled {scroll}: {east} and {west}"
                );
            }
        }
    }

    #[test]
    fn cloud_uv_hangs_each_strip_on_its_band() {
        // the low strip: its top row 0.53 down the dome, its bottom row 0.81 down
        let low = &CLOUD_LAYERS[1];
        assert!((cloud_uv(low, 0.0, 0.53, 0.0).y - 1.0).abs() < 1e-4);
        assert!((cloud_uv(low, 0.0, 0.67, 0.0).y - 0.5).abs() < 1e-4);
        assert!(cloud_uv(low, 0.0, 0.81, 0.0).y.abs() < 1e-4);
        // and held past both
        assert_eq!(cloud_uv(low, 0.0, 0.2, 0.0).y, 1.0);
        assert_eq!(cloud_uv(low, 0.0, 0.95, 0.0).y, 0.0);
        // a quarter turn of scroll is a quarter of the strip, against its offset
        assert!((cloud_uv(low, 0.25, 0.6, 0.25).x - low.offset_u).abs() < 1e-6);
    }

    #[test]
    fn scene_cloud_colour_keeps_the_ramp_contrast() {
        let noon = time_to_phase(day_fraction(12.0));
        let lit = sample_ramp(&CLOUD_LIT_RAMP, noon);
        let shadow = sample_ramp(&CLOUD_SHADOW_RAMP, noon);
        assert_eq!(cloud_colors(noon, None), (lit, shadow));

        let authored = Vec3::new(0.2, 3.0, 0.5);
        let mut sky = StylizedSky::default();
        (sky.cloud_lit, sky.cloud_shadow) = cloud_colors(noon, Some(authored));
        let uniform = StylizedSkyUniform::from(&sky);
        assert_eq!(uniform.cloud_lit, authored);
        // the authored hue, darkened by how much darker the preset's shadow is than its lit
        let luminance = |colour: Vec3| colour.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let expected = authored * (luminance(shadow) / luminance(lit).max(1e-4)).min(1.0);
        assert!(
            (uniform.cloud_shadow - expected).abs().max_element() < 1e-6,
            "{} for {expected}",
            uniform.cloud_shadow
        );

        // the shadow is never brighter than the lit colour, in any channel, at any anchor. At
        // sunrise the preset's own shadow is far bluer than its lit colour
        for phase in [0.0, 0.25, 0.5, 0.75] {
            for authored in [Vec3::ONE, authored] {
                let (lit, shadow) = cloud_colors(phase, Some(authored));
                assert_eq!(lit, authored);
                assert!(
                    shadow.cmple(lit).all(),
                    "phase {phase}: shadow {shadow} over lit {lit}"
                );
            }
        }
    }

    #[test]
    fn a_scene_speed_drives_every_strip() {
        // the preset's speeds, each strip its own
        let preset = cloud_scroll_rates(None);
        for (layer, rate) in CLOUD_LAYERS.iter().zip(preset.to_array()) {
            assert_eq!(rate, layer.speed * 0.0004);
        }
        // a scene's radians per second, as turns, and backwards when negative
        assert_eq!(cloud_scroll_rates(Some(TAU)), Vec3::ONE);
        assert_eq!(cloud_scroll_rates(Some(-TAU / 2.0)), Vec3::splat(-0.5));
        assert_eq!(cloud_scroll_rates(Some(0.0)), Vec3::ZERO);
    }

    #[test]
    fn the_uniform_carries_the_cloud_layers() {
        let mut sky = StylizedSky {
            phase: 0.0,
            cloud_scroll: Vec3::new(0.1, 0.2, 0.3),
            ..default()
        };
        let night = StylizedSkyUniform::from(&sky);
        assert_eq!(night.cloud_params, Vec4::new(0.15, 2.0, 0.6, 3.0));
        assert_eq!(night.cloud_fades, Vec4::new(0.97, 1.0, 0.35, 0.9));
        // stretch, offset down the dome, repeats, offset around it
        assert_eq!(night.cloud_layers[1], Vec4::new(0.28, -0.17, 1.0, 0.5));
        // strength, opacity, flow at the phase, scroll: the cumulus only flows by day
        assert_eq!(night.cloud_looks[0], Vec4::new(1.2, 1.0, 1.0, 0.1));
        assert_eq!(night.cloud_looks[2], Vec4::new(1.2, 1.0, 0.0, 0.3));
        sky.phase = 0.5;
        assert_eq!(StylizedSkyUniform::from(&sky).cloud_looks[2].z, 1.0);
    }

    #[test]
    fn disc_is_hidden_mid_swap() {
        // half the swing before moonrise the dip is whole
        let half_swap_hours = MOON_SWAP_DURATION * 12.0;
        let disc = disc_params((MOONRISE - half_swap_hours) / 24.0, false);
        assert_eq!(disc.opacity, 0.0);
        // and clear of the swings the disc is whole
        assert_eq!(disc_params(day_fraction(12.0), false).opacity, 1.0);
        assert_eq!(disc_params(day_fraction(0.0), false).opacity, 1.0);
    }

    #[test]
    fn moon_disc_is_a_crescent_by_night() {
        let disc = disc_params(day_fraction(0.0), false);
        assert!(disc.moon_active);
        assert_eq!((disc.size, disc.mask_size), (0.12, 0.16));
        assert_eq!(
            (disc.size, disc.mask_size),
            (MOON_DISC_SIZE, MOON_MASK_SIZE)
        );
    }

    #[test]
    fn sun_disc_follows_the_curve() {
        let disc = disc_params(0.5, false);
        assert!(!disc.moon_active);
        assert_eq!(disc.size, sample_keys(&SUN_SIZE_CURVE, 0.5));
        assert_eq!(disc.mask_size, 0.0);
        // the halo keeps its curves at any hour, the moon's included
        for fraction in [0.0, 0.3, 0.5] {
            assert_eq!(
                disc_params(fraction, false).halo,
                Vec2::new(
                    sample_keys(&SUN_RADIANCE_CURVE, fraction),
                    sample_keys(&SUN_RADIANCE_INTENSITY_CURVE, fraction),
                )
            );
        }
    }

    #[test]
    fn scene_sun_direction_draws_a_whole_disc() {
        let midnight = day_fraction(0.0);
        let disc = disc_params(midnight, true);
        assert!(!disc.moon_active);
        assert_eq!(disc.mask_size, 0.0);
        assert_eq!(disc.size, sample_keys(&SUN_SIZE_CURVE, midnight));
    }

    #[test]
    fn disc_colour_is_the_active_body_or_the_scene() {
        // the ramp linearised as a material colour, as the sky shader receives it
        let midnight = day_fraction(0.0);
        let moon = disc_params(midnight, false);
        assert_eq!(
            disc_color(&moon, midnight, None),
            gamma_to_linear(sample_ramp(&MOON_COLOR_RAMP, moon_progress(midnight)))
        );
        let sun = disc_params(0.5, false);
        assert_eq!(
            disc_color(&sun, 0.5, None),
            gamma_to_linear(sample_ramp(&SUN_COLOR_RAMP, time_to_phase(0.5)))
        );
        // a scene's sun colour tints the disc: its hue at the active body's preset brightness,
        // times the authored brightness above 1
        let authored = Vec3::new(0.0, 4.0, 0.5);
        let hue = Vec3::new(0.0, 1.0, 0.125);
        let moon_preset = disc_color(&moon, midnight, None);
        let sun_preset = disc_color(&sun, 0.5, None);
        assert!(disc_color(&moon, midnight, Some(authored))
            .abs_diff_eq(hue * moon_preset.max_element() * 4.0, 1e-5));
        assert!(disc_color(&sun, 0.5, Some(authored))
            .abs_diff_eq(hue * sun_preset.max_element() * 4.0, 1e-5));
        // white at noon is the preset's brightness, not a tenth of it; black is black
        let white = disc_color(&sun, 0.5, Some(Vec3::ONE));
        assert!(white.abs_diff_eq(Vec3::splat(sun_preset.max_element()), 1e-5));
        assert!(white.max_element() > 5.0);
        assert_eq!(disc_color(&sun, 0.5, Some(Vec3::ZERO)), Vec3::ZERO);
        // below 1 the authored brightness does not dim it
        let dim = disc_color(&sun, 0.5, Some(Vec3::splat(0.2)));
        assert!(dim.abs_diff_eq(white, 1e-4));
    }

    #[test]
    fn the_uniform_carries_the_disc() {
        let mut sky = StylizedSky::default();
        sky.set_disc(&disc_params(day_fraction(0.0), false));
        assert_eq!((sky.disc_opacity, sky.disc_size), (1.0, 0.12));
        let uniform = StylizedSkyUniform::from(&sky);
        assert_eq!(uniform.mask_size, 0.16);
        assert_eq!(uniform.mask_offset, Vec2::new(-0.009, -0.007));
        assert_eq!(uniform.halo, sky.halo);
    }

    /// Renders the model into its cube at noon and at midnight and reads all six faces back.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn sky_cube_is_finite() {
        let mut app = sky_test_app(SIZE as u32);
        let cube = start_sky(&mut app);
        start_clouds(&mut app);
        for hours in [12.0, 0.0] {
            draw_sky(&mut app, |sky| {
                sky.phase = time_to_phase(day_fraction(hours));
                sky.time = 1234.5;
            });
            let texels = read_cube(&mut app, &cube, SIZE, &format!("{hours} h"));
            // and drawn: no texel of either sky is black
            let darkest = texels
                .iter()
                .map(|texel| texel[0] + texel[1] + texel[2])
                .fold(f32::MAX, f32::min);
            assert!(darkest > 0.0, "{hours} h: a texel is black");
        }
    }

    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn noon_sky_matches_the_lookup() {
        let lut = bake_sky_lut();
        let row = |row: usize| &lut[row * SKY_LUT_WIDTH..(row + 1) * SKY_LUT_WIDTH];
        let distance = |texel: [f32; 4], lut: [f32; 4]| {
            (0..3)
                .map(|channel| (texel[channel] - lut[channel]).abs())
                .fold(0.0, f32::max)
        };
        let luminance =
            |colour: [f32; 4]| 0.2126 * colour[0] + 0.7152 * colour[1] + 0.0722 * colour[2];
        // the lookup holds sRGB-encoded values, which the sky decodes as its last step
        let decode = |colour: [f32; 4]| {
            let linear = sky_rgb_to_linear(Vec3::new(colour[0], colour[1], colour[2]));
            [linear.x, linear.y, linear.z, colour[3]]
        };
        let mut app = sky_test_app(SIZE as u32);
        let cube = start_sky(&mut app);

        draw_sky(&mut app, |sky| {
            sky.phase = time_to_phase(day_fraction(12.0))
        });
        let texels = read_cube(&mut app, &cube, SIZE, "noon");
        let day = row(2);
        // straight up is the end of the day row
        let zenith = texel(&texels, SIZE, UP, CENTRE, CENTRE);
        assert!(
            distance(zenith, decode(day[SKY_LUT_WIDTH - 1])) < 1e-2,
            "noon zenith {zenith:?}, lookup {:?}",
            decode(day[SKY_LUT_WIDTH - 1])
        );
        // the horizon is the start of the row, lifted by the noise: at most its strength
        // times the 0.64 it weighs at the horizon
        let horizon = texel(&texels, SIZE, NORTH, CENTRE, CENTRE);
        let reach = HORIZON_NOISE.x * 0.64 * SKY_LUT_WIDTH as f32;
        let nearest = (0..=(reach * 8.0).ceil() as usize)
            .map(|step| {
                let x = step as f32 / 8.0;
                let (from, to) = (day[x.floor() as usize], day[x.ceil() as usize]);
                let between: [f32; 4] =
                    std::array::from_fn(|c| from[c] + (to[c] - from[c]) * x.fract());
                distance(horizon, decode(between))
            })
            .fold(f32::MAX, f32::min);
        assert!(
            nearest < 1e-2,
            "noon horizon {horizon:?} is {nearest} from the lookup's first {reach} texels"
        );
        assert!(
            luminance(horizon) >= luminance(decode(day[0])) - 0.05,
            "noon horizon {horizon:?}, lookup {:?}",
            decode(day[0])
        );
        // the noise is the texture, not its one-colour stand-in: the lift varies with azimuth
        let (low, high) = (0..SIZE)
            .map(|x| texel(&texels, SIZE, NORTH, x, CENTRE)[0])
            .fold((f32::MAX, f32::MIN), |(low, high), red| {
                (low.min(red), high.max(red))
            });
        assert!(
            high - low > 0.02,
            "the horizon is one colour: {low}..{high}"
        );
        // alpha is the cloud cover, and until the strips load there is none
        assert!(texels.iter().all(|texel| texel[3] == 0.0));

        // with the strips, cloud covers part of the sky 10 degrees up and never the zenith.
        // each side face's row there, 7 to 10 degrees up
        start_clouds(&mut app);
        draw_sky(&mut app, |_| ());
        let clouded = read_cube(&mut app, &cube, SIZE, "noon clouds");
        let ten_up = ((1.0 - 10f32.to_radians().tan()) * CENTRE as f32) as usize;
        let cover = |cube: &[[f32; 4]]| {
            [0, 1, 4, 5]
                .into_iter()
                .flat_map(|face| (0..SIZE).map(move |x| (face, x)))
                .map(|(face, x)| texel(cube, SIZE, face, x, ten_up)[3])
                .fold((f32::MAX, f32::MIN), |(low, high), alpha| {
                    (low.min(alpha), high.max(alpha))
                })
        };
        let (clear, covered) = cover(&clouded);
        assert!(covered > 0.0, "no cloud 10 degrees up");
        // the strips, not their one-colour stand-in, which covers everything
        assert_eq!(clear, 0.0, "cloud all around 10 degrees up");
        assert_eq!(texel(&clouded, SIZE, UP, CENTRE, CENTRE), zenith);
        // the cloud is drawn over the sky
        let changed = texels
            .iter()
            .zip(&clouded)
            .filter(|(sky, clouded)| distance(**sky, **clouded) > 0.05)
            .count();
        assert!(changed > 0, "the strips change no texel");
        // a scene's opacity of 0 takes them away
        draw_sky(&mut app, |sky| sky.cloud_opacity = 0.0);
        let cleared = read_cube(&mut app, &cube, SIZE, "noon, clouds at 0");
        assert!(cleared.iter().all(|texel| texel[3] == 0.0));
        draw_sky(&mut app, |sky| sky.cloud_opacity = 1.0);

        // the rows run night, sunrise, day, sunset, night: sunrise is the second
        draw_sky(&mut app, |sky| sky.phase = 0.25);
        let texels = read_cube(&mut app, &cube, SIZE, "sunrise");
        let zenith = texel(&texels, SIZE, UP, CENTRE, CENTRE);
        let sunrise = decode(row(1)[SKY_LUT_WIDTH - 1]);
        assert!(
            distance(zenith, sunrise) < 1e-2,
            "sunrise zenith {zenith:?}, lookup {sunrise:?}"
        );
    }

    /// The disc straight up, where the centre texel of the up face looks along it: the sun at
    /// noon, then the moon at midnight.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn the_disc_and_the_crescent_are_drawn() {
        let sum = |texel: [f32; 4]| texel[0] + texel[1] + texel[2];
        let mut app = sky_test_app(SIZE as u32);
        let cube = start_sky(&mut app);
        let draw_body = |sky: &mut StylizedSky, fraction: f32| {
            let disc = disc_params(fraction, false);
            sky.phase = time_to_phase(fraction);
            sky.body_direction = Vec3::Y;
            sky.disc_color = disc_color(&disc, fraction, None);
            sky.set_disc(&disc);
        };

        // no body: the model's default disc has no opacity
        let noon = day_fraction(12.0);
        draw_sky(&mut app, |sky| sky.phase = time_to_phase(noon));
        let bare = read_cube(&mut app, &cube, SIZE, "noon, no body");
        draw_sky(&mut app, |sky| draw_body(sky, noon));
        let texels = read_cube(&mut app, &cube, SIZE, "noon");
        // the sun's disc is a texel or two across here, far brighter than the sky beside it
        let sun = sum(texel(&texels, SIZE, UP, CENTRE, CENTRE));
        let beside = sum(texel(&texels, SIZE, UP, CENTRE + 8, CENTRE));
        assert!(sun > beside + 10.0, "sun {sun}, the sky beside it {beside}");
        // the halo lights the sky around the disc, and none of it a radian and more away
        let bare_beside = sum(texel(&bare, SIZE, UP, CENTRE + 8, CENTRE));
        assert!(
            beside > bare_beside + 0.2,
            "the sky by the sun {beside}, with no sun {bare_beside}"
        );
        assert_eq!(
            texel(&texels, SIZE, NORTH, CENTRE, CENTRE),
            texel(&bare, SIZE, NORTH, CENTRE, CENTRE)
        );
        // a scene hides the disc and its halo
        draw_sky(&mut app, |sky| sky.scene_sun_visible = 0.0);
        assert_eq!(read_cube(&mut app, &cube, SIZE, "noon, hidden"), bare);

        // the moon: its mask leaves a crescent, so the middle of the disc is the night sky
        // and a texel beside it is lit
        let midnight = day_fraction(0.0);
        draw_sky(&mut app, |sky| {
            sky.scene_sun_visible = 1.0;
            draw_body(sky, midnight);
        });
        let crescent = read_cube(&mut app, &cube, SIZE, "midnight");
        let around = |cube: &[[f32; 4]]| {
            (CENTRE - 3..=CENTRE + 3)
                .flat_map(|y| (CENTRE - 3..=CENTRE + 3).map(move |x| (x, y)))
                .map(|(x, y)| sum(texel(cube, SIZE, UP, x, y)))
                .fold(f32::MIN, f32::max)
        };
        let middle = sum(texel(&crescent, SIZE, UP, CENTRE, CENTRE));
        assert!(
            around(&crescent) > middle + 3.0,
            "no crescent: the middle {middle}, the brightest by it {}",
            around(&crescent)
        );
        // without the mask the disc is whole
        draw_sky(&mut app, |sky| sky.mask_size = 0.0);
        let whole = read_cube(&mut app, &cube, SIZE, "midnight, no mask");
        let unmasked = sum(texel(&whole, SIZE, UP, CENTRE, CENTRE));
        assert!(
            unmasked > middle + 3.0,
            "the mask hides nothing: {middle} with it, {unmasked} without"
        );
    }

    /// The screen shows the sky per pixel, not the cube: a camera ten degrees tall, 64 pixels,
    /// looks up at the noon sun over a cube of 64 texels a side (1.4 degrees, nine pixels, a
    /// texel). The disc, dimmed so it does not saturate, is 0.03 radians in radius: 11 pixels
    /// either side of the middle, its edge one step. Drawn from the cube, the edge ramps over
    /// about a texel.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn the_screen_draws_the_sky_per_pixel() {
        assert_sharp_disc(&sky_row_through_the_disc());
    }

    /// The disc's edges are single steps between pixels 20 and 21, and 42 and 43.
    fn assert_sharp_disc(row: &[u8]) {
        // the two largest steps between neighbours are the disc's edges
        let mut steps: Vec<(i32, usize)> = (0..row.len() - 1)
            .map(|x| ((row[x + 1] as i32 - row[x] as i32).abs(), x))
            .collect();
        steps.sort_unstable_by(|a, b| b.cmp(a));
        let (mut left, mut right) = (steps[0].1, steps[1].1);
        if left > right {
            std::mem::swap(&mut left, &mut right);
        }
        assert!(
            steps[1].0 > 40,
            "the disc's edge ramps: the steps are {:?}; row {row:?}",
            &steps[..4]
        );
        assert!(
            left.abs_diff(20) <= 1 && right.abs_diff(42) <= 1,
            "the disc's edges are after pixels {left} and {right}, not 20 and 42; row {row:?}"
        );
    }

    /// The red of the middle row of a 64 pixel camera, ten degrees tall, that looks up at the
    /// dimmed noon disc.
    fn sky_row_through_the_disc() -> Vec<u8> {
        use bevy::{
            core_pipeline::tonemapping::Tonemapping,
            render::{
                camera::RenderTarget,
                view::screenshot::{Screenshot, ScreenshotCaptured},
            },
        };
        use bevy_atmosphere::prelude::AtmosphereCamera;
        use pipeline_test::render_target;

        #[derive(Resource, Default)]
        struct Captured(Option<Image>);

        let mut app = sky_test_app(64);
        app.add_plugins(StylizedSkyMaterialPlugin)
            .init_resource::<Captured>();
        start_sky(&mut app);
        let target = render_target(&mut app);
        app.world_mut().spawn((
            Camera3d::default(),
            Tonemapping::None,
            Camera {
                target: RenderTarget::Image(target.clone().into()),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: 10f32.to_radians(),
                ..default()
            }),
            Msaa::Off,
            AtmosphereCamera::default(),
            Transform::default().looking_at(Vec3::Y, Vec3::Z),
        ));
        let noon = day_fraction(12.0);
        draw_sky(&mut app, |sky| {
            let disc = disc_params(noon, false);
            sky.phase = time_to_phase(noon);
            sky.body_direction = Vec3::Y;
            sky.set_disc(&disc);
            sky.disc_color = Vec3::splat(0.3);
        });
        assert!((disc_params(noon, false).size * 0.3 - 0.03).abs() < 1e-6);

        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |event: Trigger<ScreenshotCaptured>, mut captured: ResMut<Captured>| {
                    captured.0 = Some(event.event().0.clone())
                },
            );
        let start = std::time::Instant::now();
        let image = loop {
            app.update();
            if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
                break image;
            }
            assert!(start.elapsed().as_secs() < 30, "GPU screenshot timed out");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let rgba = image.try_into_dynamic().unwrap().to_rgba8();
        (0..64).map(|x| rgba.get_pixel(x, 32).0[0]).collect()
    }

    #[test]
    fn a_new_skybox_gets_the_per_pixel_sky_in_its_spawn_flush() {
        let mut world = World::new();
        let material = Handle::<StylizedSkyMaterial>::default();
        world.insert_resource(StylizedSkyMaterialHandle(material.clone()));
        world.add_observer(use_stylized_skybox);
        world.commands().spawn((
            MeshMaterial3d(Handle::<SkyBoxMaterial>::default()),
            AtmosphereSkyBox,
        ));
        world.flush();

        let mut skyboxes = world.query_filtered::<(
            Option<&MeshMaterial3d<SkyBoxMaterial>>,
            &MeshMaterial3d<StylizedSkyMaterial>,
        ), With<AtmosphereSkyBox>>();
        let (cube, sky) = skyboxes.single(&world).unwrap();
        assert!(cube.is_none());
        assert_eq!(sky.0, material);
    }
}
