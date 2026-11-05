#!/usr/bin/env bash
# Regenerates the committed fixtures the `audio` scene plays (assets/audio/*.mp3, Git LFS). The
# output is committed so no run depends on a local ffmpeg; rerun this only when a fixture must
# change, then commit the result and re-check the spec's expectations
# (specs/visual/audio.test.ts). Requires ffmpeg with libmp3lame. MP3, because a stream is
# served without range requests and ffmpeg's ogg demuxer seeks while probing.
#
# tone_440_loop.mp3: 20 s of a 440 Hz sine at -3 dB peak, stereo 48 kHz: the clip the
# AudioSources loop and the AudioStream streams. assets/video/red_tone.mp4 is the video scene's
# fixture, copied, for the VideoPlayer's audio.
set -euo pipefail
cd "$(dirname "$0")/assets/audio"

ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=20" \
    -af "volume=18dB,aformat=sample_fmts=fltp:channel_layouts=stereo" \
    -c:a libmp3lame -b:a 128k \
    tone_440_loop.mp3
ffprobe -v error -show_entries stream=codec_name,sample_rate,channels,duration -of compact tone_440_loop.mp3
