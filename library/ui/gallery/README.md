# Runtime component gallery

A review page for the hand-written runtime components that `@dcl/one-ui` exports, plus its foundations, and for the Explorer HUD's design primitives. It imports every component from the package root and loads `@dcl/one-ui/foundations.css`, as a consumer does. Each page renders its examples inside an iframe canvas, so the chosen width is the viewport that media queries read. Width presets follow the foundations breakpoints (`mobile` at 360px, `tablet`, `desktop`, `largeDesktop`), plus fill and a custom width.

The sidebar lists the foundations first, then the components grouped by kind, then the HUD, then a collapsed Figma reference section: the generated Review (`figma/review`), previews generated from the Figma capture for comparison, which apps never use. The generated Review itself is unchanged and keeps its own deep links. Only names present in both the runtime and Figma lists (Button, NavBar and others) carry a `runtime` or `Figma` label. Selecting an entry navigates in place (back and forward work), so the sidebar keeps its scroll position. A grouped export opens by its own name (`?component=EventCardHost` shows the EventCard page).

## HUD

The HUD section shows the Explorer HUD's Showcase (`apps/hud/design/Showcase.tsx`, `?showcase=1` in a HUD host), its single source: `Showcase` opens every section, and each section has a page of its own, named in `apps/hud/design/ShowcaseIndex.ts`. A component shown on another section opens it by its own name (`?hud=MenuRow` shows MenuPanel). The glyphs (`apps/hud/design/Glyphs.tsx`) have a page the gallery draws itself, every glyph, as it draws the foundations' icons: a Showcase section would keep the glyphs the HUD does not use in the HUD's design chunk.

Its pages render in their own document, `hud.html`, which loads what a HUD host's page does: the foundations, the HUD tokens and the HUD's page styles (`apps/hud/styles/page.css`, which both hosts import too). The gallery's Vite config gives the HUD the names its hosts give it (`@hud`, `@explorer-core`, `@hud-host`); the host is the library's own HUD test host (`apps/hud/test/host.ts`), a web host on decentraland.org. The canvas is a HUD window: the chosen width and a 16:9 height (at least 480px), with the `--ui-scale` the hosts install for that size, so dialogs, toasts and type sized in `vh` behave as in a host, and the sections scroll inside it. `tsconfig.hud.json` checks this part with the HUD's compiler profile.

## Run it

From `library/ui`:

```sh
pnpm exec vite --config gallery/vite.config.ts      # http://127.0.0.1:6020/
pnpm review                                          # optional: the Figma reference in development (own Vite cache, so both run side by side)
pnpm review:build --base ./ && pnpm exec vite build --config gallery/vite.config.ts
node gallery/scripts/check-build.mjs figma/review/review-dist [--prefix /nested/prefix/]
pnpm exec vite build --config gallery/vite.config.ts --mode hosted && node gallery/scripts/check-build.mjs gallery/dist --hosted
pnpm exec tsc -p gallery/tsconfig.json && node node_modules/typescript-7/bin/tsc -p gallery/tsconfig.hud.json
```

To review a pull request without building it, download the `ui-review-<sha>` artifact of its Shared UI library run, unzip it into a folder such as `ui-review`, run `python3 -m http.server 8080 --directory ui-review` and open http://localhost:8080/gallery/. `--mode hosted` builds the gallery alone into `gallery/dist`, without the Figma reference, for a hosted preview.

The build writes `figma/review/review-dist/gallery/`, beside the generated Review, and uses relative URLs, so the combined output works at the root or under any prefix. Build the generated Review with `--base ./` (as CI does for the uploaded artifact) to serve the same output anywhere, or with the absolute prefix it is served from.

| URL                                       | Opens                                                      |
| ----------------------------------------- | ---------------------------------------------------------- |
| `gallery/?component=Button&width=375`     | a runtime page at a canvas width (`fill` when omitted)     |
| `gallery/?foundations=icons`              | a runtime foundations page                                 |
| `gallery/?hud=Tabs&width=1280`            | a HUD Showcase section in a HUD window of that width       |
| `gallery/?hud=Showcase`                   | every HUD Showcase section, then the glyphs, in one window |
| `gallery/?source=generated&component=...` | the generated Review, with every other parameter forwarded |

## Coverage

`test/coverage.test.ts` enumerates every React component the package root exports (the foundations' icons have their own page) and fails when one has neither an example nor an entry with a reason in `exclusions` (`src/examples/index.ts`). A page lists the exports its examples render in `components`. Internal components are not exports: CardImage is shown through EventCard and CommunityCard. Examples use inline data only: no network, wallet or engine call.

`test/hud/coverage.test.tsx` does the same for the HUD's design entry (`apps/hud/design/index.ts`): every component it exports is named by a Showcase section, or excluded with a reason in `SHOWCASE_EXCLUSIONS`, and each section mounts the components it names through that entry. Its functions and constants are listed, each with the reason the rule leaves it out, in `SHOWCASE_NOT_COMPONENTS` beside the sections. The glyphs are the HUD's icon set: the gallery's Glyphs page draws every one. The HUD tests run in the `hud` Vitest project, under the harness the HUD's own tests use.

Covered: every component export of `@dcl/one-ui`, with the colors, typography, spacing, layout and icons foundations, and every component and glyph of the HUD's design entry. The two coverage tests keep it so, so this page lists no counts.

Not covered yet: the app compositions in `apps/shop`, `apps/sites` and the HUD feature screens.
