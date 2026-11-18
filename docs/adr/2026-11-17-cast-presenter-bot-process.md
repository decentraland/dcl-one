# ADR: The Cast presenter's LiveKit participant is a separate `cast-bot` process

- Status: Accepted
- Date: 2026-11-17

## Context

A Cast presentation needs a LiveKit participant: it publishes the slide track and
reads and writes the data channel. Upstream (`decentraland/cast-presenter-server` @
cae78612) does this from Node with `@livekit/rtc-node`, and for client composition it
starts a Go `cast-sidecar` per session that owns the connection and takes JSON-line
commands.

The only LiveKit participant code in this repository is the Explorer's pin of the
Rust SDK (`rendering/bevy-explorer/source/Cargo.toml`, rev `b2f8bc84`). That SDK links
libwebrtc through `webrtc-sys`, whose build script downloads a 275 MB prebuilt archive
and compiles a C++ bridge. A probe build on macOS took 12 minutes. In Nix the archive
must be fetched ahead (`LK_CUSTOM_WEBRTC`, as `infra/packages/bevy-explorer.nix` does),
and Linux needs the clang that `apps/bevy-explorer/install-clang.sh` installs. The
node's Nix package (`infra/packages/catalyst.nix`) has none of this. It also vendors
every package in `services/catalyst/Cargo.lock`, and the SDK's git commits live on
refs that a clone does not fetch.

## Decision

- **The node owns the presentation:** `comms::cast_presenter` serves
  `POST /cast-presenter-service/presentations`. It checks the request and downloads the
  file, renders the PDF (hayro, pure Rust), keeps the sessions in memory, handles the
  presenters' commands and builds every byte clients parse.
- **`cast-bot` owns the LiveKit connection:** `services/catalyst/cast-bot` is one
  binary, started once per session by the node at `CAST_BOT_BIN`. It connects with the
  bot token, publishes the frames it is given as a SCREENSHARE track, and forwards data,
  metadata and participant events. It speaks JSON lines on stdio, the shape of
  upstream's sidecar IPC. The protocol is documented in `comms::cast_presenter::bot`.
- **A workspace of its own:** `cast-bot` has its own `Cargo.lock` and is excluded from
  the node's workspace. A node build, test or Nix package never compiles libwebrtc. It
  uses the Explorer's SDK rev.

## Rejected options

- **Link the SDK into `catalyst`:** every node build and the node's Nix package would
  take libwebrtc and its toolchain, and a libwebrtc crash would take the node down.
- **A `cast-bot` member of the node's workspace:** the node's lock would name the SDK.
  `catalyst.nix` would then vendor it, and workspace-wide builds would compile it.
- **Implement LiveKit signaling over a pure-Rust WebRTC stack:** a second participant
  implementation to maintain beside the Explorer's.

## Consequences

- The route answers 503 until `CAST_BOT_BIN` names a built `cast-bot`.
- Each session costs a process, and a frame crosses the pipe as base64 RGBA (about
  11 MB for 1920x1080). Frames cross only on navigation, because the bot repeats the
  last frame itself.
- `cast-bot` is a Nix package of its own (`infra/packages/cast-bot.nix`) with the
  Explorer's libwebrtc provisioning; the APIs image carries it and sets `CAST_BOT_BIN`.
- On macOS its build script adds `-ObjC` to the link, as the Explorer's cargo config does.
