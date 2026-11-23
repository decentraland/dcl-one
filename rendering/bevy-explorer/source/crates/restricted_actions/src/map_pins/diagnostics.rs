//! The Map pins section of the runtime debug panel.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::ReadsMapPins;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use scene_runner::{renderer_context::RendererSceneContext, SceneEntity};
use serde_json::json;

use super::{MapPin, MapPinStreams};

pub(super) struct MapPinsDebugPanelPlugin;

impl Plugin for MapPinsDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<MapPinsState, _>(map_pins_state)
            .add_debug_section(DebugSection {
                title: "Map pins",
                query: "/map_pins_state",
                fields: &[
                    ("Portable experiences read", "scenes"),
                    ("Pins read from those scenes", "read"),
                    ("Pins of other scenes, ignored", "ignored"),
                    ("HUD subscribers", "subscribers"),
                    ("Pins last sent (with a scene icon / in all)", "sent"),
                    ("Pins last sent (parcel and title)", "pins"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/map_pins_state")]
struct MapPinsState;

fn map_pins_state(
    mut input: ConsoleCommand<MapPinsState>,
    streams: Res<MapPinStreams>,
    pins: Query<&SceneEntity, With<MapPin>>,
    reading: Query<&RendererSceneContext, With<ReadsMapPins>>,
) {
    if input.take().is_none() {
        return;
    }
    let read = pins
        .iter()
        .filter(|scene_entity| reading.contains(scene_entity.root))
        .count();
    let ignored = pins.iter().count() - read;
    let scenes = reading
        .iter()
        .map(|context| format!("{} ({})", context.title, context.hash))
        .collect::<Vec<_>>();
    let sent = streams.last.as_ref();
    input.reply_ok(
        json!({
            "scenes": (!scenes.is_empty()).then(|| scenes.join(", ")),
            "read": read,
            "ignored": ignored,
            "subscribers": streams.senders.iter().filter(|s| !s.is_closed()).count(),
            "sent": sent.map(|list| [list.iter().filter(|p| p.icon.is_some()).count(), list.len()]),
            "pins": sent.filter(|list| !list.is_empty()).map(|list| list
                .iter()
                .map(|p| format!("{},{} {}", p.x, p.y, p.title))
                .collect::<Vec<_>>()
                .join("; ")),
            "unavailable": {
                "scenes": "No running portable experience declares isPortableExperience: true.",
                "sent": "The HUD is not following the map pins.",
                "pins": "No pins sent.",
            },
        })
        .to_string(),
    );
}
