# ADR: Scene hosts are untrusted processes

- Status: Accepted
- Date: 2026-11-09

## Context

On native platforms the Explorer engine runs scene code in a separate process, the scene host
(`dcl_deno_ipc`), which talks to the engine over a local socket. Every scene ran in one scene
host, a V8 isolate per scene. A scene that crashes V8 or escapes its isolate is in a process that
runs every other scene, and losing that process ends all of them. The headless server exits when
it is lost, so that its orchestrator restarts the engine and re-adds every scene.

## Decision

- **The engine trusts no scene host.** It validates every frame a host sends, ends a host whose
  frame it cannot read, and accepts from a host only responses for the scenes it gave that host
  and `SystemApi` messages from a host it gave a system scene
  (`rendering/bevy-explorer/docs/guidelines/security.md`).
- **A host never reaches the server's control channel.** The headless server's stdin and stdout
  carry the orchestrator protocol. A host gets an empty stdin, and its stdout and stderr are read
  by the engine and logged a line at a time, cut at 4 KiB and at 200 lines a second, with the
  host's pid; nothing a host prints reaches the engine's stdout.
- **The server can place each scene in its own host.** The engine starts scene hosts through a
  pool (`crates/dcl_deno_ipc/src/pool.rs`). The placement is `shared`, one host for every scene,
  or `per-scene`, a host for each scene that is never given another one. The headless server
  chooses it with `--scene-hosts`; `shared` is the default everywhere, and the desktop client
  always uses it.
- **A lost host ends only its scenes.** With `per-scene`, each scene of a lost host is sent an
  error, so it becomes broken, and the headless server reports the loss on its control channel
  (`scene-host-exited`) and keeps running. A later `add-scene` starts the scene in a new host.
  The engine exits only after three host starts in a row fail while no other host is running:
  starts run concurrently, so the run alone says nothing about the hosts that work. A removed
  scene's host is ended after the scene's kill grace period.
- **A per-scene host gets only what it needs.** It starts with a fixed V8 worker pool and an
  environment filtered to the variables it reads; on Linux it dies with the engine. The pool keeps
  warm spares so a new scene does not wait for a process, and it can end a host whose resident
  memory goes over a cap (`--scene-host-memory-mb`), outside the per-isolate V8 heap limit.
- **On Linux a scene host sandboxes itself, and the orchestrated server requires it.** The host
  applies Landlock and seccomp to itself (`crates/scene_host_sandbox`) after it connects and
  before it starts V8 or reads a scene, and reports the result in its first frame. It fails closed:
  a kernel that has a mechanism but refuses the ruleset is an error, and in `require` mode a host
  without seccomp and Landlock exits. The engine gives no scene to a host whose report is below
  the mode it asked for (`--scene-host-sandbox`, `require` by default with `--orchestrated` on
  Linux). The host can still open outbound TCP for fetch and WebSocket, and runs as the engine's
  user.

## Rejected options

- **A WASM or QuickJS runtime in the engine process:** interpreter speed, and a second JavaScript
  engine beside the V8 the client already runs scenes in.
- **One engine per scene:** the engine is the expensive part of the server; a scene host carries
  only V8 and the scene runtime.
- **More isolates in one process:** no memory or crash isolation between scenes.
- **The container's seccomp profile and capabilities alone:** they apply to the engine too, which
  must keep what a scene host must not have (its own files, other processes), and they are set
  by whoever runs the container, outside this tree.
- **An external wrapper such as bubblewrap:** another binary to ship and keep in step, user
  namespaces that container runtimes often disable, and a sandbox the host cannot report on.

## Verification

- `crates/dcl_deno_ipc/src/engine_ipc_tests.rs` plays a scene host against the engine side of the
  connection; `src/pool_tests.rs` drives the pool with in-memory hosts (placement, loss, removal,
  failed starts); `tests/scene_host_pool.rs` kills a real scene host and checks the other one
  keeps running. All run in the "Test Suite" job of `.github/workflows/bevy-explorer-ci.yml`.
- `src/headless/scene_hosts.rs` checks the shape of the `scene-host-exited` and
  `scene-host-sandbox` events.
- On Linux, `crates/scene_host_sandbox/tests/sandbox.rs` and `dcl_deno_ipc --sandbox-self-test`
  try each denied and each allowed action; `tests/sidecar_startup.rs` reads a real host's sandbox
  report, and `tests/sandboxed_scene.rs` runs fetch and WebSocket from a scene on a sandboxed
  host. "Headless Smoke" runs the self-test and the server with `--scene-host-sandbox require`.

## Consequences

Each `per-scene` host is a process with its own V8 platform, so memory and start-up cost grow with
the number of scenes. The orchestrator handles `scene-host-exited` and `scene-broken` as it
handles a broken scene today. With `shared`, losing the host still exits the engine. On a Linux
kernel without Landlock the orchestrated server refuses to run scenes unless started with
`--scene-host-sandbox off` or `on`; macOS and Windows hosts are not sandboxed.
