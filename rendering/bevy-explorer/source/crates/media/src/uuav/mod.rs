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
    os::raw::{c_char, c_void},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
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

/// The protocols the helper's demuxer may open: http(s) over tcp and tls, nothing else. No local
/// files, and no udp, rtp or rtsp for an http body that probes as SDP to open towards hosts
/// the scene names.
const PROTOCOL_WHITELIST: &CStr = c"https,http,tls,tcp,crypto";
/// FFmpeg's `AV_LOG_WARNING`.
const LOG_LEVEL: i32 = 24;
/// Frames to retry a busy render device before giving up on the player runtime.
const INIT_ATTEMPTS: u32 = 600;
/// Waits before the runtime is started again after a failed start, by failure count; the last
/// one repeats. A source arriving after the wait starts it.
const INIT_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(10),
    Duration::from_secs(30),
];
/// How often a playing player's clock goes to the scene while no frame is presented (no target
/// image yet, or a platform without frames).
const CLOCK_INTERVAL: Duration = Duration::from_millis(250);

pub struct UuavPlugin;

impl Plugin for UuavPlugin {
    fn build(&self, app: &mut App) {
        let shared = Arc::new(Shared::default());
        app.insert_resource(UuavRuntime::new(shared.clone()))
            .add_systems(PreUpdate, serve_uuav_players);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            shared.render_world.store(true, Ordering::Release);
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `(width, height)` in one atomic: 0 is "no size yet".
pub(crate) fn pack_size(width: u32, height: u32) -> u64 {
    (u64::from(width) << 32) | u64::from(height)
}

fn unpack_size(packed: u64) -> Option<(u32, u32)> {
    (packed != 0).then_some(((packed >> 32) as u32, packed as u32))
}

/// A stream header's size when it has one: FFmpeg leaves an edge at 0 when the probe decoded
/// no frame (live HLS and RTMP, often), and the planes bring the size later.
fn known_size((width, height): (u32, u32)) -> Option<(u32, u32)> {
    (width != 0 && height != 0).then_some((width, height))
}

/// A video player's target image and how many frames the render world has presented into it.
pub(crate) struct Target {
    pub image: AssetId<Image>,
    pub presented: AtomicU64,
    /// The size of the planes the render world presents, [`pack_size`]d, once it has read them;
    /// the main world sizes the image to it.
    pub presented_size: AtomicU64,
    /// Set by the render world when the player's frames cannot be presented (a size the device
    /// refuses); it presents nothing more for the player.
    pub failed: AtomicBool,
}

/// What the main and render worlds share: the video players to present, and the closed ones
/// the render world still has to let go of.
#[derive(Default)]
pub(crate) struct Shared {
    pub targets: Mutex<HashMap<PlayerId, Arc<Target>>>,
    /// Closed players whose planes the render world may still be reading; it frees each once
    /// it has dropped its textures.
    retired: Mutex<Vec<PlayerId>>,
    /// Whether a render world presents frames: with none, a closed player is freed at once.
    render_world: AtomicBool,
    /// The render device's texture edge limit, once the render world has read it; 0 before.
    pub max_dimension: AtomicU32,
}

impl Shared {
    pub fn take_retired(&self) -> Vec<PlayerId> {
        std::mem::take(&mut *lock(&self.retired))
    }

    /// Ends a player's presentation and frees it: at once when no render world can be reading
    /// its planes, otherwise from the render world ([`take_retired`](Self::take_retired)).
    /// Returns whether the free was deferred.
    fn retire(&self, id: PlayerId) -> bool {
        let had_target = lock(&self.targets).remove(&id).is_some();
        if had_target && self.render_world.load(Ordering::Acquire) {
            lock(&self.retired).push(id);
            true
        } else {
            uuav::uuav_player_free(id);
            false
        }
    }
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
    lock(&NEW_SOURCES).push(NewSource {
        commands,
        video,
        url,
        image: image.id(),
        audio,
        audio_handle,
    });
}

/// The probe kept alive while `uuav_init` runs on another thread.
struct ProbeHold {
    _probe: platform::Probe,
}

// SAFETY: the probe is a 1x1 texture that nothing here reads or writes after creation; it is
// only kept alive and dropped. Metal and D3D11 objects are free-threaded, and the Linux probe
// holds nothing.
unsafe impl Send for ProbeHold {}
unsafe impl Sync for ProbeHold {}

enum Runtime {
    Pending {
        attempts: u32,
    },
    /// The start failed; a source arriving at or after `retry_at` starts it again.
    Failed {
        retry_at: Instant,
    },
    /// `uuav_init` runs on its own thread: it spawns the helper and waits for its handshake. The
    /// probe stays alive until the call has answered.
    Starting {
        _probe: ProbeHold,
        done: oneshot::Receiver<Result<(), String>>,
    },
    Ready,
}

/// A source whose player is being created on another thread (`uuav_player_new` round-trips to
/// the helper).
struct Creating {
    source: NewSource,
    done: oneshot::Receiver<Result<PlayerId, String>>,
}

#[derive(Resource)]
struct UuavRuntime {
    runtime: Runtime,
    /// Failed starts so far, for the retry wait.
    init_failures: u32,
    creating: Vec<Creating>,
    players: Vec<Player>,
    shared: Arc<Shared>,
}

impl UuavRuntime {
    fn new(shared: Arc<Shared>) -> Self {
        Self {
            runtime: Runtime::Pending { attempts: 0 },
            init_failures: 0,
            creating: Vec::new(),
            players: Vec::new(),
            shared,
        }
    }

    /// Starts the helper once a render device is there to derive the decode device from, and
    /// picks up the answer on a later frame.
    fn initialize(&mut self, device: Option<&RenderDevice>, adapter: Option<&RenderAdapter>) {
        match &mut self.runtime {
            Runtime::Pending { attempts } => {
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
                            self.fail_start(format!(
                                "no decode device probe after {INIT_ATTEMPTS} frames: {e}"
                            ));
                        } else {
                            debug!("uuav: probe not ready: {e}");
                        }
                        return;
                    }
                };
                // the pointer crosses as an address: the probe object itself stays here
                let probe_ptr = probe.ptr() as usize;
                let (done_tx, done) = oneshot::channel();
                let spawned = std::thread::Builder::new()
                    .name("uuav-init".to_owned())
                    .spawn(move || {
                        let result = unsafe {
                            uuav::uuav_init(
                                probe_ptr as *const c_void,
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
                        _ = done_tx.send(ffi::consume(result));
                    });
                match spawned {
                    Ok(_) => {
                        self.runtime = Runtime::Starting {
                            _probe: ProbeHold { _probe: probe },
                            done,
                        }
                    }
                    Err(e) => self.fail_start(e.to_string()),
                }
            }
            Runtime::Starting { done, .. } => match done.try_recv() {
                Ok(Ok(())) => {
                    info!("uuav: player runtime started");
                    self.init_failures = 0;
                    self.runtime = Runtime::Ready;
                }
                Ok(Err(e)) => self.fail_start(e),
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.fail_start("the init thread ended".to_owned());
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            },
            Runtime::Ready | Runtime::Failed { .. } => {}
        }
    }

    /// Parks the runtime until the retry wait for this many failures has passed.
    fn fail_start(&mut self, why: String) {
        self.init_failures += 1;
        let delay =
            INIT_RETRY_DELAYS[(self.init_failures as usize - 1).min(INIT_RETRY_DELAYS.len() - 1)];
        error!(
            "uuav: player runtime failed to start (failure {}): {why}; next try after {delay:?}",
            self.init_failures
        );
        self.runtime = Runtime::Failed {
            retry_at: Instant::now() + delay,
        };
    }

    /// Begins creating a player for the source; [`poll_creating`](Self::poll_creating) starts
    /// it once the helper has answered.
    fn adopt(&mut self, source: NewSource, has_renderer: bool) {
        if has_renderer
            && matches!(&self.runtime, Runtime::Failed { retry_at } if Instant::now() >= *retry_at)
        {
            info!(
                "uuav: starting the player runtime again for `{}`",
                source.url
            );
            self.runtime = Runtime::Pending { attempts: 0 };
        }
        match self.runtime {
            Runtime::Ready => {}
            Runtime::Pending { .. } | Runtime::Starting { .. } if has_renderer => {
                // the runtime is still coming up; look again next frame
                lock(&NEW_SOURCES).push(source);
                return;
            }
            _ => {
                debug!("uuav: no player runtime, `{}` will not play", source.url);
                send(source.video.as_ref(), VideoData::State(VideoState::VsError));
                return;
            }
        }

        let (done_tx, done) = oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("uuav-player".to_owned())
            .spawn(move || {
                _ = done_tx.send(ffi::player_new());
            });
        match spawned {
            Ok(_) => self.creating.push(Creating { source, done }),
            Err(e) => {
                warn!("uuav: player for `{}` not created: {e}", source.url);
                send(source.video.as_ref(), VideoData::State(VideoState::VsError));
            }
        }
    }

    fn poll_creating(&mut self) {
        for mut creating in std::mem::take(&mut self.creating) {
            let created = match creating.done.try_recv() {
                Ok(created) => created,
                Err(oneshot::error::TryRecvError::Empty) => {
                    self.creating.push(creating);
                    continue;
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    Err("the creation thread ended".to_owned())
                }
            };
            match created {
                Ok(id) => self.start(id, creating.source),
                Err(e) => {
                    warn!(
                        "uuav: player for `{}` not created: {e}",
                        creating.source.url
                    );
                    send(
                        creating.source.video.as_ref(),
                        VideoData::State(VideoState::VsError),
                    );
                }
            }
        }
    }

    /// Opens the source on its new player and begins serving it.
    fn start(&mut self, id: PlayerId, source: NewSource) {
        debug!("uuav player {id}: open `{}`", source.url);
        if let Err(e) = ffi::open_media(id, &source.url) {
            warn!("uuav player {id}: open `{}` failed: {e}", source.url);
            self.shared.retire(id);
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
                presented_size: AtomicU64::new(0),
                failed: AtomicBool::new(false),
            });
            lock(&self.shared.targets).insert(id, target.clone());
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
            seeking: false,
            last_info: None,
            last_presented: 0,
            last_clock: None,
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
    /// What went out last; a send the channel refused leaves these as they were, so it is
    /// tried again next frame.
    last_state: Option<VideoState>,
    /// A seek went out this frame: the helper's state is read again next frame, so the scene
    /// sees Seeking before the state the seek lands in.
    seeking: bool,
    /// (width, height, framerate, duration)
    last_info: Option<(u32, u32, f64, f64)>,
    last_presented: u64,
    last_clock: Option<Instant>,
}

impl Player {
    /// One frame of service; `false` once the player is closed.
    fn serve(&mut self, max_dimension: u32) -> bool {
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

        let mut state = match ffi::state(self.id) {
            uuav::UUAVState::UUAV_CLOSED | uuav::UUAVState::UUAV_OPENING => VideoState::VsLoading,
            uuav::UUAVState::UUAV_READY => VideoState::VsReady,
            uuav::UUAVState::UUAV_PLAYING => VideoState::VsPlaying,
            uuav::UUAVState::UUAV_PAUSED | uuav::UUAVState::UUAV_ENDED => VideoState::VsPaused,
            uuav::UUAVState::UUAV_ERROR | uuav::UUAVState::UUAV_UNKNOWN => VideoState::VsError,
        };
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.failed.load(Ordering::Acquire))
        {
            state = VideoState::VsError;
        }
        if std::mem::take(&mut self.seeking) {
            // Seeking went out with the seek; the helper answers it from next frame
        } else if self.last_state != Some(state)
            && send(self.video.as_ref(), VideoData::State(state))
        {
            debug!("uuav player {}: {state:?}", self.id);
            self.last_state = Some(state);
        }

        if self.video.is_some() {
            if let Some((width, height)) = self.size(max_dimension) {
                let rate = ffi::framerate(self.id).unwrap_or(0.0);
                // live streams have no duration
                let length = ffi::duration(self.id).unwrap_or(-1.0);
                let info = (width, height, rate, length);
                if self.last_info != Some(info)
                    && send(
                        self.video.as_ref(),
                        VideoData::Info(VideoInfo {
                            width,
                            height,
                            rate,
                            length,
                        }),
                    )
                {
                    self.last_info = Some(info);
                }
            }
            if let Some(target) = &self.target {
                let presented = target.presented.load(Ordering::Acquire);
                let clock_due = state == VideoState::VsPlaying
                    && self
                        .last_clock
                        .is_none_or(|at| at.elapsed() >= CLOCK_INTERVAL);
                if presented != self.last_presented || clock_due {
                    let time = ffi::current_time(self.id).unwrap_or(0.0);
                    if send(self.video.as_ref(), VideoData::Frame(VideoFrame {}, time)) {
                        self.last_presented = presented;
                        self.last_clock = Some(Instant::now());
                    }
                }
            }
        }
        true
    }

    /// The size the image is kept at: the planes the render world presents once it has read
    /// them, the stream header until then. A header out of bounds (the helper passes it through
    /// unchecked) fails the player instead of sizing the image.
    fn size(&self, max_dimension: u32) -> Option<(u32, u32)> {
        let target = self.target.as_ref()?;
        if let Some(presented) = unpack_size(target.presented_size.load(Ordering::Acquire)) {
            return Some(presented);
        }
        let (width, height) = ffi::video_size(self.id).and_then(known_size)?;
        match render::check_bounds(width, height, max_dimension) {
            Ok(()) => Some((width, height)),
            Err(e) => {
                if !target.failed.swap(true, Ordering::AcqRel) {
                    warn!("uuav player {}: stream header refused: {e}", self.id);
                }
                None
            }
        }
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
                if send(self.video.as_ref(), VideoData::State(VideoState::VsSeeking)) {
                    self.last_state = Some(VideoState::VsSeeking);
                }
                self.seeking = true;
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

    /// Ends the sound; the player itself is freed by [`Shared::retire`].
    fn close(&mut self) {
        debug!("uuav player {}: close", self.id);
        self.alive.store(false, Ordering::Release);
        if let Some(mut handle) = self.handle.take() {
            handle.stop(Tween::default());
        }
    }
}

/// Whether the data went out. A player without a video channel has nowhere to send and counts
/// as sent.
fn send(video: Option<&mpsc::Sender<VideoData>>, data: VideoData) -> bool {
    let Some(video) = video else {
        return true;
    };
    match video.try_send(data) {
        Ok(()) => true,
        Err(_) => {
            trace!("uuav: video channel full or closed");
            false
        }
    }
}

fn serve_uuav_players(
    mut runtime: ResMut<UuavRuntime>,
    device: Option<Res<RenderDevice>>,
    adapter: Option<Res<RenderAdapter>>,
) {
    runtime.initialize(device.as_deref(), adapter.as_deref());

    let new_sources = std::mem::take(&mut *lock(&NEW_SOURCES));
    for source in new_sources {
        runtime.adopt(source, device.is_some());
    }
    runtime.poll_creating();

    let runtime = &mut *runtime;
    let shared = &runtime.shared;
    let max_dimension = shared.max_dimension.load(Ordering::Acquire);
    runtime.players.retain_mut(|player| {
        let keep = player.serve(max_dimension);
        if !keep {
            shared.retire(player.id);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Arc<Target> {
        Arc::new(Target {
            image: AssetId::default(),
            presented: AtomicU64::new(0),
            presented_size: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        })
    }

    #[test]
    fn a_header_with_a_zero_edge_has_no_size_yet() {
        assert_eq!(known_size((0, 0)), None);
        assert_eq!(known_size((1920, 0)), None);
        assert_eq!(known_size((0, 1080)), None);
        assert_eq!(known_size((1920, 1080)), Some((1920, 1080)));
    }

    #[test]
    fn a_size_round_trips_and_zero_is_none() {
        assert_eq!(unpack_size(pack_size(852, 480)), Some((852, 480)));
        assert_eq!(unpack_size(pack_size(4096, 4096)), Some((4096, 4096)));
        assert_eq!(unpack_size(0), None);
    }

    #[test]
    fn without_a_render_world_a_closed_player_is_freed_at_once() {
        let shared = Shared::default();
        lock(&shared.targets).insert(7, target());
        assert!(!shared.retire(7));
        assert!(lock(&shared.targets).is_empty());
        assert!(shared.take_retired().is_empty());
    }

    #[test]
    fn under_a_render_world_the_free_waits_for_it() {
        let shared = Shared::default();
        shared.render_world.store(true, Ordering::Release);
        lock(&shared.targets).insert(7, target());
        assert!(shared.retire(7));
        assert!(lock(&shared.targets).is_empty());
        assert_eq!(shared.take_retired(), vec![7]);
        assert!(shared.take_retired().is_empty());
    }

    #[test]
    fn a_player_without_frames_is_freed_at_once() {
        let shared = Shared::default();
        shared.render_world.store(true, Ordering::Release);
        assert!(!shared.retire(9));
        assert!(shared.take_retired().is_empty());
    }
}
