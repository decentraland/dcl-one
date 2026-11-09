use std::sync::OnceLock;

use common::structs::AudioDecoderError;
use cpal::traits::{DeviceTrait, HostTrait};
use kira::sound::streaming::StreamingSoundData;

use super::{ffi, ffi::PlayerId};

pub type AudioHandle = <StreamingSoundData<AudioDecoderError> as kira::sound::SoundData>::Handle;

/// kira's `Frame` is stereo; the helper converts every source to this layout.
pub const CHANNELS: usize = 2;

/// For an output device that reports no format.
const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// Longest wait for audio before the player is looked up again.
const WAIT_FOR_AUDIO_MS: u32 = 20;

/// The default output device's sample rate, the device kira's default backend opens. Read once
/// per process.
pub fn output_sample_rate() -> u32 {
    static RATE: OnceLock<u32> = OnceLock::new();
    *RATE.get_or_init(|| {
        cpal::default_host()
            .default_output_device()
            .and_then(|device| device.default_output_config().ok())
            .map_or(FALLBACK_SAMPLE_RATE, |config| config.sample_rate().0)
    })
}

/// Feeds a player's audio to kira. `decode` blocks until the player has audio, so kira never
/// receives padded silence, and ends the sound once the player is freed.
pub struct UuavKiraDecoder {
    id: PlayerId,
    sample_rate: u32,
    started: bool,
    samples_10ms: Vec<f32>,
}

impl UuavKiraDecoder {
    pub fn new(id: PlayerId, sample_rate: u32) -> Self {
        Self {
            id,
            sample_rate,
            started: false,
            samples_10ms: vec![0.0; sample_rate as usize / 100 * CHANNELS],
        }
    }
}

impl kira::sound::streaming::Decoder for UuavKiraDecoder {
    type Error = AudioDecoderError;

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn num_frames(&self) -> usize {
        // open-ended: `decode` ends the sound once the player is freed
        u32::MAX as usize
    }

    fn decode(&mut self) -> Result<Vec<kira::Frame>, Self::Error> {
        self.started = true;
        let frames = self.samples_10ms.len() / CHANNELS;
        loop {
            let copied = unsafe {
                uuav::uuav_player_read_audio(self.id, self.samples_10ms.as_mut_ptr(), frames as i32)
            };
            if copied > 0 {
                let samples = (copied as usize).min(frames) * CHANNELS;
                return Ok(self.samples_10ms[..samples]
                    .as_chunks::<CHANNELS>()
                    .0
                    .iter()
                    .map(|frame| kira::Frame::new(frame[0], frame[1]))
                    .collect());
            }
            if matches!(ffi::state(self.id), uuav::UUAVState::UUAV_UNKNOWN) {
                return Err(AudioDecoderError::StreamClosed);
            }
            // kira calls `decode` again as soon as it returns, so returning empty would spin a
            // core; a custom kira `Sound` reading the ring on the audio thread would need no wait
            uuav::uuav_player_wait_audio(self.id, WAIT_FOR_AUDIO_MS);
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
