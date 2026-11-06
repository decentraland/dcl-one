# Explorer end-to-end suite

explorer-automation's Explorer tests restated against the Bevy Explorer: the InWorld scenarios,
the self-preview ones and the Visual suite, driven through the client's console on the web and on
the standalone, by `node --test`, with no browser-automation framework
(the Explorer readme's [Agents and tests](../../../rendering/bevy-explorer/source/readme.md#agents-and-tests)).
The sign-in flows
(explorer-automation's Auth and CrossVerify fixtures) are left aside.

The scenarios were copied once from `decentraland/explorer-automation` at `955b338` (2026-10-22)
and rewritten; no Unity code moved. Nothing here compares the two clients: every visual reference
is recorded anew in dcl-one.

**Status.** The driver and the tooling are covered by unit tests, which run in CI. The scenarios
have not yet run against the engine: that needs the Windows GPU runner and the zone accounts
(see [CI](#ci)). Until a run exists, every scenario's accessible names and waits are as read from
the HUD's source, not as observed.

## Run it

Needs Node 24, Google Chrome, and a web app build: the `bevy-explorer-web-<sha>` artifact of a
Bevy Explorer / CI run, or `make web` in `apps/bevy-explorer` (`dist/web`). The engine needs a GPU
and a visible window: Chrome runs headed, with the benchmark runner's flags
(`driver/chrome.ts`).

```sh
cd apps/bevy-explorer/e2e
npm ci
npm run typecheck                      # tsc
npm test                               # the driver's tests (a stub page in headless Chrome)
DCL_E2E_ACCOUNT_KEYS=0x... npm run e2e -- --inworld     # the InWorld scenarios against zone
npm run e2e -- --visual                # the Visual suite (build the scenes first, see below)
npm run e2e -- --filter 'opens the Backpack'            # a name pattern
npm run gpu-probe                      # the GPU the browser gives, started as the scenarios start it (chrome://gpu to report/)
npm run gpu-probe -- --adapter nvidia  # and once more on that adapter when the browser runs on another (Windows)
```

The suite serves the web app on loopback (`driver/serve.ts`, with the isolation headers the app's
own dev server sets per path: the shell plain, the app frame isolated on its own), starts Chrome
through the web driver (`rendering/bevy-explorer/source/local-harness/web-driver`), opens
`/?baseDomain=<domain>` (a realm or a position on the url would make the HUD launch the engine only
once a sign-in is submitted on its start page, and the sign-in here goes through the engine's
console; the realm and the parcel are reached by console afterwards), signs in with
`/login_identity` and the worker's account, presses the lobby's `Jump In` if it shows one, and
waits for `/world_state`
`inWorld`. One client serves every spec file of a run (`--test-isolation=none`); the Visual suite
boots a second one on its own realm.

| Variable | Meaning | Default |
| --- | --- | --- |
| `DCL_E2E_ACCOUNT_KEYS` | Zone test accounts' private keys, comma-separated, one per worker; never logged | none: the InWorld scenarios need one |
| `DCL_E2E_WORKER` | Which account this run takes | `0` |
| `DCL_E2E_BASE_DOMAIN`, `DCL_E2E_REALM`, `DCL_E2E_POSITION` | The backend, the realm (URL or world name) and the spawn parcel | `decentraland.zone`, `https://peer.decentraland.zone`, `100,100` |
| `DCL_E2E_WEB_APP` | The web app folder to serve | `apps/bevy-explorer/dist/web` |
| `DCL_E2E_WEB_URL` | An already served web app to open instead (zone's `/play` for a smoke) | unset |
| `DCL_E2E_WEB_PORT`, `DCL_E2E_REALM_PORT` | The loopback ports of the app and of the visual realm | `18743`, `18744` |
| `CHROME_PATH`, `CHROME_EXTRA_ARGS` | The browser, and more flags | the installed Chrome |
| `DCL_E2E_HEADLESS` | `1` runs Chrome headless (the engine then has no GPU: for the driver's tests only) | headed |
| `DCL_E2E_WINDOW` | The window size, `<width>x<height>` | `1600x900` (the Visual suite uses 960x540) |
| `DCL_E2E_RECORD` | `1` records a missing visual reference instead of failing | unset |
| `DCL_E2E_TRANSPORT` | `native` drives the standalone through `dcl/console` instead of a browser | `web` |
| `DCL_E2E_NATIVE_BIN`, `DCL_E2E_NATIVE_ARGS` | A standalone to start with its loopback console on (`--realm`, `--position`, `--automation-token` and `--automation-port` are added); the UUAV runtime set must be staged next to it (`tooling/uuav/stage-dist.py`) for scene video to play | unset |
| `DCL_E2E_NATIVE_PORT`, `DCL_E2E_NATIVE_TOKEN` | A running standalone's loopback console and the token it was started with | `15702`, unset |

To run the native Explorer against a gateway, set
`DCL_E2E_NATIVE_ARGS="--gateway https://gateway.example.test"`.
The [running guide](../../../rendering/bevy-explorer/docs/guidelines/running.md#gateway-routing)
describes the gateway layouts and service overrides.

The report goes to `report/`: `junit-<groups>.xml`, and per scenario `commands.jsonl` (every
console command, its reply or error and its time; `/login_identity` payloads redacted) and
`final.png` (the engine's frame as the scenario left it). A failed visual scenario adds
`frame.png`, `reference.png` and `diff.png`. A client that fails to boot, at the open, at the
console's coming or on the way into the world, leaves `boot-failures/boot-<n>.console.txt`, what
the page said, and `boot-<n>.jpg`, how it looked; it is booted once more for the next scenario,
and after a second failure every later scenario fails at once with that error, which the run logs
as it happens.

## How a spec is written

One file per explorer-automation fixture under `specs/`, headed by a table of its Unity tests
with the class of each: **Port** (the same scenario), **Partial** (an assertion changed, said
in the header), **Gap** (the Bevy HUD has no such feature or control) or **Drop** (a Unity
harness self-test). Each Unity test is one `scenario` (`driver/harness.ts`); a Gap or Drop is a
`todo` with its reason, counted apart in the report and never a pass.

A scenario reaches the client only through `Explorer` (`driver/explorer.ts`): console commands
with their replies parsed. It asserts through the engine where it can (`/world_state`,
`/avatar_state`, `/get_user_data`'s profile version after an equip, `/chat_history`) and through
`/hud_read` for what only the HUD shows. HUD controls are named by role and accessible name
(`specs/pages.ts` holds the pages); a control that appears on hover is reached with `hud.hover`
first. Every wait is bounded and says what it waits for; nothing asserts zone's live listings as
literals. A scenario that changes account state restores it.

The in-world scenario in `specs/debug-panel.test.ts` uses the lower-right bug button in development builds and Sidebar settings → Debug panel in release builds. It reads the current realm, observes a live FPS-target change and verifies an avatar action against the engine. The same spec reads `/client_login_state` for unsigned guest sessions and checks that authenticated
login delivery is inactive. The **Client login events** section is included in the registered-reading
check. `test/client-login-storage.test.ts` checks the browser queue's transaction acknowledgment,
abort handling, concurrent-instance merging, shared capacity, acknowledgment replay and restart read. `test/client-login-browser.test.ts` runs the production storage adapter in two independent
same-origin Chrome page contexts, recreates those contexts to check recovery, and aborts an actual
IndexedDB transaction. These checks do not substitute for running the debug panel against a built
Explorer. The chat scenarios in `specs/chat.test.ts` also check that a slash keypress and Unicode text reach the composer.

## Scenario map

explorer-automation's 89 Explorer tests outside the sign-in flows, by fixture; the class of each
test is in the spec file's header. Port 40, Partial 32, Gap 15, Drop 2.

| Unity fixture (tests) | Spec | Port | Partial | Gap | Drop | What changed |
| --- | --- | --- | --- | --- | --- | --- |
| ExplorePanel (7) | `explore-panel` | 6 | 1 | | | Pages instead of one panel; the top bar has no Communities tab |
| Shortcuts (7) | `shortcuts` | 5 | 1 | 1 | | Gallery is `G`, not `K`; no key opens Events |
| Navbar (4) | `navbar` | | 2 | 2 | | A fixed rail set, no flags; no help menu or controls panel |
| Minimap (5) | `minimap` | 2 | 1 | 2 | | No favourite heart or kebab menu; name and compass are unreadable text |
| Navmap (1) | `navmap` | 1 | | | | `Sports`, not `Sport` |
| Events (3) | `events` | | | 3 | | Two lists, no calendar, detail popup or Create button |
| Places (5) | `places` | | 4 | 1 | | Tabs and dropdowns without counters; a card jumps in, so no detail popup |
| Communities (4) | `communities` | | 3 | 1 | | No invites view, titles or counts; the detail is a modal |
| Chat (6) | `chat` | 2 | 4 | | | Escape leaves the input but keeps the panel; history checked through `/chat_history` |
| EmoteWheel (3) | `emote-wheel` | 2 | 1 | | | The emote playing is asserted through `/avatar_state` |
| Camera (3) | `camera` | | | 3 | | No photo mode or capture |
| Skybox (2) | `skybox` | | 2 | | | The slider has no name; the time is set with `/time` |
| BackpackWearables (3) | `backpack-wearables` | 2 | 1 | | | An equip is a draft until the Backpack closes; then the profile version and `/avatar_state` |
| BackpackEmotes (4) | `backpack-emotes` | | 4 | | | Slots read `<n> <emote>`; a base-only account has one page |
| BackpackOutfits (4) | `backpack-outfits` | 2 | 2 | | | No delete confirmation; two closes to commit a look |
| WearableVisibility (2) | `wearable-visibility` | | 2 | | | Thumbnails are not addressable; `rendered.wearables` by category instead |
| Passport (5) | `passport` | 1 | 4 | | | Header and bio are plain text; name and address from `/get_user_data` |
| SelfPreview (2) | `self-preview` | | | 2 | | No `--self-preview-wearables` launch option |
| ViewSignalSmoke (2) | `view-signal` | | | | 2 | A Unity harness self-test |
| Visual (17) | `visual/visual` | 17 | | | | The scenes at their own parcels; references recorded anew |
| UUAV PlayMode (unity-explorer's player tests) | `visual/video` | 2 | 1 | 4 | | One scenario on the `video` scene: the state reaches Playing, the clock runs, the frame is red; seek, audio capture and helper recovery stay in the UUAV workspace's tests |
| AudioEventsSystemShould (unity-explorer's audio event tests) | `visual/audio` | 3 | 1 | 3 | | One scenario on the `audio` scene: a clip reports Loading, Playing, Ready and its `playing` is written back false, a stream plays, an unreachable url errors; the stop, loop and video-only cases are engine unit tests |

What the HUD would need for more Ports, found while porting: roles or labels on the minimap's
name and coordinates, the chat message text, the communities titles and counts, the passport
header and bio, the skybox slider and switch; `aria-selected` and `aria-pressed` readable through
`/hud_read`; implicit roles for `nav`, `section` and `header` in `hudAutomation.ts`.

## Visual suite

`scenes/` holds the 17 SDK7 scenes, each at its own parcels ([scenes/README.md](scenes/README.md)).
`specs/visual/visual.test.ts` builds their entities, serves them as a realm on loopback
(`driver/realm.ts`: `/about`, `/content/contents/<hash>`), boots the client on it as a guest in a
960x540 window, teleports to each scene, waits for its entities and its settle time, and compares
the engine's frame (`/screenshot`, the canvas without the HUD) with `references/<id>.png`: a pixel
differs when any RGB channel differs by more than 8, and the scene passes when at most 0.3% of the
pixels differ (1.5% for `avatar-shape`), as explorer-automation compared.

`specs/visual/video.test.ts` plays the `video` scene ([scenes/README.md](scenes/README.md), after
unity-explorer's UUAV PlayMode suite) on the same realm (`specs/visual/realm.ts` serves every
workspace scene once per process) and asserts through the engine instead of a reference: the
player's `VideoEvent`s (`/inspect_component`) reach Playing without an error and report a running
clock, and the frame's pixel at the screen (`/entity_screen_position`) is the fixture's red. On
the web the player is the browser's video element; on the standalone it is UUAV, which the
scenes workflow runs (below).

`specs/visual/audio.test.ts` plays the `audio` scene the same way (after unity-explorer's
`AudioEventsSystemShould`): a clip that ends, a streamed tone and a stream nothing serves, and
asserts the `AudioEvent`s each entity receives and the `playing: false` the engine writes back into
the clip's `AudioSource` when it ends.

Build the scenes first with `apps/dcl-scene-cli`:

```sh
DCL_SCENE_CLI=/path/to/dcl-scene-cli npm run build-scenes
```

References are recorded only in CI ([references/README.md](references/README.md)): a manual run
of the workflow with `record` on uploads them as an artifact, and a PR commits them (Git LFS).
A reference counts as approved once a reviewer has checked it against the scene's source.

## CI

`.github/workflows/bevy-explorer-e2e.yml`:

- **Suite checks**, on every pull request that changes this folder, the workflow or its tooling:
  `tsc`, the driver's tests (the web transport against a stub page in the runner's Chrome) and
  the tooling tests. The workflow also listens to the explorer's paths, for the label below, and
  the job then passes without running them.
- **Scenarios**, as unity-explorer runs explorer-automation: on a pull request of this repository
  when a maintainer applies the `ci:e2e` label (the label admits the commit it is applied to, and
  that alone: a push takes it off and runs nothing, and the maintainer puts it back to run the
  new commit; a draft or a fork does not run), after each Bevy Explorer / CI run of a push to
  main, nightly against main's newest web app, and by hand (a commit, a group, record). A pull
  request's run comes from its own event, so it is one of the pull request's checks and an
  approval of the `e2e` deployment names its commit. `tooling/bevy-e2e-inputs.py` picks the web
  app: a pull request's job waits for the commit's Bevy Explorer / CI run and takes the web app it
  built (one that has expired is reported, never replaced by main's), or main's newest when the
  pull request changed no web input but changed the suite. The job downloads it, runs the InWorld
  scenarios, then builds the scenes and runs the Visual ones, and uploads `report/`. It runs on the Windows runner with a GPU that the
  `BEVY_E2E_WINDOWS_RUNNER` repository variable names (explorer-automation's
  `win-gpu-t4-explorer`, which an org admin enables for this repository), in the `e2e`
  environment, whose `DCL_E2E_ACCOUNT_KEYS` secret reaches only the two scenario steps. That
  image carries MinGit alone, so the job runs its steps in PowerShell, puts a pinned git-lfs on
  PATH before the checkout, and drives Chrome where the image has it, else a pinned Chrome for
  Testing build it downloads and checks against the sum the workflow records (the image's Edge gave
  WebGPU no adapter), with `--enable-unsafe-webgpu --ignore-gpu-blocklist`, the first of which the
  app's own GPU page asks for. A GPU probe step says first what GPU that browser gives, bare and
  with those flags (`npm run gpu-probe`), and when the browser runs on an adapter other than the
  NVIDIA one (the VM's paravirtual display adapter comes first, the GPU second), the scenarios get
  `--use-adapter-luid=<its LUID>` as well, so the browser's GPU process starts on the GPU. The
  scenes' CLI is built on a
  hosted Windows runner and handed over, so the image needs no Rust toolchain. The label
  is what admits a pull request's code to that runner and those accounts; the Inputs summary
  names the pull request and the commit under test; the job holds no write permission and saves
  no cache; required reviewers on the environment (a repository setting) add an approval on top.
  Until the variable is set the job is skipped.
- **Status comment**, after a pull request's scenarios: a job without secrets writes the
  *InWorld* and *Automation* sections of the PR's CI status comment from the uploaded report
  (`tooling/bevy-e2e-report.py`).

`tooling/bevy-ci.py` selects the scenes lane of `bevy-explorer-ci.yml` for a change to an
input of the video run below (the scenes, the driver, the runner scripts, the visual specs and
the package files) and no engine lane for the rest of this folder.

The standalone runs in `.github/workflows/bevy-explorer-scenes.yml` only: its graphical job
plays the video spec (`npm run e2e -- --visual --filter 'video plays'`) on the Linux release
build it made, through the loopback console under xvfb (`DCL_E2E_TRANSPORT=native`,
`UUAV_FORCE_SW_DECODE=1`: hosted runners have no VA driver), and uploads `report/` as
`bevy-explorer-video-e2e-<sha>`. That build has no HUD bundle: the standalone holds the world for
its welcome page until the first realm change, and with no `Jump In` to press the driver asks for
the realm through the console (`driver/session.ts`, `enterWorld`), which is what the button does. `tooling/bevy-ci.py` selects that job for the scenes, the
driver, the runner scripts, the video spec and its realm.

Not in this workflow yet: the standalone for the other specs, a macOS leg, and sharding across
accounts.

## Practices

Kept from explorer-automation's guides:

- Spend no wall-clock without a need behind it: a wait, a wider ceiling or another attempt must
  name the failure it prevents. Wait on the state a pause stands in for; a fixed sleep is right
  only where there is nothing to observe.
- A wait after an interaction waits for its effect, as a condition. A wait before one guards
  readiness. A ceiling routinely most spent is a scheduled failure: read the durations of passing
  runs before calling a scenario flaky.
- Never drive a flow with a double-click. A hover-revealed control is pressed after the pointer
  is on its parent.
- A skip reason is a claim: a Gap names the missing feature, and is re-checked against the HUD
  when that feature lands. No skip to paper over a pending rollout.
- A test that asserts flag-gated UI asserts the environment: the Bevy HUD reads no flags, so the
  rail's buttons are a fixed set.
- Tests that change shared account state (favourites, name, bio, equipped items, outfits) restore
  it in-test; a name longer than the editor's 15 characters cannot be restored.
- References are recorded only by CI, one PNG per scene, sizes must match exactly, and a missing
  reference fails. Diagnose from the report's artifacts, not from a video.
- One file per feature; each scenario starts with the panels closed and leaves them closed; no
  state shared between spec files through module globals; comments say why, in a line or two.

## Keeping the suite true

The suite and the console surface read the code they test (the repository's `AGENTS.md`). A
change on the left updates what is on the right in the same change, then runs the check.

| Changed | Update | Check |
| --- | --- | --- |
| A HUD control's role, accessible name, placeholder, `aria-*` state or visibility; a page's heading or tablist (`react-web/src`) | The selector in the spec (`specs/*.test.ts`, `specs/pages.ts`, `specs/backpack.ts`); the spec header's row and the scenario map above when a test's class changed; the HUD copy in `apps/ui3/src/play` and `hud_source_sha256` in `rendering/bevy-explorer/upstream.lock.json` (`python3 tooling/play-artifact.py hud-inputs --root .`) | `npm run typecheck`; the scenarios in CI |
| A key binding | The keys in `specs/pages.ts`; the shortcuts spec's header | the same |
| A console command's name, arguments or reply, in the engine or in `hudAutomation.ts` | `driver/explorer.ts` and `test/explorer.test.ts`; the Explorer readme's "Agents and tests" | `npm test` |
| `--automation-token`, `--automation-port`, their environment fallbacks, the `dcl/console` methods, or the `__dclTrustedInput` contract | `driver/native.ts`, `driver/web.ts`, `local-harness/web-driver` and its README; the readme section | `npm test`; the web driver's tests |
| A scene under `scenes/`, its parcels or spawn, `specs/visual/scenes.ts`, the 960x540 window or the tolerance | The references, re-recorded in CI (`record`) and committed in a PR; `scenes/README.md` | the visual scenarios in CI |
| The `video` scene, its fixture (`scenes/video/generate-fixture.sh`), the engine's `VideoEvent` reporting or the video player's source resolution (`crates/media/src/uuav`, `crates/av`) | `specs/visual/video.test.ts` (the states, the clock mark, the colour probe); `scenes/README.md`; the scenes workflow's video steps | `npm run typecheck`; the video scenario on the web (this workflow) and on the standalone (the scenes workflow) |
| The `audio` scene, its fixtures (`scenes/audio/generate-fixture.sh`), the engine's `AudioEvent` reporting or the AudioSource finish write-back (`crates/av/src/audio_events.rs`, `crates/av/src/video_player.rs`) | `specs/visual/audio.test.ts` (the state order, the write-back, the stream outcomes); `scenes/README.md`; the scenes workflow's media step | `npm run typecheck`; the audio scenario on the web (this workflow) and on the standalone (the scenes workflow) |
| A path moved under `apps/bevy-explorer/e2e`, `tooling/bevy-e2e-*.py` or the workflow | The workflow's `paths` filters, `tooling/bevy-ci.py` and `tooling/tests/test_bevy_ci.py`, the LFS patterns in `.gitattributes`, the links in this README | the tooling tests |
| The CI workflow's name, its job names, the `bevy-explorer-web-<sha>` artifact or the status comment's sections | `tooling/bevy-e2e-inputs.py`, `tooling/bevy-e2e-report.py` and their tests; the e2e workflow's `workflow_run` | the tooling tests |
| A scenario the HUD can no longer run, or a Gap whose feature landed | A `todo` with the reason, or the todo back into a scenario; its header row and the scenario map | the scenarios in CI |

```sh
cd apps/bevy-explorer/e2e && npm run typecheck && npm test
python3 -m unittest discover -s tooling/tests -p 'test_bevy_*.py'
```

The login-storage adapter and real-Chrome tests exercise corrupt/empty store recovery and mixed
valid/invalid records and additions as well as independent-context merge/ack behavior. Native Rust
storage tests also force a failed quarantine copy and verify that valid records are still repaired. The Rust `system_ui`
`login::client_login::tests` drive signed requests to a localhost stub for acceptance, rejection,
deployment-order retries and explicit memory-only delivery; they also check save-before-send,
account cancellation and storage recovery. These are focused runtime tests, not a built Explorer
signed-in journey. `/client_login_state` exposes `memoryOnly` and `storageFailures` for HUD diagnosis.

The Rust login sender regression also checks gateway routing: the HTTP request uses the public
`/social-api/v1/client-login` path while its signature binds the forwarded `/v1/client-login` path.
The direct-origin retry test checks that its signature retains the full request path. These tests
run in the Bevy CI native suite (`cargo test --all --profile ci-test --locked -- --test-threads=1`).
