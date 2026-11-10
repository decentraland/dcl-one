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
- **The server can place each scene in its own host.** The engine starts scene hosts through a
  pool (`crates/dcl_deno_ipc/src/pool.rs`). The placement is `shared`, one host for every scene,
  or `per-scene`, a host for each scene that is never given another one. The headless server
  chooses it with `--scene-hosts`; `shared` is the default everywhere, and the desktop client
  always uses it.
- **A lost host ends only its scenes.** With `per-scene`, each scene of a lost host is sent an
  error, so it becomes broken, and the headless server reports the loss on its control channel
  (`scene-host-exited`) and keeps running. A later `add-scene` starts the scene in a new host.
  The engine exits only after three host starts in a row fail. A removed scene's host is ended
  after the scene's kill grace period.

## Rejected options

- **A WASM or QuickJS runtime in the engine process:** interpreter speed, and a second JavaScript
  engine beside the V8 the client already runs scenes in.
- **One engine per scene:** the engine is the expensive part of the server; a scene host carries
  only V8 and the scene runtime.
- **More isolates in one process:** no memory or crash isolation between scenes.

## Verification

- `crates/dcl_deno_ipc/src/engine_ipc_tests.rs` plays a scene host against the engine side of the
  connection; `src/pool_tests.rs` drives the pool with in-memory hosts (placement, loss, removal,
  failed starts); `tests/scene_host_pool.rs` kills a real scene host and checks the other one
  keeps running. All run in the "Test Suite" job of `.github/workflows/bevy-explorer-ci.yml`.
- `src/headless/scene_hosts.rs` checks the shape of the `scene-host-exited` event.

## Consequences

Each `per-scene` host is a process with its own V8 platform, so memory and start-up cost grow with
the number of scenes. The orchestrator handles `scene-host-exited` and `scene-broken` as it
handles a broken scene today. With `shared`, losing the host still exits the engine.
