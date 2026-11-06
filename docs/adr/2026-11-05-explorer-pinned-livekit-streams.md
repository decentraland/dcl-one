# ADR: A VideoPlayer can pin one participant's LiveKit track

- Status: Accepted
- Date: 2026-11-05

## Context

A scene's `VideoPlayer` can show live video from the scene's LiveKit room. The Explorer played
one address, `livekit-video://current-stream`: whatever the livestream manager picks
(presentations, screenshares, casts, then streams, in that order), through one image every
receiver shares. The scene API `getActiveVideoStreams` already listed every published track as
`livekit-video://{identity}/{sid}`, but a player naming one of those was rejected as a non-http
source. unity-explorer binds such an address to exactly that participant's track and never
follows the active speaker (`LivekitPlayer.cs`, parity card K3 SDK media, task `K3 · Pinned
LiveKit streams`).

## Decision

- `livekit-video://` addresses are parsed, not matched as strings
  (`crates/livestream_manager/src/address.rs`): the sentinel, or exactly `{identity}/{sid}` with
  both non-empty. Anything else is a malformed address: the player gets a placeholder and a
  `VsError`. Unity reads `parts[2]` and `parts[3]` unchecked and throws.
- Every published video track carries its room's scene hash, its participant's identity and
  its sid (`PublishedVideoTrack`, stamped by the comms track plugin). A pinned player binds the
  track that matches all three inside its own scene's room, as unity-explorer scopes its
  players to the scene room; the link is a relationship (`PinnedTo` on the player, `PinnedBy` on
  the track), so despawning either side ends it.
- One target image per track at a time: the manager's image while the track is the active
  transmitter, otherwise an image of the track's own (`PinnedImage`, created with the first pin,
  dropped with the last). Pinned players read whichever one their track writes and switch when
  the active transmitter moves. On the web a track's `<video>` element is adopted into the media
  host once per target, so one target keeps one code path on both platforms.
- A track stays subscribed while the active transmitter or any pinned player wants it, and is
  unsubscribed when neither does.
- A pinned player plays only while its scene is current, like the sentinel, and counts
  against the video cap like any player. While its track is away it shows the manager's image
  and reports Loading or Playing with it, keeping the requested address; when the same track
  is published again it is bound again. unity-explorer stays on the manager's pick until the
  source changes; the re-bind is a deliberate divergence.
- Audio needs nothing new: a participant's microphone plays through the voice path, a cast's
  or stream's audio through the manager's active audio transmitter.

## Rejected

- **Copying each frame into both the manager's and the pinned image.** Possible natively, not
  on the web, where the element is adopted under one media id; one target per track keeps the
  platforms alike at the cost of a one-frame placeholder when the target flips.
- **A muted-camera placeholder.** The comms room plugin does not handle `TrackMuted`; a
  muted track keeps its last frame. Follow-up.
- **Binding across rooms.** An address another scene's API advertised never binds; that is
  Unity's scope as well.

## Consequences

- A scene that reads `getActiveVideoStreams` and sets a player's `src` to one entry shows that
  participant's track, whoever is speaking; the sentinel keeps today's behaviour. A reconnect
  changes sids, so a pinned player stays in fallback until the scene re-reads the list, as in
  Unity.
- The debug panel's `Media player` section counts pinned players and those in fallback, and
  shows each livekit player's requested address, mode and bound track.
- The e2e `livekit` scene (`apps/bevy-explorer/e2e/scenes/livekit`) and `specs/visual/livekit.test.ts`
  cover the address outcomes without a room on both platforms, and a pinned track playing from
  a local `livekit-server` with the LiveKit CLI publishing, opt-in on the standalone
  (`scripts/livekit-room.sh`).
- `MAX_LIVEKIT_TEXTURE_SIZE = 2048` (unity-explorer's cap) is not enforced: the livekit crate
  delivers I420 frames at the publisher's size and no scaling step exists here. Follow-up.
