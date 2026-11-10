# Explorer integration

`source/` is the official Decentraland Explorer (engine, React HUD, desktop, headless server),
Apache-2.0, building in `source/target`. App builds, workflows and the
`@dcl-regenesislabs` npm packages live in [`apps/bevy-explorer`](../../apps/bevy-explorer/README.md)
([npm packages](../../apps/bevy-explorer/README.md#npm-packages)). Support links point at
[public Decentraland support](https://decentraland.org/help/).

Pins: `upstream.lock.json` `hud_source_sha256` fingerprints the source HUD reviewed for Sites
(required by the Sites importer and Nix package, not by Explorer builds or npm); the engine artifact
is identified by its producer commit in `/play/sites-play.json` (no npm engine pin).

## What runs where

- `/play`, `/play/app`: Sites routes (`apps/sites/src/routes/routes/play*.tsx`) mounting
  `apps/ui3/src/play`. `/play/app` sends COOP `same-origin` and COEP `credentialless`; the service
  worker keeps the `/play` shell unisolated so it alone holds the sign-in key.
- `/play/engine/`, `/play/bridge-scene/`, `/play/service_worker.js`, `/play/preview_realm.js`: the
  web artifact imported by `tooling/fetch-play.py` into `.cache/play/<manifest-hash>` (production:
  `build/client/play`, described by `/play/sites-play.json`).
- `library/ui/apps/hud/engine/generated`: bindings from `crates/system_api_types`, which both HUD
  hosts compile; regenerate with `source/scripts/gen-ts-bindings.sh` and commit the result.

## The HUD

The HUD UI both hosts share lives in `library/ui/apps/hud` (`@hud/*`); where they differ, it reads
the behaviour from `@hud-host`, each host's `host.ts` (`docs/ui-standards.md`, "Explorer HUD
transition"). The rest of `apps/ui3/src/play` adapts `source/react-web/src` with these changes:

- `main.tsx` → `mount.tsx` (`mountPlayApp`), `shell/main.ts` → `shell/mount.ts` (`mountPlayShell`).
- Same-origin paths resolve against `/play/`: `features/engine/EngineHost.tsx`, `lib/systemScene.ts`
  (no `sdk-commands` live preview), `lib/publicUrl.ts` (`apps/ui3/public/play`).
- Off `decentraland.org`, service URLs come from the gateway's prefix table (`lib/baseDomain.ts`);
  `?baseDomain=decentraland.org` selects the Foundation deployments.
- `lib/editorEmbed.ts`: the Creator Hub editor frames `/play/` with its trusted scene, the session
  fragment becomes the sandbox channel suffix, the engine runs with `editor`, and no bridge is
  reported. The shell mounts its app once per document.
- The native CEF HUD and its SDK bridge receive gateway service URLs from the desktop engine.
  CEF routes eligible snapshot images when displaying profiles and notifications (`@hud-host`'s
  `nativeProfileImage`, `react-web/src/host.ts`). Sites uses its web service configuration and the
  snapshot URLs returned by its backend (`apps/ui3/src/play/host.ts` returns them as given).
- The shared UI foundations: `react-web` loads `@dcl/one-ui/foundations.css` in `main.tsx`; `/play`
  does not, as its HUD adaptation stays as it is until Sites migrates (`docs/ui-standards.md`).
- The Shop's query cache: `react-web` wraps its Shop page in `features/shop/ShopQueries.tsx` (one
  client per account, remounted and emptied when another account or a sign-out replaces it); `/play`
  keeps the app-wide `QueryClientProvider` its in-world Shop uses until that Shop moves to `react-web`.
- Tests that import bridge-scene sources or `deploy/web` were not moved; the rest run with `pnpm test`
  in `apps/ui3` under `tsconfig.play.json`. Tests of the shared HUD UI live in
  `library/ui/apps/hud`, and `react-web`'s `npm test` runs them. `?native=1` is not built.
- The benchmark's unattended run is not carried. The shared session only knows that a host may
  have an unattended run (`@hud-host`'s `bootMode` and `unattendedRun`); react-web's `host.ts`
  supplies the autopilot's (`?autopilot` read by `lib/bootMode.ts`, the local guest's sign-in, the
  run's failure report and its Genesis landing), `/play`'s supplies none. Nor are the shell's local
  guest (`shell/localGuest.ts`) and the stream params (`perfStream`, `perfCsv`). `/play` never runs the
  benchmark: `features/engine/EngineHost.tsx` keeps `autopilot` out of the engine's launch options
  on every host (the key is absent, not `false`, so an engine that predates the field still
  launches), and `perfStream` and `perfCsv` are never read. Neither HUD's `lib/launchGate.ts` has
  an entry for these switches: the Explorer's HUD ignores them except on loopback hosts and in
  native, `/play` ignores them everywhere, and `lib/entryParams.ts` does not list `autopilot`
  among the parameters a link can set. Nothing else of the run is carried either, because nothing in
  `/play` calls it: the drivers' `loginLocalGuest` and `autopilotFailed` (`engine/autopilot.ts`,
  `engine/EngineDriver.ts`, `engine/bridge.ts`), the shell's `createLocalGuest` with the root key it
  keeps (`shell/login.ts`, `shell/localGuest.ts`) and the helpers `features/auth/guest.ts` splits
  out for it, `lib/loopbackHost.ts`, the `__bevyPerfStream` page switch (`EngineHost.tsx`) and the
  bridge channel's `__engineExited` exemption (`engine/bridgeChannel.ts`; `boot.js` sets it only
  when the engine ends its own run under the autopilot, which `/play` never starts).

Open: `library/ui/apps/hud/features/auth/sso.ts` expects the Foundation auth site's `/auth/login`, which Sites does not
serve; guest sessions work.

## Forks and drift

`tooling/upstream-forks.py` writes `forks.toml` (git sources, locked revisions, crates; ahead/behind
with `--check-upstream`) and `cargo-patches.toml` (`[patch.crates-io]` pins for
`cargo --config rendering/bevy-explorer/cargo-patches.toml …`) from `source/Cargo.lock`;
`--check` fails on drift, `--verify` resolves a lock copy. The daily
`bevy-explorer-upstream.yml` runs it, opens the `upstream/forks` PR and uploads the drift page
(`tooling/upstream-drift.py`, `tooling/upstream-drift-site.py`). Bevy is not among these forks:
it is vendored at `bevy/` and changed in place ([the bevy fork](docs/guidelines/bevy-fork.md)).

## Native builds and HUD updates

Build from `rendering/bevy-explorer/source` per its README (`just wasm` for the web engine), or
`make web` in `apps/bevy-explorer` (repository root). For native builds, follow the [native build guide](docs/guidelines/running.md#native). Import with `python3 tooling/fetch-play.py --from-build apps/bevy-explorer/dist/web`
or `python3 tooling/fetch-play.py` (main CI artifact); see the
[container build contract](../../infra/containers/README.md#sites-explorer-build-input).

When the source HUD changes: review it against `apps/ui3/src/play`, keep every adaptation listed in
"The HUD", regenerate bindings, run UI3 tests and typechecks, then record
`python3 tooling/play-artifact.py hud-inputs` as `hud_source_sha256`, rebuild the engine and repeat
the UI3 tests and typechecks. Record browser and multiplayer acceptance evidence for the target
revision in the task report; a matching fingerprint alone does not establish acceptance.

## Benchmark

A pull request that changes the explorer's web app is measured against main in Chrome on the laptop fleet:
`.github/workflows/bevy-explorer-benchmark.yml` takes both web apps (`bevy-explorer-web-<sha>`) and
the `benchmark` runner (`source/local-harness/benchmark`, `bevy-explorer-benchmark-<os>-<arch>-<sha>`)
from CI's artifacts and writes the report into the pull request's CI status comment. How a run
starts, how to read the report and how to set up a laptop: [Benchmark](docs/guidelines/benchmark.md).

## Authenticated login events

The Explorer queues a client-login event at the common successful identity completion for new
sign-in, previous-login restoration and browser-imported identities. It does not need an opened
social panel or social RPC connection. Unsigned guest mode does not emit authenticated events.
Each completion has a new UUID and epoch-millisecond occurrence time. Delivery sends an empty
`POST` to the configured `SocialApi` base plus `/v1/client-login`, with both values in the
ADR-44 signed metadata alongside `signer: "dcl:explorer"`. The server derives the wallet from
the auth chain.

The queue is separate from settings: `client-logins.json` in the native configuration directory,
or the browser's `dcl-client-logins` IndexedDB database. Each instance persists only its new
IDs and explicit removals. Browser transactions and native file locks merge those changes with
the latest stored state; a busy native lock defers persistence instead of blocking the worker.
Removal records remain for 24 hours and five minutes so an uncertain earlier save cannot restore
an acknowledged or discarded occurrence. The engine awaits the queue save before
sending. Empty/corrupt stores are repaired, with one `.corrupt` file or IndexedDB `corrupt`
record retaining the previous contents when quarantine succeeds. Native quarantine failure is logged
and does not block repair. Malformed records and additions are dropped individually while valid
records and acknowledgments survive. Three consecutive storage failures (ten-second timeout per asynchronous operation,
five seconds between retries) enable visibly degraded memory-only delivery without delaying login.
Unsaved changes remain in memory and storage recovery retries every minute. Normal persistence
gating resumes after recovery; events accepted in memory mode are removed during reconciliation.
Memory-only records can be lost on process/page shutdown. Each retry signs a fresh
request with the original event ID and occurrence time. A signing snapshot binds the event to
its wallet even if the current account changes. Logout cancels active delivery; pending records
resume when that wallet logs in to the same service again.

One request runs at a time, with a ten-second deadline covering signing and HTTP. Network
failures, HTTP 408, 429 and server errors use randomized exponential backoff bounded between
one second and five minutes. HTTP 204
acknowledges the event. HTTP 404/405 retain it with a randomized 150–300 second backoff so client
and server deployments can arrive in either order. Other client-error statuses stop its delivery
and log only the status code. Native response bodies are drained up to 16 KiB or one second for
connection reuse; browser fetch manages its connections. Response contents are never logged. Records expire after 24 hours. At most 32 pending records are retained per runtime and in shared storage;
adding one beyond the limit discards the oldest. The debug panel's **Client login events**
section and `/client_login_state` expose pending count, persistence, memory-only mode, consecutive storage failures, attempts, result and
session discard count. Concurrent processes and browser tabs share the durable queue without
replacing each other’s pending records; the shared capacity policy discards the oldest occurrence.

This producer covers the owned Bevy Explorer, including its imported web identity path.
Website-only sign-ins and external Explorer clients do not emit this event through this code.
