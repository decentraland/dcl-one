//! Scene-root environment overrides. The scene the player is in owns them; leaving it restores
//! the clock.

use bevy::{
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_asset::RenderAssetUsages,
};
use bevy_atmosphere::skybox::{AtmosphereSkyBoxMaterial, SkyBoxMaterial};
use bevy_console::ConsoleCommand;
use common::{
    sets::SceneSets,
    structs::{SceneGlobalLight, TimeOfDay},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::{
        common::ColorGradient,
        sdk::components::{PbEnvironmentFog, PbSkybox},
    },
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use ipfs::IpfsAssetServer;
use scene_runner::{
    renderer_context::RendererSceneContext,
    update_world::{
        lights::{ColorRamp, Sun, MAX_SCENE_VALUE},
        AddCrdtInterfaceExt,
    },
};

use crate::day_night::RootComponentSource;

pub struct SceneEnvironmentPlugin;

impl Plugin for SceneEnvironmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneEnvironment>();
        app.add_crdt_lww_component::<PbSkybox, Skybox>(
            SceneComponentId::SKYBOX,
            ComponentPosition::RootOnly,
        );
        app.add_crdt_lww_component::<PbEnvironmentFog, EnvironmentFog>(
            SceneComponentId::ENVIRONMENT_FOG,
            ComponentPosition::RootOnly,
        );
        app.add_systems(
            Update,
            (load_sky_panoramas, update_environment)
                .chain()
                .in_set(SceneSets::PostLoop),
        )
        .add_systems(Update, update_sky_panorama)
        .add_console_command::<EnvironmentCommand, _>(environment_command)
        .add_debug_section(DebugSection {
            title: "Scene environment",
            query: "/scene_environment",
            fields: &[
                ("Scene", "scene"),
                ("Texture state", "textureState"),
                ("Sun visible", "sunVisible"),
                ("Sun intensity multiplier", "sunIntensity"),
                ("Sun direction", "sunDirection"),
                ("Sun color", "sunColor"),
                ("Fog density per metre", "fogDensity"),
                ("Fog color", "fogColor"),
                ("Sky zenith", "zenith"),
                ("Sky horizon", "horizon"),
                ("Sky nadir", "nadir"),
                ("Sky rim", "rim"),
                ("Cloud opacity", "cloudOpacity"),
                ("Cloud radians per second", "cloudSpeed"),
                ("Cloud color", "cloudColor"),
                ("Star brightness", "starsBrightness"),
            ],
            actions: &[
                ("Midnight", "/time 0 0 --force"),
                ("Noon", "/time 12 0 --force"),
                ("Resume scene time", "/time --resume"),
            ],
            inputs: &[],
        });
    }
}

fn finite(value: Option<f32>, min: f32, max: f32) -> Option<f32> {
    value.filter(|v| v.is_finite()).map(|v| v.clamp(min, max))
}

fn ramp(gradient: Option<&ColorGradient>) -> Option<ColorRamp> {
    gradient.and_then(ColorRamp::new)
}

/// A scene's sky override, on its root.
#[derive(Component)]
struct Skybox {
    texture: Option<String>,
    zenith: Option<ColorRamp>,
    horizon: Option<ColorRamp>,
    nadir: Option<ColorRamp>,
    /// follows the horizon when unset
    rim: Option<ColorRamp>,
    cloud_color: Option<ColorRamp>,
    cloud_opacity: Option<f32>,
    cloud_speed: Option<f32>,
    star_brightness: Option<f32>,
}

impl From<PbSkybox> for Skybox {
    fn from(value: PbSkybox) -> Self {
        let colors = value.sky_colors.as_ref();
        let clouds = value.clouds.as_ref();
        let horizon = ramp(colors.and_then(|c| c.horizon.as_ref()));
        Self {
            texture: value.skybox_texture.filter(|s| !s.is_empty()),
            zenith: ramp(colors.and_then(|c| c.zenith.as_ref())),
            nadir: ramp(colors.and_then(|c| c.nadir.as_ref())),
            rim: ramp(colors.and_then(|c| c.rim.as_ref())).or_else(|| horizon.clone()),
            horizon,
            cloud_color: ramp(clouds.and_then(|c| c.color.as_ref())),
            cloud_opacity: finite(clouds.and_then(|c| c.opacity), 0.0, 1.0),
            cloud_speed: finite(clouds.and_then(|c| c.speed), -f32::MAX, f32::MAX),
            star_brightness: finite(
                value.stars.as_ref().and_then(|s| s.brightness),
                0.0,
                MAX_SCENE_VALUE,
            ),
        }
    }
}

/// The image a scene's [`Skybox`] shows instead of the sky, on the same root.
#[derive(Component)]
struct SkyboxPanorama(Handle<Image>);

/// A scene's fog override, on its root.
#[derive(Component)]
struct EnvironmentFog {
    color: Option<ColorRamp>,
    density: Option<f32>,
}

impl From<PbEnvironmentFog> for EnvironmentFog {
    fn from(value: PbEnvironmentFog) -> Self {
        Self {
            color: ramp(value.color.as_ref()),
            density: finite(value.density, 0.0, f32::MAX),
        }
    }
}

/// The overrides of the scenes the player is in, at the current time of day.
#[derive(Resource, Default, Clone, PartialEq)]
pub(super) struct SceneEnvironment {
    /// the scene the sky comes from
    pub skybox: Option<Entity>,
    /// the scene the fog comes from
    pub fog: Option<Entity>,
    pub zenith: Option<Vec3>,
    pub horizon: Option<Vec3>,
    pub nadir: Option<Vec3>,
    pub rim: Option<Vec3>,
    pub fog_color: Option<Vec3>,
    pub fog_density: Option<f32>,
    pub cloud_color: Option<Vec3>,
    pub cloud_opacity: Option<f32>,
    pub cloud_speed: Option<f32>,
    pub star_brightness: Option<f32>,
    pub texture: Option<Handle<Image>>,
    pub texture_state: &'static str,
}

fn valid_sky_image(image: &Image) -> bool {
    image.height() > 0 && image.width() == image.height().saturating_mul(2)
}

fn load_sky_panoramas(
    mut commands: Commands,
    skyboxes: Query<(Entity, &Skybox, &RendererSceneContext), Changed<Skybox>>,
    mut removed: RemovedComponents<Skybox>,
    ipfs: IpfsAssetServer,
) {
    for entity in removed.read() {
        if let Ok(mut commands) = commands.get_entity(entity) {
            commands.try_remove::<SkyboxPanorama>();
        }
    }
    for (entity, skybox, context) in &skyboxes {
        let handle = skybox.texture.as_deref().and_then(|src| {
            ipfs.load_content_file_with_settings::<Image, ImageLoaderSettings>(
                src,
                &context.hash,
                |settings| {
                    settings.asset_usage = RenderAssetUsages::RENDER_WORLD;
                    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::ClampToEdge,
                        ..ImageSamplerDescriptor::linear()
                    });
                },
            )
            .ok()
        });
        match handle {
            Some(handle) => commands.entity(entity).try_insert(SkyboxPanorama(handle)),
            None => commands.entity(entity).try_remove::<SkyboxPanorama>(),
        };
    }
}

fn update_environment(
    mut skybox_source: RootComponentSource<Skybox>,
    mut fog_source: RootComponentSource<EnvironmentFog>,
    skyboxes: Query<(&Skybox, Option<&SkyboxPanorama>)>,
    fogs: Query<&EnvironmentFog>,
    time: Res<TimeOfDay>,
    images: Res<Assets<Image>>,
    server: Res<AssetServer>,
    mut state: ResMut<SceneEnvironment>,
) {
    let mut next = SceneEnvironment {
        skybox: skybox_source.changed_pick().unwrap_or(state.skybox),
        fog: fog_source.changed_pick().unwrap_or(state.fog),
        texture_state: "unset",
        ..default()
    };
    let day_fraction = time.elapsed_secs().rem_euclid(86400.0) / 86400.0;
    let sample = |ramp: &Option<ColorRamp>| ramp.as_ref().map(|ramp| ramp.sample(day_fraction));

    if let Some((skybox, panorama)) = next.skybox.and_then(|e| skyboxes.get(e).ok()) {
        next.zenith = sample(&skybox.zenith);
        next.horizon = sample(&skybox.horizon);
        next.nadir = sample(&skybox.nadir);
        next.rim = sample(&skybox.rim);
        next.cloud_color = sample(&skybox.cloud_color);
        next.cloud_opacity = skybox.cloud_opacity;
        next.cloud_speed = skybox.cloud_speed;
        next.star_brightness = skybox.star_brightness;
        if skybox.texture.is_some() {
            next.texture_state = match panorama {
                None => "unresolved",
                Some(SkyboxPanorama(handle)) => match images.get(handle) {
                    Some(image) if valid_sky_image(image) => {
                        next.texture = Some(handle.clone());
                        "ready"
                    }
                    Some(_) => "invalid aspect ratio",
                    None if matches!(
                        server.load_state(handle.id()),
                        bevy::asset::LoadState::Failed(_)
                    ) =>
                    {
                        "failed"
                    }
                    None => "loading",
                },
            };
        }
    }
    if let Some(fog) = next.fog.and_then(|e| fogs.get(e).ok()) {
        next.fog_color = sample(&fog.color);
        next.fog_density = fog.density;
    }
    state.set_if_neq(next);
}

fn update_sky_panorama(
    state: Res<SceneEnvironment>,
    material: Res<AtmosphereSkyBoxMaterial>,
    mut materials: ResMut<Assets<SkyBoxMaterial>>,
) {
    if materials
        .get(&material.0)
        .is_some_and(|sky| sky.panorama != state.texture)
    {
        if let Some(sky) = materials.get_mut(&material.0) {
            sky.panorama = state.texture.clone();
        }
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_environment")]
struct EnvironmentCommand;

fn environment_command(
    mut command: ConsoleCommand<EnvironmentCommand>,
    state: Res<SceneEnvironment>,
    light: Res<SceneGlobalLight>,
    suns: Query<&Sun>,
) {
    let Some(Ok(_)) = command.take() else { return };
    let sun_scene = light.source.filter(|scene| suns.contains(*scene));
    let sun = sun_scene.and_then(|scene| suns.get(scene).ok());
    command.reply_ok(environment_reading(&state, &light, sun_scene, sun).to_string());
}

/// The scene overrides in force. A field the scenes leave unset reads null, explained by the one
/// default entry under `unavailable`.
fn environment_reading(
    state: &SceneEnvironment,
    light: &SceneGlobalLight,
    sun_scene: Option<Entity>,
    sun: Option<&Sun>,
) -> serde_json::Value {
    let scene = state.skybox.or(state.fog).or(sun_scene);
    let reason = if scene.is_some() {
        "No override for this field; using renderer defaults."
    } else {
        "No active scene environment overrides."
    };
    serde_json::json!({
        "scene": scene.map(|e| e.to_bits().to_string()),
        "textureState": if state.texture_state.is_empty() { "unset" } else { state.texture_state },
        "sunVisible": sun.and_then(|sun| sun.visible).unwrap_or(true),
        "sunIntensity": sun.and_then(|sun| sun.intensity).unwrap_or(1.0),
        "sunDirection": sun.and_then(|sun| sun.direction).map(|d| d.to_array()),
        "sunColor": sun.is_some_and(|sun| sun.color.is_some()).then(|| light.dir_color.to_linear().to_vec3().to_array()),
        "fogDensity": state.fog_density,
        "zenith": state.zenith.map(|c| c.to_array()),
        "horizon": state.horizon.map(|c| c.to_array()),
        "nadir": state.nadir.map(|c| c.to_array()),
        "rim": state.rim.map(|c| c.to_array()),
        "fogColor": state.fog_color.map(|c| c.to_array()),
        "cloudColor": state.cloud_color.map(|c| c.to_array()),
        "cloudSpeed": state.cloud_speed,
        "cloudOpacity": state.cloud_opacity,
        "starsBrightness": state.star_brightness,
        "unavailable": { "": reason },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::proto_components::{
        common::{Color4, ColorKey},
        sdk::components::pb_skybox,
    };

    #[test]
    fn environment_reading_explains_its_nulls_with_one_default_entry() {
        let light = SceneGlobalLight::default();
        let reading = environment_reading(&SceneEnvironment::default(), &light, None, None);
        assert!(reading["scene"].is_null());
        assert_eq!(reading["textureState"], "unset");
        assert_eq!(reading["sunVisible"], true);
        assert_eq!(
            reading["unavailable"],
            serde_json::json!({"": "No active scene environment overrides."})
        );

        let scene = Entity::PLACEHOLDER;
        let state = SceneEnvironment {
            fog: Some(scene),
            fog_density: Some(0.0),
            zenith: Some(Vec3::Z),
            ..default()
        };
        let reading = environment_reading(&state, &light, None, None);
        assert_eq!(reading["scene"], scene.to_bits().to_string());
        assert_eq!(reading["fogDensity"], 0.0);
        assert_eq!(reading["zenith"], serde_json::json!([0.0, 0.0, 1.0]));
        assert!(reading["horizon"].is_null());
        assert_eq!(
            reading["unavailable"],
            serde_json::json!({"": "No override for this field; using renderer defaults."})
        );
    }

    fn gradient(keys: &[(f32, f32)]) -> ColorGradient {
        ColorGradient {
            keys: keys
                .iter()
                .map(|&(time, red)| ColorKey {
                    time,
                    color: Some(Color4 {
                        r: red,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                })
                .collect(),
        }
    }

    #[test]
    fn optional_zero_and_negative_speed_are_not_defaults() {
        let sky = Skybox::from(PbSkybox {
            clouds: Some(pb_skybox::Clouds {
                opacity: Some(0.0),
                speed: Some(-0.01),
                ..default()
            }),
            stars: Some(pb_skybox::Stars {
                brightness: Some(0.0),
            }),
            ..default()
        });
        assert_eq!(sky.cloud_opacity, Some(0.0));
        assert_eq!(sky.cloud_speed, Some(-0.01));
        assert_eq!(sky.star_brightness, Some(0.0));
        let fog = EnvironmentFog::from(PbEnvironmentFog {
            density: Some(0.0),
            ..default()
        });
        assert_eq!(fog.density, Some(0.0));
        let empty = Skybox::from(PbSkybox::default());
        assert_eq!(empty.cloud_opacity, None);
        assert!(empty.texture.is_none());
        assert_eq!(
            EnvironmentFog::from(PbEnvironmentFog::default()).density,
            None
        );
    }

    #[test]
    fn invalid_numbers_leave_defaults_and_rim_follows_horizon() {
        let sky = Skybox::from(PbSkybox {
            skybox_texture: Some(String::new()),
            sky_colors: Some(pb_skybox::SkyColors {
                horizon: Some(gradient(&[(0.0, 4.0)])),
                ..default()
            }),
            stars: Some(pb_skybox::Stars {
                brightness: Some(f32::INFINITY),
            }),
            ..default()
        });
        assert!(sky.texture.is_none());
        assert_eq!(sky.star_brightness, None);
        assert_eq!(sky.rim, sky.horizon);
        assert_eq!(sky.horizon.unwrap().sample(0.5), Vec3::X * 4.0);
        let fog = EnvironmentFog::from(PbEnvironmentFog {
            density: Some(-1.0),
            ..default()
        });
        assert_eq!(fog.density, Some(0.0));
    }

    #[test]
    fn texture_layout_requires_equirectangular_two_to_one() {
        let mut image = Image::default();
        image.resize(bevy::render::render_resource::Extent3d {
            width: 4,
            height: 2,
            depth_or_array_layers: 1,
        });
        assert!(valid_sky_image(&image));
        image.resize(bevy::render::render_resource::Extent3d {
            width: 3,
            height: 2,
            depth_or_array_layers: 1,
        });
        assert!(!valid_sky_image(&image));
    }

    #[test]
    fn fog_sky_and_ambient_overrides_reset_without_bypassing_quality() {
        use crate::{
            nishita_cloud::NishitaCloud,
            sky_envmap::{SkyAmbient, SkyEnvmapSettings},
            sky_preset::{ambient_trilight, fog_falloff_density},
            CloudCover, FogDensity, ShadowBiasOverride,
        };
        use bevy_atmosphere::prelude::AtmosphereModel;
        use common::structs::{AppConfig, FogSetting, SceneGlobalLight};
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        let mut config = AppConfig::default();
        config.graphics.fog = FogSetting::On;
        app.insert_resource(time)
            .insert_resource(config)
            .insert_resource(TimeOfDay { time: 43200.0 })
            .init_resource::<FogDensity>()
            .init_resource::<SkyAmbient>()
            .insert_resource(SceneGlobalLight {
                dir_color: Color::WHITE,
                dir_illuminance: 1000.0,
                sun_illuminance: 1000.0,
                dir_direction: Vec3::NEG_Y,
                sun_direction: Vec3::NEG_Y,
                ambient_color: Color::WHITE,
                ambient_brightness: 1.0,
                sun_hidden: true,
                ..default()
            })
            .insert_resource(AtmosphereModel::new(NishitaCloud::default()))
            .insert_resource(CloudCover {
                cover: 0.375,
                speed: 5.0,
                density_cap: 0.8,
                shadow: 0.05,
                scale: 1.5,
                steps: 44,
                lacunarity: 2.0,
                sharpness: 50.0,
                sharp_mid: 0.1,
            })
            .init_resource::<AmbientLight>()
            .init_resource::<SkyEnvmapSettings>()
            .init_resource::<ShadowBiasOverride>()
            .insert_resource(SceneEnvironment {
                fog_density: Some(0.01),
                zenith: Some(Vec3::X * 2.0),
                nadir: Some(Vec3::Z),
                cloud_opacity: Some(0.0),
                ..default()
            })
            .add_systems(Update, crate::apply_global_light);
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), DistanceFog::default()))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let world = app.world_mut();
        assert!(
            world
                .resource::<SkyEnvmapSettings>()
                .authored_lower_hemisphere
        );
        // the scene's density is in the preset's units
        let scene_falloff = fog_falloff_density(0.01);
        assert!(matches!(
            world.get::<DistanceFog>(camera).unwrap().falloff,
            FogFalloff::Exponential { density } if density == scene_falloff
        ));
        // the authored zenith and nadir are the Trilight's sky and ground; the preset keeps the
        // equator the scene leaves out
        let preset = ambient_trilight(0.5);
        assert_eq!(
            world.resource::<SkyAmbient>().bands,
            [Vec3::X * 2.0, preset[1], Vec3::Z]
        );
        let atmosphere = world
            .resource::<AtmosphereModel>()
            .to_ref::<NishitaCloud>()
            .unwrap();
        assert_eq!(atmosphere.scene_zenith, Vec4::new(2.0, 0.0, 0.0, 1.0));
        assert_eq!(atmosphere.scene_sun_visible, 0.0);
        let ambient = world.resource::<AmbientLight>();
        assert!(ambient.color.to_linear().alpha * ambient.brightness < -1.5);
        // the Off setting is no fog, whatever the scene asks for
        world.resource_mut::<AppConfig>().graphics.fog = FogSetting::Off;
        app.update();
        assert!(matches!(
            app.world().get::<DistanceFog>(camera).unwrap().falloff,
            FogFalloff::Exponential { density: 0.0 }
        ));
        *app.world_mut().resource_mut::<SceneEnvironment>() = SceneEnvironment::default();
        app.world_mut()
            .resource_mut::<SceneGlobalLight>()
            .sun_hidden = false;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.resource::<SkyAmbient>().bands, preset);
        assert!(
            !world
                .resource::<SkyEnvmapSettings>()
                .authored_lower_hemisphere
        );
        let atmosphere = world
            .resource::<AtmosphereModel>()
            .to_ref::<NishitaCloud>()
            .unwrap();
        assert_eq!(atmosphere.scene_zenith, Vec4::ZERO);
        assert_eq!(atmosphere.scene_sun_visible, 1.0);
        assert_eq!(atmosphere.scene_cloud_opacity, 1.0);
        let ambient = world.resource::<AmbientLight>();
        assert!(ambient.color.to_linear().alpha * ambient.brightness >= -1.0);
    }

    /// Compiles the procedural compute shader and the panorama skybox on a GPU.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn scene_environment_pipelines_compile() {
        use crate::nishita_cloud::{init_noise, NishitaCloud};
        use bevy::{
            core_pipeline::tonemapping::Tonemapping,
            render::{
                camera::RenderTarget,
                gpu_readback::{Readback, ReadbackComplete},
                render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
                view::screenshot::{Screenshot, ScreenshotCaptured},
                RenderApp,
            },
        };
        use pipeline_test::{
            compile_pipelines, headless_app_without_embedded_assets, render_target,
        };
        #[derive(Resource, Default)]
        struct Captured(Option<Image>);
        #[derive(Resource, Default)]
        struct RawCaptured(Option<Vec<u8>>);
        fn raw_cube(
            app: &mut App,
            handle: &Handle<Image>,
            size: usize,
            label: &str,
        ) -> Vec<[u16; 4]> {
            app.world_mut().resource_mut::<RawCaptured>().0 = None;
            let entity = app
                .world_mut()
                .spawn(Readback::texture(handle.clone()))
                .observe(
                    |event: Trigger<ReadbackComplete>, mut captured: ResMut<RawCaptured>| {
                        captured.0 = Some(event.event().0.clone());
                    },
                )
                .id();
            let start = std::time::Instant::now();
            let bytes = loop {
                app.update();
                if let Some(bytes) = app.world_mut().resource_mut::<RawCaptured>().0.take() {
                    break bytes;
                }
                assert!(
                    start.elapsed().as_secs() < 30,
                    "{label}: raw GPU readback timed out"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            app.world_mut().despawn(entity);
            // Readback rows are aligned to 256 bytes; skip padding, retain every face/texel/channel.
            let stride = (size * 8).div_ceil(256) * 256;
            assert_eq!(bytes.len(), stride * size * 6, "{label}: six cubemap faces");
            let mut texels = Vec::with_capacity(size * size * 6);
            for face in 0..6 {
                for y in 0..size {
                    for x in 0..size {
                        let offset = (face * size + y) * stride + x * 8;
                        let channels = std::array::from_fn(|channel| {
                            let i = offset + channel * 2;
                            u16::from_le_bytes([bytes[i], bytes[i + 1]])
                        });
                        for value in channels {
                            assert_ne!(
                                value & 0x7c00,
                                0x7c00,
                                "{label}: nonfinite f16 face={face} x={x} y={y}: {channels:?}"
                            );
                        }
                        texels.push(channels);
                    }
                }
            }
            texels
        }
        fn tick(mut atmosphere: bevy_atmosphere::system_param::AtmosphereMut<NishitaCloud>) {
            atmosphere.tick = atmosphere.tick.wrapping_add(1);
        }
        fn capture(app: &mut App, target: &Handle<Image>, label: &str) -> [u8; 4] {
            app.world_mut().resource_mut::<Captured>().0 = None;
            app.world_mut()
                .spawn(Screenshot::image(target.clone()))
                .observe(
                    |event: Trigger<ScreenshotCaptured>, mut captured: ResMut<Captured>| {
                        captured.0 = Some(event.event().0.clone())
                    },
                );
            let start = std::time::Instant::now();
            loop {
                app.update();
                if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
                    let rgba = image
                        .try_into_dynamic()
                        .expect("captured RGBA image")
                        .to_rgba8();
                    if let Some(directory) = std::env::var_os("DCL_ENVIRONMENT_SCREENSHOTS") {
                        std::fs::create_dir_all(&directory).unwrap();
                        rgba.save(std::path::Path::new(&directory).join(format!("{label}.png")))
                            .unwrap();
                    }
                    return rgba.get_pixel(32, 32).0;
                }
                assert!(start.elapsed().as_secs() < 30, "GPU screenshot timed out");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        use bevy_atmosphere::{
            model::AddAtmosphereModel, pipeline::AtmosphereImageBindGroupLayout, prelude::*,
        };
        let mut app = headless_app_without_embedded_assets();
        app.init_resource::<SceneEnvironment>()
            .init_resource::<Captured>()
            .init_resource::<RawCaptured>()
            .init_resource::<common::structs::AppConfig>()
            .add_plugins(console::ConsolePlugin {
                add_bevy_console: false,
            })
            .insert_resource(AtmosphereSettings {
                resolution: 128,
                ..default()
            })
            .insert_resource(AtmosphereModel::new(NishitaCloud::default()))
            .add_plugins(AtmospherePlugin)
            .add_plugins(crate::sky_envmap::SkyEnvmapPlugin)
            .add_systems(Update, (tick, update_sky_panorama).chain())
            .add_plugins(assets::EmbedAssetsPlugin);
        app.finish();
        app.cleanup();
        app.sub_app_mut(RenderApp)
            .init_resource::<AtmosphereImageBindGroupLayout>();
        app.add_atmosphere_model::<NishitaCloud>();
        let raw_sky = app
            .world()
            .resource::<bevy_atmosphere::pipeline::AtmosphereImage>()
            .handle
            .clone();
        let maps = app
            .world()
            .resource::<crate::sky_envmap::SkyEnvmap>()
            .clone();
        // the ambient map is written on the CPU and takes no part in the sky's passes
        for handle in [&raw_sky, &maps.specular, &maps.trilight_specular] {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(handle)
                .unwrap()
                .texture_descriptor
                .usage |= TextureUsages::COPY_SRC;
        }
        let noise = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(init_noise(16));
        // Centre columns face DCL +Z; the seam faces -Z.
        let mut sky_image = Image::new(
            Extent3d {
                width: 4,
                height: 2,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            [
                [0, 255, 0, 255],
                [255, 0, 0, 255],
                [255, 0, 0, 255],
                [0, 255, 0, 255],
            ]
            .repeat(2)
            .into_iter()
            .flatten()
            .collect(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        sky_image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
        let sky = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(sky_image);
        app.world_mut().resource_mut::<SceneEnvironment>().texture = Some(sky);
        app.world_mut()
            .resource_mut::<crate::sky_envmap::SkyEnvmapSettings>()
            .authored_lower_hemisphere = true;
        app.world_mut()
            .resource_mut::<AtmosphereModel>()
            .to_mut::<NishitaCloud>()
            .unwrap()
            .noise_texture = noise;
        let target = render_target(&mut app);
        let camera = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Tonemapping::None,
                Camera {
                    target: RenderTarget::Image(target.clone().into()),
                    ..default()
                },
                AtmosphereCamera::default(),
                Transform::default(),
            ))
            .id();
        for _ in 0..50 {
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        {
            use bevy::render::render_resource::{
                CachedPipelineState, PipelineCache, PipelineDescriptor,
            };
            let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
            for pipeline in cache.pipelines() {
                if matches!(pipeline.state, CachedPipelineState::Ok(_)) {
                    continue;
                }
                let label = match &pipeline.descriptor {
                    PipelineDescriptor::RenderPipelineDescriptor(p) => &p.label,
                    PipelineDescriptor::ComputePipelineDescriptor(p) => &p.label,
                };
                eprintln!(
                    "pending environment pipeline {label:?}: {:?}",
                    pipeline.state
                );
            }
        }
        assert!(compile_pipelines(&mut app) > 0);
        let front = capture(&mut app, &target, "texture-forward");
        assert!(
            front[0] > 200 && front[1] < 30 && front[2] < 30,
            "centre must face DCL +Z: {front:?}"
        );
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .rotation = Quat::from_rotation_y(std::f32::consts::PI);
        let back = capture(&mut app, &target, "texture-backward");
        assert!(
            back[1] > 200 && back[0] < 30 && back[2] < 30,
            "seam must face DCL -Z: {back:?}"
        );
        app.world_mut().resource_mut::<SceneEnvironment>().texture = None;
        {
            let mut model = app.world_mut().resource_mut::<AtmosphereModel>();
            let atmosphere = model.to_mut::<NishitaCloud>().unwrap();
            atmosphere.scene_zenith = Vec4::new(0.0, 0.0, 1.0, 1.0);
            atmosphere.scene_horizon = atmosphere.scene_zenith;
            atmosphere.scene_nadir = atmosphere.scene_zenith;
            atmosphere.scene_cloud_opacity = 0.0;
            atmosphere.scene_sun_visible = 0.0;
        }
        for _ in 0..80 {
            app.update();
        }
        let restored = capture(&mut app, &target, "procedural-restored");
        assert!(
            restored[2] > 200 && restored[0] < 30 && restored[1] < 30,
            "removing image restores authored procedural sky: {restored:?}"
        );
        // Nonzero daylight evaluates the procedural fallback even at an authored lower pole.
        // Horizontal screenshots and a zero light intensity cannot detect a NaN at face -Y.
        for band in ["nadir", "zenith", "horizon", "rim", "none"] {
            {
                let mut model = app.world_mut().resource_mut::<AtmosphereModel>();
                let atmosphere = model.to_mut::<NishitaCloud>().unwrap();
                atmosphere.dir_light_intensity = 11000.0;
                atmosphere.sun_intensity = 22.0;
                atmosphere.sun_position = Vec3::Y;
                atmosphere.scene_cloud_opacity = 0.0;
                atmosphere.scene_sun_visible = 0.0;
                atmosphere.scene_zenith = Vec4::ZERO;
                atmosphere.scene_horizon = Vec4::ZERO;
                atmosphere.scene_nadir = Vec4::ZERO;
                atmosphere.scene_rim = Vec4::ZERO;
                let red = Vec4::new(1.0, 0.0, 0.0, 1.0);
                match band {
                    "nadir" => atmosphere.scene_nadir = red,
                    "zenith" => atmosphere.scene_zenith = red,
                    "horizon" => atmosphere.scene_horizon = red,
                    "rim" => atmosphere.scene_rim = red,
                    _ => (),
                }
            }
            app.world_mut()
                .resource_mut::<crate::sky_envmap::SkyEnvmapSettings>()
                .authored_lower_hemisphere = band == "nadir";
            for quality in [
                common::structs::SkyReflectionSetting::High,
                common::structs::SkyReflectionSetting::Low,
            ] {
                app.world_mut()
                    .resource_mut::<common::structs::AppConfig>()
                    .graphics
                    .sky_reflections = quality;
                for _ in 0..80 {
                    app.update();
                }
                let raw = raw_cube(&mut app, &raw_sky, 128, band);
                let pole = raw[3 * 128 * 128 + 64 * 128 + 64];
                let expected_red = if band == "nadir" { 0x3c00 } else { 0 };
                assert_eq!(
                    pole.map(|c| c & 0x7fff),
                    [expected_red, 0, 0, 0],
                    "{band}: exact raw -Y pole"
                );
                let handle = match quality {
                    common::structs::SkyReflectionSetting::High => &maps.specular,
                    common::structs::SkyReflectionSetting::Low => &maps.trilight_specular,
                };
                let size = app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(handle)
                    .unwrap()
                    .width() as usize;
                raw_cube(
                    &mut app,
                    handle,
                    size,
                    &format!("{band} {quality:?} reflection"),
                );
            }
        }
    }
}
