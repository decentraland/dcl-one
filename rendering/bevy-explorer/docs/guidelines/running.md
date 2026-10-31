# Running and testing

The dev loop is the `justfile` in `rendering/bevy-explorer/source` (`just --list`).
Distributables are built by repository-root `apps/bevy-explorer/Makefile` and
`.github/workflows/bevy-explorer-desktop.yml`. Commands below run from
`rendering/bevy-explorer/source` unless another directory is named.

## Node

Every JS tool goes through nvm: `source ~/.nvm/nvm.sh && nvm use <version>` first. `react-web`,
the bridge scene and the auth-server scenes need Node 24. Scenes and the SDK toolchain default
to 20. A scene installed without its lockfile silently drops `fsevents`; copy the template
lockfile in.

## Native

```
just setup-cef                                   # once
just native-debug --realm <about-url> --position x,y
```

`just native-debug` bundles the HUD when stale and runs the client. A plain `cargo run` boots to
a login screen with no HUD. Both binaries must build: `cargo build --package dcl_deno_ipc`,
then `cargo build`. This loop is verified on macOS, the only platform where the client falls back
to `~/.local/share/cef` at runtime; on Linux the loader needs `LD_LIBRARY_PATH` (see
`source/readme.md`), and on Windows the desktop workflow's staging shows what CEF needs.

To package, follow the [desktop workflow](../../../../.github/workflows/bevy-explorer-desktop.yml):
pinned Rust, protoc, CEF and platform libraries, including `CEF_PATH`. With those installed:

```sh
sh scripts/gen-ts-bindings.sh
npm --prefix react-web ci
npm --prefix react-web/bridge-scene ci
npm --prefix react-web run bundle:native
cargo build --release --locked --package dcl_deno_ipc
cargo build --release --locked
```

The workflow also stages the CEF helper, runtime libraries, assets and sidecar. Follow its
platform packaging steps, then launch the packaged executable with `--realm <about-url>
--position x,y`. A Cargo build alone does not bundle the HUD or package those runtime files.

## Web

`just wasm` builds the wasm package and the sandbox worker bundle, then starts the page dev
server and the bridge. The page boots the engine from `deploy/web/engine/pkg/`. Kill any stale
dev server on the ports first; a reused one serves the old build.

For the distributable, run `make web` in repository-root `apps/bevy-explorer`
(`python3 build-web.py` without Make). It builds the engine wasm, sandbox worker and HUD into `apps/bevy-explorer/dist/web`; it
does not start a dev server. `make web-dev` skips wasm optimization. Import the artifact
with `python3 tooling/fetch-play.py --from-build apps/bevy-explorer/dist/web` from the
repository root, then follow the root README to run Sites. Sites serves it under `/play`
with its own HUD, `apps/ui3/src/play`; see `rendering/bevy-explorer/README.md` for how that
copy tracks `react-web`. Full DevTools logging needs DevTools open before the page loads.
The engine console lives in the app iframe.

## Headless

```
cargo run --bin headless --no-default-features --features headless,livekit -- <args>
```

Check the bin and feature every time; the wrong bin exits 0 through a pipe. Run it against a
real scene before pushing anything touching shared systems, because a `Res<>` from a plugin the
headless build skips panics only at runtime.

## The console

The console (`crates/console`) is the debugging surface in every build: `/help` lists
commands: `/time`, `/crdt_snapshot`, `/new_entity` and the rest, plus the preview-only
commands registered with `add_preview_console_command`. On web the same surface is reachable
from the browser console through `window.engine`, which lives in the app frame (`app.html`),
not the shell page: pick the app frame in the DevTools context selector first.

## Test commands

> **Implementation and acceptance:** the web driver, HUD automation helper and native
> automation modules are checked in. Verify command registration at the target revision
> and record runtime evidence in the task report before claiming acceptance on a platform.
> The [web driver README](../../source/local-harness/web-driver/README.md) describes its
> requirements and invocation; source availability alone is not a test pass.

End-to-end tests drive the client through the console, the HUD included, so one spec runs on the
web and on the standalone, with no Playwright
([ADR](../../../../docs/adr/2026-10-29-explorer-tests-through-the-console.md)).

State, in every build:

- `/world_state`: JSON with `inWorld` and its parts (signed in, world hold, realm connected, out
  of world), the position, and `arrivals`, the player's returns to the world. Every teleport and
  realm change takes the player out of world first, so a test reads `arrivals` before one and
  waits for it to grow with `inWorld`.
- `/avatar_state [all|<address>]`: avatars as selected (body shape, wearables, emotes, forced
  slots), as rendered (built or not, wearables by category, hidden categories) and the emote
  each plays, as a JSON array; the player's by default. The HUD's preview avatars are tagged
  `preview`. Rendered URNs are normalised (lowercase, default wearables included); selected ones
  are as written.
- `/chat_history [count]`: the last chat messages sent and received (200 kept), oldest first,
  with the sender's address and `messageId`, and `total` counting every message so far. Commands,
  emote markers and direct messages are left out; `/dm_history` has those.
- `/voice_state`: the mic (`available`, `enabled`) and, in LiveKit builds, each voice room's state
  and the addresses speaking.
- `/entity_screen_position <entity> [--bounds]`: where a scene entity is on screen, in the
  coordinates `/click_at` takes, `null` behind the camera. The scene is the one `/set_scene`
  picked, else the player's; the entity is as `/scene_entities` lists it. `--bounds` aims at the
  centre of its visible meshes instead of its origin.
- `/open_deep_link <url>` (standalone): applies a `decentraland://` place link (realm, position)
  as `--deep-link` does at launch, and answers once the realm or the teleport is set. Sign-in links
  are not handled.

The HUD and input:

- `/hud_list [role]`, `/hud_read`, `/hud_click`, `/hud_fill`, `/hud_wait`: a HUD control by role
  and accessible name. The selector is JSON `{"name", "role", "index", "exact"}` or just the name;
  `/hud_fill` adds `"text"`, and `/hud_wait` adds `"state"` (`visible`, `hidden`, `enabled`,
  `actionable`) and `"timeout"` in ms (10000 by default). A click needs the control visible,
  enabled and not covered, and several matches need an `index`. `/hud_fill` answers with the field
  as the typing left it. The page's side is `react-web/src/lib/hudAutomation.ts`.
- `/press_key <code> [--state down|up]`, with the key named as `KeyboardEvent.code` names it, and
  `/click_at <x> <y> [--button left|right|middle]`.

The accepted design makes these commands the client's agent surface in every build,
so agents and tests share the same interface. Its native implementation asks the page where
a control is, then clicks through CEF and types key by key (`src/react_hud_cef/automation.rs`),
always `trusted`, and `/press_key` and `/click_at` feed the engine the window's own input events
(`src/automation.rs`). On the web, `engine.js` runs the HUD and input commands in the page. There
they get real input when a DevTools client has installed `window.__dclTrustedInput` (the web driver
does, see below); elsewhere (the DevTools console, a page script) they fall back to DOM events,
which prove the HUD's handlers but not the browser's input path, and give a page script nothing it
lacks. `/hud_click` and `/hud_fill` say which in `"input"`: `trusted` or `synthetic`. Arguments
are split on whitespace, so runs of spaces in a name or a text collapse to one. The preview-only
commands (`/jump`, `/speed`, `/move_player_to`, the debug dumps) stay preview-only.

**The loopback console contract.** The design requires every native build, including headless client mode, to carry a
JSON-RPC server that runs console commands from outside the process, off unless the client is
started with `--automation-token <token>` (`--automation-port <port>`, 15702 by default; the
environment's `DCL_AUTOMATION_TOKEN` and `DCL_AUTOMATION_PORT` stand in when the flags are
absent). It serves two methods on `127.0.0.1` and none of Bevy's own: `dcl/console`
(`{"command": "/teleport 10 20", "token"}` answers `{"id"}`) and `dcl/console_result` (`{"id",
"token"}` answers `{"done": false}` until the command has answered, then `done`, `ok` and
`output`). Both need `token` to match, and refuse every call without it. A test reaches the
standalone through it, with no browser, and the web client through the web driver's `console`.
The headless client registers the state commands that have meaning without a window
(`/world_state`, `/chat_history`, `/voice_state`), not the HUD, input or screen-position ones;
`/avatar_state` is not among them, since no avatar is selected without rendering.

**Trusted input on the web: a stable contract.** A DevTools client that drives the page (the web
driver in `local-harness/web-driver`, or a Playwright or Puppeteer script through `exposeBinding`
and its own mouse and keyboard) installs `window.__dclTrustedInput(json)` in every frame of the
page's origin. The page keeps the finding: `/hud_click`, `/hud_fill`, `/press_key` and `/click_at`
locate the control and call the hook with one JSON string, `{"id", "op", ...}`:

| `op` | fields |
| --- | --- |
| `click` | `x`, `y` (CSS pixels in the calling frame's viewport), `button`: `left`, `right` or `middle` |
| `key` | `code`, `key`, `keyCode` (Windows virtual key code), `text` (optional), `location` (0, 1 left, 2 right), `state` (optional, `down` or `up`; absent presses and releases) |
| `insertText` | `text` |

The client performs the input on the top-level page (offsetting `x` and `y` by where the calling
frame sits, since the app frame is out of process) and then evaluates
`window.__dclTrustedInputDone(id, error)` in the frame that asked, `error` a string or `null`. A
key with `state` `down` stays held, with its modifier applied to every event until its `up`. The
page waits 10 s for the answer and then fails the command. Nothing else about the runner is part
of the contract.

## Debugging

- **Capture the whole web log.** The engine runs in a worker inside the app frame, and
  DevTools only records a worker from the moment it is attached. Open DevTools on an empty tab
  first, tick "preserve log" on Console and Network and "show timestamps", then type the URL.
  Attached late, the console holds only the last ~1500 lines. The Chrome extension sees only
  the top frame, never the engine.
- **The log level is compiled in on wasm.** `RUST_LOG="ipfs=debug,scene_runner=info"
  wasm-pack build ...`. Natively `RUST_LOG` replaces the whole filter, so prefix `info,` or
  the scene runner's info lines vanish.
- **Regression hunts start with the dependency bumps.** Diff `package.json`, both lockfiles
  and SDK channel moves before the code; a regression that lives in `node_modules` is invisible
  to a code-path sweep. Get the exact appearance of the reported symptom (misaligned, wrong
  size, stale, missing?) before committing to a diagnosis, and check a confirmed bug against
  that description before declaring it the cause.
- **When reading keeps saying "this should work", trace it.** Bugs that show only on the
  first occurrence or first state transition are the usual case. Treat black-box clues from the
  reporter ("a delay doesn't help") as constraints, not noise.
- **Shader geometry.** Add a toggleable debug colour early: recolour fragments by the suspect
  quantity (sign of a dot, a uv, which tile was picked) and look. Weight-blend the debug
  output the way the real shader does, or it shows artifacts the render never produces.
- `/player_position` reports z, and parcel y is `floor(z / 16)`, not `floor(-z / 16)`. That
  sign slip has looked like a hung console RPC before.
- The `tracy` cargo feature enables bevy's Tracy tracing for native profiling.

## Testing gotchas

- **Out-of-bounds culling can't be tested against localhost.** A loopback realm forces
  show-out-of-bounds on regardless of `--preview`. Point the client at the same preview server
  by LAN address, `--realm http://$(ipconfig getifaddr en0):<port>`.
- **The bridge scene's build output is not what the engine loads.** After any bridge-scene
  source change run `npm run export-native` (native) or `npm run bundle` (web) from
  `react-web/bridge-scene/`, or the client runs the stale scene from the exported folder.
- **Local SDK builds install through overrides, never `file:` paths** in a scene's
  `package.json`. The `@dcl/sdk` tarball carries internal `file:` references that break once
  moved.
- **Wasm clippy is not a CI gate** and fails on known warnings in the livekit web code. The
  wasm build is the gate; see contributing for when it's required.
- **Pulse locally** is a .NET server built from source. `.NET` lives under `~/.dotnet` on
  macOS; port 5000 is taken by AirPlay so override `HttpService__Port`; ENet needs
  `BindHost: "::"`.

## Tests

- `cargo test --all`, not `cargo test -p <crate>`. The full workspace unifies `serde_json` with
  `preserve_order` from `deno_core`, so a per-crate run can pass on map ordering that the
  workspace run breaks. Parse JSON in tests; never compare serialised strings.
- `react-web`: `npm test` runs the checked-in Vitest suite. The Playwright visual/engine
  gate specs and configs are absent from this public checkout; the corresponding package
  scripts do not establish suite availability. See `react-web/review.md` for the optional
  tiers and report them as not run unless the authorized test workspace supplies them.
- `local-harness/presence` tests presence isolation and needs
  `Transport__PeerTimeoutMs=8000` on the server. The scene-teardown harness
  `local-harness/scene-kill` is not included in this public checkout.
- `local-harness/benchmark` is the benchmark's runner and report, a binary with its own tests (the
  stream protocol, the server's security rules, the report on fixtures, the page's sender and the
  reset page under node when it is installed); see [Benchmark](benchmark.md#the-runner).
- `local-harness/web-driver` drives the web client in Chrome for end-to-end tests, with no
  dependency: `node --test local-harness/web-driver/driver.test.mjs` checks its real input against
  the installed Chrome, with the app frame in and out of the page's process.
- `crates/perf_output/tests/gpu_timestamps.rs` and `gpu_frame_time.rs` need a GPU, so their tests
  are `#[ignore]`d and CI skips them. Run each per backend, as [Benchmark](benchmark.md#gpu-time)
  lists:
  `WGPU_BACKEND=dx12|vulkan cargo test -p perf_output --test gpu_timestamps -- --ignored --nocapture`,
  and the same with `--test gpu_frame_time`.
- There is no visual regression suite for the 3D side. Rendering PRs carry before and after
  screenshots instead.
