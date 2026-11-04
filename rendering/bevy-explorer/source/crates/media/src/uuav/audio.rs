use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use common::structs::AudioDecoderError;
use kira::sound::streaming::StreamingSoundData;

use super::ffi::PlayerId;

pub type AudioHandle = <StreamingSoundData<AudioDecoderError> as kira::sound::SoundData>::Handle;

/// The sample rate and channel count negotiated with the helper at init; every player's audio
/// arrives in this format.
pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;

/// Frames pulled per decode call: 10 ms, the helper's own pump cadence.
const PULL_FRAMES: usize = 480;

/// kira polls the decoder in a tight loop while its buffer has room; a read that found nothing
/// waits this long before trying again.
const EMPTY_WAIT: Duration = Duration::from_millis(2);

/// Pulls a player's audio out of the uuav client's jitter ring for kira.
///
/// The helper releases audio at the media clock and the ring pads silence when read ahead of it,
/// so a decode that would return nothing blocks until real frames arrive: that paces kira at
/// realtime delivery instead of letting it drain the ring into silence.
pub struct UuavKiraDecoder {
    id: PlayerId,
    /// Cleared when the player is disposed; the next decode ends the sound.
    alive: Arc<AtomicBool>,
    started: bool,
    buffer: Vec<f32>,
}

impl UuavKiraDecoder {
    pub fn new(id: PlayerId, alive: Arc<AtomicBool>) -> Self {
        Self {
            id,
            alive,
            started: false,
            buffer: vec![0.0; PULL_FRAMES * CHANNELS],
        }
    }
}

impl kira::sound::streaming::Decoder for UuavKiraDecoder {
    type Error = AudioDecoderError;

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn num_frames(&self) -> usize {
        // open-ended: the sound is stopped explicitly when the player is disposed
        u32::MAX as usize
    }

    fn decode(&mut self) -> Result<Vec<kira::Frame>, Self::Error> {
        self.started = true;
        loop {
            if !self.alive.load(Ordering::Acquire) {
                return Err(AudioDecoderError::StreamClosed);
            }
            // never blocks; returns the frames actually copied and pads the rest with silence
            let copied = unsafe {
                uuav::uuav_player_read_audio(self.id, self.buffer.as_mut_ptr(), PULL_FRAMES as i32)
            };
            if copied > 0 {
                let samples = (copied as usize).min(PULL_FRAMES) * CHANNELS;
                return Ok(self.buffer[..samples]
                    .chunks_exact(CHANNELS)
                    .map(|frame| kira::Frame::new(frame[0], frame[1]))
                    .collect());
            }
            std::thread::sleep(EMPTY_WAIT);
        }
    }

    fn seek(&mut self, seek: usize) -> Result<usize, Self::Error> {
        if !self.started && seek == 0 {
            return Ok(0);
        }
        Err(AudioDecoderError::Other(format!(
            "Can't seek (requested {seek})"
        )))
    }
}
