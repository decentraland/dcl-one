# Runtime component gallery

A review page for the hand-written runtime components that `@dcl/one-ui` exports, plus its foundations. It imports every component from the package root and loads `@dcl/one-ui/foundations.css`, as a consumer does. Each page renders its examples inside an iframe canvas, so the chosen width is the viewport that media queries read. Width presets follow the foundations breakpoints (`mobile` at 360px, `tablet`, `desktop`, `largeDesktop`), plus fill and a custom width.

The sidebar lists the foundations first, then the components grouped by kind, then a collapsed Figma reference section: the generated Review (`figma/review`), previews generated from the Figma capture for comparison, which apps never use. The generated Review itself is unchanged and keeps its own deep links. Only names present in both lists (Button, NavBar and others) carry a `runtime` or `Figma` label. Selecting an entry navigates in place (back and forward work), so the sidebar keeps its scroll position. A grouped export opens by its own name (`?component=EventCardHost` shows the EventCard page).

## Run it

From `library/ui`:

```sh
pnpm exec vite --config gallery/vite.config.ts      # http://127.0.0.1:6020/
pnpm review                                          # optional: the Figma reference in development (own Vite cache, so both run side by side)
pnpm review:build --base ./ && pnpm exec vite build --config gallery/vite.config.ts
node gallery/scripts/check-build.mjs figma/review/review-dist [--prefix /nested/prefix/]
pnpm exec vite build --config gallery/vite.config.ts --mode hosted && node gallery/scripts/check-build.mjs gallery/dist --hosted
```

To review a pull request without building it, open the UI gallery link in its comment: `ui-review-preview.yml` builds the gallery alone (`--mode hosted`, into `gallery/dist`, without the Figma reference) and publishes it per branch. The same comment links the full Review artifact, which also has the Figma reference: unzip it into a folder such as `ui-review`, run `python3 -m http.server 8080 --directory ui-review` and open http://localhost:8080/gallery/.

The build writes `figma/review/review-dist/gallery/`, beside the generated Review, and uses relative URLs, so the combined output works at the root or under any prefix. Build the generated Review with `--base ./` (as CI does for the uploaded artifact) to serve the same output anywhere, or with the absolute prefix it is served from.

| URL                                       | Opens                                                      |
| ----------------------------------------- | ---------------------------------------------------------- |
| `gallery/?component=Button&width=375`     | a runtime page at a canvas width (`fill` when omitted)     |
| `gallery/?foundations=icons`              | a runtime foundations page                                 |
| `gallery/?source=generated&component=...` | the generated Review, with every other parameter forwarded |

## Coverage

`test/coverage.test.ts` enumerates every React component the package root exports (the foundations' icons have their own page) and fails when one has neither an example nor an entry with a reason in `exclusions` (`src/examples/index.ts`). A page lists the exports its examples render in `components`. Internal components are not exports: CardImage is shown through EventCard and CommunityCard. Examples use inline data only: no network, wallet or engine call.

Covered: all 45 component exports, on 39 pages, and the colors, typography, spacing, layout and icons foundations.

Not covered yet:

- The Explorer HUD design primitives in `apps/hud` (their own showcase is `apps/hud/App.tsx` with `?showcase=1`).
- The app compositions in `apps/shop`, `apps/sites` and the HUD feature screens.
