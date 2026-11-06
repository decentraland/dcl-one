//! A single-slot window of a player's decoded audio for the analysis, after unity-explorer's
//! `LastAudioFrameBuffer` (`Plugins/NativeAudioAnalysis`) and the capture `UUAVPlayer` runs in
//! its audio fill: the producer writes every chunk it decodes, never blocks, and a window the
//! consumer did not take in time is overwritten; nothing is kept while no analysis asks for it.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

/// Unity's DSP buffer (`ProjectSettings/AudioManager.asset`, `m_DSPBufferSize: 1024`): the frames
/// one analysis window holds.
pub const CAPTURE_FRAMES: usize = 1024;
/// Interleaved left and right, as Unity's `OnAudioFilterRead` buffer is.
pub const CAPTURE_CHANNELS: usize = 2;

#[derive(Default)]
pub struct AudioCapture {
    enabled: AtomicBool,
    /// Chunks the producer dropped because the slot was held.
    dropped: AtomicU32,
    slot: Mutex<Slot>,
}

struct Slot {
    /// `CAPTURE_FRAMES` interleaved frames; `write` is the oldest frame's index.
    ring: Vec<f32>,
    write: usize,
    /// Frames written since the last window was taken, saturating at the window.
    fresh: usize,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            ring: vec![0.0; CAPTURE_FRAMES * CAPTURE_CHANNELS],
            write: 0,
            fresh: 0,
        }
    }
}

impl Slot {
    fn reset(&mut self) {
        self.ring.fill(0.0);
        self.write = 0;
        self.fresh = 0;
    }
}

impl AudioCapture {
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Turns the capture on or off; either way the slot starts over empty.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        if let Ok(mut slot) = self.slot.try_lock() {
            slot.reset();
        }
    }

    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Writes `frames` into the ring; a chunk that arrives while the consumer holds the slot is
    /// dropped and counted. Nothing happens while disabled.
    pub fn push(&self, frames: &[kira::Frame]) {
        if !self.enabled() {
            return;
        }
        let Ok(mut slot) = self.slot.try_lock() else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        for frame in frames {
            let at = slot.write * CAPTURE_CHANNELS;
            slot.ring[at] = frame.left;
            slot.ring[at + 1] = frame.right;
            slot.write = (slot.write + 1) % CAPTURE_FRAMES;
        }
        slot.fresh = (slot.fresh + frames.len()).min(CAPTURE_FRAMES);
    }

    /// Copies the window into `out`, oldest frame first, when a whole window of fresh frames is
    /// there; a window is taken once. `false` while the slot is busy or not yet full.
    pub fn take_window(&self, out: &mut Vec<f32>) -> bool {
        let Ok(mut slot) = self.slot.try_lock() else {
            return false;
        };
        if slot.fresh < CAPTURE_FRAMES {
            return false;
        }
        out.clear();
        for i in 0..CAPTURE_FRAMES {
            let at = ((slot.write + i) % CAPTURE_FRAMES) * CAPTURE_CHANNELS;
            out.push(slot.ring[at]);
            out.push(slot.ring[at + 1]);
        }
        slot.fresh = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(start: usize, len: usize) -> Vec<kira::Frame> {
        (start..start + len)
            .map(|i| kira::Frame::new(i as f32, -(i as f32)))
            .collect()
    }

    #[test]
    fn a_window_needs_a_full_set_of_fresh_frames_and_is_taken_once() {
        let capture = AudioCapture::default();
        capture.set_enabled(true);
        let mut out = Vec::new();
        capture.push(&chunk(0, 480));
        assert!(!capture.take_window(&mut out));
        capture.push(&chunk(480, 480));
        assert!(!capture.take_window(&mut out));
        capture.push(&chunk(960, 480));
        assert!(capture.take_window(&mut out));
        assert_eq!(out.len(), CAPTURE_FRAMES * CAPTURE_CHANNELS);
        // the ring holds frames 416..1440 after 1440 pushes
        assert_eq!(out[0], 416.0);
        assert_eq!(out[1], -416.0);
        assert_eq!(out[out.len() - 2], 1439.0);
        assert!(!capture.take_window(&mut out), "a window is taken once");
    }

    #[test]
    fn nothing_is_written_while_disabled_and_enabling_starts_over() {
        let capture = AudioCapture::default();
        let mut out = Vec::new();
        capture.push(&chunk(0, 2048));
        capture.set_enabled(true);
        assert!(!capture.take_window(&mut out));
        capture.push(&chunk(0, 2048));
        capture.set_enabled(false);
        capture.set_enabled(true);
        assert!(
            !capture.take_window(&mut out),
            "a pending window is dropped"
        );
    }

    #[test]
    fn a_chunk_arriving_while_the_slot_is_held_is_dropped_and_counted() {
        let capture = AudioCapture::default();
        capture.set_enabled(true);
        let held = capture.slot.lock().unwrap();
        capture.push(&chunk(0, 480));
        drop(held);
        assert_eq!(capture.dropped(), 1);
        let mut out = Vec::new();
        assert!(!capture.take_window(&mut out));
    }
}
