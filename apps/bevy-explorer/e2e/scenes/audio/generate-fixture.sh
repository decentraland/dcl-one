#!/usr/bin/env bash
# Regenerates the committed fixtures the `audio` scene plays (assets/audio/*.mp3 and
# assets/video/red_tone.mp4, Git LFS). The output is committed so no run depends on a local
# ffmpeg; rerun this only when a fixture must change, then commit the result and re-check the
# spec's expectations (specs/visual/audio.test.ts). Requires ffmpeg with libmp3lame, libx264 and
# aac. MP3, because a stream is served without range requests and ffmpeg's ogg demuxer seeks
# while probing.
#
# tone_440_loop.mp3: 20 s of a 440 Hz sine near 0 dBFS peak (the sine source's 1/8 amplitude
# raised 18 dB), stereo 48 kHz: the clip the
# AudioSources loop and the AudioStream streams.
# red_tone.mp4: 6 s, 320x240 at 30 fps, a keyframe every second; solid red video and the same
# 440 Hz sine, stereo 48 kHz aac: the VideoPlayer's audio.
set -euo pipefail
cd "$(dirname "$0")/assets"

ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=20" \
    -af "volume=18dB,aformat=sample_fmts=fltp:channel_layouts=stereo" \
    -c:a libmp3lame -b:a 128k \
    audio/tone_440_loop.mp3
ffprobe -v error -show_entries stream=codec_name,sample_rate,channels,duration -of compact audio/tone_440_loop.mp3

ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "color=c=red:size=320x240:rate=30:duration=6" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=6" \
    -filter_complex "[1:a]volume=18dB,aformat=sample_fmts=fltp:channel_layouts=stereo[a]" \
    -map 0:v -map "[a]" \
    -c:v libx264 -preset veryfast -profile:v baseline -pix_fmt yuv420p \
    -g 30 -keyint_min 30 \
    -c:a aac -b:a 96k \
    -movflags +faststart \
    video/red_tone.mp4
ffprobe -v error -show_entries stream=codec_name,width,height,duration -of compact video/red_tone.mp4
