# Runtime component gallery

A review page for the hand-written runtime components that `@dcl/one-ui` exports, plus its foundations, and for the Explorer HUD's design primitives. It imports every component from the package root and loads `@dcl/one-ui/foundations.css`, as a consumer does. Each page renders its examples inside an iframe canvas, so the chosen width is the viewport that media queries read. Width presets follow the foundations breakpoints (`mobile` at 360px, `tablet`, `desktop`, `largeDesktop`), plus fill and a custom width.

The sidebar lists the foundations first, then the components grouped by kind, then the HUD, then a collapsed Figma reference section: the generated Review (`figma/review`), previews generated from the Figma capture for comparison, which apps never use. The generated Review itself is unchanged and keeps its own deep links. Only names present in both the runtime and Figma lists (Button, NavBar and others) carry a `runtime` or `Figma` label. Selecting an entry navigates in place (back and forward work), so the sidebar keeps its scroll position. A grouped export opens by its own name (`?component=EventCardHost` shows the EventCard page).

The heading of each group (Foundations, each kind of component, HUD) is a button that collapses and expands its list, and the button under the filter collapses every group, or expands them all once every one is collapsed. The browser remembers the collapsed groups (`localStorage`; without it they last for the visit). Opening a page expands the group that lists it, and a filter shows every match whatever is collapsed.

## Components Showcase

`Showcase`, the first entry under Components, draws every component page on one canvas (`src/frame/ComponentShowcase.tsx`): the groups in the sidebar's order, each page under its name, which opens the page itself, and its summary. It renders the registry's examples with the canvas's own example view, so a page and the Showcase cannot differ. A page whose content is positioned against the viewport (a `minHeight` in its entry: Modal, NavBar, Dropdown) is drawn in a canvas of its own inside the Showcase, loaded as it nears the window, so a dialog covers its own page only.

## HUD

The HUD section shows the Explorer HUD's Showcase (`apps/hud/design/Showcase.tsx`, `?showcase=1` in a HUD host), its single source: `Showcase` opens every section, and each section has a page of its own, named in `apps/hud/design/ShowcaseIndex.ts`. A component shown on another section opens it by its own name (`?hud=MenuRow` shows MenuPanel). The glyphs (`apps/hud/design/Glyphs.tsx`) have a page the gallery draws itself, every glyph, as it draws the foundations' icons: a Showcase section would keep the glyphs the HUD does not use in the HUD's design chunk.

Its pages render in their own document, `hud.html`, which loads what a HUD host's page does: the foundations, the HUD tokens and the HUD's page styles (`apps/hud/styles/page.css`, which both hosts import too). The gallery's Vite config gives the HUD the names its hosts give it (`@hud`, `@explorer-core`, `@hud-host`); the host is the library's own HUD test host (`apps/hud/test/host.ts`), a web host on decentraland.org. The packages the core signs with (`@noble/*`) are the hosts' to install, not the library's: the build leaves them out and fails if a page would load one, so the gallery signs nothing. The canvas is a HUD window: the chosen width and a 16:9 height (at least 480px), with the `--ui-scale` the hosts install for that size, so dialogs, toasts and type sized in `vh` behave as in a host, and the sections scroll inside it. `tsconfig.hud.json` checks this part with the HUD's compiler profile.

## Run it

A clean checkout needs, before the commands below:

- Node 24 or later (`engines` in `library/ui/package.json`) and pnpm through Corepack (`corepack enable`), then `pnpm install --frozen-lockfile` in `library/ui`.
- The library's Git LFS files: `git lfs pull --include "library/ui/**"`. The foundation fonts are LFS objects, and a checkout without them has pointer files in their place.
- Chrome or Chromium for `check-build.mjs`, which drives it. It looks in the usual install paths; set `BROWSER_PATH` (or `CHROME_PATH`) to the executable otherwise.

From `library/ui`:

```sh
pnpm exec vite --config gallery/vite.config.ts      # http://127.0.0.1:6020/
pnpm review                                          # optional: the Figma reference in development (own Vite cache, so both run side by side)
pnpm review:build --base ./ && pnpm exec vite build --config gallery/vite.config.ts
node gallery/scripts/check-build.mjs figma/review/review-dist [--prefix /nested/prefix/]
pnpm exec vite build --config gallery/vite.config.ts --mode hosted && node gallery/scripts/check-build.mjs gallery/dist --hosted
pnpm exec tsc -p gallery/tsconfig.json && node node_modules/typescript-7/bin/tsc -p gallery/tsconfig.hud.json
```

`--mode hosted` builds the gallery alone into `gallery/dist`, without the Figma reference: the build Sites serves and pull request previews publish.

The Sites image serves that hosted build at `/ui-gallery/` (`infra/packages/ui-gallery.nix`, [ADR](../../../docs/adr/2026-11-20-ui-gallery-on-sites.md)), so a deployment shows the gallery of the main commit it was deployed from. Its pages carry `noindex`.

The Shared UI library run puts that hosted build in the key-scanned Review artifact as `ui-gallery/`, and an image is published only when a trusted key scan read the same bytes it ships (`tooling/ui-gallery-receipt.py`). Both builds use Node 24 and this lockfile, so they stay byte-identical. `gallery/scripts/hosted-digest.mjs` prints the digest both sides compare and fails on a hidden path, which the artifact upload would drop. The HUD canvas keeps its Web Storage, cookies and the HUD's preferences in the page (`src/hud/isolateStorage.ts`); BroadcastChannel and same-origin access to the parent page remain.

The build writes `figma/review/review-dist/gallery/`, beside the generated Review, and uses relative URLs, so the combined output works at the root or under any prefix. Build the generated Review with `--base ./` (as CI does for the uploaded artifact) to serve the same output anywhere, or with the absolute prefix it is served from.

| URL                                       | Opens                                                      |
| ----------------------------------------- | ---------------------------------------------------------- |
| `gallery/?component=Button&width=375`     | a runtime page at a canvas width (`fill` when omitted)     |
| `gallery/?component=Showcase`             | every component page, in one canvas                        |
| `gallery/?foundations=icons`              | a runtime foundations page                                 |
| `gallery/?hud=Tabs&width=1280`            | a HUD Showcase section in a HUD window of that width       |
| `gallery/?hud=Showcase`                   | every HUD Showcase section, then the glyphs, in one window |
| `gallery/?source=generated&component=...` | the generated Review, with every other parameter forwarded |

## Review a pull request

Once publication is configured ([ADR](../../../docs/adr/2026-11-20-ui-gallery-previews.md), which also lists what is checked before a preview is published), a pull request from this repository that changes a file under `library/ui` gets one comment from `github-actions[bot]` whose **Open preview** button opens the hosted gallery built from that pull request. The line under it names the commit it was built from and the date the preview is available until, at least: five days after its upload. A preview runs the pull request's code on a host outside Decentraland's sites, so never connect a wallet or sign anything there, as the comment's warning says. The link follows the latest head: it updates a few minutes after the head's Shared UI library run and its key scan pass. Pull requests from forks get no link. The preview host serves every page under a Content-Security-Policy that admits only the gallery's own files (`PREVIEW_CSP` in `tooling/ui-gallery-publish.py`); the Shared UI library run opens the hosted build under that policy (`check-build.mjs --csp`), so a page that needs anything else fails the pull request.

Without a link, download the `ui-review-<sha>` artifact of the pull request's Shared UI library run, unzip it into a folder such as `ui-review`, run `python3 -m http.server 8080 --directory ui-review` and open http://localhost:8080/gallery/ (the gallery beside the Figma reference) or http://localhost:8080/ui-gallery/ (the hosted gallery the link opens).

## Coverage

`test/coverage.test.ts` enumerates every React component the package root exports (the foundations' icons have their own page) and fails when one has neither an example nor an entry with a reason in `exclusions` (`src/examples/index.ts`). A page lists the exports its examples render in `components`. Internal components are not exports: CardImage is shown through EventCard and CommunityCard. Examples use inline data only: no network, wallet or engine call.

`test/hud/coverage.test.tsx` does the same for the HUD's design entry (`apps/hud/design/index.ts`): every component it exports is named by a Showcase section, or excluded with a reason in `SHOWCASE_EXCLUSIONS`, and each section mounts the components it names through that entry. Its functions and constants are listed, each with the reason the rule leaves it out, in `SHOWCASE_NOT_COMPONENTS` beside the sections. The glyphs are the HUD's icon set: the gallery's Glyphs page draws every one. The HUD tests run in the `hud` Vitest project, under the harness the HUD's own tests use.

Covered: every component export of `@dcl/one-ui`, with the colors, typography, spacing, layout and icons foundations, and every component and glyph of the HUD's design entry. The two coverage tests keep it so, so this page lists no counts.

Not covered yet: the app compositions in `apps/shop`, `apps/sites` and the HUD feature screens.
