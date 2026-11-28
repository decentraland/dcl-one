use bevy::{
    core_pipeline::prepass::{DeferredPrepass, DepthPrepass, NormalPrepass},
    pbr::{CascadeShadowConfig, DirectionalLightShadowMap, RenderMeshInstances},
    prelude::*,
    render::{
        batching::gpu_preprocessing::GpuPreprocessingSupport,
        experimental::occlusion_culling::OcclusionCulling, view::NoIndirectDrawing, RenderApp,
    },
    window::PrimaryWindow,
};
use bevy_console::ConsoleCommand;
use common::structs::{
    AppConfig, FogSetting, LandscapeDistance, LensFlare, PrimaryCamera, SceneLoadDistance,
    SessionGraphics, StageLight, TimeOfDay,
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::json;

use crate::{
    lens_flare::{LensFlareSettings, DAY_FLARE},
    shell_texturing::{Ground, ParcelGrass},
    sky_envmap::{SkyAmbient, SkyEnvmapSettings},
    trees::Tree,
    upscale::{Compositor, SceneTarget},
    CloudCover, FogDensity, ShadowBiasOverride, SUN_SHADOW_BIAS,
};

/// The Lighting section's action that puts the fog density back to the sky preset's.
const RESTORE_FOG_DENSITY: &str = "/fog true 0.0005";

const fn toggle(
    label: &'static str,
    field: &'static str,
    command: &'static str,
) -> DebugInput<'static> {
    DebugInput {
        label,
        field,
        command,
        kind: DebugInputKind::Toggle,
    }
}

const fn slider(
    label: &'static str,
    field: &'static str,
    command: &'static str,
    min: f64,
    max: f64,
    step: f64,
) -> DebugInput<'static> {
    DebugInput {
        label,
        field,
        command,
        kind: DebugInputKind::Number { min, max, step },
    }
}

pub(super) struct VisualsDebugPanelPlugin;

impl Plugin for VisualsDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LandscapeLayers {
            ground: true,
            trees: true,
            grass: true,
        })
        .add_console_command::<LandscapeCommand, _>(landscape_command)
        .add_console_command::<SkyState, _>(sky_state)
        .add_console_command::<RenderView, _>(render_view)
        .add_console_command::<RenderScaleState, _>(render_scale_state)
        .add_debug_section(DebugSection {
            title: "Render scale",
            query: "/render_scale_state",
            fields: &[
                ("Scene resolution (width / height)", "scene"),
                ("Window resolution (width / height)", "window"),
                ("FSR compositor", "compositor"),
                ("Depth prepass", "depth_prepass"),
                ("Normal prepass", "normal_prepass"),
            ],
            actions: &[(
                "Back to the player's setting",
                "/session_graphics render_scale",
            )],
            inputs: &[slider(
                "Render scale (this session)",
                "scale",
                "/session_graphics render_scale {value}",
                SessionGraphics::MIN_RENDER_SCALE as f64,
                1.0,
                0.05,
            )],
        })
        .add_console_command::<LightingState, _>(lighting_state)
        .add_debug_section(DebugSection {
            title: "Lighting",
            query: "/lighting_state",
            fields: &[
                ("Fog falloff rendered (per m)", "fogFalloff"),
                ("Fog colour, sRGB (R / G / B)", "fogColor"),
                ("Ambient sky, linear (R / G / B)", "ambientSky"),
                ("Ambient equator, linear (R / G / B)", "ambientEquator"),
                ("Ambient ground, linear (R / G / B)", "ambientGround"),
                ("Directional light (lux)", "sunLux"),
                ("Directional light colour, sRGB (R / G / B)", "sunColor"),
                ("Reflection floor scale", "reflectionFloor"),
                ("Reflection compression", "reflectionCompression"),
            ],
            actions: &[
                ("Restore fog density", RESTORE_FOG_DENSITY),
                (
                    "Restore ambient scales",
                    "/ambient --sky 1 --equator 1 --ground 1",
                ),
            ],
            inputs: &[
                toggle("Fog", "fog", "/fog {value}"),
                slider(
                    "Fog density (per m)",
                    "fogDensity",
                    "/fog true {value}",
                    0.0,
                    0.01,
                    0.0001,
                ),
                slider(
                    "Ambient sky scale",
                    "skyScale",
                    "/ambient --sky {value}",
                    0.0,
                    4.0,
                    0.05,
                ),
                slider(
                    "Ambient equator scale",
                    "equatorScale",
                    "/ambient --equator {value}",
                    0.0,
                    4.0,
                    0.05,
                ),
                slider(
                    "Ambient ground scale",
                    "groundScale",
                    "/ambient --ground {value}",
                    0.0,
                    4.0,
                    0.05,
                ),
            ],
        })
        .add_console_command::<SunShadows, _>(sun_shadows)
        .add_console_command::<SunShadowMap, _>(sun_shadow_map)
        .add_preview_debug_inputs(DebugSection {
            title: "Landscape",
            query: "/landscape",
            fields: &[
                ("Scene load distance (m)", "loadDistance"),
                ("Landscape distance (m)", "landscapeDistance"),
                ("Loaded trees", "treeCount"),
                ("Grass parcels", "grassParcels"),
            ],
            actions: &[],
            inputs: &[
                toggle("Ground", "ground", "/landscape ground {value}"),
                toggle("Trees", "trees", "/landscape trees {value}"),
                toggle("Parcel grass", "grass", "/landscape grass {value}"),
            ],
        })
        .add_debug_section(DebugSection {
            title: "Sun shadows",
            query: "/sun_shadows",
            fields: &[
                ("Shadow quality", "quality"),
                ("Shadow distance (m)", "distance"),
                ("Cascade far bounds (m)", "cascades"),
                ("Bias overridden", "overridden"),
                ("Default bias (depth m / normal texels)", "presetBias"),
            ],
            actions: &[("Restore default bias", "/shadowbias --reset")],
            inputs: &[
                DebugInput {
                    label: "Shadow map size (texels)",
                    field: "mapSize",
                    command: "/shadow_map {value}",
                    kind: DebugInputKind::Select(&[
                        ("512", "512"),
                        ("1024", "1024"),
                        ("2048 (default)", "2048"),
                        ("4096", "4096"),
                    ]),
                },
                slider(
                    "Depth bias (m)",
                    "depthBias",
                    "/shadowbias --depth {value}",
                    0.0,
                    1.0,
                    0.005,
                ),
                slider(
                    "Normal bias (texels)",
                    "normalBias",
                    "/shadowbias --normal {value}",
                    0.0,
                    10.0,
                    0.05,
                ),
            ],
        })
        .add_debug_section(DebugSection {
            title: "Skybox",
            query: "/sky_state",
            fields: &[
                ("Time set by", "timeSource"),
                ("Held by the scene (menu locked)", "sceneLocked"),
                ("Allow overriding scene controls", "overrideAllowed"),
            ],
            actions: &[("Resume sky clock", "/time --resume")],
            inputs: &[
                slider(
                    "Fixed hour",
                    "hour",
                    "/time {value} 0 --force",
                    0.0,
                    23.99,
                    0.01,
                ),
                slider("Cloud cover", "cover", "/cloud {value}", 0.0, 1.0, 0.01),
                slider(
                    "Cloud speed",
                    "speed",
                    "/cloud --speed {value}",
                    0.0,
                    4.0,
                    0.01,
                ),
            ],
        })
        .add_console_command::<LensFlareCommand, _>(lens_flare_command)
        .add_debug_section(DebugSection {
            title: "Lens flare",
            query: "/lens_flare",
            fields: &[
                ("Flare (day / night)", "flare"),
                ("Intensity by time of day", "intensity"),
                ("Light elevation (degrees)", "elevation"),
                ("Light from view direction (degrees)", "fromView"),
            ],
            actions: &[],
            inputs: &[toggle(
                "Lens flare (this session)",
                "enabled",
                "/lens_flare {value}",
            )],
        });
        if app
            .world()
            .get_resource::<common::structs::PreviewMode>()
            .is_some_and(|p| p.is_preview)
        {
            app.add_systems(Update, apply_landscape_visibility);
        }
    }

    fn finish(&self, app: &mut App) {
        let render = app.sub_app(RenderApp).world();
        let gpu_instances = matches!(
            render.get_resource::<RenderMeshInstances>(),
            Some(RenderMeshInstances::GpuBuilding(_))
        );
        let culling = gpu_instances
            && render
                .get_resource::<GpuPreprocessingSupport>()
                .is_some_and(|s| s.is_culling_supported());
        app.insert_resource(RenderCapabilities {
            gpu_instances,
            culling,
        });
        let inputs = [
            toggle(
                "Indirect drawing",
                "indirect",
                "/render_view indirect {value}",
            ),
            toggle(
                "GPU occlusion culling",
                "occlusion",
                "/render_view occlusion {value}",
            ),
        ];
        app.add_preview_debug_inputs(DebugSection {
            title: "GPU rendering",
            query: "/render_view",
            fields: &[
                ("GPU-built mesh instances", "gpuInstances"),
                ("GPU culling supported", "supported"),
            ],
            actions: &[],
            inputs: if culling { &inputs } else { &[] },
        });
    }
}

/// Read the lens flare, or turn it on or off on the player's camera for this session (the Lens
/// Flare setting applies again to a new camera).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lens_flare")]
struct LensFlareCommand {
    enabled: Option<bool>,
}

fn lens_flare_command(
    mut cmd: ConsoleCommand<LensFlareCommand>,
    mut commands: Commands,
    settings: Res<LensFlareSettings>,
    camera: Query<(Entity, &GlobalTransform, Has<LensFlare>), With<PrimaryCamera>>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    let camera = camera.single().ok();
    let mut enabled = camera.is_some_and(|(_, _, flare)| flare);
    if let (Some(set), Some((entity, _, _))) = (command.enabled, camera) {
        if set {
            commands.entity(entity).try_insert(LensFlare);
        } else {
            commands.entity(entity).remove::<LensFlare>();
        }
        enabled = set;
    }
    let toward = settings.toward_light;
    let elevation = toward.y.clamp(-1.0, 1.0).asin().to_degrees();
    let from_view = camera.map(|(_, transform, _)| {
        transform
            .forward()
            .dot(toward)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    });
    let flare = if std::ptr::eq(settings.elements, DAY_FLARE.as_slice()) {
        "day"
    } else {
        "night"
    };
    cmd.reply_ok(
        json!({"enabled": enabled, "flare": flare, "intensity": settings.intensity,
            "elevation": elevation, "fromView": from_view})
        .to_string(),
    );
}

#[derive(Resource)]
struct LandscapeLayers {
    ground: bool,
    trees: bool,
    grass: bool,
}

/// Change visibility for a landscape render layer, including subsequently streamed entities.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/landscape")]
struct LandscapeCommand {
    #[arg(value_parser = ["ground", "trees", "grass"], requires = "visible")]
    layer: Option<String>,
    #[arg(requires = "layer")]
    visible: Option<bool>,
}

fn landscape_command(
    mut cmd: ConsoleCommand<LandscapeCommand>,
    preview: Option<Res<common::structs::PreviewMode>>,
    mut layers: ResMut<LandscapeLayers>,
    load: Res<SceneLoadDistance>,
    landscape: Res<LandscapeDistance>,
    trees: Query<(), With<Tree>>,
    grass: Query<(), With<ParcelGrass>>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if let (Some(layer), Some(value)) = (args.layer.as_deref(), args.visible) {
        if !preview.is_some_and(|p| p.is_preview) {
            cmd.reply_failed("landscape overrides require preview mode");
            return;
        }
        match layer {
            "ground" => layers.ground = value,
            "trees" => layers.trees = value,
            _ => layers.grass = value,
        }
    }
    cmd.reply_ok(json!({"ground":layers.ground, "trees":layers.trees, "grass":layers.grass,
        "treeCount":trees.iter().len(), "grassParcels":grass.iter().len(), "loadDistance":load.load, "landscapeDistance":landscape.0}).to_string());
}

type LandscapeObjects = Or<(With<Ground>, With<Tree>, With<ParcelGrass>)>;
type AddedLandscapeObjects = Or<(Added<Ground>, Added<Tree>, Added<ParcelGrass>)>;

fn apply_landscape_visibility(
    layers: Res<LandscapeLayers>,
    mut objects: Query<(Has<Ground>, Has<Tree>, &mut Visibility), LandscapeObjects>,
    added: Query<Entity, AddedLandscapeObjects>,
) {
    let set = |(ground, tree, mut visibility): (bool, bool, Mut<Visibility>)| {
        let show = if ground {
            layers.ground
        } else if tree {
            layers.trees
        } else {
            layers.grass
        };
        visibility.set_if_neq(if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    };
    if layers.is_changed() {
        objects.iter_mut().for_each(set);
    } else if !layers.ground || !layers.trees || !layers.grass {
        for entity in &added {
            if let Ok(object) = objects.get_mut(entity) {
                set(object);
            }
        }
    }
}

/// Read the effective rendered sky time and atmosphere settings.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/sky_state")]
struct SkyState;

fn sky_state(
    mut cmd: ConsoleCommand<SkyState>,
    time: Res<TimeOfDay>,
    clouds: Res<CloudCover>,
    hold: Single<&crate::day_night::SkyHold>,
    config: Res<AppConfig>,
) {
    if let Some(Ok(_)) = cmd.take() {
        let mut state = sky_state_json(time.time, &clouds);
        state["timeSource"] = hold.source.name().into();
        state["sceneLocked"] = time.locked.into();
        state["overrideAllowed"] = config.allow_scene_control_override.into();
        cmd.reply_ok(state.to_string());
    }
}

fn sky_state_json(clock_secs: f32, clouds: &CloudCover) -> serde_json::Value {
    json!({"hour": clock_secs / 3600., "cover": clouds.cover, "speed": clouds.speed})
}

/// Read the render scale in effect, the resolution the world is drawn at, and whether the FSR
/// compositor and the prepasses run.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/render_scale_state")]
struct RenderScaleState;

fn render_scale_state(
    mut cmd: ConsoleCommand<RenderScaleState>,
    scene_target: Res<SceneTarget>,
    images: Res<Assets<Image>>,
    window: Query<&Window, With<PrimaryWindow>>,
    compositor: Query<&Camera, With<Compositor>>,
    camera: Query<(Has<DepthPrepass>, Has<NormalPrepass>), With<PrimaryCamera>>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let window = window.single().ok().map(|w| w.physical_size().to_array());
    let compositor = compositor.single().is_ok_and(|c| c.is_active);
    // at scale 1 the world draws to the window itself
    let scene = if compositor {
        images.get(&scene_target.image).map(|i| i.size().to_array())
    } else {
        window
    };
    let (depth, normal) = camera.single().ok().unzip();
    cmd.reply_ok(
        json!({"scale": scene_target.scale, "scene": scene, "window": window,
            "compositor": compositor, "depth_prepass": depth, "normal_prepass": normal})
        .to_string(),
    );
}

/// Read the fog, the Trilight ambient and the directional light in effect. The fog density is
/// the one `/fog` holds; the falloff is what the primary camera renders, a scene's fog component
/// and the Off setting included. The ambient colours are before the band scales.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lighting_state")]
struct LightingState;

fn lighting_state(
    mut cmd: ConsoleCommand<LightingState>,
    config: Res<AppConfig>,
    fog_density: Res<FogDensity>,
    fog: Query<&DistanceFog, With<PrimaryCamera>>,
    ambient: Res<SkyAmbient>,
    envmap: Res<SkyEnvmapSettings>,
    sun: Query<&DirectionalLight, Without<StageLight>>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let srgb = |color: Color| color.to_srgba().to_vec3().to_array();
    let fog = fog.single().ok();
    let falloff = fog.and_then(|fog| match fog.falloff {
        FogFalloff::Exponential { density } => Some(density),
        _ => None,
    });
    let [sky, equator, ground] = ambient.bands.map(|band| band.to_array());
    let sun = sun.iter().next();
    cmd.reply_ok(
        json!({"fog": config.graphics.fog == FogSetting::On, "fogDensity": fog_density.0,
            "fogFalloff": falloff, "fogColor": fog.map(|fog| srgb(fog.color)),
            "ambientSky": sky, "ambientEquator": equator, "ambientGround": ground,
            "skyScale": envmap.trilight.x, "equatorScale": envmap.trilight.y,
            "groundScale": envmap.trilight.z,
            "sunLux": sun.map(|light| light.illuminance),
            "sunColor": sun.map(|light| srgb(light.color)),
            "reflectionFloor": envmap.floor, "reflectionCompression": envmap.compression})
        .to_string(),
    );
}

/// Read the sun's effective shadow quality, cascades and biases.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/sun_shadows")]
struct SunShadows;

fn sun_shadows(
    mut cmd: ConsoleCommand<SunShadows>,
    config: Res<AppConfig>,
    session: Res<SessionGraphics>,
    bias: Res<ShadowBiasOverride>,
    map: Res<DirectionalLightShadowMap>,
    sun: Query<(&DirectionalLight, &CascadeShadowConfig), Without<StageLight>>,
) {
    if let Some(Ok(_)) = cmd.take() {
        let preset = SUN_SHADOW_BIAS;
        let (depth, normal) = bias.0.unwrap_or(preset);
        // every sun layer shares one config; read the first that casts shadows
        let cascades = sun
            .iter()
            .find(|(light, _)| light.shadows_enabled)
            .map(|(_, cascades)| cascades.bounds.clone());
        cmd.reply_ok(
            json!({"quality":format!("{:?}", session.shadow_setting(&config)),
            "distance":config.graphics.shadow_distance, "mapSize":map.size.to_string(),
            "cascades":cascades, "overridden":bias.0.is_some(), "presetBias":[preset.0, preset.1],
            "depthBias":depth, "normalBias":normal})
            .to_string(),
        );
    }
}

/// Set the sun's shadow map size in texels per cascade until restart, to compare biases at
/// another resolution. The normal bias is in texels, so the same value offsets further on a
/// smaller map.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/shadow_map")]
struct SunShadowMap {
    size: usize,
}

fn sun_shadow_map(
    mut cmd: ConsoleCommand<SunShadowMap>,
    mut map: ResMut<DirectionalLightShadowMap>,
) {
    if let Some(Ok(SunShadowMap { size })) = cmd.take() {
        if !(256..=8192).contains(&size) || !size.is_power_of_two() {
            cmd.reply_failed("shadow map size must be a power of two from 256 to 8192");
            return;
        }
        map.size = size;
        cmd.reply_ok(format!("sun shadow map {size} texels"));
    }
}

#[derive(Resource)]
struct RenderCapabilities {
    gpu_instances: bool,
    culling: bool,
}

/// Inspect main camera GPU drawing or set its indirect draw and occlusion mode.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/render_view")]
struct RenderView {
    #[arg(value_parser = ["indirect", "occlusion"], requires = "enabled")]
    feature: Option<String>,
    #[arg(requires = "feature")]
    enabled: Option<bool>,
}

fn render_view(
    mut cmd: ConsoleCommand<RenderView>,
    preview: Option<Res<common::structs::PreviewMode>>,
    mut commands: Commands,
    caps: Res<RenderCapabilities>,
    camera: Query<
        (
            Entity,
            Has<NoIndirectDrawing>,
            Has<OcclusionCulling>,
            Has<DepthPrepass>,
            Has<DeferredPrepass>,
        ),
        With<PrimaryCamera>,
    >,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    let Ok((entity, direct, occlusion, depth, deferred)) = camera.single() else {
        cmd.reply_failed("player camera unavailable");
        return;
    };
    let mut indirect = caps.culling && !direct;
    let mut occlusion = indirect && depth && !deferred && occlusion;
    if let (Some(feature), Some(enabled)) = (args.feature.as_deref(), args.enabled) {
        if !preview.is_some_and(|p| p.is_preview) {
            cmd.reply_failed("renderer overrides require preview mode");
            return;
        }
        if !caps.culling {
            cmd.reply_failed("GPU indirect culling is unavailable");
            return;
        }
        let mut view = commands.entity(entity);
        match feature {
            "indirect" => {
                indirect = enabled;
                if enabled {
                    view.remove::<NoIndirectDrawing>();
                } else {
                    view.insert(NoIndirectDrawing).remove::<OcclusionCulling>();
                    occlusion = false;
                }
            }
            _ => {
                if enabled && !(indirect && depth && !deferred) {
                    cmd.reply_failed(
                        "occlusion needs indirect drawing and a forward depth prepass",
                    );
                    return;
                }
                occlusion = enabled;
                if enabled {
                    view.insert(OcclusionCulling);
                } else {
                    view.remove::<OcclusionCulling>();
                }
            }
        }
    }
    cmd.reply_ok(json!({"gpuInstances":caps.gpu_instances, "supported":caps.culling, "indirect":indirect, "occlusion":occlusion}).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_state_reports_cover_and_speed() {
        let clouds = CloudCover {
            cover: 0.25,
            speed: 2.0,
        };
        let state = sky_state_json(7.5 * 3600.0, &clouds);
        assert_eq!(state["hour"], 7.5);
        assert_eq!(state["cover"], 0.25);
        assert_eq!(state["speed"], 2.0);
        assert_eq!(state.as_object().unwrap().len(), 3);
    }

    #[test]
    fn restore_action_carries_the_preset_fog_density() {
        let density = crate::sky_preset::FOG_DENSITY_PER_METRE.to_string();
        assert_eq!(RESTORE_FOG_DENSITY, format!("/fog true {density}"));
    }
}
