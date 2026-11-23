//! Plays the sounds the Explorer's emote code asks for. The Explorer's player (its `av` crate)
//! brings scenes and comms with it, so the preview plays the same `AudioEmitter` components itself:
//! each emitter that changes to playing starts its sound once, follows the emote's pause, and goes
//! away with the emitter.

use bevy::prelude::*;
use bevy_kira_audio::{
    prelude::{AudioControl, AudioInstance},
    Audio, AudioTween, PlaybackState,
};
use common::structs::AudioEmitter;

/// Whether emote sound plays at all (the host's sound toggle), and whether it is held (the emote
/// is paused).
#[derive(Resource, Debug)]
pub struct PreviewSound {
    pub enabled: bool,
    pub held: bool,
}

impl Default for PreviewSound {
    fn default() -> Self {
        Self {
            enabled: true,
            held: false,
        }
    }
}

#[derive(Component)]
struct Playing(Handle<AudioInstance>);

pub struct PreviewAudioPlugin;

impl Plugin for PreviewAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreviewSound>()
            .add_systems(PostUpdate, (play_emitters, follow_sound_state).chain());
    }
}

fn play_emitters(
    mut commands: Commands,
    audio: Res<Audio>,
    sound: Res<PreviewSound>,
    emitters: Query<(Entity, &AudioEmitter, Option<&Playing>), Changed<AudioEmitter>>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    for (entity, emitter, playing) in emitters.iter() {
        if let Some(instance) = playing.and_then(|p| instances.get_mut(&p.0)) {
            instance.stop(AudioTween::default());
        }
        if !emitter.playing || !sound.enabled {
            commands.entity(entity).remove::<Playing>();
            continue;
        }
        let handle = audio
            .play(emitter.handle.clone())
            .with_volume(emitter.volume as f64)
            .handle();
        commands.entity(entity).insert(Playing(handle));
    }
}

/// Pauses and resumes playing sounds with the emote, stops them when sound is turned off.
fn follow_sound_state(
    sound: Res<PreviewSound>,
    playing: Query<&Playing>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if !sound.is_changed() {
        return;
    }
    for handle in playing.iter() {
        let Some(instance) = instances.get_mut(&handle.0) else {
            continue;
        };
        if !sound.enabled {
            instance.stop(AudioTween::default());
        } else if sound.held {
            if matches!(instance.state(), PlaybackState::Playing { .. }) {
                instance.pause(AudioTween::default());
            }
        } else if matches!(instance.state(), PlaybackState::Paused { .. }) {
            instance.resume(AudioTween::default());
        }
    }
}
