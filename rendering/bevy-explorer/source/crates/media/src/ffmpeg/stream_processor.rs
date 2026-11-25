use anyhow::bail;
use bevy::log::{debug, info, trace};
use common::util::watch_send_if_modified;
use dcl_component::proto_components::sdk::components::VideoState;
use ffmpeg_next::Packet;
use kira::tween::Tween;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::error::TryRecvError;

use crate::{
    AVCommand,
    ffmpeg::{
        AudioHandle,
        util::{BUFFER_TIME, PacketIter},
    },
};

/// The stream's audio output: the kira sound the engine spawns once the stream's sound data
/// reaches it, handed here so the backend applies `AVCommand::Volume` itself.
pub struct AudioOutput {
    handle: tokio::sync::oneshot::Receiver<AudioHandle>,
    sound: Option<AudioHandle>,
    volume: f32,
}

impl AudioOutput {
    pub fn new(handle: tokio::sync::oneshot::Receiver<AudioHandle>) -> Self {
        Self {
            handle,
            sound: None,
            volume: 1.0,
        }
    }

    fn poll(&mut self) {
        if self.sound.is_none()
            && let Ok(mut sound) = self.handle.try_recv()
        {
            sound.set_volume(self.volume as f64, Tween::default());
            self.sound = Some(sound);
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        if let Some(sound) = &mut self.sound {
            sound.set_volume(volume as f64, Tween::default());
        }
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        if let Some(sound) = &mut self.sound {
            sound.stop(Tween::default());
        }
    }
}

pub trait FfmpegContext {
    fn is_live(&self) -> bool;
    fn stream_index(&self) -> Option<usize>;
    fn buffered_time(&self) -> f64;
    fn receive_packet(&mut self, packet: Packet) -> Result<(), anyhow::Error>;
    fn send_frame(&mut self);
    fn set_start_frame(&mut self);
    fn reset_start_frame(&mut self);
    fn seconds_till_next_frame(&self) -> f64;
    fn clear(&mut self);
}

// pumps packets through stream contexts keeping them in sync
pub fn process_streams(
    mut input_context: impl PacketIter,
    streams: &mut [&mut dyn FfmpegContext],
    mut commands: tokio::sync::mpsc::UnboundedReceiver<AVCommand>,
    mut audio: AudioOutput,
    state_sender: tokio::sync::watch::Sender<VideoState>,
) -> Result<(), anyhow::Error> {
    let mut start_instant: Option<Instant> = None;
    // Reading EOF can precede playback EOF while decoded frames remain buffered.
    let mut play_requested = false;
    let mut repeat = false;
    // only repeat if the last pass produced data, else a dead or empty input spins on reset
    let mut read_since_reset = false;
    let mut init = false;

    let mut tick = 0;

    loop {
        trace!("Process stream");
        audio.poll();
        // check if all receivers were dropped
        if streams.iter().all(|ctx| !ctx.is_live()) {
            bail!("all streams disconnected without dispose command");
        }

        // ensure frame available
        if !input_context.is_eof() && streams.iter().any(|ctx| ctx.buffered_time() == 0.0) {
            trace!("Buffering stream");
            watch_send_if_modified(&state_sender, VideoState::VsBuffering);
            while !input_context.is_eof() && streams.iter().any(|ctx| ctx.buffered_time() == 0.0) {
                if let Some((stream_index, packet)) = input_context.blocking_next() {
                    read_since_reset = true;
                    for stream in streams.iter_mut() {
                        if Some(stream_index) == stream.stream_index() {
                            stream.receive_packet(packet)?;
                            break; // for
                        }
                    }
                }
            }
        }

        // state ready if required
        if !init {
            trace!("Init stream");
            watch_send_if_modified(&state_sender, VideoState::VsReady);
            init = true;
        }

        if input_context.is_eof() {
            trace!("End of stream");
            // eof
            if repeat && read_since_reset {
                // a disposed player's reads fail straight to eof; don't reload for it
                if streams.iter().all(|ctx| !ctx.is_live()) {
                    bail!("all streams disconnected without dispose command");
                }
                input_context.reset();
                read_since_reset = false;
                for stream in streams.iter_mut() {
                    stream.reset_start_frame();
                }
                continue;
            } else if streams.iter().all(|ctx| ctx.buffered_time() == 0.0) {
                info!("eof");
                for stream in streams.iter() {
                    info!("stream: {}", stream.buffered_time());
                }
                start_instant = None;
            }
        }

        let cmd = if start_instant.is_some() {
            commands.try_recv()
        } else {
            trace!("Blocking on command channel.");
            commands.blocking_recv().ok_or(TryRecvError::Disconnected)
        };

        match cmd {
            Ok(AVCommand::Play) => {
                trace!("Play command.");
                play_requested = true;
                if start_instant.is_none()
                    && (!input_context.is_eof()
                        || streams.iter().any(|ctx| ctx.buffered_time() > 0.0))
                {
                    start_instant = Some(Instant::now());
                    for stream in streams.iter_mut() {
                        stream.set_start_frame();
                    }
                }
            }
            Ok(AVCommand::Pause) => {
                trace!("Pause command.");
                play_requested = false;
                start_instant = None;
            }
            Ok(AVCommand::Repeat(r)) => {
                trace!("Repeat command.");
                repeat = r;
            }
            Ok(AVCommand::Seek(time)) => {
                trace!("Seek command.");
                for stream in streams.iter_mut() {
                    stream.clear();
                }
                input_context.seek_to(time);
                if play_requested {
                    start_instant = Some(Instant::now());
                    for stream in streams.iter_mut() {
                        stream.set_start_frame();
                    }
                }
                watch_send_if_modified(&state_sender, VideoState::VsSeeking);
                continue;
            }
            Ok(AVCommand::Volume(volume)) => {
                trace!("Volume command.");
                audio.set_volume(volume);
            }
            Ok(AVCommand::Dispose) => {
                trace!("Dispose stream.");
                return Ok(());
            }
            Err(TryRecvError::Empty) => {
                trace!("Empty command channel.");
            }
            Err(TryRecvError::Disconnected) => {
                trace!("Command channel disconnected.");
                return Ok(());
            }
        }

        if start_instant.is_some() {
            watch_send_if_modified(&state_sender, VideoState::VsPlaying);
        } else {
            watch_send_if_modified(&state_sender, VideoState::VsPaused);
        }

        if let Some(play_instant) = start_instant {
            let (next_index, next_frame_time) = streams.iter().enumerate().fold(
                (None, f64::MAX),
                |(context_index, min), (ix, context)| {
                    let ctx_time = context.seconds_till_next_frame();
                    if ctx_time < min {
                        (Some(ix), ctx_time)
                    } else {
                        (context_index, min)
                    }
                },
            );
            let now = Instant::now();
            let next_frame_time = play_instant + Duration::from_secs_f64(next_frame_time);

            if tick % 25 == 0 {
                trace!(
                    "[{:?}] next frame time: {next_frame_time:?}/ now: {now:?}",
                    std::thread::current().id()
                );
            }
            tick += 1;
            let buffer_till_time = next_frame_time - Duration::from_millis(10);
            // preload frames
            while streams.iter().any(|ctx| ctx.buffered_time() < BUFFER_TIME)
                && Instant::now() < buffer_till_time
            {
                if let Some((stream_index, packet)) = input_context.try_next() {
                    read_since_reset = true;
                    for stream in streams.iter_mut() {
                        if Some(stream_index) == stream.stream_index() {
                            stream.receive_packet(packet)?;
                            break; // for
                        }
                    }
                }
            }

            #[expect(clippy::collapsible_if)]
            if let Some(sleep_time) = next_frame_time.checked_duration_since(Instant::now()) {
                std::thread::sleep(sleep_time);
            } else if let Some(lost_time) = Instant::now().checked_duration_since(next_frame_time) {
                if lost_time > Duration::from_secs(1) {
                    // we lost time - reset start frame and instant
                    debug!("reset on loss");
                    for stream in streams.iter_mut() {
                        stream.set_start_frame();
                    }
                    start_instant = Some(now);
                }
            }

            if let Some(index) = next_index {
                let context = streams.get_mut(index).unwrap();
                context.send_frame();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Input {
        eof: bool,
    }

    impl PacketIter for Input {
        fn is_eof(&self) -> bool {
            self.eof
        }
        fn try_next(&mut self) -> Option<(usize, Packet)> {
            self.blocking_next()
        }
        fn blocking_next(&mut self) -> Option<(usize, Packet)> {
            self.eof = true;
            Some((0, Packet::empty()))
        }
        fn reset(&mut self) {
            self.eof = false;
        }
        fn seek_to(&mut self, _: f64) {
            self.eof = false;
        }
    }

    struct Stream {
        buffered: bool,
        frames: std::sync::mpsc::Sender<()>,
        commands: tokio::sync::mpsc::UnboundedSender<AVCommand>,
    }

    impl FfmpegContext for Stream {
        fn is_live(&self) -> bool {
            true
        }
        fn stream_index(&self) -> Option<usize> {
            Some(0)
        }
        fn buffered_time(&self) -> f64 {
            if self.buffered { BUFFER_TIME } else { 0.0 }
        }
        fn receive_packet(&mut self, _: Packet) -> Result<(), anyhow::Error> {
            self.buffered = true;
            Ok(())
        }
        fn send_frame(&mut self) {
            self.frames.send(()).unwrap();
            self.commands.send(AVCommand::Dispose).unwrap();
        }
        fn set_start_frame(&mut self) {}
        fn reset_start_frame(&mut self) {}
        fn seconds_till_next_frame(&self) -> f64 {
            0.0
        }
        fn clear(&mut self) {
            self.buffered = false;
        }
    }

    fn renders_after(commands: Vec<AVCommand>, buffered: bool) -> bool {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for command in commands {
            tx.send(command).unwrap();
        }
        let (frames, received) = std::sync::mpsc::channel();
        let dispose = tx.clone();
        let worker = std::thread::spawn(move || {
            let mut stream = Stream {
                buffered,
                frames,
                commands: tx,
            };
            let (_audio_tx, audio_rx) = tokio::sync::oneshot::channel();
            let (state, _state_rx) = tokio::sync::watch::channel(VideoState::VsNone);
            process_streams(
                Input { eof: true },
                &mut [&mut stream],
                rx,
                AudioOutput::new(audio_rx),
                state,
            )
            .unwrap();
        });
        let rendered = received.recv_timeout(Duration::from_secs(1)).is_ok();
        let _ = dispose.send(AVCommand::Dispose);
        worker.join().unwrap();
        rendered
    }

    #[test]
    fn a_play_before_seek_at_eof_still_renders_a_frame() {
        assert!(renders_after(
            vec![AVCommand::Play, AVCommand::Seek(0.0)],
            false
        ));
    }

    #[test]
    fn a_paused_seek_at_eof_does_not_resume_playback() {
        assert!(!renders_after(
            vec![AVCommand::Pause, AVCommand::Seek(0.0)],
            false
        ));
        assert!(!renders_after(
            vec![AVCommand::Play, AVCommand::Pause, AVCommand::Seek(0.0)],
            false
        ));
    }

    #[test]
    fn playback_resumes_buffered_frames_after_reading_eof() {
        assert!(renders_after(vec![AVCommand::Pause, AVCommand::Play], true));
    }
}
