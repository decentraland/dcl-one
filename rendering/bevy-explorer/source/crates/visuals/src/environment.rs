//! Scene-root environment overrides. The scene the player is in owns them; leaving it restores
//! the clock.

use bevy::{
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_asset::RenderAssetUsages,
};
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
                ("Midnight", "/time 0 0"),
                ("Noon", "/time 12 0"),
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
    material: Res<crate::stylized_sky::StylizedSkyMaterialHandle>,
    mut materials: ResMut<Assets<crate::stylized_sky::StylizedSkyMaterial>>,
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

    /// An app running `apply_global_light` at noon, a second into the session, with the fog on
    /// and the scene's sun hidden.
    fn light_app() -> App {
        use crate::{
            sky_envmap::{SkyAmbient, SkyEnvmapSettings},
            stylized_sky::StylizedSky,
            CloudCover, FogDensity, ShadowBiasOverride,
        };
        use bevy_atmosphere::prelude::AtmosphereModel;
        use common::structs::{AppConfig, FogSetting};
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        let mut config = AppConfig::default();
        config.graphics.fog = FogSetting::On;
        app.insert_resource(time)
            .insert_resource(config)
            .insert_resource(TimeOfDay { time: 43200.0 })
            .init_resource::<FogDensity>()
            .init_resource::<CloudCover>()
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
            .insert_resource(AtmosphereModel::new(StylizedSky::default()))
            .init_resource::<AmbientLight>()
            .init_resource::<SkyEnvmapSettings>()
            .init_resource::<ShadowBiasOverride>()
            .init_resource::<SceneEnvironment>()
            .add_systems(Update, crate::apply_global_light);
        app
    }

    fn sky(app: &App) -> &crate::stylized_sky::StylizedSky {
        app.world()
            .resource::<bevy_atmosphere::prelude::AtmosphereModel>()
            .to_ref()
            .unwrap()
    }

    #[test]
    fn fog_sky_and_ambient_overrides_reset_without_bypassing_quality() {
        use crate::{
            sky_envmap::{SkyAmbient, SkyEnvmapSettings},
            sky_preset::{ambient_trilight, fog_falloff_density, time_to_phase},
            stylized_sky::cloud_colors,
        };
        use common::structs::{AppConfig, FogSetting};
        let mut app = light_app();
        app.insert_resource(SceneEnvironment {
            fog_density: Some(0.01),
            zenith: Some(Vec3::X * 2.0),
            nadir: Some(Vec3::Z),
            cloud_opacity: Some(0.0),
            ..default()
        });
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), DistanceFog::default()))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let world = app.world();
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
        let atmosphere = sky(&app);
        assert_eq!(atmosphere.scene_zenith, Vec4::new(2.0, 0.0, 0.0, 1.0));
        assert_eq!(atmosphere.scene_sun_visible, 0.0);
        // the sky follows the clock: the lookup's phase at noon, a further dither step every
        // frame and the scrolls a second on each
        assert_eq!(atmosphere.phase, time_to_phase(0.5));
        assert_eq!(atmosphere.tick, 3);
        assert_eq!(atmosphere.time, 3.0);
        // the scene's cloud opacity, and the preset's colours and speeds where it sets none:
        // three seconds of the overhead strip's 0.0008 turns a second
        assert_eq!(atmosphere.cloud_opacity, 0.0);
        assert_eq!(
            (atmosphere.cloud_lit, atmosphere.cloud_shadow),
            cloud_colors(time_to_phase(0.5), None)
        );
        assert!((atmosphere.cloud_scroll.x - 0.0024).abs() < 1e-6);
        let scrolled = atmosphere.cloud_scroll;
        let ambient = world.resource::<AmbientLight>();
        assert!(ambient.color.to_linear().alpha * ambient.brightness < -1.5);
        // the Off setting is no fog, whatever the scene asks for
        app.world_mut().resource_mut::<AppConfig>().graphics.fog = FogSetting::Off;
        app.update();
        assert!(matches!(
            app.world().get::<DistanceFog>(camera).unwrap().falloff,
            FogFalloff::Exponential { density: 0.0 }
        ));
        // a scene's cloud colour is the lit colour, and its speed turns every strip: half a
        // turn back in this one second, from wherever each was
        *app.world_mut().resource_mut::<SceneEnvironment>() = SceneEnvironment {
            cloud_color: Some(Vec3::new(0.0, 2.0, 0.0)),
            cloud_speed: Some(-std::f32::consts::PI),
            ..default()
        };
        let before = sky(&app).cloud_scroll;
        app.update();
        let atmosphere = sky(&app);
        assert_eq!(atmosphere.cloud_lit, Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(
            atmosphere.cloud_shadow,
            cloud_colors(time_to_phase(0.5), Some(atmosphere.cloud_lit)).1
        );
        assert_eq!(atmosphere.cloud_opacity, 1.0);
        assert!(before.cmpgt(scrolled).all());
        assert!(
            (atmosphere.cloud_scroll - (before + 0.5))
                .abs()
                .max_element()
                < 1e-6
        );
        *app.world_mut().resource_mut::<SceneEnvironment>() = SceneEnvironment::default();
        app.world_mut()
            .resource_mut::<SceneGlobalLight>()
            .sun_hidden = false;
        app.update();
        let world = app.world();
        assert_eq!(world.resource::<SkyAmbient>().bands, preset);
        assert!(
            !world
                .resource::<SkyEnvmapSettings>()
                .authored_lower_hemisphere
        );
        let atmosphere = sky(&app);
        assert_eq!(atmosphere.scene_zenith, Vec4::ZERO);
        assert_eq!(atmosphere.scene_sun_visible, 1.0);
        let ambient = world.resource::<AmbientLight>();
        assert!(ambient.color.to_linear().alpha * ambient.brightness >= -1.0);
    }

    #[test]
    fn the_disc_follows_the_light_and_the_scene_sun() {
        use crate::sky_preset::{
            sample_ramp, time_to_phase, MOON_COLOR_RAMP, MOON_MASK_SIZE, SUN_COLOR_RAMP,
        };
        use crate::stylized_sky::gamma_to_linear;
        use scene_runner::update_world::lights::moon_progress;
        let mut app = light_app();
        // midnight: the light is on the moon's arc and the sky's sun under the horizon
        let moonlight = Vec3::new(0.6, -0.8, 0.0);
        app.world_mut().resource_mut::<TimeOfDay>().time = 0.0;
        let mut light = app.world_mut().resource_mut::<SceneGlobalLight>();
        light.dir_direction = moonlight;
        light.sun_direction = Vec3::Y;
        app.update();
        let atmosphere = sky(&app);
        assert!(atmosphere.body_direction.abs_diff_eq(-moonlight, 1e-6));
        assert_eq!(atmosphere.mask_size, MOON_MASK_SIZE);
        assert_eq!(
            (atmosphere.disc_opacity, atmosphere.backlight_weight),
            (1.0, 1.0)
        );
        assert_eq!(
            atmosphere.disc_color,
            gamma_to_linear(sample_ramp(&MOON_COLOR_RAMP, moon_progress(0.0)))
        );

        // a scene's direction is the light's and the sky sun's: the sun's whole disc is there
        app.world_mut()
            .resource_mut::<SceneGlobalLight>()
            .sun_direction = moonlight;
        app.update();
        let atmosphere = sky(&app);
        assert!(atmosphere.body_direction.abs_diff_eq(-moonlight, 1e-6));
        assert_eq!(atmosphere.mask_size, 0.0);
        assert_eq!(
            atmosphere.disc_color,
            gamma_to_linear(sample_ramp(&SUN_COLOR_RAMP, time_to_phase(0.0)))
        );

        // and its sun colour tints the disc, at the preset's brightness times the authored one above 1
        let scene = app
            .world_mut()
            .spawn(Sun {
                color: ColorRamp::new(&gradient(&[(0.0, 4.0)])),
                visible: None,
                intensity: None,
                direction: None,
            })
            .id();
        app.world_mut().resource_mut::<SceneGlobalLight>().source = Some(scene);
        app.update();
        let preset = gamma_to_linear(sample_ramp(&SUN_COLOR_RAMP, time_to_phase(0.0)));
        assert!(sky(&app)
            .disc_color
            .abs_diff_eq(Vec3::X * preset.max_element() * 4.0, 1e-5));
    }

    #[test]
    fn scene_bands_clear_when_the_component_goes() {
        use common::structs::{OutOfWorld, PrimaryPlayerRes};
        use scene_runner::initialize_scene::{
            LiveScenes, PortableScenes, PortableSource, ScenePointers,
        };
        let mut app = light_app();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .add_systems(Update, update_environment.before(crate::apply_global_light));
        // a player out of the world is in the super-user portables
        let scene = app
            .world_mut()
            .spawn((
                RendererSceneContext::new(
                    dcl_types::SceneId::DUMMY,
                    "hash".to_owned(),
                    "storage_root".to_owned(),
                    true,
                    0,
                    "title".to_owned(),
                    IVec2::ZERO,
                    [IVec2::ZERO].into_iter().collect(),
                    vec![],
                    Default::default(),
                    vec![],
                    Entity::PLACEHOLDER,
                    0.0,
                    false,
                    "sdk_version",
                    false,
                    false,
                ),
                Skybox::from(PbSkybox {
                    sky_colors: Some(pb_skybox::SkyColors {
                        zenith: Some(gradient(&[(0.0, 3.0)])),
                        ..default()
                    }),
                    ..default()
                }),
            ))
            .id();
        let world = app.world_mut();
        world
            .resource_mut::<LiveScenes>()
            .scenes
            .insert("hash".to_owned(), scene);
        world.resource_mut::<PortableScenes>().insert(
            "hash".to_owned(),
            PortableSource::new("pid".to_owned(), None, None, true),
        );
        let player = world
            .spawn((GlobalTransform::default(), OutOfWorld { spawn_point: None }))
            .id();
        world.insert_resource(PrimaryPlayerRes(player));
        app.update();
        assert_eq!(sky(&app).scene_zenith, Vec4::new(3.0, 0.0, 0.0, 1.0));
        app.world_mut().entity_mut(scene).remove::<Skybox>();
        app.update();
        assert_eq!(sky(&app).scene_zenith, Vec4::ZERO);
    }

    /// Compiles the sky's compute shader and the sky material, with and without a panorama, on
    /// a GPU.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn scene_environment_pipelines_compile() {
        use crate::stylized_sky::{
            test_support::{read_cube, sky_test_app, start_sky, texel},
            StylizedSky,
        };
        use bevy::{
            core_pipeline::tonemapping::Tonemapping,
            render::{
                camera::RenderTarget,
                render_resource::{
                    CachedPipelineState, Extent3d, PipelineCache, PipelineDescriptor,
                    TextureDimension, TextureFormat, TextureUsages,
                },
                view::screenshot::{Screenshot, ScreenshotCaptured},
                RenderApp,
            },
        };
        use bevy_atmosphere::prelude::*;
        use pipeline_test::{compile_pipelines, render_target};
        #[derive(Resource, Default)]
        struct Captured(Option<Image>);
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
        let mut app = sky_test_app(128);
        app.init_resource::<SceneEnvironment>()
            .init_resource::<Captured>()
            .init_resource::<common::structs::AppConfig>()
            .add_plugins(console::ConsolePlugin {
                add_bevy_console: false,
            })
            .add_plugins(crate::sky_envmap::SkyEnvmapPlugin)
            .add_plugins(crate::stylized_sky::StylizedSkyMaterialPlugin)
            .add_systems(Update, update_sky_panorama);
        let raw_sky = start_sky(&mut app);
        let maps = app
            .world()
            .resource::<crate::sky_envmap::SkyEnvmap>()
            .clone();
        // the ambient map is written on the CPU and takes no part in the sky's passes
        for handle in [&maps.specular, &maps.trilight_specular] {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(handle)
                .unwrap()
                .texture_descriptor
                .usage |= TextureUsages::COPY_SRC;
        }
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
        let panorama = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(sky_image);
        app.world_mut().resource_mut::<SceneEnvironment>().texture = Some(panorama);
        app.world_mut()
            .resource_mut::<crate::sky_envmap::SkyEnvmapSettings>()
            .authored_lower_hemisphere = true;
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
            let atmosphere = model.to_mut::<StylizedSky>().unwrap();
            atmosphere.scene_zenith = Vec4::new(0.0, 0.0, 1.0, 1.0);
            atmosphere.scene_horizon = atmosphere.scene_zenith;
            atmosphere.scene_nadir = atmosphere.scene_zenith;
        }
        // the sky without the image is another skybox pipeline: wait for it, then for every
        // dither step of the cube
        compile_pipelines(&mut app);
        for _ in 0..80 {
            app.update();
        }
        let restored = capture(&mut app, &target, "procedural-restored");
        assert!(
            restored[2] > 200 && restored[0] < 30 && restored[1] < 30,
            "removing image restores authored procedural sky: {restored:?}"
        );
        // Under the horizon the lookup holds its horizon colour down to the pole, where the
        // azimuth is undefined; only an authored nadir replaces it. A horizontal screenshot
        // cannot detect a NaN at face -Y.
        // the sky decodes the lookup from sRGB as its last step
        let lut_horizon = crate::stylized_sky::sky_rgb_to_linear(Vec3::from_slice(
            &crate::sky_preset::bake_sky_lut()[2 * crate::sky_preset::SKY_LUT_WIDTH][..3],
        ));
        for band in ["nadir", "zenith", "horizon", "rim", "none"] {
            {
                let mut model = app.world_mut().resource_mut::<AtmosphereModel>();
                let atmosphere = model.to_mut::<StylizedSky>().unwrap();
                atmosphere.phase = 0.5;
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
                let raw = read_cube(&mut app, &raw_sky, 128, band);
                let pole = texel(&raw, 128, 3, 64, 64);
                let expected = if band == "nadir" {
                    [1.0, 0.0, 0.0, 0.0]
                } else {
                    [lut_horizon[0], lut_horizon[1], lut_horizon[2], 0.0]
                };
                for channel in 0..4 {
                    assert!(
                        (pole[channel] - expected[channel]).abs() < 1e-2,
                        "{band}: raw -Y pole {pole:?}, expected {expected:?}"
                    );
                }
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
                read_cube(
                    &mut app,
                    handle,
                    size,
                    &format!("{band} {quality:?} reflection"),
                );
            }
        }
    }
}
