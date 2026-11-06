use bevy::prelude::*;
use common::util::ReportErr;
use dcl_component::proto_components::sdk::components::VideoState;
use ipfs::IpfsResource;
use media::{AVCommand, VideoData};

#[cfg(feature = "uuav")]
use crate::audio_sink::AudioSink;
use crate::{AVPlayer, AVSinks};

pub struct VideoSink {
    pub source: String,
    pub video_receiver: tokio::sync::mpsc::Receiver<VideoData>,
    pub image: Handle<Image>,
    pub current_time: f64,
    pub length: Option<f64>,
    /// The image changed size and the materials on it must rebind.
    pub resized: bool,
}

impl VideoSink {
    fn new(
        source: String,
        video_receiver: tokio::sync::mpsc::Receiver<VideoData>,
        image: Handle<Image>,
    ) -> Self {
        Self {
            source,
            video_receiver,
            image,
            current_time: -1.0,
            length: None,
            resized: false,
        }
    }
}

/// Opens `source` on the platform's av backend, rendering video into `image`. The audio and
/// video sinks share one command channel.
pub fn av_sinks<T: AVPlayer>(
    ipfs: IpfsResource,
    source: String,
    hash: String,
    image: Option<Handle<Image>>,
    playing: bool,
    repeat: bool,
) -> AVSinks<T> {
    let (command_sender, command_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (video_sender, video_receiver) = tokio::sync::mpsc::channel(10);
    let (state_sender, state_receiver) = tokio::sync::watch::channel(VideoState::VsLoading);

    #[cfg(feature = "uuav")]
    let (audio, rejected_commands) = {
        let (audio_sender, audio_receiver) = tokio::sync::mpsc::channel(10);
        let (handle_sender, handle_receiver) = tokio::sync::oneshot::channel();
        let rejected_commands = match media::resolve_source(&ipfs, &source, &hash) {
            Ok(url) => {
                media::spawn_av(
                    command_receiver,
                    T::HAS_VIDEO.then_some(video_sender),
                    state_sender,
                    url,
                    image.as_ref(),
                    audio_sender,
                    handle_receiver,
                );
                None
            }
            Err(e) => {
                warn!("av source `{source}`: {e}");
                state_sender.send_replace(VideoState::VsError);
                Some(command_receiver)
            }
        };
        (
            Some(AudioSink::new(audio_receiver, handle_sender)),
            rejected_commands,
        )
    };
    #[cfg(not(feature = "uuav"))]
    let rejected_commands = None;
    #[cfg(feature = "html")]
    {
        let url = ipfs
            .content_url(&source, &hash)
            .unwrap_or_else(|| source.clone());
        media::spawn_av(
            command_receiver,
            video_sender,
            state_sender,
            Some(url),
            T::HAS_VIDEO,
            image.as_ref(),
        );
    }

    if playing {
        command_sender.send(AVCommand::Play).report();
    }
    command_sender.send(AVCommand::Repeat(repeat)).report();

    AVSinks {
        #[cfg(feature = "uuav")]
        audio,
        video: image
            .filter(|_| T::HAS_VIDEO)
            .map(|image| VideoSink::new(source, video_receiver, image)),
        command_sender,
        rejected_commands,
        rate: None,
        state: state_receiver,
        sent_event: 0.,
        sent_volume: None,
        sent_state: VideoState::VsNone,
        _phantom: Default::default(),
    }
}

/// Sinks that never play: an empty source.
pub fn noop_sinks<T: AVPlayer>(source: String, image: Option<Handle<Image>>) -> AVSinks<T> {
    let (command_sender, command_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (video_sender, video_receiver) = tokio::sync::mpsc::channel(10);
    #[cfg_attr(not(feature = "html"), expect(unused))]
    let (state_sender, state_receiver) = tokio::sync::watch::channel(VideoState::VsLoading);

    #[cfg(feature = "uuav")]
    let audio = {
        let (_audio_sender, audio_receiver) = tokio::sync::mpsc::channel(10);
        let (handle_sender, _handle_receiver) = tokio::sync::oneshot::channel();
        drop(command_receiver);
        drop(video_sender);
        Some(AudioSink::new(audio_receiver, handle_sender))
    };
    #[cfg(feature = "html")]
    media::spawn_av(
        command_receiver,
        video_sender,
        state_sender,
        None,
        T::HAS_VIDEO,
        image.as_ref(),
    );

    AVSinks {
        #[cfg(feature = "uuav")]
        audio,
        video: image.map(|image| VideoSink::new(source, video_receiver, image)),
        command_sender,
        rejected_commands: None,
        rate: None,
        state: state_receiver,
        sent_event: 0.,
        sent_volume: None,
        sent_state: VideoState::VsNone,
        _phantom: Default::default(),
    }
}
