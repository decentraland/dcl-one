//! Plays the audio of other players' voices (foreign audio sources) on kira.

use std::marker::PhantomData;

use bevy::{prelude::*, render::view::RenderLayers};
use common::{
    structs::{AudioDecoderError, AudioSettings},
    util::VolumePanning,
};
use comms::global_crdt::{ForeignAudioSource, NearbyPanning, VoicePan};
use kira::{
    manager::backend::DefaultBackend,
    sound::{streaming::StreamingSoundData, PlaybackState},
    tween::Tween,
};

pub struct AudioSinkPlugin;

impl Plugin for AudioSinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                spawn_new_foreign_audio_sources,
                check_foreign_audio_source_still_playing,
                update_foreign_audio_player_volume,
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct AudioSpawned<T> {
    handle: <StreamingSoundData<AudioDecoderError> as kira::sound::SoundData>::Handle,
    _phantom: PhantomData<T>,
}

impl<T> AudioSpawned<T> {
    pub fn new(
        handle: <StreamingSoundData<AudioDecoderError> as kira::sound::SoundData>::Handle,
    ) -> Self {
        Self {
            handle,
            _phantom: Default::default(),
        }
    }
}

impl<T> Drop for AudioSpawned<T> {
    fn drop(&mut self) {
        self.handle.stop(Tween::default());
    }
}

fn spawn_new_foreign_audio_sources(
    mut commands: Commands,
    foreign_audio_sources: Populated<
        (Entity, &mut ForeignAudioSource),
        Without<AudioSpawned<ForeignAudioSource>>,
    >,
    mut audio_manager: NonSendMut<bevy_kira_audio::audio_output::AudioOutput<DefaultBackend>>,
) {
    for (entity, mut foreign_audio_source) in foreign_audio_sources.into_inner() {
        if let Some(sound_data) = foreign_audio_source
            .audio_receiver
            .as_mut()
            .and_then(|rx| rx.try_recv().ok())
        {
            info!("{entity:?} received foreign sound data!");
            let handle = audio_manager
                .manager
                .as_mut()
                .unwrap()
                .play(sound_data)
                .unwrap();

            commands
                .entity(entity)
                .try_insert(AudioSpawned::<ForeignAudioSource>::new(handle));
        }
    }
}

fn check_foreign_audio_source_still_playing(
    mut commands: Commands,
    foreign_audio_sources: Populated<
        (Entity, &AudioSpawned<ForeignAudioSource>),
        With<ForeignAudioSource>,
    >,
) {
    for (entity, audio_spawned) in foreign_audio_sources.into_inner() {
        if !matches!(audio_spawned.handle.state(), PlaybackState::Playing) {
            commands
                .entity(entity)
                .try_remove::<AudioSpawned<ForeignAudioSource>>();
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_foreign_audio_player_volume(
    mut streams: Query<
        (
            &GlobalTransform,
            Option<&RenderLayers>,
            &mut AudioSpawned<ForeignAudioSource>,
            &mut VoicePan,
        ),
        With<ForeignAudioSource>,
    >,
    pan: VolumePanning,
    settings: Res<AudioSettings>,
    panning_settings: Res<NearbyPanning>,
    time: Res<Time>,
) {
    for (emitter_transform, render_layers, mut audio_spawned, mut voice_pan) in streams.iter_mut() {
        let (volume, panning) =
            pan.volume_and_panning(emitter_transform.translation(), render_layers);
        let volume = volume * settings.voice();
        let panning = voice_pan.update(panning, &panning_settings, time.delta_secs());

        audio_spawned
            .handle
            .set_volume(volume as f64, Tween::default());
        audio_spawned
            .handle
            .set_panning(panning as f64, Tween::default());
    }
}
