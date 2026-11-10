# ADR: The native scene runtime runs on upstream deno_core

- Status: Accepted
- Date: 2026-11-09

## Context

The Explorer runs scene JavaScript natively in one sidecar process, `dcl_deno_ipc`, which hosts
every desktop scene and the authoritative server's scenes. It ran on `robtfm` forks: deno_core
0.307, the Deno 1.46 extensions (net, fetch, tls, websocket, webstorage, console, url, web,
webidl) and fastwebsockets. That pinned V8 at 12.9 (Chrome M129). Only the forks' owner could move
them, and upstream deno_core accepted V8 15 (M150).

The sidecar is one trust domain: a panic or abort in it ends every scene and the engine with it.
Scenes can call every registered op directly with any arguments. The Deno extensions were built
for a trusted CLI, not for code that targets them: several ops unwrapped on scene input, deno_net
exited the process on an unstable-feature check, and deno_webstorage gave scenes a disk store.

## Decision

Run scenes on upstream **deno_core, deno_web and deno_webidl only**, with no forks, and own the
rest.

- **Dependencies:** deno_core with `include_icu_data` (Intl needs it), deno_web, deno_webidl and a
  pinned deno_error. No deno_net, deno_fetch, deno_tls, deno_websocket or deno_webstorage: deno_tls
  would pin rustls exactly and pull aws-lc, and the others were the sources of the aborts.
- **Fetch:** our own ops (permissions, public-address resolution, budgets, signed fetch) under the
  deno_fetch scripts, vendored. Redirects are followed in JS, so every hop goes back through the
  ops and their checks; https→http hops are refused; custom HTTP clients are removed.
- **WebSocket:** our own extension (`crates/dcl_deno/src/js/websocket.rs`): per-scene slots, an
  outbound cap that charges each message a fixed overhead, an inbound frame cap, the same address
  rules as fetch.
- **Scripts are compiled in.** The deno_web and deno_webidl scripts the runtime loads are vendored
  byte-identical under `crates/dcl_deno/src/js/vendor/` (MIT, versions and hashes in
  `upstream.lock.json`), so no build-machine path is read at run time.
- **Per scene, nothing shared:** a BroadcastChannel and a Blob store (capped at 64 MiB of distinct
  parts) per runtime; Web Locks and the broadcast ops disabled; the web classes whose constructors cannot be
  wrapped are not installed. No persistent storage of any kind.
- **Hardening:** an op never panics or exits on scene input (`security.md`). Upstream ops that do
  are overridden with ops of the same shape, structured arguments go through `Plain<T>`, and op
  results are ordinary objects (`JsPlain<T>`), replacing the fork's `toString` patch.
- **Clocks:** `performance.now()`, the time origin, the timer clock, `Date`, `Temporal.Now` and
  `Intl.DateTimeFormat`'s current time are coarsened to 2 ms; scenes share a process.
- **V8 platform:** initialized once on the sidecar's main thread, before any scene thread
  exists, so every isolate thread descends from it, as Linux memory protection keys require.
  Only `dcl_deno`'s own tests use the unprotected platform, under `cfg(test)`, never through a
  Cargo feature that could unify into a shipped binary.
- **Performance first:** V8 keeps its JIT; `--jitless` is an emergency lever only.

## Rejected options

- **Keep the forks and rebase them:** each V8 bump would wait on one person's branch, and the
  fork patches (`__raw_fn`, `toString`) would have to be carried forward.
- **Upstream deno_fetch and deno_websocket crates:** deno_fetch brings deno_tls (an exact rustls
  pin and aws-lc) and its HTTP client features; deno_websocket's ops unwrap on bad resource ids.
- **Loading the extension scripts from the crates at run time:** without a snapshot deno_core
  reads them from the path they had on the build machine, so a shipped sidecar can't start a
  scene anywhere else.
- **A process-wide BroadcastChannel or Blob store:** state, and SharedArrayBuffers, would cross
  scenes.
- **The unprotected platform through `unsafe_use_unprotected_platform`:** a dev-dependency feature
  unifies into every binary of the same `cargo test` build, including the sidecar the scene_runner
  tests start.
- **`--jitless`:** removes a class of JIT bugs at a large cost in scene frame time.

## Verification

- `cargo test --profile ci-test --locked -p dcl_deno -p dcl_js -p dcl_deno_ipc -p scene_runner -p common`.
- `invalid_op_input_tests.rs`: every registered op survives malformed arguments in every position;
  `every_override_replaces_an_op_of_the_same_shape` fails when an upgrade renames or reshapes an
  overridden op.
- `runtime_surface_tests.rs`: Intl, console, op results as ordinary objects, 2 ms clocks, Blob
  and BroadcastChannel isolation and the Blob cap, the fetch redirect contract.
- `embedded_upstream_scripts_match_the_locked_crates` and `every_runtime_script_is_compiled_in`;
  the scene tests also pass with the deno registry sources removed.
- `websocket_tests.rs`: caps, slots, close and error ordering, address rules.

## Consequences

- deno_core, deno_web and deno_webidl are bumped together, by the procedure in
  [contributing](../../rendering/bevy-explorer/docs/guidelines/contributing.md#upgrading-the-scene-runtime).
  The vendored scripts move with them.
- V8 trails Chrome stable by the gap between Deno's V8 and Chrome's (M150 against M157 at the
  time of writing). The nightly V8-age check reports it and opens an issue when we fall behind
  what deno_core accepts.
- Scene-visible changes: fetch follows up to 20 redirects with a permission check per hop,
  transport errors are `TypeError: fetch failed` with a `cause`, clocks are 2 ms granular, and
  `String(opResult)` is `[object Object]`.
