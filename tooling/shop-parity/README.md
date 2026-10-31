# Shop parity

Compares `apps/ui3/src/shop` with the Shop UI at the `decentraland/shop` revision pinned in
[`reference.json`](reference.json), page by page, on the same recorded data. The pin is the revision
production serves on the date it names; moving it means recording a new set against it.

The tool is committed; the data it compares is not. Recordings, the gate and every run stay under
`state/shop-parity/` (gitignored) on the machine that made them, so numbers quoted elsewhere name the
recording they were measured on.

```sh
pnpm install                           # playwright-core; then `pnpm exec playwright-core install chromium`
node reference.mts                     # check out and build the pinned Shop UI under state/
node record.mts --set <date>           # record the answers both sides read (see Recording)
node capture.mts --build               # build apps/sites, compare every page and viewport, print the summary
node gate.mts                          # fail if a page marked identical is not
node gate.mts --promote                # mark the pages that are identical now
node inspect.mts item desktop dcl-one 'document.title'   # evaluate in one side's page
pnpm test && pnpm typecheck
```

Node 24+, with `apps/ui3` and `apps/sites` installed (`pnpm install` in each). `--build` builds
`apps/sites` with an allowlisted environment, so none of the developer's `VITE_*` or deploy variables
is baked into the bundle, and stamps the build with what it was built from: the commit, a hash of
every uncommitted change that can reach the build (edits and untracked files anywhere in the
repository but this tool, and `apps/sites`' gitignored `.env*` files), and the Node version. Without `--build`, a
run refuses a build whose stamp is not the current sources. `gate.mts --promote` runs only on a fully
committed tree, this tool included. `--pages overview,items` and `--viewports desktop` narrow a run.
`CHROMIUM_BIN` picks the browser; both sides always use the same one. `pnpm test` needs no install.

The three loopback servers listen on 5176–5178 (`SHOP_PARITY_PORT_BASE` moves them). The replay
answers the sites preview only, server to server, and refuses any request a browser makes; the
preview refuses any request another site makes. So a website open elsewhere cannot make a recording
read production. A page opened on the preview or the reference in another browser calls the real
services and is not what is compared.

## What a run does

- **Same data on both sides.** `state/shop-parity/recordings/<set>/` holds the API answers both UIs read, keyed by
  service, path, canonical query and request body. The reference shop is served from its build and
  every request it makes off its origin is answered from the recording; dcl-one runs as the built
  `apps/sites` tree (`sites-preview.mts`) whose node is a replay of the same recording, so SSR and
  the browser read it too. `/shop-api` paths map to upstream routes through
  `apps/ui3/src/shop/api/endpoints.ts`. Content pictures are deterministic placeholders of the
  recorded shape (at most 1024 px a side), telemetry is answered empty, WebSockets are closed, and
  nothing reaches the network. The sites preview may connect to loopback only: a server-side module
  that names a production host of its own is refused, and the host is listed with the page as one
  the harness does not know. Clock, time zone, locale, `Math.random` and scale are fixed per context,
  the clock and `Math.random` of the server-side render too (one render at a time, each from the
  seed), and transitions are turned off before anything is measured. The reference build, the sites
  build and the sites preview get an allowlisted environment, so a developer's own variables reach
  none of them.
- **Structure.** Key elements per page (`pages.mts`), every `data-testid` element the two sides share
  and every visible text run are paired and compared: box (±1px, measured inside the nearest paired
  ancestor so a shift is reported where it starts), font family, size, weight, line height, letter
  spacing, colour, background, radius and text. One line each:
  `trending.card.price#0: font-weight 600→700, text "20"→"20 Credits"`.
- **Pixels.** Each page is taken as a user scrolls it (viewport tiles stitched at their scroll
  offset, fixed bars painted once, sticky elements at their place in the layout) and diffed perceptually (YIQ threshold 0.1, anti-aliasing
  tolerated). Masks cover only what cannot match between two renders (3D previews); regions cover a
  declared deliberate difference, and a structural difference inside a region takes its rule. Both
  are declared per page with their reason.
- **Report.** `state/shop-parity/runs/<time>/index.html`: per page and viewport the reference,
  dcl-one and diff images, the bands where pixels differ, the unexpected structural lines, the
  expected ones with their rule, console errors and any request the recording could not answer.

A page×viewport is identical when no pixel differs outside its masks and regions, every
structural difference is a declared expectation, and every request the recording could not answer
or that went to a host the harness does not know is declared by the page (`unrecorded`).
`state/shop-parity/gate.json` lists the identical ones and the recording they were found on; the gate
fails when one of them stops being identical. An expectation records a difference, it does not
approve one: each names the rule or code behind it.

## Recording

```sh
node record.mts --set <date>           # read the answers once from production
node record.mts --set <date> --fill    # replay, read what the replay could not answer, once
```

`--fill` sends each missed request as the page sent it, body included, and saves what it read even
when one read fails; the failures are listed and the exit code is 1.

Recording drives both UIs against the production services (public reads only) and keeps their
answers in `state/shop-parity/recordings/<set>/`. They are real public data (profiles, listings,
wallets), which is why they are never committed. An answer recorded as an error is listed when the
recording ends: it would replay as the same error on both sides and hide the page it feeds. dcl-one's
`POST /shop-api/v1/purchase-offers` has no upstream; while recording it is answered from the recorded
listing the way `services/catalyst/shop/src/mana/quote.rs` answers a signed-out primary trade.
