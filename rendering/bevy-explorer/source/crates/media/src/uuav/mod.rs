//! Native media on the UUAV player (`library/uuav`): the sandboxed `uuav-helper` process
//! demuxes and decodes every scene video and audio stream (hardware first, software fallback)
//! and shares its frames with this process through the `uuav` client crate, which this module
//! drives as plain Rust calls. The engine side is the [`AVCommand`]s and [`VideoData`] the
//! `av` crate speaks ([`spawn_av`]): the main world polls each player's state and clock
//! ([`serve_uuav_players`]), kira pulls each player's audio through [`audio::UuavKiraDecoder`],
//! and the render world converts each player's presented NV12 planes into its target image
//! ([`render`]), so decoded pixels never cross the main world.

mod audio;
mod ffi;
#[cfg_attr(target_os = "macos", path = "platform_macos.rs")]
#[cfg_attr(target_os = "linux", path = "platform_linux.rs")]
#[cfg_attr(windows, path = "platform_windows.rs")]
mod platform;
mod render;
mod source;

use std::{
    collections::HashMap,
    ffi::CStr,
    os::raw::c_char,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use bevy::{
    prelude::*,
    render::{
        RenderApp,
        renderer::{RenderAdapter, RenderDevice},
    },
};
use common::structs::AudioDecoderError;
use dcl_component::proto_components::sdk::components::VideoState;
use kira::{sound::streaming::StreamingSoundData, tween::Tween};
use tokio::sync::{mpsc, oneshot};

pub use audio::AudioHandle;
pub use source::resolve_source;

use crate::{AVCommand, VideoData, VideoFrame, VideoInfo};
use audio::UuavKiraDecoder;
use ffi::PlayerId;

/// The protocols the helper's demuxer may open: network streaming only, no local files.
const PROTOCOL_WHITELIST: &CStr = c"https,http,tls,tcp,crypto,data,udp,rtp,rtcp,rtsp";
/// FFmpeg's `AV_LOG_WARNING`.
const LOG_LEVEL: i32 = 24;
/// Frames to retry a busy render device before giving up on the player runtime.
const INIT_ATTEMPTS: u32 = 600;

pub struct UuavPlugin;

impl Plugin for UuavPlugin {
    fn build(&self, app: &mut App) {
        let shared = Arc::new(Shared::default());
        app.insert_resource(UuavRuntime::new(shared.clone()))
            .add_systems(PreUpdate, serve_uuav_players);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render::build(render_app, shared);
        } else {
            warn!("no render app: uuav video frames will not be presented");
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render::finish(render_app);
        }
    }
}

/// Stops the helper; for the end of the process, after the app has run.
pub fn uuav_shutdown() {
    uuav::uuav_deinit();
}

/// A video player's target image and how many frames the render world has presented into it.
pub(crate) struct Target {
    pub image: AssetId<Image>,
    pub presented: AtomicU64,
}

/// What the main and render worlds share: the video players to present.
#[derive(Default)]
pub(crate) struct Shared {
    pub targets: Mutex<HashMap<PlayerId, Arc<Target>>>,
}

/// A source the `av` crate opened, waiting for the runtime's next look.
struct NewSource {
    commands: mpsc::UnboundedReceiver<AVCommand>,
    video: Option<mpsc::Sender<VideoData>>,
    url: String,
    image: AssetId<Image>,
    audio: mpsc::Sender<StreamingSoundData<AudioDecoderError>>,
    audio_handle: oneshot::Receiver<AudioHandle>,
}

static NEW_SOURCES: Mutex<Vec<NewSource>> = Mutex::new(Vec::new());

/// Plays `url` on a uuav player, rendering its frames into `image`, driven by `commands` and
/// reporting on `video` (`None` for an audio stream). The player's sound goes out on `audio`
/// for the audio sink to play; the sink hands the playing sound's handle back on
/// `audio_handle`, which is where the volume commands land.
pub fn spawn_av(
    commands: mpsc::UnboundedReceiver<AVCommand>,
    video: Option<mpsc::Sender<VideoData>>,
    url: String,
    image: &Handle<Image>,
    audio: mpsc::Sender<StreamingSoundData<AudioDecoderError>>,
    audio_handle: oneshot::Receiver<AudioHandle>,
) {
    NEW_SOURCES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(NewSource {
            commands,
            video,
            url,
            image: image.id(),
            audio,
            audio_handle,
        });
}

enum Runtime {
    Pending { attempts: u32 },
    Ready,
    Failed,
}

#[derive(Resource)]
struct UuavRuntime {
    runtime: Runtime,
    players: Vec<Player>,
    shared: Arc<Shared>,
}

impl UuavRuntime {
    fn new(shared: Arc<Shared>) -> Self {
        Self {
            runtime: Runtime::Pending { attempts: 0 },
            players: Vec::new(),
            shared,
        }
    }

    /// Starts the helper once a render device is there to derive the decode device from.
    fn initialize(&mut self, device: Option<&RenderDevice>, adapter: Option<&RenderAdapter>) {
        let Runtime::Pending { attempts } = &mut self.runtime else {
            return;
        };
        if uuav::uuav_status().initialized {
            self.runtime = Runtime::Ready;
            return;
        }
        let (Some(device), Some(adapter)) = (device, adapter) else {
            return;
        };
        let probe = match platform::probe(device, adapter) {
            Ok(probe) => probe,
            Err(e) => {
                *attempts += 1;
                if *attempts >= INIT_ATTEMPTS {
                    error!("uuav: no decode device probe after {INIT_ATTEMPTS} frames: {e}");
                    self.runtime = Runtime::Failed;
                } else {
                    debug!("uuav: probe not ready: {e}");
                }
                return;
            }
        };
        let result = unsafe {
            uuav::uuav_init(
                probe.ptr(),
                uuav::AudioOptionsRaw {
                    sample_rate: audio::SAMPLE_RATE as i32,
                    channels: audio::CHANNELS as i32,
                },
                Some(on_error),
                Some(on_warning),
                Some(on_log),
                PROTOCOL_WHITELIST.as_ptr(),
                LOG_LEVEL,
            )
        };
        drop(probe);
        match ffi::consume(result) {
            Ok(()) => {
                info!("uuav: player runtime started");
                self.runtime = Runtime::Ready;
            }
            Err(e) => {
                error!("uuav: player runtime failed to start: {e}");
                self.runtime = Runtime::Failed;
            }
        }
    }

    fn adopt(&mut self, source: NewSource, has_renderer: bool) {
        match self.runtime {
            Runtime::Ready => {}
            Runtime::Pending { .. } if has_renderer => {
                // the runtime is still coming up; look again next frame
                NEW_SOURCES
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(source);
                return;
            }
            _ => {
                debug!("uuav: no player runtime, `{}` will not play", source.url);
                send(source.video.as_ref(), VideoData::State(VideoState::VsError));
                return;
            }
        }

        let id = match ffi::player_new() {
            Ok(id) => id,
            Err(e) => {
                warn!("uuav: player for `{}` not created: {e}", source.url);
                send(source.video.as_ref(), VideoData::State(VideoState::VsError));
                return;
            }
        };
        debug!("uuav player {id}: open `{}`", source.url);
        if let Err(e) = ffi::open_media(id, &source.url) {
            warn!("uuav player {id}: open `{}` failed: {e}", source.url);
            uuav::uuav_player_free(id);
            send(source.video.as_ref(), VideoData::State(VideoState::VsError));
            return;
        }

        let alive = Arc::new(AtomicBool::new(true));
        let sound = StreamingSoundData::from_decoder(UuavKiraDecoder::new(id, alive.clone()));
        if source.audio.try_send(sound).is_err() {
            debug!("uuav player {id}: audio sink gone before the sound started");
        }

        let target = source.video.as_ref().map(|_| {
            let target = Arc::new(Target {
                image: source.image,
                presented: AtomicU64::new(0),
            });
            self.shared
                .targets
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, target.clone());
            target
        });

        self.players.push(Player {
            id,
            commands: source.commands,
            video: source.video,
            audio_handle: Some(source.audio_handle),
            handle: None,
            volume: None,
            alive,
            target,
            last_state: None,
            last_size: None,
            last_presented: 0,
        });
    }
}

struct Player {
    id: PlayerId,
    commands: mpsc::UnboundedReceiver<AVCommand>,
    video: Option<mpsc::Sender<VideoData>>,
    audio_handle: Option<oneshot::Receiver<AudioHandle>>,
    handle: Option<AudioHandle>,
    /// The last volume commanded, applied once the sound handle arrives.
    volume: Option<f32>,
    /// Cleared on close so the kira decoder ends the sound.
    alive: Arc<AtomicBool>,
    target: Option<Arc<Target>>,
    last_state: Option<VideoState>,
    last_size: Option<(u32, u32)>,
    last_presented: u64,
}

impl Player {
    /// One frame of service; `false` once the player is closed.
    fn serve(&mut self) -> bool {
        if let Some(receiver) = &mut self.audio_handle {
            match receiver.try_recv() {
                Ok(mut handle) => {
                    if let Some(volume) = self.volume {
                        handle.set_volume(volume as f64, Tween::default());
                    }
                    self.handle = Some(handle);
                    self.audio_handle = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => self.audio_handle = None,
            }
        }

        loop {
            match self.commands.try_recv() {
                Ok(AVCommand::Dispose) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.close();
                    return false;
                }
                Ok(command) => self.apply(command),
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }

        let state = match ffi::state(self.id) {
            uuav::UUAVState::UUAV_CLOSED | uuav::UUAVState::UUAV_OPENING => VideoState::VsLoading,
            uuav::UUAVState::UUAV_READY => VideoState::VsReady,
            uuav::UUAVState::UUAV_PLAYING => VideoState::VsPlaying,
            uuav::UUAVState::UUAV_PAUSED | uuav::UUAVState::UUAV_ENDED => VideoState::VsPaused,
            uuav::UUAVState::UUAV_ERROR | uuav::UUAVState::UUAV_UNKNOWN => VideoState::VsError,
        };
        if self.last_state != Some(state) {
            debug!("uuav player {}: {state:?}", self.id);
            self.last_state = Some(state);
            send(self.video.as_ref(), VideoData::State(state));
        }

        if self.video.is_some() {
            if let Some(size) = ffi::video_size(self.id)
                && self.last_size != Some(size)
            {
                self.last_size = Some(size);
                send(
                    self.video.as_ref(),
                    VideoData::Info(VideoInfo {
                        width: size.0,
                        height: size.1,
                        rate: ffi::framerate(self.id).unwrap_or(0.0),
                        // live streams have no duration
                        length: ffi::duration(self.id).unwrap_or(-1.0),
                    }),
                );
            }
            if let Some(target) = &self.target {
                let presented = target.presented.load(Ordering::Acquire);
                if presented != self.last_presented {
                    self.last_presented = presented;
                    let time = ffi::current_time(self.id).unwrap_or(0.0);
                    send(self.video.as_ref(), VideoData::Frame(VideoFrame {}, time));
                }
            }
        }
        true
    }

    fn apply(&mut self, command: AVCommand) {
        let id = self.id;
        let result = match command {
            AVCommand::Play => ffi::consume(uuav::uuav_player_play(id)),
            AVCommand::Pause => ffi::consume(uuav::uuav_player_pause(id)),
            AVCommand::Repeat(repeat) => {
                ffi::consume(uuav::uuav_player_set_looping(id, u8::from(repeat)))
            }
            AVCommand::Seek(time) => {
                // the real state is reported again after the seek
                self.last_state = None;
                send(self.video.as_ref(), VideoData::State(VideoState::VsSeeking));
                ffi::consume(uuav::uuav_player_seek_async(id, time))
            }
            AVCommand::Volume(volume) => {
                self.volume = Some(volume);
                if let Some(handle) = &mut self.handle {
                    handle.set_volume(volume as f64, Tween::default());
                }
                Ok(())
            }
            AVCommand::Dispose => Ok(()),
        };
        if let Err(e) = result {
            warn!("uuav player {id}: command failed: {e}");
        }
    }

    fn close(&mut self) {
        debug!("uuav player {}: close", self.id);
        self.alive.store(false, Ordering::Release);
        if let Some(mut handle) = self.handle.take() {
            handle.stop(Tween::default());
        }
        uuav::uuav_player_free(self.id);
    }
}

fn send(video: Option<&mpsc::Sender<VideoData>>, data: VideoData) {
    if let Some(video) = video
        && video.try_send(data).is_err()
    {
        trace!("uuav: video channel full or closed");
    }
}

fn serve_uuav_players(
    mut runtime: ResMut<UuavRuntime>,
    device: Option<Res<RenderDevice>>,
    adapter: Option<Res<RenderAdapter>>,
) {
    runtime.initialize(device.as_deref(), adapter.as_deref());

    let new_sources = std::mem::take(
        &mut *NEW_SOURCES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    for source in new_sources {
        runtime.adopt(source, device.is_some());
    }

    let runtime = &mut *runtime;
    runtime.players.retain_mut(|player| {
        let keep = player.serve();
        if !keep {
            runtime
                .shared
                .targets
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&player.id);
        }
        keep
    });
}

// The client's callbacks fire on its own threads; they only log.

fn message(message: *const c_char) -> String {
    if message.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned()
}

extern "C" fn on_error(text: *const c_char) {
    error!(target: "uuav", "{}", message(text));
}

extern "C" fn on_warning(text: *const c_char) {
    warn!(target: "uuav", "{}", message(text));
}

extern "C" fn on_log(text: *const c_char) {
    debug!(target: "uuav", "{}", message(text));
}
