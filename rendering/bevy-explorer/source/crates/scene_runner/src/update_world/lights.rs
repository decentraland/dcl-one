use std::f32::consts::PI;

use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};

use bevy::{
    math::FloatOrd,
    pbr::decal::clustered::{CubemapLayout, PointLightTexture, SpotLightTexture},
    prelude::*,
    render::view::RenderLayers,
};
use common::{
    dynamics::PLAYER_COLLIDER_RADIUS,
    sets::SceneSets,
    sky_preset::{
        day_fraction, sample_keys, sample_ramp, time_to_phase, DIRECTIONAL_COLOR_RAMP,
        LIGHT_INTENSITY_BY_TIME, LIGHT_UNIT_LUX,
    },
    structs::{
        AppConfig, PrimaryUser, SceneGlobalLight, TimeOfDay, PRIMARY_AVATAR_LIGHT_LAYER,
        SPOT_SHADOW_MAX_DISTANCE,
    },
    util::TryPushChildrenEx,
};
use dcl_component::{
    proto_components::{
        common::{ColorGradient, TextureUnion, Vector3},
        sdk::components::{pb_light_source, PbGlobalLight, PbLightSource, PbSun},
        Color3DclToBevy,
    },
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;

use crate::{
    renderer_context::RendererSceneContext,
    update_world::material::{TextureResolveError, TextureResolver},
    ContainerEntity, ContainingScene,
};

use super::AddCrdtInterfaceExt;

pub struct LightsPlugin;

impl Plugin for LightsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LightSwitches {
            lights: true,
            shadows: true,
            point_shadows: false,
        })
        .add_console_command::<LightCommand, _>(light_command)
        .add_preview_console_command::<SetLightSwitch, _>(set_light_switch)
        .add_preview_debug_inputs(DebugSection {
            title: "Light sources",
            query: "/scene_lights",
            fields: &[
                ("SDK lights", "count"),
                ("Visible SDK lights", "visible"),
                ("Active shadow casters", "casters"),
                ("Shadow caster slots", "casterSlots"),
                ("Shadow caster range from the player (m)", "casterRange"),
                ("Nearest caster distance (m)", "nearestCaster"),
            ],
            actions: &[],
            inputs: &[
                DebugInput {
                    label: "Lights",
                    field: "lights",
                    command: "/set_scene_lights lights {value}",
                    kind: DebugInputKind::Toggle,
                },
                DebugInput {
                    label: "Shadows",
                    field: "shadows",
                    command: "/set_scene_lights shadows {value}",
                    kind: DebugInputKind::Toggle,
                },
                DebugInput {
                    label: "Point light shadows",
                    field: "pointShadows",
                    command: "/set_scene_lights point-shadows {value}",
                    kind: DebugInputKind::Toggle,
                },
            ],
        });
        app.add_crdt_lww_component::<PbLightSource, LightSource>(
            SceneComponentId::LIGHT_SOURCE,
            ComponentPosition::Any,
        );
        app.add_crdt_lww_component::<PbGlobalLight, GlobalLight>(
            SceneComponentId::GLOBAL_LIGHT,
            ComponentPosition::RootOnly,
        );
        app.add_crdt_lww_component::<PbSun, Sun>(
            SceneComponentId::SUN,
            ComponentPosition::RootOnly,
        );
        app.add_systems(
            Update,
            (
                update_directional_light,
                update_point_lights,
                manage_shadow_casters,
            )
                .chain()
                .in_set(SceneSets::PostLoop),
        );
    }
}

#[derive(Resource)]
struct LightSwitches {
    lights: bool,
    shadows: bool,
    /// Lets point lights take shadow caster slots too. Off by default: a point light's shadow
    /// renders six faces, so scene shadows go to spot lights; this is a tester's aid, not a
    /// setting.
    point_shadows: bool,
}

/// Inspect the SDK lights and their shadow budget.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_lights")]
struct LightCommand;

/// Toggle a class of SDK lights or shadows for the preview session.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/set_scene_lights")]
struct SetLightSwitch {
    #[arg(value_parser = ["lights", "shadows", "point-shadows"])]
    setting: String,
    #[arg(action = clap::ArgAction::Set)]
    value: bool,
}

fn set_light_switch(mut cmd: ConsoleCommand<SetLightSwitch>, mut switches: ResMut<LightSwitches>) {
    let Some(Ok(args)) = cmd.take() else { return };
    match args.setting.as_str() {
        "lights" => switches.lights = args.value,
        "shadows" => switches.shadows = args.value,
        _ => switches.point_shadows = args.value,
    }
    cmd.reply_ok(format!("{} {}", args.setting, args.value));
}

fn light_command(
    mut cmd: ConsoleCommand<LightCommand>,
    switches: Res<LightSwitches>,
    config: Res<AppConfig>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    lights: Query<
        (
            &Visibility,
            &GlobalTransform,
            Option<&PointLight>,
            Option<&SpotLight>,
        ),
        With<LightEntity>,
    >,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let player = player.single().ok().map(GlobalTransform::translation);
    let mut visible = 0;
    let mut casters = 0;
    let mut nearest: Option<f32> = None;
    for (vis, gt, point, spot) in &lights {
        if *vis != Visibility::Hidden {
            visible += 1;
            if point.is_some_and(|l| l.shadows_enabled) || spot.is_some_and(|l| l.shadows_enabled) {
                casters += 1;
                if let Some(player) = player {
                    let distance = (gt.translation() - player).length();
                    nearest = Some(nearest.map_or(distance, |n| n.min(distance)));
                }
            }
        }
    }
    cmd.reply_ok(serde_json::json!({"lights": switches.lights, "shadows": switches.shadows, "pointShadows":switches.point_shadows,
        "count":lights.iter().len(), "visible":visible, "casters":casters,
        "casterSlots":config.graphics.shadow_caster_count, "casterRange":SPOT_SHADOW_MAX_DISTANCE,
        "nearestCaster":nearest,
        "unavailable": {"nearestCaster": "no light casts a shadow"}}).to_string());
}

#[derive(Component, Debug)]
pub struct LightSource {
    pub enabled: bool,
    /// The scene's own intensity, unset when it gives none.
    pub intensity: Option<f32>,
    /// What a colour above 1 multiplies the intensity by, the colour itself being scaled back
    /// to 1.
    pub intensity_gain: f32,
    pub shadow: Option<bool>,
    pub color: Option<Color>,
    pub range: Option<f32>,
    pub spotlight_angles: Option<(f32, f32)>,
    pub light_texture: Option<TextureUnion>,
}

impl Default for LightSource {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: None,
            intensity_gain: 1.0,
            shadow: None,
            color: None,
            range: None,
            spotlight_angles: None,
            light_texture: None,
        }
    }
}

/// A point or spot light's intensity when the scene gives none.
const DEFAULT_LAMP_INTENSITY: f32 = 16000.0;

impl LightSource {
    /// A point or spot light's intensity: the scene's or the default, times the colour's gain.
    fn lamp_intensity(&self) -> f32 {
        self.intensity.unwrap_or(DEFAULT_LAMP_INTENSITY) * self.intensity_gain
    }
}

impl From<PbLightSource> for LightSource {
    fn from(value: PbLightSource) -> Self {
        // the colour is sRGB. a colour above 1 is rescaled to 1 and multiplies the intensity by
        // max^2.2, the gain decoding it as sRGB would give
        let color_scale = value
            .color
            .map(|c| c.r.max(c.g).max(c.b).max(1.0))
            .unwrap_or(1.0);

        Self {
            enabled: value.active.unwrap_or(true),
            intensity: value.intensity,
            intensity_gain: color_scale.powf(2.2),
            shadow: value.shadow,
            color: value
                .color
                .map(|c| Color::srgb(c.r / color_scale, c.g / color_scale, c.b / color_scale)),
            range: value.range,
            spotlight_angles: if let Some(pb_light_source::Type::Spot(spot)) = value.r#type {
                Some((
                    spot.inner_angle.unwrap_or(21.8),
                    spot.outer_angle.unwrap_or(30.0),
                ))
            } else {
                None
            },
            light_texture: value.shadow_mask_texture,
        }
    }
}

#[derive(Component)]
pub struct GlobalLight {
    pub direction: Option<Vec3>,
    pub ambient_color: Option<Color>,
    pub ambient_brightness: Option<f32>,
}

impl From<PbGlobalLight> for GlobalLight {
    fn from(value: PbGlobalLight) -> Self {
        Self {
            direction: value.direction.as_ref().map(Vector3::world_vec_to_vec3),
            ambient_color: value.ambient_color.map(Color3DclToBevy::convert_linear_rgb),
            ambient_brightness: value.ambient_brightness,
        }
    }
}

/// The sun at noon, lux: the light intensity curve (`common::sky_preset`) peaks at 1.2 there.
pub const NOON_ILLUMINANCE: f32 = 1.2 * LIGHT_UNIT_LUX;
/// The sky's own sun at noon, in the atmosphere's units (`dir_light_intensity`).
const SKY_SUN_ILLUMINANCE: f32 = 7000.0;

/// A scene point or spot light's intensity times this is its intensity in light units: the
/// default 16000 is 8 units.
const LIGHT_SOURCE_INTENSITY_SCALE: f32 = 0.0005;

/// A scene point or spot light's bevy intensity (lumens) from its `PbLightSource` intensity.
pub fn light_source_lumens(intensity: f32) -> f32 {
    intensity * LIGHT_SOURCE_INTENSITY_SCALE * LIGHT_UNIT_LUX * 4.0 * PI
}

/// The largest scene colour or light multiplier: the largest finite half float, as the sky is
/// drawn into 16-bit float textures.
pub const MAX_SCENE_VALUE: f32 = 65504.0;

/// Gradient keys past this many are dropped.
const MAX_GRADIENT_KEYS: usize = 100;

/// Keys a gradient is resampled to when it has more.
const RAMP_KEYS: usize = 8;

/// A scene colour gradient over the day (0 = midnight, 1 = the next midnight), ready to sample:
/// keys in time order, colours finite and in `0..=MAX_SCENE_VALUE`.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorRamp(Vec<(f32, Vec3)>);

impl ColorRamp {
    /// `None` for a gradient with no usable keys. Of keys at the same time the later one wins.
    pub fn new(gradient: &ColorGradient) -> Option<Self> {
        let mut keys: Vec<(f32, Vec3)> = gradient
            .keys
            .iter()
            .take(MAX_GRADIENT_KEYS)
            .filter_map(|key| {
                let c = key.color.as_ref()?;
                let color = Vec3::new(c.r, c.g, c.b);
                (key.time.is_finite() && color.is_finite()).then_some((
                    key.time.clamp(0.0, 1.0),
                    color.clamp(Vec3::ZERO, Vec3::splat(MAX_SCENE_VALUE)),
                ))
            })
            .collect();
        // a stable sort keeps keys at the same time (-0 and 0 included) in their given order
        keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut ramp = Self(Vec::with_capacity(keys.len()));
        for key in keys {
            if ramp.0.last().is_some_and(|previous| previous.0 == key.0) {
                ramp.0.pop();
            }
            ramp.0.push(key);
        }
        if ramp.0.is_empty() {
            return None;
        }
        if ramp.0.len() > RAMP_KEYS {
            ramp = Self(
                (0..RAMP_KEYS)
                    .map(|n| {
                        let t = n as f32 / (RAMP_KEYS - 1) as f32;
                        (t, ramp.sample(t))
                    })
                    .collect(),
            );
        }
        Some(ramp)
    }

    /// The colour at `time` of day, 0 to 1. Before the first key and after the last, that key's.
    pub fn sample(&self, time: f32) -> Vec3 {
        let time = time.clamp(0.0, 1.0);
        let next = self.0.partition_point(|(t, _)| *t < time);
        match (next.checked_sub(1).map(|i| self.0[i]), self.0.get(next)) {
            (Some((t0, c0)), Some(&(t1, c1))) => c0.lerp(c1, (time - t0) / (t1 - t0)),
            (Some((_, c)), None) | (None, Some(&(_, c))) => c,
            (None, None) => Vec3::ZERO,
        }
    }
}

/// A scene's override of the sun, on its root.
#[derive(Component)]
pub struct Sun {
    pub color: Option<ColorRamp>,
    pub visible: Option<bool>,
    pub intensity: Option<f32>,
    pub direction: Option<Vec3>,
}

impl From<PbSun> for Sun {
    fn from(value: PbSun) -> Self {
        Self {
            color: value.color.as_ref().and_then(ColorRamp::new),
            visible: value.visible,
            intensity: value
                .intensity
                .filter(|i| i.is_finite())
                .map(|i| i.clamp(0.0, MAX_SCENE_VALUE)),
            direction: value
                .direction
                .as_ref()
                .and_then(|d| d.world_vec_to_vec3().try_normalize()),
        }
    }
}

/// Unity's sunrise and sunset (hours): the sun is overhead halfway between, at 12:37.
const SUNRISE: f32 = 6.422;
const SUNSET: f32 = 18.828;

/// Unity's moonrise and moonset (hours): the moon is up for six and a half hours around 00:45.
pub const MOONRISE: f32 = 21.504;
pub const MOONSET: f32 = 4.008;

/// Unity's moon arc leans this far from the zenith, so the moon stays low for long shadows.
const MOON_TILT_DEGREES: f32 = 40.0;

/// Direction a body's light travels at `hours` past midnight. Like Unity, the body rises where
/// the sun rises, crosses the sky at a steady rate to the opposite point on an arc leaning
/// `tilt_degrees` from the zenith, and carries on below the horizon to its next rise.
fn celestial_direction(hours: f32, rise: f32, set: f32, tilt_degrees: f32) -> Vec3 {
    let since_rise = (hours - rise).rem_euclid(24.0);
    let up = (set - rise).rem_euclid(24.0);
    // from the rise horizon: 0 to PI while up, PI to 2 PI while down
    let angle = if since_rise < up {
        since_rise / up * PI
    } else {
        PI + (since_rise - up) / (24.0 - up) * PI
    };
    // away from the body as it rises, and away from it at its peak
    let rising = Vec3::new(1.0, 0.0, -1.0).normalize();
    let (tilt_sin, tilt_cos) = tilt_degrees.to_radians().sin_cos();
    let peak = -(Vec3::Y * tilt_cos + Vec3::new(-1.0, 0.0, -1.0).normalize() * tilt_sin);
    rising * angle.cos() + peak * angle.sin()
}

/// Direction the sunlight travels at `hours` past midnight: straight over the zenith in a
/// vertical plane diagonal to the parcel grid.
pub fn sun_direction(hours: f32) -> Vec3 {
    celestial_direction(hours, SUNRISE, SUNSET, 0.0)
}

/// Direction the moonlight travels at `hours` past midnight: the sun's arc leaned to one side,
/// crossed overnight.
pub fn moon_direction(hours: f32) -> Vec3 {
    celestial_direction(hours, MOONRISE, MOONSET, MOON_TILT_DEGREES)
}

/// How far a body is along its circle at `day_fraction`: 0 at moonrise to 1 at moonset above the
/// horizon, then 1 to 2 below it until the next moonrise. Unity's
/// `SkyboxCelestialMath.CelestialProgress` for the moon.
pub fn moon_progress(day_fraction: f32) -> f32 {
    const MIN_ARC_FRACTION: f32 = 1e-4;
    let (rise, set) = (MOONRISE / 24.0, MOONSET / 24.0);
    let above = (set - rise).rem_euclid(1.0).max(MIN_ARC_FRACTION);
    let since_rise = (day_fraction - rise).rem_euclid(1.0);
    if since_rise < above {
        since_rise / above
    } else {
        1.0 + (since_rise - above) / (1.0 - above).max(MIN_ARC_FRACTION)
    }
}

/// The directional light's colour at phase 0.5, the ramp's noon key (sRGB).
pub const SUN_COLOR_NOON: Vec3 = Vec3::new(1.0, 0.98, 0.94);

/// The fraction of a day over which the light swings from the sun's arc to the moon's before
/// moonrise, and back after moonset.
pub const MOON_SWAP_DURATION: f32 = 0.042;
/// How far the light dims mid-swing to hide the move.
const SWAP_INTENSITY_DIP: f32 = 0.99;

/// At `day_fraction`: how far the light has swung onto the moon's arc (0 sun, 1 moon) and the
/// dip hiding the swing (1 at its middle).
pub fn celestial_swap(day_fraction: f32) -> (f32, f32) {
    let (moonrise, moonset) = (MOONRISE / 24.0, MOONSET / 24.0);
    let evening = (day_fraction - (moonrise - MOON_SWAP_DURATION).rem_euclid(1.0)).rem_euclid(1.0)
        / MOON_SWAP_DURATION;
    let morning = (day_fraction - moonset).rem_euclid(1.0) / MOON_SWAP_DURATION;
    let swing = |progress: f32| smoothstep(0.3, 0.7, progress);
    let dip =
        |progress: f32| smoothstep(0.0, 0.25, progress) * (1.0 - smoothstep(0.75, 1.0, progress));
    if evening < 1.0 {
        (swing(evening), dip(evening))
    } else if morning < 1.0 {
        (1.0 - swing(morning), dip(morning))
    } else {
        let moon_up =
            (day_fraction - moonrise).rem_euclid(1.0) < (moonset - moonrise).rem_euclid(1.0);
        (if moon_up { 1.0 } else { 0.0 }, 0.0)
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The directional light at `hours` past midnight: the direction it travels and its illuminance
/// (lux). The intensity is the curve's at the time; the light rides the sun's arc by day and the
/// moon's by night, swung between them under a dip. From sunset until the swing, and from the
/// swing until sunrise, it stays on the sun's arc below the horizon: it grazes walls from under
/// them and leaves floors unlit while the curve fades it.
fn directional_light(hours: f32) -> (Vec3, f32) {
    let day_fraction = day_fraction(hours);
    let sun_direction = sun_direction(hours);
    let (moon_weight, dip) = celestial_swap(day_fraction);
    let direction = Quat::IDENTITY.slerp(
        Quat::from_rotation_arc(sun_direction, moon_direction(hours)),
        moon_weight,
    ) * sun_direction;
    let illuminance = sample_keys(&LIGHT_INTENSITY_BY_TIME, day_fraction)
        * LIGHT_UNIT_LUX
        * (1.0 - dip * SWAP_INTENSITY_DIP);
    (direction, illuminance)
}

/// A root-entity light replaces what it sets on the preset's light: its colour, and its intensity
/// as lux, which the sky's sun takes as the same number in its own scale. Without an intensity
/// the preset's stays, times the gain of a colour above 1. A disabled light turns both off.
fn apply_root_light(global_light: &mut SceneGlobalLight, light: &LightSource) {
    if let Some(color) = light.color {
        global_light.dir_color = color;
    }
    if !light.enabled {
        global_light.dir_illuminance = 0.0;
        global_light.sun_illuminance = 0.0;
        return;
    }
    global_light.dir_illuminance =
        light.intensity.unwrap_or(global_light.dir_illuminance) * light.intensity_gain;
    global_light.sun_illuminance =
        light.intensity.unwrap_or(global_light.sun_illuminance) * light.intensity_gain;
}

pub fn update_directional_light(
    lights: Query<(
        &RendererSceneContext,
        Option<&LightSource>,
        Option<&GlobalLight>,
        Option<&Sun>,
    )>,
    mut global_light: ResMut<SceneGlobalLight>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    time: Res<TimeOfDay>,
) {
    // the colour from the ramp at the phase, the direction and intensity from the time
    let hours = time.elapsed_secs() / 3600.0;
    let sun_direction = sun_direction(hours);
    let (dir_direction, dir_illuminance) = directional_light(hours);
    let dir = sample_ramp(&DIRECTIONAL_COLOR_RAMP, time_to_phase(day_fraction(hours)));

    // the sky's own sun: its energy follows the elevation, a floor keeping the night sky lit
    let elevation = -sun_direction.y;
    let energy = smoothstep(-0.05, 0.3, elevation);

    *global_light = SceneGlobalLight {
        source: None,
        dir_color: Color::srgb(dir.x, dir.y, dir.z),
        dir_illuminance,
        dir_direction,
        sun_illuminance: (energy * SKY_SUN_ILLUMINANCE).max(1500.0),
        sun_direction,
        ambient_color: Color::WHITE,
        ambient_brightness: 1.0,
        sun_hidden: false,
        layers: RenderLayers::default(),
    };

    let Ok(player) = player.single() else {
        return;
    };

    let day_fraction = day_fraction(hours);
    let mut apply = |parcel: Entity,
                     maybe_light: Option<&LightSource>,
                     maybe_global: Option<&GlobalLight>,
                     maybe_sun: Option<&Sun>| {
        global_light.source = Some(parcel);
        if let Some(light) = maybe_light {
            apply_root_light(&mut global_light, light);
        }

        if let Some(global) = maybe_global {
            if let Some(dir) = global.direction {
                global_light.dir_direction = dir;
                global_light.sun_direction = dir;
            }
            if let Some(color) = global.ambient_color {
                global_light.ambient_color = color;
            }
            if let Some(brightness) = global.ambient_brightness {
                global_light.ambient_brightness = brightness;
            };
        }

        if let Some(sun) = maybe_sun {
            if let Some(color) = &sun.color {
                // the ramp is linear: colours > 1 are rescaled to 1, and the light is
                // multiplied by the max, so the sky gets the hue only
                let color = color.sample(day_fraction);
                let color_scale = color.max_element().max(1.0);
                let color = color / color_scale;
                global_light.dir_color = Color::linear_rgb(color.x, color.y, color.z);
                global_light.dir_illuminance *= color_scale;
            }
            // the light only: the sky keeps its time-of-day brightness
            if let Some(intensity) = sun.intensity {
                global_light.dir_illuminance *= intensity;
            }
            if let Some(dir) = sun.direction {
                global_light.dir_direction = dir;
                global_light.sun_direction = dir;
            }
            if let Some(visible) = sun.visible {
                global_light.sun_hidden = !visible;
            }
        }
    };

    if let Some(active_parcel) = containing_scene.get_parcel_oow(player) {
        // try and get settings from active parcel scene first
        if let Ok((_, maybe_light, maybe_global, maybe_sun)) = lights.get(active_parcel) {
            if maybe_light.is_some() || maybe_global.is_some() || maybe_sun.is_some() {
                apply(active_parcel, maybe_light, maybe_global, maybe_sun);
                return;
            }
        }
    }

    // if the primary parcel doesn't specify anything, check any portables
    let mut portable_settings: Option<(
        &str,
        Entity,
        Option<&LightSource>,
        Option<&GlobalLight>,
        Option<&Sun>,
    )> = None;
    for entity in containing_scene.get_portables(false) {
        if let Ok((ctx, maybe_light, maybe_global, maybe_sun)) = lights.get(entity) {
            if maybe_light.is_none() && maybe_global.is_none() && maybe_sun.is_none() {
                continue;
            }

            let apply = match portable_settings {
                None => true,
                Some((existing, ..)) => &*ctx.hash < existing,
            };

            if !apply {
                continue;
            }

            portable_settings = Some((&ctx.hash, entity, maybe_light, maybe_global, maybe_sun));
        }
    }

    if let Some((_, portable, maybe_light, maybe_global, maybe_sun)) = portable_settings {
        apply(portable, maybe_light, maybe_global, maybe_sun);
    }
}

#[derive(Component)]
pub struct LightEntity {
    pub scene: Entity,
    pub enabled: bool,
    pub shadows_enabled: bool,
    pub range: f32,
}

fn update_point_lights(
    q: Query<
        (Entity, &ContainerEntity, &LightSource, Option<&Children>),
        (
            Without<RendererSceneContext>,
            Or<(Changed<LightSource>, With<RetryLightTexture>)>,
        ),
    >,
    scenes: Query<&RendererSceneContext>,
    mut commands: Commands,
    mut removed_points: RemovedComponents<LightSource>,
    children: Query<&Children>,
    child_lights: Query<&LightEntity>,
    mut resolver: TextureResolver,
) {
    for (entity, container, light, maybe_children) in q.iter() {
        commands.entity(entity).remove::<RetryLightTexture>();

        // try and reuse any previous
        let previous_light_entity = maybe_children.and_then(|children| {
            children
                .iter()
                .find(|child| child_lights.get(*child).is_ok())
        });

        let range = light.range.unwrap_or(-1.0);
        let range = if range < 0.0 {
            light.lamp_intensity().powf(0.25)
        } else {
            range.min(light.lamp_intensity().powf(0.25))
        };

        let mut light_cmds = match previous_light_entity {
            Some(prev) => {
                let mut cmds = commands.entity(prev);
                cmds.insert((
                    LightEntity {
                        scene: container.root,
                        enabled: light.enabled,
                        shadows_enabled: light.shadow.unwrap_or(false),
                        range,
                    },
                    Visibility::Hidden,
                ));
                cmds
            }
            None => {
                let mut cmds = commands.spawn((
                    LightEntity {
                        scene: container.root,
                        enabled: light.enabled,
                        shadows_enabled: light.shadow.unwrap_or(false),
                        range,
                    },
                    // light hidden avatars too
                    RenderLayers::default().union(&PRIMARY_AVATAR_LIGHT_LAYER),
                    Visibility::Hidden,
                ));
                let light_id = cmds.id();
                cmds.commands()
                    .entity(entity)
                    .try_push_children(&[light_id]);
                cmds
            }
        };

        let maybe_light_texture = if let Some(texture) = light.light_texture.as_ref() {
            let Ok(ctx) = scenes.get(container.root) else {
                continue;
            };
            let image = texture
                .tex
                .as_ref()
                .map(|tex| resolver.resolve_texture(ctx, tex));
            let resolved_texture = match image {
                Some(Err(TextureResolveError::SourceNotReady)) => {
                    commands.entity(entity).try_insert(RetryLightTexture);
                    continue;
                }
                None | Some(Err(_)) => None,
                Some(Ok(t)) => Some(t),
            };

            resolved_texture.map(|rt| rt.image)
        } else {
            None
        };

        let lumens = light_source_lumens(light.lamp_intensity());
        match light.spotlight_angles {
            Some(angles) => {
                light_cmds.try_insert(SpotLight {
                    color: light.color.unwrap_or(Color::WHITE),
                    intensity: lumens,
                    range,
                    radius: 0.0,
                    shadows_enabled: light.shadow.unwrap_or(false),
                    outer_angle: angles.1.clamp(0.0, 179.0) * PI / 360.0,
                    inner_angle: angles.0.clamp(0.0, angles.1.min(179.0)) * PI / 360.0,
                    ..Default::default()
                });

                if let Some(light_texture) = maybe_light_texture {
                    light_cmds.try_insert(SpotLightTexture {
                        image: light_texture,
                    });
                }

                light_cmds.remove::<(PointLight, PointLightTexture)>();
            }
            None => {
                light_cmds.try_insert(PointLight {
                    color: light.color.unwrap_or(Color::WHITE),
                    intensity: lumens,
                    range,
                    radius: 0.0,
                    shadows_enabled: light.shadow.unwrap_or(false),
                    ..Default::default()
                });

                if let Some(light_texture) = maybe_light_texture {
                    light_cmds.try_insert(PointLightTexture {
                        image: light_texture,
                        cubemap_layout: CubemapLayout::CrossHorizontal,
                    });
                }

                light_cmds.remove::<(SpotLight, SpotLightTexture)>();
            }
        };
    }

    for removed_light in removed_points.read() {
        let Ok(children) = children.get(removed_light) else {
            continue;
        };
        for child in children {
            if child_lights.get(*child).is_ok() {
                commands.entity(*child).despawn();
            }
        }
    }
}

#[derive(Component)]
pub struct RetryLightTexture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    Point,
    Spot,
}

/// Entities to give shadows: spot lights only (point lights
/// too with `point_lights`), within SPOT_SHADOW_MAX_DISTANCE of the player, nearest first, at
/// most `max`.
pub fn shadow_caster_slots(
    candidates: impl IntoIterator<Item = (Entity, LightKind, f32)>,
    max: usize,
    point_lights: bool,
) -> Vec<Entity> {
    let mut spots: Vec<_> = candidates
        .into_iter()
        .filter(|(_, kind, distance)| {
            (point_lights || *kind == LightKind::Spot) && *distance <= SPOT_SHADOW_MAX_DISTANCE
        })
        .collect();
    spots.sort_by_key(|(_, _, distance)| FloatOrd(*distance));
    spots.into_iter().take(max).map(|(e, _, _)| e).collect()
}

fn manage_shadow_casters(
    read_lights: Populated<(
        Entity,
        &GlobalTransform,
        &LightEntity,
        &ChildOf,
        Has<SpotLight>,
    )>,
    mut write_lights: Query<(
        Option<&mut PointLight>,
        Option<&mut SpotLight>,
        &mut Visibility,
    )>,
    parent_visibility: Query<&InheritedVisibility>,
    player: Single<(Entity, &GlobalTransform), With<PrimaryUser>>,
    containing_scene: ContainingScene,
    config: Res<AppConfig>,
    switches: Res<LightSwitches>,
    mut lights: Local<Vec<(Entity, bool, FloatOrd, bool, LightKind, f32)>>,
) {
    let (player, player_gt) = player.into_inner();
    let player_t = player_gt.translation();

    let active_scenes = containing_scene.get_area(player, PLAYER_COLLIDER_RADIUS);

    // collect lights
    lights.extend(
        read_lights
            .iter()
            .map(|(entity, gt, light, parent, is_spot)| {
                let enabled = light.enabled
                    && parent_visibility
                        .get(parent.parent())
                        .is_ok_and(|pv| pv.get());
                let distance = (gt.translation() - player_t).length();
                let importance = if enabled {
                    let importance = light.range / distance;
                    FloatOrd(importance)
                } else {
                    FloatOrd(0.0)
                };
                let kind = if is_spot {
                    LightKind::Spot
                } else {
                    LightKind::Point
                };

                (
                    entity,
                    active_scenes.contains(&light.scene),
                    importance,
                    light.shadows_enabled,
                    kind,
                    distance,
                )
            }),
    );
    // sort by scene-active and distance
    lights.sort_by_key(|(_, scene_active, importance, ..)| (*scene_active, *importance));
    // enable up to limit
    let max_enabled = if switches.lights {
        config.graphics.light_count
    } else {
        0
    };
    let mut max_casters = match config.graphics.shadow_settings {
        common::structs::ShadowSetting::Off => 0,
        _ => config.graphics.shadow_caster_count,
    };
    if !switches.shadows || config.graphics.shadow_distance <= 0. {
        max_casters = 0;
    }
    debug!(
        "found {} lights, enabling up to {} with {} casters",
        lights.len(),
        max_enabled,
        max_casters
    );
    let casters = shadow_caster_slots(
        lights
            .iter()
            .rev()
            .filter(|(_, _, importance, ..)| importance.0 != 0.0)
            .take(max_enabled)
            .filter(|(_, _, _, shadows_enabled, ..)| *shadows_enabled)
            .map(|(entity, _, _, _, kind, distance)| (*entity, *kind, *distance)),
        max_casters,
        switches.point_shadows,
    );
    let mut max_enabled = max_enabled;
    // only write (and trigger change detection) when the target value differs
    for (light, _, importance, ..) in lights.drain(..).rev() {
        let (maybe_p, maybe_s, mut vis) = write_lights.get_mut(light).unwrap();

        if importance.0 != 0.0 && max_enabled > 0 {
            vis.set_if_neq(Visibility::Visible);
            max_enabled -= 1;

            let cast = casters.contains(&light);
            if let Some(mut p) = maybe_p {
                if p.shadows_enabled != cast {
                    p.shadows_enabled = cast;
                }
            }
            if let Some(mut s) = maybe_s {
                if s.shadows_enabled != cast {
                    s.shadows_enabled = cast;
                }
            }
        } else {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::proto_components::common::{Color3, Color4, ColorKey};

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

    fn sample(keys: &[(f32, f32)], time: f32) -> Option<Vec3> {
        ColorRamp::new(&gradient(keys)).map(|ramp| ramp.sample(time))
    }

    #[test]
    fn ramps_sort_clamp_and_keep_later_duplicates_without_midnight_wrap() {
        let keys = [(1.5, 5.0), (0.5, 2.0), (-1.0, 1.0), (0.5, 3.0)];
        assert_eq!(sample(&keys, 0.0), Some(Vec3::X));
        assert_eq!(sample(&keys, 0.25), Some(Vec3::X * 2.0));
        assert_eq!(sample(&keys, 0.5), Some(Vec3::X * 3.0));
        assert_eq!(sample(&keys, 1.0), Some(Vec3::X * 5.0));
        assert_eq!(sample(&[], 0.5), None);
        assert_eq!(sample(&[(f32::NAN, 1.0), (0.0, f32::INFINITY)], 0.5), None);
    }

    #[test]
    fn ramp_colours_are_bounded_to_half_floats() {
        assert_eq!(sample(&[(0.0, 1e30)], 0.5), Some(Vec3::X * MAX_SCENE_VALUE));
        assert_eq!(sample(&[(0.0, -4.0)], 0.5), Some(Vec3::ZERO));
    }

    #[test]
    fn ramps_drop_keys_past_the_bound_and_resample_to_eight() {
        let mut keys: Vec<_> = (0..MAX_GRADIENT_KEYS)
            .map(|n| (n as f32 / (MAX_GRADIENT_KEYS - 1) as f32, n as f32))
            .collect();
        keys.push((1.0, 999.0));
        let ramp = ColorRamp::new(&gradient(&keys)).unwrap();
        assert_eq!(ramp.0.len(), RAMP_KEYS);
        assert_eq!(ramp.sample(1.0), Vec3::X * (MAX_GRADIENT_KEYS - 1) as f32);
        assert!((ramp.sample(0.5).x - (MAX_GRADIENT_KEYS - 1) as f32 / 2.0).abs() < 1e-3);
    }

    #[test]
    fn sun_keeps_explicit_zero_and_false_and_bounds_its_numbers() {
        let sun = Sun::from(PbSun {
            intensity: Some(0.0),
            visible: Some(false),
            direction: Some(Vector3 {
                x: 0.0,
                y: 0.0,
                z: 4.0,
            }),
            ..Default::default()
        });
        assert_eq!(sun.intensity, Some(0.0));
        assert_eq!(sun.visible, Some(false));
        assert!(sun.direction.unwrap().abs_diff_eq(Vec3::NEG_Z, 1e-6));
        let sun = Sun::from(PbSun {
            intensity: Some(1e30),
            direction: Some(Vector3::default()),
            ..Default::default()
        });
        assert_eq!(sun.intensity, Some(MAX_SCENE_VALUE));
        assert_eq!(sun.direction, None);
        let sun = Sun::from(PbSun {
            intensity: Some(f32::INFINITY),
            ..Default::default()
        });
        assert_eq!(sun.intensity, None);
    }

    #[test]
    fn shadow_casters_are_nearest_spots_within_twenty_metres() {
        let e = |i: u32| Entity::from_raw(i);
        let slots = shadow_caster_slots(
            [
                (e(1), LightKind::Spot, 25.0),
                (e(2), LightKind::Point, 1.0),
                (e(3), LightKind::Spot, 5.0),
                (e(4), LightKind::Spot, 3.0),
                (e(5), LightKind::Spot, 19.9),
                (e(6), LightKind::Spot, 10.0),
            ],
            3,
            false,
        );
        assert_eq!(slots, vec![e(4), e(3), e(6)]);
        assert!(shadow_caster_slots([(e(1), LightKind::Spot, 5.0)], 0, false).is_empty());
        // the point-shadows switch lets point lights compete for the slots
        let slots = shadow_caster_slots(
            [
                (e(2), LightKind::Point, 1.0),
                (e(3), LightKind::Spot, 5.0),
                (e(7), LightKind::Point, 30.0),
            ],
            3,
            true,
        );
        assert_eq!(slots, vec![e(2), e(3)]);
    }

    #[test]
    fn sun_follows_unitys_path() {
        let angle = |a: Vec3, b: Vec3| a.angle_between(b).to_degrees();
        // unity-explorer's sun at noon, from its recorded day cycle (bevy axes)
        let noon = Vec3::new(0.109_261_24, -0.987_989_84, -0.109_261_2);
        assert!(angle(sun_direction(12.0), noon) < 0.5);
        for horizon in [SUNRISE, SUNSET] {
            assert!(sun_direction(horizon).y.abs() < 1e-5);
        }
        assert!(sun_direction((SUNRISE + SUNSET) / 2.0).y < -0.9999);
        // below the horizon through the night, and continuous round the whole day
        assert!(sun_direction(0.0).y > 0.0);
        assert!(angle(sun_direction(0.0), sun_direction(24.0)) < 0.01);
        for step in 0..2400 {
            let hours = step as f32 / 100.0;
            assert!(angle(sun_direction(hours), sun_direction(hours + 0.01)) < 0.3);
        }
    }

    #[test]
    fn moon_follows_unitys_path() {
        let angle = |a: Vec3, b: Vec3| a.angle_between(b).to_degrees();
        // unity-explorer's moon light, from its computed celestial path (bevy axes, light travel)
        for (hours, unity) in [
            (0.0, Vec3::new(0.677_1, -0.715_5, 0.172_0)),
            (0.756, Vec3::new(0.454_5, -0.766_0, 0.454_5)),
            (2.0, Vec3::new(-0.024_9, -0.631_9, 0.774_7)),
            (12.0, Vec3::new(-0.546_0, 0.759_0, -0.354_6)),
        ] {
            assert!(angle(moon_direction(hours), unity) < 0.5);
        }
        // rises where the sun rises and sets where the sun sets
        assert!(angle(moon_direction(MOONRISE), sun_direction(SUNRISE)) < 0.01);
        assert!(angle(moon_direction(MOONSET), sun_direction(SUNSET)) < 0.01);
        // above the horizon from moonrise to moonset, and continuous round the whole day
        for step in 1..100 {
            let hours = MOONRISE + (24.0 - MOONRISE + MOONSET) * step as f32 / 100.0;
            assert!(moon_direction(hours).y < 0.0);
        }
        for step in 0..2400 {
            let hours = step as f32 / 100.0;
            assert!(angle(moon_direction(hours), moon_direction(hours + 0.01)) < 0.3);
        }
    }

    #[test]
    fn scene_lamps_use_the_intensity_scale() {
        // the default 16000 lamp is 8 light units
        let units = light_source_lumens(16000.0) / (4.0 * PI) / LIGHT_UNIT_LUX;
        assert!((units - 8.0).abs() < 1e-3, "{units}");
    }

    #[test]
    fn lamps_default_to_16000_and_take_the_colour_gain() {
        let lamp = LightSource::from(PbLightSource::default());
        assert_eq!(lamp.intensity, None);
        assert_eq!(lamp.lamp_intensity(), 16000.0);
        // a colour above 1 comes back to 1 and brightens the lamp instead
        let lamp = LightSource::from(PbLightSource {
            color: Some(Color3 {
                r: 2.0,
                g: 1.0,
                b: 0.5,
            }),
            intensity: Some(100.0),
            ..Default::default()
        });
        assert!((lamp.lamp_intensity() - 100.0 * 2f32.powf(2.2)).abs() < 1e-3);
        assert_eq!(lamp.color, Some(Color::srgb(1.0, 0.5, 0.25)));
        // the colour is sRGB: a warm lamp stays saturated rather than washing out
        let linear = lamp.color.unwrap().to_linear();
        assert!(linear.green < 0.22 && linear.blue < 0.06, "{linear:?}");
    }

    #[test]
    fn a_light_written_back_keeps_its_colour() {
        use dcl_component::proto_components::Color3BevyToDcl;
        // a glTF light is mirrored to the scene's component as sRGB; a scene writing that
        // component back must light it with the same linear colour
        let gltf = Color::linear_rgb(1.0, 0.5, 0.25);
        let lamp = LightSource::from(PbLightSource {
            color: Some(gltf.convert_srgb()),
            ..Default::default()
        });
        let (a, b) = (gltf.to_linear(), lamp.color.unwrap().to_linear());
        for (x, y) in [(a.red, b.red), (a.green, b.green), (a.blue, b.blue)] {
            assert!((x - y).abs() < 1e-4, "{a:?} {b:?}");
        }
        assert_eq!(lamp.intensity_gain, 1.0);
    }

    #[test]
    fn root_light_replaces_only_what_it_sets() {
        let preset = SceneGlobalLight {
            dir_color: Color::WHITE,
            dir_illuminance: NOON_ILLUMINANCE,
            sun_illuminance: SKY_SUN_ILLUMINANCE,
            ..Default::default()
        };
        let apply = |light: PbLightSource| {
            let mut global_light = preset.clone();
            apply_root_light(&mut global_light, &light.into());
            global_light
        };
        // no intensity: the preset's light and sky stay, a colour alone tints the sun
        let tinted = apply(PbLightSource {
            color: Some(Color3 {
                r: 1.0,
                g: 0.5,
                b: 0.5,
            }),
            ..Default::default()
        });
        assert_eq!(tinted.dir_illuminance, NOON_ILLUMINANCE);
        assert_eq!(tinted.sun_illuminance, SKY_SUN_ILLUMINANCE);
        assert_eq!(tinted.dir_color, Color::srgb(1.0, 0.5, 0.5));
        // an intensity is lux as given, for the light and the sky's sun
        let set = apply(PbLightSource {
            intensity: Some(5000.0),
            ..Default::default()
        });
        assert_eq!(set.dir_illuminance, 5000.0);
        assert_eq!(set.sun_illuminance, 5000.0);
        assert_eq!(set.dir_color, preset.dir_color);
        // a disabled light turns both off
        let off = apply(PbLightSource {
            active: Some(false),
            intensity: Some(5000.0),
            ..Default::default()
        });
        assert_eq!((off.dir_illuminance, off.sun_illuminance), (0.0, 0.0));
    }

    #[test]
    fn light_unit_lands_at_one_under_the_default_exposure() {
        let exposure = bevy::render::camera::Exposure::default().exposure();
        assert!((LIGHT_UNIT_LUX / PI * exposure - 1.0).abs() < 1e-3);
    }

    #[test]
    fn light_follows_the_intensity_curve() {
        let lux = |hours: f32| {
            sample_keys(&LIGHT_INTENSITY_BY_TIME, day_fraction(hours)) * LIGHT_UNIT_LUX
        };
        assert!((lux(12.0) - NOON_ILLUMINANCE).abs() < 1e-2);
        // (0.91667, 1): the moon at full from 22:00
        assert!((lux(22.0) - LIGHT_UNIT_LUX).abs() < 1.0);
        // (0.85417, 0.01) .. (0.875, 0.01): dark between 20:30 and 21:00
        assert!((lux(20.75) - 0.01 * LIGHT_UNIT_LUX).abs() < 1e-1);
        // 19:47 is seven tenths of the way down from (0.75, 1) to (0.85417, 0.01)
        let late = lux(19.78) / LIGHT_UNIT_LUX;
        assert!(late > 0.28 && late < 0.32, "{late}");
    }

    #[test]
    fn light_swings_to_the_moon_under_a_dip() {
        // moonrise 0.896 with the swap before it, moonset 0.167 with the swap after it
        assert_eq!(celestial_swap(day_fraction(20.0)), (0.0, 0.0));
        assert_eq!(celestial_swap(day_fraction(22.0)), (1.0, 0.0));
        assert_eq!(celestial_swap(day_fraction(3.0)), (1.0, 0.0));
        assert_eq!(celestial_swap(day_fraction(6.0)), (0.0, 0.0));
        let half_swap_hours = MOON_SWAP_DURATION * 12.0;
        let (weight, dip) = celestial_swap((MOONRISE - half_swap_hours) / 24.0);
        assert!((weight - 0.5).abs() < 1e-3 && dip == 1.0, "{weight} {dip}");
        let (weight, dip) = celestial_swap((MOONSET + half_swap_hours) / 24.0);
        assert!((weight - 0.5).abs() < 1e-3 && dip == 1.0, "{weight} {dip}");
        let (weight, dip) = celestial_swap((MOONRISE - 2.0 * half_swap_hours + 0.001) / 24.0);
        assert!(weight < 1e-3 && dip < 0.01, "{weight} {dip}");
    }

    #[test]
    fn light_turns_fast_only_under_the_dip() {
        let angle = |a: Vec3, b: Vec3| a.angle_between(b).to_degrees();
        // what the light throws upward while it is below the horizon, in light units
        let mut upward = 0.0f32;
        for step in 0..2400 {
            let hours = step as f32 / 100.0;
            let (direction, lux) = directional_light(hours);
            let (next, _) = directional_light(hours + 0.01);
            assert!((direction.length() - 1.0).abs() < 1e-4);
            // it follows a body's arc, or swings between the two while dimmed to a hundredth
            if angle(direction, next) > 0.3 {
                assert!(lux < 0.02 * LIGHT_UNIT_LUX, "{hours}: {lux}");
            }
            upward = upward.max(direction.y.max(0.0) * lux / LIGHT_UNIT_LUX);
        }
        // by day it is the sun's arc, after sunset too until the swing; by night the moon's
        // within 0.1 degrees: an f32 acos near 1 resolves only about 0.02 degrees, and the
        // slerp moves the vector by a few ulps
        for hours in [7.0, 12.0, 18.0, 19.5, 5.5] {
            assert!(
                angle(directional_light(hours).0, sun_direction(hours)) < 0.1,
                "{hours}"
            );
        }
        for hours in [22.0, 0.0, 3.0] {
            assert!(
                angle(directional_light(hours).0, moon_direction(hours)) < 0.1,
                "{hours}"
            );
        }
        // below the horizon the light grazes: at most a tenth of a unit goes upward
        assert!(upward > 0.05 && upward < 0.11, "{upward}");
    }

    #[test]
    fn light_colour_follows_the_ramp() {
        let colour =
            |hours: f32| sample_ramp(&DIRECTIONAL_COLOR_RAMP, time_to_phase(day_fraction(hours)));
        assert_eq!(colour(12.0), SUN_COLOR_NOON);
        // night: a blue-violet moon
        let night = colour(0.0);
        assert!(night.z > night.x && night.x < 0.5);
    }

    #[test]
    fn moon_progress_spans_rise_to_set() {
        let (rise, set) = (MOONRISE / 24.0, MOONSET / 24.0);
        assert!(moon_progress(rise).abs() < 1e-5);
        // halfway between moonrise and moonset, across midnight
        let mid = (rise + (set - rise).rem_euclid(1.0) / 2.0).rem_euclid(1.0);
        assert!((moon_progress(mid) - 0.5).abs() < 1e-5);
        assert!((moon_progress(set) - 1.0).abs() < 1e-5);
        // by day the moon is below the horizon, past 1
        let noon = moon_progress(0.5);
        assert!(noon > 1.0 && noon < 2.0, "{noon}");
    }
}
