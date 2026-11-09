use bevy::{
    diagnostic::FrameCountPlugin, platform::collections::HashSet, prelude::*,
    state::app::StatesPlugin, time::TimePlugin,
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

use crate::{
    AVPlayer, AVSinks, AudioStream, InScene, ShouldBePlaying, Stream, VideoPlayer,
    LIVEKIT_VIDEO_STREAM,
};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

fn stalled_media_url() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static SERVER: std::sync::OnceLock<std::net::TcpListener> = std::sync::OnceLock::new();
        let server = SERVER.get_or_init(|| std::net::TcpListener::bind("127.0.0.1:0").unwrap());
        format!("http://{}/video.mp4", server.local_addr().unwrap())
    }
    #[cfg(target_arch = "wasm32")]
    "https://example.com/video.mp4".to_owned()
}

macro_rules! test_component {
    ($app:expr, $entity:expr, $component:ty, $expected:expr) => {
        assert_eq!(
            $app.world().entity($entity).contains::<$component>(),
            $expected
        );
    };
}

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
    test_component!(app, entity, AVSinks<T>, media);
}

fn record_commands<T: AVPlayer>(
    app: &mut App,
    entity: Entity,
) -> tokio::sync::mpsc::UnboundedReceiver<media::AVCommand> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    app.world_mut()
        .get_mut::<AVSinks<T>>(entity)
        .unwrap()
        .command_sender = Some(sender);
    receiver
}

fn assert_disposed_once(mut commands: tokio::sync::mpsc::UnboundedReceiver<media::AVCommand>) {
    assert!(matches!(
        commands.try_recv().unwrap(),
        media::AVCommand::Dispose
    ));
    assert!(matches!(
        commands.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    ));
}

fn min_test_app() -> App {
    common::test_support::init_logging();
    let mut app = App::new();

    app.add_plugins((
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
        TimePlugin,
    ));

    app.init_asset::<Image>();
    app.init_asset::<Shader>();
    app.init_asset::<Mesh>();

    app.add_plugins((
        SceneRunnerPlugin,
        LivestreamManagerPlugin,
        bevy_console::ConsolePlugin,
    ));

    crate::plugin::add_av_players(&mut app);

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
                url: stalled_media_url(),
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
        false,
        true,
    );
    let commands = record_commands::<AudioStream>(&mut app, audio_stream);
    app.world_mut()
        .entity_mut(audio_stream)
        .insert(AudioStream(PbAudioStream::default()));
    assert_disposed_once(commands);
}

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
        false,
        true,
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn livekit_audio_stream_is_not_a_livestream_receiver() {
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
        false,
        true,
    );
    #[cfg(feature = "ffmpeg")]
    {
        use dcl_component::proto_components::sdk::components::VideoState;
        let mut state = app
            .world()
            .get::<AVSinks<AudioStream>>(audio_stream)
            .unwrap()
            .state
            .clone();
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    state.wait_for(|state| *state == VideoState::VsError),
                )
                .await
                .expect("unsupported LiveKit audio URL must fail promptly")
                .expect("the worker must publish its error state");
            });
    }
    app.world_mut().entity_mut(audio_stream).despawn();
}

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
                src: stalled_media_url(),
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
                src: stalled_media_url(),
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

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
pub(super) fn test_source_change() {
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

    let commands = record_commands::<VideoPlayer>(&mut app, video_player);
    app.world_mut()
        .entity_mut(video_player)
        .insert(VideoPlayer(PbVideoPlayer {
            src: stalled_media_url(),
            ..Default::default()
        }));
    assert_disposed_once(commands);

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
            src: stalled_media_url(),
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
            src: stalled_media_url(),
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
                src: stalled_media_url(),
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

#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub(super) fn test_stalled_source_shutdown() {
    use std::{
        io::Read,
        net::TcpListener,
        time::{Duration, Instant},
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut app = min_test_app();
    let ipfs = app.world().resource::<ipfs::IpfsResource>().clone();
    let sinks = crate::video_stream::av_sinks::<VideoPlayer>(
        ipfs,
        format!("http://{}/stalled.mp4", listener.local_addr().unwrap()),
        "hash".to_owned(),
        Some(Handle::default()),
        true,
        false,
    );
    app.world_mut().spawn(sinks);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut connection = loop {
        match listener.accept() {
            Ok((connection, _)) => break connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "FFmpeg never opened the local source"
                );
                std::thread::park_timeout(Duration::from_millis(10));
            }
            Err(error) => panic!("{error}"),
        }
    };
    connection
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = [0; 4];
    connection.read_exact(&mut request).unwrap();
    assert_eq!(&request, b"GET ");
    drop(app);
    assert!(crate::video_stream::join_av_threads(Duration::from_secs(
        10
    )));
}
