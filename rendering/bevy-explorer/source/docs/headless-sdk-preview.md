# bevy-headless as the local preview authoritative server

Replaces the Node `@dcl/hammurabi-server` that `sdk-commands start` spawns for
authoritative-multiplayer scenes, with the Rust `headless` binary from this repo.

## What the SDK actually requires

`sdk-commands start` (auth-server branch) spawns the server unconditionally for the first
project in the workspace:

```
npx --yes @dcl/hammurabi-server@next --realm=http://localhost:<port>
```

`cwd` is the project directory, `stdio` is `inherit`, and the child is killed with SIGTERM
on shutdown. That is the entire contract — `--realm` is the only argument, and the CLI
never parses the child's output, so there is no readiness marker to reproduce.

The scene room both servers join is **not** the `ws-room` fixed adapter from `/about`.
It is a LiveKit room minted by the comms gatekeeper, keyed on the scene entity id, which
the preview server derives from the scene path:

```
scene id : b64-<base64(<sceneDir>-<hostname>)>
room     : preview-<scene id>
```

Both implementations resolve the same id from the same realm, so they mint the same room.
(Consequence worth knowing: two previews of the *same directory* on different ports share
one room and will fight over the synced entities.)

## Running it

```bash
# what the launcher does under the hood
headless --realm http://localhost:8000 --preview --server-mode --location 0,0
```

`--server-mode` makes `isServer()` true for scene code; `--preview` selects the local
gatekeeper and disables the failed-asset backoff. The npm launcher translates hammurabi's
`--realm=<url>` form into exactly this.

## Testing it locally

Build the binaries and assemble the platform package once:

```bash
cargo build --release -p dcl_deno_ipc
cargo build --release --bin headless --no-default-features --features headless,livekit
node deploy/headless/build-platform-package.js --platform darwin-arm64 --version 0.1.0 \
  --engine target/release/headless --sidecar target/release/dcl_deno_ipc --out deploy/headless/dist
```

**The command on its own** — `npm link` the platform package into the launcher, then the
launcher globally:

```bash
(cd deploy/headless/dist/bevy-headless-server-darwin-arm64 && npm link)
(cd deploy/headless/launcher && npm link @dcl-regenesislabs/bevy-headless-server-darwin-arm64 && npm link)

bevy-headless-server --realm=http://localhost:8000
```

**The full SDK spawn path**, without publishing anything. `npx` takes a directory or a
tarball as well as a registry spec, so `DCL_SERVER_PACKAGE` pointed at `deploy/headless/launcher`
makes `sdk-commands start` install and run the local build exactly as it will run the
published one:

```bash
cd <your scene>
DCL_SERVER_ENGINE=bevy \
DCL_SERVER_PACKAGE=/path/to/bevy-explorer/deploy/headless/launcher \
  npm start
```

Look for `[Game] Running as SERVER` / `[Server] Ready` in the preview output, and confirm
the child is the engine rather than node with `pgrep -fl "headless --realm"`.

To verify a client syncs without needing a GPU, run a second engine as a plain client and
watch for `[Game] Connected to server`:

```bash
headless --realm http://localhost:8000 --preview --location 0,0
```

## Validation and measurement

Verify a pinned SDK preview with a server and a separate headless client. Exercise
join, bidirectional messages, authoritative CRDT state, a full scene lifecycle,
timer transitions and storage operations. Capture both sides' logs and identify
the exact revisions and scene inputs. An observed log message alone does not
prove the complete flow.

Measure the complete engine/sidecar process tree with the same hardware, scene,
client population and lifecycle window. Record CPU from cumulative-time deltas,
RSS, tick rate, startup latency and install size; distinguish platform and cache
conditions. Per-process RSS can double-count shared pages, and a sample without
clients is not comparable to one with clients. Choose a currently supported
profiler for the platform and record its invocation.

Save run results, benchmark values and remaining acceptance work in the task report.
Do not carry old measurements forward as evidence for a new revision.

## Packaging

`deploy/headless/` holds the publishable layout, built by root `.github/workflows/bevy-explorer-headless.yml`:

- `@dcl-regenesislabs/bevy-headless-server` — a ~10 KB JS launcher. Translates hammurabi's CLI, resolves
  the platform package, forwards signals, and exits `78` when the engine can't run here.
- `@dcl-regenesislabs/bevy-headless-server-{darwin-arm64,darwin-x64,linux-x64,win32-x64}` — the binary
  pair, selected automatically through `optionalDependencies` + `os`/`cpu` fields
  (the pattern esbuild uses, which `sdk-commands` already depends on).

The same package serves the multiplayer-server: the CLI passes `--orchestrated` through
(stdin control protocol, no realm), and the package's main export exposes
`resolveBinary()` for orchestrators that spawn the engine directly — replacing the
Dockerfile's build-from-source stage with an `npm install`.

Verified locally on darwin-arm64: both packages `npm pack`ed, installed into a clean
project, and `./node_modules/.bin/bevy-headless-server --realm=http://localhost:8100`
booted the scene as SERVER and exited 0.

The SDK side is one file on the js-sdk-toolchain `auth-server` branch
(`commands/start/hammurabi-server.ts`): a `DCL_SERVER_ENGINE=bevy|hammurabi` switch that
picks the package name, a `DCL_SERVER_PACKAGE` override so a local build can be spawned
without publishing, and a `close` handler that falls back to hammurabi on exit 78.
Default stays `hammurabi` until the bevy packages are published.

## Known gap: `--bevy-web` cannot reach a local realm

`sdk-commands start --bevy-web` opens the hosted client at
`https://decentraland.zone/bevy-web/?preview=true&realm=http://localhost:<port>`. That
client fails with *"World not found — the world isn't reachable right now"* for both
`localhost` and the LAN IP: an https page cannot fetch an http realm. It fails at the
client→realm hop, before any contact with the authoritative server, so it is unrelated to
which server implementation runs. Use the native explorer, or a headless client
(`headless --realm <url> --preview`), to exercise a local preview end to end.
