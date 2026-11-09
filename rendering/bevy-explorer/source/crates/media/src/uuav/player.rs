//! One player as the main world serves it: commands in, state, info and clock out.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use common::util::watch_send_if_modified;
use dcl_component::proto_components::sdk::components::VideoState;
use kira::tween::Tween;
use tokio::sync::{mpsc, watch};

use super::{
    audio::AudioHandle,
    ffi,
    ffi::PlayerId,
    link::{FromRender, MainLink},
    size::{check_bounds, known_size},
};
use crate::{AVCommand, VideoData, VideoInfo};

/// How often a playing player's clock goes to the scene while no frame is presented.
const CLOCK_INTERVAL: Duration = Duration::from_millis(250);

/// The ends of a player the engine holds the other side of.
pub(super) struct Channels {
    pub commands: mpsc::UnboundedReceiver<AVCommand>,
    /// `None` for an audio stream.
    pub video: Option<mpsc::Sender<VideoData>>,
    /// The playback state; the watch keeps the latest value.
    pub state: watch::Sender<VideoState>,
}

/// What the main world knows about a video player's frames.
#[derive(Default)]
pub(super) struct Presentation {
    /// Whether a render world was asked to present them. It then also frees the player, after
    /// every message sent before.
    pub render_world: bool,
    /// The size of the planes the frames come in, once the render world has read them.
    pub size: Option<(u32, u32)>,
    pub failed: bool,
    /// A frame was presented since the clock last went out.
    pub new_frame: bool,
}

pub(super) struct Player {
    pub id: PlayerId,
    channels: Channels,
    sound: Option<AudioHandle>,
    /// `None` for a player without a video channel or image.
    presentation: Option<Presentation>,
    /// A seek went out this frame: the helper's state is read again next frame, so Seeking is
    /// sent before the state the seek lands in.
    seeking: bool,
    /// (width, height, framerate, duration); a send the channel refused leaves it as it was,
    /// so it is tried again next frame.
    last_info: Option<(u32, u32, f64, f64)>,
    last_clock: Option<Instant>,
}

impl Player {
    pub(super) fn new(
        id: PlayerId,
        channels: Channels,
        sound: Option<AudioHandle>,
        presentation: Option<Presentation>,
    ) -> Self {
        Self {
            id,
            channels,
            sound,
            presentation,
            seeking: false,
            last_info: None,
            last_clock: None,
        }
    }

    /// Applies what the render world reported about this player.
    pub(super) fn on_render(&mut self, message: FromRender) {
        let Some(presentation) = &mut self.presentation else {
            return;
        };
        match message {
            FromRender::Size { width, height, .. } => presentation.size = Some((width, height)),
            FromRender::Presented(_) => presentation.new_frame = true,
            FromRender::Failed(_) => presentation.failed = true,
        }
    }

    /// One frame of service; `false` once the player is disposed.
    pub(super) fn serve(&mut self, link: &MainLink, max_dimension: u32) -> bool {
        if !self.apply_commands() {
            return false;
        }
        let state = self.publish_state();
        if self.channels.video.is_some() {
            self.publish_info(link, max_dimension);
            self.publish_clock(state);
        }
        true
    }

    /// Ends the sound and frees the player.
    pub(super) fn close(mut self, link: &MainLink) {
        debug!("uuav player {}: close", self.id);
        if let Some(sound) = &mut self.sound {
            sound.stop(Tween::default());
        }
        let render_world = self.presentation.as_ref().is_some_and(|p| p.render_world);
        link.free(self.id, render_world);
    }

    /// Applies the queued commands; `false` once the player is disposed.
    fn apply_commands(&mut self) -> bool {
        loop {
            match self.channels.commands.try_recv() {
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
        if self.presentation.as_ref().is_some_and(|p| p.failed) {
            state = VideoState::VsError;
        }
        if std::mem::take(&mut self.seeking) {
            // Seeking went out with the seek; the helper answers it from next frame
        } else if watch_send_if_modified(&self.channels.state, state) {
            debug!("uuav player {}: {state:?}", self.id);
        }
        state
    }

    /// Sends the video's size, frame rate and duration when they changed.
    fn publish_info(&mut self, link: &MainLink, max_dimension: u32) {
        let Some((width, height)) = self.size(link, max_dimension) else {
            return;
        };
        let rate = ffi::framerate(self.id).unwrap_or(0.0);
        // live streams have no duration
        let length = ffi::duration(self.id).unwrap_or(-1.0);
        let info = (width, height, rate, length);
        if self.last_info != Some(info)
            && self.send(VideoData::Info(VideoInfo {
                width,
                height,
                rate,
                length,
            }))
        {
            self.last_info = Some(info);
        }
    }

    /// Sends the media time after each presented frame, and every [`CLOCK_INTERVAL`] while
    /// playing.
    fn publish_clock(&mut self, state: VideoState) {
        let Some(presentation) = &self.presentation else {
            return;
        };
        let clock_due = state == VideoState::VsPlaying
            && self
                .last_clock
                .is_none_or(|at| at.elapsed() >= CLOCK_INTERVAL);
        if presentation.new_frame || clock_due {
            let time = ffi::current_time(self.id).unwrap_or(0.0);
            if self.send(VideoData::Frame(time)) {
                self.last_clock = Some(Instant::now());
                if let Some(presentation) = &mut self.presentation {
                    presentation.new_frame = false;
                }
            }
        }
    }

    /// The size the image is kept at: the planes' size once the render world has read them, the
    /// stream header's until then. A header out of bounds (the helper passes it through
    /// unchecked) fails the player and stops its presentation.
    fn size(&mut self, link: &MainLink, max_dimension: u32) -> Option<(u32, u32)> {
        let presentation = self.presentation.as_mut()?;
        if presentation.size.is_some() {
            return presentation.size;
        }
        if presentation.failed {
            return None;
        }
        let (width, height) = ffi::video_size(self.id).and_then(known_size)?;
        match check_bounds(width, height, max_dimension) {
            Ok(()) => Some((width, height)),
            Err(e) => {
                warn!("uuav player {}: stream header refused: {e}", self.id);
                presentation.failed = true;
                if presentation.render_world {
                    link.stop(self.id);
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
                watch_send_if_modified(&self.channels.state, VideoState::VsSeeking);
                self.seeking = true;
                ffi::consume(uuav::uuav_player_seek_async(id, time))
            }
            AVCommand::Volume(volume) => {
                if let Some(sound) = &mut self.sound {
                    sound.set_volume(volume as f64, Tween::default());
                }
                Ok(())
            }
            AVCommand::Dispose => Ok(()),
        };
        if let Err(e) = result {
            warn!("uuav player {id}: command failed: {e}");
        }
    }

    /// Whether the data went out. A player without a video channel has nowhere to send and counts
    /// as sent.
    fn send(&self, data: VideoData) -> bool {
        let Some(video) = &self.channels.video else {
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
}
