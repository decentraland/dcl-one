pub mod bake_scene;
pub mod floor_imposter;
pub mod imposter_mesh;
pub mod imposter_spec;
pub mod render;

use std::path::PathBuf;

use bake_scene::DclImposterBakeScenePlugin;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::{AppConfig, DebugInfo, SceneLoadDistance};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use render::{DclImposterRenderPlugin, DebugImpostersEnabled, SceneImposter};

#[derive(Resource, Clone)]
pub struct DclImposterPlugin {
    pub zip_output: Option<PathBuf>,
    pub download: bool,
}

impl Plugin for DclImposterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((DclImposterBakeScenePlugin, DclImposterRenderPlugin))
            .add_preview_console_command::<ImpostDistanceCommand, _>(set_impost_distance)
            .add_preview_console_command::<ImpostMultisampleCommand, _>(set_impost_multi)
            .add_preview_console_command::<DebugImpostersCommand, _>(toggle_debug_imposters);
        app.insert_resource(self.clone());
        app.init_resource::<LodOutlines>()
            .add_console_command::<LodOutlinesCommand, _>(set_lod_outlines)
            .add_systems(
                Update,
                draw_lod_outlines.run_if(|show: Res<LodOutlines>| show.0),
            )
            .add_console_command::<LodState, _>(lod_state)
            .add_debug_section(DebugSection {
                title: "LOD",
                query: "/lod_state",
                fields: &[
                    ("Distant scene rendering", "enabled"),
                    ("Imposter LOD thresholds (m)", "distances"),
                    ("Scene load radius (m)", "load"),
                    ("Imposter load radius (m)", "range"),
                    ("Imposter multisampling", "multisample"),
                    ("Ready tiles", "ready"),
                    ("Pending tiles", "pending"),
                    ("Cached imposter assets", "assets"),
                    ("Imposter VRAM (bytes)", "vram"),
                ],
                actions: &[],
                inputs: &[DebugInput {
                    label: "Color LOD tile outlines",
                    field: "outlines",
                    command: "/lod_outlines {value}",
                    kind: DebugInputKind::Toggle,
                }],
            });
    }
}

#[derive(Resource, Default)]
struct LodOutlines(bool);

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lod_outlines")]
struct LodOutlinesCommand {
    #[arg(action = clap::ArgAction::Set)]
    enabled: bool,
}

fn set_lod_outlines(
    mut input: ConsoleCommand<LodOutlinesCommand>,
    mut outlines: ResMut<LodOutlines>,
) {
    if let Some(Ok(cmd)) = input.take() {
        outlines.0 = cmd.enabled;
        input.reply_ok(if cmd.enabled {
            "Showing LOD tile outlines"
        } else {
            "Hid LOD tile outlines"
        });
    }
}

fn draw_lod_outlines(mut gizmos: Gizmos, tiles: Query<&SceneImposter>) {
    for tile in &tiles {
        if tile.as_ingredient {
            continue;
        }
        let size = (1u32 << tile.level) as f32 * scene_runner::initialize_scene::PARCEL_SIZE;
        let corner = Vec3::new(
            tile.parcel.x as f32 * 16.0,
            0.1,
            -tile.parcel.y as f32 * 16.0,
        );
        let x = Vec3::X * size;
        let z = Vec3::NEG_Z * size;
        let color = Color::hsl((tile.level as f32 * 65.0) % 360.0, 1.0, 0.6);
        gizmos.linestrip(
            [corner, corner + x, corner + x + z, corner + z, corner],
            color,
        );
    }
}

/// Distant-scene rendering settings and imposter tiles in this client.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lod_state")]
struct LodState;

fn lod_state(
    mut input: ConsoleCommand<LodState>,
    config: Res<AppConfig>,
    distance: Res<SceneLoadDistance>,
    outlines: Res<LodOutlines>,
    tiles: Query<Has<render::RetryImposter>, With<SceneImposter>>,
    assets: Res<Assets<boimp::Imposter>>,
) {
    if input.take().is_some() {
        let (mut ready, mut pending) = (0, 0);
        for retry in &tiles {
            if retry {
                pending += 1
            } else {
                ready += 1
            }
        }
        input.reply_ok(serde_json::json!({
            "outlines": outlines.0, "enabled": !config.scene_imposter_distances.is_empty(), "distances": config.scene_imposter_distances,
            "load": distance.load, "range": distance.load_imposter, "multisample": config.scene_imposter_multisample,
            "ready":ready, "pending":pending, "assets":assets.len(),
            "vram":assets.iter().map(|(_,a)| a.vram_bytes).sum::<usize>(),
        }).to_string());
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_imposters")]
struct DebugImpostersCommand {
    on: Option<bool>,
}

fn toggle_debug_imposters(
    mut input: ConsoleCommand<DebugImpostersCommand>,
    mut enabled: ResMut<DebugImpostersEnabled>,
    mut debug: ResMut<DebugInfo>,
) {
    if let Some(Ok(command)) = input.take() {
        enabled.0 = command.on.unwrap_or(!enabled.0);
        if !enabled.0 {
            debug.info.remove("Imposter memory");
            debug.info.remove("Imposter states");
        }
        input.reply_ok(format!("imposter debug info: {}", enabled.0));
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/impost")]
struct ImpostDistanceCommand {
    distances: String,
}

fn set_impost_distance(
    mut input: ConsoleCommand<ImpostDistanceCommand>,
    mut config: ResMut<AppConfig>,
    mut scene_distance: ResMut<SceneLoadDistance>,
) {
    if let Some(Ok(command)) = input.take() {
        let distances = command
            .distances
            .split(|c: char| !c.is_numeric() && c != '.')
            .map(str::parse::<f32>)
            .flat_map(Result::ok)
            .collect::<Vec<_>>();
        input.reply_ok(format!("imposter distances set to {distances:?}"));
        scene_distance.load_imposter = distances
            .last()
            .map(|last| {
                // actual distance we need is last + diagonal of the largest mip size
                let mip_size = (1 << (distances.len() - 1)) as f32 * 16.0;
                last + (2.0 * mip_size * mip_size).sqrt()
            })
            .unwrap_or(0.0);
        config.scene_imposter_distances = distances;
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/impost_multisample")]
struct ImpostMultisampleCommand {
    on: Option<u32>,
}

fn set_impost_multi(
    mut input: ConsoleCommand<ImpostMultisampleCommand>,
    mut config: ResMut<AppConfig>,
    mut commands: Commands,
    q: Query<Entity, With<SceneImposter>>,
) {
    if let Some(Ok(command)) = input.take() {
        let multisample = command
            .on
            .unwrap_or_else(|| {
                if config.scene_imposter_multisample {
                    0
                } else {
                    99
                }
            })
            .clamp(0, 99) as f32
            / 100.0;
        config.scene_imposter_multisample = multisample != 0.0;
        config.scene_imposter_multisample_amount = multisample;
        input.reply_ok("imposter multisample set to {multisample}");
        for e in q.iter() {
            commands.entity(e).despawn();
        }
    }
}
