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
- `apps/ui3/src/play/engine/generated`: bindings from `crates/system_api_types`; regenerate with
  `tooling/play-bindings.sh`.

## The HUD

`apps/ui3/src/play` adapts `source/react-web/src` with these changes:

- `main.tsx` → `mount.tsx` (`mountPlayApp`), `shell/main.ts` → `shell/mount.ts` (`mountPlayShell`).
- Same-origin paths resolve against `/play/`: `features/engine/EngineHost.tsx`, `lib/systemScene.ts`
  (no `sdk-commands` live preview), `lib/publicUrl.ts` (`apps/ui3/public/play`).
- Off `decentraland.org`, service URLs come from the gateway's prefix table (`lib/baseDomain.ts`);
  `?baseDomain=decentraland.org` selects the Foundation deployments.
- `lib/editorEmbed.ts`: the Creator Hub editor frames `/play/` with its trusted scene, the session
  fragment becomes the sandbox channel suffix, the engine runs with `editor`, and no bridge is
  reported. The shell mounts its app once per document.
- Tests that import bridge-scene sources or `deploy/web` were not moved; the rest run with `pnpm test`
  in `apps/ui3` under `tsconfig.play.json`. `?native=1` is not built.
- The benchmark's unattended run is not carried: the `autopilot` boot mode (`lib/bootMode.ts`) and
  its sign-in in `features/session/useEngineSession.ts`, the shell's local guest
  (`shell/localGuest.ts`) and the stream params (`perfStream`, `perfCsv`). `/play` never runs the
  benchmark: `features/engine/EngineHost.tsx` keeps `autopilot` out of the engine's launch options
  on every host (the key is absent, not `false`, so an engine that predates the field still
  launches), and `perfStream` and `perfCsv` are never read. Neither HUD's `lib/launchGate.ts` has
  an entry for these switches: the Explorer's HUD ignores them except on loopback hosts and in
  native, `/play` ignores them everywhere, and `lib/entryParams.ts` does not list `autopilot`
  among the parameters a link can set. Nothing else of the run is carried either, because nothing in
  `/play` calls it: the drivers' `loginLocalGuest` and `autopilotFailed` (`engine/driver.ts`,
  `engine/EngineDriver.ts`, `engine/bridge.ts`), the shell's `createLocalGuest` with the root key it
  keeps (`shell/login.ts`, `features/auth/sso.ts`) and the helpers `features/auth/guest.ts` splits
  out for it, `lib/loopbackHost.ts`, the `__bevyPerfStream` page switch (`EngineHost.tsx`) and the
  bridge channel's `__engineExited` exemption (`engine/bridgeChannel.ts`; `boot.js` sets it only
  when the engine ends its own run under the autopilot, which `/play` never starts).

Open: `features/auth/sso.ts` expects the Foundation auth site's `/auth/login`, which Sites does not
serve; guest sessions work.

## Forks and drift

`tooling/upstream-forks.py` writes `forks.toml` (git sources, locked revisions, crates; ahead/behind
with `--check-upstream`) and `cargo-patches.toml` (`[patch.crates-io]` pins for
`cargo --config rendering/bevy-explorer/cargo-patches.toml …`; Bevy stays lock-pinned) from
`source/Cargo.lock`; `--check` fails on drift, `--verify` resolves a lock copy. The daily
`bevy-explorer-upstream.yml` runs it, opens the `upstream/forks` PR and uploads the drift page
(`tooling/upstream-drift.py`, `tooling/upstream-drift-site.py`).

## Native builds and HUD updates

Build the web engine with `make web` in `apps/bevy-explorer` (repository root).
For native builds, follow the [native build guide](docs/guidelines/running.md#native). Import with `python3 tooling/fetch-play.py --from-build apps/bevy-explorer/dist/web`
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
