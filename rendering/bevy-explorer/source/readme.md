# bevy-explorer

A forward-looking implementation of the Decentraland protocol, written in [rust](https://www.rust-lang.org/) on the [Bevy](https://bevy.org) engine.

![screenshots](montage.png)

One engine, three products:

| Target | What it is | Where it ships |
| --- | --- | --- |
| **Web client** | The engine compiled to WebAssembly (WebGPU), with a React DOM HUD, running in the browser | `@dcl-regenesislabs/bevy-explorer-web` → served at `decentraland.zone/bevy-web` |
| **Desktop client** | Native binary for macOS / Linux / Windows, same React HUD rendered through an offscreen CEF webview | [Releases](https://github.com/decentraland/bevy-explorer/releases/latest) |
| **Headless server** | Authoritative scene server, no rendering — drop-in replacement for `@dcl/hammurabi-server` | `@dcl-regenesislabs/bevy-headless-server` (npm) |

This project's goals are to:
- document current and future protocol standards
- experiment with changes to the protocol
- increase the field of alternative Explorers
- prioritize solid fundamentals, extensibility, and the use of modern open-source frameworks

## Repository layout

| Path | What |
| --- | --- |
| `src/` | binaries: `decentra-bevy` (client), `decentra-bevy-cef` (CEF render-process helper), `headless` (scene server) |
| `crates/` | the engine, split by domain (`scene_runner`, `comms`, `avatar`, `ipfs`, `dcl_deno`, `system_bridge`, …) |
| `react-web/` | the React DOM HUD — one codebase for both web and desktop; see [`react-web/README.md`](react-web/README.md) |
| `react-web/bridge-scene/` | headless SDK7 "super-user" scene that relays engine ↔ React over a `BroadcastChannel` |
| `deploy/web/` | the published web tree: engine boot module + workers + wasm, bridge scene, service worker |
| `deploy/headless/` | npm launcher + per-platform packaging for the headless server |
| `deploy/macos`, `deploy/linux` | desktop packaging (installer, AppImage) |
| `docs/`, `react-web/docs/` | design notes and backlog |

## Prerequisites

Common to every target:

- [rust](https://www.rust-lang.org/tools/install) (stable; the wasm build needs nightly — see below)
- [protoc](https://github.com/protocolbuffers/protobuf/releases) — `brew install protobuf`
- node 20+ (24 in CI) for the React HUD and bridge scene
- optionally [just](https://github.com/casey/just) — `just --list` for the dev entry points used below

Platform libraries (needed by the native and headless builds):

- **linux**: `sudo apt-get install --no-install-recommends libasound2-dev libudev-dev ninja-build clang cmake pkg-config libssl-dev libx11-dev libgl1-mesa-dev libxext-dev` plus ffmpeg dev packages (`libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev`)
- **macos**: `brew install ffmpeg@6 pkg-config ninja` and `export PKG_CONFIG_PATH=/opt/homebrew/opt/ffmpeg@6/lib/pkgconfig`
- **windows**: install [clang/LLVM](https://github.com/llvm/llvm-project/releases/) and set `LIBCLANG_PATH`; unzip [ffmpeg 6.0 shared](https://github.com/GyanD/codexffmpeg/releases/download/6.0/ffmpeg-6.0-full_build-shared.7z), set `FFMPEG_DIR` to its root and add `ffmpeg\bin` to `PATH` (ninja and cmake ship with visual studio)

## Web client

The engine is compiled to wasm and boots **in the React page's own document** (no iframe): the canvas sits behind the HUD, and the bridge scene relays between them.

Hidden tabs and an editor viewport covered by another preview run at up to 5 FPS. Returning
to the viewport restores the configured frame rate without changing saved graphics settings.

```bash
rustup toolchain install nightly-2026-04-15 --target wasm32-unknown-unknown --component rust-src
cargo install wasm-pack
just wasm     # builds the wasm into deploy/web/engine/pkg, then serves react-web and opens a browser
```

`just wasm` is the whole loop: `wasm-pack build` for the engine and for the scene runtime (`crates/dcl_scene_wasm`, into `pkg-scene`), re-bundle the sandbox worker (it inlines the wasm glue, so it must be rebuilt with the wasm), `npm install` in `react-web` and `react-web/bridge-scene`, then `npm run dev`.

Useful URLs once the dev server is up:

- `http://localhost:5173/` — real engine + live bridge-scene preview on :8100
- `http://localhost:5173/?mock=1` — full HUD on a fake bridge, **no engine build needed**
- `http://localhost:5173/?bundled=1` — engine loads the exported static bridge scene, i.e. exactly what ships

Requires a WebGPU-capable browser. Deployment topology (versioned CDN base, same-origin rules, COEP service worker) is documented in [`react-web/README.md`](react-web/README.md).

## Desktop client

The desktop build renders the same React HUD through an offscreen CEF webview (`react-hud-cef`, a default feature). Without the HUD bundle the app runs with **no UI at all**.

```bash
just setup-cef                 # once per machine: exports the CEF distribution to ~/.local/share/cef
export CEF_PATH=$HOME/.local/share/cef
# linux only, to run from the target dir:
export LD_LIBRARY_PATH=$CEF_PATH:$LD_LIBRARY_PATH

just native-release            # bundles the HUD if stale, then builds + runs everything
```

`just native-debug` is the same in debug. Both pass extra arguments through:

```bash
just native-release --realm https://realm-provider-ea.decentraland.org/main --position 52,-52
```

`--base-domain` retargets the backend hosts (auth, comms, places, worlds, social, ...) at another deployment's domain. `renderer-artifacts` (sdk6 adaption layer) and `builder-items` (inspector asset catalog) stay on decentraland.org — they have no other deployment:

```bash
just native-release --base-domain interconnected.online --position 0,0
```

On web the same thing is the `?baseDomain=` query param, e.g. `?baseDomain=interconnected.online`. Without the param, the hosting origin decides: a page served under decentraland.zone keys to zone backends, anything else to org.

One service at a time: each has its own flag taking a full base url that replaces that service's composition while the rest keep following the domain (`--help`, "Service endpoints"). The same names are web query params, e.g. `--catalyst http://localhost:3000` / `?catalyst=http://localhost:3000`:

```bash
just native-release --places http://localhost:5000 --realm-provider http://localhost:8000
```

Doing it by hand instead of via `just`:

```bash
./scripts/gen-ts-bindings.sh                     # TS types for the system API (generated, gitignored)
cd react-web && npm ci && (cd bridge-scene && npm ci) && npm run bundle:native && cd ..
cargo build --release --package dcl_deno_ipc     # scene runtime sidecar
cargo build --release                            # decentra-bevy + decentra-bevy-cef
cargo run --release
```

To build without CEF at all (the engine's own bevy-ui HUD instead of React):

```bash
cargo run --release --no-default-features --features "livekit,ffmpeg,inspect,social"
```

## Headless server

An authoritative scene server with no renderer — the SDK spawns it for scenes with `authoritativeMultiplayer` enabled.

```bash
npx @dcl-regenesislabs/bevy-headless-server --realm http://localhost:8000
```

From source (the binary is feature-gated out of default builds, and execs the sidecar from its own directory, so both land in `target/release`):

```bash
cargo build --release -p dcl_deno_ipc
cargo build --release --bin headless --no-default-features --features headless,livekit
./target/release/headless --realm <url> --location 0,0 --server-mode
```

`--orchestrated` runs it as a multi-scene worker driven over stdin/stdout instead. See [`deploy/headless/launcher/README.md`](deploy/headless/launcher/README.md) for the CLI contract and [`docs/headless-sdk-preview.md`](docs/headless-sdk-preview.md) for how it replaces hammurabi in the SDK preview.

## Arguments

`cargo run --release --bin decentra-bevy -- --help` lists every flag with its description.

The launch parameters shared with the web build — realm, spawn parcel, ui scene, portables, preview, editor, content server, pulse server, imposter source, base domain, fps logging, gpu bytes per frame — are declared once, in `crates/system_api_types/src/launch_options.rs`: each field is the native `--flag`, the `engine_run` options key and the entry-url query param, and the react page's parameter table is generated from it. Which of those a link may set without a warning is the page's own policy (`react-web/src/lib/launchGate.ts`). Native-only flags (rendering, imposters, debug, the `--builtin-*` ui pieces) live on `DecentralandArguments` in `src/lib.rs`.

## Testing

```bash
cargo test --all                      # engine
npm test --prefix react-web           # HUD, deterministic (vitest, no engine)
npm run test:e2e --prefix react-web   # HUD against a real engine (playwright, needs a GPU)
```

`react-web/review.md` is the pre-merge checklist for anything under `react-web/`.

## Agents and tests

The console (`/help` lists it; `window.engine_console_command` in the app frame on the web) is the
client's agent surface, the same in every build: end-to-end tests and people's own agents drive the
deployed clients through it, the HUD included, so nothing outside the client rebuilds the plumbing.
Replies are JSON where the command says so; a failed command rejects (web) or answers `ok: false`
(loopback), with no marker in the text. Arguments split on whitespace.

- State: `/world_state` (`inWorld`, `arrivals`, which grows on every return to the world, so a wait
  after a teleport reads it first), `/avatar_state [all|<address>]`, `/chat_history [count]`,
  `/voice_state`, `/entity_screen_position <entity> [--bounds]`, plus the older `/player_position`,
  `/get_user_data`, `/connected_players`, `/time` and the scene commands.
- The HUD, by role and accessible name (`react-web/src/lib/hudAutomation.ts`): `/hud_list [role]`,
  `/hud_read`, `/hud_click`, `/hud_fill` (`{"name", "text"}`), `/hud_wait` (`"state"`: `visible`,
  `hidden`, `enabled`, `actionable`; `"timeout"` ms). A selector is the name or JSON
  `{"name", "role", "index", "exact"}`; several matches need an `index`.
- Input: `/press_key <code> [--state down|up]` (`KeyboardEvent.code` names) and
  `/click_at <x> <y> [--button left|right|middle]`. Natively these and the HUD commands go through
  CEF and the window's own input, always real. On the web the page needs a DevTools client to make
  them real (below); without one they fall back to DOM events and say `"input": "synthetic"`.
- The preview-only commands (`/jump`, `/speed`, `/move_player_to`, the debug dumps) stay preview-only.

**The loopback console.** Every native build, the headless client mode included, carries a JSON-RPC
server on `127.0.0.1` that runs console commands from outside the process, off unless started with
`--automation-token <token>` (`--automation-port <port>`, 15702 by default; `DCL_AUTOMATION_TOKEN`
and `DCL_AUTOMATION_PORT` stand in for the flags). Two methods, none of Bevy's own: `dcl/console`
(`{"command": "/teleport 10 20", "token"}` answers `{"id"}`) and `dcl/console_result` (`{"id",
"token"}` answers `{"done": false}` until the command has answered, then `done`, `ok`, `output`).
Both need the token. The headless client registers `/world_state`, `/chat_history` and
`/voice_state`; the HUD, input and screen-position commands do not apply there.

**Trusted input on the web: a stable contract.** A DevTools client that drives the page (the runner
in `local-harness/web-driver`, or a Playwright or Puppeteer script through `exposeBinding` and its
own mouse and keyboard) installs `window.__dclTrustedInput(json)` in every frame of the page's
origin. The page keeps the finding and calls the hook with one JSON string, `{"id", "op", ...}`:

| `op` | fields |
| --- | --- |
| `click` | `x`, `y` (CSS pixels in the calling frame's viewport), `button`: `left`, `right` or `middle` |
| `key` | `code`, `key`, `keyCode` (Windows virtual key code), `text` (optional), `location` (0, 1 left, 2 right), `state` (optional, `down` or `up`; absent presses and releases) |
| `insertText` | `text` |

The client performs the input on the top-level page, offsetting `x` and `y` by where the calling
frame sits (the app frame is out of process), then evaluates `window.__dclTrustedInputDone(id,
error)` in the frame that asked, `error` a string or `null`. A key with `state` `down` stays held,
its modifier on every event until its `up`. The page waits 10 s for the answer, then fails the
command. Nothing else about the runner is part of the contract; `local-harness/web-driver/README.md`
documents that runner (Node 24 and Chrome, no dependency), and the end-to-end suite in
`apps/bevy-explorer/e2e` builds on it.

## CI and release channels

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) is the most accurate source of build information — it covers fmt, clippy, the test matrix, a headless smoke test, and the web build/deploy.

- **Web** — every push to `main` publishes `@dcl-regenesislabs/bevy-explorer-web` (npm + CDN) from `deploy/web`.
- **Headless** — `publish-headless.yml` publishes a snapshot on `next` per main push; releasing to SDK previews means dispatching it with `dist_tag=latest`.
- **Desktop** — `package.yml`, dispatched manually, cuts a GitHub prerelease (linux + windows; the macOS leg is disabled pending notarization secrets).

Built by DCL Regenesis Labs — dclregenesislabs.xyz

Powered by the Decentraland DAO
![Decentraland DAO logo](https://bafkreibci6gg3wbjvxzlqpuh353upzrssalqqoddb6c4rez33bcagqsc2a.ipfs.nftstorage.link/)
