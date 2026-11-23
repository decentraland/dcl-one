# ADR: The native scene runtime runs on upstream deno_core

- Status: Accepted
- Date: 2026-11-09

## Context

The Explorer runs scene JavaScript natively in one sidecar process, `dcl_deno_ipc`, which hosts
every desktop scene and the authoritative server's scenes. It ran on patched sources: deno_core
0.307, the Deno 1.46 extensions (net, fetch, tls, websocket, webstorage, console, url, web,
webidl) and fastwebsockets. That pinned V8 at 12.9 (Chrome M129), while upstream deno_core
accepted V8 15 (M150). Each V8 bump meant rebasing every patch series onto a new Deno release.

## Decision

Run scenes on upstream **deno_core, deno_web and deno_webidl only**, with no patched sources,
and own the rest. Scenes keep the behaviour they had on 0.307 wherever the new crates allow it.

- **Dependencies:** deno_core with `include_icu_data` (Intl needs it), deno_web, deno_webidl and a
  pinned deno_error. No deno_net, deno_fetch, deno_tls, deno_websocket or deno_webstorage:
  deno_fetch and deno_websocket bring deno_net and deno_tls, which pins rustls exactly and pulls
  aws-lc.
- **Fetch:** our own ops (permissions, public-address resolution, budgets, signed fetch, custom
  clients) under the deno_fetch scripts, vendored. Redirects stay where they were: the client
  follows them in reqwest under the public-only redirect policy, and the authoritative server
  hands the 3xx back to the scene.
- **WebSocket:** our own extension (`crates/dcl_deno/src/js/websocket.rs`) on tokio-tungstenite
  and native-tls: per-scene slots, an outbound cap that charges each message a fixed overhead, a
  1 MiB inbound message cap, and the same address rules as fetch.
- **`localStorage`:** our own ops over SQLite, with deno_webstorage 0.163's script, database file,
  schema and 10 MiB limit, keyed by the signed-in player's address. Data written before the
  upgrade stays readable. deno_webstorage 0.255 exposes storage as a native `Storage` class whose
  methods take the raw key, so the per-player prefix has nowhere to go.
- **Scripts are compiled in.** The deno_web and deno_webidl scripts the runtime loads are vendored
  byte-identical under `crates/dcl_deno/src/js/vendor/` with the fetch and webstorage scripts
  (MIT, versions and hashes in `upstream.lock.json`).
- **Per scene, nothing shared:** a Blob store per runtime; the lock manager and broadcast ops are
  not registered, and the native BroadcastChannel stays our own bridge shim. deno_web's native
  classes are not installed.
- **Upstream ops that panic on scene input:** `op_compile_function` is disabled, and
  `op_structured_clone` (`performance.mark` and `measure` reach it) is replaced by a serializer
  with no host-object path (`structured_clone.rs`).
- **Op results** are ordinary objects (`JsPlain<T>`): serde_v8 builds them with a null
  prototype, so `String(result)` would throw.
- **Clock:** `performance.now()` keeps its 2 ms resolution (`clock.rs`).
- **V8 platform:** `JsRuntime::init_platform` on the sidecar's main thread; only `dcl_deno`'s own
  tests use the unprotected platform, under `cfg(test)`, never through a Cargo feature that
  could unify into a shipped binary. Each scene thread enters its tokio runtime before it creates
  its isolate, because V8 posts delayed tasks onto it.

## Rejected options

- **Keep the patched sources and rebase them:** each V8 bump would carry the patch series
  (`__raw_fn`, `toString`, the websocket and fetch patches) onto a new Deno release.
- **Upstream deno_fetch and deno_websocket crates:** they bring deno_net and deno_tls (an exact
  rustls pin and aws-lc), and deno_websocket would need its patches again for the connect-time
  address check and the inbound cap.
- **deno_webstorage 0.255:** see `localStorage` above.
- **Loading the extension scripts from the crates at run time:** the vendored copies are pinned,
  hashed and reviewed with each bump, and the shipped sidecar carries them.
- **The unprotected platform through `unsafe_use_unprotected_platform`:** a dev-dependency feature
  unifies into every binary of the same `cargo test` build, including the sidecar the scene_runner
  tests start.

## Verification

- `cargo test --locked -p dcl_deno -p dcl_js`.
- `runtime_surface_tests.rs`: Intl, console, op results as ordinary objects, `performance.now()`,
  per-scene BroadcastChannel, the vendored fetch contract, `localStorage` per player and over an
  existing database.
- `embedded_upstream_scripts_match_the_locked_crates` and `every_runtime_script_is_compiled_in`.
- `websocket_tests.rs`: caps, slots, close and error ordering, address rules.
- `client_follows_redirects` and `server_hands_redirects_back_to_the_scene`.

## Consequences

- deno_core, deno_web and deno_webidl are bumped together, by the procedure in
  [contributing](../../rendering/bevy-explorer/docs/guidelines/contributing.md#upgrading-the-scene-runtime).
  The vendored scripts move with them.
- Scene-visible changes: V8 15 adds the `Temporal`, `DisposableStack`, `AsyncDisposableStack`
  and `SuppressedError` globals; `String(opResult)` is `[object Object]` and op results inherit
  `Object.prototype`; `WebAssembly.compileStreaming`/`instantiateStreaming` are absent; the raw
  deno_net, deno_url and deno_console ops are gone from `Deno.core.ops` on the client; the
  deno_fetch 0.284 scripts replace 0.192's. The WebSocket follows the fetch private-network
  rule, uses the operating system's trust store, fails a socket that sends past its outbound cap
  (error, then close 1006), and reports its own counters in the scene debug panel.
