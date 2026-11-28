# ADR: Scenes follow the stream and Cast through `@dcl/sdk/cast`

- Status: Accepted
- Date: 2026-11-20

## Context

A scene that shows a stream or a Cast presentation reads the room's tracks, the video player state, the presentation's topic messages, the presenters and the stream key. The Admin Toolkit had its own polling, fetch wrappers, presentation detector and stored copies of the key state. Its open panel did not follow a key that was revoked, replaced or expired, and each scene that wanted the same data wrote the same code again.

## Decision

The SDK has one module for this, `@dcl/sdk/cast`.

- The state is one value, changed only by a pure reducer. Signed fetches, runtime reads, topic packets, the player source and the timer are data that the reducer returns; a thin runtime runs them.
- The role of the handle decides what a scene can send: a viewer nothing, an admin the Cast commands, the owner the stream key commands too. The server still decides what the wallet may do, and a refusal arrives as a typed error that says if and when to retry.
- Slide state comes only from packets that the presentation bot sent. The bot's identity comes from the server; any wallet in the room can publish on the topic.
- Each connection has a budget: the room's tracks once a second and the slide packets four times a second (both local to the Explorer), each server read at most once a minute, one write at a time, and backoff after a failure.
- The Admin Toolkit reads one handle of this module and keeps no copy of the state.

## Consequences

The module is a new public surface of `@dcl/sdk` and ships with an SDK release. A package that imports it, the asset pack included, builds only with an SDK that has it. The scene CLI serves the module from its SDK chunk. The presenter's camera for viewers on the web Explorer is not part of this module yet.
