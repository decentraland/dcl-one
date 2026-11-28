# Screenshots: capture and the PR body

- **Before / After, never base / head.** That goes for captions, file suffixes (`-before` / `-after`), grid columns (Before | After | Figma) and PR headings.
- **One script per environment, run at both ends**, so a pair can only differ because of the code. A PR body's Before is the PR's base commit, not the previous round's state.
- **Reset state before every shot, and log it.** Step order and starting state produce false findings: a 2D round trip earlier in a script reset Align to World, and a toggle that starts on was turned off by the shot's own click. A forced `:hover` can fail to register; move the real mouse.
- **Name the renderer and fixture per row.** The standalone inspector runs Babylon and Creator Hub runs Bevy, and some controls exist under only one. A state the fixture can't reach (a menu that needs scene info) is listed as such, or shot on a fixture that has it.
- **Evidence must be newer than the code.** Compare the shots' timestamps with the touched files' modification times before judging them.
- **Composites:** Creator Hub shots and grids come from `ch-capture` (below). Elsewhere, `montage -depth 8` (it writes 16-bit PNGs by default, twice the size); stack a pair vertically when the image is ≥ 4× wider than tall.
- **Upload without posting.** Uploading needs the user's approval first, asked through AskUserQuestion, as the architect's "PR actions" block requires (`.claude/agents/dcl-ch-feature-architect.md`); the same goes for any PR comment, reply or draft/ready change. `gh` can't upload images, so once approved you upload them yourself (user decision): attach them in the PR's comment box, harvest each `user-attachments` URL, clear the box, and never submit it. Upload in batches of 5–10, under 8 MB per call (the browser upload tool refuses more than 10 MB at once), and count the resolved links after each batch. GitHub silently stops accepting uploads after a burst of attachments: the tool still reports success, but no link appears. Plan a large set as batches of about 60 roughly two hours apart (one wait of 70 minutes after 66 uploads was not enough), and probe with a single file before each batch. GitHub drops each upload into the box as an `<img … src="https://github.com/user-attachments/assets/…">` tag, not markdown, so harvest the `src` attributes. Fill the body from the first batch right away and leave the missing slots as HTML comments, which render as nothing. After writing the body, check every image with a ranged GET (`curl -r 0-0`, expect 206; HEAD returns 403). A brand-new upload can 404 for a few seconds until a posted body references it. An upload stays reachable on the public repo even if the comment is never posted, which is safe because a shot only ever captures fixture data: never a signed-in profile's wallet, email or scene names.

## Capturing the render

- **Screenshots are ground truth.** `getComputedStyle` over CDP is unreliable for `:hover` / `:focus-within`. Capture hover and press states only after their transitions have finished, and in Electron launch with `--force-device-scale-factor=1`. On a Retina display, `page.screenshot({ scale: 'css' })` switches the page to DPR 1 for the shot. That re-layout drops the synthetic hover, so the shot shows the rest paint and the computed style reads rest right after it. With the flag, `scale: 'css'` has nothing to switch and the hover survives.
- **Compare the full window at Figma's frame size, DPR 1.** Size the window to the frame (1920 × 1080): Playwright `page.setViewportSize` or CDP `Emulation.setDeviceMetricsOverride`, then `screenshot({ scale: 'css' })`; launch Electron with `--force-color-profile=srgb`. A window capped by the display height (953 on a laptop) is not a valid capture. Never compare crops at mismatched scales. Compare side by side, as a 50% overlay, and as per-row pixel profiles, and report measured numbers (px, alpha, ratio), not impressions.
- **Render traps that move pixels against Figma:**
  - Variable Inter applies its optical-size axis automatically (`opsz` = font size), rendering 2–3% narrower than Figma. `font-optical-sizing: none` gives Figma's widths.
  - Fractional line heights (MUI h1 1.167 → 56.016px) push every rule below onto half pixels, smeared over two device rows. Pin line heights to whole px. `line-height: normal` is fractional too (a `<dl>` of Inter rows came out 98.5 tall, and the card around it 118.5, which left a 1px light line under a flush thumbnail): where Figma gives the frame a fixed height, set that height with `box-sizing: border-box`.
  - `border-collapse: collapse` borders straddle the cell edge and smear the same way. Use `separate` and an inset `box-shadow`.
  - Figma layer blur N = CSS `blur(N / 2)`.
  - A centre-constrained child (a modal's glow) is positioned from the parent's centre, not its top: `top: calc(50% + offset)`.
  - A frame's background can be a different image from the app's. Measure the wash row by row against the app's render rather than copying the layer.
- **An element screenshot of a panel taller than its scroll area lies.** Playwright captures the element's whole box: the part clipped by the scroll container comes out as a dark band, and whatever is painted over the rest (the docked asset panel) lands in the image — it reads as a layout overlap that isn't there. Before shooting a tall panel, raise the viewport until the element sits entirely inside every `overflow` ancestor, check that in the page, and refuse the shot otherwise; shoot the Before and the After at that same viewport. An all-dark shot of a panel is this artifact, not a design. The guard, in Playwright:

  ```js
  const fullyVisible = selector =>
    page.evaluate(sel => {
      const r = document.querySelector(sel).getBoundingClientRect();
      for (let n = document.querySelector(sel).parentElement; n; n = n.parentElement) {
        if (/(auto|scroll|hidden)/.test(getComputedStyle(n).overflowY)) {
          const p = n.getBoundingClientRect();
          if (r.top < p.top - 0.5 || r.bottom > p.bottom + 0.5) return false;
        }
      }
      return r.top >= 0 && r.bottom <= window.innerHeight;
    }, selector);

  if (!(await fullyVisible('.ui-designer-property-panel'))) throw new Error('raise the viewport');
  ```

- **One image per component, states as rows.** Capture each state separately, then compose one grid per component or surface: a row per state (idle, hover, active, selected, focus, disabled…), columns **Before | After | Figma**, each row and column labelled (ImageMagick `montage`, with `-font /System/Library/Fonts/Supplemental/Arial.ttf` or it fails here). A button recoloured across four states is one image, not four. Where Figma is silent for a state, that cell says so. The Figma image is a browser capture zoomed to selection, or `get_screenshot` when the day's budget allows.
- **Inside Creator Hub too.** Shoot the full list on the standalone inspector, and the key surfaces inside the app: Creator Hub in dev with `VITE_INSPECTOR_PORT` set to the port serving the inspector under test, signed out under a temp `HOME` and `--user-data-dir`, with `--force-color-profile=srgb`. The host sets the iframe width, the colour profile and the chrome around the panel, which the standalone server can't show. After a CSS change, a plain iframe reload in a running Creator Hub can keep serving the cached `bundle.css`; fetch it with a cache-busting query (or restart the app) before you judge the render. To shoot Before and After inside Creator Hub side by side, build one renderer per inspector port (`VITE_INSPECTOR_PORT` is baked in at build time) and pair each with the same `main/dist` and `preload/dist` in its own scratch app folder, since main loads the renderer relative to its own real path. Launch each with Playwright `_electron`, signed out under its own temp `HOME` and `--user-data-dir`.
- **Shooting a branch that has no fixtures.** When the data that reaches a state lives only on a fixture branch (a QA build), check out the fixture branch detached and bring in the files under test with `git checkout <branch> -- <path>`. Do it only after `git diff <fixture-branch> <branch-base> -- <path>` shows the fixture branch's copy of each such file equals the base's, so the shot can only differ by the change. List the files you brought in beside the shots.

## ch-capture: Creator Hub Before | After

[`scripts/ch-capture.mjs`](../scripts/ch-capture.mjs) shoots the built Creator Hub app from a states file and composes the grids. It launches Electron once per side and walks every state in that session. Measured on an M-series Mac with five states:

| Round | Time |
| --- | --- |
| First (cold): Before worktree, `npm ci`, full build | 147 s (47 s checkout and install, 87 s build, 13 s capture and grids) |
| Warm: Before reused, After rebuilt, `--only` one state | 19 s (13 s After build) |
| Capture only (nothing rebuilt), five states per side | about 10 s |

```bash
node .ai/skills/figma/scripts/ch-capture.mjs <states.mjs> --out <dir> --base <ref>   # both sides, all states
node .ai/skills/figma/scripts/ch-capture.mjs <states.mjs> --out <dir> --base <ref> --only home-es,update-available
node .ai/skills/figma/scripts/ch-capture.mjs <states.mjs> --out <dir> --side after   # no Before
node .ai/skills/figma/scripts/ch-capture.mjs --self-test                             # the Segment-key scanner
```

Run it from any directory. Grids are composed in Playwright's headless Chromium, so install it once: `npx playwright install --only-shell chromium` in `apps/creator-hub`. Keep `--out` outside the repo; the PNGs go in the PR's Screenshots section, never into git.

### States file

An ES module whose default export is the config. [`scripts/ch-capture.example.mjs`](../scripts/ch-capture.example.mjs) shoots the signed-out Home page in English and Spanish, the App Settings update check in three IPC-stubbed states, and the Create button at rest and on hover.

| Field | Where | Meaning |
| --- | --- | --- |
| `states` | config | The ordered list. A state's position names its files, `NN-<name>-<before\|after>.png`, and `--only` keeps those numbers. |
| `name` | state | Unique; what `--only` takes. |
| `component` | state | The grid it goes in (`grid-<component>.png`), one row per state. Defaults to `name`. |
| `ready` | state | Required. A Playwright selector for the state's own outcome. The shot waits for it, then for fonts and one animation frame. Never a sleep. |
| `steps` | state | `async ({ page, app }) => …`, run after the reset, to reach the state. |
| `ipc` | config, state | `{ channel: stub }`. A stub is either the exact value the main handler resolves with (the `{ success, value }` `IpcResult`), or a self-contained handler `(event, ...args) => result` (it is serialised into main, so no closures). A function can hang (`() => new Promise(() => {})`) or branch on the args. |
| `locale` | state | `es` or `zh`; English when absent. |
| `viewport` | config, state | Defaults to 1440 × 900. Use Figma's frame size when comparing against Figma. |
| `now` | config | An ISO date for a fixed clock (`page.clock.setFixedTime`). |
| `bin` | config | A folder put first on `PATH`, for fake agent CLIs. |
| `pin` | config | URL regexes to fetch once and replay to both sides (see below). Defaults to YouTube thumbnails; `[]` turns pinning off. |
| `setup` | config | `async ({ page, app }) => …`, run once per launch: network routes, fixtures. |

### What every round does

- **Before is built once per base commit.** `--base <ref>` is the PR's base. The first round checks it out as a detached worktree at `~/.cache/ch-capture/<sha12>` (`--base-dir` overrides that). It skips the LFS smudge, pulls only the asset-pack LFS objects, and runs its own `npm ci` and `make protoc build`. Never symlink `node_modules`. Later rounds reuse that build. Remove it with `git worktree remove <dir>` once the PR merges.
- **After is the checkout the script lives in.** It runs `npm run build` in `packages/creator-hub` only when a file under `main`, `preload`, `renderer` or `shared` is newer than the oldest of the three `dist` folders. Tests and specs don't count. A checkout with no inspector bundle gets `make protoc build`. Inspector changes are not tracked, so run `make build-inspector` yourself when the iframe changed.
- **Segment and Sentry stay off.** Builds run with every `VITE_SEGMENT_*` name and `VITE_SENTRY_DSN` blank, and with `DRY_RUN=1`, so Sentry gets no source-map upload either. Before each launch the script greps `main`, `preload` and `renderer/dist` for every Segment key value it can see, in the environment and in the After checkout's `.env*` files. A `dist` that holds one is rebuilt. If the key is still there after the rebuild, the run aborts. It can't catch a key it never sees, such as one exported only in another shell.
- **Capture rules are built in:**
  - a temp `HOME` and `--user-data-dir`, signed out, with `Scenes/` created before launch (otherwise the legacy-paths migration copies your real scenes in);
  - `--force-color-profile=srgb` and `--force-device-scale-factor=1` (without it, every hover shot on a Retina Mac comes out at rest), then `page.screenshot({ scale: 'css', animations: 'disabled' })` at the emulated viewport, with the caret hidden;
  - `SHELL=/usr/bin/true`, and `PATH` set to `bin`, then Node's own bin folder, then the system folders, so Volta's shim and your login shell's `PATH` stay out of it;
  - animated images frozen on their first frame (`webContents.setImageAnimationPolicy('noAnimation')`);
  - remote media pinned. Every URL matching `pin` is fetched once, saved under `<out>/.pinned/`, and replayed to both sides on every later round. The default pins YouTube thumbnails, whose CDN edges serve different JPEG encodes, which turned every Home pair into a false diff. Delete `.pinned/` to refresh them.
- **Each state resets without relaunching:**

  1. It swaps the state's IPC stubs into main and restores the previous state's originals.
  2. It reloads the renderer and waits for the navbar.
  3. It waits for the startup update check and the feature flags to settle, then clears the snackbar the update check always pushes. `updater.checkForUpdates` is stubbed to "up to date" unless the state overrides it; that snackbar used to cost a 20 s wait per launch. Flag-gated UI (the ANALYTICS tab) is only on screen once the flags land.
  4. It applies the locale, parks the mouse at 0,0, then runs `steps` and waits for `ready`.
  5. It settles: fonts, every `<img>`, CSS `background-image` and video poster decoded. Finite animations and transitions are finished and infinite ones cancelled, so hover and press states are captured after the settle step finishes their transitions, never mid-transition. Then two animation frames.

  Stubs are in place before the reload, so the startup check and the page's mount see them too. One unshot reset runs before the first state to warm the image decode cache.

- **Teardown kills Electron** (SIGKILL) instead of quitting it: the app's quit path waits for its own cleanup and telemetry flush, up to ~13 s, and the profile is thrown away anyway.

- **A failed state** leaves `_failed-NN-<name>-<side>.png`, the run carries on, and it exits 1 at the end.
- **Grids never go stale.** Every run rebuilds every component's grid from whatever PNGs are in `--out`. Each row is labelled `NN-name · locale · viewport`, and a missing side reads "not captured". Composing in Chromium needs neither ImageMagick nor a font path.

### Known gaps

- **The main window never localizes itself.** It never dispatches `translation/changeLocale`, and `--lang` does nothing. `ch-capture` finds the Redux store in the React fiber tree and dispatches the locale. Because components read `t()` once at render, it then remounts the route through the `MemoryRouter`'s navigator (an unmatched route, then back). Remote data such as template titles stays in whatever language the server sends.
- **The stub swap reads Electron's private `ipcMain._invokeHandlers`** to restore originals, and fails loudly if a future Electron drops it.
- **Full-window shots only.** For an element that can scroll, apply the fully-visible guard above in `steps` before trusting the shot.
- **Live data is live.** Feature flags and remote content come from the network. Both sides of one round see the same values, but a later round may not. Pin or route anything that has to hold still (`pin`, or `page.route` in `setup`).
- **Residual noise.** With Before and After on the same code, 15 of 21 pairs across three rounds were byte-identical. The rest, all on the Home page, differed in at most 141 px by at most 5 per channel. Judge a diff with a per-channel threshold above 5, never by byte equality.
