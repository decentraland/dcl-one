//! The `AudioAnalysis` SDK component: the amplitude and eight band levels of the audio an
//! entity plays, written back to the scene whenever a new window of it has been heard, after
//! unity-explorer's `AudioAnalysisSystem` (`Explorer/Assets/DCL/SDKComponents/AudioAnalysis`).
//! A clip (`AudioSource`) is analysed from its decoded frames at the instance's position; a
//! stream (`AudioStream`, the audio of a `VideoPlayer`) from the window the FFmpeg decoder
//! captures while an analysis sits on the entity. The maths is [`dsp`], the port of the Unity library.

pub mod dsp;
mod fft;

use bevy::prelude::*;
use dcl_component::{
    proto_components::sdk::components::PbAudioAnalysis, CrdtType, SceneComponentId, SceneEntityId,
};
use dcl_types::interface::ComponentPosition;
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::AddCrdtInterfaceExt, ContainerEntity,
};

use dsp::{Analysis, Mode, Scratch, DEFAULT_AMPLITUDE_GAIN, DEFAULT_BANDS_GAIN};

#[derive(Component, Debug)]
pub struct AudioAnalysis(pub PbAudioAnalysis);

impl From<PbAudioAnalysis> for AudioAnalysis {
    fn from(value: PbAudioAnalysis) -> Self {
        Self(value)
    }
}

/// The analysis of an entity so far: the last result, which window produced it, and counters
/// for the debug panel. Lives next to the `AudioAnalysis` component.
#[derive(Component, Default)]
pub struct AudioAnalysisState {
    pub last: Analysis,
    /// The clip window last analysed (`position / WINDOW_FRAMES`), so a window is analysed once.
    pub last_block: Option<u64>,
    pub window_frames: u32,
    pub sample_rate: u32,
    pub analyses: u32,
    window: Vec<f32>,
    scratch: Scratch,
}

pub struct AudioAnalysisPlugin;

impl Plugin for AudioAnalysisPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbAudioAnalysis, AudioAnalysis>(
            SceneComponentId::AUDIO_ANALYSIS,
            ComponentPosition::EntityOnly,
        );
        app.add_observer(analysis_added);
        app.add_observer(analysis_removed);
        #[cfg(not(feature = "html"))]
        app.add_systems(PostUpdate, analyse_audio_clips);
        #[cfg(feature = "ffmpeg")]
        {
            app.add_observer(sinks_inserted::<crate::AudioStream>);
            app.add_observer(sinks_inserted::<crate::VideoPlayer>);
            app.add_systems(
                PostUpdate,
                (
                    analyse_audio_streams::<crate::AudioStream>,
                    analyse_audio_streams::<crate::VideoPlayer>,
                ),
            );
        }
    }
}

/// The gains the scene set, or Unity's defaults.
fn gains(source: &PbAudioAnalysis) -> (Mode, f32, f32) {
    (
        Mode::from(source.mode()),
        source.amplitude_gain.unwrap_or(DEFAULT_AMPLITUDE_GAIN),
        source.bands_gain.unwrap_or(DEFAULT_BANDS_GAIN),
    )
}

/// Writes the whole component back, the scene's parameters as received plus the result, as
/// Unity's `PutMessage` does.
fn put_analysis(
    context: &mut RendererSceneContext,
    id: SceneEntityId,
    source: &PbAudioAnalysis,
    result: &Analysis,
) {
    let written = PbAudioAnalysis {
        mode: source.mode,
        amplitude_gain: source.amplitude_gain,
        bands_gain: source.bands_gain,
        amplitude: result.amplitude,
        band_0: result.bands[0],
        band_1: result.bands[1],
        band_2: result.bands[2],
        band_3: result.bands[3],
        band_4: result.bands[4],
        band_5: result.bands[5],
        band_6: result.bands[6],
        band_7: result.bands[7],
    };
    context.update_crdt(
        SceneComponentId::AUDIO_ANALYSIS,
        CrdtType::LWW_ENT,
        id,
        &written,
    );
}

/// Analyses `state.window` at `sample_rate` and writes the result to the scene.
fn analyse(
    state: &mut AudioAnalysisState,
    sample_rate: u32,
    source: &PbAudioAnalysis,
    context: &mut RendererSceneContext,
    id: SceneEntityId,
) {
    let (mode, amplitude_gain, bands_gain) = gains(source);
    let AudioAnalysisState {
        window, scratch, ..
    } = state;
    let result = dsp::analyze(
        window,
        sample_rate as f32,
        mode,
        amplitude_gain,
        bands_gain,
        scratch,
    );
    state.last = result;
    state.sample_rate = sample_rate;
    state.window_frames = dsp::WINDOW_FRAMES as u32;
    state.analyses = state.analyses.wrapping_add(1);
    put_analysis(context, id, source, &result);
}

#[expect(clippy::type_complexity)]
fn analysis_added(
    trigger: Trigger<OnAdd, AudioAnalysis>,
    mut commands: Commands,
    #[cfg(feature = "ffmpeg")] sinks: Query<(
        Option<&crate::AVSinks<crate::AudioStream>>,
        Option<&crate::AVSinks<crate::VideoPlayer>>,
    )>,
) {
    let entity = trigger.target();
    commands
        .entity(entity)
        .try_insert(AudioAnalysisState::default());
    #[cfg(feature = "ffmpeg")]
    if let Ok((audio, video)) = sinks.get(entity) {
        set_capture(audio, video, true);
    }
}

#[expect(clippy::type_complexity)]
fn analysis_removed(
    trigger: Trigger<OnRemove, AudioAnalysis>,
    mut commands: Commands,
    #[cfg(feature = "ffmpeg")] sinks: Query<(
        Option<&crate::AVSinks<crate::AudioStream>>,
        Option<&crate::AVSinks<crate::VideoPlayer>>,
    )>,
) {
    let entity = trigger.target();
    commands.entity(entity).try_remove::<AudioAnalysisState>();
    #[cfg(feature = "ffmpeg")]
    if let Ok((audio, video)) = sinks.get(entity) {
        set_capture(audio, video, false);
    }
}

#[cfg(feature = "ffmpeg")]
fn set_capture(
    audio: Option<&crate::AVSinks<crate::AudioStream>>,
    video: Option<&crate::AVSinks<crate::VideoPlayer>>,
    enabled: bool,
) {
    use crate::AVPlayerSinks;
    for sink in [
        audio.and_then(|sinks| sinks.audio_sink()),
        video.and_then(|sinks| sinks.audio_sink()),
    ]
    .into_iter()
    .flatten()
    {
        sink.capture.set_enabled(enabled);
    }
}

/// A player's sinks arrived on an entity that already asks for an analysis: capture from the
/// first chunk.
#[cfg(feature = "ffmpeg")]
fn sinks_inserted<T: crate::AVPlayer>(
    trigger: Trigger<OnInsert, crate::AVSinks<T>>,
    sinks: Query<&crate::AVSinks<T>, With<AudioAnalysis>>,
) {
    use crate::AVPlayerSinks;
    if let Ok(sinks) = sinks.get(trigger.target()) {
        if let Some(sink) = sinks.audio_sink() {
            sink.capture.set_enabled(true);
        }
    }
}

/// A clip is analysed from its decoded frames: the window that ends at the instance's position,
/// once per window, at the clip's own rate. Pitch and volume are not applied (a divergence from
/// Unity, which analyses the mixed output). Values stay while the clip is paused or stopped.
#[cfg(not(feature = "html"))]
fn analyse_audio_clips(
    mut analysed: Query<
        (
            &ContainerEntity,
            &AudioAnalysis,
            &common::structs::AudioEmitter,
            &crate::audio_source_native::ClipInstance,
            &mut AudioAnalysisState,
        ),
        With<crate::audio_source_native::Playing>,
    >,
    instances: Res<Assets<bevy_kira_audio::AudioInstance>>,
    clips: Res<Assets<bevy_kira_audio::AudioSource>>,
    mut scenes: Query<&mut RendererSceneContext>,
) {
    for (container, analysis, emitter, instance, mut state) in analysed.iter_mut() {
        let Some(bevy_kira_audio::PlaybackState::Playing { position }) =
            instances.get(&**instance).map(|instance| instance.state())
        else {
            continue;
        };
        let Some(clip) = clips.get(&emitter.handle) else {
            continue;
        };
        let sample_rate = clip.sound.sample_rate;
        let end = (position.max(0.0) * sample_rate as f64) as usize;
        let block = (end / dsp::WINDOW_FRAMES) as u64;
        if state.last_block == Some(block) {
            continue;
        }
        let Ok(mut context) = scenes.get_mut(container.root) else {
            continue;
        };
        state.last_block = Some(block);
        dsp::window_at(&clip.sound.frames, end, &mut state.window);
        analyse(
            &mut state,
            sample_rate,
            &analysis.0,
            &mut context,
            container.container_id,
        );
    }
}

/// A stream is analysed from the window its decoder captured since the last one, at the
/// stream's own rate; nothing is captured while the player is not playing.
#[cfg(feature = "ffmpeg")]
fn analyse_audio_streams<T: crate::AVPlayer>(
    mut analysed: Query<(
        &ContainerEntity,
        &AudioAnalysis,
        &crate::AVSinks<T>,
        &mut AudioAnalysisState,
    )>,
    mut scenes: Query<&mut RendererSceneContext>,
) {
    use crate::AVPlayerSinks;
    for (container, analysis, sinks, mut state) in analysed.iter_mut() {
        let Some(sink) = sinks.audio_sink() else {
            continue;
        };
        let sample_rate = sink.capture.sample_rate();
        if sample_rate == 0 || !sink.capture.take_window(&mut state.window) {
            continue;
        }
        let Ok(mut context) = scenes.get_mut(container.root) else {
            continue;
        };
        analyse(
            &mut state,
            sample_rate,
            &analysis.0,
            &mut context,
            container.container_id,
        );
    }
}

/// Whether the entity's player captures for analysis right now, for the debug panel.
#[cfg(feature = "ffmpeg")]
pub fn capture_enabled(
    audio: Option<&crate::AVSinks<crate::AudioStream>>,
    video: Option<&crate::AVSinks<crate::VideoPlayer>>,
) -> Option<(bool, u32)> {
    use crate::AVPlayerSinks;
    audio
        .and_then(|sinks| sinks.audio_sink())
        .or_else(|| video.and_then(|sinks| sinks.audio_sink()))
        .map(|sink| (sink.capture.enabled(), sink.capture.dropped()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::platform::collections::HashSet;
    use dcl_component::{
        proto_components::sdk::components::PbAudioAnalysisMode, DclReader, FromDclReader,
    };
    use dcl_types::SceneId;

    fn scene_context() -> RendererSceneContext {
        RendererSceneContext::new(
            SceneId::DUMMY,
            "hash".to_owned(),
            "storage_root".to_owned(),
            false,
            0,
            "title".to_owned(),
            IVec2::splat(0),
            HashSet::from_iter([IVec2::splat(0)]),
            vec![],
            Default::default(),
            vec![],
            Entity::PLACEHOLDER,
            0.,
            false,
            "sdk_version",
            false,
            false,
        )
    }

    #[test]
    fn the_gains_default_to_unity_s_when_the_scene_sets_none() {
        let source = PbAudioAnalysis {
            mode: PbAudioAnalysisMode::ModeLogarithmic as i32,
            ..Default::default()
        };
        assert_eq!(gains(&source), (Mode::Logarithmic, 5.0, 0.05));
        let source = PbAudioAnalysis {
            mode: PbAudioAnalysisMode::ModeRaw as i32,
            amplitude_gain: Some(2.0),
            bands_gain: Some(0.5),
            ..Default::default()
        };
        assert_eq!(gains(&source), (Mode::Raw, 2.0, 0.5));
    }

    #[test]
    fn the_write_back_keeps_the_scene_s_parameters_and_carries_the_result() {
        let mut context = scene_context();
        let id = SceneEntityId::new(512, 0);
        let source = PbAudioAnalysis {
            mode: PbAudioAnalysisMode::ModeLogarithmic as i32,
            amplitude_gain: Some(3.0),
            bands_gain: None,
            ..Default::default()
        };
        let result = Analysis {
            amplitude: 0.5,
            bands: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
        };
        put_analysis(&mut context, id, &source, &result);
        let updates = context.crdt_store.take_updates();
        let bytes = &updates
            .lww
            .get(&SceneComponentId::AUDIO_ANALYSIS)
            .unwrap()
            .last_write
            .get(&id)
            .unwrap()
            .data;
        let written = PbAudioAnalysis::from_reader(&mut DclReader::new(bytes)).unwrap();
        assert_eq!(written.mode, source.mode);
        assert_eq!(written.amplitude_gain, Some(3.0));
        assert_eq!(written.bands_gain, None);
        assert_eq!(written.amplitude, 0.5);
        assert_eq!(written.band_0, 0.1);
        assert_eq!(written.band_7, 0.8);
    }

    #[test]
    fn an_analysis_counts_and_records_its_window() {
        let mut context = scene_context();
        let mut state = AudioAnalysisState {
            window: vec![0.5; dsp::WINDOW_SAMPLES],
            ..Default::default()
        };
        let source = PbAudioAnalysis {
            mode: PbAudioAnalysisMode::ModeRaw as i32,
            ..Default::default()
        };
        analyse(
            &mut state,
            48_000,
            &source,
            &mut context,
            SceneEntityId::new(512, 0),
        );
        assert_eq!(state.analyses, 1);
        assert_eq!(state.sample_rate, 48_000);
        assert_eq!(state.window_frames, 1024);
        assert!((state.last.amplitude - 0.5).abs() < 1e-6);
    }
}
