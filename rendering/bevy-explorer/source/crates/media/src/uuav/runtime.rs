//! The player runtime: starts the helper once a render device exists, retries a failed start,
//! and creates a player for every new source.

use std::{
    ffi::CStr,
    os::raw::{c_char, c_void},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
    time::{Duration, Instant},
};

#[cfg(windows)]
use bevy::render::renderer::RenderAdapter;
use bevy::{prelude::*, render::renderer::RenderDevice};
use dcl_component::proto_components::sdk::components::VideoState;
use kira::sound::streaming::StreamingSoundData;
use tokio::sync::oneshot;

use super::{
    NEW_SOURCES, NewSource, audio,
    audio::UuavKiraDecoder,
    ffi,
    ffi::PlayerId,
    platform,
    player::Player,
    presentation::{Target, VideoPresentation},
};

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
pub(super) struct UuavRuntime {
    runtime: Runtime,
    /// Failed starts so far, for the retry wait.
    init_failures: u32,
    requested_sample_rate: u32,
    creating: Vec<Creating>,
    pub players: Vec<Player>,
    pub presentation: Arc<VideoPresentation>,
}

impl UuavRuntime {
    pub(super) fn new(presentation: Arc<VideoPresentation>) -> Self {
        Self {
            runtime: Runtime::Pending { attempts: 0 },
            init_failures: 0,
            requested_sample_rate: audio::output_sample_rate(),
            creating: Vec::new(),
            players: Vec::new(),
            presentation,
        }
    }

    /// Starts the helper once a render device is there to derive the decode device from, and
    /// picks up the answer on a later frame.
    pub(super) fn initialize(
        &mut self,
        device: Option<&RenderDevice>,
        #[cfg(windows)] adapter: Option<&RenderAdapter>,
    ) {
        match &mut self.runtime {
            Runtime::Pending { attempts } => {
                if uuav::uuav_status().initialized {
                    self.runtime = Runtime::Ready;
                    return;
                }
                // both are `None` until the renderer is up: Bevy inserts the device and the
                // adapter together, once the GPU is chosen
                #[cfg(not(windows))]
                let Some(device) = device else {
                    return;
                };
                #[cfg(windows)]
                let (Some(device), Some(adapter)) = (device, adapter) else {
                    return;
                };
                #[cfg(target_os = "macos")]
                let probe = platform::Probe::new(device);
                // the client takes a null probe on Linux: nothing to fail
                #[cfg(target_os = "linux")]
                let probe = Ok::<_, &str>(platform::Probe);
                #[cfg(windows)]
                let probe = platform::Probe::new(device, adapter);
                let probe = match probe {
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
                self.spawn_init(probe);
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

    /// Runs `uuav_init` on its own thread; the probe stays alive until it has answered.
    fn spawn_init(&mut self, probe: platform::Probe) {
        // the pointer crosses as an address: the probe object itself stays here
        let probe_ptr = probe.ptr() as usize;
        let sample_rate = self.requested_sample_rate;
        let (done_tx, done) = oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("uuav-init".to_owned())
            .spawn(move || {
                let result = unsafe {
                    uuav::uuav_init(
                        probe_ptr as *const c_void,
                        uuav::AudioOptionsRaw {
                            sample_rate: sample_rate as i32,
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
    pub(super) fn adopt(&mut self, source: NewSource, has_renderer: bool) {
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
                NEW_SOURCES.lock().push(source);
                return;
            }
            _ => {
                debug!("uuav: no player runtime, `{}` will not play", source.url);
                source.state.send_replace(VideoState::VsError);
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
                source.state.send_replace(VideoState::VsError);
            }
        }
    }

    pub(super) fn poll_creating(&mut self) {
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
                    creating.source.state.send_replace(VideoState::VsError);
                }
            }
        }
    }

    /// Opens the source on its new player and begins serving it.
    fn start(&mut self, id: PlayerId, source: NewSource) {
        debug!("uuav player {id}: open `{}`", source.url);
        if let Err(e) = ffi::open_media(id, &source.url) {
            warn!("uuav player {id}: open `{}` failed: {e}", source.url);
            self.presentation.retire(id);
            source.state.send_replace(VideoState::VsError);
            return;
        }

        let sound =
            StreamingSoundData::from_decoder(UuavKiraDecoder::new(id, self.requested_sample_rate));
        if source.audio.try_send(sound).is_err() {
            debug!("uuav player {id}: audio sink gone before the sound started");
        }

        let target = source.video.as_ref().and(source.image).map(|image| {
            let target = Arc::new(Target {
                image,
                presented: AtomicU64::new(0),
                presented_size: AtomicU64::new(0),
                failed: AtomicBool::new(false),
            });
            self.presentation.targets.lock().insert(id, target.clone());
            target
        });

        self.players.push(Player::new(id, source, target));
    }
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
