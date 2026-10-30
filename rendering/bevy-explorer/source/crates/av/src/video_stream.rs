use bevy::prelude::*;
use common::util::ReportErr;
use dcl_component::proto_components::sdk::components::VideoState;
use ipfs::IpfsResource;
use media::{AVCommand, VideoData};

use crate::{AVPlayer, AVSinks};
#[cfg(feature = "ffmpeg")]
use {
    crate::audio_sink::AudioSink,
    common::structs::AudioDecoderError,
    kira::sound::streaming::StreamingSoundData,
    media::{ffmpeg_worker, AudioHandle},
};

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

    #[cfg(feature = "ffmpeg")]
    let audio = {
        let (audio_sender, audio_receiver) = tokio::sync::mpsc::channel(10);
        let (handle_sender, handle_receiver) = tokio::sync::oneshot::channel();
        spawn_av_thread(
            ipfs,
            SendersAndReceivers {
                commands: command_receiver,
                disposed: command_sender.downgrade(),
                frames: T::HAS_VIDEO.then_some(video_sender),
                audio: audio_sender,
                state_sender,
                audio_handle: handle_receiver,
            },
            source.clone(),
            hash,
        );
        Some(AudioSink::new(audio_receiver, handle_sender))
    };
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
        #[cfg(feature = "ffmpeg")]
        audio,
        video: image
            .filter(|_| T::HAS_VIDEO)
            .map(|image| VideoSink::new(source, video_receiver, image)),
        command_sender,
        rate: None,
        state: state_receiver,
        sent_event: 0.,
        sent_volume: None,
        sent_state: VideoState::VsNone,
        _phantom: Default::default(),
    }
}

/// Every av thread not yet seen finished, for [`join_av_threads`].
#[cfg(feature = "ffmpeg")]
static AV_THREADS: std::sync::Mutex<Vec<std::thread::JoinHandle<()>>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(feature = "ffmpeg")]
pub struct SendersAndReceivers {
    commands: tokio::sync::mpsc::UnboundedReceiver<AVCommand>,
    disposed: tokio::sync::mpsc::WeakUnboundedSender<AVCommand>,
    frames: Option<tokio::sync::mpsc::Sender<VideoData>>,
    audio: tokio::sync::mpsc::Sender<StreamingSoundData<AudioDecoderError>>,
    state_sender: tokio::sync::watch::Sender<VideoState>,
    audio_handle: tokio::sync::oneshot::Receiver<AudioHandle>,
}

#[cfg(feature = "ffmpeg")]
pub fn spawn_av_thread(
    ipfs: IpfsResource,
    senders_and_receivers: SendersAndReceivers,
    path: String,
    hash: String,
) {
    let thread = std::thread::spawn(move || av_thread(ipfs, senders_and_receivers, path, hash));
    let mut threads = AV_THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    threads.retain(|thread| !thread.is_finished());
    threads.push(thread);
}

/// Waits up to `timeout` for every av thread spawned so far, returning whether all finished.
/// A thread whose sinks are gone leaves promptly: ffmpeg's open and reads are interrupted
/// once the command sender is dropped.
#[cfg(feature = "ffmpeg")]
pub fn join_av_threads(timeout: std::time::Duration) -> bool {
    let mut threads = std::mem::take(
        &mut *AV_THREADS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    let deadline = std::time::Instant::now() + timeout;
    loop {
        // `is_finished` only says the thread's body returned; join it so its teardown is over too
        let (done, pending): (Vec<_>, Vec<_>) =
            threads.into_iter().partition(|thread| thread.is_finished());
        for thread in done {
            let _ = thread.join();
        }
        threads = pending;
        if threads.is_empty() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(feature = "ffmpeg")]
fn av_thread(
    ipfs: IpfsResource,
    senders_and_receivers: SendersAndReceivers,
    path: String,
    hash: String,
) {
    info!(
        "spawned av thread {:?}, path {path}",
        std::thread::current().id()
    );
    let _span = bevy::log::tracing::info_span!("av-thread").entered();
    let SendersAndReceivers {
        commands,
        disposed,
        frames,
        audio,
        state_sender,
        audio_handle,
    } = senders_and_receivers;
    if let Err(e) = ffmpeg_worker(
        &ipfs,
        commands,
        disposed,
        frames.clone(),
        audio,
        state_sender.clone(),
        audio_handle,
        path,
        hash,
    ) {
        state_sender.send_replace(VideoState::VsError);
        warn!("av error: {e}");
    } else {
        debug!("av closed");
    }
}

/// Sinks that never play: an empty source.
pub fn noop_sinks<T: AVPlayer>(source: String, image: Option<Handle<Image>>) -> AVSinks<T> {
    let (command_sender, command_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (video_sender, video_receiver) = tokio::sync::mpsc::channel(10);
    #[cfg_attr(not(feature = "html"), expect(unused))]
    let (state_sender, state_receiver) = tokio::sync::watch::channel(VideoState::VsLoading);

    #[cfg(feature = "ffmpeg")]
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
        #[cfg(feature = "ffmpeg")]
        audio,
        video: image.map(|image| VideoSink::new(source, video_receiver, image)),
        command_sender,
        rate: None,
        state: state_receiver,
        sent_event: 0.,
        sent_volume: None,
        sent_state: VideoState::VsNone,
        _phantom: Default::default(),
    }
}
