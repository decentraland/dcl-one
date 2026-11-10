# Explorer end-to-end suite

explorer-automation's Explorer tests restated against the Bevy Explorer: the InWorld scenarios,
the self-preview ones and the Visual suite, driven through the client's console on the web and on
the standalone, by `node --test`, with no browser-automation framework
(the Explorer readme's [Agents and tests](../../../rendering/bevy-explorer/source/readme.md#agents-and-tests)).
The sign-in flows
(explorer-automation's Auth and CrossVerify fixtures) are left aside.

The scenario inventory is pinned to `decentraland/explorer-automation@955b338` (2026-10-22),
with scenarios implemented through this repository's console driver. Every visual reference
is recorded anew by dcl-one's CI against this client's rendering behavior.

**Status.** The driver and the tooling are covered by unit tests, which run in CI. The scenarios
run on the Windows GPU runner with the zone accounts ([CI](#ci)), and on a workstation with a GPU
and an account's key ([Run it](#run-it)); a run's report says which pass.

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
own dev server sets per path: the shell plain, the app frame isolated on its own; and the bundled
bridge scene's realm document with this origin's absolute content url, as Sites serves it, since
the build's root-relative one is no url to the engine and the bridge scene would never load), starts Chrome
through the web driver (`rendering/bevy-explorer/source/local-harness/web-driver`), opens
`/?baseDomain=<domain>` (no realm or position on the url: a destination there makes the HUD launch
the engine only once a sign-in is submitted on its start page, and the console the sign-in needs
comes up only with the engine; without one the engine launches at once, holding the world), signs
in with `/hud_login` and the worker's account (the HUD's own sign-in flow, which the page registers
only under a test runner driving the browser, the web driver's `__dclTrustedInput`: it takes the
identity as the page's stored sign-in and goes to the realm and the parcel from the lobby as a link
would; the engine's `/login_identity` alone leaves the HUD on its start page and the world held), waits
for `/world_state` `inWorld`, and corrects the parcel by console if the realm ignored it. The
sign-in accepts the terms the welcome page asks for; the name and look it carries are a first
login's, and an account that has its profile keeps it. The Visual suite signs in the same way as
a guest: a fresh key each run, marked as a guest for the engine, whose first login deploys a new
profile on the target's catalyst (the engine's own `/login_guest` stays local but leaves the HUD
on its start page). On the standalone the sign-in is `/login_identity`, and the client is started
at the realm and the parcel.
One client serves every spec file of a run (`--test-isolation=none`); the Visual suite boots a
second one on its own realm.

| Variable | Meaning | Default |
| --- | --- | --- |
| `DCL_E2E_ACCOUNT_KEYS` | Zone test accounts' private keys, comma-separated, one per worker; never logged | none: the InWorld scenarios need one |
| `DCL_E2E_WORKER` | Which account this run takes | `0` |
| `DCL_E2E_BASE_DOMAIN`, `DCL_E2E_REALM`, `DCL_E2E_POSITION` | The backend, the realm (URL or world name) and the spawn parcel | `decentraland.zone`, `https://peer.decentraland.zone`, `100,100` |
| `DCL_E2E_WEB_APP` | The web app folder to serve | `apps/bevy-explorer/dist/web` |
| `DCL_E2E_WEB_URL` | An already served web app to open instead (zone's `/play` for a smoke); its HUD must carry `/hud_login`, or the boot fails saying so | unset |
| `DCL_E2E_WEB_PORT`, `DCL_E2E_REALM_PORT` | The loopback ports of the app and of the visual realm | `18743`, `18744` |
| `CHROME_PATH`, `CHROME_EXTRA_ARGS` | The browser, and more flags | the installed Chrome |
| `DCL_E2E_HEADLESS` | `1` runs Chrome headless (the engine then has no GPU: for the driver's tests only) | headed |
| `DCL_E2E_WINDOW` | The window size, `<width>x<height>` | `1600x900` (the Visual suite uses 960x540) |
| `DCL_E2E_RECORD` | `1` records a missing visual reference instead of failing | unset |
| `DCL_E2E_TRANSPORT` | `native` drives the standalone through `dcl/console` instead of a browser | `web` |
| `DCL_E2E_NATIVE_BIN`, `DCL_E2E_NATIVE_ARGS` | A standalone to start with its loopback console on (`--realm`, `--position`, `--automation-token` and `--automation-port` are added) | unset |
| `DCL_E2E_NATIVE_PORT`, `DCL_E2E_NATIVE_TOKEN` | A running standalone's loopback console and the token it was started with | `15702`, unset |

To run the native Explorer against a gateway, set
`DCL_E2E_NATIVE_ARGS="--gateway https://gateway.example.test"`.
The [running guide](../../../rendering/bevy-explorer/docs/guidelines/running.md#gateway-routing)
describes the gateway layouts and service overrides.

The report goes to `report/`: `junit-<groups>.xml`, and per scenario `commands.jsonl` (every
console command, its reply or error and its time; `/login_identity` and `/hud_login` payloads
redacted) and
`final.png` (the engine's frame as the scenario left it). A failed scenario adds `page.jpg`, the
page as it looked, the HUD included (on the web the engine's frame has none), and `console.txt`,
the page's last 200 console lines; a failed restore adds the same as `restore-page.jpg` and
`restore-console.txt`. What the report keeps of the page's console, here and for a
boot, has its credentials replaced as the engine's own benchmark log does
(`crates/perf_output/src/scrub.rs`: an `access_token`, a named key or signature, an identity),
and a `Bearer` token. A failed visual scenario adds `frame.png`, `reference.png` and
`diff.png`. A client that fails to boot, at the open, at the
console's coming or on the way into the world, leaves `boot-failures/boot-<n>.console.txt`, what
the page said, and `boot-<n>.jpg`, how it looked; it is booted once more for the next scenario,
and after a second failure every later scenario fails at once with that error, which the run logs
as it happens.

## How a spec is written

One file per explorer-automation fixture under `specs/`, headed by a table of its test cases
with the class of each: **Port** (the same scenario), **Partial** (an assertion changed, said
in the header), **Gap** (the Bevy HUD has no such feature or control) or **Drop** (a Unity
harness self-test). Each test case is one `scenario` (`driver/harness.ts`); a Gap or Drop is a
`todo` with its reason, counted apart in the report and never a pass.

A scenario reaches the client only through `Explorer` (`driver/explorer.ts`): console commands
with their replies parsed. It asserts through the engine where it can (`/world_state`,
`/avatar_state`, `/get_user_data`'s profile version after an equip, `/chat_history`) and through
`/hud_read` for what only the HUD shows. HUD controls are named by role and accessible name
(`specs/pages.ts` holds the pages); a control that appears on hover is reached with `hud.hover`
first. Every wait is for a state the client reports, bounded by a ceiling and saying what it
waits for; none waits for time to pass, save the catalyst's deployment window below. Nothing
asserts zone's live listings as literals. A scenario that changes account state restores it.

What the driver does for every scenario, so a spec need not:

- **One client per session, closed when done.** A session (the account, a guest, the unsigned
  guest, the visual group's window) is booted on first use and kept for the scenarios that share
  it. A scenario whose session no later one uses closes it (`closeSession`), so no idle client
  renders beside the rest on the one GPU.
- **A rested HUD to start from.** Each scenario starts with Escape, a layer per press, until the
  rail's top button takes a click (a page hides the rail, a popup covers it), no panel beside the
  rail reads pressed, the rail's settings popover is closed and no field holds the keys
  (`/world_state`'s `keyboardClaimed`). The rail is read once it holds still (sliding in, it reads
  as covered with nothing open). Each press waits for the HUD to show it took effect (a layer
  gone, the rail done sliding back in, and after leaving the chat input the engine told of it)
  before the next. A HUD already at rest gets no Escape; a client with no HUD (the standalone
  built without it) gets none; a HUD still not at rest after four presses is left to the
  scenario, whose first step says what it found; any other error fails the scenario at its
  start. The web boot waits for the HUD's own world (the rail), which
  comes after the engine's.
- **Clicks that land.** `hud.click`, `hud.fill` and `hud.hover` wait for the HUD to call the
  control actionable: visible, enabled, uncovered and no longer moving. The HUD reports a control
  inside a running CSS animation or transition as `still moving` (pages slide and fade in, a
  card's button grows in), since a click aimed at it there lands where it no longer is, and one
  under the browser's pointer lock as locked to another element (photo mode's camera holds the
  pointer until the right button frees it), since every click then goes to that element.
- **The keyboard where the scenario expects it.** The HUD relays a text field's focus to the
  engine through the bridge scene, a few frames late, and `/world_state` says which side has the
  keys (`keyboardClaimed`). A real `hud.fill` returns once the field has them, so the next key is
  typed (Enter sends, Escape leaves the field); after an Escape out of a field, or a click that
  closed one, `keyboardReleased()` waits for the engine to have them back. `closePage` presses
  Escape twice only when a field holds the keys, and falls back to the page's Close button when a
  field keeps them. A client whose `/world_state` has no `keyboardClaimed` is older than the suite
  and is refused, not raced: run the suite against a web build of its own commit.
- **Lists that say when they are loaded.** The Backpack's `Catalog` grid and `Saved outfits`
  slots, and the Places page's `Places results`, are regions marked `aria-busy` from the moment
  their query changes (a filter, a page, the search box's debounce) until its answer shows; the
  specs wait for `ready` (`hud.waitReady`) and read the cards once. Lists the engine sends whole
  (the emotes, the communities) are read once they show.
- **Profile deployments apart.** The catalyst takes one deployment of a profile per address every
  3 s (`Entity rate limited`), a rule of the server's clock that the client cannot observe;
  closing the Backpack on a changed look and saving the passport are deployments, so they first
  wait out that window since the profile was last seen to change (`profileDeployWindow()`). A
  change and its restore would otherwise fail silently and leave the shared account changed.

The in-world scenario in `specs/debug-panel.test.ts` uses the lower-right bug button in development builds and Sidebar settings → Debug panel in release builds. It reads the current realm, observes a live FPS-target change and verifies an avatar action against the engine. The same spec reads `/client_login_state` for an unsigned guest (a session the engine's `/login_guest`
signs in, the guest a scene preview's `?guest=1` gets; the HUD's Continue as guest makes a signed
guest account instead) and checks that authenticated login delivery is inactive. The **Client login events** section is included in the registered-reading
check. `test/client-login-storage.test.ts` checks the browser queue's transaction acknowledgment,
abort handling, concurrent-instance merging, shared capacity, acknowledgment replay and restart read. `test/client-login-browser.test.ts` runs the production storage adapter in two independent
same-origin Chrome page contexts, recreates those contexts to check recovery, and aborts an actual
IndexedDB transaction. These checks do not substitute for running the debug panel against a built
Explorer. The chat scenarios in `specs/chat.test.ts` also check that a slash keypress and Unicode text reach the composer.

## Scenario map

explorer-automation's 89 Explorer tests outside the sign-in flows, by fixture; the class of each
test is in the spec file's header. Port 43, Partial 32, Gap 12, Drop 2.

| Fixture (tests) | Spec | Port | Partial | Gap | Drop | What changed |
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
| Camera (3) | `camera` | 3 | | | | The camera and the capture are the engine's (`/photo_mode`, `/take_photo`); the HUD opens and closes photo mode (the C key, the rail's Camera button) and shows the overlay's Shoot and Close |
| Skybox (2) | `skybox` | | 2 | | | The slider has no name; the time is set with `/time` |
| BackpackWearables (3) | `backpack-wearables` | 2 | 1 | | | An equip is a draft until the Backpack closes; then the profile version and `/avatar_state` |
| BackpackEmotes (4) | `backpack-emotes` | | 4 | | | Slots read `<n> <emote>`; a base-only account has one page |
| BackpackOutfits (4) | `backpack-outfits` | 2 | 2 | | | No delete confirmation; two closes to commit a look |
| WearableVisibility (2) | `wearable-visibility` | | 2 | | | Thumbnails are not addressable; `rendered.wearables` by category instead |
| Passport (5) | `passport` | 1 | 4 | | | Header and bio are plain text; name and address from `/get_user_data` |
| SelfPreview (2) | `self-preview` | | | 2 | | No `--self-preview-wearables` launch option |
| ViewSignalSmoke (2) | `view-signal` | | | | 2 | A Unity harness self-test |
| Visual (17) | `visual/visual` | 17 | | | | The scenes at their own parcels; references recorded anew |

Beyond explorer-automation, `specs/community-chat.test.ts` covers community text chat: a
member's community card opens its chat (`Open chat`), the rail shows a
pressed `Chat in <name>` tab and the title bar its `Conversation options`, and a message typed there
counts in `/community_state` and lands in `/dm_history community:<id>`. It needs an automation
account that is a member of a community, whose card it finds under My Communities (VIEW ALL), and
fails saying so otherwise.

What the HUD would need for more Ports, found while porting: roles or labels on the minimap's
name and coordinates, the chat message text, the communities titles and counts, the passport
header and bio, the skybox slider and switch; `aria-selected` and `aria-pressed` readable through
`/hud_read`; implicit roles for `section` and `header` in `hudAutomation.ts`.

## Visual suite

`scenes/` holds the 17 SDK7 scenes, each at its own parcels ([scenes/README.md](scenes/README.md)).
`specs/visual/visual.test.ts` builds their entities, serves them as a realm on loopback
(`driver/realm.ts`: `/about`, `/content/contents/<hash>`), boots the client on it as a guest in a
960x540 window, turns the nametags off and holds the landscape's trees still (`/nametags false`:
the scenes hide avatars, not the nametag over the hidden guest, whose name changes every run;
`/tree_wind false`: the trees and bushes sway in the wind, so no two frames of them agree),
teleports to each scene, waits for its entities and its settle time, and compares the engine's frame (`/screenshot`, the canvas without the HUD) with `references/<id>.png`: a pixel
differs when any RGB channel differs by more than 8, and the scene passes when at most 0.3% of the
pixels differ (1.5% for `avatar-shape`), as explorer-automation compared.

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

`tooling/bevy-ci.py` selects no engine lane for a change under this folder.

Not in this workflow yet: the standalone (`DCL_E2E_TRANSPORT=native` drives a native build
through its loopback console; no CI job runs one), a macOS leg, and sharding across accounts.

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

Connectivity diagnostics use `explorer.connectivity()` (`/connectivity_state`) and
`explorer.retryConnectivity()` (`/connectivity_retry`). The snapshot contains `connectionLost`,
`probing`, `everConnected`, `suppressed`, and `rooms` (`scope`, `state`, `retryAt`), without
credentials or raw server responses. The HUD exposes one `alertdialog` named `Connection Lost`,
with `Retry` and `Exit Application` buttons, and a `status` named `Multiplayer status` for scoped room
degradation. Retry is disabled and `aria-busy` while its command is pending. The dialog
follows engine state through retry, reopening or remaining available when a probe fails.
Recovery retracts it automatically; Exit follows session sign-out.

The suite and the console surface read the code they test (the repository's `AGENTS.md`). A
change on the left updates what is on the right in the same change, then runs the check.

| Changed | Update | Check |
| --- | --- | --- |
| A HUD control's role, accessible name, placeholder, `aria-*` state or visibility; a page's heading or tablist (`react-web/src`) | The selector in the spec (`specs/*.test.ts`, `specs/pages.ts`, `specs/backpack.ts`); the spec header's row and the scenario map above when a test's class changed; the HUD copy in `apps/ui3/src/play` and `hud_source_sha256` in `rendering/bevy-explorer/upstream.lock.json` (`python3 tooling/play-artifact.py hud-inputs --root .`) | `npm run typecheck`; the scenarios in CI |
| A list region's `aria-busy` or what sets it (the Backpack's `Catalog` and `Saved outfits`, `Places results`), or a HUD control animated other than by CSS (which `hudAutomation.ts` cannot see moving) | `CATALOG` and `SAVED_OUTFITS` in `specs/backpack.ts`, `RESULTS` in `specs/places.test.ts`; the HUD tests that pin the state (`backpackStates`, `placesBrowser`, `hudAutomation`) | the HUD's tests; the scenarios in CI |
| What `/world_state`'s `keyboardClaimed` reads (`InputPriorities`), or how the HUD relays a field's focus (`uiFocus`) | `keyboardTaken` and `keyboardReleased` in `driver/explorer.ts`, `closePage` in `specs/pages.ts`, `closeToRest` in `driver/harness.ts` and their tests | `cargo test world_state`; `npm test` |
| A key binding | The keys in `specs/pages.ts`; the shortcuts spec's header | the same |
| A console command's name, arguments or reply, in the engine or in `hudAutomation.ts`; what `/hud_login` does with the session (`useEngineSession.ts`: the stored sign-in, the welcome page, the lobby's pick) | `driver/explorer.ts`, `driver/session.ts` and their tests; the Explorer readme's "Agents and tests" and `docs/guidelines/running.md` "Test commands" | `npm test`; the HUD's tests (`react-web`: `npm test`) |
| `--automation-token`, `--automation-port`, their environment fallbacks, the `dcl/console` methods, or the `__dclTrustedInput` contract | `driver/native.ts`, `driver/web.ts`, `local-harness/web-driver` and its README; the readme section | `npm test`; the web driver's tests |
| A scene under `scenes/`, its parcels or spawn, `specs/visual/scenes.ts`, the 960x540 window or the tolerance | The references, re-recorded in CI (`record`) and committed in a PR; `scenes/README.md` | the visual scenarios in CI |
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
