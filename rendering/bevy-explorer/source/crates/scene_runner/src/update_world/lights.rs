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
        LIGHT_INTENSITY_BY_TIME,
    },
    structs::{
        AppConfig, PrimaryUser, SceneGlobalLight, TimeOfDay, PRIMARY_AVATAR_LIGHT_LAYER,
        SPOT_SHADOW_MAX_DISTANCE,
    },
    util::TryPushChildrenEx,
};
use dcl_component::{
    proto_components::{
        common::{TextureUnion, Vector3},
        sdk::components::{pb_light_source, PbGlobalLight, PbLightSource},
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
    /// Lets point lights take shadow caster slots too. Off by default: Unity gives scene shadows
    /// to spot lights only, so this is a comparison aid for testers, not a setting.
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
    lights: Query<(&Visibility, Option<&PointLight>, Option<&SpotLight>), With<LightEntity>>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let mut visible = 0;
    let mut casters = 0;
    for (vis, point, spot) in &lights {
        if *vis != Visibility::Hidden {
            visible += 1;
            if point.is_some_and(|l| l.shadows_enabled) || spot.is_some_and(|l| l.shadows_enabled) {
                casters += 1;
            }
        }
    }
    cmd.reply_ok(serde_json::json!({"lights": switches.lights, "shadows": switches.shadows, "pointShadows":switches.point_shadows,
        "count":lights.iter().len(), "visible":visible, "casters":casters}).to_string());
}

#[derive(Component, Debug, Default)]
pub struct LightSource {
    pub enabled: bool,
    pub intensity: Option<f32>,
    pub shadow: Option<bool>,
    pub color: Option<Color>,
    pub range: Option<f32>,
    pub spotlight_angles: Option<(f32, f32)>,
    pub light_texture: Option<TextureUnion>,
}

impl From<PbLightSource> for LightSource {
    fn from(value: PbLightSource) -> Self {
        // rescale colors > 1 to 1, and multiply intensity by pow(max^2.2)
        // to match unity
        let color_scale = value
            .color
            .map(|c| c.r.max(c.g).max(c.b).max(1.0))
            .unwrap_or(1.0);

        Self {
            enabled: value.active.unwrap_or(true),
            intensity: Some(value.intensity.unwrap_or(16000.0) * color_scale.powf(2.2)),
            shadow: value.shadow,
            color: value.color.map(|c| {
                Color::linear_rgb(c.r / color_scale, c.g / color_scale, c.b / color_scale)
            }),
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

/// unity: one directional light unit lights a facing white surface to 1 (URP's Lambert has no
/// 1 / pi). bevy divides illuminance by pi and multiplies by the view exposure, so the lux that
/// land at 1 are `pi / exposure`; the explorer leaves every camera on bevy's default exposure
/// (`Exposure::BLENDER`, ev100 9.7: `1 / exposure = 1.2 * 2^9.7`).
pub const UNITY_LIGHT_LUX: f32 = PI * 1.2 * 831.7465;
/// The sun at noon, lux: the light intensity curve (`common::sky_preset`) peaks at 1.2 there.
pub const NOON_ILLUMINANCE: f32 = 1.2 * UNITY_LIGHT_LUX;
/// The sky's own sun at noon, in the atmosphere's units (`dir_light_intensity`).
const SKY_SUN_ILLUMINANCE: f32 = 7000.0;

/// unity: `LightSourceSettings.PointLightIntensityScale` and `SpotLightIntensityScale`: a scene
/// light's intensity times this is its `Light.intensity`, in Unity's unit.
const LIGHT_SOURCE_INTENSITY_SCALE: f32 = 0.0005;

/// A scene point or spot light's bevy intensity (lumens) from its `PbLightSource` intensity.
pub fn light_source_lumens(intensity: f32) -> f32 {
    intensity * LIGHT_SOURCE_INTENSITY_SCALE * UNITY_LIGHT_LUX * 4.0 * PI
}

/// Unity's sunrise and sunset (hours): the sun is overhead halfway between, at 12:37.
const SUNRISE: f32 = 6.422;
const SUNSET: f32 = 18.828;

/// Unity's moonrise and moonset (hours): the moon is up for six and a half hours around 00:45.
const MOONRISE: f32 = 21.504;
const MOONSET: f32 = 4.008;

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
fn sun_direction(hours: f32) -> Vec3 {
    celestial_direction(hours, SUNRISE, SUNSET, 0.0)
}

/// Direction the moonlight travels at `hours` past midnight: the sun's arc leaned to one side,
/// crossed overnight.
pub fn moon_direction(hours: f32) -> Vec3 {
    celestial_direction(hours, MOONRISE, MOONSET, MOON_TILT_DEGREES)
}

/// unity: `DirectionalColorRamp` at phase 0.5, the ramp's noon key (sRGB).
pub const SUN_COLOR_NOON: Vec3 = Vec3::new(1.0, 0.98, 0.94);

/// unity: `celestialSwapDuration`, the fraction of a day over which the light swings from the
/// sun's arc to the moon's before moonrise, and back after moonset.
const MOON_SWAP_DURATION: f32 = 0.042;
/// unity: `CELESTIAL_SWAP_INTENSITY_DIP`, how far the light dims mid-swing to hide the move.
const SWAP_INTENSITY_DIP: f32 = 0.99;

/// unity: `SkyboxCelestialMath.EvaluateSwap`. At `day_fraction`: how far the light has swung
/// onto the moon's arc (0 sun, 1 moon) and the dip hiding the swing (1 at its middle).
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

pub fn update_directional_light(
    lights: Query<(
        &RendererSceneContext,
        Option<&LightSource>,
        Option<&GlobalLight>,
    )>,
    mut global_light: ResMut<SceneGlobalLight>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    time: Res<TimeOfDay>,
) {
    // unity: SkyboxRenderController.UpdateDirectionalLight on its computed celestial path: the
    // colour from the ramp at the phase, the intensity from the curve at the time, the light on
    // the sun's arc by day and the moon's by night, swung between them under a dip
    let hours = time.elapsed_secs() / 3600.0;
    let day_fraction = day_fraction(hours);
    let sun_direction = sun_direction(hours);
    let moon_direction = moon_direction(hours);
    let (moon_weight, dip) = celestial_swap(day_fraction);
    let dir_direction = Quat::IDENTITY.slerp(
        Quat::from_rotation_arc(sun_direction, moon_direction),
        moon_weight,
    ) * sun_direction;
    let dir = sample_ramp(&DIRECTIONAL_COLOR_RAMP, time_to_phase(day_fraction));
    let dir_illuminance = sample_keys(&LIGHT_INTENSITY_BY_TIME, day_fraction)
        * UNITY_LIGHT_LUX
        * (1.0 - dip * SWAP_INTENSITY_DIP);

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
        layers: RenderLayers::default(),
    };

    let Ok(player) = player.single() else {
        return;
    };

    let mut apply =
        |parcel: Entity, maybe_light: Option<&LightSource>, maybe_global: Option<&GlobalLight>| {
            global_light.source = Some(parcel);
            if let Some(light) = maybe_light {
                if let Some(color) = light.color {
                    global_light.dir_color = color;
                }
                if let Some(ill) = if light.enabled {
                    light.intensity
                } else {
                    Some(0.0)
                } {
                    // unity: a directional light source keeps its intensity as Unity's unit
                    // (scale 1); the sky's sun follows at its noon ratio
                    global_light.dir_illuminance = ill * UNITY_LIGHT_LUX;
                    global_light.sun_illuminance = ill / 1.2 * SKY_SUN_ILLUMINANCE;
                }
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
        };

    if let Some(active_parcel) = containing_scene.get_parcel_oow(player) {
        // try and get settings from active parcel scene first
        if let Ok((_, maybe_light, maybe_global)) = lights.get(active_parcel) {
            if maybe_light.is_some() || maybe_global.is_some() {
                apply(active_parcel, maybe_light, maybe_global);
                return;
            }
        }
    }

    // if the primary parcel doesn't specify anything, check any portables
    let mut portable_settings: Option<(&str, Entity, Option<&LightSource>, Option<&GlobalLight>)> =
        None;
    for entity in containing_scene.get_portables(false) {
        if let Ok((ctx, maybe_light, maybe_global)) = lights.get(entity) {
            if maybe_light.is_none() && maybe_global.is_none() {
                continue;
            }

            let apply = match portable_settings {
                None => true,
                Some((existing, ..)) => &*ctx.hash < existing,
            };

            if !apply {
                continue;
            }

            portable_settings = Some((&ctx.hash, entity, maybe_light, maybe_global));
        }
    }

    if let Some((_, portable, maybe_light, maybe_global)) = portable_settings {
        apply(portable, maybe_light, maybe_global);
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
            light.intensity.unwrap_or(16000.0).powf(0.25)
        } else {
            range.min(light.intensity.unwrap_or(16000.0).powf(0.25))
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

        let lumens = light_source_lumens(light.intensity.unwrap_or(16000.0));
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

/// Entities to give shadows, Unity's LightSourceCullingSystem: spot lights only (point lights
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
    fn scene_lamps_use_unitys_intensity_scale() {
        // the default 16000 lamp is Light.intensity 8 in unity
        let unity_units = light_source_lumens(16000.0) / (4.0 * PI) / UNITY_LIGHT_LUX;
        assert!((unity_units - 8.0).abs() < 1e-3, "{unity_units}");
    }

    #[test]
    fn unity_light_unit_lands_at_one_under_the_default_exposure() {
        let exposure = bevy::render::camera::Exposure::default().exposure();
        assert!((UNITY_LIGHT_LUX / PI * exposure - 1.0).abs() < 1e-3);
    }

    #[test]
    fn light_follows_unitys_intensity_curve() {
        let lux = |hours: f32| {
            sample_keys(&LIGHT_INTENSITY_BY_TIME, day_fraction(hours)) * UNITY_LIGHT_LUX
        };
        assert!((lux(12.0) - NOON_ILLUMINANCE).abs() < 1e-2);
        // (0.91667, 1): the moon at full from 22:00
        assert!((lux(22.0) - UNITY_LIGHT_LUX).abs() < 1.0);
        // (0.85417, 0.01) .. (0.875, 0.01): dark between 20:30 and 21:00
        assert!((lux(20.75) - 0.01 * UNITY_LIGHT_LUX).abs() < 1e-1);
        // 19:47 is seven tenths of the way down from (0.75, 1) to (0.85417, 0.01)
        let late = lux(19.78) / UNITY_LIGHT_LUX;
        assert!(late > 0.28 && late < 0.32, "{late}");
    }

    #[test]
    fn light_swings_to_the_moon_under_a_dip() {
        // unity: moonrise 0.896 with the swap before it, moonset 0.167 with the swap after it
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
    fn light_colour_follows_unitys_ramp() {
        let colour =
            |hours: f32| sample_ramp(&DIRECTIONAL_COLOR_RAMP, time_to_phase(day_fraction(hours)));
        assert_eq!(colour(12.0), SUN_COLOR_NOON);
        // night: a blue-violet moon
        let night = colour(0.0);
        assert!(night.z > night.x && night.x < 0.5);
    }
}
