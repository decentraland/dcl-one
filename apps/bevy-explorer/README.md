# Bevy Explorer app

Builds and publishes the Explorer in `../../rendering/bevy-explorer/source` (its own Cargo
workspace and `target/`); artifacts go to this app's ignored `dist/`.

## Build locally

Prerequisites: the [Explorer README](../../rendering/bevy-explorer/source/readme.md)'s platform
libraries, Rust, protoc, Node 24, npm. Web also needs `nightly-2026-04-15` with `rust-src` and
`wasm32-unknown-unknown`, and wasm-pack 0.15.0; desktop needs FFmpeg and the pinned CEF.

```sh
make web          # engine WASM, scene WASM, sandbox guard, HUD tests, bridge scene -> dist/web
make web-dev      # same, skipping wasm-opt
make headless     # headless engine + sidecar -> dist/headless/native
python3 build-headless.py --target x86_64-unknown-linux-gnu
```

For native prerequisites, CEF setup, HUD bundling and packaging, follow the checked-in
[desktop workflow](../../.github/workflows/bevy-explorer-desktop.yml) and
[native build guide](../../rendering/bevy-explorer/docs/guidelines/running.md#native).
Importing into Sites (production requires the main CI artifact):
[Explorer integration](../../rendering/bevy-explorer/README.md#native-builds-and-hud-updates).

CI caches WASM outputs in `.cache/bevy-engine`; a restore that fails its identity, inventory or
checksum check fails the build. Native CI uses Rust 1.99.1 and `protoc` 26.1; see
the current measurement evidence in the task report before claiming time savings.

Analytics: every environment reports to one Segment source, keyed by `BEVY_SEGMENT_WRITE_KEY` (a
repository variable; unset, the engine keeps the key it shared with the mobile client). Every event
carries `environment`, `prod` or `dev`, and the data stack filters on it. Only a run against
`decentraland.org` is `prod`: on the desktop a release build whose resolved `--base-domain` is
`decentraland.org` (a debug build or the zone staging domain is `dev`), on the web (always a release
build, one npm version on every site) a page whose host is exactly `decentraland.org` (any
subdomain, including the `cdn.decentraland.org` PR previews, `decentraland.zone` and localhost are
`dev`). The desktop workflow sets `BEVY_ANALYTICS_DEV=1` for a build of any ref but `main` or a tag,
so a QA build pointed at `decentraland.org` is `dev`. Setting the variable builds nothing by itself:
the key ships with the next web build on `main` and the next desktop `workflow_dispatch`. It is part
of the compiled engine's cache key, so that build recompiles the WASM.

## Actions and outputs

| Root workflow | Work | Outputs |
| --- | --- | --- |
| `bevy-explorer-ci.yml` | Selected engine checks and consumer build dependencies | Web app, tested Linux headless package, platform packages and APIs/Sites images, the `benchmark` runner for each *Test Suite* platform (Windows, macOS, Linux) |
| `bevy-explorer-benchmark.yml` | A pull request's web app against main's in Chrome on the laptop fleet: after each passed CI run of a PR that built the web app, or by hand ([Benchmark](../../rendering/bevy-explorer/docs/guidelines/benchmark.md)) | The *Performance* section of the PR's [CI status comment](#bevy-explorer-ci-status); the report in the job summary |
| `bevy-explorer-benchmark-leg.yml` | One laptop's leg, called by `bevy-explorer-benchmark.yml` | `benchmark-result-<label>` (run CSVs and logs) |
| `bevy-explorer-ci-status.yml` | After each `bevy-explorer-ci.yml` run of a PR that selected an explorer job | The *Build* and *Tests* sections of the PR's [CI status comment](#bevy-explorer-ci-status) |
| `bevy-explorer-scenes.yml` | Graphical scene tests (main pushes, nightly 03:17 UTC, manual) | Verdict, logs, screenshots |
| `bevy-explorer-desktop.yml` | Manual Linux/Windows AMD64 packaging | Desktop archives |
| `bevy-explorer-headless-build.yml` | Reusable platform compilation; Windows and macOS run beside Linux smoke | Engine + sidecar archives; platform npm tarballs |
| `bevy-explorer-headless.yml` | Publish the four validated platform packages after CI; explicit manual builds include Linux | Headless npm packages; manual prerelease |
| `apis-sites-images.yml` | Reusable image builds after the web producer; publish after complete CI | Tested AMD64/ARM64 APIs and Sites images |
| `bevy-explorer-images.yml` | Images from a successful main CI run | Web (Linux AMD64/ARM64), headless (Linux AMD64) |
| `bevy-explorer-rollout{,-manual}.yml` | Roll out at `decentraland.zone/play` after a CDN upload, or by hand | Rollout records |
| `bevy-explorer-upstream.yml` | Daily fork check and drift report | `upstream/forks` PR; drift artifact |

`tooling/bevy-ci.py` selects engine and image jobs per changed input. Unknown engine inputs and
manual Explorer dispatch select all engine checks. Image-only edits reuse a matching completed
web producer and do not select native compilation. On main, non-Linux headless packages build
alongside Headless Smoke; its stripped, tested Linux binaries also supply the Linux npm package.

Image builds use `needs` to start after the selected web producer completes. GitHub holds the
job without allocating a runner; imports and publication receipts use `--wait 0`. This moves
API compilation after the web producer too, so mixed backend/engine edits need elapsed-time
measurement as well as runner-minute measurement. Backend-only edits have no web dependency
work to wait for. Manual/tag image builds require an already available matching producer.

### Bevy Explorer CI status

A pull request that changes the explorer gets one `🚦 Bevy Explorer CI` comment, kept by the
[`bevy-explorer-ci-status`](../../.github/actions/bevy-explorer-ci-status/action.yml) action, with a section
per writer:

| Section | Written by | Holds |
| --- | --- | --- |
| *Build* | `bevy-explorer-ci-status.yml` | The web app and the headless engine with their artifacts, the web app's PR preview on the zone services, and the `benchmark` runner per platform |
| *Tests* | `bevy-explorer-ci-status.yml` | *Test Suite* per platform and *Headless Smoke*, with job times |
| *Performance* | `bevy-explorer-benchmark.yml` | After each passed CI run that built the web app: running, not run and why, cancelled, or the report with a badge from its *Overall* line |

`bevy-explorer-ci-status.yml` runs after each `bevy-explorer-ci.yml` run (`workflow_run`), in the
base context with write permission, and checks out only the action. It resolves the pull request
(`resolve-comment-pr.sh`: the run's own, else the open pull request whose head is the run's
commit) and whether the run's *Changes* job selected an explorer job (`explorer-run-selected.sh`:
*Test Suite*, *Headless Smoke*, *Clippy* or the web build not skipped). A run that selected none,
such as a backend-only change that CI runs for its image jobs, leaves the pull request without a
comment. When the run is requested, both sections go to pending; when it completes, they are
composed from its jobs and artifacts (`explorer-ci-sections.sh`). A cancelled run of a commit the
pull request moved past writes nothing. `automation` and `inworld` sections are allowed for suites
to come; they appear the first time one writes.

Each writer replaces only its section, strips marker-shaped lines from a body, truncates a section
over 12000 characters, re-reads to confirm the write and collapses duplicate comments
(`upsert-ci-status.sh`). The scripts' tests (`test-*.sh`, against a stubbed `gh`) run in CI's
*Integration guards*.

Image and headless publishers run on successful main CI completion. They verify the source
repository, workflow, SHA, attempt, complete platform matrix and artifacts before publishing.
The launcher takes its version from all four platform packages, never the callback run ID.
Unrelated newer commits do not supersede a component's release. Any failed consumer build
also fails the coordinated run and prevents automatic publication; graphical acceptance is
still a separate verdict. The standalone manual build/release entrypoints remain available.

For timing queries, image jobs now appear under `Bevy Explorer / CI` as `APIs + Sites / build (...)`;
non-Linux package jobs use `Headless packages / build (...)`. Linux package compilation is the
`Headless Smoke` job; count it once rather than adding a second compilation series.
Graphical tests do not run automatically on PRs or block builds; check their verdict and
readiness (evidence supplied for the task) before claiming graphical acceptance. Manual dispatch accepts a
chosen ref; use `--ref main` for main validation. The default `full` suite runs all 14 scenes;
`snapshots` selects three diagnostic scenes and cannot establish acceptance.

The graphical job has a 100-minute limit; scene execution has 20 minutes, leaving time to upload
logs and captures. Same-ref runs are serialized without cancelling the active run. Pushes compare
`before..head`; nightly dispatch retries all scenes even on unchanged main. Failed main assertions
still allow dependency-cache saves. Artifacts contain logs, screenshots and Cargo timings.

The harness retains player simulation but renders only 256×256 snapshot windows, skipping the
unused sky cubemap. Native UI is hidden and the background clock is frozen; scene lighting and
scene-authored time overrides remain active. Captures wait for completed render frames without
pending shader pipelines. Native video textures are held until GPU readback, then queued updates
resume. References keep their CPU pixels; invalid images fail comparison. References, thresholds
and allowed failures are unchanged; see measurements (evidence supplied for the task).

## Naming and publication

Components: `bevy-explorer-web`, `bevy-explorer-desktop`, `bevy-explorer-headless`, `bevy-explorer-benchmark`. Artifacts:
`<component>-image-linux-<arch>-<sha>` and `<component>-<os>-<arch>-<sha>` (`linux|windows|macos`,
`amd64|arm64`, full dcl-one SHA).

Images publish only from a successful main CI run to `QUAY_BEVY_EXPLORER_WEB_REPOSITORY` and
`QUAY_BEVY_EXPLORER_HEADLESS_REPOSITORY` (secrets `QUAY_USERNAME`, `QUAY_TOKEN`), tagged
`sha-<sha>`, `sha-<sha>-<arch>`, `main`; only products that succeeded in that run publish. Global `v*`
tags do not trigger Bevy CI. Resolve each product's tag independently; never derive one product's tag
from the other's revision. Pin deployments to `quay.io/<org>/dcl-one-<component>@sha256:<digest>`:
replays can overwrite tags. Desktop/headless `publish` creates a prerelease tagged
`bevy-explorer-<component>-v0.1.0-<run-number>`.

## npm packages

| Package | Published by | Contents |
| --- | --- | --- |
| `@dcl-regenesislabs/bevy-explorer-web` | `bevy-explorer-ci.yml`, web job | `deploy/web` addressed to `https://cdn.decentraland.org/<package>/<version>` |
| `@dcl-regenesislabs/bevy-headless-server` + `-darwin-arm64`, `-darwin-x64`, `-linux-x64`, `-win32-x64` | `bevy-explorer-headless.yml`, `publish` job | Launcher, engine and `dcl_deno_ipc` sidecar |

`decentraland/oddish-action` stamps `0.1.0-<run id>.commit-<sha>`: `next` from main, `latest` from a
manual headless run on main, a chosen `publish-tag` from a branch. npm trusted publishing (OIDC): each
of the six names must list `decentraland/dcl-one` and its workflow on npmjs.com. The CDN upload needs
`GITLAB_CDN_DEPLOYER_TOKEN` and `GITLAB_CDN_DEPLOYER_URL`. Retry with "re-run failed jobs".

Same-repo PRs publish under the `pr-<n>` dist-tag, which the CI status *Build* section links; a PR publish failure does not
fail the build, and closing the PR removes the tag (the trusted publisher needs "npm dist-tag"). The
automatic rollout takes only the `next` version, so `publish-tag` builds need "Manual CDN rollout". Before
archiving `decentraland/bevy-explorer`, verify one `next` of each package and re-adapt
`apps/ui3/src/play`.

## Container runtime

Web: port 8080, UID/GID 101; serves the app at `/` (a gateway must keep engine, bridge-scene and
service-worker paths same-origin). Headless: UID/GID 1000, writable `/data` and `/tmp`; supply realm,
scene, admission and deployment settings explicitly (no Catalyst or LiveKit included).

## Headless scene metrics

The native scene runtime samples counters and memory after completed updates, at
most once per five seconds. The headless emitter reports fresh observations about
every ten seconds as `@scene-stats` JSON, with `observed_at_ms`, `sample_seq`, an
execution identity and a monotonic `window_s`. Baselines belong to the scene
entity; restarted executions start fresh and stalled updates leave gaps.

`dcl_types` owns the shared counter payload. Native sampling and flushing run in
`dcl_deno`; web stats flush after the shared CRDT send returns, in the `dcl_wasm`
wrapper. The shared `dcl_js` operation updates counters but schedules no telemetry.

`cpu.run_ms` preserves fractional milliseconds and covers `onStart`/`onUpdate`
scene-thread CPU on Unix, including host-side fetch, websocket and renderer-response
work performed on that thread during those callbacks. It excludes other threads
and module loading. The first sample window includes module loading and the initial
CRDT send so startup counters have a matching interval; its CPU numerator still
covers only `onStart`/`onUpdate`. Non-Unix
builds omit CPU time and duration buckets because their runtime timer measures
wall time; update counts remain available. Heap usage,
V8 external memory and retained CRDT payload bytes are separate measurements,
not total scene RSS. Process RSS and cgroup memory are measured by the server.
Fetch status/duration/in-flight counts, open sockets, inbound comms and completed
update CPU buckets provide additional context.

Changes to `SceneResourceCounters` require publishing the headless engine and
`dcl_deno_ipc` sidecar from the same build, then updating the server's exact
`@dcl-regenesislabs/bevy-headless-server` dependency. Coordinate the server and telemetry schema rollout before updating that pin; resolve deployment-specific revisions and acceptance evidence from the current records supplied for the task. Review alerts on `scene_stats_reporting_scenes`: stalled
scenes stop producing samples and leave that count after 60 seconds. The server preserves source
windows through OTLP; receive-timed legacy observations cannot establish CPU
utilization. Metrics serialization, execution baselines and CPU bucket tests run with:

```sh
cd rendering/bevy-explorer/source # from the repository root
cargo test --locked --no-default-features --features headless --lib headless::stats
```
