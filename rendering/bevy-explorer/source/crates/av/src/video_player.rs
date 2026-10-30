use bevy::{
    asset::RenderAssetTransferPriority,
    color::palettes::basic,
    diagnostic::FrameCount,
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
};
use common::{
    debug_panic,
    structs::{AudioSettings, PrimaryUser},
    util::ReportErr,
};
use ipfs::IpfsResource;
use livestream_manager::{ReceiverImage, ReceiverVolume};
use media::{AVCommand, VideoData, VideoInfo};
use scene_runner::{
    automatic_testing::SnapshotInProgress,
    renderer_context::RendererSceneContext,
    update_world::material::{update_materials, VideoTextureOutput},
    ContainerEntity, ContainingScene, SceneEntity,
};

use crate::{
    plugin::push_state_to_crdt,
    video_stream::{av_sinks, noop_sinks},
    AVPlayer, AVPlayerConfig, AVSinks, AudioStream, ShouldBePlaying, Stream, VideoPlayer,
    LIVEKIT_VIDEO_STREAM,
};

pub struct VideoPlayerPlugin;

impl Plugin for VideoPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, play_videos.before(update_materials));
        app.add_systems(
            Update,
            (
                publish_av_player_state::<AudioStream>,
                publish_av_player_state::<VideoPlayer>,
                publish_stream_state,
            ),
        );
        app.add_systems(
            Update,
            (
                update_av_volume::<AudioStream>,
                update_av_volume::<VideoPlayer>,
            ),
        );

        app.add_observer(new_audio_stream_source);
        app.add_observer(new_video_player_source);
        app.add_observer(player_source_replaced::<AudioStream>);
        app.add_observer(player_source_replaced::<VideoPlayer>);
        app.add_observer(player_config_added::<AudioStream>);
        app.add_observer(player_config_added::<VideoPlayer>);
        app.add_observer(player_position_added::<AudioStream>);
        app.add_observer(player_position_added::<VideoPlayer>);
        app.add_observer(av_sinks_on_remove::<AudioStream>);
        app.add_observer(av_sinks_on_remove::<VideoPlayer>);
        app.add_observer(av_player_should_be_playing_on_add::<AudioStream>);
        app.add_observer(av_player_should_be_playing_on_add::<VideoPlayer>);
        app.add_observer(av_player_should_be_playing_on_remove::<AudioStream>);
        app.add_observer(av_player_should_be_playing_on_remove::<VideoPlayer>);
    }
}

#[expect(clippy::type_complexity)]
fn new_audio_stream_source(
    trigger: Trigger<OnInsert, <AudioStream as AVPlayer>::Source>,
    mut commands: Commands,
    av_players: Query<(
        &<AudioStream as AVPlayer>::Source,
        &ContainerEntity,
        Option<&AVSinks<AudioStream>>,
    )>,
    scenes: Query<&RendererSceneContext>,
    ipfs: Res<IpfsResource>,
) {
    let entity = trigger.target();

    let (source, container_entity, maybe_sinks) = av_players
        .get(entity)
        .unwrap_or_else(|e| panic!("new_audio_stream_source query failed: {e}"));
    let Ok(context) = scenes.get(container_entity.root) else {
        debug_panic!(
            "{} has an invalid link to RendererSceneContext",
            disqualified::ShortName::of::<<AudioStream as AVPlayer>::Source>()
        );
    };

    if let Some(sinks) = maybe_sinks {
        sinks.command_sender.send(AVCommand::Dispose).report();
    }

    let sinks = match &(**source) {
        "" => noop_sinks::<AudioStream>((**source).to_owned(), None),
        other => av_sinks::<AudioStream>(
            (*ipfs).clone(),
            other.to_owned(),
            context.hash.to_string(),
            None,
            true,
            false,
        ),
    };

    debug!(
        "Creating new sinks for {} targeting \"{}\"",
        entity,
        &(**source)
    );
    commands.entity(entity).try_insert(sinks);
}

#[expect(clippy::type_complexity)]
fn new_video_player_source(
    trigger: Trigger<OnInsert, <VideoPlayer as AVPlayer>::Source>,
    mut commands: Commands,
    av_players: Query<(
        &<VideoPlayer as AVPlayer>::Source,
        &ContainerEntity,
        Option<&VideoTextureOutput>,
        Option<&AVSinks<VideoPlayer>>,
        Has<Stream>,
    )>,
    scenes: Query<&RendererSceneContext>,
    mut images: ResMut<Assets<Image>>,
    ipfs: Res<IpfsResource>,
) {
    let entity = trigger.target();

    let (source, container_entity, mut maybe_video_texture_output, maybe_sinks, has_stream) =
        av_players
            .get(entity)
            .unwrap_or_else(|e| panic!("new_video_player_source query failed: {e}"));
    let Ok(context) = scenes.get(container_entity.root) else {
        debug_panic!(
            "{} has an invalid link to RendererSceneContext",
            disqualified::ShortName::of::<<VideoPlayer as AVPlayer>::Source>()
        );
    };

    if let Some(sinks) = maybe_sinks {
        sinks.command_sender.send(AVCommand::Dispose).report();
    }

    let livestream = &**source == LIVEKIT_VIDEO_STREAM;
    if livestream != has_stream {
        if livestream {
            debug!("VideoPlayer {} now a stream.", entity);
            commands.entity(entity).try_insert(Stream);
        } else {
            debug!("VideoPlayer {} no longer a stream.", entity);
            commands.entity(entity).try_remove::<Stream>();
            let _ = maybe_video_texture_output.take();
        }
    }

    let mut create_image_handle = || match maybe_video_texture_output {
        None => {
            let mut image = Image::new_fill(
                bevy::render::render_resource::Extent3d {
                    width: 8,
                    height: 8,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &basic::FUCHSIA.to_u8_array(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::all(),
            );
            image.texture_descriptor.usage = TextureUsages::COPY_DST
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT;
            image.transfer_priority = RenderAssetTransferPriority::Immediate;
            images.add(image)
        }
        Some(texture) => {
            debug!("Reusing VideoTextureOutput.");
            texture.0.clone()
        }
    };

    let image = match &(**source) {
        LIVEKIT_VIDEO_STREAM => return,
        _ => create_image_handle(),
    };
    let sinks = match &(**source) {
        "" => noop_sinks::<VideoPlayer>((**source).to_owned(), Some(image.clone())),
        other => av_sinks::<VideoPlayer>(
            (*ipfs).clone(),
            other.to_owned(),
            context.hash.to_string(),
            Some(image.clone()),
            true,
            false,
        ),
    };

    debug!(
        "Creating new sinks for {} targeting \"{}\"",
        entity,
        &(**source)
    );
    commands
        .entity(entity)
        .try_insert((VideoTextureOutput(image), sinks));
}

fn player_source_replaced<T: AVPlayer>(
    trigger: Trigger<OnReplace, T::Source>,
    mut commands: Commands,
    av_players: Query<Option<&AVSinks<T>>, With<T::Source>>,
) {
    let entity = trigger.target();
    let maybe_sinks = av_players.get(entity).unwrap_or_else(|e| {
        panic!(
            "player_source_replaced::<{}> query failed: {e}",
            disqualified::ShortName::of::<T>()
        )
    });

    debug!(
        "{}'s {} was replaced.",
        entity,
        disqualified::ShortName::of::<T::Source>(),
    );
    commands.entity(entity).try_remove::<AVSinks<T>>();

    if let Some(sinks) = maybe_sinks {
        sinks.command_sender.send(AVCommand::Dispose).report();
    }
}

#[expect(clippy::type_complexity)]
fn player_config_added<T: AVPlayer>(
    trigger: Trigger<OnInsert, T::Config>,
    mut av_players: Query<(
        &T::Config,
        Option<&mut AVSinks<T>>,
        Has<ShouldBePlaying<T>>,
        Has<Stream>,
        Option<&mut ReceiverVolume>,
    )>,
) {
    let entity = trigger.target();
    let (config, maybe_sinks, has_should_be_playing, has_stream, maybe_receiver_volume) =
        av_players.get_mut(entity).unwrap_or_else(|e| {
            panic!(
                "player_config_added::<{}> query failed: {e}",
                disqualified::ShortName::of::<T>()
            )
        });
    let Some(mut sinks) = maybe_sinks else {
        if !has_stream {
            debug_panic!(
                "Non-stream {} did not have sinks.",
                disqualified::ShortName::of::<T::Source>()
            );
        }
        if let Some(mut receiver_volume) = maybe_receiver_volume {
            debug!("Updated volume of stream.");
            **receiver_volume = config.volume();
        }
        return;
    };

    let AVSinks {
        #[cfg(feature = "ffmpeg")]
            audio: _,
        video: _,
        command_sender,
        rate,
        state: _,
        sent_event: _,
        sent_volume: _,
        sent_state: _,
        _phantom,
    } = sinks.as_mut();

    if config.playing() && has_should_be_playing {
        command_sender.send(AVCommand::Play).report();
    } else {
        command_sender.send(AVCommand::Pause).report();
    }
    command_sender
        .send(AVCommand::Repeat(config.r#loop()))
        .report();
    *rate = Some(config.playback_rate() as f64);
}

/// Where a player's volume is decided: its config, the user's scene-audio setting, and silence
/// while the user is outside its scene. The backend applies it at its output.
fn update_av_volume<T: AVPlayer>(
    mut av_players: Query<(&SceneEntity, &T::Config, &mut AVSinks<T>)>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    audio_settings: Res<AudioSettings>,
) {
    let containing_scenes = player
        .single()
        .ok()
        .map(|player| containing_scene.get(player))
        .unwrap_or_default();

    for (scene, config, mut sinks) in av_players.iter_mut() {
        let AVSinks {
            #[cfg(feature = "ffmpeg")]
                audio: _,
            video: _,
            command_sender,
            rate: _,
            state: _,
            sent_event: _,
            sent_volume,
            sent_state: _,
            _phantom,
        } = sinks.as_mut();

        let volume = if containing_scenes.contains(&scene.root) {
            config.volume() * audio_settings.scene()
        } else {
            0.0
        };
        if sent_volume != &Some(volume) {
            *sent_volume = Some(volume);
            command_sender.send(AVCommand::Volume(volume)).report();
        }
    }
}

#[expect(clippy::type_complexity)]
fn player_position_added<T: AVPlayer>(
    trigger: Trigger<OnInsert, T::Position>,
    av_players: Query<(&T::Position, Option<&AVSinks<T>>, Has<Stream>)>,
) {
    let entity = trigger.target();
    let (position, maybe_sinks, has_stream) = av_players.get(entity).unwrap_or_else(|e| {
        panic!(
            "player_position_added::<{}> query failed: {e}",
            disqualified::ShortName::of::<T>()
        )
    });
    let Some(sinks) = maybe_sinks else {
        if !has_stream {
            debug_panic!(
                "Non-stream {} did not have sinks.",
                disqualified::ShortName::of::<T::Source>()
            );
        }
        return;
    };

    debug!(
        "Seeking {} to {}",
        disqualified::ShortName::of::<T::Source>(),
        (**position)
    );
    sinks
        .command_sender
        .send(AVCommand::Seek((**position) as f64))
        .report();
}

fn av_sinks_on_remove<T: AVPlayer>(trigger: Trigger<OnRemove, AVSinks<T>>, mut commands: Commands) {
    let entity = trigger.target();
    commands.entity(entity).try_remove::<VideoTextureOutput>();
}

fn av_player_should_be_playing_on_add<T: AVPlayer>(
    trigger: Trigger<OnAdd, ShouldBePlaying<T>>,
    av_players: Query<&AVSinks<T>, With<T>>,
) {
    let entity = trigger.target();
    let Ok(sinks) = av_players.get(entity) else {
        return;
    };

    sinks.command_sender.send(AVCommand::Play).report();
}

/// Sends [`AVCommand::Pause`] to [`AVPlayer`]
/// on removal of [`ShouldBePlaying`]
fn av_player_should_be_playing_on_remove<T: AVPlayer>(
    trigger: Trigger<OnRemove, ShouldBePlaying<T>>,
    av_players: Query<&AVSinks<T>, With<T>>,
) {
    let entity = trigger.target();
    let Ok(sinks) = av_players.get(entity) else {
        return;
    };

    sinks.command_sender.send(AVCommand::Pause).report();
}

fn play_videos(
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(&mut AVSinks<VideoPlayer>, &mut VideoTextureOutput)>,
    snapshot: Option<Res<SnapshotInProgress>>,
) {
    // A software-rendered capture can take several video frames to become ready.
    // Keep the requested frame until readback, then resume consuming the bounded queue.
    if snapshot.is_some_and(|snapshot| snapshot.0) {
        return;
    }

    for (mut video_player_sinks, mut output) in q.iter_mut() {
        let AVSinks {
            #[cfg(feature = "ffmpeg")]
                audio: _,
            video: Some(video),
            command_sender: _,
            rate: sink_rate,
            state: _,
            sent_event: _,
            sent_volume: _,
            sent_state: _,
            _phantom,
        } = video_player_sinks.as_mut()
        else {
            continue;
        };

        let mut last_frame_received = None;
        loop {
            match video.video_receiver.try_recv() {
                Ok(VideoData::Info(VideoInfo {
                    width,
                    height,
                    rate,
                    length,
                })) => {
                    debug!("resize");
                    let image = images.get_mut(&video.image).unwrap();
                    let target_extent = Extent3d {
                        width: width.max(16),
                        height: height.max(16),
                        depth_or_array_layers: 1,
                    };
                    if image.texture_descriptor.size != target_extent {
                        debug!("resize {target_extent:?}");
                        image.data = None;
                        image.texture_descriptor.size = target_extent;
                        image.transfer_priority = RenderAssetTransferPriority::Immediate;
                        video.resized = true;
                    }
                    video.length = Some(length);
                    *sink_rate = Some(rate);
                }
                Ok(VideoData::Frame(frame, time)) => {
                    last_frame_received = Some(frame);
                    video.current_time = time;
                }
                Err(_) => break,
            }
        }

        if let Some(frame) = last_frame_received {
            trace!("set frame on {:?}", video.image);

            // no pixels: the render world copied the frame into the gpu image itself
            if let Some(pixels) = frame.data() {
                let image = images.get_mut(&video.image).unwrap();

                match &mut image.data {
                    Some(data) => {
                        data.copy_from_slice(pixels);
                        image.transfer_priority = RenderAssetTransferPriority::Priority(-2);
                    }
                    None => {
                        image.data = Some(pixels.to_owned());
                    }
                }
            }
            if video.resized {
                video.resized = false;
                output.set_changed();
            }
        }
    }
}

const EVENT_REPORT_FREQUENCY: f64 = 1.0;

fn publish_av_player_state<T: AVPlayer>(
    mut q: Query<(&mut AVSinks<T>, &ContainerEntity)>,
    mut scenes: Query<&mut RendererSceneContext>,
    time: Res<Time>,
    frame: Res<FrameCount>,
) {
    for (mut video_player_sinks, container) in q.iter_mut() {
        let AVSinks {
            #[cfg(feature = "ffmpeg")]
                audio: _,
            video,
            command_sender: _,
            rate: _,
            state,
            sent_event,
            sent_volume: _,
            sent_state,
            _phantom,
        } = video_player_sinks.as_mut();

        let Ok(mut context) = scenes.get_mut(container.root) else {
            continue;
        };

        let new_state = *state.borrow_and_update();
        if &new_state != sent_state
            || time.elapsed_secs_f64() >= *sent_event + EVENT_REPORT_FREQUENCY
        {
            *sent_event = time.elapsed_secs_f64();
            *sent_state = new_state;
            push_state_to_crdt::<T>(&mut context, container, &frame, new_state, video.as_mut());
        }
    }
}

fn publish_stream_state(
    mut q: Query<(&mut ReceiverImage, &ContainerEntity), With<VideoPlayer>>,
    mut scenes: Query<&mut RendererSceneContext>,
    time: Res<Time>,
    frame: Res<FrameCount>,
) {
    for (mut receiver_image, container) in q.iter_mut() {
        let ReceiverImage {
            image: _,
            state,
            sent_event,
            sent_state,
        } = receiver_image.as_mut();

        let Ok(mut context) = scenes.get_mut(container.root) else {
            continue;
        };

        let new_state = *state.borrow_and_update();
        if &new_state != sent_state
            || time.elapsed_secs_f64() >= *sent_event + EVENT_REPORT_FREQUENCY
        {
            *sent_event = time.elapsed_secs_f64();
            *sent_state = new_state;
            push_state_to_crdt::<VideoPlayer>(&mut context, container, &frame, new_state, None);
        }
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use dcl_component::{proto_components::sdk::components::VideoState, SceneEntityId};

    #[test]
    fn holds_video_updates_until_snapshot_readback_finishes() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<FrameCount>()
            .add_systems(Update, play_videos);
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width: 16,
                    height: 16,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::all(),
            ));
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let (commands, _commands_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_state_sender, state_receiver) = tokio::sync::watch::channel(VideoState::VsLoading);
        app.world_mut().spawn((
            AVSinks::<VideoPlayer> {
                #[cfg(feature = "ffmpeg")]
                audio: None,
                video: Some(crate::video_stream::VideoSink {
                    source: "fixture.mp4".to_owned(),
                    video_receiver: receiver,
                    image: image.clone(),
                    current_time: -1.0,
                    length: None,
                    resized: false,
                }),
                command_sender: commands,
                rate: None,
                state: state_receiver,
                sent_event: 0.,
                sent_state: VideoState::VsLoading,
                sent_volume: None,
                _phantom: Default::default(),
            },
            ContainerEntity {
                container: Entity::PLACEHOLDER,
                root: Entity::PLACEHOLDER,
                container_id: SceneEntityId::ROOT,
            },
            VideoTextureOutput(image.clone()),
        ));
        let queue_size = |width| {
            assert!(sender
                .try_send(VideoData::Info(VideoInfo {
                    width,
                    height: 16,
                    rate: 30.0,
                    length: 4.0,
                }))
                .is_ok());
        };
        let width = |app: &App| {
            app.world()
                .resource::<Assets<Image>>()
                .get(&image)
                .unwrap()
                .texture_descriptor
                .size
                .width
        };

        // Normal playback has no capture resource and consumes queued updates.
        queue_size(32);
        app.update();
        assert_eq!(width(&app), 32);

        app.insert_resource(SnapshotInProgress(true));
        queue_size(64);
        for _ in 0..4 {
            app.update();
            assert_eq!(width(&app), 32);
        }

        app.world_mut().resource_mut::<SnapshotInProgress>().0 = false;
        app.update();
        assert_eq!(width(&app), 64);
    }
}
