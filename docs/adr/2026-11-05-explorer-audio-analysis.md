# ADR: The Explorer analyses scene audio in the engine

- Status: Accepted
- Date: 2026-11-05

## Context

The SDK's `AudioAnalysis` component (`library/protocol/proto/decentraland/sdk/components/audio_analysis.proto`)
lets a scene ask for the amplitude and eight band levels of the audio an entity plays, raw or
on a log scale with gains, so visualisers and reactive scenes can follow music. unity-explorer
implements it (`Explorer/Assets/DCL/SDKComponents/AudioAnalysis`) with a small Rust library
(`native/audio-analysis`: RMS and an FFT split into eight bands) fed from Unity's audio thread
through a single-slot buffer, once per DSP buffer, for `AudioSource` clips and the UUAV media
players. Its design report ruled out the mature music-information-retrieval libraries (Aubio,
Essentia) for their GPL licences and settled on this FFT subset. The Explorer registered the
component but computed nothing (parity card K3 SDK media, task `K3 · AudioAnalysis`).

## Decision

- The maths is a port of unity-explorer's library, line for line, in `crates/av/src/audio_analysis/dsp.rs`
  (RMS, eight bands with edges at 60, 120, 250, 500, 1000, 2000 and 4000 Hz and Nyquist,
  `log10(x * gain + 1)` clamped to `[0, 1]`, default gains 5 and 0.05), over an in-house
  radix-2 real FFT (`fft.rs`, unnormalised like fundsp's), tested against the definition. No
  new crate and no FFT dependency: the window is 2048 samples and the transform is sixty
  lines.
- The window is what Unity analyses: 1024 frames of interleaved stereo at the nominal sample
  rate, channel count discarded. That makes a stereo 440 Hz tone read in band 2 with an image
  near Nyquist; the quirk is part of the contract scenes were authored against and is kept.
- A clip (`AudioSource`) is analysed from its decoded frames: the window that ends at the kira
  instance's position, once per window, at the clip's own sample rate. No audio-thread code and
  no second playback path. Pitch and volume are not applied, and a 44.1 kHz clip is analysed at
  44.1 kHz (the band edges are in Hz, so the bands match; the window is 8% longer).
- A stream (`AudioStream`, the audio of a `VideoPlayer`) is analysed from a single-slot capture
  (`crates/media/src/capture.rs`) that the FFmpeg decoder's kira bridge
  (`crates/media/src/ffmpeg/audio_context.rs`) writes on kira's streaming thread, after Unity's
  `LastAudioFrameBuffer`: never blocking, a window not taken in time is overwritten, enabled
  only while an `AudioAnalysis` sits on the entity, nothing captured while the player is not
  playing. The decoder does not resample, so a stream is analysed at its own sample rate, which
  the capture records.
- The whole component is written back, the scene's parameters as received plus the result,
  as Unity's `PutMessage` does; the last values stay while the source is paused. There is no
  frame-time budget: a 2048-point FFT costs microseconds.
- The web build registers the component and compiles the maths but writes no results, as
  unity-explorer's WebGL build (`OnAudioFilterRead` is unavailable there) does.

## Rejected

- **A kira `Effect` tap on a per-source sub-track for clips.** Closest to Unity's post-mix
  data, but bevy_kira_audio's `play` offers no output destination, so analysed clips would play
  through the raw manager with a second play, volume and pan path.
- **A mono downmix or a resample to 48 kHz.** Either would move the bands away from what the
  same scene shows in unity-explorer.
- **fundsp or rustfft as the FFT.** fundsp is a large DSP graph library for one function;
  rustfft would be hand-edited lock entries (the Explorer never runs `cargo update`) for a
  transform this small.

## Consequences

- A scene adding `AudioAnalysis` to an entity with an `AudioSource`, `AudioStream` or
  `VideoPlayer` reads amplitude and bands updated about once per 1024 frames while the audio
  plays, on the standalone. On the web the values stay at zero.
- The e2e `audio` scene (`apps/bevy-explorer/e2e/scenes/audio`) shows four equalisers driven
  by the component, and `specs/visual/audio.test.ts` asserts the values through the engine on
  the standalone.
- The debug panel's `Media player` section lists every analysed entity with its mode, gains,
  window, sample rate, last values and whether its stream capture is on.
- Follow-up: a web implementation (an `AnalyserNode` or `AudioWorklet` in the page's audio
  graph and a `createMediaElementSource` for media elements, subject to CORS) would have to
  decide whether to reproduce the interleaved window or downmix.
