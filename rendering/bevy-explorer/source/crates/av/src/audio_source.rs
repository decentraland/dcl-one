use std::path::PathBuf;

use bevy::{asset::LoadState, prelude::*, render::view::RenderLayers};
use common::{
    sets::{SceneSets, SetupSets},
    structs::{
        AudioEmitter, AudioType, PrimaryCameraRes, SystemAudio, PRIMARY_AVATAR_LIGHT_LAYER_INDEX,
    },
    util::AudioReceiver,
};
use dcl_component::{
    proto_components::sdk::components::{common::MediaState, PbAudioSource},
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    IpfsAssetServer,
};
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::AddCrdtInterfaceExt, SceneEntity,
};

#[derive(Component, Debug)]
pub struct AudioSource(pub PbAudioSource);

/// The emitter has a playback instance (kira natively, a page graph on the web).
#[derive(Component)]
pub struct Playing;

/// The emitter's instance could not start this frame (the clip or the page buffer is not ready).
#[derive(Component)]
pub struct RetryEmitter;

/// Where the emitter's clip asset is, as the asset server last reported it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipLoad {
    Loading,
    Loaded,
    Failed,
}

/// The last `PBAudioEvent` state the scene was told about for this AudioSource
/// (unity-explorer's `AudioSourceComponent.LastPropagatedAudioState`, initially `MS_NONE`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastPropagatedAudioState(pub MediaState);

impl Default for LastPropagatedAudioState {
    fn default() -> Self {
        Self(MediaState::MsNone)
    }
}

impl From<PbAudioSource> for AudioSource {
    fn from(value: PbAudioSource) -> Self {
        Self(value)
    }
}

pub struct AudioSourcePlugin;

impl Plugin for AudioSourcePlugin {
    fn build(&self, app: &mut App) {
        // Custom `.audio` loader for scene-content clips fetched by content hash
        // (no on-wire extension). Format is detected from the byte stream.
        app.register_asset_loader(crate::audio_loader::AudioAssetLoader);
        app.add_event::<SystemAudio>();
        app.add_crdt_lww_component::<PbAudioSource, AudioSource>(
            SceneComponentId::AUDIO_SOURCE,
            ComponentPosition::EntityOnly,
        );
        app.add_systems(
            Update,
            (
                map_scene_audio_sources,
                track_clip_load,
                crate::audio_events::report_audio_source_events,
            )
                .chain()
                .in_set(SceneSets::PostLoop),
        );
        app.add_systems(Startup, setup_audio.in_set(SetupSets::Main));
    }

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy_kira_audio::AudioPlugin>() {
            error!("bevy_kira_audio is missing, there will be no sounds.");
        }
    }
}

fn setup_audio(mut commands: Commands, camera: Res<PrimaryCameraRes>) {
    commands.entity(camera.0).try_insert(AudioReceiver {
        layers: RenderLayers::default().with(PRIMARY_AVATAR_LIGHT_LAYER_INDEX),
    });
}

fn map_scene_audio_sources(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &SceneEntity,
            &AudioSource,
            Option<&mut AudioEmitter>,
        ),
        Changed<AudioSource>,
    >,
    ipfas: IpfsAssetServer,
    scenes: Query<&RendererSceneContext>,
) {
    for (ent, scene_ent, audio_source, maybe_emitter) in query.iter_mut() {
        let Ok(scene) = scenes.get(scene_ent.root) else {
            warn!("failed to load audio source scene");
            continue;
        };
        let ipfs_path = PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
            scene.hash.to_string(),
            audio_source.0.audio_clip_url.to_owned(),
        )));

        let maybe_emitter_was_none = maybe_emitter.is_none();
        let handle = maybe_emitter
            .and_then(|mut existing| {
                if existing.handle.path().is_none_or(|p| p.path() != ipfs_path) {
                    None
                } else {
                    Some(std::mem::take(&mut existing.handle))
                }
            })
            .unwrap_or_else(|| ipfas.asset_server().load(ipfs_path));

        let seek_time = audio_source.0.current_time.and_then(|time| {
            if time < 1e6 {
                Some(time)
            } else {
                warn!(
                    "ignoring ridiculous time offset {} for audio clip `{}`",
                    time, audio_source.0.audio_clip_url
                );
                None
            }
        });

        let emitter = AudioEmitter {
            handle,
            playing: audio_source.0.playing(),
            playback_speed: audio_source.0.pitch.unwrap_or(1.0),
            r#loop: audio_source.0.r#loop(),
            volume: audio_source.0.volume.unwrap_or(1.0),
            global: audio_source.0.global(),
            seek_time,
            ty: AudioType::Scene,
        };

        debug!("emitter: {emitter:?}");

        let mut entity = commands.entity(ent);
        entity.try_insert(emitter);
        if maybe_emitter_was_none {
            entity.try_insert(LastPropagatedAudioState::default());
        }
    }
}

/// Mirrors the asset server's view of each emitter's clip into [`ClipLoad`], writing only on a
/// change.
pub fn track_clip_load(
    mut commands: Commands,
    emitters: Query<(Entity, &AudioEmitter, Option<&ClipLoad>), With<AudioSource>>,
    asset_server: Res<AssetServer>,
) {
    for (ent, emitter, maybe_load) in emitters.iter() {
        let load = match asset_server.get_load_state(emitter.handle.id()) {
            Some(LoadState::Loaded) => ClipLoad::Loaded,
            Some(LoadState::Failed(_)) => ClipLoad::Failed,
            Some(LoadState::NotLoaded | LoadState::Loading) | None => ClipLoad::Loading,
        };
        if maybe_load != Some(&load) {
            commands.entity(ent).try_insert(load);
        }
    }
}
