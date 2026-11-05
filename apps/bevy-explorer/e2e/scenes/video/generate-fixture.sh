#!/usr/bin/env bash
# Regenerates the committed fixture the `video` scene plays (assets/video/red_tone.mp4, Git LFS),
# after unity-explorer's UUAV test fixtures (Explorer/Assets/Plugins/UUAV/Packages/UUAV/Tests/
# Fixtures~/generate.sh). The output is committed so no run depends on a local ffmpeg; rerun
# this only when the fixture must change, then commit the result and re-check the spec's
# expectations (specs/visual/video.test.ts). Requires ffmpeg with libx264 and aac.
#
# red_tone.mp4: 6 s, 320x240 at 30 fps, a keyframe every second; solid red video and a 440 Hz
# sine at -3 dB peak, stereo 48 kHz aac. Solid, so every frame is the same red; with a tone, so
# the file carries an audio stream.
set -euo pipefail
cd "$(dirname "$0")/assets/video"

ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "color=c=red:size=320x240:rate=30:duration=6" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=6" \
    -filter_complex "[1:a]volume=18dB,aformat=sample_fmts=fltp:channel_layouts=stereo[a]" \
    -map 0:v -map "[a]" \
    -c:v libx264 -preset veryfast -profile:v baseline -pix_fmt yuv420p \
    -g 30 -keyint_min 30 \
    -c:a aac -b:a 96k \
    -movflags +faststart \
    red_tone.mp4

ffprobe -v error -show_entries stream=codec_name,width,height,duration -of compact red_tone.mp4
