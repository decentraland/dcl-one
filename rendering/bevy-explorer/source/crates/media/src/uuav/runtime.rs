//! The helper runtime: starts the helper once a render device exists and retries a failed
//! start.

use std::{
    ffi::CStr,
    os::raw::{c_char, c_void},
    time::{Duration, Instant},
};

#[cfg(windows)]
use bevy::render::renderer::RenderAdapter;
use bevy::{prelude::*, render::renderer::RenderDevice};
use tokio::sync::oneshot;

#[cfg(windows)]
use super::link::MainLink;
use super::{audio, ffi, platform};

/// The protocols the helper's demuxer may open: http(s) over tcp and tls, nothing else. No local
/// files, and no udp, rtp or rtsp for an http body that probes as SDP to open towards hosts
/// the scene names.
const PROTOCOL_WHITELIST: &CStr = c"https,http,tls,tcp,crypto";
/// FFmpeg's `AV_LOG_WARNING`.
const LOG_LEVEL: i32 = 24;
/// Frames to retry a busy render device before giving up on the player runtime.
const INIT_ATTEMPTS: u32 = 600;
/// Waits before the runtime is started again after a failed start, by failure count; the last
/// one repeats. A source arriving after the wait starts it. Sources keep waiting through the
/// failures before the last wait is reached.
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

enum State {
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

/// What becomes of a source the runtime is asked to play.
pub(super) enum Admission {
    Open,
    /// The runtime is still coming up.
    Wait,
    Refuse,
}

pub(super) struct HelperRuntime {
    state: State,
    /// Failed starts so far, for the retry wait.
    init_failures: u32,
    /// The rate the helper delivers audio at; read from the output device when the helper
    /// first starts.
    sample_rate: Option<u32>,
}

impl Default for HelperRuntime {
    fn default() -> Self {
        Self {
            state: State::Pending { attempts: 0 },
            init_failures: 0,
            sample_rate: None,
        }
    }
}

impl HelperRuntime {
    /// `None` before the helper first starts.
    pub fn sample_rate(&self) -> Option<u32> {
        self.sample_rate
    }

    /// Starts the helper once a render device is there to derive the decode device from, and
    /// picks up the answer on a later frame. On Windows the decode device goes to the render
    /// world through `link`.
    pub fn initialize(
        &mut self,
        device: Option<&RenderDevice>,
        #[cfg(windows)] adapter: Option<&RenderAdapter>,
        #[cfg(windows)] link: &MainLink,
    ) {
        match &mut self.state {
            State::Pending { attempts } => {
                let status = uuav::uuav_status();
                if status.initialized {
                    self.sample_rate = Some(status.audio_options.sample_rate as u32);
                    self.state = State::Ready;
                    return;
                }
                // both are `None` until the renderer is up: Bevy inserts the device and the
                // adapter together, once the GPU is chosen
                #[cfg(target_os = "macos")]
                let Some(device) = device else {
                    return;
                };
                #[cfg(target_os = "linux")]
                if device.is_none() {
                    return;
                }
                #[cfg(windows)]
                let (Some(device), Some(adapter)) = (device, adapter) else {
                    return;
                };
                #[cfg(target_os = "macos")]
                let probe = platform::Probe::new(device);
                // the client takes a null probe on Linux: nothing to fail
                #[cfg(target_os = "linux")]
                let probe = Ok::<_, &str>(platform::Probe);
                // a renderer the decode device cannot serve does not change between frames, so
                // the runtime starts at once on an audio-only probe
                #[cfg(windows)]
                let probe = platform::Probe::new(device, adapter)
                    .map(|(probe, decode_device)| {
                        link.send_decode_device(decode_device);
                        probe
                    })
                    .or_else(|e| {
                        warn!("uuav: video frames will not be shown, audio still plays: {e}");
                        platform::Probe::audio_only()
                    });
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
            State::Starting { done, .. } => match done.try_recv() {
                Ok(Ok(())) => {
                    info!("uuav: player runtime started");
                    self.init_failures = 0;
                    self.state = State::Ready;
                }
                Ok(Err(e)) => self.fail_start(e),
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.fail_start("the init thread ended".to_owned());
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            },
            State::Ready | State::Failed { .. } => {}
        }
    }

    /// Whether a source can open now. A failed runtime whose retry wait has passed starts again
    /// when a renderer is there; until the failures reach the last retry wait, the source waits
    /// for the next start.
    pub fn admit(&mut self, has_renderer: bool, url: &str) -> Admission {
        if has_renderer
            && matches!(&self.state, State::Failed { retry_at } if Instant::now() >= *retry_at)
        {
            info!("uuav: starting the player runtime again for `{url}`");
            self.state = State::Pending { attempts: 0 };
        }
        match self.state {
            State::Ready => Admission::Open,
            State::Pending { .. } | State::Starting { .. } if has_renderer => Admission::Wait,
            State::Failed { .. }
                if has_renderer && (self.init_failures as usize) < INIT_RETRY_DELAYS.len() =>
            {
                Admission::Wait
            }
            _ => Admission::Refuse,
        }
    }

    /// Runs `uuav_init` on its own thread; the probe stays alive until it has answered. After a
    /// successful start the thread stays parked for the life of the process: on Linux the helper
    /// it spawned is killed when that thread exits (`PR_SET_PDEATHSIG`).
    fn spawn_init(&mut self, probe: platform::Probe) {
        // the pointer crosses as an address: the probe object itself stays here
        let probe_ptr = probe.ptr() as usize;
        let sample_rate = *self
            .sample_rate
            .get_or_insert_with(audio::output_sample_rate);
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
                let result = ffi::consume(result);
                let started = result.is_ok();
                _ = done_tx.send(result);
                if started {
                    loop {
                        std::thread::park();
                    }
                }
            });
        match spawned {
            Ok(_) => {
                self.state = State::Starting {
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
        self.state = State::Failed {
            retry_at: Instant::now() + delay,
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(init_failures: u32) -> HelperRuntime {
        HelperRuntime {
            state: State::Failed {
                retry_at: Instant::now() + Duration::from_secs(60),
            },
            init_failures,
            sample_rate: None,
        }
    }

    #[test]
    fn a_source_waits_through_the_early_failed_starts() {
        assert!(matches!(failed(1).admit(true, "u"), Admission::Wait));
    }

    #[test]
    fn a_source_is_refused_once_the_failed_starts_reach_the_last_retry_wait() {
        let failures = INIT_RETRY_DELAYS.len() as u32;
        assert!(matches!(
            failed(failures).admit(true, "u"),
            Admission::Refuse
        ));
    }

    #[test]
    fn a_failed_runtime_without_a_renderer_refuses() {
        assert!(matches!(failed(1).admit(false, "u"), Admission::Refuse));
    }
}
