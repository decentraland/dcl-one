# Shop parity

Compares `apps/ui3/src/shop` with the Shop UI at the `decentraland/shop` revision pinned in
[`reference.json`](reference.json), page by page, on the same recorded data. The pin is the revision
production serves on the date it names; moving it means recording a new set against it.

```sh
pnpm install                           # playwright-core; then `pnpm exec playwright-core install chromium`
node reference.mts                     # check out and build the pinned Shop UI under state/
node capture.mts --build               # build apps/sites, compare every page and viewport, print the summary
node gate.mts                          # fail if a page marked identical is not
node gate.mts --promote                # mark the pages that are identical now
node inspect.mts item desktop dcl-one 'document.title'   # evaluate in one side's page
pnpm test && pnpm typecheck
```

Node 24+, with `apps/ui3` and `apps/sites` installed (`pnpm install` in each). Without `--build` the
existing `apps/sites/build` is compared (the report names the commit, `+uncommitted` when
`apps/ui3/src` or `apps/sites/src` differ from it). `--pages overview,items` and `--viewports desktop`
narrow a run. `CHROMIUM_BIN` picks the browser; both sides always use the same one. The three
loopback servers listen on 5176–5178 (`SHOP_PARITY_PORT_BASE` moves them) and answer from the
recording only through the harness's own browser routing; a page opened on them in another browser
calls the real services and is not what is compared.

## What a run does

- **Same data on both sides.** `recordings/<set>/` holds the API answers both UIs read, keyed by
  service, path, canonical query and request body. The reference shop is served from its build and
  every request it makes off its origin is answered from the recording; dcl-one runs as the built
  `apps/sites` tree (`sites-preview.mts`) whose node is a replay of the same recording, so SSR and
  the browser read it too. `/shop-api` paths map to upstream routes through
  `apps/ui3/src/shop/api/endpoints.ts`. Content pictures are deterministic placeholders of the
  recorded shape (at most 1024 px a side), telemetry is answered empty, WebSockets are closed, and
  nothing reaches the network. Clock, time zone, locale, `Math.random` and scale are fixed per
  context, the clock and `Math.random` of the server-side render too, and transitions are turned off
  before anything is measured. The sites preview and the reference build get an allowlisted
  environment, so a developer's own variables reach neither.
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
or that went to a host the harness does not know is declared by the page (`unrecorded`). `gate.json` lists the identical ones; the gate
fails when one of them stops being identical. An expectation records a difference, it does not
approve one: each names the rule or code behind it.

## Recording

```sh
node record.mts --set <date>           # read the answers once from production, anonymize them
node record.mts --set <date> --fill    # replay, read what was missing once, re-anonymize
node record.mts --set <date> --assets  # fetch the recording's web fonts (kept out of git)
```

Recording drives both UIs against the production services (public reads only). The raw answers
and the real-to-fake map stay in `state/shop-parity/raw/<set>/`. What is committed is anonymized:
addresses become `0xfa4e…` sequence numbers (ABI-aware in JSON-RPC, so numbers stay numbers),
signatures and salts same-length filler, content ids same-length stand-ins, people's names, bios,
NAMEs and ENS names same-shape ASCII pseudo-text in whatever script they were written, emails and
off-site links `example.org`, the account, community or world a first-party link points at a
stand-in, and birth dates zero; constants either app ships in its own code are kept. The write is
refused if any replaced value survives anywhere in the output, if a person's text comes out
unchanged, if JSON-RPC data is not in whole ABI words, or if any address, content id, NAME, email,
token or birth date is left that is not a stand-in or a bundled constant. dcl-one's
`POST /shop-api/v1/purchase-offers` has no upstream; while recording it is answered from the recorded
listing the way `services/catalyst/shop/src/mana/quote.rs` answers a signed-out primary trade.
