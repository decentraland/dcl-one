# @dcl-regenesislabs/bevy-headless-server

Decentraland authoritative scene server, powered by the [bevy explorer](https://github.com/decentraland/bevy-explorer)
engine. A drop-in replacement for `@dcl/hammurabi-server`: same CLI contract, native binary
instead of Node + Babylon.

```bash
npx @dcl-regenesislabs/bevy-headless-server --realm http://localhost:8000
```

The Decentraland SDK spawns this automatically for scenes with `authoritativeMultiplayer`
enabled; you rarely need to run it by hand.

## Options

| Flag | Meaning |
| --- | --- |
| `--realm <url>` | Realm to serve. Required. |
| `--position <x,y>` | Parcel to load. Defaults to `0,0`. |
| `--production` | Production mode; disables preview-only behaviour. |
| `--tick-hz <n>` | Scene tick rate. Defaults to 30. |
| `--pulse-realm <key>` | Pulse realm to announce verbatim — the key sdk-commands mints for a local preview. Standalone only; an orchestrated engine takes one per scene on `add-scene`. |
| `--pulse-server <host:port>` | Pulse server to join. Defaults to the production server; a zone deployment passes the zone one. |
| `--base-domain <domain>` | Deployment domain the backend hosts (comms gatekeeper, worlds, ...) are composed from. Defaults to `decentraland.org`; a zone deployment passes `decentraland.zone`. |
| `--timeout <secs>` | Exit cleanly after N seconds. |
| `--scene-hosts <shared\|per-scene>` | `shared` (default): one scene host process runs every scene. `per-scene`: each scene runs in its own scene host process, so losing one ends only that scene. |
| `--scene-host-spares <n>` | `per-scene` only: scene hosts kept started with no scene, so a new scene does not wait for one. Defaults to 1. |
| `--scene-host-memory-mb <n>` | `per-scene` only: a scene host running a scene whose resident memory goes over this many MiB is ended and reported with reason `memory`; idle spares are not capped. No cap by default. |
| `--scene-host-sandbox <off\|on\|require>` | The sandbox each scene host applies to itself before it takes a scene, on Linux (see [Scene host sandbox](#scene-host-sandbox)). `on` applies what the kernel offers; `require` also refuses a host that is not sandboxed. Defaults to `require` with `--orchestrated` on Linux, `off` otherwise. |

`--scene-id`, `--private-key` and `--env` are accepted for hammurabi compatibility and ignored.

## Orchestrated mode (multiplayer-server)

`--orchestrated` starts the engine in multi-scene worker mode: no realm needed, scenes are
added and removed over stdin (JSON lines) with pre-minted comms adapters, and control
events come back on stdout with the `@bevy-ctl ` prefix. Scene host processes get an empty
stdin, and their output goes to the engine's log (stderr) a line at a time, so stdout carries
only the engine's own lines.

With `--scene-hosts per-scene`, a lost scene host process (it exited, closed its connection, or
the engine ended it over a message it could not accept) ends only its scenes. The engine prints

```
@bevy-ctl {"type":"scene-host-exited","scenes":["<hash>"],"code":<int|null>,"signal":<int|null>,"reason":"exit"|"protocol"|"memory"|"spawn"|"sandbox"}
```

before the `scene-broken` line of each of those scenes, and keeps running; a later `add-scene` for
the scene starts it in a new host. `reason` is `exit` when the process ended or closed its
connection, `protocol` when the engine ended it over a message, `memory` when it went over
`--scene-host-memory-mb`, `spawn` when it could not be started, and `sandbox` when it did not apply
the sandbox `--scene-host-sandbox require` asks for. The engine exits with status 1 only after
three host starts in a row fail while no other host is running (`78` when they failed the
sandbox). A removed scene's host process is ended once the scene's 5-second kill grace period is
over, and reported to nobody. A per-scene host starts with a fixed pool of two V8 worker threads
and an environment holding only the variables it reads (logging, the scene log budget, temporary
files, TLS roots, HTTP proxies, time zone and locale); on Linux it is killed when the engine exits.
With the default `shared`, losing the one host process exits the engine with status 1, as before.
The `starting` event names the mode in `sceneHosts`.

Orchestrators that manage the process themselves can skip the CLI and resolve the engine
path programmatically:

```js
const { resolveBinary } = require('@dcl-regenesislabs/bevy-headless-server')
spawn(resolveBinary(), ['--orchestrated'], { stdio: ['pipe', 'pipe', 'inherit'] })
```

### Scene host sandbox

On Linux each scene host sandboxes itself (Landlock and seccomp) after it connects to the engine
and before it reads a scene. Sandboxed, it cannot start programs, read or write files beyond the
system libraries, TLS and resolver configuration and time zone data, open Unix sockets, or
signal, trace or read the memory of another process. It still opens its own TCP connections for
fetch and WebSocket, and the address rules for those stay inside the host.

**With `--orchestrated` on Linux the default is `require`.** An engine that cannot start a
sandboxed shared host exits with status `78`; with `per-scene`, each host that does not report the
required sandbox ends its scene with `reason: "sandbox"`, and three in a row exit the engine with
`78`. `--scene-host-sandbox off` runs hosts unsandboxed. The kernel needs Landlock (5.13 or later,
enabled in the LSM list) and seccomp, and a container runtime whose seccomp profile allows
`landlock_*` and `seccomp` (the default profile of Docker 29 does). macOS and Windows hosts are not
sandboxed: `require` refuses to start there.

The `starting` event carries the mode, and the first host's report follows once (not with `off`):

```
@bevy-ctl {"type":"starting","realm":"...","sceneHosts":"shared","sceneHostSandbox":{"mode":"require"}}
@bevy-ctl {"type":"scene-host-sandbox","mode":"require","landlock":"fullyEnforced"|"partiallyEnforced"|"notEnforced"|"unsupported","seccomp":true,"noNewPrivs":true}
```

`partiallyEnforced` means the kernel enforces an older Landlock version than the host asks for.
On a sandboxed host a scene's `localStorage` is unavailable (it is a file) and the scene
inspector is off.

Check a machine or image before deploying, with the sidecar next to the engine:

```bash
dcl_deno_ipc --sandbox-self-test --require
```

It applies the sandbox to itself, tries each action the sandbox denies and each it must allow,
prints the result as one JSON line and exits `0` only if every check holds. In a container, do
not run it as PID 1 (`docker run --init`).

## How the binary is delivered

The engine ships as four platform packages (`darwin-arm64`, `darwin-x64`, `linux-x64`,
`win32-x64`) listed under `optionalDependencies`; your package manager installs only the one
matching your machine. Each contains two files that **must stay in the same directory** —
the engine execs its scene-runtime sidecar from its own location.

Set `DCL_BEVY_SERVER_PATH` to an absolute path to use a pre-installed engine instead
(useful for Electron hosts that bundle their own copy).

Exit code `78` means the engine is permanently unavailable here — unsupported platform,
missing binary, or bad arguments — so a caller can fall back to another implementation
instead of retrying.

## Linux runtime dependencies

The engine links the graphics/audio stack even when running headless:

```bash
sudo apt install libasound2 libudev1 libgl1 libx11-6 libxext6
```
