# ADR: Conventions for the Explorer's automation console commands

- Status: Accepted
- Date: 2026-11-17

## Context

Agents and the end-to-end suite drive the Bevy Explorer through its console
([ADR](2026-10-29-explorer-tests-through-the-console.md)): the web page's `engine_console_command`,
the native loopback server (`dcl/console`) and the CEF HUD's page all send a command line, and a
player types one in the chat input. An agent judges a scene from the replies alone, so a command
must say what it did and against which frame of reference, and free text must arrive as written.

The shared scene test script, `sdk7-test-scenes`'
[`MCP_SHOWCASE.md`](https://github.com/decentraland/sdk7-test-scenes/blob/0bc387a3bad4a6a835d520fc3a54d3c4cba5a118/scenes/149,149-synthetic-input-showcase/MCP_SHOWCASE.md),
also drives the Unity Explorer through its MCP server. It names arguments and results such as
`crdtEntityId` and `screen`, and a script written once must run against either client.

## Decision

The console's automation commands follow these rules. `crates/console/src/line.rs` and
`crates/console/src/reply.rs` hold the shared pieces. A rule for a kind of command the console
does not have yet (durations, waits, refusals) is added here with its first command.

1. **Quoting.** A line is split into words on whitespace (Unicode's White_Space); a double-quoted
   run is part of a word with its quotes removed, and inside it `\"` and `\\` escape. A backslash
   elsewhere and a single quote are ordinary characters, so `it's` and Windows paths need no
   quoting. A command whose last argument takes the rest of the line (`trailing_var_arg`, how
   the JSON commands take their JSON) gets its words split on whitespace alone, quotes kept, so
   the JSON it joins back is the JSON sent, except that a run of whitespace inside one of its
   strings becomes one space. An unclosed quote fails the call. `console::line::split` splits
   every line the engine receives whole: the chat input's and each transport's. Two senders split
   for themselves: a scene's `op_console_command` passes its arguments as a list, and a web page
   command (`/hud_*`, the page's `/click_at`) reads its own line in the page.
2. **Coordinates.** A command takes and returns normalized screen coordinates, 0 to 1 from the
   top left, 1 being the window's last pixel. Every reply that carries a screen coordinate or rect
   carries `screen` `{width, height, scale}`: the window in logical pixels and its device pixel
   ratio, so a screenshot's pixels are `scale` times as many. The pixel commands (`/click_at`,
   `/entity_screen_position`) take logical pixels and reply with `screen` and `normalized` beside
   them; `/click_at --normalized` takes the normalized form.
3. **Names and ids.** Arguments and reply fields use the shared script's names. SDK entity ids are
   the only id space: replies report them as `crdtEntityId`, and keep `entity` where a reply
   already uses it.

## Consequences

- `SystemApi::ConsoleLine` carries a transport's whole line; the system bridge splits it once it
  knows the command. The web page's `window.engine.<name>()` functions quote string arguments the
  same way, so `engine.chat('nearby', 'two words')` sends one message; for a command that takes
  the rest of the line, by the engine's own `console::line::takes_rest`, and for a page command
  they send the words unquoted.
- A command typed in the chat input follows the same rule: a single quote and a backslash outside
  double quotes are ordinary characters, and an unclosed double quote is reported in the System
  channel and runs nothing.
- `/click_at` replies JSON: `x`, `y`, `normalized`, `button`, `screen` and, on the web, `input`.
- A value holding a lone `"` is quoted or escaped on non-JSON commands.

## Alternatives considered

- **A JSON-argument form (`/cmd {json}`).** Rejected: quoting carries free text, and clap keeps
  parsing, validating and documenting every argument.
- **A POSIX shell split.** Rejected: `#` starts a comment and a backslash escapes outside quotes,
  which would cut chat text at a hashtag and mangle Windows paths.
- **Changing the pixel commands to normalized units.** Rejected: the end-to-end suite clicks at
  the pixels `/hud_read` and `/entity_screen_position` report. They gain `screen` and the
  normalized form instead.
