//! `PBAudioEvent` for scene AudioSources, after unity-explorer's `AudioEventsSystem`
//! (`Explorer/Assets/DCL/SDKComponents/AudioSources/Systems/AudioEventsSystem.cs`): the state
//! of each clip goes to the scene as a grow-only append whenever it changes, and a clip that
//! plays to its end writes `playing = false` back into the scene's `PBAudioSource`.

use bevy::prelude::*;
use dcl_component::{
    proto_components::sdk::components::{common::MediaState, PbAudioEvent, PbAudioSource},
    CrdtType, SceneComponentId,
};
use scene_runner::{renderer_context::RendererSceneContext, ContainerEntity};

use crate::audio_source::{AudioSource, ClipLoad, LastPropagatedAudioState, Playing};

/// The state a clip reports (`AudioEventsSystem.GetAudioSourceState`): nothing to play, the clip
/// loading, a load failure, or playing versus ready. A stopped or finished clip is `Ready`:
/// Unity has no paused or finished state for clips.
pub fn audio_source_state(url_empty: bool, load: Option<ClipLoad>, playing: bool) -> MediaState {
    if url_empty {
        return MediaState::MsNone;
    }
    match load {
        None | Some(ClipLoad::Loading) => MediaState::MsLoading,
        Some(ClipLoad::Failed) => MediaState::MsError,
        Some(ClipLoad::Loaded) => {
            if playing {
                MediaState::MsPlaying
            } else {
                MediaState::MsReady
            }
        }
    }
}

/// A clip played to its end: it was playing, it is ready again, `playing` is still set and it
/// does not loop (`AudioEventsSystem.cs:111-148`). A source with `playing` cleared or looping is
/// not a finish.
pub fn is_natural_finish(previous: MediaState, state: MediaState, source: &PbAudioSource) -> bool {
    previous == MediaState::MsPlaying
        && state == MediaState::MsReady
        && source.playing()
        && !source.r#loop()
}

/// Appends an `AudioEvent` for every AudioSource whose state changed since the last one, and
/// writes the finish of a non-looping clip back as `playing = false`.
#[expect(clippy::type_complexity)]
pub fn report_audio_source_events(
    mut sources: Query<(
        &ContainerEntity,
        &mut AudioSource,
        Option<&ClipLoad>,
        Has<Playing>,
        &mut LastPropagatedAudioState,
    )>,
    mut scenes: Query<&mut RendererSceneContext>,
) {
    for (container, mut source, maybe_load, playing, mut last) in sources.iter_mut() {
        let state = audio_source_state(
            source.0.audio_clip_url.is_empty(),
            maybe_load.copied(),
            playing,
        );
        if state == last.0 {
            continue;
        }
        let Ok(mut context) = scenes.get_mut(container.root) else {
            continue;
        };
        let finished = is_natural_finish(last.0, state, &source.0);
        last.0 = state;
        let event = PbAudioEvent {
            state: state as i32,
            timestamp: context.tick_number,
        };
        context.update_crdt(
            SceneComponentId::AUDIO_EVENT,
            CrdtType::GO_ANY,
            container.container_id,
            &event,
        );
        if finished {
            debug!(
                "audio clip `{}` finished: writing playing=false back",
                source.0.audio_clip_url
            );
            let written = PbAudioSource {
                playing: Some(false),
                ..source.0.clone()
            };
            context.update_crdt(
                SceneComponentId::AUDIO_SOURCE,
                CrdtType::LWW_ENT,
                container.container_id,
                &written,
            );
            // the engine's copy follows what was written, without a change event
            source.bypass_change_detection().0 = written;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::platform::collections::HashSet;
    use dcl_component::{DclReader, FromDclReader, SceneEntityId};
    use dcl_types::SceneId;

    #[test]
    fn clip_states_follow_unity() {
        use ClipLoad::*;
        use MediaState::*;
        assert_eq!(audio_source_state(true, Some(Loaded), true), MsNone);
        assert_eq!(audio_source_state(false, None, true), MsLoading);
        assert_eq!(audio_source_state(false, Some(Loading), true), MsLoading);
        assert_eq!(audio_source_state(false, Some(Failed), true), MsError);
        assert_eq!(audio_source_state(false, Some(Loaded), true), MsPlaying);
        assert_eq!(audio_source_state(false, Some(Loaded), false), MsReady);
    }

    fn source(playing: bool, looping: bool) -> PbAudioSource {
        PbAudioSource {
            playing: Some(playing),
            volume: Some(0.5),
            r#loop: Some(looping),
            pitch: Some(1.25),
            audio_clip_url: "sounds/clip.ogg".to_owned(),
            current_time: None,
            global: Some(true),
        }
    }

    #[test]
    fn a_finish_is_only_playing_to_ready_of_a_non_looping_source_still_asked_to_play() {
        use MediaState::*;
        assert!(is_natural_finish(MsPlaying, MsReady, &source(true, false)));
        assert!(!is_natural_finish(
            MsPlaying,
            MsReady,
            &source(false, false)
        ));
        assert!(!is_natural_finish(MsPlaying, MsReady, &source(true, true)));
        assert!(!is_natural_finish(MsReady, MsPlaying, &source(true, false)));
        assert!(!is_natural_finish(MsPlaying, MsError, &source(true, false)));
        assert!(!is_natural_finish(MsLoading, MsReady, &source(true, false)));
    }

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

    fn entity_id() -> SceneEntityId {
        SceneEntityId::new(512, 0)
    }

    struct Harness {
        app: App,
        scene: Entity,
        clip: Entity,
    }

    fn harness(source: PbAudioSource, previous: MediaState, playing: bool) -> Harness {
        let mut app = App::new();
        app.add_systems(Update, report_audio_source_events);
        let mut context = scene_context();
        context.tick_number = 7;
        let scene = app.world_mut().spawn(context).id();
        let mut clip = app.world_mut().spawn((
            AudioSource(source),
            ContainerEntity {
                container: scene,
                root: scene,
                container_id: entity_id(),
            },
            ClipLoad::Loaded,
            LastPropagatedAudioState(previous),
        ));
        if playing {
            clip.insert(Playing);
        }
        let clip = clip.id();
        Harness { app, scene, clip }
    }

    impl Harness {
        fn update(&mut self) {
            self.app.update();
        }

        /// The audio events appended since the last call, and the AudioSource written back, if any.
        fn take(&mut self) -> (Vec<PbAudioEvent>, Option<PbAudioSource>) {
            let mut context = self
                .app
                .world_mut()
                .get_mut::<RendererSceneContext>(self.scene)
                .unwrap();
            let updates = context.crdt_store.take_updates();
            let events = updates
                .go
                .get(&SceneComponentId::AUDIO_EVENT)
                .and_then(|state| state.0.get(&entity_id()))
                .map(|entries| {
                    entries
                        .iter()
                        .map(|entry| {
                            PbAudioEvent::from_reader(&mut DclReader::new(&entry.data)).unwrap()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let written = updates
                .lww
                .get(&SceneComponentId::AUDIO_SOURCE)
                .and_then(|state| state.last_write.get(&entity_id()))
                .map(|entry| PbAudioSource::from_reader(&mut DclReader::new(&entry.data)).unwrap());
            (events, written)
        }

        fn last_state(&self) -> MediaState {
            self.app
                .world()
                .get::<LastPropagatedAudioState>(self.clip)
                .unwrap()
                .0
        }

        fn engine_source(&self) -> PbAudioSource {
            self.app
                .world()
                .get::<AudioSource>(self.clip)
                .unwrap()
                .0
                .clone()
        }
    }

    #[test]
    fn a_finished_clip_is_reported_ready_and_written_back_once() {
        let mut h = harness(source(true, false), MediaState::MsPlaying, false);
        h.update();
        let (events, written) = h.take();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, MediaState::MsReady as i32);
        assert_eq!(events[0].timestamp, 7);
        let written = written.expect("the AudioSource is written back");
        assert_eq!(written.playing, Some(false));
        assert_eq!(
            PbAudioSource {
                playing: Some(true),
                ..written.clone()
            },
            source(true, false),
            "every other field is kept"
        );
        assert_eq!(h.engine_source().playing, Some(false));
        assert_eq!(h.last_state(), MediaState::MsReady);

        h.update();
        let (events, written) = h.take();
        assert!(events.is_empty(), "no second event: {events:?}");
        assert!(written.is_none(), "no second write-back");
    }

    #[test]
    fn a_scene_stop_is_an_event_without_a_write_back() {
        let mut h = harness(source(false, false), MediaState::MsPlaying, false);
        h.update();
        let (events, written) = h.take();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, MediaState::MsReady as i32);
        assert!(written.is_none());
    }

    #[test]
    fn a_looping_clip_is_never_written_back() {
        let mut h = harness(source(true, true), MediaState::MsPlaying, false);
        h.update();
        let (events, written) = h.take();
        assert_eq!(events.len(), 1);
        assert!(written.is_none());
    }

    #[test]
    fn an_unchanged_state_is_not_repeated() {
        let mut h = harness(source(true, false), MediaState::MsPlaying, true);
        h.update();
        let (events, written) = h.take();
        assert!(events.is_empty());
        assert!(written.is_none());
    }

    #[test]
    fn a_failed_load_and_an_empty_url_are_reported() {
        let mut h = harness(source(true, false), MediaState::MsNone, false);
        h.app
            .world_mut()
            .entity_mut(h.clip)
            .insert(ClipLoad::Failed);
        h.update();
        let (events, _) = h.take();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, MediaState::MsError as i32);

        let mut empty = source(true, false);
        empty.audio_clip_url.clear();
        let mut h = harness(empty, MediaState::MsLoading, false);
        h.update();
        let (events, _) = h.take();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, MediaState::MsNone as i32);
    }
}
