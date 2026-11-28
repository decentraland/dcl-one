use std::path::PathBuf;

use bevy::{prelude::*, render::view::RenderLayers};
use common::{
    sets::{SceneSets, SetupSets},
    structs::{
        AudioEmitter, AudioType, PrimaryCameraRes, SystemAudio, PRIMARY_AVATAR_LIGHT_LAYER_INDEX,
    },
    util::AudioReceiver,
};
use dcl_component::{proto_components::sdk::components::PbAudioSource, SceneComponentId};
use dcl_types::interface::ComponentPosition;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    IpfsAssetServer,
};
use scene_runner::{
    initialize_scene::SuperUserScene, renderer_context::RendererSceneContext,
    update_world::AddCrdtInterfaceExt, SceneEntity,
};

#[derive(Component, Debug)]
pub struct AudioSource(PbAudioSource);

impl AudioSource {
    /// The component as the scene wrote it.
    pub(crate) fn proto(&self) -> &PbAudioSource {
        &self.0
    }
}

/// A decoded clip's length in seconds.
pub(crate) fn clip_seconds(clip: &bevy_kira_audio::AudioSource) -> f64 {
    clip.sound.frames.len() as f64 / f64::from(clip.sound.sample_rate)
}

/// What the backend is doing with a sounding emitter: written while the backend has an instance
/// of its clip, and removed when it stops or while the clip is not started yet, so `/media_state --audio` reads the same state on native and on the web.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct EmitterPlayback {
    /// Seconds into the clip.
    pub(crate) position: f64,
    /// The gain sent to the backend: the emitter's volume, the scene volume setting, zero while
    /// its scene is not current, and the distance falloff of a spatial emitter.
    pub(crate) gain: f32,
}

/// The backend saw the emitter's clip play to its end. It stays while the scene still asks for
/// the clip and goes when the scene stops asking or the clip starts again, so `/media_state --audio` can
/// tell a clip that ended from one the backend has not started yet.
#[derive(Component, Debug)]
pub(crate) struct EmitterEnded;

impl EmitterPlayback {
    pub(crate) fn write(
        self,
        commands: &mut Commands,
        entity: Entity,
        existing: Option<Mut<EmitterPlayback>>,
    ) {
        if let Some(mut existing) = existing {
            existing.set_if_neq(self);
        } else {
            commands.entity(entity).try_insert(self);
        }
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
        app.add_systems(Update, map_scene_audio_sources.in_set(SceneSets::PostLoop));
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
    scenes: Query<(&RendererSceneContext, Has<SuperUserScene>)>,
) {
    for (ent, scene_ent, audio_source, maybe_emitter) in query.iter_mut() {
        let Ok((scene, system_scene)) = scenes.get(scene_ent.root) else {
            warn!("failed to load audio source scene");
            continue;
        };
        let ipfs_path = PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
            scene.hash.to_string(),
            audio_source.0.audio_clip_url.to_owned(),
        )));

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
            volume: crate::scene_volume(audio_source.0.volume),
            global: audio_source.0.global(),
            seek_time,
            ty: if system_scene {
                AudioType::System
            } else {
                AudioType::Scene
            },
        };

        debug!("emitter: {emitter:?}");

        commands.entity(ent).try_insert(emitter);
    }
}
