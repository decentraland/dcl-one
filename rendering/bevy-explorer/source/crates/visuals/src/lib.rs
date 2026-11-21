#![forbid(unsafe_code)]

mod coast;
mod day_night;
mod debug_panel;
mod environment;
mod lens_flare;
mod props;
pub mod shell_texturing;
mod sky_envmap;
pub use common::sky_preset;
mod solids;
mod stars;
mod stylized_sky;
pub mod terrain;
mod trees;
pub mod upscale;

use bevy::{
    core_pipeline::dof::{DepthOfField, DepthOfFieldMode},
    pbr::{wireframe::WireframePlugin, DirectionalLightShadowMap},
    prelude::*,
    render::view::{Layer, RenderLayers},
};

use bevy::render::{Render, RenderApp, RenderSet};
use bevy_atmosphere::{
    model::AddAtmosphereModel,
    pipeline::{AtmosphereImageBindGroupLayout, AtmosphereUpdateEvent},
    prelude::{AtmosphereCamera, AtmosphereModel, AtmospherePlugin, AtmosphereSettings},
    system_param::AtmosphereMut,
};
use scene_runner::update_world::lights::Sun;
use stylized_sky::{
    cloud_colors, cloud_scroll_rates, disc_color, disc_params, load_cloud_strips,
    load_horizon_noise, sky_lut_image, stack_cloud_strips, CloudStrips, StylizedSky,
    StylizedSkyMaterialPlugin,
};

use bevy_console::ConsoleCommand;
use common::{
    sets::SetupSets,
    structs::{
        sun_cascades, AppConfig, DofConfig, FogSetting, PrimaryCamera, PrimaryCameraRes,
        PrimaryUser, SceneGlobalLight, StageLight, TimeOfDay, AMBIENT_BRIGHTNESS_DEFAULT,
        PRIMARY_AVATAR_LIGHT_LAYER,
    },
};
use console::DoAddConsoleCommand;

use crate::{
    coast::CoastPlugin,
    day_night::{day_fraction, DayNightPlugin},
    props::PropsPlugin,
    shell_texturing::ShellTexturingPlugin,
    sky_envmap::{SkyAmbient, SkyEnvmapPlugin, SkyEnvmapSettings},
    sky_preset::{
        ambient_trilight, fog_color, fog_falloff_density, time_to_phase, FOG_DENSITY_PER_METRE,
    },
    solids::SolidsPlugin,
    terrain::TerrainPlugin,
    trees::TreesPlugin,
};

pub struct VisualsPlugin {
    pub no_fog: bool,
}

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(debug_panel::VisualsDebugPanelPlugin);
        app.insert_resource(DirectionalLightShadowMap { size: 4096 })
            .init_resource::<SceneGlobalLight>()
            .init_resource::<FogDensity>()
            .init_resource::<CloudCover>()
            .add_plugins(WireframePlugin::default())
            .add_plugins(DayNightPlugin)
            .add_plugins(ShellTexturingPlugin)
            .add_plugins(TerrainPlugin)
            .add_plugins(CoastPlugin)
            .add_plugins((TreesPlugin, PropsPlugin, SolidsPlugin))
            .add_systems(
                Update,
                apply_global_light
                    .before(scene_runner::update_world::lights::update_directional_light),
            )
            .add_systems(Update, update_dof)
            .add_systems(
                Update,
                stack_cloud_strips.run_if(resource_exists::<CloudStrips>),
            )
            .add_systems(Startup, setup.in_set(SetupSets::Main));

        app.insert_resource(AtmosphereSettings {
            resolution: 1024,
            dithering: true,
            skybox_creation_mode:
                bevy_atmosphere::settings::SkyboxCreationMode::FromProjectionFarWithFallback(
                    99999.0,
                ),
        })
        .insert_resource(AtmosphereModel::new(StylizedSky::default()))
        .add_plugins(AtmospherePlugin)
        .add_plugins(StylizedSkyMaterialPlugin)
        .add_plugins(environment::SceneEnvironmentPlugin)
        .add_plugins(SkyEnvmapPlugin)
        .add_plugins(stars::StarsPlugin)
        .add_plugins(lens_flare::LensFlarePlugin)
        .add_plugins(upscale::UpscalePlugin);

        app.add_console_command::<ShadowConsoleCommand, _>(shadow_console_command);
        app.init_resource::<ShadowBiasOverride>();
        app.add_console_command::<ShadowBiasConsoleCommand, _>(shadow_bias_console_command);
        app.add_console_command::<FogConsoleCommand, _>(fog_console_command);
        app.add_console_command::<DofConsoleCommand, _>(dof_console_command);
        app.add_console_command::<CloudConsoleCommand, _>(cloud_console_command);
        app.add_console_command::<TonemapConsoleCommand, _>(tonemap_console_command);
        app.add_console_command::<ExposureConsoleCommand, _>(exposure_console_command);
        app.add_console_command::<GammaConsoleCommand, _>(gamma_console_command);
        app.add_console_command::<SaturationConsoleCommand, _>(saturation_console_command);
        app.add_console_command::<BloomConsoleCommand, _>(bloom_console_command);
    }

    fn finish(&self, app: &mut App) {
        let snapshots_only = app
            .world()
            .resource::<scene_runner::initialize_scene::TestingData>()
            .test_scenes
            .is_some();
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<AtmosphereImageBindGroupLayout>();
        if snapshots_only {
            // The player camera is inactive and snapshot cameras have no sky or
            // environment map. Do not draw an unused 1024px sky cubemap every
            // frame; scene lighting and snapshot rendering still run normally.
            render_app.add_systems(
                Render,
                (|mut updates: ResMut<Events<AtmosphereUpdateEvent>>| updates.clear())
                    .in_set(RenderSet::PrepareBindGroups),
            );
        }

        app.add_atmosphere_model::<StylizedSky>();
    }
}

#[derive(Component)]
struct DirectionalLightLayer(Layer);

fn setup(
    mut commands: Commands,
    camera: Res<PrimaryCameraRes>,
    mut atmosphere: AtmosphereMut<StylizedSky>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    info!("visuals::setup");

    commands.entity(camera.0).try_insert(DistanceFog {
        color: fog_color(0.5),
        directional_light_color: Color::NONE,
        directional_light_exponent: 0.0,
        falloff: FogFalloff::Exponential {
            density: fog_falloff_density(FOG_DENSITY_PER_METRE),
        },
    });

    {
        commands.entity(camera.0).try_insert(AtmosphereCamera {
            render_layers: Some(RenderLayers::default()),
        });

        atmosphere.lut = images.add(sky_lut_image());
        atmosphere.noise = load_horizon_noise(&asset_server);
        commands.insert_resource(load_cloud_strips(&asset_server));
    }
}

static TRANSITION_TIME: f32 = 1.0;

fn apply_global_light(
    mut commands: Commands,
    setting: Res<AppConfig>,
    mut atmosphere: AtmosphereMut<StylizedSky>,
    mut sun: Query<(
        Entity,
        &DirectionalLightLayer,
        &mut Transform,
        &mut DirectionalLight,
    )>,
    mut ambient: ResMut<AmbientLight>,
    time: Res<Time>,
    mut fogs: Query<&mut DistanceFog, With<Camera3d>>,
    scene_light: (Res<SceneGlobalLight>, Res<environment::SceneEnvironment>),
    scene_suns: Query<&Sun>,
    time_of_day: Res<TimeOfDay>,
    mut envmap_settings: ResMut<SkyEnvmapSettings>,
    mut sky_ambient: ResMut<SkyAmbient>,
    fog_density: Res<FogDensity>,
    shadow_bias: Res<ShadowBiasOverride>,
    cloud: Res<CloudCover>,
    mut prev: Local<(f32, SceneGlobalLight)>,
) {
    let (scene_global_light, environment) = scene_light;
    // the transition has settled once the previous output exactly matches the target
    let settled = prev.0 >= TRANSITION_TIME && prev.1 == *scene_global_light;

    let next_light = if prev.0 >= TRANSITION_TIME && prev.1.source == scene_global_light.source {
        scene_global_light.clone()
    } else {
        // transition part way
        let new_amount = if prev.1.source == scene_global_light.source {
            (time.delta_secs() / (TRANSITION_TIME - prev.0)).clamp(0.0, 1.0)
        } else {
            time.delta_secs() / TRANSITION_TIME
        };
        let old_amount = 1.0 - new_amount;
        SceneGlobalLight {
            source: scene_global_light.source,
            dir_color: (scene_global_light.dir_color.to_srgba() * new_amount
                + prev.1.dir_color.to_srgba() * old_amount)
                .into(),
            dir_illuminance: scene_global_light.dir_illuminance * new_amount
                + prev.1.dir_illuminance * old_amount,
            dir_direction: prev
                .1
                .dir_direction
                .lerp(scene_global_light.dir_direction, new_amount),
            sun_illuminance: scene_global_light.sun_illuminance * new_amount
                + prev.1.sun_illuminance * old_amount,
            sun_direction: prev
                .1
                .sun_direction
                .lerp(scene_global_light.sun_direction, new_amount),
            ambient_color: (scene_global_light.ambient_color.to_srgba() * new_amount
                + prev.1.ambient_color.to_srgba() * old_amount)
                .into(),
            ambient_brightness: scene_global_light.ambient_brightness * new_amount
                + prev.1.ambient_brightness * old_amount,
            sun_hidden: scene_global_light.sun_hidden,
            layers: scene_global_light.layers.clone(),
        }
    };

    let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, next_light.dir_direction);

    // the sky follows the clock: the lookup's phase, the scrolls and the next dither step. a
    // scene's own sky colours blend over the lookup, and its cloud colour, opacity and speed
    // replace the preset's
    let clock_fraction = day_fraction(time_of_day.elapsed_secs() / 3600.0);
    atmosphere.phase = time_to_phase(clock_fraction);
    atmosphere.tick = atmosphere.tick.wrapping_add(1);
    let band = |color: Option<Vec3>| color.map(|c| c.extend(1.0)).unwrap_or(Vec4::ZERO);
    atmosphere.scene_zenith = band(environment.zenith);
    atmosphere.scene_horizon = band(environment.horizon);
    atmosphere.scene_nadir = band(environment.nadir);
    atmosphere.scene_rim = band(environment.rim);
    let procedural_clouds = environment.texture.is_none();
    (atmosphere.cloud_lit, atmosphere.cloud_shadow) = cloud_colors(
        atmosphere.phase,
        environment.cloud_color.filter(|_| procedural_clouds),
    );
    advance_sky_clouds(
        &mut atmosphere,
        environment.cloud_opacity,
        environment.cloud_speed,
        &cloud,
        time.delta_secs(),
    );
    atmosphere.scene_sun_visible = if next_light.sun_hidden { 0.0 } else { 1.0 };

    // the disc is where the light comes from, so it follows the swing between the sun's arc and
    // the moon's, and a scene's own direction. A scene's direction is written to the light and
    // to the sky's sun alike. Without one the two differ whenever the moon is the body, which
    // is the only time the flag changes the disc
    let scene_direction = scene_global_light.dir_direction == scene_global_light.sun_direction;
    let disc = disc_params(clock_fraction, scene_direction);
    let scene_sun_color = scene_global_light
        .source
        .and_then(|scene| scene_suns.get(scene).ok())
        .and_then(|sun| sun.color.as_ref())
        .map(|ramp| ramp.sample(clock_fraction));
    atmosphere.set_disc(&disc);
    atmosphere.disc_color = disc_color(&disc, clock_fraction, scene_sun_color);
    atmosphere.body_direction = (-next_light.dir_direction)
        .try_normalize()
        .unwrap_or(atmosphere.body_direction);
    // the backlight fades with the disc, so it changes bodies unseen
    atmosphere.backlight_weight = disc.opacity;

    // the fog and the Trilight ambient follow the clock, not the scene light, so they are kept
    // out of the gate below; each is written only when its value changes, which also covers a
    // (re)inserted DistanceFog.
    // a scene's fog component replaces the colour and the density; the density is in the same
    // units as the preset's either way
    let (new_fog_color, new_fog_density) = match setting.graphics.fog {
        FogSetting::Off => (fog_color(clock_fraction), 0.0),
        FogSetting::On => (
            environment
                .fog_color
                .map(|color| Color::linear_rgb(color.x, color.y, color.z))
                .unwrap_or_else(|| fog_color(clock_fraction)),
            fog_falloff_density(environment.fog_density.unwrap_or(fog_density.0)),
        ),
    };
    for mut fog in fogs.iter_mut() {
        let written = fog.color == new_fog_color
            && fog.directional_light_color == Color::NONE
            && matches!(
                fog.falloff,
                FogFalloff::Exponential { density } if density == new_fog_density
            );
        if !written {
            fog.color = new_fog_color;
            fog.directional_light_color = Color::NONE;
            fog.falloff = FogFalloff::Exponential {
                density: new_fog_density,
            };
        }
    }

    // the diffuse ambient light is the Trilight ambient map (sky_envmap.rs): the preset's three
    // colours at the phase, or a scene's own sky colours where it authors them (zenith, horizon
    // and nadir for sky, equator and ground). A scene's ambient override tints and scales all
    // three, and so does the setting relative to its default, authored colours included
    let ambient_tint = next_light.ambient_color.to_linear().to_vec3()
        * next_light.ambient_brightness
        * (setting.graphics.ambient_brightness as f32 / AMBIENT_BRIGHTNESS_DEFAULT as f32);
    let [sky, equator, ground] = ambient_trilight(clock_fraction);
    let bands = [
        environment.zenith.unwrap_or(sky),
        environment.horizon.unwrap_or(equator),
        environment.nadir.unwrap_or(ground),
    ]
    .map(|band| band * ambient_tint);
    if sky_ambient.bands != bands {
        sky_ambient.bands = bands;
    }

    // skip the light and flat ambient writes (which trigger change detection and re-extraction)
    // when the light has settled and nothing else affecting them has changed
    // an authored nadir: the envmap generates the lower hemisphere, and the pbr lookups stop
    // mirroring it (flagged in the ambient alpha below)
    let authored_lower = environment.nadir.is_some();
    if envmap_settings.authored_lower_hemisphere != authored_lower {
        envmap_settings.authored_lower_hemisphere = authored_lower;
    }
    let skip_writes = settled
        && !environment.is_changed()
        && !setting.is_changed()
        && !envmap_settings.is_changed()
        && !shadow_bias.is_changed();

    if skip_writes {
        prev.0 += time.delta_secs();
        prev.1 = next_light;
        return;
    }

    let (shadow_depth_bias, shadow_normal_bias) = shadow_bias.0.unwrap_or(SUN_SHADOW_BIAS);
    let mut directional_layers = RenderLayers::none();
    for (entity, layer, mut light_trans, mut directional) in sun.iter_mut() {
        if !next_light.layers.intersects(&RenderLayers::layer(layer.0)) {
            commands.entity(entity).despawn();
            continue;
        }

        directional_layers = directional_layers.with(layer.0);
        light_trans.rotation = rotation;
        directional.illuminance = next_light.dir_illuminance;
        directional.color = next_light.dir_color;
        directional.shadow_depth_bias = shadow_depth_bias;
        directional.shadow_normal_bias = shadow_normal_bias;
    }

    for new_layer in next_light
        .layers
        .symmetric_difference(&directional_layers)
        .iter()
    {
        let mut layer = RenderLayers::layer(new_layer);
        if new_layer == 0 {
            layer = layer.union(&PRIMARY_AVATAR_LIGHT_LAYER);
        }

        let (shadows_enabled, cascade_shadow_config) = match sun_cascades(
            setting.graphics.shadow_settings,
            setting.graphics.shadow_distance,
        ) {
            Some(cascades) => (true, cascades),
            None => (false, Default::default()),
        };

        commands.spawn((
            DirectionalLight {
                color: next_light.dir_color,
                illuminance: next_light.dir_illuminance,
                shadows_enabled,
                shadow_depth_bias,
                shadow_normal_bias,
                ..Default::default()
            },
            Transform::default().with_rotation(rotation),
            cascade_shadow_config,
            layer,
            DirectionalLightLayer(new_layer),
        ));
    }

    // the flat ambient light is only the target the pbr shaders level the sky reflections
    // towards (they zero it as a light where the sky envmap is bound): the setting's brightness,
    // scaled and tinted by a scene's ambient override, the envmap already following the time
    // of day. the compression rides in the alpha, negated; a further -2 flags an authored lower
    // sky (see sky_envmap_lookup.wgsl). the uniform holds alpha * brightness. the brightness is
    // kept above zero so the compression and the flag survive a
    // zero setting, levelling the envmap to ~nothing
    const MIN_AMBIENT_BRIGHTNESS: f32 = 0.01;
    ambient.brightness = (setting.graphics.ambient_brightness as f32
        * 20.0
        * envmap_settings.floor
        * next_light.ambient_brightness)
        .max(MIN_AMBIENT_BRIGHTNESS);
    ambient.color = next_light.ambient_color.with_alpha(
        -(envmap_settings.compression + if authored_lower { 2.0 } else { 0.0 })
            / ambient.brightness,
    );

    if prev.1.source == scene_global_light.source {
        prev.0 += time.delta_secs()
    } else {
        prev.0 = time.delta_secs()
    };
    prev.1 = next_light;
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/shadows")]
struct ShadowConsoleCommand {
    on: Option<bool>,
}

fn shadow_console_command(
    mut input: ConsoleCommand<ShadowConsoleCommand>,
    // the lobby stage light follows the Shadows setting, not this switch
    mut lights: Query<&mut DirectionalLight, Without<StageLight>>,
) {
    if let Some(Ok(command)) = input.take() {
        for mut light in lights.iter_mut() {
            light.shadows_enabled = command.on.unwrap_or(!light.shadows_enabled);
        }

        input.reply_ok(format!(
            "shadows {}",
            match command.on {
                None => "toggled",
                Some(true) => "enabled",
                Some(false) => "disabled",
            }
        ));
    }
}

/// The sun's shadow depth bias (a world length) and normal bias (shadow map texels): bevy's
/// defaults (see decisions.md).
pub(crate) const SUN_SHADOW_BIAS: (f32, f32) = (
    DirectionalLight::DEFAULT_SHADOW_DEPTH_BIAS,
    DirectionalLight::DEFAULT_SHADOW_NORMAL_BIAS,
);

/// The sun shadow biases set live by `/shadowbias`, in place of `SUN_SHADOW_BIAS`.
#[derive(Resource, Default)]
pub struct ShadowBiasOverride(pub Option<(f32, f32)>);

/// Set the sun shadow depth bias (world length) or normal bias (shadow map texels), keeping the
/// other at its current value; `--reset` restores the default biases.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/shadowbias")]
struct ShadowBiasConsoleCommand {
    #[arg(long)]
    depth: Option<f32>,
    #[arg(long)]
    normal: Option<f32>,
    #[arg(long, conflicts_with_all = ["depth", "normal"])]
    reset: bool,
}

/// The override after a `/shadowbias` command, or an error for a negative bias.
fn next_shadow_bias(
    current: Option<(f32, f32)>,
    preset: (f32, f32),
    depth: Option<f32>,
    normal: Option<f32>,
    reset: bool,
) -> Result<Option<(f32, f32)>, &'static str> {
    if reset {
        return Ok(None);
    }
    if depth.is_some_and(|d| d < 0.0) || normal.is_some_and(|n| n < 0.0) {
        return Err("shadow biases cannot be negative");
    }
    if depth.is_none() && normal.is_none() {
        return Ok(current);
    }
    let (d, n) = current.unwrap_or(preset);
    Ok(Some((depth.unwrap_or(d), normal.unwrap_or(n))))
}

fn shadow_bias_console_command(
    mut input: ConsoleCommand<ShadowBiasConsoleCommand>,
    mut bias: ResMut<ShadowBiasOverride>,
) {
    if let Some(Ok(command)) = input.take() {
        let preset = SUN_SHADOW_BIAS;
        match next_shadow_bias(bias.0, preset, command.depth, command.normal, command.reset) {
            Ok(next) => {
                // only write when it changes, so a query does not rewrite the lights
                if bias.0 != next {
                    bias.0 = next;
                }
                let (depth, normal) = next.unwrap_or(preset);
                input.reply_ok(format!(
                    "shadow bias depth {depth:.3} normal {normal:.3} (preset {:.3} {:.3})",
                    preset.0, preset.1
                ));
            }
            Err(error) => input.reply_failed(error),
        }
    }
}

/// The fog density in the sky preset's units (`sky_preset::fog_falloff_density`), the preset's by
/// default; `/fog` sets it live. A scene's fog component takes its place while it applies.
#[derive(Resource)]
struct FogDensity(f32);

impl Default for FogDensity {
    fn default() -> Self {
        Self(FOG_DENSITY_PER_METRE)
    }
}

/// switch the fog on or off, and optionally set its density per metre (default 0.0005):
/// `/fog [true|false] [density]`
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/fog")]
struct FogConsoleCommand {
    on: Option<bool>,
    density: Option<f32>,
}

fn fog_console_command(
    mut input: ConsoleCommand<FogConsoleCommand>,
    mut config: ResMut<AppConfig>,
    mut fog_density: ResMut<FogDensity>,
) {
    if let Some(Ok(command)) = input.take() {
        let activate = command.on.unwrap_or(true);
        if let Some(density) = command.density {
            fog_density.0 = density.max(0.0);
        }

        config.graphics.fog = if activate {
            FogSetting::On
        } else {
            FogSetting::Off
        };

        input.reply_ok(format!(
            "fog {}, density {}",
            match activate {
                true => "enabled",
                false => "disabled",
            },
            fog_density.0
        ));
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/dof")]
struct DofConsoleCommand {
    focal_distance_extra: f32,
    sensor_height: f32,
    fstops: f32,
    max_circle: f32,
    max_depth: f32,
    mode: usize,
}

fn dof_console_command(
    mut input: ConsoleCommand<DofConsoleCommand>,
    mut cam: Query<(&mut DepthOfField, &mut DofConfig)>,
) {
    if let Some(Ok(command)) = input.take() {
        let Ok((mut dof, mut mydof)) = cam.single_mut() else {
            return;
        };

        *dof = DepthOfField {
            mode: if command.mode == 0 {
                DepthOfFieldMode::Gaussian
            } else {
                DepthOfFieldMode::Bokeh
            },
            aperture_f_stops: command.fstops,
            max_circle_of_confusion_diameter: command.max_circle,
            max_depth: command.max_depth,
            ..Default::default()
        };
        mydof.default_sensor_height = command.sensor_height;
        mydof.extra_focal_distance = command.focal_distance_extra;
        input.reply_ok("");
    }
}

fn update_dof(
    mut cam: Query<(&Transform, &DofConfig, &mut DepthOfField), With<PrimaryCamera>>,
    player: Query<&Transform, With<PrimaryUser>>,
) {
    let (Ok((cam, mydof, mut dof)), Ok(player)) = (cam.single_mut(), player.single()) else {
        return;
    };

    // let base_distance = 10.0;
    let constant_distance = mydof.extra_focal_distance;
    let current_distance = ((cam.translation - (player.translation + Vec3::Y * 1.81)).length()
        + constant_distance)
        .min(100.0);

    dof.sensor_height = mydof.default_sensor_height;
    // * ((current_distance * (current_distance + constant_distance))
    //     / (base_distance * (base_distance + constant_distance)))
    //     .sqrt();
    dof.focal_distance = current_distance;
}

/// `/cloud <cover> [speed]`: the strips' cover and the sky's speed. `--speed` sets the speed
/// alone, for the debug panel's slider; no argument reports both.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/cloud")]
struct CloudConsoleCommand {
    cover: Option<f32>,
    #[arg(conflicts_with = "speed_flag")]
    speed: Option<f32>,
    #[arg(long = "speed")]
    speed_flag: Option<f32>,
}

/// The user's sky settings. A scene's `PBSkybox` cloud opacity and speed apply on top of them.
#[derive(Resource)]
pub struct CloudCover {
    /// Multiplies every strip's opacity: 0 clears the sky, 1 draws the strips as authored.
    pub cover: f32,
    /// Scales the strips' scroll and the horizon noise's drift: 0 holds the sky in a fixed pose.
    pub speed: f32,
}

impl Default for CloudCover {
    fn default() -> Self {
        Self {
            cover: 1.0,
            speed: 1.0,
        }
    }
}

/// Sets the user's cloud cover and speed. A value that is not finite is refused, whole, before
/// either is set; cover is held to 0 to 1.
fn apply_cloud_settings(
    cloud: &mut CloudCover,
    cover: Option<f32>,
    speed: Option<f32>,
) -> Result<(), String> {
    if cover.is_some_and(|cover| !cover.is_finite())
        || speed.is_some_and(|speed| !speed.is_finite())
    {
        return Err("cloud cover and speed must be finite numbers".to_string());
    }
    if let Some(cover) = cover {
        cloud.cover = cover.clamp(0.0, 1.0);
    }
    if let Some(speed) = speed {
        cloud.speed = speed;
    }
    Ok(())
}

fn cloud_console_command(
    mut input: ConsoleCommand<CloudConsoleCommand>,
    mut cloud: ResMut<CloudCover>,
) {
    if let Some(Ok(command)) = input.take() {
        if let Err(error) = apply_cloud_settings(
            &mut cloud,
            command.cover,
            command.speed.or(command.speed_flag),
        ) {
            input.reply_failed(error);
            return;
        }

        input.reply_ok(format!(
            "cloud cover {}, speed {}",
            cloud.cover, cloud.speed
        ));
    }
}

/// Seconds in one repeat of the horizon noise's drift (0.002 turns a second).
const HORIZON_NOISE_PERIOD_SECS: f32 = 500.0;

/// Advances the sky's motion by a frame and sets the strips' opacity: the scene's opacity (1
/// without one) times the user's cover, and the scrolls and the shader time at the user's speed.
fn advance_sky_clouds(
    sky: &mut StylizedSky,
    scene_opacity: Option<f32>,
    scene_speed: Option<f32>,
    cloud: &CloudCover,
    delta_secs: f32,
) {
    let delta = delta_secs * cloud.speed;
    // speed 0 is a fixed pose: the horizon noise and the strips sit at their origin. The time
    // wraps at 500 s, one repeat of the horizon noise at the preset's speed, so it stays small
    sky.time = if cloud.speed == 0.0 {
        0.0
    } else {
        (sky.time + delta).rem_euclid(HORIZON_NOISE_PERIOD_SECS)
    };
    sky.cloud_opacity = scene_opacity.unwrap_or(1.0) * cloud.cover;
    // whole turns are dropped, so the scroll stays exact however long it runs
    sky.cloud_scroll = if cloud.speed == 0.0 {
        Vec3::ZERO
    } else {
        (sky.cloud_scroll + cloud_scroll_rates(scene_speed) * delta).fract_gl()
    };
}

// --- environment grading commands ---
// expose the bevy post stack (tonemap + color grading + bloom) for live
// tuning so the environment mood can be matched by eye against a reference.

/// set the tonemapping curve.
/// options: none, reinhard, reinhard_luma, aces (default), agx, sbdt, tmmf, blender
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/tonemap")]
struct TonemapConsoleCommand {
    mode: String,
}

fn tonemap_console_command(
    mut input: ConsoleCommand<TonemapConsoleCommand>,
    mut cam: Query<&mut bevy::core_pipeline::tonemapping::Tonemapping, With<PrimaryCamera>>,
) {
    use bevy::core_pipeline::tonemapping::Tonemapping;
    if let Some(Ok(command)) = input.take() {
        let Ok(mut tonemapping) = cam.single_mut() else {
            return;
        };
        let mode = match command.mode.as_str() {
            "none" => Tonemapping::None,
            "reinhard" => Tonemapping::Reinhard,
            "reinhard_luma" => Tonemapping::ReinhardLuminance,
            "aces" => Tonemapping::AcesFitted,
            "agx" => Tonemapping::AgX,
            "sbdt" => Tonemapping::SomewhatBoringDisplayTransform,
            "tmmf" => Tonemapping::TonyMcMapface,
            "blender" => Tonemapping::BlenderFilmic,
            other => {
                input.reply_failed(format!("unknown mode `{other}`; options: none, reinhard, reinhard_luma, aces, agx, sbdt, tmmf, blender"));
                return;
            }
        };
        *tonemapping = mode;
        input.reply_ok(format!("tonemapping: {}", command.mode));
    }
}

/// set global exposure (stops, default 0.0)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/exposure")]
struct ExposureConsoleCommand {
    exposure: f32,
}

fn exposure_console_command(
    mut input: ConsoleCommand<ExposureConsoleCommand>,
    mut cam: Query<&mut bevy::render::view::ColorGrading, With<PrimaryCamera>>,
) {
    if let Some(Ok(command)) = input.take() {
        let Ok(mut grading) = cam.single_mut() else {
            return;
        };
        grading.global.exposure = command.exposure;
        input.reply_ok(format!("exposure: {}", command.exposure));
    }
}

/// set gamma per tonal range (default 1.0 = neutral). `/gamma <shadows> [midtones] [highlights]`
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/gamma")]
struct GammaConsoleCommand {
    shadows: f32,
    midtones: Option<f32>,
    highlights: Option<f32>,
}

fn gamma_console_command(
    mut input: ConsoleCommand<GammaConsoleCommand>,
    mut cam: Query<&mut bevy::render::view::ColorGrading, With<PrimaryCamera>>,
) {
    if let Some(Ok(command)) = input.take() {
        let Ok(mut grading) = cam.single_mut() else {
            return;
        };
        let midtones = command.midtones.unwrap_or(command.shadows);
        let highlights = command.highlights.unwrap_or(midtones);
        grading.shadows.gamma = command.shadows;
        grading.midtones.gamma = midtones;
        grading.highlights.gamma = highlights;
        input.reply_ok(format!(
            "gamma: shadows {} midtones {} highlights {}",
            command.shadows, midtones, highlights
        ));
    }
}

/// set saturation per tonal range (default 1.0); 0 = grayscale.
/// `/saturation <shadows> [midtones] [highlights]`
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/saturation")]
struct SaturationConsoleCommand {
    shadows: f32,
    midtones: Option<f32>,
    highlights: Option<f32>,
}

fn saturation_console_command(
    mut input: ConsoleCommand<SaturationConsoleCommand>,
    mut cam: Query<&mut bevy::render::view::ColorGrading, With<PrimaryCamera>>,
) {
    if let Some(Ok(command)) = input.take() {
        let Ok(mut grading) = cam.single_mut() else {
            return;
        };
        let midtones = command.midtones.unwrap_or(command.shadows);
        let highlights = command.highlights.unwrap_or(midtones);
        grading.shadows.saturation = command.shadows;
        grading.midtones.saturation = midtones;
        grading.highlights.saturation = highlights;
        input.reply_ok(format!(
            "saturation: shadows {} midtones {} highlights {}",
            command.shadows, midtones, highlights
        ));
    }
}

/// set bloom intensity (0 = off); the bloom setting sets it again when it changes
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/bloom")]
struct BloomConsoleCommand {
    intensity: f32,
}

fn bloom_console_command(
    mut input: ConsoleCommand<BloomConsoleCommand>,
    mut cam: Query<&mut bevy::core_pipeline::bloom::Bloom, With<PrimaryCamera>>,
) {
    if let Some(Ok(command)) = input.take() {
        let Ok(mut bloom) = cam.single_mut() else {
            return;
        };
        bloom.intensity = command.intensity;
        input.reply_ok(format!("bloom intensity: {}", command.intensity));
    }
}

#[cfg(test)]
mod tests {
    use super::{advance_sky_clouds, apply_cloud_settings, next_shadow_bias, CloudCover};
    use crate::stylized_sky::StylizedSky;
    use bevy::math::Vec3;

    #[test]
    fn cloud_cover_zero_is_a_cloudless_shader_cover() {
        // `/cloud 0 0`: no strip is drawn and nothing in the sky moves
        let cloud = CloudCover {
            cover: 0.0,
            speed: 0.0,
        };
        let mut sky = StylizedSky::default();
        advance_sky_clouds(&mut sky, Some(0.8), Some(1.0), &cloud, 0.5);
        // a run at speed, then the fixed pose
        sky.time = 7.0;
        sky.cloud_scroll = Vec3::new(0.3, 0.2, 0.1);
        advance_sky_clouds(&mut sky, Some(0.8), Some(1.0), &cloud, 0.5);
        assert_eq!(sky.time, 0.0);
        assert_eq!(sky.cloud_scroll, Vec3::ZERO);
        let (scroll, time) = (sky.cloud_scroll, sky.time);
        advance_sky_clouds(&mut sky, Some(0.8), Some(1.0), &cloud, 0.5);
        assert_eq!(sky.cloud_opacity, 0.0);
        assert_eq!(sky.cloud_scroll, scroll);
        assert_eq!(sky.time, time);
    }

    #[test]
    fn a_non_finite_cloud_setting_is_rejected_and_cover_is_capped_at_1() {
        let mut cloud = CloudCover::default();
        assert!(apply_cloud_settings(&mut cloud, Some(1.0), Some(f32::NAN)).is_err());
        assert!(apply_cloud_settings(&mut cloud, Some(f32::INFINITY), None).is_err());
        assert_eq!((cloud.cover, cloud.speed), (1.0, 1.0));
        let mut sky = StylizedSky::default();
        advance_sky_clouds(&mut sky, None, None, &cloud, 0.5);
        assert!(sky.time.is_finite() && sky.cloud_scroll.is_finite());
        apply_cloud_settings(&mut cloud, Some(5.0), None).unwrap();
        assert_eq!(cloud.cover, 1.0);
        apply_cloud_settings(&mut cloud, Some(-2.0), None).unwrap();
        assert_eq!(cloud.cover, 0.0);
    }

    #[test]
    fn the_shader_time_wraps_at_one_horizon_noise_repeat() {
        let mut sky = StylizedSky {
            time: 499.9,
            ..Default::default()
        };
        advance_sky_clouds(&mut sky, None, None, &CloudCover::default(), 0.25);
        assert!((sky.time - 0.15).abs() < 1e-3);
    }

    #[test]
    fn cloud_cover_multiplies_the_scene_opacity_and_speed_scales_the_motion() {
        let mut fast = StylizedSky::default();
        let cloud = CloudCover {
            cover: 0.5,
            speed: 2.0,
        };
        advance_sky_clouds(&mut fast, Some(0.8), None, &cloud, 0.1);
        assert!((fast.cloud_opacity - 0.4).abs() < 1e-6);
        assert!((fast.time - 0.2).abs() < 1e-6);
        let mut normal = StylizedSky::default();
        advance_sky_clouds(&mut normal, None, None, &CloudCover::default(), 0.2);
        assert_eq!(normal.cloud_opacity, 1.0);
        assert!((normal.cloud_scroll - fast.cloud_scroll).length() < 1e-6);
    }

    #[test]
    fn shadow_bias_flags_change_one_bias_at_a_time() {
        let preset = (0.02, 4.375);
        // one flag keeps the other bias, from the preset or from an earlier override
        assert_eq!(
            next_shadow_bias(None, preset, Some(0.05), None, false),
            Ok(Some((0.05, 4.375)))
        );
        assert_eq!(
            next_shadow_bias(Some((0.05, 4.375)), preset, None, Some(1.0), false),
            Ok(Some((0.05, 1.0)))
        );
        // no flag only reports; reset drops the override
        assert_eq!(
            next_shadow_bias(Some((0.05, 1.0)), preset, None, None, false),
            Ok(Some((0.05, 1.0)))
        );
        assert_eq!(
            next_shadow_bias(Some((0.05, 1.0)), preset, None, None, true),
            Ok(None)
        );
        assert!(next_shadow_bias(None, preset, Some(-0.1), None, false).is_err());
    }
}
