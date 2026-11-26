# Toolkit video Restart commands

## Status

Accepted.

## Context

VideoPlayer position is component state. Repeating a Restart at position zero
can produce identical bytes, which current SDKs suppress. Treating identical
component deliveries as seek commands instead makes older scenes that resend
state every frame continually rewind.

A position-only update also cannot restart playing video on Unity. Its playback
state must change. Keeping a previous seek on ordinary Pause or Resume can
cause another unwanted seek.

## Decision

The Toolkit receives Restart through its existing authenticated admin command
channel. On each recipient, a command requesting playing at position zero first
sets `playing: false, position: -1`, then sets `playing: true, position: 0` after
a complete SDK update and at least 200 milliseconds of scene time. The second
position differs by more than the engine's seek threshold. No new protocol
field or SDK component wrapper is required.

Stage both writes on each recipient rather than sending two network messages:
network batching must not collapse them into one local component update. A new
playback, source or seek command cancels the pending Restart. Volume and loop
changes survive its completion. Missing players and changed sources discard
pending work. Ordinary commands clear a retained seek position.

The engine compares position changes using its existing half-second threshold.
Repeated identical state carries no new seek intent. This restores the behavior
before the repeated-request heuristic; first zero seeks without a preceding
position change are not treated as commands.

## Consequences and verification

Scenes must be rebuilt with the updated Toolkit. Existing deployments do not
receive this behavior automatically. Other SDK code that repeatedly assigns the
same position still does not express a new seek.

Separate SDK sends are not a renderer acknowledgement. Validate repeated Restart,
Restart after Pause and natural EOF, and Pause/volume/Resume on each client.
Unity requires the paused stage to reach its playback system. Android and iOS
negative seek handling and playback debounce require device acceptance; their
compatibility is not established by a source harness. Browser playback also
requires runtime acceptance.

Automated regressions cover authenticated remote delivery, batched commands,
separate SDK serialization, cancellation, setting preservation and repeated
engine updates. Cast and Stream lifecycle acceptance remains separate.
