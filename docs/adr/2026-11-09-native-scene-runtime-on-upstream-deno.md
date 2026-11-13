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
- **Hardening:** an op never panics or exits on scene input (`security.md`). Every upstream op a
  scene can call is overridden with an op of the same shape, disabled, or on a reviewed list
  (`hardened_ops.rs`); structured clone uses our serializer, which has no host objects. Structured
  arguments go through `Plain<T>`, and op results are ordinary objects (`JsPlain<T>`), replacing
  the fork's `toString` patch. The V8
  callbacks scene code reaches without an op are ours (`isolate_hooks.rs`): stack formatting
  reads data properties only and runs no scene code, WebAssembly streaming compilation is
  removed, and WebAssembly results are settled without assuming the isolate is still running.
- **Memory per scene:** a 512 MiB V8 heap and 512 MiB of ArrayBuffers (a counting allocator);
  past either the allocation fails or the scene ends. Native memory that ops hold has its own
  caps: Blob parts 64 MiB (a clone or slice shares its parent's bytes), a scene-fed stream read
  whole 64 MiB, one stream's bytes 10 MiB on the server and 512 MiB on the client, 256 open
  streams and sinks; cancel handles stop at 4096 open resources; 32 MiB per non-UTF-8 decode.
  Not bounded per scene: resizable ArrayBuffers and WebAssembly memories, which V8 reserves outside the
  allocator; buffers ops return (a Blob read, a stream read whole), which wrap a Rust allocation;
  the timer and unref sets a client scene can grow through the raw ops; and fetch request and
  response bodies on the client.
- **Clocks:** `performance.now()`, the time origin, the timer clock, `Date`, `Temporal.Now`,
  `Intl.DateTimeFormat`'s current time and `File.lastModified` are coarsened to 2 ms; scenes
  share a process. This holds on the authoritative server only: a desktop client scene keeps
  `Deno` and its primordials, which read the native clock. It limits resolution only: there is
  no jitter, so a scene that counts work between clock ticks still measures finer intervals.
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
  overridden op, and `every_upstream_op_is_reviewed` when it adds an op on none of the lists.
- `runtime_surface_tests.rs`: Intl, console, op results as ordinary objects, 2 ms clocks, Blob
  and BroadcastChannel isolation and the Blob cap, the fetch redirect contract.
- `isolate_hooks_tests.rs`: the V8 callbacks and memory caps, each case that could end the
  process run in a process of its own.
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
  transport errors are `TypeError: fetch failed` with a `cause`, clocks are 2 ms granular,
  `String(opResult)` is `[object Object]`, `Error.prepareStackTrace` is not called, a Blob,
  File or DOMException structured-cloned (a `performance.mark` detail, a tee'd stream chunk)
  comes back as a plain object, index properties cannot be defined on `Array.prototype` or
  `Object.prototype` and `Array.prototype`'s prototype cannot be replaced (deno_core's rejection
  drain writes through them; a client scene can still reach them with its primordials), and
  `WebAssembly.compileStreaming`/`instantiateStreaming` are absent. New globals from V8 15:
  `Temporal`, `DisposableStack`, `AsyncDisposableStack` and `SuppressedError`.
