# ADR: Conventions for the Explorer's automation console commands

- Status: Accepted
- Date: 2026-11-17

## Context

Agents and the end-to-end suite drive the Bevy Explorer through its console
([ADR](2026-10-29-explorer-tests-through-the-console.md)): the web page's `engine_console_command`,
the native loopback server (`dcl/console`) and the CEF HUD's page all send a command line. The
console's scene-driving commands are growing from a few pointer commands into a set that walks,
turns the camera, presses inputs, drives scene UI and reads evidence. An agent judges a scene
from these replies alone, so each command must say what it did, against which frame of reference,
and why it did nothing when it refused.

The shared scene test script, `sdk7-test-scenes`'
[`MCP_SHOWCASE.md`](https://github.com/decentraland/sdk7-test-scenes/blob/0bc387a3bad4a6a835d520fc3a54d3c4cba5a118/scenes/149,149-synthetic-input-showcase/MCP_SHOWCASE.md),
also drives the Unity Explorer through its MCP server. It names arguments and results such as
`crdtEntityId`, `hitPoint`, `aimErrorDegrees`, `blockedByUi` and `screen`, and a script written
once must run against either client.

## Decision

Every automation command follows these rules. `crates/console/src/line.rs` and
`crates/console/src/reply.rs` hold the shared pieces.

1. **Quoting.** A line is split into words on whitespace; a double-quoted run
   is part of a word with its quotes removed, and inside it `\"` and `\\` escape. A backslash
   elsewhere and a single quote are ordinary characters, so `it's` and Windows paths need no
   quoting. A command whose last argument takes the rest of the line (`trailing_var_arg`, how
   the JSON commands take their JSON) gets its words split on whitespace alone, quotes kept, so
   the JSON it joins back is the JSON sent, except that a run of whitespace inside one of its
   strings becomes one space. An unclosed quote fails the call. One function,
   `console::line::split`, splits every line: the chat input's and each transport's.
2. **Coordinates.** New commands take and return normalized screen coordinates, 0 to 1 from the
   top left. Every reply that carries a screen coordinate or rect carries `screen`
   `{width, height, scale}`: the window in logical pixels and its device pixel ratio, so a
   screenshot's pixels are `scale` times as many. Existing pixel commands (`/click_at`,
   `/entity_screen_position`) keep logical pixels and add `screen` and `normalized`;
   `/click_at --normalized` takes the normalized form.
3. **Durations.** New commands take `--seconds <f32>`; the existing pointer commands keep
   `--frames` and gain `--seconds`. Seconds count in frames at the frame rate
   `FrameTimeDiagnosticsPlugin` measures. Rates are per second: `camera_look` turns in degrees per second, and its
   reply says so.
4. **Waiting.** A command waits for its effect inside the command, bounded by a
   `--timeout-seconds` argument, and replies what it observed. A timeout reply says what it waited
   for.
5. **Refusals and warnings.** A command that does nothing fails with
   `{"refused": "<reason>", "cause": {...}}`, the cause naming what stood in the way: the covering
   element, the camera mode area, the disabled input, the held press, by `crdtEntityId` where it
   has one. A command that did what it was asked but may surprise the caller adds a `warning` to
   its reply (a rig limit kept the aim error up, the hit entity declares no handler for the
   button).
6. **No press on a miss.** A world-aimed pointer call whose ray reaches nothing refuses without
   pressing and reports `rootBroadcast: false`. A press the caller did not aim at anything would
   change the scene's state under a call that reports failure.
7. **Names and ids.** Arguments and reply fields use the shared scripts' names. SDK entity ids are
   the only id space: replies report them as `crdtEntityId`, and keep `entity` where an existing
   reply already uses it.
8. **One input path.** New gestures (walking, held inputs, camera deltas, sweeps) are intents
   beside `SyntheticAim` in `crates/scene_runner/src/update_scene/synthetic_pointer.rs`, consumed
   by the systems real input feeds. The commands in `src/automation/` are thin wrappers: none
   writes the player transform or casts a ray of its own.

## Consequences

- `SystemApi::ConsoleLine` carries a transport's whole line; the system bridge splits it once it
  knows the command. The web page's `window.engine.<name>()` functions quote string arguments the
  same way, so `engine.chat('nearby', 'two words')` sends one message; for a command that takes
  the rest of the line they send the words unquoted.
- A command typed in the chat input follows the same rule: a single quote and a backslash outside
  double quotes are ordinary characters, and an unclosed double quote is reported in the System
  channel and runs nothing.
- `/click_at` replies JSON: `x`, `y`, `normalized`, `button`, `screen` and, on the web, `input`.
- A value holding a lone `"` is quoted or escaped on non-JSON commands.
- `crates/console/src/reply.rs` holds `screen` and the normalized form; the duration, refusal and
  warning helpers join it with the first command that uses each.

## Alternatives considered

- **A JSON-argument form (`/cmd {json}`).** Rejected: quoting carries free text, and clap keeps
  parsing, validating and documenting every argument.
- **A POSIX shell split.** Rejected: `#` starts a comment and a backslash escapes outside quotes,
  which would cut chat text at a hashtag and mangle Windows paths.
- **Changing the pixel commands to normalized units.** Rejected: the end-to-end suite clicks at
  the pixels `/hud_read` and `/entity_screen_position` report. They gain `screen` and the
  normalized form instead.
