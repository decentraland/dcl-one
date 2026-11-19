# ADR: Collections, the item editor and curation live in Sites under `/create`

- Status: Accepted
- Date: 2026-11-19

## Context

Wearable and emote creators managed their collections in `decentraland/creators-website`, a
React 18 single-page app served at `decentraland.org/create` from the CDN. It talks to the
Foundation builder-api from the browser, with requests signed by the visitor's identity, and to
the chain through the visitor's wallet or the gasless relayer. Sites already served the `/create`
landing and, under `/create/wearables` and `/create/curate`, placeholder wizards over the Rust
builder service that covered only part of those flows.

## Decision

The creators pages move into `apps/sites` from `decentraland/creators-website` `main`
(`c625799e`), which stops taking changes:

- **Routes.** `/create/collections`, `/create/collections/:collectionId`,
  `/create/collections/editor`, `/create/curation` and `/create/live-preview`, one route file each.
  The placeholder wizards, their routes and the `/create/wearables` and `/create/curate` pages are
  removed without redirects: they never served production and nothing links to them. The
  `/builder/*` redirects point at the closest page that exists (an item link, which names no
  collection, opens the collections list). The pages share
  `features/creators/CreatorsShell.tsx`: the site chrome, the Create section tabs (which the landing
  shows too) and full-screen pages for the item editor and the live preview.
- **Layers.** Fetchers, flows and encoding go in `src/data/lib/creators/`, components and query
  hooks in `src/features/creators/`, following `boundaries.json`.
- **Rendering.** The server renders the frame and the tabs; each page loads its data in the
  browser, because builder-api requires requests signed by the visitor's identity. The curation tab
  appears once the page is idle, after the committee check.
- **Back ends.** The pages keep calling the Foundation services from the browser (listed under
  "Production APIs in use" in `docs/development.md`) until the Rust builder service implements the
  endpoints each flow needs. Feature flags are the exception: the pages read the `builder` and
  `dapps` flags through the site's `/internal/feature-flags` route, like the rest of Sites.
- **Session and chain.** The pages read the Sites session (`data/lib/auth`) and sign with its
  identity. A browser wallet sends transactions; email and social accounts sign typed data through
  the in-app signer and go through the relayer.
- **State.** Server data uses TanStack Query, with account data under `["private", account, …]` keys
  so the site's auth bridge clears it on sign-out and account switch. Client state uses small
  `useSyncExternalStore` stores.
- **Copy.** English only, through a local `t()` over one message file. Sites has no i18n to join.
- **Telemetry and errors.** Events are typed in the catalogue under the `bd_` prefix, one story per
  page (`creator-hub/<page>`), and the save, publish, curation and listing steps feed the creator
  funnel. Unexpected failures, failed queries and mutations included, are logged redacted with
  `console.error`, which the site reports to `/internal/client-error` like any other page's.
- **Styling.** CSS Modules, mostly over the shared `--dcl-*` tokens, with the creators base styles
  scoped to `[data-creators]`. Some colors are still the creators' own (`styles/tokens.css`): status
  colors, translucent fills at opacities the foundations lack, and shades carried over from the Shop. The pages keep their own components for now; moving them to `@dcl/one-ui` is a
  separate change.
- **Browser tests.** The mocked end-to-end suite runs as a local tool, `tooling/creators-e2e`
  (`playwright-core`, a fake wallet, chain and services, the Sites dev server). It is not part of
  CI. Unit tests run with the Sites suite.
- **Ownership.** The `web` area (`@decentraland/web-applications`), `tooling/creators-e2e` included.

## Consequences

- One pull request can change a creators page, the site chrome and the shared UI together.
- These pages depend on the Foundation services listed in `docs/development.md` until the Rust
  builder service covers them, one endpoint at a time.
- The landing's first load does not change: the creators code loads only on its own routes, and the
  curation check loads after the page is idle.
- History before the move stays in `decentraland/creators-website`.

## Follow-ups

1. Replace the local components with `@dcl/one-ui` (Button, Switch, Spinner, Skeleton, SearchBar,
   inputs, TabsBar), adding the variants the creators pages need.
2. Move each flow to the Rust builder service as its endpoints land.
3. Bring the server state fully under TanStack Query: the feature flags' module-level cache and the
   module-level copies in the roles, sales and discounts hooks become queries, and the inline query keys
   move to key factories (`docs/web-app-standards.md`).
4. Retire the creators environment files (`data/lib/creators/config/env`): the pages pick the
   development, staging or production Foundation services and chains from the page's host. Each
   service leaves them when it moves behind the node and is reached like the rest of Sites; once the
   chain choice is deployment configuration too, the files and the host detection go.

5. Move the creators colors onto `--dcl-*` tokens, adding the status colors and the missing opacity
   steps to `library/ui/foundations`. Do it with or after follow-up 1, since the shared components
   take many of these colors with them.