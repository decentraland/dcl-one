//! One video player as the main world serves it: commands in, state, info and clock out.

use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use bevy::prelude::*;
use common::util::watch_send_if_modified;
use dcl_component::proto_components::sdk::components::VideoState;
use kira::tween::Tween;
use tokio::sync::{mpsc, oneshot, watch};

use super::{
    AudioHandle, NewSource, ffi,
    ffi::PlayerId,
    presentation::Target,
    size::{check_bounds, known_size, unpack_size},
};
use crate::{AVCommand, VideoData, VideoInfo};

/// How often a playing player's clock goes to the scene while no frame is presented (no target
/// image yet).
const CLOCK_INTERVAL: Duration = Duration::from_millis(250);

pub(super) struct Player {
    pub id: PlayerId,
    commands: mpsc::UnboundedReceiver<AVCommand>,
    video: Option<mpsc::Sender<VideoData>>,
    /// The playback state; the watch keeps the latest value.
    state: watch::Sender<VideoState>,
    audio_handle: Option<oneshot::Receiver<AudioHandle>>,
    handle: Option<AudioHandle>,
    /// The last volume commanded, applied once the sound handle arrives.
    volume: Option<f32>,
    /// Cleared on close.
    target: Option<Arc<Target>>,
    /// A seek went out this frame: the helper's state is read again next frame, so Seeking is
    /// sent before the state the seek lands in.
    seeking: bool,
    /// (width, height, framerate, duration); a send the channel refused leaves it as it was,
    /// so it is tried again next frame.
    last_info: Option<(u32, u32, f64, f64)>,
    last_presented: u64,
    last_clock: Option<Instant>,
}

impl Player {
    pub(super) fn new(id: PlayerId, source: NewSource, target: Option<Arc<Target>>) -> Self {
        Self {
            id,
            commands: source.commands,
            video: source.video,
            state: source.state,
            audio_handle: Some(source.audio_handle),
            handle: None,
            volume: None,
            target,
            seeking: false,
            last_info: None,
            last_presented: 0,
            last_clock: None,
        }
    }

    /// One frame of service; `false` once the player is closed.
    pub(super) fn serve(&mut self, max_dimension: u32) -> bool {
        self.take_audio_handle();
        if !self.apply_commands() {
            self.close();
            return false;
        }
        let state = self.publish_state();
        if self.video.is_some() {
            self.publish_info(max_dimension);
            self.publish_clock(state);
        }
        true
    }

    /// Picks up the playing sound's handle once it arrives and gives it the last volume.
    fn take_audio_handle(&mut self) {
        let Some(receiver) = &mut self.audio_handle else {
            return;
        };
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

    /// Applies the queued commands; `false` once the player is disposed.
    fn apply_commands(&mut self) -> bool {
        loop {
            match self.commands.try_recv() {
                Ok(AVCommand::Dispose) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    return false;
                }
                Ok(command) => self.apply(command),
                Err(mpsc::error::TryRecvError::Empty) => return true,
            }
        }
    }

    /// Sends the playback state when it changed, and returns it.
    fn publish_state(&mut self) -> VideoState {
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
        } else if watch_send_if_modified(&self.state, state) {
            debug!("uuav player {}: {state:?}", self.id);
        }
        state
    }

    /// Sends the video's size, frame rate and duration when they changed.
    fn publish_info(&mut self, max_dimension: u32) {
        let Some((width, height)) = self.size(max_dimension) else {
            return;
        };
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

    /// Sends the media time after each presented frame, and every [`CLOCK_INTERVAL`] while
    /// playing.
    fn publish_clock(&mut self, state: VideoState) {
        let Some(target) = &self.target else {
            return;
        };
        let presented = target.presented.load(Ordering::Acquire);
        let clock_due = state == VideoState::VsPlaying
            && self
                .last_clock
                .is_none_or(|at| at.elapsed() >= CLOCK_INTERVAL);
        if presented != self.last_presented || clock_due {
            let time = ffi::current_time(self.id).unwrap_or(0.0);
            if send(self.video.as_ref(), VideoData::Frame(time)) {
                self.last_presented = presented;
                self.last_clock = Some(Instant::now());
            }
        }
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
        match check_bounds(width, height, max_dimension) {
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
                watch_send_if_modified(&self.state, VideoState::VsSeeking);
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

    /// Ends the sound; the player itself is freed by [`VideoPresentation::retire`].
    fn close(&mut self) {
        debug!("uuav player {}: close", self.id);
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
