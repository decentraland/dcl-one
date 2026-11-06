use bevy::{
    diagnostic::FrameCountPlugin,
    log::{Level, LogPlugin},
    platform::collections::HashSet,
    prelude::*,
    state::app::StatesPlugin,
};
use common::structs::{AudioSettings, PrimaryCameraRes};
use dcl_component::{
    proto_components::sdk::components::{PbAudioStream, PbVideoPlayer},
    SceneEntityId,
};
use dcl_types::SceneId;
use ipfs::IpfsIoPlugin;
use livestream_manager::{plugin::LivestreamManagerPlugin, ActiveReceiver, ReceiverImage};
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::material::VideoTextureOutput,
    ContainerEntity, SceneRunnerPlugin,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

#[cfg(any(feature = "uuav", feature = "html"))]
use crate::AVSinks;
use crate::{AVPlayer, AVPlayerPlugin, AudioStream, InScene, ShouldBePlaying, Stream, VideoPlayer};
use livestream_manager::address::CURRENT_STREAM as LIVEKIT_VIDEO_STREAM;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

const EXAMPLE_VIDEO: &str = "https://example.com/video.mp4";

macro_rules! test_component {
    ($app:expr, $entity:expr, $component:ty, $expected:expr) => {
        assert_eq!(
            $app.world().entity($entity).contains::<$component>(),
            $expected
        );
    };
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[expect(clippy::too_many_arguments)]
fn test_components<T: AVPlayer>(
    app: &mut App,
    entity: Entity,
    component: bool,
    source: bool,
    config: bool,
    position: bool,
    stream: bool,
    in_scene: bool,
    should_be_playing: bool,
    active_receiver: bool,
    receiver_image: bool,
    video_texture_output: bool,
    media: bool,
) {
    test_component!(app, entity, T, component);
    test_component!(app, entity, T::Source, source);
    test_component!(app, entity, T::Config, config);
    test_component!(app, entity, T::Position, position);
    test_component!(app, entity, Stream, stream);
    test_component!(app, entity, InScene, in_scene);
    test_component!(app, entity, ShouldBePlaying<T>, should_be_playing);
    test_component!(app, entity, ActiveReceiver, active_receiver);
    test_component!(app, entity, ReceiverImage, receiver_image);
    test_component!(app, entity, VideoTextureOutput, video_texture_output);
    #[cfg(any(feature = "uuav", feature = "html"))]
    test_component!(app, entity, AVSinks<T>, media);
}

#[cfg(any(feature = "uuav", feature = "html"))]
fn min_test_app() -> App {
    let mut app = App::new();

    app.add_plugins((
        LogPlugin {
            level: Level::DEBUG,
            ..Default::default()
        },
        IpfsIoPlugin {
            preview: false,
            starting_realm: None,
            content_server_override: None,
            assets_root: Default::default(),
            num_slots: 1,
        },
        AssetPlugin::default(),
        StatesPlugin,
        FrameCountPlugin,
    ));

    app.init_asset::<Image>();
    app.init_asset::<Shader>();
    app.init_asset::<Mesh>();

    app.add_plugins((
        SceneRunnerPlugin,
        LivestreamManagerPlugin,
        AVPlayerPlugin,
        bevy_console::ConsolePlugin,
    ));

    app.insert_resource(PrimaryCameraRes(Entity::PLACEHOLDER));
    app.init_resource::<AudioSettings>();

    app.finish();

    app.world_mut().run_schedule(Startup);

    #[cfg(target_arch = "wasm32")]
    {
        let set_video_source = wasm_bindgen::closure::Closure::new(Box::new(set_video_source)
            as Box<dyn FnMut(wasm_bindgen::JsValue, wasm_bindgen::JsValue)>)
        .into_js_value();
        let window = web_sys::window().unwrap();
        js_sys::Reflect::set(
            &window,
            &wasm_bindgen::JsValue::from_str("setVideoSource"),
            &set_video_source,
        )
        .unwrap();
    }

    app
}

#[cfg(target_arch = "wasm32")]
fn set_video_source(_p0: wasm_bindgen::JsValue, _p1: wasm_bindgen::JsValue) {
    debug!("set_video_source");
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_insert_audio_stream() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let audio_stream = app
        .world_mut()
        .spawn((
            AudioStream(PbAudioStream {
                url: EXAMPLE_VIDEO.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<AudioStream>(
        &mut app,
        audio_stream,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        cfg!(not(target_arch = "wasm32")),
        true,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_insert_empty_audio_stream() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let audio_stream = app
        .world_mut()
        .spawn((
            AudioStream(PbAudioStream {
                url: "".to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<AudioStream>(
        &mut app,
        audio_stream,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        true,
        true,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_insert_livekit_audio_stream() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let audio_stream = app
        .world_mut()
        .spawn((
            AudioStream(PbAudioStream {
                url: LIVEKIT_VIDEO_STREAM.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<AudioStream>(
        &mut app,
        audio_stream,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        cfg!(not(target_arch = "wasm32")),
        true,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_insert_video_player() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: EXAMPLE_VIDEO.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        true,
        true,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_insert_empty_video_player() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: "".to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        true,
        true,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_insert_livekit_video_player() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: LIVEKIT_VIDEO_STREAM.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_removing_video_player() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: EXAMPLE_VIDEO.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        true,
        true,
    );

    app.world_mut()
        .entity_mut(video_player)
        .remove::<VideoPlayer>();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_removing_livekit_video_player() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: LIVEKIT_VIDEO_STREAM.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
    );

    app.world_mut()
        .entity_mut(video_player)
        .remove::<VideoPlayer>();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_livekit_video_player_without_should_be_playing_should_not_be_receiver() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: LIVEKIT_VIDEO_STREAM.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
    );

    app.world_mut().entity_mut(video_player).insert(InScene);

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
    );

    app.world_mut()
        .entity_mut(video_player)
        .insert(ShouldBePlaying::<VideoPlayer>::default());

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
    );

    app.world_mut()
        .entity_mut(video_player)
        .remove::<ShouldBePlaying<VideoPlayer>>();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
    );

    app.world_mut().entity_mut(video_player).remove::<InScene>();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
    );
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_source_change() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: "".to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
            InScene,
            ShouldBePlaying::<VideoPlayer>::default(),
        ))
        .id();

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        true,
        true,
        false,
        false,
        true,
        true,
    );

    let noop_handle = app
        .world()
        .entity(video_player)
        .get::<VideoTextureOutput>()
        .unwrap()
        .0
        .clone();

    app.world_mut()
        .entity_mut(video_player)
        .insert(VideoPlayer(PbVideoPlayer {
            src: EXAMPLE_VIDEO.to_owned(),
            ..Default::default()
        }));

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        true,
        true,
        false,
        false,
        true,
        true,
    );

    let video_handle = app
        .world()
        .entity(video_player)
        .get::<VideoTextureOutput>()
        .unwrap()
        .0
        .clone();
    assert_eq!(noop_handle, video_handle);

    app.world_mut()
        .entity_mut(video_player)
        .insert(VideoPlayer(PbVideoPlayer {
            src: LIVEKIT_VIDEO_STREAM.to_owned(),
            ..Default::default()
        }));

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
    );

    let stream_handle = app
        .world()
        .entity(video_player)
        .get::<VideoTextureOutput>()
        .unwrap()
        .0
        .clone();
    assert_ne!(video_handle, stream_handle);

    app.world_mut()
        .entity_mut(video_player)
        .insert(VideoPlayer(PbVideoPlayer {
            src: EXAMPLE_VIDEO.to_owned(),
            ..Default::default()
        }));

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        true,
        true,
        false,
        false,
        true,
        true,
    );

    let video_handle = app
        .world()
        .entity(video_player)
        .get::<VideoTextureOutput>()
        .unwrap()
        .0
        .clone();
    assert_ne!(stream_handle, video_handle);

    app.world_mut()
        .entity_mut(video_player)
        .insert(VideoPlayer(PbVideoPlayer {
            src: EXAMPLE_VIDEO.to_owned(),
            ..Default::default()
        }));

    test_components::<VideoPlayer>(
        &mut app,
        video_player,
        true,
        true,
        true,
        true,
        false,
        true,
        true,
        false,
        false,
        true,
        true,
    );

    let repeat_video_handle = app
        .world()
        .entity(video_player)
        .get::<VideoTextureOutput>()
        .unwrap()
        .0
        .clone();
    assert_eq!(video_handle, repeat_video_handle);
}

#[cfg(any(feature = "uuav", feature = "html"))]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_despawn_av_player() {
    let mut app = min_test_app();

    let renderer_context = app
        .world_mut()
        .spawn(RendererSceneContext::new(
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
        ))
        .id();

    let video_player = app
        .world_mut()
        .spawn((
            VideoPlayer(PbVideoPlayer {
                src: EXAMPLE_VIDEO.to_owned(),
                ..Default::default()
            }),
            ContainerEntity {
                container: renderer_context,
                root: renderer_context,
                container_id: SceneEntityId::new(0, 0),
            },
            InScene,
            ShouldBePlaying::<VideoPlayer>::default(),
        ))
        .id();

    app.world_mut().entity_mut(video_player).despawn();
}

/// Pinned livekit players (`pinned_stream`): the binding, the images and the events, headless.
/// Tracks are plain `PublishedVideoTrack` entities: the comms plugin that subscribes them is
/// not part of this app.
#[cfg(any(feature = "uuav", feature = "html"))]
mod pinned {
    use super::*;
    use crate::{pinned_stream::PinnedStream, SetState};
    use dcl_component::proto_components::sdk::components::VideoState;
    use livestream_manager::{
        PinnedBy, PinnedImage, PinnedTo, PublishedVideoTrack, VideoTrackLive,
    };

    #[derive(Resource, Default)]
    struct States(Vec<VideoState>);

    fn app_with_scene() -> (App, Entity) {
        let mut app = min_test_app();
        app.init_resource::<States>();
        app.add_observer(
            |trigger: Trigger<SetState<VideoPlayer>>, mut states: ResMut<States>| {
                states.0.push(trigger.event().state);
            },
        );
        let scene = app
            .world_mut()
            .spawn(RendererSceneContext::new(
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
            ))
            .id();
        (app, scene)
    }

    fn player(app: &mut App, scene: Entity, src: &str) -> Entity {
        app.world_mut()
            .spawn((
                VideoPlayer(PbVideoPlayer {
                    src: src.to_owned(),
                    playing: Some(true),
                    ..Default::default()
                }),
                ContainerEntity {
                    container: scene,
                    root: scene,
                    container_id: SceneEntityId::new(512, 0),
                },
            ))
            .id()
    }

    fn track(app: &mut App, room: &str, identity: &str, sid: &str) -> Entity {
        app.world_mut()
            .spawn(PublishedVideoTrack {
                room: room.to_owned(),
                identity: identity.to_owned(),
                sid: sid.to_owned(),
            })
            .id()
    }

    fn schedule(app: &mut App, player: Entity) {
        app.world_mut()
            .entity_mut(player)
            .insert((InScene, ShouldBePlaying::<VideoPlayer>::default()));
    }

    /// A frame's worth of the pinned machinery: `min_test_app` cannot run its schedules (the
    /// scene runner's systems want resources no test creates), so the state transitions and the
    /// two pinned systems run on their own; observers fire as their commands apply.
    fn tick(app: &mut App) {
        use bevy::{ecs::system::RunSystemOnce, state::state::StateTransition};
        for _ in 0..3 {
            app.world_mut().run_schedule(StateTransition);
            app.world_mut()
                .run_system_once(crate::pinned_stream::bind_pinned_streams)
                .unwrap();
            app.world_mut()
                .run_system_once(crate::pinned_stream::sync_pinned_texture)
                .unwrap();
        }
    }

    fn states(app: &mut App) -> Vec<VideoState> {
        std::mem::take(&mut app.world_mut().resource_mut::<States>().0)
    }

    fn texture(app: &App, player: Entity) -> Option<Handle<Image>> {
        app.world()
            .get::<VideoTextureOutput>(player)
            .map(|output| output.0.clone())
    }

    fn pinned_image(app: &App, track: Entity) -> Option<Handle<Image>> {
        app.world()
            .get::<PinnedImage>(track)
            .map(|image| (**image).clone())
    }

    const PINNED: &str = "livekit-video://0xabc/TR_1";

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn a_pinned_source_is_a_stream_without_sinks() {
        let (mut app, scene) = app_with_scene();
        let player = player(&mut app, scene, PINNED);
        tick(&mut app);
        assert!(app.world().get::<Stream>(player).is_some());
        assert!(app.world().get::<PinnedStream>(player).is_some());
        assert!(app.world().get::<AVSinks<VideoPlayer>>(player).is_none());
        assert!(app.world().get::<PinnedTo>(player).is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn a_malformed_address_plays_nothing_and_reports_an_error() {
        let (mut app, scene) = app_with_scene();
        let player = player(&mut app, scene, "livekit-video://broken");
        tick(&mut app);
        assert!(app.world().get::<Stream>(player).is_none());
        assert!(app.world().get::<PinnedStream>(player).is_none());
        assert!(app.world().get::<AVSinks<VideoPlayer>>(player).is_some());
        assert!(states(&mut app).contains(&VideoState::VsError));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn a_scheduled_pinned_player_binds_the_track_in_its_room_and_shows_its_image() {
        let (mut app, scene) = app_with_scene();
        let elsewhere = track(&mut app, "other", "0xabc", "TR_1");
        let track = track(&mut app, "hash", "0xabc", "TR_1");
        let player = player(&mut app, scene, PINNED);
        tick(&mut app);
        assert!(
            app.world().get::<PinnedTo>(player).is_none(),
            "not scheduled yet"
        );

        schedule(&mut app, player);
        tick(&mut app);
        assert_eq!(
            app.world().get::<PinnedTo>(player).map(|p| p.0),
            Some(track)
        );
        assert!(app.world().get::<PinnedBy>(track).is_some());
        assert!(app.world().get::<PinnedBy>(elsewhere).is_none());
        let image = pinned_image(&app, track);
        assert!(image.is_some());
        assert_eq!(
            texture(&app, player),
            image,
            "the player shows the track's image"
        );
        assert_eq!(states(&mut app).last(), Some(&VideoState::VsLoading));

        app.world_mut().entity_mut(track).insert(VideoTrackLive);
        tick(&mut app);
        assert!(states(&mut app).contains(&VideoState::VsPlaying));
        app.world_mut().entity_mut(track).remove::<VideoTrackLive>();
        tick(&mut app);
        assert!(states(&mut app).contains(&VideoState::VsLoading));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn a_wrong_sid_never_binds() {
        let (mut app, scene) = app_with_scene();
        track(&mut app, "hash", "0xabc", "TR_2");
        let player = player(&mut app, scene, PINNED);
        schedule(&mut app, player);
        tick(&mut app);
        assert!(app.world().get::<PinnedTo>(player).is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn a_departed_track_falls_back_to_the_manager_and_is_bound_again_on_return() {
        let (mut app, scene) = app_with_scene();
        let track = track(&mut app, "hash", "0xabc", "TR_1");
        let player = player(&mut app, scene, PINNED);
        schedule(&mut app, player);
        tick(&mut app);
        assert!(app.world().get::<PinnedTo>(player).is_some());
        states(&mut app);

        app.world_mut().entity_mut(track).despawn();
        tick(&mut app);
        assert!(app.world().get::<PinnedTo>(player).is_none());
        let manager = app
            .world()
            .get::<ReceiverImage>(player)
            .map(|image| (**image).clone());
        assert!(manager.is_some(), "the player is still a receiver");
        assert_eq!(
            texture(&app, player),
            manager,
            "back on the manager's image"
        );
        assert!(states(&mut app).contains(&VideoState::VsLoading));
        assert!(
            app.world().get::<PinnedStream>(player).is_some(),
            "the address is kept"
        );

        let returned = self::track(&mut app, "hash", "0xabc", "TR_1");
        tick(&mut app);
        assert_eq!(
            app.world().get::<PinnedTo>(player).map(|p| p.0),
            Some(returned)
        );
        assert_eq!(texture(&app, player), pinned_image(&app, returned));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn unscheduling_unbinds_and_pauses_and_despawning_releases_the_track() {
        let (mut app, scene) = app_with_scene();
        let track = track(&mut app, "hash", "0xabc", "TR_1");
        let player = player(&mut app, scene, PINNED);
        schedule(&mut app, player);
        tick(&mut app);
        states(&mut app);

        app.world_mut()
            .entity_mut(player)
            .remove::<ShouldBePlaying<VideoPlayer>>();
        tick(&mut app);
        assert!(app.world().get::<PinnedTo>(player).is_none());
        assert!(states(&mut app).contains(&VideoState::VsPaused));
        assert!(app.world().get::<PinnedBy>(track).is_none());

        schedule(&mut app, player);
        tick(&mut app);
        assert!(app.world().get::<PinnedBy>(track).is_some());
        app.world_mut().entity_mut(player).despawn();
        tick(&mut app);
        assert!(app.world().get::<PinnedBy>(track).is_none());
        assert!(app.world().get::<PinnedImage>(track).is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn a_source_change_from_pinned_to_the_sentinel_to_http_clears_the_pin() {
        let (mut app, scene) = app_with_scene();
        let track = track(&mut app, "hash", "0xabc", "TR_1");
        let player = player(&mut app, scene, PINNED);
        schedule(&mut app, player);
        tick(&mut app);
        assert!(app.world().get::<PinnedTo>(player).is_some());

        app.world_mut()
            .entity_mut(player)
            .insert(VideoPlayer(PbVideoPlayer {
                src: LIVEKIT_VIDEO_STREAM.to_owned(),
                playing: Some(true),
                ..Default::default()
            }));
        tick(&mut app);
        assert!(app.world().get::<Stream>(player).is_some());
        assert!(app.world().get::<PinnedStream>(player).is_none());
        assert!(app.world().get::<PinnedTo>(player).is_none());
        assert!(app.world().get::<PinnedBy>(track).is_none());

        app.world_mut()
            .entity_mut(player)
            .insert(VideoPlayer(PbVideoPlayer {
                src: EXAMPLE_VIDEO.to_owned(),
                playing: Some(true),
                ..Default::default()
            }));
        tick(&mut app);
        assert!(app.world().get::<Stream>(player).is_none());
        assert!(app.world().get::<AVSinks<VideoPlayer>>(player).is_some());
    }
}
