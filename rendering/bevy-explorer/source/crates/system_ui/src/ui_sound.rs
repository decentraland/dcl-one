//! UI sounds the HUD asks for by name. The HUD decides when a line warrants a ping; the engine
//! owns the clips and the mixer.

use bevy::prelude::*;
use common::structs::SystemAudio;
use system_bridge::SystemApi;

/// The embedded `sounds/ui` clips a scene may play.
const ALLOWED: &[&str] = &["chat_message_receive.wav", "chat_mention_receive.wav"];

pub struct UiSoundPlugin;

impl Plugin for UiSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, play_ui_sounds);
    }
}

fn play_ui_sounds(mut requests: EventReader<SystemApi>, mut commands: Commands) {
    for request in requests.read() {
        let SystemApi::PlayUiSound(name) = request else {
            continue;
        };
        if !ALLOWED.contains(&name.as_str()) {
            warn!("ignoring ui sound `{name}`");
            continue;
        }
        commands.send_event(SystemAudio(format!("embedded://sounds/ui/{name}")));
    }
}
